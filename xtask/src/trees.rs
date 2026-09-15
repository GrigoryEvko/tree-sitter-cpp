//! Parse a list of files with the C++ grammar, and record a stable hash of each full tree.
//!
//! `cargo xtask trees ROOT LIST OUT` writes one TSV line for each file of LIST:
//!
//! 1. The path, relative to ROOT
//! 2. 1 if the tree has an ERROR or a MISSING node, or if the parse gave no tree. 0 if not
//! 3. The hash of the full tree as 16 hexadecimal digits. `stopped` if the time limit stopped the
//!    parse, or `unreadable` if the file is not readable
//! 4. The number of nodes
//! 5. The start byte of the first ERROR or MISSING node, or `-`
//! 6. The kind of that node (`ERROR`, or `MISSING` and the kind of the missing node), or `-`
//! 7. The block hashes: the block size in bytes, a colon, and one hash for each block. The hash of
//!    block K is the high 32 bits of the tree hash after all the nodes that start before the end of
//!    block K.
//!
//! The tree hash is FNV-1a. For each node in document order, it mixes the depth, the kind name, the
//! named, MISSING, and extra flags, the field name, the start byte, and the end byte. The hash does
//! not use the symbol numbers of the grammar, a random key, or an address. Two builds with
//! different grammars or runtimes give the same hash for the same tree.
//!
//! `cargo xtask trees --compare A.tsv B.tsv` compares two outputs of the same file list. It prints
//! the counts of files and of changed hashes, and the first paths whose hash changed with no error
//! in A. The start of a node does not decrease in document order. The first block hash that
//! differs gives the byte range that contains the start of the first node that differs.

use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write};
use std::num::NonZeroU16;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rayon::prelude::*;
use tree_sitter::{Language, Node, Tree};

use crate::corpus::{new_parser, parse_with_limit};

const USAGE: &str = "usage: cargo xtask trees ROOT LIST OUT | cargo xtask trees --compare A.tsv B.tsv";
/// The offset basis of the 64-bit FNV-1a hash.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// The prime of the 64-bit FNV-1a hash.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
/// The minimum size of a block of the block hashes.
const MIN_BLOCK: usize = 4096;
/// The maximum number of blocks of one file. A larger file gets larger blocks.
const MAX_BLOCKS: usize = 32;
/// The compare prints this number of paths whose hash changed with no error in A.
const LISTED_PATHS: usize = 60;

/// A 64-bit FNV-1a hash. The value is the same in each process, build, and machine.
#[derive(Clone, Copy)]
struct Fnv(u64);

impl Fnv {
    /// A hash of no bytes.
    const fn new() -> Self {
        Self(FNV_OFFSET)
    }

    /// Mix bytes into the hash. O(n) in the bytes.
    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }

    /// Mix the 8 little-endian bytes of a value into the hash.
    fn word(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }
}

/// The FNV-1a hash of a name and of a 0xff byte. UTF-8 text has no 0xff byte, so that two
/// sequences of names that make the same text give different hashes.
fn name_hash(name: &str) -> u64 {
    let mut hash = Fnv::new();
    hash.bytes(name.as_bytes());
    hash.bytes(&[0xff]);
    hash.0
}

/// The name hashes of the node kinds and the field names of a language, by symbol and field id.
struct Names {
    kinds: Vec<u64>,
    fields: Vec<u64>,
}

impl Names {
    /// Hash each kind name and each field name of a language. O(n) in the names.
    fn new(language: &Language) -> Self {
        let kinds = (0..language.node_kind_count())
            .map(|id| {
                u16::try_from(id)
                    .ok()
                    .and_then(|id| language.node_kind_for_id(id))
                    .map_or(0, name_hash)
            })
            .collect();
        let fields = (0..=language.field_count())
            .map(|id| {
                u16::try_from(id)
                    .ok()
                    .and_then(|id| language.field_name_for_id(id))
                    .map_or(0, name_hash)
            })
            .collect();
        Self { kinds, fields }
    }

    /// The hash of the kind name of a node. The ERROR symbol is not in the table of the language.
    fn kind(&self, node: Node) -> u64 {
        self.kinds
            .get(usize::from(node.kind_id()))
            .copied()
            .unwrap_or_else(|| name_hash(node.kind()))
    }

    /// The hash of a field name, or 0 for a node with no field name.
    fn field(&self, field: Option<NonZeroU16>) -> u64 {
        field.map_or(0, |id| self.fields.get(usize::from(id.get())).copied().unwrap_or(0))
    }
}

