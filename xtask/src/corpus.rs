//! Parse a list of files with the C++ grammar, and record the parse errors of each file.
//!
//! The output has one TSV line for each file of the list: the path, the bytes, the ERROR
//! nodes, the MISSING nodes, the bytes in ERROR nodes, the row, kind, and text of the first
//! error, the source line of the first error, the parse time in microseconds, and 1 if the
//! time limit stopped the parse.

use std::error::Error;
use std::fs;
use std::io::{BufWriter, Write};
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rayon::prelude::*;
use tree_sitter::{Language, Node, ParseOptions, ParseState, Parser, Tree};

/// The parse of one file stops after this time.
const LIMIT: Duration = Duration::from_secs(20);
/// The maximum length of the source line of the first error.
const LINE_BYTES: usize = 120;
/// The maximum length of the text of the first error.
const WHAT_BYTES: usize = 40;

/// The first ERROR or MISSING node of a file.
struct FirstError {
    byte: usize,
    row: usize,
    kind: String,
    what: String,
    line: String,
}

/// The parse errors of one file.
#[derive(Default)]
struct Record {
    bytes: usize,
    errors: u32,
    missing: u32,
    error_bytes: usize,
    first: Option<FirstError>,
    parse_us: u128,
    stopped: bool,
    unreadable: bool,
}

impl Record {
    /// True when the parse gave no tree, or a tree with an ERROR or MISSING node.
    fn failed(&self) -> bool {
        self.stopped || self.errors + self.missing > 0
    }
}

/// Cut a string to a maximum number of bytes, at a character boundary.
fn truncate(text: &mut String, max: usize) {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
}

/// The start of the text of a node, with each run of white space as one space.
fn what(node: Node, source: &[u8]) -> String {
    let end = node.end_byte().min(node.start_byte() + 4 * WHAT_BYTES);
    let text = String::from_utf8_lossy(&source[node.start_byte()..end]);
    let mut out = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&mut out, WHAT_BYTES);
    out
}

/// The trimmed source line that contains a byte.
fn line_at(source: &[u8], byte: usize) -> String {
    let start = source[..byte].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let end = source[byte..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(source.len(), |i| byte + i);
    let mut line = String::from_utf8_lossy(&source[start..end]).trim().replace('\t', " ");
    truncate(&mut line, LINE_BYTES);
    line
}

/// Count the ERROR and MISSING nodes of a tree, and find the first one.
///
/// The walk goes only into subtrees that have an error, and it does not go into an ERROR
/// node. The bytes of nested errors count one time. O(n) in the number of visited nodes.
fn scan(root: Node, source: &[u8], record: &mut Record) {
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        let is_error = node.is_error();
        if is_error || node.is_missing() {
            if is_error {
                record.errors += 1;
                record.error_bytes += node.end_byte() - node.start_byte();
            } else {
                record.missing += 1;
            }
            if record.first.as_ref().is_none_or(|first| node.start_byte() < first.byte) {
                record.first = Some(FirstError {
                    byte: node.start_byte(),
                    row: node.start_position().row + 1,
                    kind: if is_error {
                        "ERROR".to_owned()
                    } else {
                        format!("MISSING {}", node.kind())
                    },
                    what: if is_error { what(node, source) } else { String::new() },
                    line: line_at(source, node.start_byte()),
                });
            }
        }
        if !is_error && node.has_error() && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

/// Parse a source with no old tree. Stop the parse after the time limit.
///
/// Return None when the time limit stopped the parse. The parser is then reset, and it is ready
/// for the next source.
pub fn parse_with_limit(parser: &mut Parser, source: &[u8]) -> Option<Tree> {
    let started = Instant::now();
    let mut stop = |_: &ParseState| {
        if started.elapsed() > LIMIT {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let tree = parser.parse_with_options(
        &mut |at, _| source.get(at..).unwrap_or_default(),
        None,
        Some(ParseOptions::new().progress_callback(&mut stop)),
    );
    if tree.is_none() {
        parser.reset();
    }
    tree
}

/// A parser for the C++ language of the repository.
pub fn new_parser(language: &Language) -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .expect("the ABI of the grammar must agree with the tree-sitter runtime");
    parser
}

/// Parse one file and record its errors.
fn probe(parser: &mut Parser, root: &Path, rel: &str) -> Record {
    let mut record = Record::default();
    let Ok(source) = fs::read(root.join(rel)) else {
        record.unreadable = true;
        return record;
    };
    record.bytes = source.len();
    let started = Instant::now();
    let tree = parse_with_limit(parser, &source);
    record.parse_us = started.elapsed().as_micros();
    match tree {
        Some(tree) => scan(tree.root_node(), &source, &mut record),
        None => record.stopped = true,
    }
    record
}

/// Parse the files of a list in parallel, write one TSV line for each, and print a summary.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let [root, list, out] = args else {
        return Err("usage: cargo xtask corpus ROOT LIST OUT".into());
    };
    let root = Path::new(root);
    let list = fs::read_to_string(list).map_err(|e| format!("cannot read the file list {list}: {e}"))?;
    let paths: Vec<&str> = list.lines().filter(|line| !line.is_empty()).collect();
    let language = Language::new(tree_sitter_cpp::LANGUAGE);
    let done = AtomicUsize::new(0);
    let started = Instant::now();
    let records: Vec<Record> = paths
        .par_iter()
        .map_init(
            || new_parser(&language),
            |parser, rel| {
                let record = probe(parser, root, rel);
                let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                if count.is_multiple_of(100_000) {
                    eprintln!("{count} files in {:.0} s", started.elapsed().as_secs_f64());
                }
                record
            },
        )
        .collect();

    let mut sink = BufWriter::new(fs::File::create(out).map_err(|e| format!("cannot create {out}: {e}"))?);
    for (rel, r) in paths.iter().zip(&records) {
        let (row, kind, what, line) = match &r.first {
            Some(f) => (f.row, f.kind.as_str(), f.what.as_str(), f.line.as_str()),
            None => (0, "", "", ""),
        };
        writeln!(
            sink,
            "{rel}\t{}\t{}\t{}\t{}\t{row}\t{kind}\t{what}\t{line}\t{}\t{}",
            r.bytes,
            r.errors,
            r.missing,
            r.error_bytes,
            r.parse_us,
            u8::from(r.stopped)
        )?;
    }
    sink.flush()?;

    let files = records.iter().filter(|r| !r.unreadable).count();
    let failed = records.iter().filter(|r| r.failed()).count();
    let bytes: usize = records.iter().map(|r| r.bytes).sum();
    let error_bytes: usize = records.iter().map(|r| r.error_bytes).sum();
    let stopped = records.iter().filter(|r| r.stopped).count();
    println!(
        "files {files}  with an error {failed} ({:.3}%)  error bytes {:.3}%  stopped {stopped}  unreadable {}  wall {:.0} s",
        100.0 * failed as f64 / files.max(1) as f64,
        100.0 * error_bytes as f64 / bytes.max(1) as f64,
        records.len() - files,
        started.elapsed().as_secs_f64(),
    );
    Ok(())
}
