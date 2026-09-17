//! The development tasks of tree-sitter-cpp.
//!
//! Run a task with `cargo xtask TASK`. The alias is in `.cargo/config.toml`.

mod allocation;
mod corpus;
/// The `dedupe` task, which reads the files whose tree depends on the dedupe of the parse stack.
mod dedupe;
/// The names that a source declares as a type or as a value, read from its text with no parse.
mod declarations;
/// The names that the `#define` lines of a source define, which the `seed` task reads.
mod defines;
mod differ;
mod directives;
mod fuzz;
mod generate;
mod limits;
mod parse;
mod precedence;
/// The invariants of the source of `src/scanner.c`. The module holds tests and nothing else.
#[cfg(test)]
mod scanner;
mod seed;
/// The `seed` task, which writes the seed that `seed` reads.
mod seedcollect;
mod sexp;
mod syntax;
mod test;
mod ties;
mod trees;
mod vendor;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask TASK

tasks:
  generate [--check]       Write src/grammar.json from the Rust grammar. Then write src/parser.c,
                           src/node-types.json, and src/tree_sitter/ from src/grammar.json.
                           With --check, compare src/grammar.json with the Rust grammar only.
                           With --diagnostics, print each diagnostic of the generator.
                           Fail when the grammar declares a conflict set that the parse table
                           builder does not use.
  limits [PARSER_C]        Print the use of each fixed-width limit of the parser tables of
                           src/parser.c or PARSER_C. Fail when a use is more than 90% of its limit.
  precedence [PARSER_C]    Print each dynamic precedence of src/grammar.json and the values of the
                           reduce actions of src/parser.c or PARSER_C. Fail when the parse table
                           holds no reduce action with a value that the grammar declares.
  ties table               Print each action list of src/parser.c that holds more than one action,
                           by class and by the rules that the reduce actions reduce.
  ties corpus [ROOT LIST]  Parse each file of LIST with the logger of the runtime, and print each
                           site where the order of the symbol ids selects the tree. Fail for a site
                           that test/ties/baseline.txt does not hold. With --write-baseline before
                           ROOT, write the sites to that file and fail for none. With no ROOT and no
                           LIST, read /tmp/cpp-corpora and test/ties/sample.txt.
  ties population ROOT LIST [--write PATH]
                           Parse each file of LIST, and print the count of the sites of each file
                           and each message. Fail for a count that test/ties/population.txt does not
                           hold. The full corpus takes 27 seconds, and it belongs in a full gate.
                           With --write, also write the measured rows to PATH, in the format of the
                           baseline, so that a landing installs the rows that its gate measured.
  ties versions [ROOT LIST]
                           Print each file of LIST whose GLR version count passes MAX_VERSION_COUNT
                           of the runtime. The condense step then removes a version by arrival
                           order. Fail for a file that test/ties/versions.txt does not hold.
  ties trace FILE          Print the parse log of FILE. With --state N, print only the rows where
                           the parser enters the state N.
  ties flip ROOT LIST --reference BINARY|none [--target DIRECTORY]
                           Build a second parser whose tie comparison is turned around, and parse
                           each file of LIST with this build, with that build, and with the
                           reference. Print the two answers apart: the files whose tree the commits
                           changed, and the files whose tree DEPENDS on the order of the symbol
                           ids. A file of the second group that the commits did not change was
                           order-dependent before them. With --rose TIES_LOG in the place of LIST,
                           read the files of the ROSE rows of a tie report. The command needs a
                           second build of the workspace, and no gate runs it.
  test [--update] [NAME]   Run the corpus tests in test/corpus. With --update, write the actual
                           tree of each failed test that has no error into the test file.
  test --pins [DIRECTORY]  Write the fingerprint of the recorded tree of each example of test/corpus
                           or of DIRECTORY, as the group, the name, and a hash.
  test --pins-compare BEFORE AFTER
                           Compare two outputs of `test --pins`. Print each example whose recorded
                           tree changed, and the counts of the changed, added, and removed examples.
                           A recorded tree is the memory of a decision, and `--update` rewrites it.
  syntax [NAME]            Parse each snippet in test/syntax, and report each parse error.
                           With a NAME, also print the tree of each snippet that it selects.
  parse FILE [--seed SEED] [--text]
                           Print the syntax tree of FILE. With -, read the standard input. Fail
                           when the parse passes the budget or the memory ceiling of `corpus`.
                           With --seed, give the scanner the names of the seed file SEED, and
                           print the id of that seed in the first line. With --text, print one
                           named node for each line, with its field, the text of each leaf, and
                           the line and the column of each node.
  corpus ROOT LIST OUT [--seed FILE | --seeds DIRECTORY]
                           Parse each file of LIST, a list of paths relative to ROOT. Write the
                           parse errors of each file to OUT, one TSV line for each file, with a
                           flag for a parse that the budget stopped, a flag for a parse that the
                           memory ceiling stopped, the peak bytes of the parse, its ceiling, and
                           the id of its seed. With --seed, each file takes the names of FILE.
                           With --seeds, each file takes DIRECTORY/<project>.seed, where the
                           project is the first component of its path, and a project with no such
                           file parses with no seed. The report names each project of the two
                           groups. THE GATE RUNS NO SEED, and a seeded run is a second report.
  dedupe population ROOT LIST [--write PATH]
                           Parse each file of LIST twice, with each preference of the dedupe of the
                           parse stack, and report each file whose tree hash differs. Such a file
                           holds a tree that no rule decides. A file that JOINS the population fails
                           the check. With --write, also write the rows that the run measured.
  dedupe population --write-baseline ROOT LIST
                           Write test/dedupe/population.txt again from a measurement.
  seed collect ROOT LIST (--out FILE | --out-dir DIRECTORY) [--types tree|text] [--scope list|project]
                           [--conflicts all|outer|namespace|methods]
                           Write the names that each project of LIST declares as a type or as a
                           template, and the names that it defines as a macro, as `name<TAB>kinds`
                           rows that ascend by the bytes of the name. The type rows read the
                           declaration positions of the text of each file of LIST, with no parse, or
                           with --types tree the tree of each file. With --scope project, the type
                           rows read every source file of each project. The macro rows read the text
                           of the `#define` lines of every source file under ROOT/<project>, and no
                           tree. A value at namespace scope and a member function remove a type row
                           of the text reader. With --conflicts namespace only a value at namespace
                           scope does, with outer also a data member, and with all each value.
                           With --out-dir, write one <project>.seed for each project. The task reads
                           back what it wrote with the reader of `seed`, and it reports the names
                           that it dropped for passing TS_CPP_SEED_WORD_SIZE - 1 bytes.
  seed check ROOT LIST DIRECTORY [--types tree|text] [--scope list|project] [--conflicts all|outer|namespace|methods]
                           Collect again and compare each <project>.seed of DIRECTORY with the
                           result. Fail on a name added, a name REMOVED, and a kind changed. A seed
                           is the memory of what a project declares, and a change to it changes
                           every parse that loads it.
  seed facts ROOT LIST OUT [--types tree|text] [--scope list|project]
                           Write the facts of each file to OUT: its bytes and hashes, its include
                           lines, and the kinds of each name that it declares. Print the count of the
                           files and of the distinct contents for each extension.
  directives ROOT LIST BASELINE
                           Check that no node outside a directive begins on a line whose first
                           character that is not a blank is `#`. Such a node holds a token that the
                           preprocessor discards. Fail on a site that BASELINE does not hold, and
                           report a site of BASELINE that is gone without a failure.
  trees ROOT LIST OUT [--directives BASELINE] [--seeds DIRECTORY]
                           Parse each file of LIST, and write one TSV line for each file to OUT:
                           the path, 1 if the tree has an error, a hash of the full tree that is
                           the same in each build, the node count, the byte and the kind of the
                           first error, and the hashes of the blocks of the file.
                           With --directives, the same pass also runs the check of `directives` on
                           the tree that it already holds, so the check costs no second parse.
                           With --seeds, each file takes DIRECTORY/<project>.seed as `corpus
                           --seeds` gives it, and each line ends with the id of that seed, or `-`.
                           THE GATE RUNS NO SEED, so a rule that reads a seed changes no tree there,
                           and a seeded base against a seeded head measures the reach of the rule.
  trees --compare A B      Compare two outputs of `trees`. Print the count of files, of files
                           whose hash changed with no error in A, and of files whose hash changed
                           with an error in A. Then print the first 60 paths of the first group,
                           with the byte range of the first node that differs.
  differ ROOT LIST OUT     Compare the trees of the files of LIST with GCC and Clang, and write the
                           disagreements to the directory OUT. `cargo xtask differ --help` gives
                           the modes for the syntax snippets and for one snippet.
  fuzz OUT                 Parse small changes of the syntax snippets, of the inputs of the corpus
                           tests, and of the files of an optional list in child processes. Write each
                           changed input whose parse does not stop in a time limit, stops the process,
                           or gives an incorrect range to the directory OUT. `cargo xtask fuzz --help`
                           gives the options.
  vendor --to VERSION      Put the repairs of the fork on the release VERSION of vendor/tree-sitter
                           and vendor/tree-sitter-generate. The merge uses the pristine sources in
                           vendor/upstream as the base, and it writes the conflict markers of
                           `git merge-file` into each file with a conflict.
  vendor --check           Compare the pristine sources in vendor/upstream with the hashes of the
                           last update.
  identity                 Print the identity of the parser that this binary links: the CRC-32 and
                           the length of src/parser.c and of src/scanner.c, the identity of the whole
                           src directory of its build, and the ABI
                           version of the language. A measurement compares the two values with the
                           files of the tree that it means to measure. A binary that a build did not
                           relink keeps the parser of its last link, and a scan with it measures a
                           different commit.";

