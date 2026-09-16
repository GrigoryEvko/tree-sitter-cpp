//! Find each tree that the order of the symbol ids selects, and no rule.
//!
//! A TIE is one text with two readings that no rule separates. `ts_parser__select_tree`
//! (vendor/tree-sitter/src/parser.c) compares the error cost of the two readings, then the dynamic
//! precedence. When the two values are equal it calls `ts_subtree_compare`
//! (vendor/tree-sitter/src/subtree.c), which compares the NUMERIC SYMBOL IDS of the two trees and
//! takes the smaller one. The generator gives the symbol ids, and it moves them when the grammar
//! gains or loses a rule. A tree that this comparison selects is then correct by accident, and a
//! later change of any rule can give the other reading with NO ERROR NODE.
//!
//! The four defects of this class that the fork already repaired are the operand of an assignment,
//! the operand of a reflect expression, the culled version of a deeply nested template argument
//! list, and the constraint of a requires clause that ended at a component of a qualified name.
//! None of them gave an ERROR node, and the corpus report showed none of them.
//!
//! THE COMMAND. `cargo xtask ties corpus` parses each file of `test/ties/sample.txt` with the logger
//! of the runtime, and it reports each site where the symbol order selected the tree. It compares
//! the sites with the baseline in `test/ties/baseline.txt`:
//!
//! - A site that the baseline does not hold fails the command. Such a site is one more tree that
//!   the next change of the grammar can flip.
//! - A site of the baseline that is gone does not fail the command. A repair of a tie must never
//!   fail its own check.
//!
//! The test `the_sites_of_the_sample_hold_no_site_that_the_baseline_does_not_hold` runs the same
//! comparison, and `cargo test --workspace` of the gate runs that test. `ROOT LIST` after `corpus`
//! reads a different corpus and a different list, and `--write-baseline` writes the sites to
//! `test/ties/baseline.txt`.
//!
//! THE CHECK IS SENSITIVE. With the base type of the first form of `sized_type_specifier` optional
//! again, as it was before fork commit 1edab87, the command names 493 sites with the file, the row,
//! the column, and the two symbols, and it fails.
//!
//! THE SAMPLE IS NOT THE POPULATION. The full corpus of 329,387 files gives 101,100 sites in 3,697
//! files, and the 3,000-file sample gives 103. The sample holds 1/110 of the files and 1/981 of the
//! sites, because the distribution is extreme: the median file of the 3,697 has 2 sites, the p90
//! file has 14, and one generated file of boost/libs/qvm holds 45,826.
//!
//! `cargo xtask ties population ROOT LIST` covers the full corpus. A per-site baseline of it is
//! 11 MB and no person reads it, so the baseline holds one row for each file and each message with
//! a count: 4,117 rows and 562 KB. The run takes 27 seconds, which fits a full gate and not a test.
//!
//! - WHAT THE POPULATION BASELINE PROMISES: a count that rises fails the check, and a count that
//!   falls does not. A message that moves to a different class in one file gives one fall and one
//!   rise, and the rise fails the check.
//! - WHAT IT DOES NOT PROMISE: a site that moves to a different row or column of one file, with no
//!   change of the count, fails no check. `test/ties/baseline.txt` covers that case for the 3,000
//!   files of `test/ties/sample.txt`, and `cargo test --workspace` runs that comparison.
//!
//! WHO WRITES THE BASELINE AGAIN. A commit that adds a reading to the grammar adds sites, and the
//! author of that commit writes the baseline again and gives the reason. The first run of the check
//! on a master that moved found 43 counts that rose and 4 that fell, 341 sites more, from the four
//! commits between 5271a72 and 215b2b0. A parser with the two branches of `ts_subtree_compare`
//! turned around reads the 41 files, and 40 of them give the same visible tree, so the added sites
//! are in the classes that a consumer does not see.
//!
//! WHICH TIES A CONSUMER SEES. A parser with the two branches of `ts_subtree_compare` turned around
//! reads the 3,697 files with ties, and 2,829 of them give the same visible tree. The class of a tie
//! then tells whether the reading of the tree depends on it:
//!
//! - INVISIBLE: `template_argument_list_repeat1` with 64,400 sites and 0 of 446 files that differ,
//!   `comma_expression` with 4,558 and 0 of 273, `expression_statement` with 522 and 0 of 199,
//!   `_conditional_expression` with 527 and 0 of 37, and six smaller ones.
//!
//! AN INVISIBLE TIE IS NOT ALWAYS ONE TREE, AND THE LARGEST ONE IS NOT. The smallest text of
//! `template_argument_list_repeat1` is `A<F<G,H<G>,I> > x;`. A runtime that prints both raw
//! subtrees at the tie, with the hidden nodes, shows two DIFFERENT readings of one span: two
//! arguments `F<G,(H<G)>` and `I`, or three arguments `(F<G)`, `H<G>` and `I`. Each holds two
//! `type_descriptor` and one `expression`, so `_template_argument` gives each of them 3+3+1 and the
//! dynamic precedence separates nothing. The finished tree is the same either way, because the
//! merge is on a stack version that the continuation kills. The tie is wasted work of the GLR
//! parser, and the ambiguity under it is the template-versus-comparison one of #138, which the
//! grammar declares as the conflict of `template_function` and `template_type`. It is NOT the
//! redundancy of `sized_type_specifier`, where the two readings were one tree.
//!
//! The parse time of that class is no reason to repair it. 45,826 of the 64,400 sites are in
//! boost/libs/qvm/include/boost/qvm/gen/swizzle4.hpp, which is 1,249,713 bytes and parses in
//! 845,285 microseconds, or 693 microseconds for each KB. The corpus files larger than 20 KB
//! average 768 microseconds for each KB.
//! - VISIBLE, and the name lookup of #138: `alignas_qualifier` with 1,522 sites and 794 of 794 files
//!   that differ, which is `alignas(name)` as an expression or as a type-id ([dcl.align]),
//!   `parameter_list` with 87 and 20 of 20, `type_definition` with 33 and 4 of 4, and
//!   `binary_expression` with 19,799 and 1 of 1,235.
//!
//! `cargo xtask ties table` reads `src/parser.c` and reports each action list of the parse table
//! that holds more than one action, by the rules that the actions reduce. That report gives the
//! shape of the ambiguity, and the corpus report gives the texts that reach it.
//!
//! THE SECOND ARRIVAL ORDER IS THE VERSION COUNT. `ts_parser__condense_stack` removes each version
//! after the index `MAX_VERSION_COUNT`, and the pairwise comparison of that function moves a version
//! only for a smaller error cost or a larger dynamic precedence. Two versions of equal cost and
//! equal dynamic precedence keep the order in which they arrived, and the cull then takes the one
//! that arrived later. `cargo xtask ties versions` reports each file whose count passes the limit,
//! and it compares the files with `test/ties/versions.txt` under the same baseline rule.
//!
//! The measurement of the full corpus of 329,387 files: 59 files pass the limit of 32, the largest
//! count is 66, and no file aborts a reduce for the limit `MAX_VERSION_COUNT_OVERFLOW`. With the
//! limit at 256, which removes each of these culls, two files get a different tree, and the two
//! already have an ERROR node with each limit. The remaining arrival-order culls of the runtime
//! change no tree of a file that parses with no error.
//!
//! `cargo xtask ties trace FILE` prints the parse log of one file. With `--state N` it prints only
//! the rows where the parser enters the state N, which tells which text reaches a state of the
//! parse table.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use tree_sitter::{Language, LogType, Parser};

/// The messages of `ts_parser__select_tree` that name a selection by the symbol order.
///
/// `select_earlier` comes from a comparison that gives -1 or 1, and `select_existing` from a
/// comparison that gives 0. The two other messages of that function, `select_smaller_error` and
/// `select_higher_precedence`, name a selection that a rule makes, and they are no tie.
const TIE_MESSAGES: [&str; 2] = ["select_earlier", "select_existing"];

/// The path of the baseline in the repository.
const BASELINE: &str = "test/ties/baseline.txt";

/// The baseline of the sites of the full corpus in the repository.
const POPULATION: &str = "test/ties/population.txt";

/// The baseline of the version counts in the repository.
const VERSIONS: &str = "test/ties/versions.txt";

/// The list of the files of the baseline, relative to the root of the corpus.
const SAMPLE_LIST: &str = "test/ties/sample.txt";

/// The root of the corpus that the baseline names.
///
/// The baseline holds the path of each file relative to this root. A machine with no corpus there
/// runs the comparison of `test/ties/sample.txt` for no file, and the test reports that.
const CORPUS: &str = "/tmp/cpp-corpora";

/// One place where the symbol order selected the tree.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Site {
    /// The path of the file, relative to the root of the corpus.
    pub path: String,
    /// The row of the lookahead of the parser, from 1.
    pub row: usize,
    /// The column of the lookahead of the parser, from 1.
    pub column: usize,
    /// The log message of the runtime, with the two symbols.
    pub what: String,
}

