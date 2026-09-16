//! Parse a list of files with the C++ grammar, and record the parse errors of each file.
//!
//! The output has one TSV line for each file of the list: the path, the bytes, the ERROR
//! nodes, the MISSING nodes, the bytes in ERROR nodes, the row, kind, and text of the first
//! error, the source line of the first error, the parse time in microseconds, 1 if the
//! budget stopped the parse, 1 if the memory ceiling stopped the parse, the peak bytes that
//! the parse allocated, the ceiling of the parse in bytes, and the id of the seed of the file or
//! `-`. The peak and the ceiling show how near each file comes to its ceiling, so that a change of
//! `CEILING_FLOOR` or of `CEILING_PER_BYTE` is a query of the rows and not a second run.
//!
//! THE TREE OF A FILE IS A FUNCTION OF THE FILE AND OF THE SEED. The id column holds the seed of
//! each row, so that no row of a seeded run reads as a row of a run with no seed. Refer to `seed`.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{BufWriter, Write};
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rayon::prelude::*;
use tree_sitter::{Language, Node, ParseOptions, ParseState, Parser, Tree};

use crate::allocation;
use crate::seed::{Seed, Seeds};

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
/// The ceiling of one parse, as the peak of the net bytes that it allocates, for a source of no
/// bytes. `ceiling` adds `CEILING_PER_BYTE` for each byte of the source.
///
/// THE CEILING GROWS WITH THE SOURCE, BECAUSE MANY THREADS PARSE AT THE SAME TIME. A corpus run
/// holds one thread for each core, 376 on the machine of 2026-09-16. A fixed ceiling with headroom
/// over the largest file gives each thread that ceiling, and a defect that makes many files run
/// away then takes 376 times it before the first stop: 1.5 TB for 4 GiB. The peak of a correct
/// parse grows with the source, so a ceiling that grows with the source keeps its headroom, and it
/// bounds 376 threads on files of the median size to 24 GiB.
///
/// The measurement, on the full corpus of 329,387 files, on 2026-09-16, with the count of
/// vendor/tree-sitter/src/alloc.c:
/// - An empty file peaks at 1,960 bytes.
/// - The largest peak of a file of 4 KB or less is 1,571,088 bytes
///   (llvm-project/clang/test/Index/index-many-call-ops.cpp).
/// - The largest peak of a file with no error is 663,825,512 bytes
///   (OpenRCT2/src/openrct2/ride/VehicleSubpositionData.cpp, 6,798,922 bytes).
/// - The largest peak of all is 806,867,464 bytes
///   (qtbase/src/corelib/time/qtimezonelocale_data_p.h, 13,995,652 bytes, 711 errors).
/// - The smallest headroom of the ceiling over the corpus is 4.80 times, at
///   duckdb/third_party/brotli/enc/dictionary_hash.cpp (147,080 bytes, peak 29,644,824 bytes).
/// - The one parse that took 152 GB on 2026-09-16 read a file of 19,912 bytes, whose ceiling is
///   77,303,808 bytes.
///
/// THE PEAK OF ONE FILE MOVES BETWEEN TWO RUNS, AND THE HEADROOM COVERS THE MOVE.
/// The parser of a corpus thread keeps the capacity of its arrays from one file to the next, and
/// an allocation before the reset counts for no file. So the peak of a file depends on the files
/// that the same thread read before it. Two full runs of 2026-09-16 gave:
/// - The largest peak of all: 806,867,528 and 806,867,240 bytes.
/// - The largest difference in absolute terms: 171,776 bytes, at
///   llvm-project/clang/test/OpenMP/distribute_parallel_for_codegen.cpp, 21,695,704 against
///   21,523,928, which is 3.84% of the ceiling of that file.
/// - The largest difference in relative terms: 3,688%, at
///   llvm-project/clang/test/Driver/darwin-header-search-libcxx-2.cpp, 904 bytes against 34,240,
///   which is 0.05% of the ceiling of that file. A small file holds a large relative move and a
///   trivial absolute one, because its own allocations are a few hundred bytes.
///
/// So the move never comes near a ceiling, and the summary of the run names the file that does.
///
/// THE PEAK OF A FILE IS NOT A PROPERTY OF THAT FILE ALONE. It is the property of that file parsed
/// after the files that the same thread read before it, because the parser keeps the capacity of
/// its arrays. So a run with a different file order, a different number of threads, or a different
/// assignment of the files to the threads can name a DIFFERENT file as the nearest to its ceiling,
/// with no change of the parser at all. A reader who sees that name change must not look for a
/// regression of the grammar. The percent is what matters, and the corpus of 2026-09-16 stands at
/// 20.8% at most.
///
/// The count also reads the size of each block from the C library, and the library gives a block of
/// a different size for the same request when it splits a free block: 32 bytes of 808,704 in the
/// unit test. A count of the requested sizes would be exact, but it needs a header in each block,
/// which costs 16 bytes for each block and breaks a block that a plain `free` releases.
///
/// THE CEILING COUNTS THE BYTES THAT THE RUNTIME ALLOCATES, AND NOT THE VIRTUAL MEMORY OF THE
/// PROCESS. `ulimit -v` bounds the virtual size, and a corpus run reserves 64 MB of virtual memory
/// for the heap of each of its threads, so a `ulimit -v` that fits one parse does not fit the
/// corpus command. The count of the allocator has no such term.
///
/// THE CEILING GUARDS THE COMMANDS OF XTASK, AND NOT A CALLER THAT BYPASSES THEM. A script that
/// runs a parse with no ceiling gets no protection from this constant, and the parse that took
/// 152 GB came from such a script, with a parser and a scanner from two generate steps. The count
/// of the external tokens in `tree_sitter_cpp_external_scanner_create` refuses that pair. A loop
/// over files must go through `xtask corpus`.
pub const CEILING_FLOOR: u64 = 64 << 20;
/// The bytes of ceiling for each byte of the source. Refer to `CEILING_FLOOR`.
pub const CEILING_PER_BYTE: u64 = 512;
/// The peak and the size of the corpus file with the smallest headroom under the ceiling, on
/// 2026-09-16: duckdb/third_party/brotli/enc/dictionary_hash.cpp.
const SMALLEST_HEADROOM_PEAK: u64 = 29_644_824;
const SMALLEST_HEADROOM_BYTES: usize = 147_080;
/// The ceiling of each corpus file holds four times its peak. A build with a smaller ceiling
/// stops the parse of a file that has no defect.
const _: () = assert!(ceiling(SMALLEST_HEADROOM_BYTES) >= 4 * SMALLEST_HEADROOM_PEAK);

