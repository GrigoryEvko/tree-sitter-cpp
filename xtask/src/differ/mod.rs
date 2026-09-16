//! Compare our syntax trees with GCC and Clang, and find the wrong trees that have no ERROR node.
//!
//! GCC and Clang are the oracles of validity. The Clang JSON AST is the oracle of structure. The tool gives
//! the nodes of the two trees categories from one set, compares the categories at the same byte ranges, and
//! reports each Clang node whose range has a different category or no category in our tree.

mod clang;
mod compare;
mod json;
mod label;
mod options;
mod oracle;
mod ours;
mod report;

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rayon::prelude::*;
use tree_sitter::{Language as Grammar, Parser};

use compare::{Fact, Outcome, Verdict};
use label::{Category, Label};
use options::Language;
use oracle::{Check, ClangRun, Compiler, Oracle, Status};
use ours::OurTree;

const USAGE: &str = "\
usage: cargo xtask differ [OPTIONS] ROOT LIST OUT
       cargo xtask differ [OPTIONS] --syntax OUT
       cargo xtask differ [OPTIONS] --show [--tree] [-- FLAG...] < SNIPPET
       cargo xtask differ --compare A B

  ROOT LIST OUT  Compare each file of LIST, a list of paths relative to ROOT.
  --syntax OUT   Compare each snippet of test/syntax/*.txt.
  --show         Read one snippet from the standard input. Print the facts of Clang and of our tree
                 by byte range. The FLAGs go to the two compilers. --tree also prints our tree.
  OUT            A directory for validity.tsv, disagreements.tsv, and summary.txt.
  --compare A B  Compare the category tables of two OUT directories. Print each category that moved,
                 and each category that fell. A fall is a decrease of the agree percent.

options:
  --top N          The number of classes of disagreements in the summary (20).
  --work DIR       The directory of the cache and of the compiler runs (target/differ).
  --jobs N         The number of files in parallel (128).
  --timeout S      The time limit of a compiler run in seconds (60).
  --max-json-mb N  The maximum size of a Clang JSON AST (256). A larger file has no structure comparison.
  --gcc PATH       The GCC driver (/usr/bin/g++).
  --clang PATH     The Clang driver (/usr/bin/clang++).
  --no-cache       Do not read or write the cache.";

/// The time limit of our parse of one file.
const PARSE_LIMIT: Duration = Duration::from_secs(20);
/// The maximum length of the excerpt of a disagreement.
const EXCERPT_CHARS: usize = 100;
/// The number of files from which a run with no cache gives a warning.
const CACHE_WARNING_FILES: usize = 100;

/// The settings of a run.
struct Settings {
    top: usize,
    work: PathBuf,
    jobs: usize,
    timeout: u64,
    max_json_mb: u64,
    gcc: String,
    clang: String,
    cache: bool,
    syntax: bool,
    show: bool,
    compare: bool,
    tree: bool,
    positional: Vec<String>,
    flags: Vec<String>,
}

impl Settings {
    /// Read the settings from the arguments of the task.
    fn parse(repository: &Path, args: &[String]) -> Result<Settings, String> {
        let mut settings = Settings {
            top: 20,
            work: repository.join("target").join("differ"),
            jobs: 128,
            timeout: 60,
            max_json_mb: 256,
            gcc: "/usr/bin/g++".to_owned(),
            clang: "/usr/bin/clang++".to_owned(),
            cache: true,
            syntax: false,
            show: false,
            compare: false,
            tree: false,
            positional: Vec::new(),
            flags: Vec::new(),
        };
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            let mut value = |name: &str| {
                iter.next()
                    .cloned()
                    .ok_or_else(|| format!("the option {name} needs a value\n\n{USAGE}"))
            };
            let number = |name: &str, text: String| {
                text.parse::<u64>()
                    .map_err(|_| format!("the value of {name} must be a number, not `{text}`"))
            };
            match arg.as_str() {
                "--top" => settings.top = number("--top", value("--top")?)? as usize,
                "--work" => settings.work = PathBuf::from(value("--work")?),
                "--jobs" => settings.jobs = number("--jobs", value("--jobs")?)?.max(1) as usize,
                "--timeout" => settings.timeout = number("--timeout", value("--timeout")?)?,
                "--max-json-mb" => settings.max_json_mb = number("--max-json-mb", value("--max-json-mb")?)?,
                "--gcc" => settings.gcc = value("--gcc")?,
                "--clang" => settings.clang = value("--clang")?,
                "--no-cache" => settings.cache = false,
                "--syntax" => settings.syntax = true,
                "--compare" => settings.compare = true,
                "--show" => settings.show = true,
                "--tree" => settings.tree = true,
                "--" => {
                    settings.flags.extend(iter.by_ref().cloned());
                }
                text if text.starts_with("--") => return Err(format!("the option {text} is not known\n\n{USAGE}")),
                text => settings.positional.push(text.to_owned()),
            }
        }
        let expected = match (settings.compare, settings.show, settings.syntax) {
            (true, _, _) => 2,
            (false, true, _) => 0,
            (false, false, true) => 1,
            (false, false, false) => 3,
        };
        // Each mode takes its own positional arguments, and no two modes go together.
        let modes = u8::from(settings.compare) + u8::from(settings.show) + u8::from(settings.syntax);
        if settings.positional.len() != expected || modes > 1 {
            return Err(USAGE.into());
        }
        settings.work = std::path::absolute(&settings.work).map_err(|e| {
            format!(
                "cannot make the work directory {} absolute: {e}",
                settings.work.display()
            )
        })?;
        Ok(settings)
    }
}

/// A parser of our grammar.
fn new_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&Grammar::new(tree_sitter_cpp::LANGUAGE))
        .expect("the ABI of the grammar must agree with the tree-sitter runtime");
    parser
}

/// Everything that the tool knows about one file.
pub struct Analysis {
    pub language: Language,
    pub source: Vec<u8>,
    pub gcc_flags: Vec<String>,
    pub clang_flags: Vec<String>,
    pub gcc: Check,
    pub clang: ClangRun,
    /// Our flat tree, or `None` when the time limit stopped the parse.
    pub tree: Option<OurTree>,
    pub our_facts: Vec<Fact>,
    pub outcomes: Vec<Outcome>,
    /// Our facts that no Clang fact stands at. Refer to [`compare::unmatched`].
    pub unmatched: Vec<Fact>,
    /// True when Clang accepts the file and its JSON AST is read.
    pub compared: bool,
}

/// Run the two compilers and our parser on a file, and compare the structures.
///
/// `extra` are more flags for the two compilers. The compilers run in parallel with our parse.
fn analyze(parser: &mut Parser, oracle: &Oracle, path: &Path, source: Vec<u8>, extra: &[String]) -> Analysis {
    let text = String::from_utf8_lossy(&source);
    let mut given = options::read(path, &text);
    given.gcc.extend(extra.iter().cloned());
    given.clang.extend(extra.iter().cloned());
    let cwd = oracle.cwd();
    let gcc_flags = options::filter(
        &oracle.gcc.binary,
        &given.gcc,
        given.language,
        options::GCC_DEFAULT,
        &cwd,
    );
    let clang_flags = options::filter(
        &oracle.clang.binary,
        &given.clang,
        given.language,
        options::CLANG_DEFAULT,
        &cwd,
    );
    let ((gcc, clang), tree) = rayon::join(
        || {
            rayon::join(
                || oracle.gcc(path, &gcc_flags, &source),
                || oracle.clang(path, &clang_flags, &source),
            )
        },
        || ours::parse(parser, &source, &path.to_string_lossy(), PARSE_LIMIT).map(|tree| OurTree::new(&tree)),
    );
    let our_facts = tree.as_ref().map_or_else(Vec::new, |tree| ours::facts(tree, &source));
    let compared = clang.check.status == Status::Accept && clang.json == oracle::JsonStatus::Read && tree.is_some();
    // The Clang facts serve both directions, so the walk of the JSON nodes runs one time.
    let (outcomes, unmatched) = if compared {
        let theirs = clang::facts(&clang.nodes, &source);
        (
            compare::compare(&theirs, &our_facts),
            compare::unmatched(&theirs, &our_facts),
        )
    } else {
        (Vec::new(), Vec::new())
    };
    Analysis {
        language: given.language,
        source,
        gcc_flags,
        clang_flags,
        gcc,
        clang,
        tree,
        our_facts,
        outcomes,
        unmatched,
        compared,
    }
}

/// A disagreement of one file.
pub struct Row {
    pub begin: u32,
    pub end: u32,
    pub line: usize,
    pub verdict: Verdict,
    pub clang_kind: Box<str>,
    pub clang: Label,
    pub our_kind: &'static str,
    pub ours: Option<Label>,
    /// The kind of the smallest node of our tree that contains the range.
    pub cover: &'static str,
    pub excerpt: String,
}

/// The result of one file for the report.
pub struct FileResult {
    pub label: String,
    pub readable: bool,
    pub language: Language,
    pub bytes: usize,
    pub gcc_flags: Vec<String>,
    pub clang_flags: Vec<String>,
    pub gcc: Check,
    pub clang: Check,
    pub json: oracle::JsonStatus,
    pub json_bytes: u64,
    /// The number of ERROR and MISSING nodes.
    pub our_errors: u32,
    /// The position and the kind of the first error of our tree.
    pub our_first: String,
    /// True when the time limit stopped our parse.
    pub stopped: bool,
    pub compared: bool,
    /// The number of Clang facts of each category with each verdict.
    pub counts: BTreeMap<Category, [u32; 5]>,
    /// The number of OUR facts of each category that no Clang fact stands at, and the number of
    /// those that stand inside a macro definition. Refer to [`compare::unmatched`].
    pub added: BTreeMap<Category, [u32; 2]>,
    pub rows: Vec<Row>,
}

impl FileResult {
    /// True when our tree has an ERROR or MISSING node, or has no tree.
    pub fn our_error(&self) -> bool {
        self.stopped || self.our_errors > 0
    }
}

/// The start of each line of a source. O(n) in the size of the source.
fn line_starts(source: &[u8]) -> Vec<u32> {
    std::iter::once(0)
        .chain(
            source
                .iter()
                .enumerate()
                .filter(|(_, b)| **b == b'\n')
                .map(|(i, _)| i as u32 + 1),
        )
        .collect()
}

/// The line and the column of a byte, from 1.
fn position(starts: &[u32], byte: u32) -> (usize, usize) {
    let line = starts.partition_point(|&start| start <= byte).max(1);
    (line, (byte - starts[line - 1]) as usize + 1)
}

/// The text of a byte range with each run of white space as one space, cut to [`EXCERPT_CHARS`].
fn excerpt(source: &[u8], begin: u32, end: u32) -> String {
    let end = (end as usize).min(source.len()).min(begin as usize + 4 * EXCERPT_CHARS);
    let text = String::from_utf8_lossy(source.get(begin as usize..end).unwrap_or_default());
    let mut out: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > EXCERPT_CHARS {
        out = out.chars().take(EXCERPT_CHARS).collect::<String>() + "...";
    }
    out
}

/// Make the result of one file from its analysis.
fn summarize(label: String, analysis: Analysis) -> FileResult {
    let source = &analysis.source;
    let starts = line_starts(source);
    let mut counts: BTreeMap<Category, [u32; 5]> = BTreeMap::new();
    let mut added: BTreeMap<Category, [u32; 2]> = BTreeMap::new();
    let mut rows = Vec::new();
    let (our_errors, our_first) = match &analysis.tree {
        Some(tree) if tree.errors > 0 => {
            let node = tree.node(tree.first_error);
            let (line, column) = position(&starts, node.start);
            let kind = if node.missing {
                format!("MISSING {}", node.kind)
            } else {
                "ERROR".to_owned()
            };
            (tree.errors, format!("{line}:{column} {kind}"))
        }
        _ => (0, String::new()),
    };
    if let Some(tree) = &analysis.tree {
        for outcome in &analysis.outcomes {
            counts.entry(outcome.clang.label.category).or_default()[outcome.verdict as usize] += 1;
            if outcome.verdict == Verdict::Agree {
                continue;
            }
            let fact = outcome.clang;
            let cover = tree.cover(fact.begin, fact.end);
            let cover_kind = if cover == ours::NONE { "" } else { tree.kind(cover) };
            rows.push(Row {
                begin: fact.begin,
                end: fact.end,
                line: position(&starts, fact.begin).0,
                verdict: outcome.verdict,
                clang_kind: analysis.clang.nodes[fact.node as usize].kind.clone(),
                clang: fact.label,
                our_kind: outcome.ours.map_or(cover_kind, |o| tree.kind(o.node)),
                ours: outcome.ours.map(|o| o.label),
                cover: cover_kind,
                excerpt: excerpt(source, fact.begin, fact.end),
            });
        }
        // A FACT INSIDE A MACRO DEFINITION IS NOT A DEFECT, AND IT IS THE LARGEST GROUP HERE.
        // Clang expands a macro before it builds an AST, so the body of a `#define` reaches no
        // Clang fact whatever it holds. The count separates that group at the point of measurement,
        // because a number and its caution must travel in one report.
        for fact in &analysis.unmatched {
            let mut current = Some(fact.node);
            let mut in_macro = false;
            while let Some(index) = current {
                if tree.kind(index).starts_with("preproc_") {
                    in_macro = true;
                    break;
                }
                current = tree.parent(index);
            }
            let entry = added.entry(fact.label.category).or_default();
            entry[0] += 1;
            entry[1] += u32::from(in_macro);
        }
    }
    FileResult {
        label,
        readable: true,
        language: analysis.language,
        bytes: source.len(),
        gcc_flags: analysis.gcc_flags,
        clang_flags: analysis.clang_flags,
        gcc: analysis.gcc,
        clang: analysis.clang.check,
        json: analysis.clang.json,
        json_bytes: analysis.clang.bytes,
        our_errors,
        our_first,
        stopped: analysis.tree.is_none(),
        compared: analysis.compared,
        counts,
        added,
        rows,
    }
}

/// The result of a file that cannot be read.
fn unreadable(label: String, error: &str) -> FileResult {
    let check = Check {
        status: Status::Unavailable,
        error: error.to_owned(),
    };
    FileResult {
        label,
        readable: false,
        language: Language::Cxx,
        bytes: 0,
        gcc_flags: Vec::new(),
        clang_flags: Vec::new(),
        gcc: check.clone(),
        clang: check,
        json: oracle::JsonStatus::Absent,
        json_bytes: 0,
        our_errors: 0,
        our_first: String::new(),
        stopped: false,
        compared: false,
        counts: BTreeMap::new(),
        added: BTreeMap::new(),
        rows: Vec::new(),
    }
}

/// The inputs of a list: the absolute path and the label of each file.
fn list_inputs(root: &str, list: &str) -> Result<Vec<(PathBuf, String)>, Box<dyn Error>> {
    let root = std::path::absolute(root).map_err(|e| format!("cannot make the root {root} absolute: {e}"))?;
    let text = fs::read_to_string(list).map_err(|e| format!("cannot read the file list {list}: {e}"))?;
    Ok(text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| (root.join(line.trim()), line.trim().to_owned()))
        .collect())
}

/// Write each snippet of `test/syntax` to a file in the work directory, and give the inputs.
fn syntax_inputs(repository: &Path, work: &Path) -> Result<Vec<(PathBuf, String)>, Box<dyn Error>> {
    let directory = repository.join("test").join("syntax");
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .map_err(|e| format!("cannot read {}: {e}", directory.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    files.retain(|path| path.extension().is_some_and(|extension| extension == "txt"));
    files.sort();
    let mut inputs = Vec::new();
    for file in files {
        let content = fs::read_to_string(&file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        let stem = file
            .file_stem()
            .map_or(String::new(), |s| s.to_string_lossy().into_owned());
        let target = work.join("syntax").join(&stem);
        fs::create_dir_all(&target).map_err(|e| format!("cannot create {}: {e}", target.display()))?;
        for (index, snippet) in crate::syntax::snippets(&content).into_iter().enumerate() {
            let path = target.join(format!("{index:04}.cc"));
            if fs::read(&path).ok().as_deref() != Some(snippet.source.as_bytes()) {
                fs::write(&path, &snippet.source).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            }
            inputs.push((path, format!("{stem}.txt: {}", snippet.name)));
        }
    }
    Ok(inputs)
}

/// Compare the files or the snippets, write the report, and print the summary.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let settings = Settings::parse(repository, args)?;
    // The comparison of two runs reads two files. It starts no compiler and it makes no work directory.
    if settings.compare {
        let (a, b) = (&settings.positional[0], &settings.positional[1]);
        return report::compare(Path::new(a), Path::new(b));
    }
    fs::create_dir_all(&settings.work).map_err(|e| format!("cannot create {}: {e}", settings.work.display()))?;
    let oracle = Oracle {
        gcc: Compiler::new(&settings.gcc)?,
        clang: Compiler::new(&settings.clang)?,
        work: settings.work.clone(),
        timeout: Duration::from_secs(settings.timeout),
        limit: settings.max_json_mb.saturating_mul(1 << 20),
        cache: settings.cache,
    };
    if settings.show {
        return show(&oracle, &settings);
    }
    let (inputs, out) = if settings.syntax {
        (syntax_inputs(repository, &settings.work)?, &settings.positional[0])
    } else {
        (
            list_inputs(&settings.positional[0], &settings.positional[1])?,
            &settings.positional[2],
        )
    };
    // The cache holds the result of each compiler run. A run with no cache starts two compilers for
    // each file, and it takes minutes in the place of seconds. The gate links the shared cache into
    // the work directory, and a run that forgets the link has no other warning.
    let cache = settings.work.join("cache");
    if settings.cache && inputs.len() >= CACHE_WARNING_FILES && !cache.exists() {
        eprintln!(
            "differ: {} does not exist. This run starts the compilers for {} files in the place of a cache read.",
            cache.display(),
            inputs.len()
        );
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(settings.jobs)
        .build()
        .map_err(|e| format!("cannot start {} threads: {e}", settings.jobs))?;
    let started = Instant::now();
    let done = AtomicUsize::new(0);
    let results: Vec<FileResult> = pool.install(|| {
        inputs
            .par_iter()
            .map_init(new_parser, |parser, (path, label)| {
                let result = match fs::read(path) {
                    Ok(source) => summarize(label.clone(), analyze(parser, &oracle, path, source, &[])),
                    Err(error) => unreadable(label.clone(), &format!("cannot read {}: {error}", path.display())),
                };
                let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                if count.is_multiple_of(1000) {
                    eprintln!("{count} files in {:.0} s", started.elapsed().as_secs_f64());
                }
                result
            })
            .collect()
    });
    let summary = report::write(Path::new(out), &oracle, &results, settings.top, started.elapsed())?;
    print!("{summary}");
    Ok(())
}

/// Read a snippet from the standard input, and print the facts of the two sides by byte range.
fn show(oracle: &Oracle, settings: &Settings) -> Result<(), Box<dyn Error>> {
    let mut source = Vec::new();
    std::io::stdin().read_to_end(&mut source)?;
    let directory = oracle.work.join("show");
    fs::create_dir_all(&directory).map_err(|e| format!("cannot create {}: {e}", directory.display()))?;
    let path = directory.join(format!("snippet-{}.cc", std::process::id()));
    fs::write(&path, &source).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let mut parser = new_parser();
    let analysis = analyze(&mut parser, oracle, &path, source, &settings.flags);
    let _ = fs::remove_file(&path);
    let tree = if settings.tree {
        ours::parse(&mut parser, &analysis.source, &path.to_string_lossy(), PARSE_LIMIT)
            .map(|tree| crate::sexp::format(&tree.root_node().to_sexp()))
    } else {
        None
    };
    let disagreements = report::show(&analysis, tree.as_deref());
    if disagreements > 0 {
        return Err(format!("{disagreements} mismatches or missing nodes").into());
    }
    Ok(())
}
