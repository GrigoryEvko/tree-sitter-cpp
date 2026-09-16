//! The files whose tree depends on the dedupe of the parse stack.
//!
//! `stack_node_add_link` of vendor/tree-sitter/src/stack.c drops one of two links that hold a
//! subtree of the same symbol, the same size, the same padding and the same child count, AND IT
//! NEVER COMPARES THE CHILDREN. The dynamic precedence decides, and an equal precedence keeps the
//! link that came first. So two readings of one span that reduce to one symbol give one tree, and
//! the tree that survives comes from the order of the links and from no rule of the grammar.
//!
//! `cargo xtask dedupe population ROOT LIST` parses each file of LIST twice, once with each value
//! of `ts_set_dedupe_prefers_later_link`, and reports each file whose tree hash differs. That file
//! holds a text whose tree DEPENDS on the order, and no rule of this grammar decides it.
//!
//! THE COUNT IS NOT A COUNT OF DEFECTS. IT IS A COUNT OF TREES THAT NO RULE DECIDES. The
//! measurement of 2026-09-16 read the largest family of the population, 2,225 files of
//! `sizeof(NAME[i])`, and the reading of the default is CORRECT in 2,021 of them, because the name
//! is an object and not a type. A commit that moves this count toward zero is as likely to make the
//! trees worse as better. The count exists so that a commit which changes the population SAYS SO.
//!
//! WHY THE INSTRUMENT MEASURES THE CONSEQUENCE AND NOT THE EVENT. A counter in the dedupe itself
//! fires 23,619,145 times over the corpus of 329,387 files, and in 14,730,977 of those the two
//! subtrees hold children that differ. THE CHOICE CHANGES A TREE IN 2,967 FILES. Four orders of
//! magnitude lie between the event and its consequence, so a baseline of the event would hold 45
//! rows for each file and would move at every change of the grammar.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;

use rayon::prelude::*;
use tree_sitter::Language;

use crate::corpus::{Stop, new_parser, parse_with_limit};
use crate::trees::{Names, tree_facts};

const USAGE: &str = "usage: cargo xtask dedupe population [--write-baseline] ROOT LIST [--write PATH]";

/// The baseline of the files whose tree depends on the dedupe, in the repository.
const POPULATION: &str = "test/dedupe/population.txt";

unsafe extern "C" {
    fn ts_set_dedupe_prefers_later_link(prefer_later: bool);
}

/// Make the dedupe of this thread keep the later of two equivalent links, or the earlier one.
///
/// THE FLAG IS A TEST ENTRY POINT OF THE FORK. Only this instrument sets it, and it sets it back
/// before the thread parses anything else.
fn prefer_later(value: bool) {
    // SAFETY: the function writes a thread-local flag of the runtime and reads no memory.
    unsafe { ts_set_dedupe_prefers_later_link(value) }
}

/// Parse one file with each preference, and give true when the two trees differ.
///
/// A file that no parse can read, and a parse that a limit stops, give false: such a file has no
/// tree to compare, and the corpus report already holds it. O(n) in the bytes of the file.
fn depends(language: &Language, names: &Names, root: &Path, rel: &str) -> bool {
    let Ok(source) = fs::read(root.join(rel)) else {
        return false;
    };
    let mut parser = new_parser(language);
    let mut hashes = [0u64; 2];
    for (slot, later) in [false, true].into_iter().enumerate() {
        prefer_later(later);
        let parsed = parse_with_limit(&mut parser, &source, rel);
        prefer_later(false);
        match parsed {
            Ok(tree) => hashes[slot] = tree_facts(&tree, names, source.len()).hash,
            Err(Stop::Budget(_) | Stop::Memory(_)) => return false,
        }
    }
    hashes[0] != hashes[1]
}

/// The header of the baseline file. The text says what the count is and what it is not.
fn baseline_header(files: usize) -> String {
    format!(
        "# The files whose tree DEPENDS on the dedupe of the parse stack, over the full corpus.\n\
         # Refer to xtask/src/dedupe.rs and to `stack_node_add_link` of vendor/tree-sitter/src/stack.c.\n\
         #\n\
         # THE COUNT IS NOT A COUNT OF DEFECTS. IT IS A COUNT OF TREES THAT NO RULE DECIDES. The\n\
         # dedupe drops one of two readings of one span without a comparison of the children, and the\n\
         # reading that survives comes from the order of the links. In the largest family of this\n\
         # population, 2,225 files of `sizeof(NAME[i])`, THE DEFAULT READING IS CORRECT IN 2,021,\n\
         # because the name is an object and not a type. A commit that drives this count toward zero\n\
         # is as likely to make the trees worse as better.\n\
         #\n\
         # WHAT THIS FILE PROMISES: a file that joins the population fails the check. A file that\n\
         # leaves it does not, so a repair never fails its own check.\n\
         # WHAT IT DOES NOT PROMISE: nothing about the SITE. A file that leaves the population at one\n\
         # site and joins it at another keeps its row, and no check here sees that.\n\
         #\n\
         # A COUNT MEASURED AGAINST A STALE BASELINE IS A FINDING AND NOT A BASELINE. integrate.sh\n\
         # writes this file again at each landing whose gate found falls, so a fall of the next gate\n\
         # belongs to the commits under that gate. The first run of this check read 136 falls against\n\
         # a baseline of an earlier master, and 124 of those belonged to one landing that nobody had\n\
         # connected to this class.\n\
         # A FALL IS WORTH ATTRIBUTING. A file leaves this population when a rule DECIDES its tree,\n\
         # and also when a reading is REMOVED. The two look the same here, and only the commit that\n\
         # caused the fall knows which one it made.\n\
         #\n\
         # {files} files, of the corpus of the fork master.\n"
    )
}