impl Site {
    /// The key of the site in the baseline: the file and the position.
    fn key(&self) -> (&str, usize, usize) {
        (self.path.as_str(), self.row, self.column)
    }

    /// One row of the baseline file.
    fn row_text(&self) -> String {
        format!("{}\t{}\t{}\t{}", self.path, self.row, self.column, self.what)
    }

    /// Read one row of the baseline file.
    fn from_row(row: &str) -> Option<Self> {
        let mut parts = row.split('\t');
        let path = parts.next()?.to_owned();
        let line = parts.next()?.parse().ok()?;
        let column = parts.next()?.parse().ok()?;
        let what = parts.next().unwrap_or_default().to_owned();
        Some(Self { path, row: line, column, what })
    }
}

/// The count of the sites of one message in one file.
///
/// The baseline of the full corpus holds one row for each file and message, and not one row for
/// each site. A per-site baseline of 329,387 files is 11 MB and 100,997 rows, and one generated file
/// of boost/libs/qvm writes 45 percent of each diff of it. The count gives the same failure rule
/// with 4,082 rows: a count that rises fails, and a count that falls does not.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Group {
    /// The path of the file, relative to the root of the corpus.
    pub path: String,
    /// The log message of the runtime, with the two symbols.
    pub what: String,
    /// The count of the sites of that message in that file.
    pub count: usize,
}

impl Group {
    /// One row of the baseline file.
    fn row_text(&self) -> String {
        format!("{}\t{}\t{}", self.path, self.count, self.what)
    }

    /// Read one row of the baseline file.
    fn from_row(row: &str) -> Option<Self> {
        let mut parts = row.split('\t');
        let path = parts.next()?.to_owned();
        let count = parts.next()?.parse().ok()?;
        let what = parts.next()?.to_owned();
        Some(Self { path, what, count })
    }
}

/// Put the sites of a run together by the file and the message.
pub fn group(sites: &[Site]) -> Vec<Group> {
    let mut counts: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for site in sites {
        *counts.entry((site.path.as_str(), site.what.as_str())).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|((path, what), count)| Group {
            path: path.to_owned(),
            what: what.to_owned(),
            count,
        })
        .collect()
}

/// The largest version count of the GLR parser in one file.
///
/// `ts_parser__condense_stack` of vendor/tree-sitter/src/parser.c removes each version after the
/// index `MAX_VERSION_COUNT`. The index order is the order in which the versions arrived, and the
/// pairwise comparison of that function moves a version only for a smaller error cost or a larger
/// dynamic precedence. A file whose count passes the limit then loses a version by arrival order.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileVersions {
    /// The path of the file, relative to the root of the corpus.
    pub path: String,
    /// The largest count of versions of the parse.
    pub versions: usize,
    /// The row of the largest count, from 1.
    pub row: usize,
    /// The column of the largest count, from 1.
    pub column: usize,
}

impl FileVersions {
    /// One row of the baseline file.
    fn row_text(&self) -> String {
        format!("{}\t{}\t{}\t{}", self.path, self.versions, self.row, self.column)
    }

    /// Read one row of the baseline file.
    fn from_row(row: &str) -> Option<Self> {
        let mut parts = row.split('\t');
        let path = parts.next()?.to_owned();
        let versions = parts.next()?.parse().ok()?;
        let line = parts.next()?.parse().ok()?;
        let column = parts.next()?.parse().ok()?;
        Some(Self { path, versions, row: line, column })
    }
}

/// The state of the logger of one parser.
#[derive(Default)]
struct Log {
    /// The path of the file that the parser reads.
    path: String,
    /// The row of the last `process version` message, from 0.
    row: usize,
    /// The column of the last `process version` message, from 0.
    column: usize,
    /// The sites of the file that the parser reads.
    sites: Vec<Site>,
    /// The largest version count of the file, and the position of that count.
    versions: usize,
    versions_row: usize,
    versions_column: usize,
}

/// Read a numeric field of a log message, for example `row:` of `process version:0, ..., row:3`.
fn field(message: &str, name: &str) -> Option<usize> {
    let rest = message.split_once(name)?.1;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Take the position and the tie of one parse message.
///
/// The runtime gives the position in the `process version` message that starts each step, and it
/// gives no position in the message of the selection. The position of the last step is the position
/// of the lookahead that the two readings meet at.
fn read_message(log: &mut Log, message: &str) {
    if let Some(rest) = message.strip_prefix("process version:") {
        if let (Some(row), Some(column)) = (field(rest, "row:"), field(rest, "col:")) {
            log.row = row;
            log.column = column;
        }
        if let Some(count) = field(rest, "version_count:")
            && count > log.versions
        {
            log.versions = count;
            log.versions_row = log.row;
            log.versions_column = log.column;
        }
        return;
    }
    if TIE_MESSAGES.iter().any(|name| message.starts_with(name)) {
        log.sites.push(Site {
            path: log.path.clone(),
            row: log.row + 1,
            column: log.column + 1,
            what: message.to_owned(),
        });
    }
}

/// The result of one run over a list of files.
#[derive(Default)]
pub struct Run {
    /// Each site where the symbol order selected the tree.
    pub sites: Vec<Site>,
    /// The largest version count of each file, in the order of the paths.
    pub versions: Vec<FileVersions>,
}

/// Parse each file of a list, and give the sites and the largest version count of each file.
///
/// O(n) in the bytes of the files. The parse with a logger is approximately ten times the parse
/// with no logger, because the runtime writes a message for each character of the lexer.
pub fn measure(root: &Path, paths: &[&str]) -> Run {
    let language = Language::new(tree_sitter_cpp::LANGUAGE);
    let mut rows: Vec<(Vec<Site>, FileVersions)> = paths
        .par_iter()
        .map_init(
            || {
                let mut parser = Parser::new();
                parser
                    .set_language(&language)
                    .expect("the ABI of the grammar must agree with the tree-sitter runtime");
                // The logger and the reader of the sites are on one thread, and the lock has no
                // other user. A channel or a thread-local gives no simpler code here.
                let shared = Arc::new(Mutex::new(Log::default()));
                let state = Arc::clone(&shared);
                parser.set_logger(Some(Box::new(move |kind, message| {
                    if kind != LogType::Parse {
                        return;
                    }
                    read_message(&mut state.lock().expect("the logger holds one log"), message);
                })));
                (parser, shared)
            },
            |(parser, shared), path| {
                let file = FileVersions {
                    path: (*path).to_owned(),
                    versions: 0,
                    row: 1,
                    column: 1,
                };
                let Ok(source) = fs::read(root.join(path)) else {
                    return (Vec::new(), file);
                };
                {
                    let mut log = shared.lock().expect("the reader holds one log");
                    log.path.clear();
                    log.path.push_str(path);
                    log.row = 0;
                    log.column = 0;
                    log.sites.clear();
                    log.versions = 0;
                    log.versions_row = 0;
                    log.versions_column = 0;
                }
                let tree = crate::corpus::parse_with_limit(parser, &source, path);
                let mut log = shared.lock().expect("the reader holds one log");
                let sites = std::mem::take(&mut log.sites);
                let file = FileVersions {
                    versions: log.versions,
                    row: log.versions_row + 1,
                    column: log.versions_column + 1,
                    ..file
                };
                drop(tree);
                (sites, file)
            },
        )
        .collect();
    let mut sites: Vec<Site> = Vec::new();
    let mut versions: Vec<FileVersions> = Vec::with_capacity(rows.len());
    for (file_sites, file) in rows.drain(..) {
        sites.extend(file_sites);
        versions.push(file);
    }
    sites.sort_unstable();
    versions.sort_unstable();
    Run { sites, versions }
}

/// The comparison of the sites of a run with the sites of the baseline.
#[derive(Debug, PartialEq, Eq)]
pub struct Comparison {
    /// The sites that the baseline does not hold. The command fails for each of them.
    pub added: Vec<Site>,
    /// The sites of the baseline that the run does not hold.
    pub removed: Vec<Site>,
    /// The sites of the baseline that the run holds with a different message.
    pub changed: Vec<(Site, Site)>,
}

/// Compare the sites of a run with the sites of the baseline.
///
/// The key of a site is the file and the position. A site with a different message at one position
/// is one tie in the two runs, and it goes into `changed` and not into `added`.
pub fn compare(baseline: &[Site], found: &[Site]) -> Comparison {
    let mut old: BTreeMap<(&str, usize, usize), Vec<&Site>> = BTreeMap::new();
    for site in baseline {
        old.entry(site.key()).or_default().push(site);
    }
    let mut new: BTreeMap<(&str, usize, usize), Vec<&Site>> = BTreeMap::new();
    for site in found {
        new.entry(site.key()).or_default().push(site);
    }
    let mut comparison = Comparison { added: Vec::new(), removed: Vec::new(), changed: Vec::new() };
    for (key, sites) in &new {
        match old.get(key) {
            None => comparison.added.extend(sites.iter().map(|&site| site.clone())),
            Some(before) => {
                // One position can hold more than one tie. The extra ones are new sites.
                if sites.len() > before.len() {
                    comparison.added.extend(sites[before.len()..].iter().map(|&site| site.clone()));
                }
                for (one, two) in before.iter().zip(sites.iter()) {
                    if one.what != two.what {
                        comparison.changed.push(((*one).clone(), (*two).clone()));
                    }
                }
            }
        }
    }
    for (key, sites) in &old {
        match new.get(key) {
            None => comparison.removed.extend(sites.iter().map(|&site| site.clone())),
            Some(after) if after.len() < sites.len() => {
                comparison.removed.extend(sites[after.len()..].iter().map(|&site| site.clone()));
            }
            Some(_) => {}
        }
    }
    comparison
}

/// The comparison of the groups of a run with the groups of the baseline.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GroupComparison {
    /// The groups whose count is larger than the count of the baseline, with the two counts. The
    /// command fails for each of them.
    pub risen: Vec<(Group, usize)>,
    /// The groups whose count is smaller than the count of the baseline, with the two counts.
    pub fallen: Vec<(Group, usize)>,
}

