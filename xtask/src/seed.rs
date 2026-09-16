//! The seed of a parse: the type names and the template names that a project declares.
//!
//! A seed file holds one row for each name, as `name<TAB>kind`, sorted by the bytes of the name,
//! with `#` on a comment line. The kind is `type`, `template` or `macro`, and `template` implies
//! `type`. The reader builds the C structs of the seed format, and `Seed::as_context` gives the
//! pointer that `Parser::set_scanner_context` takes. The scanner reads the names through that
//! pointer, and the runtime never reads the target.
//!
//! THE IDENTITY OF A SEED IS THE SHA-256 OF THE WHOLE FILE, comments included. The tree of a file
//! is a function of the file AND the seed, so each output that holds a tree also holds the id: the
//! first line of `xtask parse`, a column of each corpus row, and the header of a report. A tree
//! with no id beside it cannot be read again.

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::c_void;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The longest name of a seed, in bytes.
///
/// THE SEED HAS ITS OWN CONSTANT, AND IT IS NOT `MACRO_WORD_SIZE`. The scanner reads a seed name
/// into a buffer of `TS_CPP_SEED_WORD_SIZE` bytes, 65, which src/seed.h declares, and the last byte
/// holds the end of the string. `MACRO_WORD_SIZE` stays 41 and sizes the buffer of a word that
/// compares with a LIST, whose longest word has 39 characters. The two constants belong to two
/// consumers, and a change of one must not move the other: `cut` of src/scanner.c reads
/// `MACRO_WORD_SIZE` and decides whether a word was cut, which the seed does not do.
///
/// `the_longest_name_agrees_with_the_header_of_the_scanner` compares this number with the header,
/// because two constants in two languages drift. The two halves of this task were out of step
/// within an hour of agreeing the number, and a comment did not stop it.
///
/// The cost of the limit, from the collection of task 274. At 64 bytes the collector drops 157
/// names of 594,857, 0.026%, in 22 of the 64 projects. The largest share is stdexec with 24 of
/// 2,236, 1.07%, which writes a diagnostic sentence into a type name:
/// `_A_GET_COMPLETION_SIGNATURES_CUSTOMIZATION_RETURNED_A_TYPE_THAT_IS_NOT_A_COMPLETION_SIGNATURES_SPECIALIZATION`
/// has 109 bytes. Chromium follows with 30 of 17,984, 0.17%. A limit of 128 bytes would hold 24
/// diagnostic tags of one project, and each state of the scanner would carry the larger buffer.
///
/// At 40 bytes the collector dropped 7,629 names, 1.28%, and Vulkan-Hpp alone lost 548 of its
/// 3,111, 17.6%. At 64 bytes Vulkan-Hpp loses 2.
const MAX_NAME: usize = 64;
/// The first four bytes of the seed struct, `TSSD` in the order of the bytes of the machine. The
/// value of a little-endian machine is 0x44535354.
const MAGIC: u32 = u32::from_le_bytes(*b"TSSD");
/// The version of the seed struct.
const VERSION: u32 = 1;
/// The bit of a name that gives a type.
const KIND_TYPE: u16 = 1;
/// The bit of a name that gives a template. A template name is also a type name, so a row of the
/// kind `template` holds the two bits.
const KIND_TEMPLATE: u16 = 2;
/// The bit of a name that a macro defines. #276 reads it, and a reader of #275 ignores it.
const KIND_MACRO: u16 = 4;

/// One name of the seed, the layout of `TSCppSeedEntry`.
#[repr(C)]
#[derive(Clone, Copy)]
struct Entry {
    /// The start of the name in the text block.
    offset: u32,
    /// The length of the name in bytes.
    length: u16,
    /// The kinds of the name, as bit flags.
    kinds: u16,
}