/// The directory of the repository: the parent directory of the xtask crate.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the xtask crate is in a directory of the repository")
        .to_owned()
}

/// Print the identity of the parser that this binary links.
///
/// The build script of the crate writes the CRC-32 and the length of `src/parser.c` and of
/// `src/scanner.c` into each binary that links the parser. `cargo build --release` in the tree
/// builds the root package only, so a binary of another package keeps the parser of its last link.
/// A scan with that binary gives whole tables with no error, and it measures a different commit.
/// `bin/identity.py check` compares these values with the files of a tree.
fn identity() -> Result<(), Box<dyn Error>> {
    println!("parser_c {}", tree_sitter_cpp::PARSER_C_IDENTITY);
    println!("scanner_c {}", tree_sitter_cpp::SCANNER_C_IDENTITY);
    println!("src_tree {}", tree_sitter_cpp::SRC_IDENTITY);
    let language = tree_sitter::Language::from(tree_sitter_cpp::LANGUAGE);
    println!("abi_version {}", language.abi_version());
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result: Result<(), Box<dyn Error>> = match args.first().map(String::as_str) {
        Some("generate") => generate::run(&repository(), &args[1..]),
        Some("limits") => limits::run(&repository(), &args[1..]),
        Some("precedence") => precedence::run(&repository(), &args[1..]),
        Some("test") => test::run(&repository(), &args[1..]),
        Some("ties") => ties::run(&repository(), &args[1..]),
        Some("syntax") => syntax::run(&repository(), &args[1..]),
        Some("parse") => parse::run(&args[1..]),
        Some("corpus") => corpus::run(&args[1..]),
        Some("dedupe") => dedupe::run(&repository(), &args[1..]),
        Some("directives") => directives::run(&args[1..]),
        Some("trees") => trees::run(&args[1..]),
        Some("differ") => differ::run(&repository(), &args[1..]),
        Some("fuzz") => fuzz::run(&repository(), &args[1..]),
        Some("seed") => seedcollect::run(&args[1..]),
        Some("vendor") => vendor::run(&repository(), &args[1..]),
        Some("identity") => identity(),
        // The child process of `fuzz`. The usage does not show it.
        Some("fuzz-worker") => fuzz::worker(),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod identity_tests {
    use std::path::Path;

    /// The table of the CRC-32 of each byte value, for the polynomial of IEEE 802.3.
    fn crc_table() -> [u32; 256] {
        let mut table = [0u32; 256];
        for (index, slot) in table.iter_mut().enumerate() {
            let mut value = index as u32;
            for _ in 0..8 {
                let mask = (value & 1).wrapping_neg();
                value = (value >> 1) ^ (0xEDB8_8320 & mask);
            }
            *slot = value;
        }
        table
    }

    /// Add the bytes to a running CRC-32.
    fn crc_bytes(crc: u32, bytes: &[u8], table: &[u32; 256]) -> u32 {
        let mut value = crc;
        for byte in bytes {
            value = (value >> 8) ^ table[((value ^ u32::from(*byte)) & 0xFF) as usize];
        }
        value
    }

    /// The identity of one file: the CRC-32 of its bytes and its length, as `CRC:LENGTH`.
    ///
    /// The function is a second implementation of the one in `bindings/rust/build.rs`. The test
    /// below compares the two over the real sources, so a change of one of them fails the test.
    /// The complexity is O(n) in the size of the file.
    fn identity(path: &Path) -> String {
        let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let crc = crc_bytes(0xFFFF_FFFF, &bytes, &crc_table());
        format!("{:08x}:{}", !crc, bytes.len())
    }

    /// The identity of a directory of sources, as `bindings/rust/build.rs` writes it.
    ///
    /// The files come in the order of their relative paths, and each one adds its path, a zero byte
    /// and its bytes to the running value. The complexity is O(n) in the bytes of the directory.
    fn directory_identity(root: &Path) -> String {
        let mut files = Vec::new();
        let mut stack = vec![root.to_owned()];
        while let Some(directory) = stack.pop() {
            for entry in std::fs::read_dir(&directory).expect("the directory must be readable") {
                let path = entry.expect("the entry must be readable").path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let relative = path.strip_prefix(root).expect("each file is under the root").to_string_lossy().replace('\\', "/");
                    files.push((relative, path));
                }
            }
        }
        files.sort();
        let table = crc_table();
        let mut crc = 0xFFFF_FFFFu32;
        for (relative, path) in &files {
            let bytes = std::fs::read(path).expect("the file must be readable");
            crc = crc_bytes(crc, relative.as_bytes(), &table);
            crc = crc_bytes(crc, b" ", &table);
            crc = crc_bytes(crc, &bytes, &table);
        }
        format!("{:08x}:{}", !crc, files.len())
    }

    /// The identity of the build is the identity of the two C sources of the tree.
    ///
    /// A build that does not relink a binary keeps the parser of its last link, and a scan with that
    /// binary measures a different commit. The check of a measurement compares the value that this
    /// crate holds with the file of a tree, so the value must be the value of the file.
    #[test]
    fn the_identity_of_the_build_is_the_identity_of_the_sources() {
        let root = super::repository();
        assert_eq!(identity(&root.join("src/parser.c")), tree_sitter_cpp::PARSER_C_IDENTITY);
        assert_eq!(identity(&root.join("src/scanner.c")), tree_sitter_cpp::SCANNER_C_IDENTITY);
        assert_eq!(directory_identity(&root.join("src")), tree_sitter_cpp::SRC_IDENTITY);
    }
}