/// The hash facts of one full tree.
#[derive(Debug, PartialEq, Eq)]
struct TreeFacts {
    hash: u64,
    nodes: u64,
    /// The start byte and the kind of the first ERROR or MISSING node in document order.
    first_error: Option<(usize, String)>,
    block: usize,
    /// The high 32 bits of the hash after the nodes that start before the end of each block.
    blocks: Vec<u32>,
}

/// The block size for a source: a power of two, at least `MIN_BLOCK`, with a maximum of
/// `MAX_BLOCKS` blocks for the source.
fn block_size(bytes: usize) -> usize {
    let mut block = MIN_BLOCK;
    while bytes / block >= MAX_BLOCKS {
        block *= 2;
    }
    block
}

/// Hash each node of a tree in document order, with a cursor and no recursion.
///
/// `bytes` is the length of the source. It sets the blocks: block K covers the bytes from
/// K * block to (K + 1) * block. O(n) in the nodes of the tree.
fn tree_facts(tree: &Tree, names: &Names, bytes: usize) -> TreeFacts {
    let block = block_size(bytes);
    let count = bytes / block + 1;
    let mut blocks = Vec::with_capacity(count);
    let mut hash = Fnv::new();
    let mut nodes = 0u64;
    let mut first_error = None;
    let mut depth = 0u64;
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        let start = node.start_byte();
        while blocks.len() + 1 < count && start >= (blocks.len() + 1) * block {
            blocks.push((hash.0 >> 32) as u32);
        }
        let flags =
            u64::from(node.is_named()) | (u64::from(node.is_missing()) << 1) | (u64::from(node.is_extra()) << 2);
        hash.word(depth);
        hash.word(names.kind(node));
        hash.word(flags);
        hash.word(names.field(cursor.field_id()));
        hash.word(start as u64);
        hash.word(node.end_byte() as u64);
        nodes += 1;
        if first_error.is_none() && (node.is_error() || node.is_missing()) {
            let kind = if node.is_error() {
                "ERROR".to_owned()
            } else {
                format!("MISSING {}", node.kind())
            };
            first_error = Some((start, kind));
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                while blocks.len() < count {
                    blocks.push((hash.0 >> 32) as u32);
                }
                return TreeFacts {
                    hash: hash.0,
                    nodes,
                    first_error,
                    block,
                    blocks,
                };
            }
            depth -= 1;
        }
    }
}

/// Parse one file, and give its TSV line without the line end.
fn tree_line(parser: &mut tree_sitter::Parser, names: &Names, root: &Path, rel: &str) -> String {
    let Ok(source) = fs::read(root.join(rel)) else {
        return format!("{rel}\t1\tunreadable\t0\t-\t-\t-");
    };
    let Some(tree) = parse_with_limit(parser, &source) else {
        return format!("{rel}\t1\tstopped\t0\t-\t-\t-");
    };
    let facts = tree_facts(&tree, names, source.len());
    let (error_byte, error_kind) = match &facts.first_error {
        Some((byte, kind)) => (byte.to_string(), kind.escape_debug().to_string()),
        None => ("-".to_owned(), "-".to_owned()),
    };
    let mut blocks = format!("{}:", facts.block);
    for (index, value) in facts.blocks.iter().enumerate() {
        if index > 0 {
            blocks.push(',');
        }
        write!(blocks, "{value:08x}").expect("a write to a String does not fail");
    }
    format!(
        "{rel}\t{}\t{:016x}\t{}\t{error_byte}\t{error_kind}\t{blocks}",
        u8::from(tree.root_node().has_error()),
        facts.hash,
        facts.nodes
    )
}