/// Compare the groups of a run with the groups of the baseline.
///
/// The key of a group is the file and the message. A group that the baseline does not hold has the
/// baseline count 0, and it goes into `risen`. A group of the baseline that the run does not hold
/// has the run count 0, and it goes into `fallen`. A run that reads only a part of the files of the
/// baseline reports no fall for the files that it did not read.
pub fn compare_groups(baseline: &[Group], found: &[Group], read: &BTreeSet<&str>) -> GroupComparison {
    let old: BTreeMap<(&str, &str), usize> = baseline
        .iter()
        .map(|group| ((group.path.as_str(), group.what.as_str()), group.count))
        .collect();
    let new: BTreeMap<(&str, &str), usize> = found
        .iter()
        .map(|group| ((group.path.as_str(), group.what.as_str()), group.count))
        .collect();
    let mut comparison = GroupComparison::default();
    for group in found {
        let before = *old.get(&(group.path.as_str(), group.what.as_str())).unwrap_or(&0);
        if group.count > before {
            comparison.risen.push((group.clone(), before));
        }
    }
    for group in baseline {
        if !read.contains(group.path.as_str()) {
            continue;
        }
        let after = *new.get(&(group.path.as_str(), group.what.as_str())).unwrap_or(&0);
        if after < group.count {
            comparison.fallen.push((group.clone(), after));
        }
    }
    comparison
}

/// One action of the parse table, as `src/parser.c` writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A shift to a state, or a shift of an extra.
    Shift,
    /// A reduce of a symbol, with the child count and the dynamic precedence.
    Reduce { symbol: u32, children: u32, precedence: i32, production: u32 },
    /// An accept or a recovery.
    Other,
}

/// The class of one action list of the parse table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    /// Two or more reduce actions of one production. The two readings give one tree.
    Duplicate,
    /// Two or more reduce actions of equal dynamic precedence. The symbol order selects the tree.
    ReduceTie,
    /// Two or more reduce actions of different dynamic precedence.
    ReducePrecedence,
    /// A shift action and one or more reduce actions.
    ShiftReduce,
    /// Each other list with more than one action.
    Other,
}

impl Class {
    /// The name of the class in the report.
    fn name(self) -> &'static str {
        match self {
            Self::Duplicate => "one production twice",
            Self::ReduceTie => "reduce and reduce, equal dynamic precedence",
            Self::ReducePrecedence => "reduce and reduce, different dynamic precedence",
            Self::ShiftReduce => "shift and reduce",
            Self::Other => "other",
        }
    }
}

/// One action list of the parse table with more than one action.
#[derive(Debug)]
pub struct ActionList {
    /// The class of the list.
    pub class: Class,
    /// The names of the symbols that the reduce actions of the list reduce, in order and with no
    /// repetition.
    pub symbols: Vec<String>,
}

/// Read the map from the numeric symbol id to the name of the symbol.
///
/// `enum ts_symbol_identifiers` gives the id of each symbol name, and `ts_symbol_names` gives the
/// text of each symbol name. A hidden symbol has no row in `ts_symbol_names`, and it keeps the name
/// of the enum.
fn read_symbol_names(text: &str) -> BTreeMap<u32, String> {
    let mut identifier_of_name: BTreeMap<String, u32> = BTreeMap::new();
    if let Some(start) = text.find("enum ts_symbol_identifiers {") {
        let body = &text[start..];
        let end = body.find("};").unwrap_or(body.len());
        for line in body[..end].lines().skip(1) {
            let Some((name, value)) = line.trim().trim_end_matches(',').split_once(" = ") else {
                continue;
            };
            if let Ok(value) = value.trim().parse() {
                identifier_of_name.insert(name.trim().to_owned(), value);
            }
        }
    }
    let mut names: BTreeMap<u32, String> = identifier_of_name
        .iter()
        .map(|(name, &value)| (value, name.clone()))
        .collect();
    if let Some(start) = text.find("static const char * const ts_symbol_names[] = {") {
        let body = &text[start..];
        let end = body.find("};").unwrap_or(body.len());
        for line in body[..end].lines().skip(1) {
            let line = line.trim();
            let Some(rest) = line.strip_prefix('[') else { continue };
            let Some((name, value)) = rest.split_once("] = ") else { continue };
            let Some(&id) = identifier_of_name.get(name.trim()) else { continue };
            let value = value.trim().trim_end_matches(',').trim_matches('"');
            names.insert(id, value.to_owned());
        }
    }
    names
}

/// Read one action of the text of `ts_parse_actions`.
fn read_action(name: &str, arguments: &str) -> Option<Action> {
    let values: Vec<&str> = arguments.split(',').filter(|value| !value.is_empty()).collect();
    match name {
        "S" | "SR" | "SX" => Some(Action::Shift),
        "R" => Some(Action::Reduce {
            symbol: values.first()?.parse().ok()?,
            children: values.get(1)?.parse().ok()?,
            precedence: values.get(2)?.parse().ok()?,
            production: values.get(3)?.parse().ok()?,
        }),
        "RC" | "AC" => Some(Action::Other),
        _ => None,
    }
}

/// Read each `NAME(ARGUMENTS)` item of a text, in order.
///
/// The arguments of an item hold commas, so a split on the comma reads an item incorrectly. O(n) in
/// the text.
fn read_items(text: &str) -> Vec<(&str, &str)> {
    let bytes = text.as_bytes();
    let mut items = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !(bytes[index].is_ascii_alphabetic() || bytes[index] == b'_') {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'(' {
            continue;
        }
        let name = &text[start..index];
        index += 1;
        let arguments = index;
        while index < bytes.len() && bytes[index] != b')' {
            index += 1;
        }
        items.push((name, &text[arguments..index]));
        index += 1;
    }
    items
}

/// Read each action list of `ts_parse_actions` that holds more than one action.
///
/// The array holds one `E(count, reusable)` header and then the actions of the list. O(n) in the
/// text of the array.
pub fn read_action_lists(text: &str) -> Vec<ActionList> {
    let names = read_symbol_names(text);
    let Some(start) = text.find("static const TSParseActionEntry ts_parse_actions[] = {") else {
        return Vec::new();
    };
    let body = &text[start..];
    let end = body.find("};").unwrap_or(body.len());
    let body = &body[..end];
    let mut lists = Vec::new();
    let mut actions: Vec<Action> = Vec::new();
    for (name, arguments) in read_items(body) {
        if name == "ts_parse_actions" {
            continue;
        }
        if name == "E" {
            if actions.len() > 1 {
                lists.push(classify(&actions, &names));
            }
            actions.clear();
            continue;
        }
        if let Some(action) = read_action(name, arguments) {
            actions.push(action);
        }
    }
    if actions.len() > 1 {
        lists.push(classify(&actions, &names));
    }
    lists
}

/// Give the class and the symbols of one action list.
fn classify(actions: &[Action], names: &BTreeMap<u32, String>) -> ActionList {
    let reduces: Vec<(u32, u32, i32, u32)> = actions
        .iter()
        .filter_map(|action| match *action {
            Action::Reduce { symbol, children, precedence, production } => {
                Some((symbol, children, precedence, production))
            }
            _ => None,
        })
        .collect();
    let shifts = actions.iter().filter(|action| **action == Action::Shift).count();
    let productions: BTreeSet<(u32, u32, i32, u32)> = reduces.iter().copied().collect();
    let precedences: BTreeSet<i32> = reduces.iter().map(|value| value.2).collect();
    let class = if reduces.len() >= 2 && shifts == 0 {
        if productions.len() == 1 {
            Class::Duplicate
        } else if precedences.len() == 1 {
            Class::ReduceTie
        } else {
            Class::ReducePrecedence
        }
    } else if !reduces.is_empty() && shifts >= 1 {
        Class::ShiftReduce
    } else {
        Class::Other
    };
    // The names and not the symbol ids, because the ids move when the grammar changes and a report
    // of two runs is then not easy to compare.
    let mut symbols: Vec<String> = productions
        .iter()
        .map(|(symbol, ..)| {
            names.get(symbol).cloned().unwrap_or_else(|| format!("symbol {symbol}"))
        })
        .collect();
    symbols.sort_unstable();
    symbols.dedup();
    ActionList { class, symbols }
}