/// The head of the seed, the layout of `TSCppSeed`. The scanner reads it through the context of
/// the parser.
#[repr(C)]
struct Head {
    magic: u32,
    version: u32,
    count: u32,
    text_size: u32,
    entries: *const Entry,
    text: *const u8,
    id: [u8; 32],
}

/// A seed that a parser can take. The head points into the two buffers of this struct, and the
/// buffers do not move while the struct lives.
pub struct Seed {
    /// The entries, sorted by the bytes of the name. The head points at them.
    _entries: Vec<Entry>,
    /// The names, one after the other, with no separator. The head points at them.
    _text: Vec<u8>,
    /// The head. The box keeps its address when the struct moves.
    head: Box<Head>,
    id: [u8; 32],
    names: usize,
    path: PathBuf,
}

// SAFETY: the struct holds no interior mutability, and nothing writes it after `read` gives it.
// The two pointers of the head refer to the heap buffers of this same struct, which live as long
// as it does. So a reference is safe to read from more than one thread.
unsafe impl Send for Seed {}
unsafe impl Sync for Seed {}

impl Seed {
    /// Read a seed file. O(n) in the bytes of the file.
    ///
    /// The reader refuses a file that a later reader cannot binary search: a row that is not
    /// `name<TAB>kind`, a kind that is not one of the three words, a name of more than
    /// `MAX_NAME` bytes, and an order that is not ascending by the bytes of the name.
    pub fn read(path: &Path) -> Result<Self, Box<dyn Error>> {
        let bytes = fs::read(path).map_err(|e| format!("cannot read the seed {}: {e}", path.display()))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|e| format!("the seed {} is not UTF-8: {e}", path.display()))?;
        let mut entries: Vec<Entry> = Vec::new();
        let mut block: Vec<u8> = Vec::new();
        let mut previous = "";
        for (number, line) in text.lines().enumerate() {
            let row = number + 1;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (name, kind) = line.split_once('\t').ok_or_else(|| {
                format!("{}:{row}: the row is `{line}`, and a row of a seed is `name<TAB>kind`", path.display())
            })?;
            if name.is_empty() {
                return Err(format!("{}:{row}: the row has no name", path.display()).into());
            }
            if name.len() > MAX_NAME {
                return Err(format!(
                    "{}:{row}: the name `{name}` has {} bytes, and the scanner compares a maximum of {MAX_NAME}, \
                     which is TS_CPP_SEED_WORD_SIZE of src/seed.h less the end of the string. The collector drops \
                     such a name, so a file that holds one comes from a tool that does not agree with this \
                     reader. The reader refuses the whole file rather than the row, because a seed that \
                     silently loses names gives a parse that looks correct and is not.",
                    path.display(),
                    name.len()
                )
                .into());
            }
            let kinds = match kind {
                "type" => KIND_TYPE,
                "template" => KIND_TYPE | KIND_TEMPLATE,
                "macro" => KIND_MACRO,
                other => {
                    return Err(format!(
                        "{}:{row}: the kind of `{name}` is `{other}`, and a kind is `type`, `template` or `macro`",
                        path.display()
                    )
                    .into());
                }
            };
            if !previous.is_empty() && name.as_bytes() <= previous.as_bytes() {
                return Err(format!(
                    "{}:{row}: `{name}` comes after `{previous}`, and the rows of a seed ascend by the bytes of \
                     the name, with one row for each name. A reader binary searches the file.",
                    path.display()
                )
                .into());
            }
            previous = name;
            entries.push(Entry {
                offset: u32::try_from(block.len()).map_err(|_| format!("the seed {} is too large", path.display()))?,
                length: u16::try_from(name.len()).expect("a name has a maximum of MAX_NAME bytes"),
                kinds,
            });
            block.extend_from_slice(name.as_bytes());
        }
        let id = sha256(&bytes);
        let names = entries.len();
        // The head takes the addresses of the two buffers. A move of this struct moves no heap
        // buffer, so the addresses stay correct.
        let head = Box::new(Head {
            magic: MAGIC,
            version: VERSION,
            count: u32::try_from(names).map_err(|_| format!("the seed {} holds too many names", path.display()))?,
            text_size: u32::try_from(block.len()).map_err(|_| format!("the seed {} is too large", path.display()))?,
            entries: entries.as_ptr(),
            text: block.as_ptr(),
            id,
        });
        Ok(Self {
            _entries: entries,
            _text: block,
            head,
            id,
            names,
            path: path.to_owned(),
        })
    }

    /// The SHA-256 of the file, as 64 lowercase hexadecimal digits.
    pub fn id(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.id {
            write!(out, "{byte:02x}").expect("a write to a String does not fail");
        }
        out
    }

    /// The number of names.
    pub fn names(&self) -> usize {
        self.names
    }

    /// The path of the file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The pointer that `Parser::set_scanner_context` takes. It stays valid while the seed lives.
    pub fn as_context(&self) -> *const c_void {
        std::ptr::from_ref::<Head>(&*self.head).cast::<c_void>()
    }
}

