//! The `seed` task: `cargo xtask seed collect`, which writes the seed of a project.
//!
//! A seed holds the names that a project declares as a type or as a template, and the names that it
//! defines as a macro. The scanner records the names that the FILE declares, and that record cannot
//! hold a name from a header, because no header is parsed. The reader of the file this task writes
//! is `crate::seed`, and the format is `src/seed.h`.
//!
//! THE TYPE ROWS HAVE TWO READERS, AND `--types` SELECTS ONE. `tree` reads the tree of each file, and
//! it is the reader of every committed seed. `text` reads the declaration positions of the text of each
//! file with `crate::declarations`, and it never parses. A misparse writes a wrong type row or drops a
//! correct one with the tree reader: the fork read `inline StringBuilder::operator StringView() const`
//! as a function named `StringView`, and WebKit lost `StringView` from its seed. The two readers give
//! the same kinds, so one rule (`resolve`, `type_word` and `rows`) writes the rows of both.
//!
//! THE MACRO ROWS READ THE TEXT OF THE `#define` LINES with `crate::defines`, and they never read a
//! tree. No grammar change can move a macro row.
//!
//! THE SCOPE. The macro rows read every source file of each project that LIST names, under ROOT.
//! population.txt holds the C++ files of the corpus and excludes the C headers, and a C header defines
//! the macros that the C++ files use: firefox/mfbt/Attributes.h defines `MOZ_UNANNOTATED`. Over the 652
//! sites of the trailing macro at f97fe26, the macro rows of population.txt reach 489 sites and the
//! macro rows of every source file reach 645. The type rows and the conflicts read the files of LIST by
//! default, and `--scope project` gives them every source file too. A wider scope changes the type rows
//! that the readers of `is_seed_type_name` read, and task 356 measures that change.
//!
//! `seed facts` WRITES THE FACTS OF EACH FILE, the input of a seed for each translation unit: the
//! kinds of each name, the include lines, and a hash of the two.
//!
//! THE TASK READS BACK WHAT IT WROTE WITH THAT READER BEFORE IT REPORTS SUCCESS. The reader refuses
//! a file that a binary search cannot read: a row that is not `name<TAB>kinds`, a kinds cell that is
//! not the words of `crate::seed::KIND_WORDS` in their order, a name of more than 64 bytes, and an
//! order that is not ascending by the bytes of the name. A collector that could write such a file
//! would be found by its user and not by itself, so the last step of the task is the reader.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use tree_sitter::{Language, Node, Parser};

use crate::declarations;
use crate::defines::{self, Shape};
use crate::seed::{KIND_WORDS, Seed};

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

/// The extensions of a source file of the C family, in lowercase. The macro rows read each file of a
/// project with one of these extensions, and each file with no extension whose first token is a
/// directive. Refer to `is_source_file`.
///
/// `.def` holds the X-macro lists of llvm-project, `.inc` and `.ipp` the included parts of a header,
/// and `.m` and `.mm` the Objective-C files, whose `#define` lines are the lines of the preprocessor
/// of C. Over the corpus of 2026-09-17 these extensions give 456,957 files.
const SOURCE_EXTENSIONS: [&str; 27] = [
    "c", "cc", "cpp", "cxx", "c++", "cp", "h", "hh", "hpp", "hxx", "h++", "inl", "ipp", "tcc", "tpp", "txx", "ixx",
    "inc", "def", "icc", "ii", "cu", "cuh", "m", "mm", "cppm", "ino",
];

/// The names of one project, with what its files say about each one.
#[derive(Default)]
struct Names {
    kinds: BTreeMap<String, BTreeSet<Kind>>,
    /// The `struct MACRO NAME;` sites, as the macro and the declared name. `resolve` reads them.
    attributed: Vec<(String, String)>,
    /// The names that the `#define` lines of the project define, with the bits of their shapes.
    /// `crate::defines` reads them from the text, and no parse writes this map.
    macros: BTreeMap<String, u16>,
    /// The alias targets of the object-like macros of the project. Refer to `defines::inherit_alias_bits`.
    aliases: BTreeMap<String, BTreeSet<String>>,
    /// The classes of the bodies of the object-like `#define` lines of the project, as the bits
    /// `BODY_TAG`, `BODY_ATTRIBUTE` and `BODY_OTHER` of each name, and the macro-shaped names that an alias
    /// body of `crate::declarations::macro_bodies` names. The text reader resolves its heads with them.
    bodies: BTreeMap<String, u8>,
    body_aliases: BTreeMap<String, BTreeSet<String>>,
}