/// Parse the files of a list in parallel, and write one TSV line for each file.
fn write_trees(root: &str, list: &str, out: &str) -> Result<(), Box<dyn Error>> {
    let root = Path::new(root);
    let list = fs::read_to_string(list).map_err(|e| format!("cannot read the file list {list}: {e}"))?;
    let paths: Vec<&str> = list.lines().filter(|line| !line.is_empty()).collect();
    let language = Language::new(tree_sitter_cpp::LANGUAGE);
    let names = Names::new(&language);
    let done = AtomicUsize::new(0);
    let started = Instant::now();
    let lines: Vec<String> = paths
        .par_iter()
        .map_init(
            || new_parser(&language),
            |parser, rel| {
                let line = tree_line(parser, &names, root, rel);
                let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                if count.is_multiple_of(100_000) {
                    eprintln!("{count} files in {:.0} s", started.elapsed().as_secs_f64());
                }
                line
            },
        )
        .collect();

    let mut sink = BufWriter::new(fs::File::create(out).map_err(|e| format!("cannot create {out}: {e}"))?);
    for line in &lines {
        writeln!(sink, "{line}")?;
    }
    sink.flush()?;

    let column = |line: &String, index: usize| line.split('\t').nth(index).map(str::to_owned).unwrap_or_default();
    let failed = lines.iter().filter(|line| column(line, 1) == "1").count();
    let stopped = lines.iter().filter(|line| column(line, 2) == "stopped").count();
    let unreadable = lines.iter().filter(|line| column(line, 2) == "unreadable").count();
    println!(
        "files {}  with an error {failed}  stopped {stopped}  unreadable {unreadable}  wall {:.0} s",
        lines.len(),
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// One line of an output of `xtask trees`.
struct Row<'a> {
    path: &'a str,
    error: bool,
    hash: &'a str,
    blocks: &'a str,
}

/// Read the lines of an output of `xtask trees`. O(n) in the bytes of the text.
fn read_rows<'a>(name: &str, text: &'a str) -> Result<Vec<Row<'a>>, Box<dyn Error>> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.is_empty())
        .map(|(index, line)| {
            let columns: Vec<&str> = line.split('\t').collect();
            let [path, error, hash, _nodes, _byte, _kind, blocks] = columns[..] else {
                return Err(format!(
                    "{name}:{}: the line has {} columns, and an output of `xtask trees` has 7",
                    index + 1,
                    columns.len()
                )
                .into());
            };
            Ok(Row {
                path,
                error: error == "1",
                hash,
                blocks,
            })
        })
        .collect()
}

/// The byte range of the first block whose hash differs, as `START-END`. None when the block
/// sizes or the block counts differ, or when no block hash differs.
fn first_different_block(a: &str, b: &str) -> Option<String> {
    let (size_a, hashes_a) = a.split_once(':')?;
    let (size_b, hashes_b) = b.split_once(':')?;
    let size: usize = size_a.parse().ok()?;
    let (hashes_a, hashes_b): (Vec<&str>, Vec<&str>) = (hashes_a.split(',').collect(), hashes_b.split(',').collect());
    if size_a != size_b || hashes_a.len() != hashes_b.len() {
        return None;
    }
    let index = hashes_a.iter().zip(&hashes_b).position(|(x, y)| x != y)?;
    Some(format!("{}-{}", index * size, (index + 1) * size))
}

/// Compare two outputs of `xtask trees`, and print the counts and the first paths whose hash
/// changed with no error in A. O(n) in the lines.
fn compare(path_a: &str, path_b: &str) -> Result<(), Box<dyn Error>> {
    let text_a = fs::read_to_string(path_a).map_err(|e| format!("cannot read {path_a}: {e}"))?;
    let text_b = fs::read_to_string(path_b).map_err(|e| format!("cannot read {path_b}: {e}"))?;
    let rows_a = read_rows(path_a, &text_a)?;
    let rows_b = read_rows(path_b, &text_b)?;
    let by_path: HashMap<&str, &Row> = rows_b.iter().map(|row| (row.path, row)).collect();

    let mut files = 0usize;
    let mut changed_clean: Vec<(&Row, &Row)> = Vec::new();
    let mut changed_error = 0usize;
    for a in &rows_a {
        let Some(b) = by_path.get(a.path) else { continue };
        files += 1;
        if a.hash == b.hash {
            continue;
        }
        if a.error {
            changed_error += 1;
        } else {
            changed_clean.push((a, b));
        }
    }
    let paths_a: std::collections::HashSet<&str> = rows_a.iter().map(|row| row.path).collect();
    let only_b = rows_b.iter().filter(|row| !paths_a.contains(row.path)).count();

    println!("files\t{files}");
    println!("changed hash, no error in A\t{}", changed_clean.len());
    println!("changed hash, error in A\t{changed_error}");
    println!("only in A\t{}", rows_a.len() - files);
    println!("only in B\t{only_b}");
    if !changed_clean.is_empty() {
        println!();
        println!("path\terror in B\tbytes of the first different node");
    }
    for (a, b) in changed_clean.iter().take(LISTED_PATHS) {
        println!(
            "{}\t{}\t{}",
            a.path,
            u8::from(b.error),
            first_different_block(a.blocks, b.blocks).unwrap_or_else(|| "-".to_owned())
        );
    }
    Ok(())
}

