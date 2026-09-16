//! The `seed` task: `cargo xtask seed collect`, which writes the seed of a project.
//!
//! A seed holds the names that a project declares as a type or as a template. The scanner records
//! the names that the FILE declares, and that record cannot hold a name from a header, because no
//! header is parsed. The reader of the file this task writes is `crate::seed`, and the format is
//! `src/seed.h`.
//!
//! THE TASK READS BACK WHAT IT WROTE WITH THAT READER BEFORE IT REPORTS SUCCESS. The reader refuses
//! a file that a binary search cannot read: a row that is not `name<TAB>kind`, a kind that is not
//! one of the three words, a name of more than 64 bytes, and an order that is not ascending by the
//! bytes of the name. A collector that could write such a file would be found by its user and not
//! by itself, so the last step of the task is the reader.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use tree_sitter::{Language, Node, Parser};

use crate::seed::Seed;

/// The bytes of the longest name that the format takes. `crate::seed` declares the same number and
/// binds it to `TS_CPP_SEED_WORD_SIZE` of src/seed.h with a test.
const MAX_NAME: usize = 64;

/// The longest dropped names that the report prints for each project.
const REPORT_LONGEST: usize = 3;

/// What a declaration says about a name.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    /// A class, struct, union or enum specifier with a body, a typedef, or an alias declaration.
    Type,
    /// Any of those under a template head.
    Template,
    /// A forward declaration or an elaborated use, `struct S;`.
    TypeUse,
    /// A name that a type name cannot also be: a function, an object, a parameter, or a
    /// function-like macro.
    Conflict,
    /// A `#define` of the project, of either shape. The kind decides nothing by itself. It tells
    /// `resolve` that a name of `struct MACRO NAME;` is the macro and not the declared type.
    Defined,
}

/// The names of one project, with what its files say about each one.
#[derive(Default)]
struct Names {
    kinds: BTreeMap<String, BTreeSet<Kind>>,
    /// The `struct MACRO NAME;` sites, as the macro and the declared name. `resolve` reads them.
    attributed: Vec<(String, String)>,
}

impl Names {
    fn add(&mut self, name: &str, kind: Kind) {
        if !name.is_empty() {
            self.kinds.entry(name.to_owned()).or_default().insert(kind);
        }
    }

    fn attributed(&mut self, macro_name: &str, name: &str) {
        if !macro_name.is_empty() && !name.is_empty() {
            self.attributed.push((macro_name.to_owned(), name.to_owned()));
        }
    }

    fn merge(&mut self, other: Self) {
        for (name, kinds) in other.kinds {
            self.kinds.entry(name).or_default().extend(kinds);
        }
        self.attributed.extend(other.attributed);
    }

    /// Decide each `struct MACRO NAME;` site with what the whole project says.
    ///
    /// THE THREE READINGS OF THAT ONE SHAPE ARE THE SAME TEXT, and the grammar cannot tell them
    /// apart, so it reads every one as a forward declaration of `NAME` with an attribute macro:
    ///     class META_TEMPLATE_VIS basic_string;        `basic_string` IS a type
    ///     struct CALL_CENTER_TBL g_w_call_center;      `g_w_call_center` is an OBJECT
    ///     class ABSL_MUST_USE_RESULT ABSL_ATTRIBUTE_TRIVIAL_ABI Ptr;   the name is a MACRO
    /// A file cannot tell them apart either. A PROJECT CAN, which is the reason a seed is collected
    /// over a project and not over a file.
    ///
    /// The name is not a type when the project defines it as a macro, which is the third reading.
    /// The name is not a type when the project declares the macro itself as a type, because the
    /// construct is then an ordinary object of that type, which is the second reading. Otherwise
    /// the name is a forward declaration.
    ///
    /// The shape gives 88 names over the 64 corpus projects, so the whole question is 0.01% of the
    /// names. It still decides whether a seed holds `g_w_call_center` as a type, and a seed that
    /// holds an object name makes every call of that name a functional cast, with no ERROR node.
    fn resolve(&mut self) {
        let mut accepted = Vec::new();
        for (macro_name, name) in &self.attributed {
            let declared_as_macro = self.kinds.get(name).is_some_and(|kinds| kinds.contains(&Kind::Defined));
            let macro_is_a_type = self.kinds.get(macro_name).is_some_and(|kinds| {
                kinds.contains(&Kind::Type) || kinds.contains(&Kind::Template) || kinds.contains(&Kind::TypeUse)
            });
            if !declared_as_macro && !macro_is_a_type {
                accepted.push(name.clone());
            }
        }
        for name in accepted {
            self.kinds.entry(name).or_default().insert(Kind::TypeUse);
        }
    }

