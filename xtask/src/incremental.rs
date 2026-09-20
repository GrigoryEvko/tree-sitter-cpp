//! Parse each file of a list twice, once fresh and once after an edit and its undo, and compare the
//! two trees.
//!
//! AN EDIT AND ITS UNDO GIVE THE BYTES THAT THE FIRST PARSE READ, so the tree after the undo must be
//! the tree of the fresh parse. An editor holds the tree of a buffer, edits it, parses again with the
//! old tree, and does that again when the reader undoes the edit. A consumer that reloads a file gets
//! the fresh tree. The two consumers must not disagree.
//!
//! THE FORK HAD NO STANDING CHECK OF THIS PROPERTY. Patch 10 of upstream-patches names an incremental
//! test of 23,768 edits, an undo test and a test of snippets, and the import 5882494 squashed the
//! history and carried none of the three. The record of class heads, the record of aliases, the
//! record of template type parameters and the record of locals all landed after that. A record that a
//! fresh parse fills as it scans is not obviously filled the same way when the parser reuses a
//! subtree and never scans its text again, and a missing record entry changes a node kind with no
//! ERROR node. Task 408 found the archived failure of the old undo test, dated it to f0adac0, and
//! measured 0 of 5,999 files at 558fe9a. This task makes that measurement a command and a gate step.
//!
//! THE CLASS IS RARE AND A SAMPLE READS AS A PASS. At f0adac0, 5 of 5,999 files differ, about one
//! file in 1,200, and a sample of 400 files gave 0 at f0adac0 and 0 at the master. Run the list.
//!
//! THE SEED IS PART OF THE TREE, so it is part of this property. A seed gives the scanner the names
//! that a project declares, and the scanner reads those names while it fills its records. Run this
//! with `--seeds`, as the gate does, or the half that a consumer uses is not checked.
//!
//! Usage:
//!     cargo xtask incremental ROOT LIST OUT [--seed FILE | --seeds DIRECTORY]
//!                                          [--edits N] [--shapes fast|all]
//!
//! OUT takes one TSV line for each file whose undo tree differs from its fresh tree: the path, the
//! byte of the edit, the shape of the edit, and the two hashes. A run with no differing file writes
//! an empty file, and the summary line holds the counts.

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rayon::prelude::*;
use tree_sitter::{InputEdit, Language, Parser, Point, Tree};

use crate::corpus::{ceiling, new_parser, parse_with_limits, parse_with_limits_and_old, BUDGET};
use crate::seed::{Seed, Seeds};
use crate::trees::{tree_facts, Names};