/// A `#define` of the name has a tag body.
const BODY_TAG: u8 = 1;
/// A `#define` of the name has an attribute body.
const BODY_ATTRIBUTE: u8 = 2;
/// A `#define` of the name has another body.
const BODY_OTHER: u8 = 4;

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
        for (name, bits) in other.macros {
            *self.macros.entry(name).or_default() |= bits;
        }
        for (name, bits) in other.bodies {
            *self.bodies.entry(name).or_default() |= bits;
        }
        for (name, targets) in other.body_aliases {
            self.body_aliases.entry(name).or_default().extend(targets);
        }
        for (name, targets) in other.aliases {
            self.aliases.entry(name).or_default().extend(targets);
        }
    }

    /// Add the `#define` lines of the text of one source.
    fn define(&mut self, source: &[u8]) {
        for (name, body) in declarations::macro_bodies(source) {
            match body {
                declarations::Body::Tag => *self.bodies.entry(name).or_default() |= BODY_TAG,
                declarations::Body::Attribute => *self.bodies.entry(name).or_default() |= BODY_ATTRIBUTE,
                declarations::Body::Other => *self.bodies.entry(name).or_default() |= BODY_OTHER,
                declarations::Body::Alias(target) => {
                    self.bodies.entry(name.clone()).or_default();
                    self.body_aliases.entry(name).or_default().insert(target);
                }
            }
        }
        for define in defines::define_lines(source) {
            let bit = match define.shape {
                Shape::Object => KIND_WORDS[2].1,
                Shape::Function => KIND_WORDS[3].1,
            };
            if let Some(target) = define.alias {
                self.aliases.entry(define.name.clone()).or_default().insert(target);
            }
            *self.macros.entry(define.name).or_default() |= bit;
        }
    }

    /// What the project defines a name as, for the text reader. O(log n) in the names.
    fn macro_of(&self, name: &str) -> declarations::Macro {
        self.macro_of_depth(name, 0)
    }

    /// `macro_of`, following alias bodies to a depth of four.
    fn macro_of_depth(&self, name: &str, depth: usize) -> declarations::Macro {
        use declarations::Macro;
        let Some(shapes) = self.macros.get(name) else {
            return Macro::None;
        };
        if *shapes & KIND_WORDS[3].1 != 0 {
            return Macro::Other;
        }
        let mut bodies = self.bodies.get(name).copied().unwrap_or(0);
        for target in self.body_aliases.get(name).into_iter().flatten() {
            bodies |= match (depth < 4).then(|| self.macro_of_depth(target, depth + 1)) {
                Some(Macro::Tag) => BODY_TAG,
                Some(Macro::Attribute) => BODY_ATTRIBUTE,
                _ => BODY_OTHER,
            };
        }
        match bodies {
            BODY_TAG => Macro::Tag,
            BODY_ATTRIBUTE => Macro::Attribute,
            _ => Macro::Other,
        }
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

    /// The type word of a name, or None for a name that gives no type row.
    ///
    /// A NAME THAT A FILE ALSO DECLARES AS A FUNCTION, AN OBJECT, A PARAMETER OR A FUNCTION-LIKE
    /// MACRO GIVES NO TYPE. The scanner reads a name in one position and has no scope, so a project
    /// that uses one name for a type and for a function would make every call of that function a
    /// cast. #274 measured this rule: without it the standard library loses `vector`, `pair` and
    /// `function`, because a declaration with no type is a constructor and not a function.
    fn type_word(&self, name: &str) -> Option<&'static str> {
        let kinds = self.kinds.get(name)?;
        if kinds.contains(&Kind::Conflict) {
            None
        } else if kinds.contains(&Kind::Template) {
            Some("template")
        } else if kinds.contains(&Kind::Type) || kinds.contains(&Kind::TypeUse) {
            Some("type")
        } else {
            None
        }
    }

    /// The rows of the seed file, as the name and the kinds cell, and the names that the length
    /// dropped. O(n log n) in the names.
    ///
    /// THE MACRO WORDS NEVER REMOVE A TYPE WORD, AND A TYPE WORD NEVER REMOVES A MACRO WORD. A project
    /// can declare a name as a type and define it as a macro, and the row then holds the two kinds:
    /// zlib renames `Bytef` with a `#define` and declares it with a `typedef`. Over the 64 corpus
    /// projects, 731 names of the type rows of population.txt also have a macro row, 687 of them of
    /// the object shape. The type word keeps the rule of `type_word`, which reads the
    /// `preproc_function_def` nodes of the tree as before, so a macro row changes no type row.
    ///
    /// AN ALIAS HAS THE MACRO WORDS OF ITS TARGETS. `#define P_ BSLIM_TESTUTIL_P_` gives `P_` the word
    /// `function-macro` of `BSLIM_TESTUTIL_P_`, because `P_(LINE)` is a call of that macro. Refer to
    /// `defines::inherit_alias_bits`.
    fn rows(&self) -> (Vec<(&str, String)>, Vec<&str>) {
        let names: BTreeSet<&str> = self.kinds.keys().chain(self.macros.keys()).map(String::as_str).collect();
        let mut macros = self.macros.clone();
        defines::inherit_alias_bits(&mut macros, &self.aliases);
        let mut rows = Vec::new();
        let mut dropped = Vec::new();
        for name in names {
            let mut words: Vec<&str> = self.type_word(name).into_iter().collect();
            let bits = macros.get(name).copied().unwrap_or(0);
            words.extend(KIND_WORDS[2..].iter().filter(|(_, bit)| bits & bit != 0).map(|(word, _)| *word));
            if words.is_empty() {
                continue;
            }
            if name.len() > MAX_NAME {
                dropped.push(name);
                continue;
            }
            rows.push((name, words.join(",")));
        }
        // `BTreeSet` gives the names in the order of their bytes, which is the order the reader
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

/// The names that the `primitive_type` token of the grammar holds besides the keywords `bool`, `char`,
/// `int`, `float` and `double` (src/grammar.json).
///
/// A TYPE ROW FOR ONE OF THESE NAMES ONLY CHANGES A CORRECT LABEL. The grammar reads `int64_t` as a
/// `primitive_type` in each type position, and the scanner reads a seed row in its own positions:
/// `alignas(int64_t)` became a `type_identifier` in 31 regions of 17 arrow and other files. The tree
/// collector writes these rows from the `type_identifier` alias of a typedef declarator
/// (`typedef long int64_t;`), and the text collector writes none.
const GRAMMAR_PRIMITIVE_NAMES: [&str; 20] = [
    "size_t",
    "ssize_t",
    "ptrdiff_t",
    "intptr_t",
    "uintptr_t",
    "charptr_t",
    "nullptr_t",
    "max_align_t",
    "int8_t",
    "int16_t",
    "int32_t",
    "int64_t",
    "uint8_t",
    "uint16_t",
    "uint32_t",
    "uint64_t",
    "char8_t",
    "char16_t",
    "char32_t",
    "char64_t",
];

/// Read the names that one file declares, from its text with no parse. `macros` holds the `#define`
/// names of the whole project, which decide the heads of the file. O(n) in the bytes of the file.
///
/// THE KINDS ARE THE KINDS OF THE TREE READER, so `resolve`, `type_word` and `rows` read the two
/// collections with one rule: a type, a template and a type use give a type row, a value of any kind
/// gives a conflict, and the `#define` lines of the file give `Defined` and, with a parameter list, a
/// conflict. The names of an `#if 0` group are read, as the tree reader reads them. A name of
/// `GRAMMAR_PRIMITIVE_NAMES` gives no type kind.
fn text_names_of(source: &[u8], c_file: bool, project: &Names) -> Names {
    let facts = declarations::read(source, c_file);
    let mut names = Names::default();
    for (name, fact) in facts.resolve(|name| project.macro_of(name)) {
        let bits = fact.live | fact.dead;
        let bits = if GRAMMAR_PRIMITIVE_NAMES.contains(&name.as_str()) {
            bits & !(declarations::TYPE | declarations::TEMPLATE | declarations::TYPE_USE)
        } else {
            bits
        };
        if bits & declarations::TYPE != 0 {
            names.add(&name, Kind::Type);
        }
        if bits & declarations::TEMPLATE != 0 {
            names.add(&name, Kind::Template);
        }
        if bits & declarations::TYPE_USE != 0 {
            names.add(&name, Kind::TypeUse);
        }
        if bits & declarations::VALUE != 0 {
            names.add(&name, Kind::Conflict);
        }
    }
    for (name, shape) in defines::defines(source) {
        if shape == Shape::Function {
            names.add(&name, Kind::Conflict);
        }
        names.add(&name, Kind::Defined);
    }
    names
}

/// The reader that writes the type rows and the conflicts of a seed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Types {
    /// The tree of each file, from a parse.
    Tree,
    /// The declaration positions of the text of each file, with no parse.
    Text,
}