/// Print the report of the parse table.
fn run_table(repository: &Path) -> Result<(), Box<dyn Error>> {
    let path = repository.join("src").join("parser.c");
    let text = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let lists = read_action_lists(&text);
    let mut counts: BTreeMap<Class, usize> = BTreeMap::new();
    let mut sets: BTreeMap<Vec<String>, usize> = BTreeMap::new();
    for list in &lists {
        *counts.entry(list.class).or_default() += 1;
        if list.class == Class::ReduceTie {
            *sets.entry(list.symbols.clone()).or_default() += 1;
        }
    }
    println!("action lists with more than one action: {}", lists.len());
    for (class, count) in &counts {
        println!("{count:>7}  {}", class.name());
    }
    println!();
    println!("the rules of each reduce and reduce list of equal dynamic precedence:");
    let mut rows: Vec<(&Vec<String>, &usize)> = sets.iter().collect();
    rows.sort_by_key(|(symbols, count)| (std::cmp::Reverse(**count), (*symbols).clone()));
    for (symbols, count) in rows {
        println!("{count:>7}  {}", symbols.join(" | "));
    }
    Ok(())
}

/// Read the baseline file of the repository.
fn read_baseline(repository: &Path) -> Result<Vec<Site>, Box<dyn Error>> {
    let path = repository.join(BASELINE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display()).into()),
    };
    Ok(text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(Site::from_row)
        .collect())
}

/// Parse the files of a list, compare the sites with the baseline, and print the report.
///
/// With no ROOT and no LIST the command reads the corpus of `CORPUS` and the list of `SAMPLE_LIST`,
/// which are the two that the baseline of the repository comes from.
fn run_corpus(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let default_list = repository.join(SAMPLE_LIST).display().to_string();
    let (write, root, list) = match args {
        [] => (false, CORPUS.to_owned(), default_list),
        [flag] if flag == "--write-baseline" => (true, CORPUS.to_owned(), default_list),
        [flag, root, list] if flag == "--write-baseline" => (true, root.clone(), list.clone()),
        [root, list] => (false, root.clone(), list.clone()),
        _ => {
            return Err("usage: cargo xtask ties corpus [--write-baseline] [ROOT LIST]".into());
        }
    };
    let root = Path::new(&root);
    let text = fs::read_to_string(&list).map_err(|e| format!("cannot read the file list {list}: {e}"))?;
    let paths: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
    let found = measure(root, &paths).sites;
    println!("files {}, sites {}", paths.len(), found.len());
    if write {
        let path = repository.join(BASELINE);
        fs::create_dir_all(path.parent().expect("the baseline is in a directory"))?;
        let mut out = String::new();
        out.push_str("# The sites where the symbol order selects the tree, and no rule.\n");
        out.push_str("# Refer to xtask/src/ties.rs. The columns are the path, the row, the column,\n");
        out.push_str("# and the message of the runtime.\n");
        for site in &found {
            out.push_str(&site.row_text());
            out.push('\n');
        }
        fs::write(&path, out)?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    let baseline = read_baseline(repository)?;
    let comparison = compare(&baseline, &found);
    println!(
        "baseline {}, found {}, added {}, removed {}, changed {}",
        baseline.len(),
        found.len(),
        comparison.added.len(),
        comparison.removed.len(),
        comparison.changed.len()
    );
    for (before, after) in &comparison.changed {
        println!("changed {}:{}:{}", after.path, after.row, after.column);
        println!("    baseline {}", before.what);
        println!("    now      {}", after.what);
    }
    for site in &comparison.removed {
        println!("gone    {}:{}:{}  {}", site.path, site.row, site.column, site.what);
    }
    for site in &comparison.added {
        println!("NEW     {}:{}:{}  {}", site.path, site.row, site.column, site.what);
    }
    // A REMOVED SITE AND A CHANGED SITE ALSO FAIL, AND THE BASELINE GOES INTO THE COMMIT THAT
    // MOVES THEM. A check that fails for an added site alone leaves each site that a landing
    // removed in the file, and the file then permits that site to come back with no failure. The
    // measurement of 2026-09-16: the baseline held 8 sites that no commit had produced for an
    // unknown number of landings, so 8 sites stood pre-approved. Refer to task 283.
    if comparison.added.is_empty() && comparison.removed.is_empty() && comparison.changed.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} sites where the symbol order selects the tree and the baseline holds none, {} sites of \
         the baseline that are gone, and {} sites whose message changed. An added site is one more \
         tree that a later change of the grammar can flip with no ERROR node. A site that is gone \
         is a repair, and a baseline that keeps it permits it to come back with no failure. Repair \
         an added tie with a rule, and write the baseline again with \
         `cargo xtask ties corpus --write-baseline ROOT LIST` in the commit that moves the sites. \
         Give the reason in the commit message.",
        comparison.added.len(),
        comparison.removed.len(),
        comparison.changed.len()
    )
    .into())
}

/// The value of `MAX_VERSION_COUNT` of the runtime.
///
/// The command reads the value from the source of the runtime, so that a change of the limit moves
/// the report with it.
fn read_max_version_count(repository: &Path) -> Result<usize, Box<dyn Error>> {
    let path = repository.join("vendor/tree-sitter/src/parser.c");
    let text = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let name = "static const unsigned MAX_VERSION_COUNT = ";
    let rest = text
        .split_once(name)
        .ok_or_else(|| format!("{} has no {name}", path.display()))?
        .1;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    Ok(digits.parse()?)
}

/// Read the baseline of the version counts of the repository.
fn read_versions_baseline(repository: &Path) -> Result<Vec<FileVersions>, Box<dyn Error>> {
    let path = repository.join(VERSIONS);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display()).into()),
    };
    Ok(text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(FileVersions::from_row)
        .collect())
}

/// Parse the files of a list and report the files whose version count passes the limit.
///
/// A file that passes the limit loses a version by arrival order. The command fails for such a file
/// that `test/ties/versions.txt` does not hold, and it does not fail for a file of that baseline
/// whose count falls.
fn run_versions(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let default_list = repository.join(SAMPLE_LIST).display().to_string();
    let (write, root, list) = match args {
        [] => (false, CORPUS.to_owned(), default_list),
        [flag] if flag == "--write-baseline" => (true, CORPUS.to_owned(), default_list),
        [flag, root, list] if flag == "--write-baseline" => (true, root.clone(), list.clone()),
        [root, list] => (false, root.clone(), list.clone()),
        _ => return Err("usage: cargo xtask ties versions [--write-baseline] [ROOT LIST]".into()),
    };
    let limit = read_max_version_count(repository)?;
    let root = Path::new(&root);
    let text = fs::read_to_string(&list).map_err(|e| format!("cannot read the file list {list}: {e}"))?;
    let paths: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
    let mut found = measure(root, &paths).versions;
    let largest = found.iter().map(|file| file.versions).max().unwrap_or(0);
    found.retain(|file| file.versions > limit);
    println!(
        "files {}, limit {limit}, largest version count {largest}, files past the limit {}",
        paths.len(),
        found.len()
    );
    if write {
        let path = repository.join(VERSIONS);
        fs::create_dir_all(path.parent().expect("the baseline is in a directory"))?;
        let mut out = String::new();
        out.push_str("# The files whose GLR version count passes MAX_VERSION_COUNT of the runtime.\n");
        out.push_str("# Refer to xtask/src/ties.rs. The columns are the path, the largest version\n");
        out.push_str("# count, the row, and the column.\n");
        for file in &found {
            out.push_str(&file.row_text());
            out.push('\n');
        }
        fs::write(&path, out)?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    let baseline = read_versions_baseline(repository)?;
    let held: BTreeSet<&str> = baseline.iter().map(|file| file.path.as_str()).collect();
    let now: BTreeSet<&str> = found.iter().map(|file| file.path.as_str()).collect();
    // A file of the baseline that the list does not hold is no fall, and the report leaves it out.
    let read: BTreeSet<&str> = paths.iter().copied().collect();
    let added: Vec<&FileVersions> =
        found.iter().filter(|file| !held.contains(file.path.as_str())).collect();
    let gone: Vec<&FileVersions> = baseline
        .iter()
        .filter(|file| read.contains(file.path.as_str()) && !now.contains(file.path.as_str()))
        .collect();
    for file in &gone {
        println!("gone    {} ({} versions)", file.path, file.versions);
    }
    for file in &added {
        println!("NEW     {}:{}:{}  {} versions", file.path, file.row, file.column, file.versions);
    }
    println!(
        "baseline {}, found {}, added {}, gone {}",
        baseline.len(),
        found.len(),
        added.len(),
        gone.len()
    );
    // A FILE THAT IS GONE ALSO FAILS. The baseline of 2026-09-16 held 59 files and the corpus gave
    // 1, so 58 files were permitted to pass the limit again with no failure. Refer to task 283.
    if added.is_empty() && gone.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} files whose version count passes the limit {limit} and the baseline holds none, and {} \
         files of the baseline that no longer pass it. The condense step of the runtime removes a \
         version of such a file by arrival order. A file that no longer passes the limit is a \
         repair, and a baseline that keeps it permits it to pass the limit again with no failure. \
         Repair the ambiguity that makes the versions, and write the baseline again with \
         `cargo xtask ties versions --write-baseline ROOT LIST` in the commit that moves the files.",
        added.len(),
        gone.len()
    )
    .into())
}

