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
//! `cargo xtask ties table` reads `src/parser.c` and reports each action list of the parse table
//! that holds more than one action, by the rules that the actions reduce. That report gives the
//! shape of the ambiguity, and the corpus report gives the texts that reach it.
//!
//! `cargo xtask ties trace FILE` prints the parse log of one file. With `--state N` it prints only
//! the rows where the parser enters the state N, which tells which text reaches a state of the
//! parse table.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::Path;
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

/// Parse each file of a list and give each site where the symbol order selected the tree.
///
/// O(n) in the bytes of the files. The parse with a logger is approximately ten times the parse
/// with no logger, because the runtime writes a message for each character of the lexer.
pub fn measure(root: &Path, paths: &[&str]) -> Vec<Site> {
    let language = Language::new(tree_sitter_cpp::LANGUAGE);
    let mut sites: Vec<Site> = paths
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
                let Ok(source) = fs::read(root.join(path)) else {
                    return Vec::new();
                };
                {
                    let mut log = shared.lock().expect("the reader holds one log");
                    log.path.clear();
                    log.path.push_str(path);
                    log.row = 0;
                    log.column = 0;
                    log.sites.clear();
                }
                let tree = crate::corpus::parse_with_limit(parser, &source);
                let mut log = shared.lock().expect("the reader holds one log");
                let sites = std::mem::take(&mut log.sites);
                drop(tree);
                sites
            },
        )
        .flatten()
        .collect();
    sites.sort_unstable();
    sites
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
    let found = measure(root, &paths);
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
    if comparison.added.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} sites where the symbol order selects the tree and the baseline holds none. \
         Each of them is one more tree that a later change of the grammar can flip with no ERROR \
         node. Repair the tie with a rule, or write the baseline again with \
         `cargo xtask ties corpus --write-baseline ROOT LIST` and give the reason in the commit \
         message.",
        comparison.added.len()
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
    let tree = crate::corpus::parse_with_limit(&mut parser, &source);
    for row in rows.lock().expect("the reader holds the rows").iter() {
        println!("{row}");
    }
    if tree.is_none() {
        println!("the budget stopped the parse");
    }
    Ok(())
}

/// Run one of the three reports.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    match args.first().map(String::as_str) {
        Some("table") => run_table(repository),
        Some("corpus") => run_corpus(repository, &args[1..]),
        Some("trace") => run_trace(&args[1..]),
        _ => Err("usage: cargo xtask ties table | corpus [--write-baseline] [ROOT LIST] | trace FILE [--state N]".into()),
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

    /// The sites of the sample list of the corpus agree with the baseline.
    ///
    /// The test fails for a site that the baseline does not hold, and it does not fail for a site of
    /// the baseline that is gone. A repair of a tie must never fail its own check. A machine with no
    /// corpus in `CORPUS` reads no file, and the test then compares nothing.
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
        let found = measure(root, &paths);
        let comparison = compare(&baseline, &found);
        let names: Vec<String> = comparison
            .added
            .iter()
            .take(10)
            .map(|site| format!("{}:{}:{}  {}", site.path, site.row, site.column, site.what))
            .collect();
        assert!(
            comparison.added.is_empty(),
            "{} sites where the symbol order selects the tree and the baseline holds none:\n{}",
            comparison.added.len(),
            names.join("\n")
        );
    }
}