/// Read the paths of the baseline. A missing file gives no path.
fn read_baseline(repository: &Path) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let path = repository.join(POPULATION);
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(BTreeSet::new());
    };
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

/// The text of a baseline: the header and one path for each file, in the order of the bytes.
fn baseline_text(found: &BTreeSet<String>) -> String {
    let mut out = baseline_header(found.len());
    for path in found {
        out.push_str(path);
        out.push('\n');
    }
    out
}

/// Measure the population, and compare it with the baseline of the repository.
fn population(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let (write, root, list, measured) = match args {
        [flag, root, list] if flag == "--write-baseline" => (true, root, list, None),
        [root, list] => (false, root, list, None),
        [root, list, flag, path] if flag == "--write" => (false, root, list, Some(path.clone())),
        _ => return Err(USAGE.into()),
    };
    let root = Path::new(root);
    let text = fs::read_to_string(list).map_err(|e| format!("cannot read the file list {list}: {e}"))?;
    let paths: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let language = tree_sitter_cpp::LANGUAGE.into();
    let names = Names::new(&language);
    let found: BTreeSet<String> = paths
        .par_iter()
        .filter(|rel| depends(&language, &names, root, rel))
        .map(|rel| (*rel).to_owned())
        .collect();
    println!("files {}, depend on the dedupe {}", paths.len(), found.len());
    if write {
        let path = repository.join(POPULATION);
        fs::create_dir_all(path.parent().expect("the baseline is in a directory"))?;
        fs::write(&path, baseline_text(&found))?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    if let Some(path) = &measured {
        fs::write(path, baseline_text(&found)).map_err(|e| format!("cannot write {path}: {e}"))?;
        println!("measured rows {path}");
    }
    let baseline = read_baseline(repository)?;
    let read: BTreeSet<&str> = paths.iter().copied().collect();
    let risen: Vec<&String> = found.iter().filter(|path| !baseline.contains(path.as_str())).collect();
    let fallen: Vec<&String> = baseline
        .iter()
        .filter(|path| read.contains(path.as_str()) && !found.contains(path.as_str()))
        .collect();
    for path in &fallen {
        println!("fell    {path}");
    }
    for path in &risen {
        println!("ROSE    {path}");
    }
    println!(
        "baseline {}, found {}, risen {}, fallen {}",
        baseline.len(),
        found.len(),
        risen.len(),
        fallen.len()
    );
    if risen.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} files whose tree DEPENDS on the dedupe of the parse stack that the baseline does not \
         hold. The dedupe drops one of two readings of one span with no comparison of the children, \
         so the tree of each file above comes from the order of the links and from no rule. Read the \
         two trees with `cargo xtask parse FILE` of this build and of the reference, decide whether \
         the reading of the default is the correct one, and say so in the commit message. Then write \
         the baseline again with `cargo xtask dedupe population --write-baseline ROOT LIST`. THE \
         COUNT IS NOT A COUNT OF DEFECTS: the default reading is correct in most of this population, \
         so a commit that ADDS a file here has made one more tree that no rule decides.",
        risen.len()
    )
    .into())
}

/// Run the `dedupe` task.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    match args.first().map(String::as_str) {
        Some("population") => population(repository, &args[1..]),
        _ => Err(USAGE.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reader of the baseline takes the header and the paths, and it gives the paths only.
    #[test]
    fn the_reader_of_the_baseline_takes_the_header() {
        let mut paths = BTreeSet::new();
        paths.insert("a/b.cpp".to_owned());
        paths.insert("c/d.h".to_owned());
        let text = baseline_text(&paths);
        assert!(text.starts_with('#'), "the baseline starts with its header");
        assert!(text.contains("NOT A COUNT OF DEFECTS"), "the header says what the count is not");
        let rows: Vec<&str> = text
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .collect();
        assert_eq!(rows, vec!["a/b.cpp", "c/d.h"], "the rows hold the paths and nothing else");
    }

    /// The flag of the runtime holds the value that the instrument sets, on this thread.
    #[test]
    fn the_flag_of_the_runtime_takes_the_two_values() {
        unsafe extern "C" {
            fn ts_dedupe_prefers_later_link() -> bool;
        }
        // SAFETY: each function reads or writes a thread-local flag of the runtime.
        unsafe {
            assert!(!ts_dedupe_prefers_later_link(), "the default keeps the earlier link");
            prefer_later(true);
            assert!(ts_dedupe_prefers_later_link(), "the instrument can keep the later link");
            prefer_later(false);
            assert!(!ts_dedupe_prefers_later_link(), "the instrument sets the default back");
        }
    }
}