/// Run `xtask trees ROOT LIST OUT` or `xtask trees --compare A.tsv B.tsv`.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args {
        [flag, a, b] if flag == "--compare" => compare(a, b),
        [root, list, out] if !root.starts_with("--") => write_trees(root, list, out),
        _ => Err(USAGE.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The facts of the tree of a source.
    fn facts_of(source: &str) -> TreeFacts {
        let language = Language::new(tree_sitter_cpp::LANGUAGE);
        let mut parser = new_parser(&language);
        let tree = parse_with_limit(&mut parser, source.as_bytes()).expect("the parse of a small source ends");
        tree_facts(&tree, &Names::new(&language), source.len())
    }

    #[test]
    fn fnv_agrees_with_the_published_test_vectors() {
        let hash = |text: &str| {
            let mut fnv = Fnv::new();
            fnv.bytes(text.as_bytes());
            fnv.0
        };
        assert_eq!(hash(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(hash("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(hash("foobar"), 0x8594_4171_f739_67e8);
    }

    /// The hash of a small tree is a fixed value. If the tree of the snippet changes, write the
    /// new value here.
    #[test]
    fn the_hash_of_a_small_tree_is_stable() {
        let facts = facts_of("int x = f(1);\n");
        assert_eq!(facts.first_error, None);
        assert_eq!(facts.block, MIN_BLOCK);
        assert_eq!(facts.blocks, vec![(facts.hash >> 32) as u32]);
        assert_eq!(
            (facts.nodes, format!("{:016x}", facts.hash)),
            (13, "3a5c9e3e6952739b".to_owned())
        );
    }

    /// The hash reads the kinds, the flags, the fields, and the ranges of the nodes, not the text.
    #[test]
    fn different_trees_give_different_hashes() {
        let hash = |source: &str| facts_of(source).hash;
        assert_eq!(facts_of("int x = f(1);\n"), facts_of("int x = f(1);\n"));
        assert_eq!(hash("int x = f(1);\n"), hash("int y = g(2);\n"));
        assert_ne!(hash("int x = f(1);\n"), hash("int x = f(12);\n"));
        assert_ne!(hash("int x = (y) + 1;\n"), hash("int x = (y) - 1;\n"));
        assert_ne!(hash("int x = (y) + 1;\n"), hash("int x = (y) *(1);\n"));
        assert_ne!(hash("a = b;\n"), hash("a . b;\n"));
    }

    #[test]
    fn the_first_error_is_the_first_error_node_in_document_order() {
        let facts = facts_of("int x = ;\nint y = f(\n");
        let (byte, kind) = facts.first_error.expect("the source has an error");
        assert!(byte <= 8, "the first error starts at byte {byte}");
        assert!(kind == "ERROR" || kind.starts_with("MISSING "), "the kind is {kind}");
    }

    #[test]
    fn the_block_hashes_find_the_first_different_block() {
        let lines = "int a;\n".repeat(1000);
        let changed = format!("{}int*a;\n", "int a;\n".repeat(999));
        let (a, b) = (facts_of(&lines), facts_of(&changed));
        assert_eq!((a.block, a.blocks.len()), (MIN_BLOCK, 2));
        let text = |facts: &TreeFacts| {
            let hashes: Vec<String> = facts.blocks.iter().map(|value| format!("{value:08x}")).collect();
            format!("{}:{}", facts.block, hashes.join(","))
        };
        assert_eq!(
            first_different_block(&text(&a), &text(&b)).as_deref(),
            Some("4096-8192")
        );
        assert_eq!(first_different_block(&text(&a), &text(&a)), None);
    }

    #[test]
    fn the_block_size_gives_a_maximum_of_32_blocks() {
        assert_eq!(block_size(0), MIN_BLOCK);
        assert_eq!(block_size(MIN_BLOCK * MAX_BLOCKS - 1), MIN_BLOCK);
        assert_eq!(block_size(MIN_BLOCK * MAX_BLOCKS), 2 * MIN_BLOCK);
        let bytes = 14_000_000;
        assert!(bytes / block_size(bytes) < MAX_BLOCKS);
    }
}
