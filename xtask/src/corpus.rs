//! Parse a list of files with the C++ grammar, and record the parse errors of each file.
//!
//! The output has one TSV line for each file of the list: the path, the bytes, the ERROR
//! nodes, the MISSING nodes, the bytes in ERROR nodes, the row, kind, and text of the first
//! error, the source line of the first error, the parse time in microseconds, and 1 if the
//! budget stopped the parse.

use std::error::Error;
use std::fs;
use std::io::{BufWriter, Write};
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rayon::prelude::*;
use tree_sitter::{Language, Node, ParseOptions, ParseState, Parser, Tree};

/// The budget of one parse, as a count of the progress callbacks of the runtime.
///
/// DO NOT PUT A TIME IN THE PLACE OF THIS COUNT. A time limit looks simpler and more direct, and
/// this function had one of 20 seconds. A wall-clock limit is not the same in each run: the same
/// bytes and the same parser then give a complete tree alone and a stopped parse under the load of
/// a parallel gate. A parser that gives two answers for the same bytes serves no data flow graph.
///
/// Two files showed the defect. `OpenRCT2/src/openrct2/ride/VehicleSubpositionData.cpp` is a
/// generated file of 6.8 MB. It takes 148,044 callbacks and 2.2 s alone, and it is the largest
/// count of the 329,387 corpus files. `boost/libs/qvm/include/boost/qvm/gen/swizzle4.hpp` is
/// 1.2 MB, and it takes 20,638 callbacks and 7.1 s in a parallel run.
///
/// The old limit stopped the two under load. `xtask trees` then wrote `stopped` in the place of the
/// tree hash, and the gate of a commit that changes only comment lines reported a changed hash. A
/// measurement of the corpus at the load average 954 found 80 files above 20 s, and each of the 80
/// gives a complete tree alone.
///
/// The runtime calls the progress callback one time for each 100 parse operations
/// (`OP_COUNT_PER_PARSER_CALLBACK_CHECK` in vendor/tree-sitter/src/parser.c), so the budget is
/// 200,000,000 operations. The count of one parse is the same on each machine, under each load, and
/// in each run. An input of 84 MB stops at the callback 2,000,001 in each run, and the two runs took
/// 43.6 s and 42.0 s.
///
/// The budget is 13 times the largest count of the corpus files, and the second largest count is
/// 147,695. A smaller budget stops a large file that has no defect, and the reports of the gate then
/// move with the load of the machine.
///
/// THE COUNT BOUNDS THE OPERATIONS OF THE PARSER, AND NOT THE WORK OF THE EXTERNAL SCANNER. A file
/// of 50,000 statements `a b;` takes 11,003 callbacks and 13.8 s. A file of 50,000 statements
/// `x = 1;` takes the same 11,003 callbacks and 0.35 s. The lookahead scans of the scanner are the
/// difference, and no counter of the runtime reads them.
const BUDGET: u64 = 2_000_000;
/// The largest count of progress callbacks of the 329,387 corpus files, for the 6.8 MB generated
/// file `OpenRCT2/src/openrct2/ride/VehicleSubpositionData.cpp`.
const LARGEST_CORPUS_CHECKS: u64 = 148_044;
/// The budget holds ten times the largest count of the corpus files. A build with a smaller budget
/// stops the parse of a file that has no defect, and the reports of the gate then move with the
/// load of the machine.
const _: () = assert!(BUDGET >= 10 * LARGEST_CORPUS_CHECKS);
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

/// Parse a source with no old tree, with the budget `BUDGET`.
///
/// Return None when the budget stopped the parse. The parser is then reset, and it is ready for the
/// next source.
pub fn parse_with_limit(parser: &mut Parser, source: &[u8]) -> Option<Tree> {
    parse_with_budget(parser, source, BUDGET)
}

/// Parse a source with no old tree. Stop the parse after `budget` progress callbacks.
///
/// Return None when the budget stopped the parse. The parser is then reset, and it is ready for the
/// next source. The count of the callbacks of one parse is the same in each run, so the result is
/// the same in each run. Refer to `BUDGET`.
pub fn parse_with_budget(parser: &mut Parser, source: &[u8], budget: u64) -> Option<Tree> {
    let mut checks: u64 = 0;
    let mut stop = |_: &ParseState| {
        checks += 1;
        if checks > budget {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A source with a parse of more than one progress callback.
    fn source_of_many_callbacks() -> String {
        let mut text = String::from("int f() { return 0");
        for index in 0..600 {
            text.push_str(&format!(" + g{index}(1, 2)"));
        }
        text.push_str("; }");
        text
    }

    /// The smallest budget that gives a tree, up to `max`.
    ///
    /// A budget that gives a tree also gives one with each larger budget, so a binary search finds
    /// the smallest budget. O(log n) parses of the source.
    fn smallest_budget(source: &str, max: u64) -> u64 {
        let language = Language::new(tree_sitter_cpp::LANGUAGE);
        let mut parser = new_parser(&language);
        let mut gives_tree = |budget: u64| parse_with_budget(&mut parser, source.as_bytes(), budget).is_some();
        assert!(
            gives_tree(max),
            "the parse of the source takes more than {max} progress callbacks"
        );
        let (mut low, mut high) = (0u64, max);
        while low < high {
            let middle = low + (high - low) / 2;
            if gives_tree(middle) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    }

    /// The budget of a parse counts the progress callbacks of the runtime, and it reads no clock.
    ///
    /// The test fails for a wall-clock limit. The parse of the source takes milliseconds, so each
    /// budget gives a tree with a clock. It also fails when the count of the callbacks of one parse
    /// is not the same in each run, because the two searches then give two values.
    #[test]
    fn the_budget_of_a_parse_counts_the_progress_callbacks_and_reads_no_clock() {
        let source = source_of_many_callbacks();
        let smallest = smallest_budget(&source, 10_000);
        assert!(
            smallest > 1,
            "the source of the test must take more than one progress callback, and it takes {smallest}"
        );
        let language = Language::new(tree_sitter_cpp::LANGUAGE);
        let mut parser = new_parser(&language);
        assert!(
            parse_with_budget(&mut parser, source.as_bytes(), smallest - 1).is_none(),
            "a budget of {} gives a tree, and the smallest budget is {smallest}",
            smallest - 1
        );
        assert!(parse_with_budget(&mut parser, source.as_bytes(), smallest).is_some());
        assert_eq!(
            smallest,
            smallest_budget(&source, 10_000),
            "the count of the progress callbacks of one parse is not the same in each run"
        );
    }
}