/// The files that the type rows and the conflicts read. The macro rows always read every source file
/// of each project.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scope {
    /// The files of the list of the collection.
    List,
    /// Every source file of each project that the list names, C headers included.
    Project,
}

/// The options of `seed collect`, `seed check` and `seed facts`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Options {
    types: Types,
    scope: Scope,
}

impl Options {
    /// The options at the end of the arguments, and the arguments before them.
    fn parse(args: &[String]) -> Result<(Vec<&String>, Self), String> {
        let mut options = Self { types: Types::Tree, scope: Scope::List };
        let mut positional = Vec::new();
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--types" => {
                    options.types = match iter.next().map(String::as_str) {
                        Some("tree") => Types::Tree,
                        Some("text") => Types::Text,
                        other => return Err(format!("--types takes `tree` or `text`, and the argument is {other:?}")),
                    }
                }
                "--scope" => {
                    options.scope = match iter.next().map(String::as_str) {
                        Some("list") => Scope::List,
                        Some("project") => Scope::Project,
                        other => return Err(format!("--scope takes `list` or `project`, and the argument is {other:?}")),
                    }
                }
                _ => positional.push(arg),
            }
        }
        Ok((positional, options))
    }
}

/// The project of a path: its first component.
fn project_of(path: &str) -> &str {
    path.split('/').next().unwrap_or("")
}