/// The ceiling of a parse of a source of `bytes` bytes. Refer to `CEILING_FLOOR`.
pub const fn ceiling(bytes: usize) -> u64 {
    CEILING_FLOOR + CEILING_PER_BYTE * bytes as u64
}
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
    memory: bool,
    peak_bytes: u64,
    ceiling: u64,
    unreadable: bool,
}

impl Record {
    /// True when the parse gave no tree, or a tree with an ERROR or MISSING node.
    fn failed(&self) -> bool {
        self.stopped || self.memory || self.errors + self.missing > 0
    }
}

/// The limit that stopped a parse, with its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The parse passed this count of progress callbacks. Refer to `BUDGET`.
    Budget(u64),
    /// The allocations of the parse passed this count of bytes. Refer to `CEILING`.
    Memory(u64),
}

impl Stop {
    /// The word of the stop in a TSV row: `stopped` for the budget, and `memory` for the ceiling.
    pub fn word(self) -> &'static str {
        match self {
            Self::Budget(_) => "stopped",
            Self::Memory(_) => "memory",
        }
    }
}

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(budget) => write!(f, "the budget of {budget} progress callbacks"),
            Self::Memory(ceiling) => write!(f, "the memory ceiling of {ceiling} bytes"),
        }
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

/// Parse a source with no old tree, with the budget `BUDGET` and the ceiling of `ceiling`.
///
/// `label` names the source in the message of the cap of the runtime. Return the limit when one
/// stopped the parse. The parser is then reset, and it is ready for the next source.
pub fn parse_with_limit(parser: &mut Parser, source: &[u8], label: &str) -> Result<Tree, Stop> {
    parse_with_limits(parser, source, label, BUDGET, ceiling(source.len()))
}