    /// The rows of the seed file, and the names that the length dropped.
    ///
    /// A NAME THAT A FILE ALSO DECLARES AS A FUNCTION, AN OBJECT, A PARAMETER OR A FUNCTION-LIKE
    /// MACRO GIVES NO ROW. The scanner reads a name in one position and has no scope, so a project
    /// that uses one name for a type and for a function would make every call of that function a
    /// cast. #274 measured this rule: without it the standard library loses `vector`, `pair` and
    /// `function`, because a declaration with no type is a constructor and not a function.
    fn rows(&self) -> (Vec<(&str, &'static str)>, Vec<&str>) {
        let mut rows = Vec::new();
        let mut dropped = Vec::new();
        for (name, kinds) in &self.kinds {
            if kinds.contains(&Kind::Conflict) {
                continue;
            }
            let kind = if kinds.contains(&Kind::Template) {
                "template"
            } else if kinds.contains(&Kind::Type) || kinds.contains(&Kind::TypeUse) {
                "type"
            } else {
                continue;
            };
            if name.len() > MAX_NAME {
                dropped.push(name.as_str());
                continue;
            }
            rows.push((name.as_str(), kind));
        }
        // `BTreeMap` gives the names in the order of their bytes, which is the order the reader
        // takes, so the rows need no second sort.
        (rows, dropped)
    }
}

/// The text of a node.
fn text<'a>(source: &'a [u8], node: Node) -> &'a str {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("")
}

/// The leaf name of a declarator or of a name node, past the qualifiers, the template arguments and
/// the declarator wrappers. None for an operator, a destructor, or a structured binding.
fn leaf_name<'a>(source: &'a [u8], node: Node<'a>) -> Option<&'a str> {
    let mut current = node;
    loop {
        match current.kind() {
            "identifier" | "field_identifier" | "type_identifier" => return Some(text(source, current)),
            "qualified_identifier" | "template_type" | "template_function" | "dependent_name" => {
                current = current.child_by_field_name("name")?;
            }
            "destructor_name" | "operator_name" | "structured_binding_declarator" => return None,
            "parenthesized_declarator" | "abstract_parenthesized_declarator" => current = current.named_child(0)?,
            _ => current = current.child_by_field_name("declarator")?,
        }
    }
}

/// True for a word with an uppercase letter and no lowercase letter, as `is_macro_name` of the
/// scanner reads it.
fn is_macro_shaped(word: &str) -> bool {
    word.chars().any(|c| c.is_ascii_uppercase()) && !word.chars().any(|c| c.is_ascii_lowercase())
}

/// True when a template declaration is within three parents of the node.
fn under_template(node: Node) -> bool {
    let mut current = node.parent();
    for _ in 0..3 {
        let Some(parent) = current else { return false };
        if parent.kind() == "template_declaration" {
            return true;
        }
        current = parent.parent();
    }
    false
}