/// The text of a seed file. One writer, so `collect` and `check` cannot disagree by a byte. The second
/// comment row names the reader and the scope of the type rows, and the tree reader of the list keeps
/// the text of the seeds that came before the text reader.
fn seed_text(rows: &[(&str, String)], options: Options) -> String {
    let mut text = String::with_capacity(rows.len() * 32 + 512);
    text.push_str(&crate::seed::format_row());
    text.push('\n');
    text.push_str("# The names that this project declares as a type or as a template, and the names that it\n");
    text.push_str(match (options.types, options.scope) {
        (Types::Tree, Scope::List) => "# defines as a macro. The type rows read the tree of each file of the list of the collection.\n",
        (Types::Tree, Scope::Project) => "# defines as a macro. The type rows read the tree of each source file of the project.\n",
        (Types::Text, Scope::List) => {
            "# defines as a macro. The type rows read the declaration positions of the text of each file of the\n# list of the collection.\n"
        }
        (Types::Text, Scope::Project) => {
            "# defines as a macro. The type rows read the declaration positions of the text of each source file\n# of the project.\n"
        }
    });
    text.push_str("# The macro rows read the text of the `#define` lines of every source file of the project.\n");
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
fn write_seed(path: &Path, project: &str, names: &Names, options: Options) -> Result<String, Box<dyn Error>> {
    let (rows, dropped) = names.rows();
    let text = seed_text(&rows, options);
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
fn check(root: &Path, list: &Path, directory: &Path, options: Options) -> Result<(), Box<dyn Error>> {
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
        let collected = collect(root, &of_project, options, false)?;
        let names = collected.get("").ok_or("the collection of a project gave no names")?;
        let (rows, _) = names.rows();
        let fresh = seed_text(&rows, options);
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

/// Collect the names of the files of `paths` with the reader and the scope of `options`, and the macros
/// of the projects that the paths name. With `per_project`, give one `Names` for each project, and
/// otherwise one for all of them under the empty key. Each `Names` is resolved. O(n) in the bytes of
/// the files that the scope names.
///
/// THE MACROS OF A PROJECT ARE READ FIRST, because the text reader resolves the heads of a file with
/// them: `class EXPORT_MACRO Name {` and `struct A B;` need to know which word the project defines.
fn collect(root: &Path, paths: &[&str], options: Options, per_project: bool) -> Result<BTreeMap<String, Names>, Box<dyn Error>> {
    let projects: BTreeSet<&str> = paths.iter().map(|path| project_of(path)).collect();
    let mut collected: BTreeMap<String, Names> = BTreeMap::new();
    for project in projects {
        let macros = macros_of_project(root, project)?;
        let files: Vec<PathBuf> = match options.scope {
            Scope::List => paths.iter().filter(|path| project_of(path) == project).map(|path| root.join(path)).collect(),
            Scope::Project => source_files_of_project(root, project)?,
        };
        let names = files
            .par_iter()
            .fold(Names::default, |mut names: Names, path| {
                if let Ok(source) = fs::read(path) {
                    match options.types {
                        Types::Tree => names.merge(names_of(&source)),
                        Types::Text => names.merge(text_names_of(&source, is_c_file(path), &macros)),
                    }
                }
                names
            })
            .reduce(Names::default, |mut left, right| {
                left.merge(right);
                left
            });
        let key = if per_project { project.to_owned() } else { String::new() };
        let entry = collected.entry(key).or_default();
        entry.merge(names);
        entry.merge(macros);
    }
    for names in collected.values_mut() {
        names.resolve();
    }
    Ok(collected)
}

/// True for a file with the extension `.c`, whose class keys the text reader counts as a trap.
fn is_c_file(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "c")
}

/// True for a path that the macro rows read: a file with an extension of `SOURCE_EXTENSIONS`, in
/// any case, or a file with no extension. The caller reads a file with no extension only when its
/// first token is a directive.
fn is_source_file(path: &Path) -> Option<bool> {
    let name = path.file_name()?.to_str()?;
    // A leading dot starts a hidden name and not an extension: `.clang-format`.
    match name.strip_prefix('.').unwrap_or(name).rsplit_once('.') {
        None => Some(false),
        Some((_, extension)) => {
            let extension = extension.to_ascii_lowercase();
            SOURCE_EXTENSIONS.contains(&extension.as_str()).then_some(true)
        }
    }
}

/// Collect the names that the `#define` lines of every source file of one project define. O(n) in
/// the bytes of the files.
///
/// THE SCAN WALKS ROOT/PROJECT AND NOT THE LIST, because the list of the corpus excludes the C
/// headers that define the macros of the C++ files. A symbolic link is not followed, so a link to a
/// directory outside the project reads nothing.
///
/// A FILE WITH NO EXTENSION IS READ WHEN ITS FIRST TOKEN IS A DIRECTIVE. The headers of libstdc++,
/// libc++, the MSVC STL and Eigen have no extension: `bits/c++config` defines `_GLIBCXX20_CONSTEXPR`.
/// A configure script, a Makefile and a ChangeLog also have none, and their `# define` comment lines
/// give names such as `$2`, `name` and `componentroot`. Over the corpus of 2026-09-17 the test keeps
/// 2,698 `#define` lines of headers and drops 190 of scripts and documents.
fn macros_of_project(root: &Path, project: &str) -> Result<Names, Box<dyn Error>> {
    Ok(source_files_of_project(root, project)?
        .par_iter()
        .fold(Names::default, |mut names: Names, path| {
            if let Ok(source) = fs::read(path) {
                names.define(&source);
            }
            names
        })
        .reduce(Names::default, |mut left, right| {
            left.merge(right);
            left
        }))
}

/// The source files of one project under ROOT: each file with an extension of `SOURCE_EXTENSIONS`, and
/// each file with no extension whose first token is a directive. A symbolic link is not followed. The
/// paths ascend. O(n) in the files of the project, and a file with no extension is read to its first
/// token.
fn source_files_of_project(root: &Path, project: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    let mut directories = vec![root.join(project)];
    while let Some(directory) = directories.pop() {
        let entries =
            fs::read_dir(&directory).map_err(|e| format!("cannot read the directory {}: {e}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("cannot read an entry of {}: {e}", directory.display()))?;
            let file_type = entry.file_type()?;
            let path = entry.path();
            if file_type.is_dir() {
                directories.push(path);
            } else if file_type.is_file()
                && let Some(has_extension) = is_source_file(&path)
            {
                files.push((path, has_extension));
            }
        }
    }
    let mut kept: Vec<PathBuf> = files
        .into_par_iter()
        .filter(|(path, has_extension)| *has_extension || fs::read(path).is_ok_and(|source| defines::starts_with_directive(&source)))
        .map(|(path, _)| path)
        .collect();
    kept.sort();
    Ok(kept)
}

/// Collect the names of a file list and write one seed, or one seed for each project.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    const USAGE: &str = "usage: cargo xtask seed collect ROOT LIST (--out FILE | --out-dir DIRECTORY) [--types tree|text] [--scope list|project]\n       cargo xtask seed check ROOT LIST DIRECTORY [--types tree|text] [--scope list|project]\n       cargo xtask seed facts ROOT LIST OUT [--types tree|text] [--scope list|project]";
    let (positional, options) = Options::parse(args)?;
    let positional: Vec<&str> = positional.iter().map(|arg| arg.as_str()).collect();
    match positional.as_slice() {
        ["check", root, list, directory] => return check(Path::new(root), Path::new(list), Path::new(directory), options),
        ["facts", root, list, out] => return facts(Path::new(root), Path::new(list), Path::new(out), options),
        _ => {}
    }
    let (root, list, out, per_project) = match positional.as_slice() {
        ["collect", root, list, "--out", out] => (*root, *list, *out, false),
        ["collect", root, list, "--out-dir", out] => (*root, *list, *out, true),
        _ => return Err(USAGE.into()),
    };
    let root = PathBuf::from(root);
    let text = fs::read_to_string(list).map_err(|e| format!("cannot read the list {list}: {e}"))?;
    let paths: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    if paths.is_empty() {
        return Err(format!("the list {list} holds no path").into());
    }
    let collected = collect(&root, &paths, options, per_project)?;
    let mut reports = Vec::new();
    if per_project {
        let directory = PathBuf::from(out);
        for (project, names) in &collected {
            let path = directory.join(format!("{project}.seed"));
            reports.push(write_seed(&path, project, names, options)?);
        }
    } else {
        let names = collected.get("").ok_or("the list gave no file that reads")?;
        reports.push(write_seed(Path::new(out), "all", names, options)?);
    }
    for report in reports {
        println!("{report}");
    }
    Ok(())
}

/// Write the facts of each file of the scope, one file after the other, as the rows of one TSV. O(n)
/// in the bytes of the files.
///
/// THE ROWS OF ONE FILE, in the order of the paths:
///   `F  path  bytes  content  facts`   the bytes of the file, the FNV-1a hash of its bytes, and the
///                                      FNV-1a hash of its include rows and its name rows
///   `I  angle|quote  header`           one row for each `#include`, in every branch (text only)
///   `N  name  live  dead  line  value`  the kinds outside and inside an `#if 0` group, the first line, and
///                                      the first line of a value (text only; the tree gives the kinds of
///                                      `names_of` and the lines 0)
///   `A  macro  name`                   a `struct MACRO NAME;` site that `resolve` decides (tree only)
/// The kinds of the text are the words of `declarations::BIT_WORDS`, with the heads resolved by the
/// macros of the project. The kinds of the tree are `type`, `template`, `type-use`, `value` and
/// `defined`.
///
/// THE REPORT ON STANDARD OUTPUT gives the files and the distinct contents, for each extension, and
/// the counts of the traps of the text reader.
fn facts(root: &Path, list: &Path, out: &Path, options: Options) -> Result<(), Box<dyn Error>> {
    use std::io::Write as _;
    let text = fs::read_to_string(list).map_err(|e| format!("cannot read the list {}: {e}", list.display()))?;
    let paths: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let projects: BTreeSet<&str> = paths.iter().map(|path| project_of(path)).collect();
    let mut writer = std::io::BufWriter::new(fs::File::create(out).map_err(|e| format!("cannot write {}: {e}", out.display()))?);
    let mut by_extension: BTreeMap<String, (usize, BTreeSet<u64>)> = BTreeMap::new();
    let mut traps = declarations::Traps::default();
    let mut heads = declarations::HeadCounts::default();
    for project in projects {
        let macros = macros_of_project(root, project)?;
        let files: Vec<PathBuf> = match options.scope {
            Scope::List => paths.iter().filter(|path| project_of(path) == project).map(|path| root.join(path)).collect(),
            Scope::Project => source_files_of_project(root, project)?,
        };
        for chunk in files.chunks(4096) {
            let rows: Vec<(String, String, u64, declarations::Traps, declarations::HeadCounts)> = chunk
                .par_iter()
                .filter_map(|path| {
                    let source = fs::read(path).ok()?;
                    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().into_owned();
                    let content = crate::trees::fingerprint(&String::from_utf8_lossy(&source));
                    let mut body = String::new();
                    let mut file_traps = declarations::Traps::default();
                    let mut file_heads = declarations::HeadCounts::default();
                    match options.types {
                        Types::Text => {
                            let facts = declarations::read(&source, is_c_file(path));
                            file_traps = facts.traps;
                            for (form, header) in &facts.includes {
                                let form = if *form == declarations::Form::Angle { "angle" } else { "quote" };
                                body.push_str(&format!("I\t{form}\t{header}\n"));
                            }
                            for (name, fact) in facts.resolve_counting(|name| macros.macro_of(name), &mut file_heads) {
                                body.push_str(&format!(
                                    "N\t{name}\t{}\t{}\t{}\t{}\n",
                                    declarations::bits_text(fact.live),
                                    declarations::bits_text(fact.dead),
                                    fact.line,
                                    fact.value_line
                                ));
                            }
                            for (name, shape) in defines::defines(&source) {
                                let word = if shape == Shape::Function { "function-macro" } else { "object-macro" };
                                body.push_str(&format!("N\t{name}\t{word}\t\t0\t0\n"));
                            }
                        }
                        Types::Tree => {
                            let names = names_of(&source);
                            for (name, kinds) in &names.kinds {
                                let words: Vec<&str> = kinds
                                    .iter()
                                    .map(|kind| match kind {
                                        Kind::Type => "type",
                                        Kind::Template => "template",
                                        Kind::TypeUse => "type-use",
                                        Kind::Conflict => "value",
                                        Kind::Defined => "defined",
                                    })
                                    .collect();
                                body.push_str(&format!("N\t{name}\t{}\t\t0\t0\n", words.join(",")));
                            }
                            for (macro_name, name) in &names.attributed {
                                body.push_str(&format!("A\t{macro_name}\t{name}\n"));
                            }
                        }
                    }
                    let hash = crate::trees::fingerprint(&body);
                    let extension = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.strip_prefix('.').unwrap_or(name))
                        .and_then(|name| name.rsplit_once('.'))
                        .map_or_else(|| "-".to_owned(), |(_, extension)| extension.to_ascii_lowercase());
                    let head = format!("F\t{rel}\t{}\t{content:016x}\t{hash:016x}\n", source.len());
                    Some((head + &body, extension, content, file_traps, file_heads))
                })
                .collect();
            for (block, extension, content, file_traps, file_heads) in rows {
                heads.add(&file_heads);
                writer.write_all(block.as_bytes())?;
                let entry = by_extension.entry(extension).or_default();
                entry.0 += 1;
                entry.1.insert(content);
                traps.template_parameter_keys += file_traps.template_parameter_keys;
                traps.macro_body_class_keys += file_traps.macro_body_class_keys;
                traps.zero_groups += file_traps.zero_groups;
                traps.c_file_class_keys += file_traps.c_file_class_keys;
            }
        }
    }
    writer.flush()?;
    let files: usize = by_extension.values().map(|(count, _)| count).sum();
    let contents: BTreeSet<u64> = by_extension.values().flat_map(|(_, set)| set.iter().copied()).collect();
    println!("files {files}\tdistinct contents {}", contents.len());
    for (extension, (count, set)) in &by_extension {
        println!("  {extension}\t{count} files\t{} contents", set.len());
    }
    println!(
        "traps: class keys in template heads {}, class keys in macro bodies {}, #if 0 groups {}, class keys in .c files {}",
        traps.template_parameter_keys, traps.macro_body_class_keys, traps.zero_groups, traps.c_file_class_keys
    );
    println!(
        "class heads of two words or more: definition with the last word {}, with an earlier word by a macro row {}, by the shape of the last word {}; brace initializer {}; forward declaration {}; type and object {}; object of a tag macro {}",
        heads.definition_last,
        heads.definition_macro_row,
        heads.definition_macro_shape,
        heads.initializer,
        heads.forward,
        heads.object,
        heads.tag_object
    );
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
            self.collect_with(&[])
        }

        /// Collect the seed with more options after the output path, and give its text.
        fn collect_with(&self, options: &[&str]) -> String {
            let out = self.0.join("out.seed");
            let args: Vec<String> = ["collect", self.0.to_str().expect("a path of UTF-8")]
                .into_iter()
                .chain([self.0.join("list.txt").to_str().expect("a path of UTF-8"), "--out"])
                .chain([out.to_str().expect("a path of UTF-8")])
                .chain(options.iter().copied())
                .map(str::to_owned)
                .collect();
            run(&args).expect("the collector writes a seed");
            fs::read_to_string(&out).expect("the seed reads")
        }

        fn seed_path(&self) -> PathBuf {
            self.0.join("out.seed")
        }

        /// Write a file of the project that the list does not name.
        fn add(&self, rel: &str, source: &[u8]) {
            let path = self.0.join("project").join(rel);
            fs::create_dir_all(path.parent().expect("a file is in a directory")).expect("the directory of the file");
            fs::write(path, source).expect("the file of the test");
        }
    }

    /// The rows of a seed text, as the name and the kinds cell.
    fn rows(text: &str) -> Vec<(&str, &str)> {
        text.lines().filter(|line| !line.starts_with('#')).filter_map(|line| line.split_once('\t')).collect()
    }

    /// The names of the rows that give a type or a template.
    fn type_names(text: &str) -> Vec<&str> {
        rows(text)
            .into_iter()
            .filter(|(_, kinds)| kinds.split(',').any(|word| word == "type" || word == "template"))
            .map(|(name, _)| name)
            .collect()
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
        assert_eq!(type_names(&text), ["Keep"], "only the class whose other declarations are constructors keeps its type");
        // The macro keeps its own row with no type word, because a macro row and a type row are two
        // facts. The function-like macro of the tree still removes the type word, as before.
        assert_eq!(rows(&text), [("AsMacro", "function-macro"), ("Keep", "type")]);
    }

    /// THE MACRO ROWS READ THE TEXT OF EVERY SOURCE FILE OF THE PROJECT, AND NO TREE.
    ///
    /// Each file and each line names an input that one rule of the collector decides:
    /// - A C header that the list does not name defines `MOZ_UNANNOTATED` and `GUARDED_BY(x)`. The
    ///   list of the corpus excludes such a header, and firefox/mfbt/Attributes.h is one.
    /// - A header with no extension that starts with a directive defines `_GLIBCXX20_CONSTEXPR`, as
    ///   libstdc++ `bits/c++config` does. A configure script with no extension and a text file do
    ///   not give their names.
    /// - A raw string of the listed file holds a `#define`, and it gives no row.
    /// - An `#if 0` branch gives its row, because the reader takes every branch.
    /// - `Bytef` is a typedef of the listed file and a macro of the header, and its row holds the
    ///   two kinds. A function-like macro of the header does not remove the type word of `Both`,
    ///   because the type rows read the tree of the listed files only.
    #[test]
    fn the_macro_rows_read_the_text_of_every_source_file_of_the_project() {
        let tree = Tree::new(
            "typedef unsigned char Bytef;\n\
             struct Both {};\n\
             const char *s = R\"(\n#define IN_RAW\n)\";\n\
             #if 0\n\
             #define IN_DEAD(x) x\n\
             #endif\n",
        );
        tree.add("include/attributes.h", b"#define MOZ_UNANNOTATED\n#define GUARDED_BY(x)\n#define Bytef z_Bytef\n#define Both(x) x\n");
        tree.add("include/bits/c++config", b"// Predefined symbols -*- C++ -*-\n#ifndef _GLIBCXX_CXX_CONFIG_H\n#define _GLIBCXX20_CONSTEXPR constexpr\n#endif\n");
        tree.add("configure", b"#! /bin/sh\n# define name\n#define PACKAGE_NAME \"x\"\n");
        tree.add("doc.txt", b"#define IN_TEXT\n");
        // An alias in one header and its function-like target in another, as in bde: `P_` gets the
        // word of `BSLIM_TESTUTIL_P_`. `Bytef` above is an alias of `z_Bytef`, which no file defines.
        tree.add("include/aliases.h", b"#define P_ BSLIM_TESTUTIL_P_\n");
        tree.add("include/testutil.h", b"#define BSLIM_TESTUTIL_P_(X) X\n");
        let text = tree.collect();
        assert_eq!(rows(&text), [
            ("BSLIM_TESTUTIL_P_", "function-macro"),
            ("Both", "type,function-macro"),
            ("Bytef", "type,object-macro"),
            ("GUARDED_BY", "function-macro"),
            ("IN_DEAD", "function-macro"),
            ("MOZ_UNANNOTATED", "object-macro"),
            ("P_", "object-macro,function-macro"),
            ("_GLIBCXX20_CONSTEXPR", "object-macro"),
        ]);
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
            let rows = rows(&text);
            assert_eq!(seed.names(), rows.len(), "{}", path.display());
            for (name, kinds) in &rows {
                assert!(name.len() <= super::MAX_NAME, "{}: `{name}` has {} bytes", path.display(), name.len());
                assert!(
                    kinds.split(',').all(|word| crate::seed::KIND_WORDS.iter().any(|(known, _)| *known == word)),
                    "{}: the kinds of `{name}` are `{kinds}`",
                    path.display()
                );
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
        let names = type_names(&text);
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

    /// THE TEXT COLLECTOR GIVES THE ROWS OF THE TREE COLLECTOR FOR THE SHAPES ABOVE, AND NO ROW FOR A
    /// NAME OF THE `primitive_type` TOKEN.
    ///
    /// `typedef long int64_t;` declares a type, but the grammar reads `int64_t` as a `primitive_type` in
    /// each type position. A row only changes that label where the scanner reads the seed.
    #[test]
    fn the_text_collector_gives_the_rows_of_the_tree_collector() {
        let conflicts = "struct Keep { Keep(); Keep(int); ~Keep(); };\n\
                         struct AsFunction {};\n\
                         int AsFunction(int);\n\
                         struct AsObject {};\n\
                         int AsObject;\n\
                         struct AsParameter {};\n\
                         void f(int AsParameter);\n\
                         struct AsMacro {};\n\
                         #define AsMacro(x) (x)\n";
        let tree = Tree::new(conflicts);
        let text = tree.collect_with(&["--types", "text"]);
        assert_eq!(rows(&text), [("AsMacro", "function-macro"), ("Keep", "type")]);
        // The header names the reader, and the tree reader of the list keeps its earlier header.
        assert!(text.contains("# defines as a macro. The type rows read the declaration positions of the text of each file of the\n"));
        assert!(tree.collect_with(&["--types", "tree"]).contains(
            "# defines as a macro. The type rows read the tree of each file of the list of the collection.\n"
        ));

        // The text reads `struct WORD NAME;` as a forward declaration only when the project defines `WORD`.
        // Otherwise `WORD` is a type and `NAME` an object. The tree rule reads a forward declaration for an
        // uppercase `WORD` that the project does not declare as a type. Over the list the two rules differ
        // on 31 sites, where Clang declares `NAME` as a value on 16 and as a type on none: clang tests
        // write `struct SS {};` in one file and `struct SS ss;` in another.
        let tree = Tree::new(
            "#define LLVM_ABI\nstruct LLVM_ABI Shape;\nstruct SS ss;\ntypedef long int64_t;\ntypedef unsigned long size_t;\ntypedef int Mine;\n",
        );
        assert_eq!(
            rows(&tree.collect_with(&["--types", "text"])),
            [("LLVM_ABI", "object-macro"), ("Mine", "type"), ("SS", "type"), ("Shape", "type")]
        );

        // A header of the project defines a type name as a macro of the compiler. The name before a
        // declarator stays, so the function `Hash` is a value and `Hash` gives no row.
        let tree = Tree::new("struct Hash {};\ninline uint64_t Hash(int x) { return 0; }\n");
        tree.add("lib/Headers/wrapper.h", b"#define uint64_t __UINT64_TYPE__\n");
        assert_eq!(rows(&tree.collect_with(&["--types", "text"])), [("uint64_t", "object-macro")]);
    }
}