/// Read the baseline of the sites of the full corpus.
/// The text of the population baseline: the header and one row for each file and message.
fn baseline_text(found: &[Group]) -> String {
    let mut out = String::new();
    out.push_str("# The count of the sites where the symbol order selects the tree, for each\n");
    out.push_str("# file and each message of the full corpus. Refer to xtask/src/ties.rs.\n");
    out.push_str("# The columns are the path, the count, and the message of the runtime.\n");
    out.push_str("#\n");
    out.push_str("# WHAT THIS FILE PROMISES: a count that rises fails the check, and a count\n");
    out.push_str("# that falls does not. A message that moves to a different class in one file\n");
    out.push_str("# gives one fall and one rise, and the rise fails the check.\n");
    out.push_str("# WHAT IT DOES NOT PROMISE: a site that moves to a different row or column of\n");
    out.push_str("# one file, with no change of the count, fails no check. The per-site\n");
    out.push_str("# baseline test/ties/baseline.txt covers that case for the 3,000 files of\n");
    out.push_str("# test/ties/sample.txt.\n");
    out.push_str("# A COUNT THAT MOVES AT ALL, UP OR DOWN, IS A REASON TO RUN `cargo xtask ties\n");
    out.push_str("# flip`. The flip reads whether the tree of a file DEPENDS on the order of the\n");
    out.push_str("# symbol ids, which is the question that no count in this file can answer.\n");
    out.push_str("#\n");
    out.push_str("# THE FILE HOLDS THE MEASUREMENT OF THE FORK MASTER. integrate.sh writes it\n");
    out.push_str("# again at each landing whose gate found fallen counts, with the rows of that\n");
    out.push_str("# gate. So a gate that reads `fallen 0` measures a commit that moves no tie,\n");
    out.push_str("# and a count that is not zero is the work of the commit under that gate. A\n");
    out.push_str("# file that is not refreshed reports the falls of an earlier landing to each\n");
    out.push_str("# later gate, and the count then measures history and not the commit.\n");
    for row in found {
        out.push_str(&row.row_text());
        out.push('\n');
    }
    out
}

fn read_population_baseline(repository: &Path) -> Result<Vec<Group>, Box<dyn Error>> {
    let path = repository.join(POPULATION);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display()).into()),
    };
    Ok(text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(Group::from_row)
        .collect())
}

/// Parse the files of a list and compare the count of each file and message with the baseline.
///
/// The full corpus of 329,387 files gives 101,100 sites, and one generated file of boost/libs/qvm
/// gives 45,826 of them. A per-site baseline of that is 11 MB, and no person reads it. This report
/// holds one row for each file and message with a count, and it recovers the positions of one file
/// with `cargo xtask ties corpus ROOT LIST` on that file.
fn run_population(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    const USAGE: &str = "usage: cargo xtask ties population [--write-baseline] ROOT LIST [--write PATH]";
    let (write, root, list, measured) = match args {
        [flag, root, list] if flag == "--write-baseline" => (true, root.clone(), list.clone(), None),
        [root, list] => (false, root.clone(), list.clone(), None),
        [root, list, flag, path] if flag == "--write" => {
            (false, root.clone(), list.clone(), Some(path.clone()))
        }
        _ => return Err(USAGE.into()),
    };
    let root = Path::new(&root);
    let text = fs::read_to_string(&list).map_err(|e| format!("cannot read the file list {list}: {e}"))?;
    let paths: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
    let sites = measure(root, &paths).sites;
    let found = group(&sites);
    println!("files {}, sites {}, rows {}", paths.len(), sites.len(), found.len());
    if write {
        let path = repository.join(POPULATION);
        fs::create_dir_all(path.parent().expect("the baseline is in a directory"))?;
        fs::write(&path, baseline_text(&found))?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    // The check writes the rows that it measured, so that a landing installs the baseline of the
    // gate that passed and never of a second run. Refer to `install_tie_baseline` in integrate.sh.
    if let Some(path) = &measured {
        fs::write(path, baseline_text(&found)).map_err(|e| format!("cannot write {path}: {e}"))?;
        println!("measured rows {path}");
    }
    let baseline = read_population_baseline(repository)?;
    let read: BTreeSet<&str> = paths.iter().copied().collect();
    let comparison = compare_groups(&baseline, &found, &read);
    for (row, after) in &comparison.fallen {
        println!("fell    {} {} -> {}  {}", row.path, row.count, after, row.what);
    }
    // A COUNT THAT FALLS CAN BE A REPAIR OR A MOVE, and this check cannot tell them apart. A row
    // holds the file, the message of the runtime, and a COUNT, and it holds no position. A commit
    // that repairs one site of a file and makes a new site of the same message in the same file
    // keeps the count, and nothing here fires. The per-site baseline test/ties/baseline.txt holds
    // the row and the column of each site, and it covers the 3,000 files of test/ties/sample.txt
    // only. THE FLIP READS WHETHER THE TREE DEPENDS ON THE ORDER, which is the question that a
    // count cannot answer, so the instruction comes with every move of a count and not with a rise.
    if !comparison.fallen.is_empty() {
        println!(
            "{} counts fell. A COUNT THAT MOVES AT ALL IS A REASON TO RUN THE FLIP: `cargo xtask \
             ties flip ROOT LIST --reference CACHE/<base>/target/release/xtask` says whether the \
             tree of each file DEPENDS on the order of the symbol ids, and whether these commits \
             changed it. A fall with no flip is a repair that nobody read, or a site that moved to \
             a different row of the same file with the same count.",
            comparison.fallen.len()
        );
    }
    for (row, before) in &comparison.risen {
        println!("ROSE    {} {} -> {}  {}", row.path, before, row.count, row.what);
    }
    println!(
        "baseline {}, found {}, risen {}, fallen {}",
        baseline.len(),
        found.len(),
        comparison.risen.len(),
        comparison.fallen.len()
    );
    if comparison.risen.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} counts of sites where the symbol order selects the tree that the baseline does not \
         hold. Each new site is one more tree that a later change of the grammar can flip with no \
         ERROR node. `cargo xtask ties corpus ROOT LIST` with a list of the file above gives the \
         row and the column of each site, and `cargo xtask ties flip ROOT LIST --reference \
         CACHE/<base>/target/release/xtask` says whether the tree of that file DEPENDS on the \
         order and whether these commits changed it. Repair the tie with a rule, or write the \
         baseline again with `cargo xtask ties population --write-baseline ROOT LIST` and give the \
         reason in the commit message.",
        comparison.risen.len()
    )
    .into())
}

/// Print the parse log of one file.
fn run_trace(args: &[String]) -> Result<(), Box<dyn Error>> {
    let (file, state) = match args {
        [file] => (file.clone(), None),
        [file, flag, value] if flag == "--state" => {
            (file.clone(), Some(value.parse::<usize>().map_err(|e| format!("--state: {e}"))?))
        }
        _ => return Err("usage: cargo xtask ties trace FILE [--state N]".into()),
    };
    let source = fs::read(&file).map_err(|e| format!("cannot read {file}: {e}"))?;
    let language = Language::new(tree_sitter_cpp::LANGUAGE);
    let mut parser = Parser::new();
    parser.set_language(&language)?;
    let rows = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&rows);
    parser.set_logger(Some(Box::new(move |kind, message| {
        if kind != LogType::Parse {
            return;
        }
        let keep = match state {
            None => true,
            Some(wanted) => {
                message.starts_with("process version:") && field(message, "state:") == Some(wanted)
            }
        };
        if keep {
            sink.lock().expect("the logger holds the rows").push(message.to_owned());
        }
    })));
    let tree = crate::corpus::parse_with_limit(&mut parser, &source, &file);
    for row in rows.lock().expect("the reader holds the rows").iter() {
        println!("{row}");
    }
    if let Err(stop) = tree {
        println!("the parse stopped at {stop}");
    }
    Ok(())
}

/// The two rows of `ts_subtree_compare` that the flip turns around, and the rows that replace them.
///
/// `ts_subtree_compare` (vendor/tree-sitter/src/subtree.c) compares the numeric symbol ids of two
/// subtrees, and `ts_parser__select_tree` takes the tree whose first differing node has the smaller
/// id. A build with the two rows turned around takes the other tree at each tie, and a file whose
/// tree differs between the two builds is a file that no rule decides.
const COMPARE_ROWS: &str = "\
    if (ts_subtree_symbol(left) < ts_subtree_symbol(right)) result = -1;
    else if (ts_subtree_symbol(right) < ts_subtree_symbol(left)) result = 1;";