/// Parse a source with no old tree. Stop the parse after `budget` progress callbacks. Refer to
/// `BUDGET`.
#[cfg(test)]
fn parse_with_budget(parser: &mut Parser, source: &[u8], budget: u64) -> Result<Tree, Stop> {
    parse_with_limits(parser, source, "a source with a budget", budget, ceiling(source.len()))
}

/// Parse a source with no old tree. Stop the parse when its allocations pass `ceiling` bytes.
/// Refer to `CEILING_FLOOR`.
#[cfg(test)]
fn parse_with_ceiling(parser: &mut Parser, source: &[u8], ceiling: u64) -> Result<Tree, Stop> {
    parse_with_limits(parser, source, "a source with a ceiling", BUDGET, ceiling)
}

/// Parse a source with no old tree. Stop the parse after `budget` progress callbacks, or when the
/// peak of its allocations passes `ceiling` bytes.
///
/// The progress callback of the runtime reads the two limits, one time for each 100 parse
/// operations. The count of the callbacks of one parse is the same in each run. The peak of the
/// bytes moves by a small fraction, and the headroom of the ceiling covers it. Refer to `BUDGET`
/// and to `CEILING_FLOOR`.
///
/// Return the limit when one stopped the parse. The parser is then reset, and it is ready for the
/// next source. `allocation::peak` gives the peak of this parse until the next parse.
pub fn parse_with_limits(
    parser: &mut Parser,
    source: &[u8],
    label: &str,
    budget: u64,
    ceiling: u64,
) -> Result<Tree, Stop> {
    let _label = allocation::Label::new(label);
    allocation::reset();
    let mut checks: u64 = 0;
    let mut stop = None;
    let mut watch = |_: &ParseState| {
        checks += 1;
        if checks > budget {
            stop = Some(Stop::Budget(budget));
        } else if allocation::peak() > ceiling {
            stop = Some(Stop::Memory(ceiling));
        }
        if stop.is_some() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let tree = parser.parse_with_options(
        &mut |at, _| source.get(at..).unwrap_or_default(),
        None,
        Some(ParseOptions::new().progress_callback(&mut watch)),
    );
    match tree {
        Some(tree) => Ok(tree),
        None => {
            parser.reset();
            Err(stop.expect("a parse that gives no tree stopped at a limit"))
        }
    }
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
    record.ceiling = ceiling(source.len());
    let started = Instant::now();
    let tree = parse_with_limit(parser, &source, rel);
    record.parse_us = started.elapsed().as_micros();
    record.peak_bytes = allocation::peak();
    match tree {
        Ok(tree) => scan(tree.root_node(), &source, &mut record),
        Err(Stop::Budget(_)) => record.stopped = true,
        Err(Stop::Memory(_)) => record.memory = true,
    }
    record
}

/// Parse the files of a list in parallel, write one TSV line for each, and print a summary.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    const USAGE: &str = "usage: cargo xtask corpus ROOT LIST OUT [--seed FILE | --seeds DIRECTORY]";
    let (root, list, out, seed) = match args {
        [root, list, out] => (root, list, out, None),
        [root, list, out, flag, value] if flag == "--seed" || flag == "--seeds" => {
            (root, list, out, Some((flag.as_str(), value)))
        }
        _ => return Err(USAGE.into()),
    };
    let root = Path::new(root);
    let list_path = list;
    let list = fs::read_to_string(list).map_err(|e| format!("cannot read the file list {list}: {e}"))?;
    let paths: Vec<&str> = list.lines().filter(|line| !line.is_empty()).collect();
    let seeds = match seed {
        None => Seeds::None,
        Some(("--seed", path)) => Seeds::One(Box::new(Seed::read(Path::new(path))?)),
        Some((_, directory)) => Seeds::by_project(Path::new(directory), &paths)?,
    };
    // A directory that holds no seed for any project of the list is a directory that the caller
    // named incorrectly. A run of that kind gives the rows of a parser with no seed, and each row
    // then says so, but the caller asked for a seed and gets no message. So the run stops here.
    if let Some((_, given)) = seed
        && seeds.is_none()
    {
        return Err(format!(
            "{given} holds no seed file for any of the {} projects of {list_path}. A seed file of a project is \
             <project>.seed, and the project is the first component of the path of a file.",
            paths
                .iter()
                .filter_map(|rel| rel.split('/').next())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
        )
        .into());
    }
    // The report comes before the run, so that a run of one project never reads as a run of all
    // of them, and so that a report of the rows names the seed that made them.
    println!("{}", seeds.report(&paths));
    let language = Language::new(tree_sitter_cpp::LANGUAGE);
    let done = AtomicUsize::new(0);
    let started = Instant::now();
    let records: Vec<Record> = paths
        .par_iter()
        .map_init(
            || new_parser(&language),
            |parser, rel| {
                // The parser of a thread reads more than one project, so the context comes before
                // each file. SAFETY: `seeds` lives until the end of this function, and each parse
                // of this closure is inside it.
                unsafe { parser.set_scanner_context(seeds.context(rel)) };
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
            "{rel}\t{}\t{}\t{}\t{}\t{row}\t{kind}\t{what}\t{line}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.bytes,
            r.errors,
            r.missing,
            r.error_bytes,
            r.parse_us,
            u8::from(r.stopped),
            u8::from(r.memory),
            r.peak_bytes,
            r.ceiling,
            seeds.id(rel)
        )?;
    }
    sink.flush()?;

    let files = records.iter().filter(|r| !r.unreadable).count();
    let failed = records.iter().filter(|r| r.failed()).count();
    let bytes: usize = records.iter().map(|r| r.bytes).sum();
    let error_bytes: usize = records.iter().map(|r| r.error_bytes).sum();
    let stopped = records.iter().filter(|r| r.stopped).count();
    let memory = records.iter().filter(|r| r.memory).count();
    let peak = records.iter().map(|r| r.peak_bytes).max().unwrap_or(0);
    println!(
        "files {files}  with an error {failed} ({:.3}%)  error bytes {:.3}%  stopped {stopped}  memory {memory}  peak bytes {peak}  unreadable {}  wall {:.0} s",
        100.0 * failed as f64 / files.max(1) as f64,
        100.0 * error_bytes as f64 / bytes.max(1) as f64,
        records.len() - files,
        started.elapsed().as_secs_f64(),
    );
    // The file nearest to its ceiling. A file whose peak moves toward its ceiling from one commit
    // to the next shows here before it stops. Refer to `CEILING_FLOOR`.
    let nearness = |r: &Record| r.peak_bytes as f64 / r.ceiling as f64;
    let nearest = paths
        .iter()
        .zip(&records)
        .filter(|(_, r)| r.ceiling > 0)
        .max_by(|a, b| nearness(a.1).total_cmp(&nearness(b.1)));
    if let Some((rel, r)) = nearest {
        println!(
            "nearest to the ceiling: {rel} at {:.1}% of its ceiling, {} of {} bytes",
            100.0 * nearness(r),
            r.peak_bytes,
            r.ceiling
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
        let mut gives_tree = |budget: u64| parse_with_budget(&mut parser, source.as_bytes(), budget).is_ok();
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

    /// The parse of one long line takes a time that grows with the length of the line.
    ///
    /// `ts_lexer__get_column` read the line again from its start at each call, and the scanner calls
    /// it one time for each `%` and for each name after a declarator. The parse of a line of
    /// declarations then took a time that grows with the square of the length of the line. Refer to
    /// `ColumnAnchor` in vendor/tree-sitter/src/lexer.h.
    ///
    /// The test compares two lines, and the second is four times the first. Under the load of four
    /// parallel gates the ratio reads 3.7 to 4.0 with the anchor, and 14.2 with the anchor off. The
    /// limit of eight separates the two, and it has a margin of two on each side.
    ///
    /// The test measures each line three times and keeps the smallest time of each. A parallel gate
    /// adds time and never removes it, so the smallest measurement is the one with the least
    /// contention. One measurement of each line is not sufficient. The longer parse continues for a
    /// longer period, and a load that comes in that period increases the ratio alone. A gate of
    /// 2026-09-16 gave 8.2 with one measurement of each line and no defect in the parser.
    ///
    /// The progress callback count of `parse_with_budget` does not replace the clock here. That
    /// count does not move for this defect, because the cost is in the lexer and not in the parse
    /// operations.
    #[test]
    fn the_parse_of_one_long_line_grows_with_the_length_of_the_line() {
        let line = |count: usize| format!("void f() {{ {} }}\n", "a b; ".repeat(count));
        let language = Language::new(tree_sitter_cpp::LANGUAGE);
        let mut parser = new_parser(&language);
        let time_of = |parser: &mut Parser, count: usize| {
            let source = line(count);
            let started = Instant::now();
            let tree = parse_with_limit(parser, source.as_bytes(), "a long line").expect("the parse of the line ends");
            let elapsed = started.elapsed();
            assert!(!tree.root_node().has_error(), "the line of {count} declarations has an error");
            elapsed
        };
        // The first parse of the process reads the tables into the cache, so it comes first and it
        // is not one of the measurements.
        time_of(&mut parser, 400);
        // The two lines alternate, so one period of load reaches the measurements of both.
        let mut short = Duration::MAX;
        let mut long = Duration::MAX;
        for _ in 0..3 {
            short = short.min(time_of(&mut parser, 3_000));
            long = long.min(time_of(&mut parser, 12_000));
        }
        let ratio = long.as_secs_f64() / short.as_secs_f64().max(1e-9);
        assert!(
            ratio < 8.0,
            "a line of 12,000 declarations takes {ratio:.1} times the time of a line of 3,000 \
             declarations, and four times is the time of a linear parse. {short:?} and {long:?}"
        );
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
        assert_eq!(
            parse_with_budget(&mut parser, source.as_bytes(), smallest - 1).err(),
            Some(Stop::Budget(smallest - 1)),
            "a budget of {} gives a tree, and the smallest budget is {smallest}",
            smallest - 1
        );
        assert!(parse_with_budget(&mut parser, source.as_bytes(), smallest).is_ok());
        assert_eq!(
            smallest,
            smallest_budget(&source, 10_000),
            "the count of the progress callbacks of one parse is not the same in each run"
        );
    }

    /// The peak of the allocations of a parse with a new parser. The parser of a corpus thread
    /// keeps the capacity of its arrays from one file to the next, so the peak of one file moves
    /// by that capacity between two runs. A new parser for each measurement removes the move.
    fn peak_of(source: &str, ceiling: u64) -> (Result<(), Stop>, u64) {
        let language = Language::new(tree_sitter_cpp::LANGUAGE);
        let mut parser = new_parser(&language);
        let result = parse_with_ceiling(&mut parser, source.as_bytes(), ceiling).map(|_| ());
        (result, allocation::peak())
    }

    /// The memory ceiling of a parse counts the bytes of the runtime, and it reads no clock.
    ///
    /// Two parses of the source give the same peak to 0.1%, a ceiling of half the peak stops the
    /// parse, and a ceiling of two times the peak gives a tree. The test also fails when the count
    /// of the runtime does not move, which is the condition of a platform with no size of a block.
    ///
    /// The two peaks are not equal to the byte. The first parse of the process gave 808,704 bytes
    /// and the second 808,672, in three runs. The C library gives a block of a different size for
    /// the same request when it splits a free block, so the count follows the state of the heap by
    /// some bytes. Refer to `CEILING_FLOOR`.
    #[test]
    fn the_memory_ceiling_of_a_parse_counts_the_bytes_of_the_runtime() {
        let source = source_of_many_callbacks();
        let (result, peak) = peak_of(&source, u64::MAX);
        assert_eq!(result, Ok(()));
        assert!(peak > 100_000, "the parse of the source allocated {peak} bytes, and a tree of it is larger");
        let again = peak_of(&source, u64::MAX).1;
        assert!(
            again.abs_diff(peak) * 1000 <= peak,
            "the peak of one parse moves between two runs by more than 0.1%: {peak} and {again}"
        );
        let (stopped, stopped_peak) = peak_of(&source, peak / 2);
        assert_eq!(stopped, Err(Stop::Memory(peak / 2)));
        assert!(stopped_peak > peak / 2, "the stopped parse reads a peak of {stopped_peak}, and the ceiling is {}", peak / 2);
        assert_eq!(peak_of(&source, 2 * peak).0, Ok(()));
    }
}