impl std::fmt::Debug for Seed {
    /// The path, the number of names, and the id. The two buffers are large, and no message holds
    /// them.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Seed({}, {} names, {})", self.path.display(), self.names, self.id())
    }
}

/// The seeds of a run: none, one for each file, or one for each project.
pub enum Seeds {
    /// No seed. Each parse takes a null context, which gives the behavior of a parser with none.
    None,
    /// One seed for each file of the run.
    One(Box<Seed>),
    /// One seed for each project, by the first component of the path of a file. A project with no
    /// file in the directory parses with no seed.
    ByProject(BTreeMap<String, Seed>),
}

impl Seeds {
    /// Read one seed file for each project of `paths`, from `directory`, as `<project>.seed`.
    ///
    /// A project with no such file gets no seed, and the report of the caller names it. A silent
    /// fall back to no seed makes a run of one project look like a run of all of them.
    pub fn by_project(directory: &Path, paths: &[&str]) -> Result<Self, Box<dyn Error>> {
        let mut seeds = BTreeMap::new();
        for project in projects(paths) {
            let path = directory.join(format!("{project}.seed"));
            if !path.exists() {
                continue;
            }
            seeds.insert(project, Seed::read(&path)?);
        }
        Ok(Self::ByProject(seeds))
    }

    /// The seed of a file, by the first component of its path.
    pub fn of(&self, rel: &str) -> Option<&Seed> {
        match self {
            Self::None => None,
            Self::One(seed) => Some(seed),
            Self::ByProject(seeds) => seeds.get(project_of(rel)),
        }
    }

    /// The context of a file, or a null pointer for a file with no seed.
    pub fn context(&self, rel: &str) -> *const c_void {
        self.of(rel).map_or(std::ptr::null(), Seed::as_context)
    }

    /// The id of the seed of a file, or `-` for a file with no seed.
    pub fn id(&self, rel: &str) -> String {
        self.of(rel).map_or_else(|| "-".to_owned(), Seed::id)
    }

    /// True when the run has no seed at all.
    pub fn is_none(&self) -> bool {
        match self {
            Self::None => true,
            Self::One(_) => false,
            Self::ByProject(seeds) => seeds.is_empty(),
        }
    }

    /// The report of the seeds of a run: the projects with a seed and the projects without one.
    ///
    /// The line names the seeded projects with their ids, and it counts the projects that parse
    /// with no seed, so that a partial run never reads as a full one.
    pub fn report(&self, paths: &[&str]) -> String {
        match self {
            Self::None => "seed: none. The trees are the trees of a parser with no seed.".to_owned(),
            Self::One(seed) => format!(
                "seed: {} for each file, {} names, id {}",
                seed.path().display(),
                seed.names(),
                seed.id()
            ),
            Self::ByProject(seeds) => {
                let all = projects(paths);
                let missing: Vec<&String> = all.iter().filter(|project| !seeds.contains_key(*project)).collect();
                let mut out = format!(
                    "seed: {} of {} projects, {} names. Each file of the other {} projects parses with no seed.",
                    seeds.len(),
                    all.len(),
                    seeds.values().map(Seed::names).sum::<usize>(),
                    missing.len()
                );
                for (project, seed) in seeds {
                    write!(out, "\n  {project}\t{}\t{}", seed.names(), seed.id())
                        .expect("a write to a String does not fail");
                }
                for project in missing {
                    write!(out, "\n  {project}\tno seed").expect("a write to a String does not fail");
                }
                out
            }
        }
    }
}