/// One edit that a run applies and undoes.
///
/// Each shape reaches a different path of the incremental parse. An insert of a space changes no
/// token and the parser reuses nearly everything. A letter changes the token under the edit. A line
/// break changes a line and every column after it. `//` turns the rest of a line into a comment, so
/// it removes tokens that the old tree holds. `{` opens a bracket and moves the shape of the file
/// below it. A cut removes a subtree of the old tree, and the undo puts the bytes back where the
/// parse has no old subtree for them.
#[derive(Clone, Copy)]
enum Shape {
    /// Insert this text and then remove it again.
    Insert(&'static str),
    /// Remove this many bytes and then put them back.
    Cut(usize),
}

impl Shape {
    /// The name of the shape for a row of the output.
    fn name(self) -> String {
        match self {
            Self::Insert(text) => format!("insert {text:?}"),
            Self::Cut(count) => format!("cut {count}"),
        }
    }
}

/// The shapes of a fast run: one insert that changes a token, and one cut.
const FAST: [Shape; 2] = [Shape::Insert("x"), Shape::Cut(8)];

/// The shapes of a full run.
const ALL: [Shape; 6] = [
    Shape::Insert(" "),
    Shape::Insert("x"),
    Shape::Insert("\n"),
    Shape::Insert("//"),
    Shape::Insert("{"),
    Shape::Cut(8),
];

/// The point of a byte offset in a source: the row and the column, both zero-based.
///
/// The column is a byte count, which is what `InputEdit` takes. O(n) in the bytes before the offset.
fn point_of(source: &[u8], at: usize) -> Point {
    let head = &source[..at];
    let row = head.iter().filter(|byte| **byte == b'\n').count();
    let column = at - head.iter().rposition(|byte| *byte == b'\n').map_or(0, |index| index + 1);
    Point::new(row, column)
}

/// The text of one shape applied at `at`, and the edit that describes it.
///
/// `at` IS A BYTE AND IT CAN LAND INSIDE A CHARACTER. The positions come from the length of the
/// file, so an edit can cut a multi-byte character in two, and the parser then reads text that is not
/// valid UTF-8. That does not weaken the check: the undo puts the same bytes back, and the property
/// is that the two trees of one text are equal, whatever the bytes are. A run makes no claim that
/// each edited text is a text a reader would type.
///
/// `Point.column` is a byte offset inside its row, which is what the runtime takes.
fn apply(source: &[u8], at: usize, shape: Shape) -> (Vec<u8>, InputEdit) {
    let start = point_of(source, at);
    match shape {
        Shape::Insert(text) => {
            let mut edited = Vec::with_capacity(source.len() + text.len());
            edited.extend_from_slice(&source[..at]);
            edited.extend_from_slice(text.as_bytes());
            edited.extend_from_slice(&source[at..]);
            let end = at + text.len();
            let edit = InputEdit {
                start_byte: at,
                old_end_byte: at,
                new_end_byte: end,
                start_position: start,
                old_end_position: start,
                new_end_position: point_of(&edited, end),
            };
            (edited, edit)
        }
        Shape::Cut(count) => {
            let end = (at + count).min(source.len());
            let mut edited = Vec::with_capacity(source.len());
            edited.extend_from_slice(&source[..at]);
            edited.extend_from_slice(&source[end..]);
            let edit = InputEdit {
                start_byte: at,
                old_end_byte: end,
                new_end_byte: at,
                start_position: start,
                old_end_position: point_of(source, end),
                new_end_position: start,
            };
            (edited, edit)
        }
    }
}

/// The edit that undoes `edit`: the two ends change places.
fn inverse(edit: &InputEdit) -> InputEdit {
    InputEdit {
        start_byte: edit.start_byte,
        old_end_byte: edit.new_end_byte,
        new_end_byte: edit.old_end_byte,
        start_position: edit.start_position,
        old_end_position: edit.new_end_position,
        new_end_position: edit.old_end_position,
    }
}

/// Parse `source` with `old` as the old tree, under the limits of a corpus run.
///
/// A file that a fresh parse reads is a file that this parse reads, because the two take the same
/// budget and the same ceiling from `corpus`. None when a limit stopped the parse.
fn reparse(parser: &mut Parser, source: &[u8], old: &Tree, label: &str) -> Option<Tree> {
    let ceiling = ceiling(source.len());
    parse_with_limits_and_old(parser, source, label, BUDGET, ceiling, Some(old)).ok()
}

/// What one file gave.
#[derive(Default)]
struct Record {
    /// The file could not be read.
    unreadable: bool,
    /// A parse of the file stopped at the budget or at the memory ceiling.
    stopped: bool,
    /// The edit cycles that ran for this file.
    cycles: usize,
    /// One row for each cycle whose undo tree differs from the fresh tree.
    rows: String,
    /// True when one cycle or more differs.
    differs: bool,
}

/// Run every shape at every position of one file, and compare each undo tree with the fresh tree.
///
/// The cost is one fresh parse and two parses for each cycle. O(cycles) in the parses.
fn probe(
    parser: &mut Parser,
    names: &Names,
    root: &Path,
    rel: &str,
    edits: usize,
    shapes: &[Shape],
) -> Record {
    let mut record = Record::default();
    let Ok(source) = fs::read(root.join(rel)) else {
        record.unreadable = true;
        return record;
    };
    if source.is_empty() {
        return record;
    }
    let ceiling = ceiling(source.len());
    let Ok(fresh) = parse_with_limits(parser, &source, rel, BUDGET, ceiling) else {
        record.stopped = true;
        return record;
    };
    let want = tree_facts(&fresh, names, source.len()).hash;

    for step in 0..edits {
        // The positions spread over the file, and the same file always takes the same bytes, so a
        // run of this command is a measurement that another run reproduces.
        let at = (source.len() * (2 * step + 1) / (2 * edits)).min(source.len());
        for &shape in shapes {
            record.cycles += 1;
            let (edited, edit) = apply(&source, at, shape);
            let mut tree = fresh.clone();
            tree.edit(&edit);
            let Some(mut tree) = reparse(parser, &edited, &tree, rel) else {
                record.stopped = true;
                continue;
            };
            tree.edit(&inverse(&edit));
            let Some(undone) = reparse(parser, &source, &tree, rel) else {
                record.stopped = true;
                continue;
            };
            let got = tree_facts(&undone, names, source.len()).hash;
            if got != want {
                record.differs = true;
                let _ = writeln!(record.rows, "{rel}\t{at}\t{}\t{want:016x}\t{got:016x}", shape.name());
            }
        }
    }
    record
}

/// Parse each file of the list fresh and after an edit with its undo, and report each file that
/// differs.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    const USAGE: &str = "usage: cargo xtask incremental ROOT LIST OUT [--seed FILE | --seeds DIRECTORY] \
                         [--edits N] [--shapes fast|all]";
    let mut positional: Vec<&String> = Vec::new();
    let mut seed: Option<(&str, &String)> = None;
    let mut edits = 1usize;
    let mut shapes: &[Shape] = &FAST;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            flag @ ("--seed" | "--seeds") => {
                let value = args.get(index + 1).ok_or(USAGE)?;
                seed = Some((flag, value));
                index += 2;
            }
            "--edits" => {
                edits = args.get(index + 1).ok_or(USAGE)?.parse()?;
                if edits == 0 {
                    return Err("--edits takes a count of 1 or more".into());
                }
                index += 2;
            }
            "--shapes" => {
                shapes = match args.get(index + 1).ok_or(USAGE)?.as_str() {
                    "fast" => &FAST,
                    "all" => &ALL,
                    other => return Err(format!("--shapes takes `fast` or `all`, and not {other}").into()),
                };
                index += 2;
            }
            _ => {
                positional.push(&args[index]);
                index += 1;
            }
        }
    }
    let [root, list, out] = positional.as_slice() else {
        return Err(USAGE.into());
    };
    let root = Path::new(root.as_str());
    let list_path = list.as_str();
    let text = fs::read_to_string(list_path).map_err(|e| format!("cannot read the file list {list_path}: {e}"))?;
    let paths: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
    let seeds = match seed {
        None => Seeds::None,
        Some(("--seed", path)) => Seeds::One(Box::new(Seed::read(Path::new(path.as_str()))?)),
        Some((_, directory)) => Seeds::by_project(Path::new(directory.as_str()), &paths)?,
    };
    if let Some((_, given)) = seed
        && seeds.is_none()
    {
        return Err(Seeds::none_found(given, list_path, &paths).into());
    }
    // The report comes before the run, so that a run with one seed never reads as a run with all of
    // them, and so that the rows name the seed that made them.
    println!("{}", seeds.report(&paths));

    let language = Language::new(tree_sitter_cpp::LANGUAGE);
    let names = Names::new(&language);
    let done = AtomicUsize::new(0);
    let started = Instant::now();
    let records: Vec<Record> = paths
        .par_iter()
        .map_init(
            || new_parser(&language),
            |parser, rel| {
                // The parser of a thread reads more than one project, so the context comes before
                // each file. SAFETY: `seeds` lives until the end of this function, and each parse of
                // this closure is inside it.
                unsafe { parser.set_scanner_context(seeds.context(rel)) };
                let record = probe(parser, &names, root, rel, edits, shapes);
                let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                if count.is_multiple_of(100_000) {
                    eprintln!("{count} files in {:.0} s", started.elapsed().as_secs_f64());
                }
                record
            },
        )
        .collect();

    let mut sink = BufWriter::new(fs::File::create(out.as_str()).map_err(|e| format!("cannot create {out}: {e}"))?);
    for record in &records {
        sink.write_all(record.rows.as_bytes())?;
    }
    sink.flush()?;

    let files = records.iter().filter(|r| !r.unreadable).count();
    let cycles: usize = records.iter().map(|r| r.cycles).sum();
    let differing = records.iter().filter(|r| r.differs).count();
    let stopped = records.iter().filter(|r| r.stopped).count();
    let unreadable = records.iter().filter(|r| r.unreadable).count();
    // THE SUMMARY NAMES THE CYCLES AND NOT ONLY THE FILES. A run whose cycle count is 0 compared
    // nothing, and its `differing 0` would read as a pass.
    println!(
        "SUMMARY files {files}  cycles {cycles}  undo tree differs {differing}  stopped {stopped}  \
         unreadable {unreadable}  wall {:.0} s",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{apply, inverse, point_of, Shape, ALL, FAST};
    use crate::trees::{tree_facts, Names};
    use std::fs;
    use tree_sitter::Language;

    #[test]
    fn an_insert_and_its_undo_give_the_bytes_again() {
        let source = b"int a;\nint b;\n";
        for shape in ALL.iter().chain(FAST.iter()) {
            for at in 0..source.len() {
                let (edited, edit) = apply(source, at, *shape);
                let back = inverse(&edit);
                assert_eq!(back.start_byte, edit.start_byte);
                assert_eq!(back.old_end_byte, edit.new_end_byte);
                assert_eq!(back.new_end_byte, edit.old_end_byte);
                // The edit describes the text that `apply` wrote.
                assert_eq!(edited.len(), source.len() + edit.new_end_byte - edit.old_end_byte);
            }
        }
    }

    #[test]
    fn a_point_holds_the_row_and_the_byte_column() {
        let source = b"ab\ncde\n";
        assert_eq!(point_of(source, 0).row, 0);
        assert_eq!(point_of(source, 0).column, 0);
        assert_eq!(point_of(source, 2).column, 2);
        assert_eq!(point_of(source, 3).row, 1);
        assert_eq!(point_of(source, 3).column, 0);
        assert_eq!(point_of(source, 5).row, 1);
        assert_eq!(point_of(source, 5).column, 2);
    }

    #[test]
    fn a_cut_at_the_end_of_a_source_takes_the_bytes_that_are_there() {
        let source = b"int a;";
        let (edited, edit) = apply(source, 4, Shape::Cut(8));
        assert_eq!(edited, b"int ");
        assert_eq!(edit.old_end_byte, source.len());
        assert_eq!(edit.new_end_byte, 4);
    }

    /// The edit cycle gives the hash of the fresh parse, and the half cycle, which stops at the
    /// edited text, does not.
    ///
    /// THE SECOND ASSERTION IS THE TEST. A run of this command reports `undo tree differs 0`, and
    /// that number says nothing until the instrument has reported a difference that is there. The
    /// half cycle drives every call that `probe` makes except the second edit, over a text that
    /// really is different, and the hash must move for each shape.
    ///
    /// THE LIMIT OF THIS TEST. A text that contradicts its own `InputEdit` is not a way to plant a
    /// difference: an incremental parse is entitled to the edit it was given, and it reuses the
    /// subtrees that the edit leaves alone with no second read of their bytes. A first version of
    /// this test appended bytes and described no edit at the end, and the hash did not move, which
    /// is correct. So this test shows that the comparison reports a real difference. Only a parser
    /// that has the defect shows that the cycle finds an incremental defect, and task 408 did that
    /// with the parser at f0adac0, where 5 of 5,999 corpus files differ.
    #[test]
    fn the_comparison_reports_a_difference_that_is_there() {
        let source = b"struct S { int m; };\nS f(int a) { return S{a}; }\n";
        let language = Language::new(tree_sitter_cpp::LANGUAGE);
        let names = Names::new(&language);
        let mut parser = crate::corpus::new_parser(&language);
        let fresh = crate::corpus::parse_with_limit(&mut parser, source, "a small source")
            .expect("the fresh parse of a small source ends");
        let want = tree_facts(&fresh, &names, source.len()).hash;
        assert!(!fresh.root_node().has_error(), "the source of this test parses with no error");

        for shape in ALL {
            let (edited, edit) = apply(source, 21, shape);
            let mut tree = fresh.clone();
            tree.edit(&edit);
            let mut tree = super::reparse(&mut parser, &edited, &tree, "a small source")
                .expect("the parse of the edit ends");
            assert_ne!(
                tree_facts(&tree, &names, edited.len()).hash,
                want,
                "the tree of the edited text has the hash of the fresh tree for the shape {}, so the \
                 comparison cannot report a difference either",
                shape.name()
            );

            tree.edit(&inverse(&edit));
            let undone = super::reparse(&mut parser, source, &tree, "a small source")
                .expect("the parse of the undo ends");
            assert_eq!(
                tree_facts(&undone, &names, source.len()).hash,
                want,
                "the undo of the shape {} did not give the tree of the fresh parse",
                shape.name()
            );
        }
    }

    /// THE SEED REACHES THE TWO PARSES OF THE EDIT CYCLE, AND NOT THE FRESH PARSE ONLY.
    ///
    /// A run of this command sets the context of the parser one time for each file and then parses
    /// three times: the fresh parse, the parse of the edit, and the parse of the undo. The context
    /// is a field of the parser that `ts_parser_reset` does not clear, and
    /// `ts_parser__external_scanner_create` gives it to each scanner it makes. Nothing else checks
    /// that, and a seed that reached the fresh parse alone would make every seeded run report a
    /// difference in each file whose tree the seed changes, or none at all.
    ///
    /// `A(x)` at a position where no declaration can start reads as a functional cast when the seed
    /// holds `A` as a type, and as a call when it does not.
    #[test]
    fn the_seed_reaches_each_parse_of_the_edit_cycle() {
        const CAST: &str = "int f() { return A(x); }\n";
        let directory = std::env::temp_dir().join(format!("xtask-incremental-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("the directory of the test");
        let path = directory.join("project.seed");
        fs::write(&path, format!("{}\nA\ttype\n", crate::seed::format_row())).expect("the seed of the test");
        let seed = crate::seed::Seed::read(&path).expect("the seed reads");

        let language = Language::new(tree_sitter_cpp::LANGUAGE);
        let names = Names::new(&language);
        let is_cast = |tree: &tree_sitter::Tree| tree.root_node().to_sexp().contains("function: (type_identifier)");

        let cycle = |context: *const std::ffi::c_void| -> (bool, bool) {
            let mut parser = crate::corpus::new_parser(&language);
            // SAFETY: `seed` lives to the end of this test, and each parse is inside it.
            unsafe { parser.set_scanner_context(context) };
            let fresh = crate::corpus::parse_with_limit(&mut parser, CAST.as_bytes(), "the cast source")
                .expect("the fresh parse ends");
            let want = tree_facts(&fresh, &names, CAST.len()).hash;
            let (edited, edit) = apply(CAST.as_bytes(), 10, Shape::Insert("x"));
            let mut tree = fresh.clone();
            tree.edit(&edit);
            let mut tree = super::reparse(&mut parser, &edited, &tree, "the cast source")
                .expect("the parse of the edit ends");
            tree.edit(&inverse(&edit));
            let undone = super::reparse(&mut parser, CAST.as_bytes(), &tree, "the cast source")
                .expect("the parse of the undo ends");
            assert_eq!(
                tree_facts(&undone, &names, CAST.len()).hash,
                want,
                "the undo did not give the tree of the fresh parse"
            );
            (is_cast(&fresh), is_cast(&undone))
        };

        let (fresh_seeded, undone_seeded) = cycle(seed.as_context());
        let (fresh_bare, undone_bare) = cycle(std::ptr::null());
        let _ = fs::remove_dir_all(&directory);

        assert!(fresh_seeded, "the seed did not reach the fresh parse");
        assert!(undone_seeded, "the seed did not reach the parses of the edit cycle");
        assert!(!fresh_bare, "a parse with no seed read the callee as a type");
        assert!(!undone_bare, "a parse with no seed read the callee as a type after the undo");
    }
}