/// Read the names that one file declares.
fn names_of(source: &[u8]) -> Names {
    let mut names = Names::default();
    let mut parser = Parser::new();
    if parser.set_language(&Language::new(tree_sitter_cpp::LANGUAGE)).is_err() {
        return names;
    }
    let Some(tree) = parser.parse(source, None) else { return names };
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "class_specifier" | "struct_specifier" | "union_specifier" | "enum_specifier" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let has_body = node.child_by_field_name("body").is_some()
                        || node.children(&mut cursor).any(|child| child.kind() == "base_class_clause");
                    let attribute = node
                        .children(&mut cursor)
                        .find(|child| child.kind() == "attribute_macro")
                        .and_then(|child| child.child_by_field_name("name"))
                        .map(|child| text(source, child));
                    let kind = if !has_body {
                        Kind::TypeUse
                    } else if under_template(node) {
                        Kind::Template
                    } else {
                        Kind::Type
                    };
                    if let Some(leaf) = leaf_name(source, name) {
                        match attribute {
                            // `struct MACRO NAME;` reads three ways and only the project tells them
                            // apart. `resolve` decides it.
                            Some(macro_name) if !has_body => names.attributed(macro_name, leaf),
                            _ => names.add(leaf, kind),
                        }
                    }
                }
            }
            "type_definition" => {
                let kind = if under_template(node) { Kind::Template } else { Kind::Type };
                let mut inner = node.walk();
                for child in node.children_by_field_name("declarator", &mut inner) {
                    if let Some(leaf) = leaf_name(source, child) {
                        names.add(leaf, kind);
                    }
                }
            }
            "alias_declaration" => {
                let kind = if under_template(node) { Kind::Template } else { Kind::Type };
                if let Some(leaf) = node.child_by_field_name("name").and_then(|name| leaf_name(source, name)) {
                    names.add(leaf, kind);
                }
            }
            // A DECLARATION WITH NO TYPE IS A CONSTRUCTOR, a destructor, a conversion function or a
            // deduction guide. Its name is the name of a class and it is no conflict. Without this
            // rule every class with a constructor falls out of its own seed.
            "function_definition" => {
                if node.child_by_field_name("type").is_some()
                    && let Some(declarator) = node.child_by_field_name("declarator")
                    && let Some(leaf) = leaf_name(source, declarator)
                {
                    names.add(leaf, Kind::Conflict);
                }
            }
            "declaration" | "field_declaration" => {
                // THE SAME THREE-WAY SHAPE, IN THE FORM THE GRAMMAR STILL READS AS A DECLARATION.
                // `struct MACRO A;` reads here as an object `A` of the type `struct MACRO`, and 6
                // of the 64 corpus projects still reach this arm. It goes to `resolve` with the
                // other form, so ONE rule decides the shape and not two.
                if let Some(specifier) = node.child_by_field_name("type")
                    && matches!(specifier.kind(), "struct_specifier" | "class_specifier" | "union_specifier")
                    && specifier.child_by_field_name("body").is_none()
                    && let Some(specifier_name) = specifier.child_by_field_name("name")
                    && specifier_name.kind() == "type_identifier"
                    && is_macro_shaped(text(source, specifier_name))
                    && node.named_child_count() == 2
                    && let Some(declarator) = node.child_by_field_name("declarator")
                    && declarator.kind() == "identifier"
                {
                    names.attributed(text(source, specifier_name), text(source, declarator));
                } else if node.child_by_field_name("type").is_some() {
                    // A function and an object are one case here. A type name that a project also
                    // gives to either of them cannot be read by position alone, and the seed drops
                    // it. #276 tells the two apart for its own report. This task does not need to.
                    let mut inner = node.walk();
                    for child in node.children_by_field_name("declarator", &mut inner) {
                        if let Some(leaf) = leaf_name(source, child) {
                            names.add(leaf, Kind::Conflict);
                        }
                    }
                }
            }
            "parameter_declaration" | "optional_parameter_declaration" | "variadic_parameter_declaration" => {
                if let Some(declarator) = node.child_by_field_name("declarator")
                    && let Some(leaf) = leaf_name(source, declarator)
                {
                    names.add(leaf, Kind::Conflict);
                }
            }
            // A function-like macro takes the name in the same position as a functional cast, so a
            // seed that also names it as a type would decide between the two by the seed and not by
            // the text. An object-like macro takes no `(` and is no conflict.
            "preproc_function_def" => {
                if let Some(name) = node.child_by_field_name("name") {
                    names.add(text(source, name), Kind::Conflict);
                    names.add(text(source, name), Kind::Defined);
                }
            }
            // An object-like macro takes no `(` and is no conflict with a functional cast. It is
            // still a `#define`, which is what `resolve` reads.
            "preproc_def" => {
                if let Some(name) = node.child_by_field_name("name") {
                    names.add(text(source, name), Kind::Defined);
                }
            }
            _ => {}
        }
        for child in node.children(&mut cursor) {
            if child.is_named() {
                stack.push(child);
            }
        }
    }
    names
}