/// The first component of a path, which names the project of a corpus file.
fn project_of(rel: &str) -> &str {
    rel.split('/').next().unwrap_or(rel)
}

/// The projects of a list of paths, each one time, in the order of the bytes.
fn projects(paths: &[&str]) -> Vec<String> {
    let mut all: Vec<String> = paths.iter().map(|rel| project_of(rel).to_owned()).collect();
    all.sort_unstable();
    all.dedup();
    all
}

/// The round constants of SHA-256, the first 32 bits of the fractional parts of the cube roots of
/// the first 64 prime numbers. Refer to FIPS 180-4, section 4.2.2.
const ROUND: [u32; 64] = [
    0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1, 0x923f_82a4, 0xab1c_5ed5,
    0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3, 0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174,
    0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
    0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967,
    0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13, 0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85,
    0xa2bf_e8a1, 0xa81a_664b, 0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
    0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3,
    0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208, 0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
];

/// The SHA-256 of bytes, as 32 bytes. O(n) in the bytes. Refer to FIPS 180-4.
///
/// The fork writes the function itself, as it writes FNV-1a in `trees.rs`, so that the identity of
/// a seed needs no dependency. The test compares the published vectors.
fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19,
    ];
    let mut message = bytes.to_vec();
    let length = (bytes.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&length.to_be_bytes());
    for block in message.chunks_exact(64) {
        let mut schedule = [0u32; 64];
        for (word, part) in schedule.iter_mut().zip(block.chunks_exact(4)) {
            *word = u32::from_be_bytes(part.try_into().expect("a part of four bytes"));
        }
        for index in 16..64 {
            let (before15, before2) = (schedule[index - 15], schedule[index - 2]);
            let s0 = before15.rotate_right(7) ^ before15.rotate_right(18) ^ (before15 >> 3);
            let s1 = before2.rotate_right(17) ^ before2.rotate_right(19) ^ (before2 >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for (round, word) in ROUND.iter().zip(&schedule) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let first = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(*round)
                .wrapping_add(*word);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let second = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(first);
            d = c;
            c = b;
            b = a;
            a = first.wrapping_add(second);
        }
        for (value, added) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *value = value.wrapping_add(added);
        }
    }
    let mut out = [0u8; 32];
    for (part, value) in out.chunks_exact_mut(4).zip(state) {
        part.copy_from_slice(&value.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// A seed file in a directory of its own. The directory goes at the end of the test.
    struct File(PathBuf);

    /// The counter of the directories of the tests. Two tests run at the same time, so each file
    /// takes a directory of its own. A shared directory lets the drop of one test remove the file
    /// of another.
    static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    impl File {
        fn new(name: &str, text: &str) -> Self {
            let count = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!("xtask-seed-{}-{count}", std::process::id()));
            fs::create_dir_all(&directory).expect("the directory of the test");
            let path = directory.join(format!("{name}.seed"));
            let mut file = fs::File::create(&path).expect("the file of the test");
            file.write_all(text.as_bytes()).expect("the text of the test");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn directory(&self) -> &Path {
            self.0.parent().expect("the file is in a directory")
        }
    }

    impl Drop for File {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.directory());
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// The published vectors of FIPS 180-4 and of the test suite of the standard.
    #[test]
    fn sha256_agrees_with_the_published_test_vectors() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // A message of more than one block, with the length in the second block.
        assert_eq!(
            hex(&sha256(&[b'a'; 1000])),
            "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
        );
    }

    /// The reader gives the entries in the order of the file, with the kinds of the three words.
    #[test]
    fn the_reader_gives_one_entry_for_each_row() {
        let file = File::new("project", "# a comment\nAaa\ttype\nBbb\ttemplate\nCcc\tmacro\n");
        let seed = Seed::read(file.path()).expect("the seed reads");
        assert_eq!(seed.names(), 3);
        assert_eq!(seed._text, b"AaaBbbCcc");
        let kinds: Vec<u16> = seed._entries.iter().map(|entry| entry.kinds).collect();
        assert_eq!(kinds, vec![KIND_TYPE, KIND_TYPE | KIND_TEMPLATE, KIND_MACRO]);
        let offsets: Vec<(u32, u16)> = seed._entries.iter().map(|entry| (entry.offset, entry.length)).collect();
        assert_eq!(offsets, vec![(0, 3), (3, 3), (6, 3)]);
    }

    /// The id is the hash of the whole file, comments included, and the head holds the same bytes.
    #[test]
    fn the_id_is_the_hash_of_the_whole_file() {
        let text = "# a comment\nAaa\ttype\n";
        let file = File::new("project", text);
        let seed = Seed::read(file.path()).expect("the seed reads");
        assert_eq!(seed.id(), hex(&sha256(text.as_bytes())));
        assert_eq!(seed.head.id, sha256(text.as_bytes()));
        // A comment is part of the file, so it is part of the id.
        let other = File::new("other", "Aaa\ttype\n");
        assert_ne!(Seed::read(other.path()).expect("the seed reads").id(), seed.id());
    }

    /// The head holds the magic, the version and the counts, and it points at the two buffers.
    #[test]
    fn the_head_points_at_the_buffers_of_the_seed() {
        let file = File::new("project", "Aaa\ttype\nBbb\ttype\n");
        let seed = Seed::read(file.path()).expect("the seed reads");
        assert_eq!(seed.head.magic, 0x4453_5354);
        assert_eq!(seed.head.version, VERSION);
        assert_eq!((seed.head.count, seed.head.text_size), (2, 6));
        assert_eq!(seed.head.entries, seed._entries.as_ptr());
        assert_eq!(seed.head.text, seed._text.as_ptr());
        assert_eq!(seed.as_context(), std::ptr::from_ref::<Head>(&*seed.head).cast::<c_void>());
        // A move of the seed keeps the pointers of the head correct.
        let moved = seed;
        assert_eq!(moved.head.entries, moved._entries.as_ptr());
        // SAFETY: the head points at the entries of `moved`, which lives here.
        let first = unsafe { *moved.head.entries };
        assert_eq!((first.offset, first.length), (0, 3));
    }

    /// The reader refuses a file that a binary search cannot read.
    #[test]
    fn the_reader_refuses_a_file_that_it_cannot_search() {
        let message = |name: &str, text: &str| {
            let file = File::new(name, text);
            Seed::read(file.path()).expect_err("the seed does not read").to_string()
        };
        assert!(message("order", "Bbb\ttype\nAaa\ttype\n").contains("ascend"));
        assert!(message("twice", "Aaa\ttype\nAaa\ttype\n").contains("ascend"));
        assert!(message("kind", "Aaa\tvariable\n").contains("`variable`"));
        assert!(message("row", "Aaa type\n").contains("name<TAB>kind"));
        assert!(message("empty", "\ttype\n").contains("no name"));
        assert!(message("long", &format!("{}\ttype\n", "A".repeat(MAX_NAME + 1))).contains("65 bytes"));
        // A name of 64 bytes is the longest that the scanner compares in full.
        let file = File::new("longest", &format!("{}\ttype\n", "A".repeat(MAX_NAME)));
        assert_eq!(Seed::read(file.path()).expect("the seed reads").names(), 1);
    }

    /// `MAX_NAME` and `TS_CPP_SEED_WORD_SIZE` of src/seed.h are one number in two languages.
    ///
    /// The scanner half of task 275 writes src/seed.h. A tree with no such header holds no reader
    /// of the seed either, and the test then makes sure that the scanner names no such constant, so
    /// the skip cannot outlive the condition that gives it. A comment in the place of this test is
    /// what let the two halves disagree within an hour of the decision.
    ///
    /// A HEADER THAT DECLARES A DIFFERENT NAME FAILS, and it does not skip. The name of the
    /// constant itself drifted between the two halves on the same day, from `SEED_WORD_SIZE` to
    /// `TS_CPP_SEED_WORD_SIZE`, and a test that skips for a name it cannot find reports success for
    /// the rest of the life of the project. The search of src/scanner.c reads the name without its
    /// prefix, so it finds the constant under either spelling.
    #[test]
    fn the_longest_name_agrees_with_the_header_of_the_scanner() {
        const NAME: &str = "TS_CPP_SEED_WORD_SIZE";
        let header = crate::repository().join("src").join("seed.h");
        let Ok(text) = fs::read_to_string(&header) else {
            let scanner = fs::read_to_string(crate::repository().join("src").join("scanner.c"))
                .expect("src/scanner.c reads");
            assert!(
                !scanner.contains("SEED_WORD_SIZE"),
                "src/scanner.c names a seed word size and {} is not in the tree, so this test cannot \
                 compare the two constants of the seed",
                header.display()
            );
            return;
        };
        let size: usize = text
            .lines()
            .find_map(|line| {
                let value = line.trim().strip_prefix("#define")?.trim_start().strip_prefix(NAME)?;
                // A define writes at least one blank between the name and the value, and the value
                // can carry a comment after it.
                value.starts_with([' ', '\t']).then(|| value.split_whitespace().next())?
            })
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| {
                panic!(
                    "{} declares no numeric {NAME}. The header is the one declaration of the number, \
                     and a test that cannot read it must fail and never pass in silence.",
                    header.display()
                )
            });
        assert_eq!(
            MAX_NAME,
            size - 1,
            "the reader takes a name of {MAX_NAME} bytes and {NAME} of {} is {size}, so the buffer of \
             the scanner holds a name of {} bytes. The two constants are one number, and the header \
             declares it.",
            header.display(),
            size - 1
        );
    }

    /// A project with no seed file parses with no seed, and the report names it.
    #[test]
    fn a_project_with_no_seed_file_parses_with_no_seed() {
        let file = File::new("alpha", "Aaa\ttype\n");
        let paths = ["alpha/src/a.cpp", "alpha/src/b.cpp", "beta/c.cpp"];
        let seeds = Seeds::by_project(file.directory(), &paths).expect("the seeds read");
        assert!(!seeds.is_none());
        assert!(!seeds.context("alpha/src/a.cpp").is_null());
        assert!(seeds.context("beta/c.cpp").is_null());
        assert_eq!(seeds.id("beta/c.cpp"), "-");
        assert_eq!(seeds.id("alpha/src/a.cpp").len(), 64);
        let report = seeds.report(&paths);
        assert!(report.contains("1 of 2 projects"), "{report}");
        assert!(report.contains("beta\tno seed"), "{report}");
        // A directory with no seed at all gives the behavior of a run with no seed.
        let empty = std::env::temp_dir().join(format!("xtask-seed-{}-none", std::process::id()));
        fs::create_dir_all(&empty).expect("the directory of the test");
        let none = Seeds::by_project(&empty, &paths).expect("the seeds read");
        assert!(none.is_none());
        assert!(none.context("alpha/src/a.cpp").is_null());
        let _ = fs::remove_dir_all(&empty);
    }
}