const FLIPPED_ROWS: &str = "\
    if (ts_subtree_symbol(left) < ts_subtree_symbol(right)) result = 1;
    else if (ts_subtree_symbol(right) < ts_subtree_symbol(left)) result = -1;";

/// A file that holds its first bytes and writes them again when it goes.
///
/// THE RESTORE RUNS ON EACH PATH, ALSO ON A PANIC, because `drop` runs while the panic unwinds. A
/// build that stops in the middle must leave no changed file in the clone: the next gate reads a
/// working tree that is not clean, and the agent then looks for a change that it did not make.
struct Restore {
    path: PathBuf,
    text: String,
}

impl Restore {
    /// Read a file and hold its bytes.
    fn new(path: &Path) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            path: path.to_owned(),
            text: fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?,
        })
    }

    /// Write a new text into the file. The drop writes the first bytes again.
    fn write(&self, text: &str) -> Result<(), Box<dyn Error>> {
        fs::write(&self.path, text).map_err(|e| format!("cannot write {}: {e}", self.path.display()).into())
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        if let Err(error) = fs::write(&self.path, &self.text) {
            eprintln!(
                "ties flip: cannot write {} again: {error}. The file holds the flipped comparison, and \
                 `git diff` gives the change. No parse of this run used it.",
                self.path.display()
            );
        }
    }
}