/// The project of a path: its first component.
fn project_of(path: &str) -> &str {
    path.split('/').next().unwrap_or("")
}

/// The text of a seed file. One writer, so `collect` and `check` cannot disagree by a byte.
fn seed_text(rows: &[(&str, &'static str)]) -> String {
    let mut text = String::with_capacity(rows.len() * 24 + 256);
    text.push_str("# The names that this project declares as a type or as a template.\n");
    text.push_str("# `cargo xtask seed collect` writes this file. The reader is xtask/src/seed.rs.\n");
    text.push_str("# The rows ascend by the bytes of the name, and a reader binary searches them.\n");
    text.push_str("# A COMMITTED SEED IS THE MEMORY OF WHAT A PROJECT DECLARES. `cargo xtask seed check`\n");
    text.push_str("# collects again and fails on any name added, any name removed, and any kind changed.\n");
    text.push_str("# A change to this file changes every parse that loads it, so the commit that makes\n");
    text.push_str("# the change must say which names moved and why.\n");
    for (name, kind) in rows {
        text.push_str(name);
        text.push('\t');
        text.push_str(kind);
        text.push('\n');
    }
    text
}

/// Write one seed file, read it back with `crate::seed`, and give the report line.
fn write_seed(path: &Path, project: &str, names: &Names) -> Result<String, Box<dyn Error>> {
    let (rows, dropped) = names.rows();
    let text = seed_text(&rows);
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    fs::write(path, &text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;

    // THE READER IS THE LAST STEP. A file that it refuses is a defect of this task, and the task
    // must find it rather than the parse that loads the seed a week later.
    let seed = Seed::read(path).map_err(|e| {
        format!("`seed collect` wrote {} and its own reader refuses it. This is a defect of the collector: {e}", path.display())
    })?;
    if seed.names() != rows.len() {
        return Err(format!(
            "`seed collect` wrote {} rows to {} and the reader gives {} names",
            rows.len(),
            path.display(),
            seed.names()
        )
        .into());
    }

    let mut longest: Vec<&str> = dropped.clone();
    longest.sort_by_key(|name| std::cmp::Reverse(name.len()));
    longest.truncate(REPORT_LONGEST);
    let tail = if dropped.is_empty() {
        String::new()
    } else {
        format!(
            "  dropped {} over {MAX_NAME} bytes, longest {}",
            dropped.len(),
            longest.iter().map(|name| format!("{} ({})", name, name.len())).collect::<Vec<_>>().join(", ")
        )
    };
    Ok(format!("{project}\t{} names\t{}\t{}{tail}", seed.names(), seed.id(), path.display()))
}

/// Compare a fresh collection with the seeds that the tree holds.
///
/// THE CHECK FAILS ON A DIFFERENCE IN EITHER DIRECTION: a name that the collection adds, a name that
/// it no longer finds, and a name whose kind changed. A check that fails only on an addition
/// pre-approves every loss, and #283 found two stored baselines of this fork in exactly that shape,
/// which pre-approved 8 tie sites and 58 files for every later commit. A SEED THAT SILENTLY LOSES
/// NAMES GIVES A PARSE THAT LOOKS CORRECT AND IS NOT, which is the failure the seed exists to
/// prevent, arriving from inside.
fn check(root: &Path, list: &Path, directory: &Path) -> Result<(), Box<dyn Error>> {
    let text = fs::read_to_string(list).map_err(|e| format!("cannot read the list {}: {e}", list.display()))?;
    let paths: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let mut committed: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|e| format!("cannot read {}: {e}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "seed"))
        .collect();
    committed.sort();
    if committed.is_empty() {
        return Err(format!("{} holds no seed to check", directory.display()).into());
    }
    let mut failures = Vec::new();
    for path in &committed {
        let project = path.file_stem().and_then(|stem| stem.to_str()).ok_or("a seed file with no name")?;
        let of_project: Vec<&str> =
            paths.iter().copied().filter(|candidate| project_of(candidate) == project).collect();
        if of_project.is_empty() {
            return Err(format!(
                "{} holds the seed of `{project}` and {} names no file of that project, so the check                  would compare a fresh collection of nothing with a committed file",
                directory.display(),
                list.display()
            )
            .into());
        }
        let mut names = collect_names(root, &of_project);
        names.resolve();
        let (rows, _) = names.rows();
        let fresh = seed_text(&rows);
        let stored = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if fresh == stored {
            println!("{project}\t{} names\tagrees with {}", rows.len(), path.display());
            continue;
        }
        failures.push(difference(project, path, &stored, &fresh));
    }
    if failures.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{}\nA seed of test/seed is the memory of what a project declares. A change to it is a change \
         to every parse that loads it, so it must be explained in the commit that makes it. Write the \
         new file with `cargo xtask seed collect` and say in the commit message which names moved and \
         why.",
        failures.join("\n")
    )
    .into())
}

/// The rows of a seed file as a map of the name to the kind.
fn rows_of(text: &str) -> BTreeMap<&str, &str> {
    text.lines().filter(|line| !line.starts_with('#')).filter_map(|line| line.split_once('\t')).collect()
}

/// A message that names every name that moved between two seed files.
fn difference(project: &str, path: &Path, stored: &str, fresh: &str) -> String {
    let before = rows_of(stored);
    let after = rows_of(fresh);
    let added: Vec<&str> = after.keys().filter(|name| !before.contains_key(*name)).copied().collect();
    let removed: Vec<&str> = before.keys().filter(|name| !after.contains_key(*name)).copied().collect();
    let changed: Vec<String> = before
        .iter()
        .filter_map(|(name, kind)| {
            after.get(name).filter(|fresh_kind| *fresh_kind != kind).map(|fresh_kind| format!("{name} {kind} -> {fresh_kind}"))
        })
        .collect();
    let mut parts = vec![format!(
        "{}: the fresh collection of `{project}` differs from the committed seed, {} names against {}",
        path.display(),
        after.len(),
        before.len()
    )];
    let mut say = |label: &str, names: &[String]| {
        if !names.is_empty() {
            let shown: Vec<&str> = names.iter().take(12).map(String::as_str).collect();
            parts.push(format!("  {label} {}: {}", names.len(), shown.join(", ")));
        }
    };
    say("added", &added.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>());
    say("REMOVED", &removed.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>());
    say("changed kind", &changed);
    parts.join("\n")
}

/// Collect the names of the files of `paths`, in parallel.
fn collect_names(root: &Path, paths: &[&str]) -> Names {
    paths
        .par_iter()
        .fold(Names::default, |mut names: Names, path| {
            if let Ok(source) = fs::read(root.join(path)) {
                names.merge(names_of(&source));
            }
            names
        })
        .reduce(Names::default, |mut left, right| {
            left.merge(right);
            left
        })
}

/// Collect the names of a file list and write one seed, or one seed for each project.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    const USAGE: &str = "usage: cargo xtask seed collect ROOT LIST (--out FILE | --out-dir DIRECTORY)\n       cargo xtask seed check ROOT LIST DIRECTORY";
    if let [command, root, list, directory] = args
        && command == "check"
    {
        return check(Path::new(root), Path::new(list), Path::new(directory));
    }
    let (root, list, out, per_project) = match args {
        [collect, root, list, flag, out] if collect == "collect" && flag == "--out" => (root, list, out, false),
        [collect, root, list, flag, out] if collect == "collect" && flag == "--out-dir" => (root, list, out, true),
        _ => return Err(USAGE.into()),
    };
    let root = PathBuf::from(root);
    let text = fs::read_to_string(list).map_err(|e| format!("cannot read the list {list}: {e}"))?;
    let paths: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    if paths.is_empty() {
        return Err(format!("the list {list} holds no path").into());
    }

    let collected: HashMap<String, Names> = paths
        .par_iter()
        .fold(HashMap::new, |mut map: HashMap<String, Names>, path| {
            let Ok(source) = fs::read(root.join(path)) else { return map };
            let project = if per_project { project_of(path).to_owned() } else { String::new() };
            map.entry(project).or_default().merge(names_of(&source));
            map
        })
        .reduce(HashMap::new, |mut left, right| {
            for (project, names) in right {
                left.entry(project).or_default().merge(names);
            }
            left
        });

    let mut collected = collected;
    for names in collected.values_mut() {
        names.resolve();
    }
    let mut reports = Vec::new();
    if per_project {
        let directory = PathBuf::from(out);
        let mut projects: Vec<&String> = collected.keys().collect();
        projects.sort();
        for project in projects {
            if project.is_empty() {
                continue;
            }
            let path = directory.join(format!("{project}.seed"));
            reports.push(write_seed(&path, project, &collected[project])?);
        }
    } else {
        let names = collected.values().next().ok_or("the list gave no file that reads")?;
        reports.push(write_seed(Path::new(out), "all", names)?);
    }
    for report in reports {
        println!("{report}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::seed::Seed;
    use std::fs;
    use std::path::PathBuf;

    /// The longest name that the format takes. A real name of chromium.
    const TAKEN: &str = "BackForwardCacheBrowserTestWithNotRestoredReasonsMaskCrossOrigin";
    /// One byte longer, and a real name of godot. The collector drops it.
    const DROPPED: &str = "D3D12_FEATURE_DATA_VIDEO_ENCODER_RESOLUTION_SUPPORT_DIRTY_REGIONS";

    /// A tree of sources and a file list, in a directory of its own.
    struct Tree(PathBuf);

    static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    impl Tree {
        fn new(source: &str) -> Self {
            let count = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!("xtask-collect-{}-{count}", std::process::id()));
            fs::create_dir_all(root.join("project")).expect("the directory of the test");
            fs::write(root.join("project").join("a.cpp"), source).expect("the source of the test");
            fs::write(root.join("list.txt"), "project/a.cpp\n").expect("the list of the test");
            Self(root)
        }

        fn collect(&self) -> String {
            let out = self.0.join("out.seed");
            let args: Vec<String> = ["collect", self.0.to_str().expect("a path of UTF-8")]
                .into_iter()
                .chain([self.0.join("list.txt").to_str().expect("a path of UTF-8"), "--out"])
                .chain([out.to_str().expect("a path of UTF-8")])
                .map(str::to_owned)
                .collect();
            run(&args).expect("the collector writes a seed");
            fs::read_to_string(&out).expect("the seed reads")
        }

        fn seed_path(&self) -> PathBuf {
            self.0.join("out.seed")
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// THE COLLECTOR CANNOT WRITE A FILE THAT ITS READER REFUSES.
    ///
    /// The two halves of a seed are a writer and a reader of one format, and the three ways my own
    /// probe wrote a file that `Seed::read` refused are each pinned here: an order that is not
    /// ascending by the bytes, a kind that is not one of the three words, and a name that is longer
    /// than the buffer of the scanner. The task itself reads back what it wrote, so a defect stops
    /// the command. This test pins that the guard works and that the rules give the rows it claims.
    ///
    /// THE TWO LONG NAMES ARE REAL CORPUS NAMES and not invented strings, so the test measures the
    /// case that the corpus actually holds at the boundary.
    #[test]
    fn the_collector_cannot_write_a_file_that_its_reader_refuses() {
        assert_eq!(TAKEN.len(), 64);
        assert_eq!(DROPPED.len(), 65);
        let source = format!(
            "struct {TAKEN} {{}};\n\
             struct {DROPPED} {{}};\n\
             typedef int Alias;\n\
             using Other = int;\n\
             template <class T> struct Tpl {{}};\n\
             struct Both {{}};\n\
             int Both(int);\n\
             struct Fwd;\n\
             struct Ctor {{ Ctor(); }};\n"
        );
        let tree = Tree::new(&source);
        let text = tree.collect();
        let rows: Vec<(&str, &str)> =
            text.lines().filter(|line| !line.starts_with('#')).filter_map(|line| line.split_once('\t')).collect();

        // The reader takes the file, which is the pairing this test exists for.
        let seed = Seed::read(&tree.seed_path()).expect("the reader takes the file that the collector wrote");
        assert_eq!(seed.names(), rows.len());

        let names: Vec<&str> = rows.iter().map(|(name, _)| *name).collect();
        assert!(names.contains(&TAKEN), "a name of exactly 64 bytes is the longest the format takes and must load");
        assert!(!names.contains(&DROPPED), "a name of 65 bytes must be dropped and not written");

        // The fold: a typedef, an alias declaration and a forward declaration all give `type`, and
        // only a declaration under a template head gives `template`.
        assert_eq!(rows, [
            ("Alias", "type"),
            (TAKEN, "type"),
            ("Ctor", "type"),
            ("Fwd", "type"),
            ("Other", "type"),
            ("Tpl", "template"),
        ]);

        // The order is strictly ascending by the bytes, which is what a binary search needs.
        assert!(names.windows(2).all(|pair| pair[0].as_bytes() < pair[1].as_bytes()), "{names:?}");
    }

    /// A NAME THAT THE PROJECT ALSO GIVES TO A FUNCTION, AN OBJECT, A PARAMETER OR A FUNCTION-LIKE
    /// MACRO GIVES NO ROW, AND A CONSTRUCTOR IS NOT SUCH A NAME.
    ///
    /// A declaration with no type is a constructor, a destructor, a conversion function or a
    /// deduction guide. #274 measured what happens without that rule: every class with a
    /// constructor falls out, and the standard library loses `vector`, `pair` and `function`.
    #[test]
    fn a_conflict_drops_a_name_and_a_constructor_does_not() {
        let source = "struct Keep { Keep(); Keep(int); ~Keep(); };\n\
                      struct AsFunction {};\n\
                      int AsFunction(int);\n\
                      struct AsObject {};\n\
                      int AsObject;\n\
                      struct AsParameter {};\n\
                      void f(int AsParameter);\n\
                      struct AsMacro {};\n\
                      #define AsMacro(x) (x)\n";
        let tree = Tree::new(source);
        let text = tree.collect();
        let names: Vec<&str> =
            text.lines().filter(|line| !line.starts_with('#')).filter_map(|line| line.split_once('\t')).map(|(name, _)| name).collect();
        assert_eq!(names, ["Keep"], "only the class whose other declarations are constructors keeps its row");
    }

    /// EVERY SEED OF test/seed IS ONE THAT THE READER TAKES.
    ///
    /// `cargo xtask seed check` compares a committed seed with a fresh collection, and it needs the
    /// corpus at /tmp/cpp-corpora, so the gate runs it and this suite cannot. THIS TEST COVERS WHAT
    /// IT CAN COVER WITH NO CORPUS: that the committed file is one the reader takes, that its rows
    /// ascend strictly by their bytes, and that no name passes the length of the format. A file
    /// that someone edits by hand fails here without waiting for a gate.
    #[test]
    fn every_committed_seed_is_one_that_the_reader_takes() {
        let directory = crate::repository().join("test").join("seed");
        let mut files: Vec<PathBuf> = fs::read_dir(&directory)
            .unwrap_or_else(|e| panic!("{} reads: {e}", directory.display()))
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|extension| extension == "seed"))
            .collect();
        files.sort();
        assert!(!files.is_empty(), "{} holds no seed, and the check of the gate would compare nothing", directory.display());
        for path in &files {
            let seed = Seed::read(path).unwrap_or_else(|e| panic!("the reader takes {}: {e}", path.display()));
            let text = fs::read_to_string(path).expect("the seed reads");
            let rows: Vec<(&str, &str)> =
                text.lines().filter(|line| !line.starts_with('#')).filter_map(|line| line.split_once('\t')).collect();
            assert_eq!(seed.names(), rows.len(), "{}", path.display());
            for (name, kind) in &rows {
                assert!(name.len() <= super::MAX_NAME, "{}: `{name}` has {} bytes", path.display(), name.len());
                assert!(matches!(*kind, "type" | "template"), "{}: the kind of `{name}` is `{kind}`", path.display());
            }
            assert!(
                rows.windows(2).all(|pair| pair[0].0.as_bytes() < pair[1].0.as_bytes()),
                "{}: the rows do not ascend strictly by their bytes",
                path.display()
            );
        }
    }

    /// THE PROJECT DECIDES THE THREE READINGS OF `struct MACRO NAME;`.
    ///
    /// The grammar reads every one of them as a forward declaration of `NAME` with an attribute
    /// macro, because the three are the same text and a file cannot tell them apart. A project can.
    /// The corpus gives 88 such names over its 64 projects, and the rule keeps 38 and drops 50.
    #[test]
    fn the_project_decides_a_forward_declaration_from_an_object_and_a_macro() {
        let source = "\
class META_TEMPLATE_VIS basic_string;\n\
struct CALL_CENTER_TBL {};\n\
struct CALL_CENTER_TBL g_w_call_center;\n\
#define ABSL_ATTRIBUTE_TRIVIAL_ABI\n\
class ABSL_MUST_USE_RESULT ABSL_ATTRIBUTE_TRIVIAL_ABI Ptr;\n";
        let tree = Tree::new(source);
        let text = tree.collect();
        let names: Vec<&str> = text
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| line.split_once('\t'))
            .map(|(name, _)| name)
            .collect();
        assert!(names.contains(&"basic_string"), "a forward declaration with a visibility macro is a type: {names:?}");
        assert!(!names.contains(&"g_w_call_center"), "an object of a type the project declares is no type: {names:?}");
        assert!(
            !names.contains(&"ABSL_ATTRIBUTE_TRIVIAL_ABI"),
            "a name that the project defines as a macro is no type: {names:?}"
        );
    }

    /// `struct LLVM_ABI A;` IS A FORWARD DECLARATION OF `A` AND NOT AN OBJECT.
    ///
    /// The fork reads it as an object `A` of the type `struct LLVM_ABI`, and both front ends read a
    /// forward declaration with an attribute. Without this rule the name reads as a conflict and
    /// falls out. libc++ loses `vector`, `pair`, `array`, `shared_ptr` and `basic_string` to it, and
    /// the corpus holds 925 file and name pairs of the shape.
    #[test]
    fn a_macro_before_a_forward_declaration_leaves_a_type() {
        let tree = Tree::new("struct LLVM_ABI Shape;\n");
        let text = tree.collect();
        let rows: Vec<(&str, &str)> =
            text.lines().filter(|line| !line.starts_with('#')).filter_map(|line| line.split_once('\t')).collect();
        assert_eq!(rows, [("Shape", "type")]);
    }
}