/// The tree of one file, as the text that `xtask parse` prints.
fn parse_with(binary: &Path, file: &Path) -> Result<String, Box<dyn Error>> {
    let out = std::process::Command::new(binary)
        .arg("parse")
        .arg(file)
        .output()
        .map_err(|e| format!("cannot run {} for {}: {e}", binary.display(), file.display()))?;
    if !out.status.success() {
        return Err(format!(
            "{} parse {} gave {}: {}",
            binary.display(),
            file.display(),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The first row where two trees differ, as the row number and the two rows.
///
/// `xtask parse` writes one node in each row, so the first row that differs names the two node
/// kinds and their place.
fn first_difference(a: &str, b: &str) -> Option<(usize, String, String)> {
    a.lines()
        .zip(b.lines())
        .enumerate()
        .find(|(_, (x, y))| x != y)
        .map(|(index, (x, y))| (index + 1, x.trim().to_owned(), y.trim().to_owned()))
        .or_else(|| {
            let (rows_a, rows_b) = (a.lines().count(), b.lines().count());
            (rows_a != rows_b).then(|| {
                (
                    rows_a.min(rows_b) + 1,
                    format!("{rows_a} rows"),
                    format!("{rows_b} rows"),
                )
            })
        })
}

/// Parse each file with the parser of this build, with a build whose tie comparison is turned
/// around, and with the parser of the reference. Report the two answers apart.
///
/// THE TWO ANSWERS ANSWER TWO QUESTIONS, AND A READER THAT JOINS THEM CHARGES THE WRONG COMMIT.
/// The reference against this build says whether the commits changed the tree of a file at all.
/// This build against the flipped build says whether the tree of that file depends on the order of
/// the numeric symbol ids, which no rule states. A file that is order-dependent and that the
/// commits did not change was order-dependent before them.
///
/// The two builds come from ONE `src/parser.c`, so the only difference between them is the
/// runtime. Refer to task 284, where r48 ran the procedure by hand twice.
fn run_flip(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    const USAGE: &str = "usage: cargo xtask ties flip ROOT LIST --reference BINARY|none [--target DIRECTORY]\n       \
                         cargo xtask ties flip ROOT --rose TIES_LOG --reference BINARY|none [--target DIRECTORY]";
    let (mut root, mut list, mut rose, mut reference, mut target) = (None, None, None, None, None);
    let mut rest = args.iter();
    while let Some(argument) = rest.next() {
        let mut value = |name: &str| -> Result<String, Box<dyn Error>> {
            rest.next()
                .cloned()
                .ok_or_else(|| format!("{name} takes a value\n\n{USAGE}").into())
        };
        match argument.as_str() {
            "--rose" => rose = Some(value("--rose")?),
            "--reference" => reference = Some(value("--reference")?),
            "--target" => target = Some(value("--target")?),
            text if text.starts_with("--") => return Err(format!("the option {text} is unknown\n\n{USAGE}").into()),
            text if root.is_none() => root = Some(text.to_owned()),
            text if list.is_none() => list = Some(text.to_owned()),
            _ => return Err(USAGE.into()),
        }
    }
    let root = root.ok_or(USAGE)?;
    // THE REFERENCE IS NOT OPTIONAL BY ACCIDENT. Step 7 of the procedure is the step that a reader
    // skips, and without it a file that was order-dependent before the commits gets charged to
    // them. `--reference none` states that the reader accepts that limit.
    let reference = reference.ok_or_else(|| {
        format!(
            "--reference is necessary. Give the xtask of the base of the gate, which is \
             CACHE/<base commit>/target/release/xtask, or `--reference none` to read the flip alone. \
             Without the reference, a file that was order-dependent before these commits reads as a \
             file that they made order-dependent.\n\n{USAGE}"
        )
    })?;
    let paths: Vec<String> = match (&list, &rose) {
        (Some(list), None) => fs::read_to_string(list)
            .map_err(|e| format!("cannot read the file list {list}: {e}"))?
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
        (None, Some(log)) => {
            // The `ROSE` rows of a tie report name the files whose site count rose.
            let text = fs::read_to_string(log).map_err(|e| format!("cannot read the tie report {log}: {e}"))?;
            let mut names: Vec<String> = text
                .lines()
                .filter_map(|line| line.strip_prefix("ROSE"))
                .filter_map(|line| line.split_whitespace().next())
                .map(str::to_owned)
                .collect();
            names.sort_unstable();
            names.dedup();
            names
        }
        _ => return Err(USAGE.into()),
    };
    if paths.is_empty() {
        println!("ties flip: the list holds no file, so this run compares nothing.");
        return Ok(());
    }
    let normal = std::env::current_exe().map_err(|e| format!("cannot find the xtask of this build: {e}"))?;
    let target = target.map_or_else(|| repository.join("target").join("flip"), PathBuf::from);
    let flipped = target.join("release").join("xtask");

    // The build of the flipped parser. The guard writes the file again when it goes out of scope,
    // which is before the first parse and also on a panic.
    {
        let subtree = repository.join("vendor").join("tree-sitter").join("src").join("subtree.c");
        let restore = Restore::new(&subtree)?;
        let count = restore.text.matches(COMPARE_ROWS).count();
        if count != 1 {
            return Err(format!(
                "{} holds the comparison of the symbols {count} times, and this command writes it one time. \
                 Read `ts_subtree_compare` and write the rows of `COMPARE_ROWS` again.",
                subtree.display()
            )
            .into());
        }
        restore.write(&restore.text.replace(COMPARE_ROWS, FLIPPED_ROWS))?;
        println!("ties flip: the build with the comparison turned around goes to {}", target.display());
        let status = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
            .args(["build", "--release", "--workspace"])
            .env("CARGO_TARGET_DIR", &target)
            .current_dir(repository)
            .status()
            .map_err(|e| format!("cannot run cargo: {e}"))?;
        if !status.success() {
            return Err(format!("the build of the flipped parser gave {status}").into());
        }
    }
    // The clone holds its own files again from here.
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain", "--", "vendor/tree-sitter/src/subtree.c"])
        .current_dir(repository)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !dirty.stdout.is_empty() {
        return Err(format!(
            "vendor/tree-sitter/src/subtree.c is not the file of the commit after the build: {}",
            String::from_utf8_lossy(&dirty.stdout).trim()
        )
        .into());
    }

    let root = Path::new(&root);
    let (mut changed, mut order) = (Vec::new(), Vec::new());
    for rel in &paths {
        let file = root.join(rel);
        let ours = parse_with(&normal, &file)?;
        let other = parse_with(&flipped, &file)?;
        if let Some(difference) = first_difference(&ours, &other) {
            order.push((rel.clone(), difference));
        }
        if reference != "none" {
            let before = parse_with(Path::new(&reference), &file)?;
            if let Some(difference) = first_difference(&before, &ours) {
                changed.push((rel.clone(), difference));
            }
        }
    }

    let show = |title: &str, rows: &[(String, (usize, String, String))], left: &str, right: &str| {
        println!("\n{title}: {} of {} files", rows.len(), paths.len());
        for (rel, (row, a, b)) in rows {
            println!("  {rel}");
            println!("    the first difference is in row {row}");
            println!("      {left:<10}{a}");
            println!("      {right:<10}{b}");
        }
    };
    if reference == "none" {
        println!("\nthe commits against the reference: not measured, because --reference none");
    } else {
        show("the commits against the reference", &changed, "reference", "new");
    }
    show("the tie order", &order, "new", "flipped");

    // The classes of the sites of each file whose tree depends on the order. A class names the
    // construct, and the agent reads whether its own commit added that class.
    for (rel, _) in &order {
        let run = measure(root, &[rel.as_str()]);
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for site in &run.sites {
            *counts.entry(site.what.as_str()).or_default() += 1;
        }
        let list: Vec<String> = counts.iter().map(|(what, count)| format!("{count} {what}")).collect();
        let also = changed.iter().any(|(path, _)| path == rel);
        println!("\n{rel}");
        println!("  the sites of this file: {}", list.join(", "));
        if reference == "none" {
            println!("  the reference is not measured, so this run does not say whether the commits made it so.");
        } else if also {
            println!("  the commits changed the tree of this file, and the tree depends on the order.");
        } else {
            println!("  the commits did not change the tree of this file, so it was order-dependent before them.");
        }
    }
    println!(
        "\nties flip: {} files, {} with a tree that depends on the order of the symbol ids, {}.",
        paths.len(),
        order.len(),
        if reference == "none" {
            "and the reference is not measured".to_owned()
        } else {
            format!("{} with a tree that the commits changed", changed.len())
        }
    );
    Ok(())
}

/// Run one of the reports.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    match args.first().map(String::as_str) {
        Some("table") => run_table(repository),
        Some("flip") => run_flip(repository, &args[1..]),
        Some("corpus") => run_corpus(repository, &args[1..]),
        Some("population") => run_population(repository, &args[1..]),
        Some("versions") => run_versions(repository, &args[1..]),
        Some("trace") => run_trace(&args[1..]),
        _ => Err("usage: cargo xtask ties table | corpus [--write-baseline] [ROOT LIST] | \
                    population [--write-baseline] ROOT LIST [--write PATH] | \
                    versions [--write-baseline] [ROOT LIST] | trace FILE [--state N] | \
                    flip ROOT LIST --reference BINARY|none"
            .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small parser file in the format of the generator.
    const SAMPLE: &str = "\
enum ts_symbol_identifiers {
  sym_identifier = 1,
  sym_expression = 2,
  sym_type_specifier = 3,
  aux_sym_list_repeat1 = 4,
};
static const char * const ts_symbol_names[] = {
  [sym_identifier] = \"identifier\",
  [sym_expression] = \"expression\",
  [sym_type_specifier] = \"type_specifier\",
};
static const TSParseActionEntry ts_parse_actions[] = {
E(0,0),E(1,0),RC(),E(2,1),R(2,1,0,0),R(3,1,0,2),
E(2,1),R(2,1,0,0),R(3,1,5,2),E(2,0),S(7),R(4,2,0,0),
E(2,1),R(2,1,0,0),R(2,1,0,0),E(1,1),S(9),
};
";

    fn site(path: &str, row: usize, column: usize, what: &str) -> Site {
        Site { path: path.to_owned(), row, column, what: what.to_owned() }
    }

    #[test]
    fn the_symbol_names_come_from_the_enum_and_from_the_name_table() {
        let names = read_symbol_names(SAMPLE);
        assert_eq!(names.get(&2), Some(&"expression".to_owned()));
        assert_eq!(names.get(&3), Some(&"type_specifier".to_owned()));
        // A hidden symbol has no row in the name table, and it keeps the name of the enum.
        assert_eq!(names.get(&4), Some(&"aux_sym_list_repeat1".to_owned()));
    }

    #[test]
    fn each_action_list_with_more_than_one_action_gets_its_class() {
        let lists = read_action_lists(SAMPLE);
        let classes: Vec<Class> = lists.iter().map(|list| list.class).collect();
        assert_eq!(
            classes,
            vec![Class::ReduceTie, Class::ReducePrecedence, Class::ShiftReduce, Class::Duplicate]
        );
        assert_eq!(lists[0].symbols, vec!["expression".to_owned(), "type_specifier".to_owned()]);
        assert_eq!(lists[3].symbols, vec!["expression".to_owned()]);
    }

    #[test]
    fn a_site_that_the_baseline_does_not_hold_is_added_and_a_site_that_is_gone_is_removed() {
        let baseline = vec![site("a.cpp", 1, 2, "select_earlier symbol:x, over_symbol:x")];
        let found = vec![
            site("a.cpp", 1, 2, "select_earlier symbol:x, over_symbol:x"),
            site("b.cpp", 3, 4, "select_earlier symbol:y, over_symbol:y"),
        ];
        let comparison = compare(&baseline, &found);
        assert_eq!(comparison.added, vec![site("b.cpp", 3, 4, "select_earlier symbol:y, over_symbol:y")]);
        assert!(comparison.removed.is_empty());
        assert!(comparison.changed.is_empty());

        let comparison = compare(&found, &baseline);
        assert!(comparison.added.is_empty());
        assert_eq!(comparison.removed, vec![site("b.cpp", 3, 4, "select_earlier symbol:y, over_symbol:y")]);
    }

    #[test]
    fn a_site_with_a_different_message_at_one_position_is_changed_and_not_added() {
        let baseline = vec![site("a.cpp", 1, 2, "select_earlier symbol:x, over_symbol:x")];
        let found = vec![site("a.cpp", 1, 2, "select_earlier symbol:y, over_symbol:y")];
        let comparison = compare(&baseline, &found);
        assert!(comparison.added.is_empty());
        assert!(comparison.removed.is_empty());
        assert_eq!(comparison.changed.len(), 1);
    }

    #[test]
    fn two_ties_at_one_position_give_one_added_site_when_the_baseline_holds_one() {
        let baseline = vec![site("a.cpp", 1, 2, "select_earlier symbol:x, over_symbol:x")];
        let found = vec![
            site("a.cpp", 1, 2, "select_earlier symbol:x, over_symbol:x"),
            site("a.cpp", 1, 2, "select_earlier symbol:x, over_symbol:x"),
        ];
        let comparison = compare(&baseline, &found);
        assert_eq!(comparison.added.len(), 1);
    }

    #[test]
    fn a_row_of_the_baseline_reads_again_as_the_same_site() {
        let one = site("a/b.cpp", 12, 34, "select_earlier symbol:x, over_symbol:y");
        assert_eq!(Site::from_row(&one.row_text()), Some(one));
    }

    #[test]
    fn the_position_of_a_tie_comes_from_the_last_step_of_the_parser() {
        let mut log = Log { path: "a.cpp".to_owned(), ..Log::default() };
        read_message(&mut log, "process version:0, version_count:2, state:5, row:7, col:9");
        read_message(&mut log, "select_earlier symbol:expression, over_symbol:expression");
        read_message(&mut log, "select_higher_precedence symbol:x, prec:1, over_symbol:y, other_prec:0");
        assert_eq!(log.sites, vec![site("a.cpp", 8, 10, "select_earlier symbol:expression, over_symbol:expression")]);
    }

    /// The directory of the repository.
    fn repository() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the xtask crate is in a directory of the repository")
            .to_owned()
    }

    /// The baseline of the repository reads again, and each row has a path, a row, and a column.
    #[test]
    fn the_baseline_of_the_repository_reads_again() {
        let baseline = read_baseline(&repository()).expect("the baseline reads");
        assert!(!baseline.is_empty(), "test/ties/baseline.txt holds no site");
        for site in &baseline {
            assert!(!site.path.is_empty());
            assert!(site.row > 0 && site.column > 0);
            assert!(TIE_MESSAGES.iter().any(|name| site.what.starts_with(name)));
        }
    }

    /// The parse table holds each rule set that the baseline names.
    ///
    /// A site of the baseline comes from an action list of the parse table with two reduce actions
    /// of equal dynamic precedence. The two reports then agree, and a repair of a rule set shows in
    /// the two of them.
    #[test]
    fn the_parse_table_holds_a_reduce_and_reduce_list_of_equal_precedence() {
        let path = repository().join("src").join("parser.c");
        let text = fs::read_to_string(&path).expect("src/parser.c reads");
        let lists = read_action_lists(&text);
        let ties = lists.iter().filter(|list| list.class == Class::ReduceTie).count();
        assert!(ties > 0, "the parse table holds no reduce and reduce list of equal precedence");
        assert!(lists.iter().any(|list| list.class == Class::ShiftReduce));
    }

    /// The report of the flip names the first row where two trees differ, and `xtask parse` writes
    /// one node in each row, so that row names the two node kinds.
    #[test]
    fn the_first_difference_of_two_trees_is_the_first_row_that_differs() {
        let a = "(translation_unit\n  (declaration\n    (identifier)))\n";
        let b = "(translation_unit\n  (expression_statement\n    (identifier)))\n";
        assert_eq!(
            first_difference(a, b),
            Some((2, "(declaration".to_owned(), "(expression_statement".to_owned()))
        );
        assert_eq!(first_difference(a, a), None);
        // A tree with more rows differs after the last row that the two hold.
        let short = "(translation_unit\n  (declaration\n";
        assert_eq!(
            first_difference(short, a),
            Some((3, "2 rows".to_owned(), "3 rows".to_owned()))
        );
    }

    /// The rows that the flip writes into the runtime are the rows of the runtime, turned around.
    ///
    /// A change of `ts_subtree_compare` that leaves the two rows in a different shape makes the
    /// command fail with a message, and this test names the file before that happens.
    #[test]
    fn the_runtime_holds_the_rows_that_the_flip_turns_around() {
        let path = repository().join("vendor").join("tree-sitter").join("src").join("subtree.c");
        let text = fs::read_to_string(&path).expect("the runtime reads");
        assert_eq!(
            text.matches(COMPARE_ROWS).count(),
            1,
            "{} holds the comparison of the symbols of `ts_subtree_compare` one time, and the flip \
             of task 284 writes it again",
            path.display()
        );
        assert!(!text.contains(FLIPPED_ROWS), "the runtime holds the rows of the flip");
        // The flip turns the two results around and leaves the two conditions as they are.
        assert_eq!(
            COMPARE_ROWS.replace("result = -1", "result = X").replace("result = 1", "result = -1").replace("result = X", "result = 1"),
            FLIPPED_ROWS
        );
    }

    fn one_group(path: &str, count: usize, what: &str) -> Group {
        Group { path: path.to_owned(), what: what.to_owned(), count }
    }

    #[test]
    fn the_sites_of_one_file_and_one_message_go_into_one_group() {
        let sites = vec![
            site("a.cpp", 1, 2, "select_earlier symbol:x, over_symbol:x"),
            site("a.cpp", 5, 6, "select_earlier symbol:x, over_symbol:x"),
            site("a.cpp", 7, 8, "select_earlier symbol:y, over_symbol:y"),
            site("b.cpp", 1, 1, "select_earlier symbol:x, over_symbol:x"),
        ];
        assert_eq!(
            group(&sites),
            vec![
                one_group("a.cpp", 2, "select_earlier symbol:x, over_symbol:x"),
                one_group("a.cpp", 1, "select_earlier symbol:y, over_symbol:y"),
                one_group("b.cpp", 1, "select_earlier symbol:x, over_symbol:x"),
            ]
        );
    }

    #[test]
    fn a_count_that_rises_is_risen_and_a_count_that_falls_is_fallen() {
        let baseline = vec![one_group("a.cpp", 2, "m"), one_group("b.cpp", 3, "m")];
        let found = vec![one_group("a.cpp", 5, "m"), one_group("b.cpp", 1, "m")];
        let read: BTreeSet<&str> = ["a.cpp", "b.cpp"].into_iter().collect();
        let comparison = compare_groups(&baseline, &found, &read);
        assert_eq!(comparison.risen, vec![(one_group("a.cpp", 5, "m"), 2)]);
        assert_eq!(comparison.fallen, vec![(one_group("b.cpp", 3, "m"), 1)]);
    }

    #[test]
    fn a_group_that_the_baseline_does_not_hold_has_the_count_zero_before_it() {
        let baseline = vec![one_group("a.cpp", 2, "m")];
        let found = vec![one_group("a.cpp", 2, "m"), one_group("a.cpp", 1, "n")];
        let read: BTreeSet<&str> = ["a.cpp"].into_iter().collect();
        let comparison = compare_groups(&baseline, &found, &read);
        assert_eq!(comparison.risen, vec![(one_group("a.cpp", 1, "n"), 0)]);
        assert!(comparison.fallen.is_empty());
    }

    /// A run that reads a part of the files reports no fall for the files that it did not read.
    #[test]
    fn a_file_that_the_run_does_not_read_gives_no_fall() {
        let baseline = vec![one_group("a.cpp", 2, "m"), one_group("b.cpp", 3, "m")];
        let found = vec![one_group("a.cpp", 2, "m")];
        let read: BTreeSet<&str> = ["a.cpp"].into_iter().collect();
        let comparison = compare_groups(&baseline, &found, &read);
        assert!(comparison.risen.is_empty());
        assert!(comparison.fallen.is_empty());
    }

    #[test]
    fn a_row_of_the_population_baseline_reads_again_as_the_same_group() {
        let one = one_group("a/b.cpp", 12, "select_earlier symbol:x, over_symbol:y");
        assert_eq!(Group::from_row(&one.row_text()), Some(one));
    }

    /// The population baseline reads again, and each row has a path, a count, and a message.
    #[test]
    fn the_population_baseline_of_the_repository_reads_again() {
        let baseline = read_population_baseline(&repository()).expect("the baseline reads");
        assert!(!baseline.is_empty(), "test/ties/population.txt holds no row");
        for row in &baseline {
            assert!(!row.path.is_empty());
            assert!(row.count > 0);
            assert!(TIE_MESSAGES.iter().any(|name| row.what.starts_with(name)));
        }
    }

    /// The value of `MAX_VERSION_COUNT` comes from the source of the runtime.
    #[test]
    fn the_version_limit_comes_from_the_runtime() {
        let limit = read_max_version_count(&repository()).expect("the runtime has the limit");
        assert!(limit >= 8, "MAX_VERSION_COUNT is {limit}");
    }

    /// The baseline of the version counts reads again, and each count passes the limit.
    #[test]
    fn the_version_baseline_of_the_repository_reads_again() {
        let repository = repository();
        let limit = read_max_version_count(&repository).expect("the runtime has the limit");
        let baseline = read_versions_baseline(&repository).expect("the baseline reads");
        assert!(!baseline.is_empty(), "test/ties/versions.txt holds no file");
        for file in &baseline {
            assert!(!file.path.is_empty());
            assert!(file.versions > limit, "{} has {} versions", file.path, file.versions);
        }
    }

    /// The sites of the sample list of the corpus agree with the baseline.
    ///
    /// The test fails for a site that the baseline does not hold, for a site of the baseline that is
    /// gone, and for a site whose message changed. A repair of a tie must write the baseline again
    /// in its own commit, with `cargo xtask ties corpus --write-baseline`, because a baseline that
    /// keeps a site that is gone permits that site to come back with no failure. A machine with no
    /// corpus in `CORPUS` reads no file, and the test then compares nothing. The test also compares
    /// the files whose version count passes `MAX_VERSION_COUNT` with `test/ties/versions.txt`, in
    /// the two directions.
    #[test]
    fn the_sites_of_the_sample_hold_no_site_that_the_baseline_does_not_hold() {
        let repository = repository();
        let root = Path::new(CORPUS);
        if !root.is_dir() {
            eprintln!("{CORPUS} is no directory, and the comparison reads no file");
            return;
        }
        let text = fs::read_to_string(repository.join(SAMPLE_LIST)).expect("the sample list reads");
        let paths: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
        let baseline = read_baseline(&repository).expect("the baseline reads");
        let run = measure(root, &paths);
        let found = run.sites;

        let limit = read_max_version_count(&repository).expect("the runtime has the limit");
        let versions = read_versions_baseline(&repository).expect("the version baseline reads");
        let held: BTreeSet<&str> = versions.iter().map(|file| file.path.as_str()).collect();
        let over: Vec<&FileVersions> = run
            .versions
            .iter()
            .filter(|file| file.versions > limit && !held.contains(file.path.as_str()))
            .collect();
        assert!(
            over.is_empty(),
            "{} files whose version count passes {limit} and test/ties/versions.txt holds none: {:?}",
            over.len(),
            over.iter().take(5).map(|file| &file.path).collect::<Vec<_>>()
        );
        let read: BTreeSet<&str> = paths.iter().copied().collect();
        let over_now: BTreeSet<&str> = run
            .versions
            .iter()
            .filter(|file| file.versions > limit)
            .map(|file| file.path.as_str())
            .collect();
        let gone: Vec<&str> = versions
            .iter()
            .map(|file| file.path.as_str())
            .filter(|path| read.contains(path) && !over_now.contains(path))
            .collect();
        assert!(
            gone.is_empty(),
            "{} files of test/ties/versions.txt no longer pass {limit}: {:?}. Write the baseline again \
             with `cargo xtask ties versions --write-baseline` in the commit that repairs them, because \
             a baseline that keeps them permits them to pass the limit again with no failure.",
            gone.len(),
            gone.iter().take(5).collect::<Vec<_>>()
        );

        let comparison = compare(&baseline, &found);
        let show = |sites: &[Site]| {
            sites
                .iter()
                .take(10)
                .map(|site| format!("{}:{}:{}  {}", site.path, site.row, site.column, site.what))
                .collect::<Vec<String>>()
                .join("\n")
        };
        assert!(
            comparison.added.is_empty(),
            "{} sites where the symbol order selects the tree and the baseline holds none:\n{}",
            comparison.added.len(),
            show(&comparison.added)
        );
        assert!(
            comparison.removed.is_empty() && comparison.changed.is_empty(),
            "{} sites of test/ties/baseline.txt are gone and {} changed their message. Write the baseline \
             again with `cargo xtask ties corpus --write-baseline` in the commit that moves them, because \
             a baseline that keeps a site that is gone permits it to come back with no failure.\n{}",
            comparison.removed.len(),
            comparison.changed.len(),
            show(&comparison.removed)
        );
    }
}
