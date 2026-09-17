//! The names that a source declares, read from its text with no parse.
//!
//! THIS READER NEVER PARSES. The type rows of a seed that read a tree inherit each misparse of the
//! tree: the fork read `inline StringBuilder::operator StringView() const` as a function named
//! `StringView`, the collector took `StringView` for a function, and WebKit lost `StringView` from its
//! seed. A rule that reads the seed then inherits the misparse too. This reader takes the names from
//! the declaration positions of the text: a class key and its name, a typedef and its declarators,
//! `using NAME =`, a template head, and the declarators of an object, a function and a parameter.
//!
//! THE READER STRIPS WHAT HIDES TEXT FROM THE COMPILER before it reads a name: line splices, line
//! comments, block comments, string literals, character literals, raw string literals, and the
//! directive lines. The body of a `#define` gives no name, because a class key in a macro body declares
//! nothing until an expansion. An `#if 0` group is read, and each name of it carries a separate bit, so
//! a consumer decides.
//!
//! EACH FILE GIVES FACTS, AND THE PROJECT RESOLVES SOME OF THEM. A fact that one file decides is a bit
//! of a name. Three shapes need the macros of the whole project, and the file keeps them as head
//! records (`Head`):
//!   `class EXPORT_MACRO Name {` and `class Name FINAL_MACRO {`: the name is the word that is no macro.
//!   `struct A B;`: a forward declaration of `B` when `A` is a macro, and an object `B` of the type `A`
//!   when it is not. `struct stat st;` is the second reading, and it still names the type `stat`.
//!   `Foo bar;`, `LLVM_ABI Foo bar();`, `int foo GUARDED_BY(mu);`: the declarator is the last word that
//!   is no macro.
//!   `typedef int foo_t ATTRIBUTE;`: the typedef name is the last word that is no macro.
//! `resolve` gives the bits of each head with the macro names of the project.
//!
//! WHAT THE READER CANNOT SEE: a name that a macro expansion declares (`DECLARE_TYPE(Foo)`), a scope
//! (a parameter `Mutex` and a class `Mutex` are one name), and the active branch of a conditional.

use std::collections::BTreeMap;

/// A class, struct, union or enum definition, a typedef, or an alias declaration.
pub const TYPE: u32 = 1;
/// A type declaration under a template head.
pub const TEMPLATE: u32 = 2;
/// A forward declaration, a friend class declaration, or an elaborated type specifier.
pub const TYPE_USE: u32 = 4;
/// A variable or a data member.
pub const OBJECT: u32 = 8;
/// A function or a member function.
pub const FUNCTION: u32 = 16;
/// A parameter of a function, of a lambda, or of a catch handler.
pub const PARAMETER: u32 = 32;
/// A template parameter that is no type: `template <int N>`.
pub const TEMPLATE_VALUE: u32 = 64;
/// A type template parameter: `template <class T>`. It is a type only inside its template, and a
/// project seed has no scope, so no row reads this bit.
pub const TYPE_PARAMETER: u32 = 128;
/// A concept name: `template <class T> concept Name = ...;`.
pub const CONCEPT: u32 = 256;
/// A `#define` of the name with no parameter list.
pub const OBJECT_MACRO: u32 = 512;
/// A `#define` of the name with a parameter list.
pub const FUNCTION_MACRO: u32 = 1024;
/// A declarator whose name follows `&` or `&&`: `A& RefFn();`, `typedef T& reference;`. The bit marks
/// the names that the tree collector of 6e1adae records nowhere, because `leaf_name` peels a declarator
/// by its `declarator` field and `reference_declarator` holds its child with no field.
pub const REFERENCE: u32 = 2048;
/// A class key after `friend`: `friend class X;`. A friend class declaration declares the class, and
/// libclang gives a FriendDecl with no class child, so the oracle of r74 cannot see this type. The bit
/// lets a score name those sites.
pub const FRIEND: u32 = 4096;
/// An object or a function at namespace scope or at class scope: each brace around the declaration opens
/// a namespace, a linkage specification or a class body. C++ gives each other value a scope of its
/// function, its block, its parameter list or its template, and such a value hides no type of a file.
pub const OUTER: u32 = 8192;
/// An object or a function at namespace scope: each brace around the declaration opens a namespace or a
/// linkage specification, and its declarator has no qualifier. A member has the scope of its class, also
/// when a qualified declarator defines it outside the class: `tripoint_abs_ms const &diag_value::tripoint()`.
pub const NAMESPACE: u32 = 16384;
/// Every bit of a declaration that gives a value.
pub const VALUE: u32 = OBJECT | FUNCTION | PARAMETER | TEMPLATE_VALUE;

/// The words of the bits, in the order of their values. `bits_text` writes them.
pub const BIT_WORDS: [(u32, &str); 15] = [
    (TYPE, "type"),
    (TEMPLATE, "template"),
    (TYPE_USE, "type-use"),
    (OBJECT, "object"),
    (FUNCTION, "function"),
    (PARAMETER, "parameter"),
    (TEMPLATE_VALUE, "template-value"),
    (TYPE_PARAMETER, "type-parameter"),
    (CONCEPT, "concept"),
    (OBJECT_MACRO, "object-macro"),
    (FUNCTION_MACRO, "function-macro"),
    (REFERENCE, "reference"),
    (FRIEND, "friend"),
    (OUTER, "outer"),
    (NAMESPACE, "namespace"),
];

/// The bits of a name as a comma list of the words of `BIT_WORDS`.
pub fn bits_text(bits: u32) -> String {
    BIT_WORDS
        .iter()
        .filter(|(bit, _)| bits & bit != 0)
        .map(|(_, word)| *word)
        .collect::<Vec<_>>()
        .join(",")
}

/// What a file says about one name.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Fact {
    /// The bits of the declarations outside an `#if 0` group.
    pub live: u32,
    /// The bits of the declarations inside an `#if 0` group.
    pub dead: u32,
    /// The line of the first declaration of the name, counted in the lines of the file.
    pub line: u32,
    /// The line of the first declaration of the name that gives a value, or 0 for none. A reader of a
    /// lost type row reads this line.
    pub value_line: u32,
}

impl Fact {
    /// Add bits at a line.
    fn add(&mut self, bits: u32, dead: bool, line: u32) {
        if dead {
            self.dead |= bits;
        } else {
            self.live |= bits;
        }
        self.line = self.line.min(line);
        if bits & VALUE != 0 && (self.value_line == 0 || line < self.value_line) {
            self.value_line = line;
        }
    }
}

/// What the body of an object-like `#define` is. `macro_bodies` reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// One name with a lowercase letter that is no keyword and no reserved name: `#define flatbuffers_stat
    /// stat`. Such a macro can name a struct tag.
    Tag,
    /// No text, or only specifier keywords, reserved names, attribute groups and a linkage string:
    /// `#define BOOST_CXX14_CONSTEXPR constexpr`, `#define MY_API __declspec(dllexport)`.
    Attribute,
    /// One name with no lowercase letter: `#define BOOST_FUSION_GPU_ENABLED BOOST_GPU_ENABLED`. The class
    /// is the class of the named macro, and `Other` when the project does not define it.
    Alias(String),
    /// Each other body: `#define DWORD unsigned long`, `#define PFX std::filesystem::`.
    Other,
}

/// What a project defines a name as. The resolution of the heads reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Macro {
    /// No `#define` of the project names it.
    None,
    /// Each `#define` of the name is a `Body::Tag`.
    Tag,
    /// Each `#define` of the name is a `Body::Attribute`.
    Attribute,
    /// A function-like `#define`, bodies of different classes, or a `Body::Other`.
    Other,
}

/// The token that follows the words of a class head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Follow {
    /// `{`, or a base clause and then `{`.
    Body,
    /// `;`.
    Semicolon,
    /// `*`, `&`, `&&`, `^`, or a cv-qualifier.
    Pointer,
    /// `(`: a function declarator.
    Call,
    /// `=`, `[`, `,`, `)`, `:` of a bit-field, and each other token.
    Other,
}

/// A record whose bits need the macros of the whole project. `resolve` reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Head {
    /// `class-key W1 ... Wn FOLLOW` with two words or more.
    Class {
        /// True after `friend`: `friend class ABSL_NULLABILITY_COMPATIBLE UniquePtr;` declares no object,
        /// so each word before the last one is a macro.
        friend: bool,
        words: Vec<String>,
        follow: Follow,
        template: bool,
        typedef: bool,
        dead: bool,
        line: u32,
        initializer: bool,
        /// The bits `OUTER` and `NAMESPACE` of an object that the head declares.
        scope: u32,
    },
    /// A declarator that ends a run of plain names: `Foo bar;`, `LLVM_ABI Foo bar();`. `typed` is true
    /// when a keyword type, a qualified name, a template-id or a pointer came before the run.
    ///
    /// `qualifier` is the name before `::` of a qualified declarator. When the project defines it and
    /// each word before the declarator as macros, the declarator is an out-of-line constructor of a
    /// class that a macro names: `PB_DS_CLASS_T_DEC PB_DS_CLASS_C_DEC::trigger(float x)`.
    Run {
        words: Vec<String>,
        typed: bool,
        bits: u32,
        dead: bool,
        line: u32,
        qualifier: Option<String>,
    },
    /// The last name of a typedef declarator and the name before it: `typedef Foo foo_t ATTRIBUTE;`.
    /// `previous_typed` is true when a type token stands before `previous`, so that `previous` can be
    /// the declarator: `typedef int foo_t PACKED;`, and not `typedef Foo T29;`.
    Typedef {
        last: String,
        previous: String,
        previous_typed: bool,
        dead: bool,
        line: u32,
    },
}

/// How an `#include` names its header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Form {
    /// `#include <path>`.
    Angle,
    /// `#include "path"`.
    Quote,
}

/// The count of each reading that the heads of the class keys take in a resolution.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct HeadCounts {
    /// `class A B {` whose name is the last word, as the grammar reads it.
    pub definition_last: u64,
    /// `class Name FINAL_MACRO {` whose name is an earlier word, because the project defines the last one.
    pub definition_macro_row: u64,
    /// `class Name FINAL_MACRO {` whose name is an earlier word by the shape of the last one.
    pub definition_macro_shape: u64,
    /// `struct stat source_descr{};`: an object with a brace initializer.
    pub initializer: u64,
    /// `class MACRO Name;` and `friend class MACRO Name;`: a forward declaration of the last word.
    pub forward: u64,
    /// `struct stat st;`: a type and an object.
    pub object: u64,
    /// `struct flatbuffers_stat file_info;`: an object of a tag that a macro names.
    pub tag_object: u64,
}

impl HeadCounts {
    /// Add the counts of another resolution.
    pub fn add(&mut self, other: &Self) {
        self.definition_last += other.definition_last;
        self.definition_macro_row += other.definition_macro_row;
        self.definition_macro_shape += other.definition_macro_shape;
        self.initializer += other.initializer;
        self.forward += other.forward;
        self.object += other.object;
        self.tag_object += other.tag_object;
    }
}

/// The counts of the traps of a file, for the report of a collection.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Traps {
    /// `class` or `typename` in a template parameter list, which gives a type parameter.
    pub template_parameter_keys: u32,
    /// A class key and a name on a `#define` line, which declares nothing.
    pub macro_body_class_keys: u32,
    /// The `#if 0` groups of the file.
    pub zero_groups: u32,
    /// The keyword `class` and a name in a file with the extension `.c`, where `class` is a name of C.
    pub c_file_class_keys: u32,
}

/// The facts of one source.
#[derive(Clone, Default, Debug)]
pub struct FileFacts {
    /// The bits of each name that the file decides alone.
    pub names: BTreeMap<String, Fact>,
    /// The records that the macros of the project decide.
    pub heads: Vec<Head>,
    /// The `#include` lines, in every branch.
    pub includes: Vec<(Form, String)>,
    /// The counts of the traps.
    pub traps: Traps,
}

impl FileFacts {
    fn add(&mut self, name: &str, bits: u32, dead: bool, line: u32) {
        if name.is_empty() {
            return;
        }
        self.names
            .entry(name.to_owned())
            .or_insert(Fact {
                live: 0,
                dead: 0,
                line,
                value_line: 0,
            })
            .add(bits, dead, line);
    }

    /// The bits of each name with the heads resolved by `macro_of`, which gives what the project defines
    /// a name as. O(n) in the names and the words of the heads.
    pub fn resolve(&self, macro_of: impl Fn(&str) -> Macro) -> BTreeMap<String, Fact> {
        self.resolve_counting(macro_of, &mut HeadCounts::default())
    }

    /// `resolve`, and each head adds one to the count of the reading that it takes.
    pub fn resolve_counting(
        &self,
        macro_of: impl Fn(&str) -> Macro,
        counts: &mut HeadCounts,
    ) -> BTreeMap<String, Fact> {
        let is_macro = |word: &str| macro_of(word) != Macro::None;
        let is_tag_macro = |word: &str| macro_of(word) == Macro::Tag;
        // A word that a strip always removes: a macro whose every body is an attribute, or a macro with
        // the shape of an attribute macro after the declarator (`int foo GUARDED_BY(mu);`).
        let is_attribute = |word: &str| macro_of(word) == Macro::Attribute;
        let is_attribute_macro = |word: &str, _: &dyn Fn(&str) -> bool| {
            is_attribute(word) || (is_macro(word) && (is_macro_shaped(word.as_bytes()) || word.starts_with("__")))
        };
        fn last_plain<'w>(range: &'w [String], is_macro: &impl Fn(&str) -> bool) -> Option<&'w str> {
            range.iter().rev().find(|word| !is_macro(word)).map(String::as_str)
        }
        let mut names = self.names.clone();
        let mut add = |name: &str, bits: u32, dead: bool, line: u32| {
            if name.is_empty() || bits == 0 {
                return;
            }
            names
                .entry(name.to_owned())
                .or_insert(Fact {
                    live: 0,
                    dead: 0,
                    line,
                    value_line: 0,
                })
                .add(bits, dead, line);
        };
        for head in &self.heads {
            match head {
                Head::Class {
                    friend,
                    words,
                    follow,
                    template,
                    typedef,
                    dead,
                    line,
                    initializer,
                    scope,
                } => {
                    let type_bits = if *template { TEMPLATE } else { 0 };
                    let last = words.len() - 1;
                    // A HEAD OF THREE WORDS OR MORE HOLDS MACROS, AND A MACRO OF ANOTHER PROJECT HAS NO ROW:
                    // protobuf writes `class ABSL_MUST_USE_RESULT ... ABSL_NULLABILITY_COMPATIBLE
                    // PROTOBUF_NULL_AFTER_MOVE UniquePtr;` with the macros of abseil. There, a word with no
                    // lowercase letter is a macro.
                    let is_macro =
                        |word: &str| is_macro(word) || (words.len() >= 3 && is_macro_shaped(word.as_bytes()));
                    // `struct Shape FLATBUFFERS_FINAL_CLASS : private Table {`: in a definition head of two
                    // words, a last word with no lowercase letter after a word with one is a macro of another
                    // project.
                    let is_macro = |word: &str| {
                        is_macro(word)
                            || (*follow == Follow::Body
                                && words.len() == 2
                                && word == words[1]
                                && is_macro_shaped(word.as_bytes())
                                && words[0].bytes().any(|byte| byte.is_ascii_lowercase()))
                    };
                    // `struct stat source_descr{};` is an object with a brace initializer, and `class EXPORT Foo {};`
                    // is a definition. A brace group with no `;` is an initializer when the last word is no
                    // macro and a word before it is no macro and has a lowercase letter, because an attribute
                    // macro that the project does not define has none: `class EXPORT Foo {};`.
                    let object = *initializer
                        && !is_macro(&words[last])
                        && words[..last]
                            .iter()
                            .any(|word| !is_macro(word) && !is_macro_shaped(word.as_bytes()));
                    match follow {
                        Follow::Body if !object => {
                            let name = last_plain(words, &is_macro).unwrap_or(&words[last]);
                            if name == words[last] {
                                counts.definition_last += 1;
                            } else if macro_of(&words[last]) != Macro::None {
                                counts.definition_macro_row += 1;
                            } else {
                                counts.definition_macro_shape += 1;
                            }
                            add(name, TYPE | type_bits, *dead, *line);
                        }
                        Follow::Pointer => {
                            if let Some(name) = last_plain(words, &is_macro) {
                                add(name, TYPE_USE | type_bits, *dead, *line);
                            }
                        }
                        Follow::Body | Follow::Semicolon | Follow::Call | Follow::Other => {
                            let before = &words[..last];
                            if *follow == Follow::Body {
                                counts.initializer += 1;
                            }
                            if *typedef {
                                // `typedef struct A B;`: the typedef reads `B`, and `A` names a type.
                                if let Some(name) = last_plain(before, &is_macro) {
                                    add(name, TYPE_USE, *dead, *line);
                                }
                            } else if *friend
                                || (before.iter().all(|word| is_macro(word))
                                    && !before.iter().any(|word| is_tag_macro(word)))
                            {
                                // `class META_TEMPLATE_VIS basic_string;`: a declaration of the last word.
                                counts.forward += 1;
                                if !is_macro(&words[last]) {
                                    add(&words[last], TYPE_USE | type_bits, *dead, *line);
                                }
                            } else if before.iter().all(|word| is_macro(word)) {
                                // `struct flatbuffers_stat file_info;` with `#define flatbuffers_stat stat`: the
                                // macro names the tag, so the last word is an object. The macro name is no type.
                                counts.tag_object += 1;
                                if !is_macro(&words[last]) {
                                    let bits = if *follow == Follow::Call { FUNCTION } else { OBJECT };
                                    add(&words[last], bits | *scope, *dead, *line);
                                }
                            } else {
                                // `struct stat st;`: the type `stat` and the object `st`.
                                counts.object += 1;
                                if let Some(name) = last_plain(before, &is_macro) {
                                    add(name, TYPE_USE, *dead, *line);
                                }
                                if !is_macro(&words[last]) {
                                    let bits = if *follow == Follow::Call { FUNCTION } else { OBJECT };
                                    add(&words[last], bits | *scope, *dead, *line);
                                }
                            }
                        }
                    }
                }
                Head::Run {
                    words,
                    typed,
                    bits,
                    dead,
                    line,
                    qualifier,
                } => {
                    // THE LEADING MACROS GO FIRST, AND A TRAILING WORD GOES ONLY WHEN IT HAS THE SHAPE OF AN
                    // ATTRIBUTE MACRO. A project defines short lowercase names as macros in a test (boost
                    // defines `f`), and `BOOST_CONSTEXPR FromTimePoint f(ms);` would give the type
                    // `FromTimePoint` as the declarator if `f` went. A run that keeps one word and has no type
                    // before it gives nothing, so no strip can turn a type into a value.
                    // A STRIP NEVER LEAVES FEWER WORDS THAN A TYPE AND A NAME NEED: `DWORD CreationDisposition = 0;`
                    // keeps both words when a file of the project defines `DWORD`.
                    // AN ATTRIBUTE MACRO GOES WHATEVER REMAINS: `BOOST_CXX14_CONSTEXPR BOOST_FUSION_GPU_ENABLED
                    // deque(T&& t)` in a header that the class body includes is a constructor, and the run then
                    // keeps one name and gives nothing.
                    if !*typed
                        && qualifier.as_deref().is_some_and(&is_macro)
                        && words[..words.len() - 1].iter().all(|word| is_macro(word))
                    {
                        continue;
                    }
                    let keep = if *typed { 1 } else { 2 };
                    let mut begin = 0;
                    while begin < words.len() && is_attribute(&words[begin]) {
                        begin += 1;
                    }
                    while words.len() - begin > keep && is_macro(&words[begin]) {
                        begin += 1;
                    }
                    let mut end = words.len();
                    while end > begin && is_attribute(&words[end - 1]) {
                        end -= 1;
                    }
                    while end - begin > keep && is_attribute_macro(&words[end - 1], &is_macro) {
                        end -= 1;
                    }
                    if end <= begin || end - begin < keep {
                        continue;
                    }
                    // A macro with arguments after the declarator was the group: `int foo GUARDED_BY(mu);`.
                    let bits = if end < words.len() && *bits & !(OUTER | NAMESPACE) == FUNCTION {
                        OBJECT | *bits & (OUTER | NAMESPACE)
                    } else {
                        *bits
                    };
                    add(&words[end - 1], bits, *dead, *line);
                }
                Head::Typedef {
                    last,
                    previous,
                    previous_typed,
                    dead,
                    line,
                } => {
                    let name = if *previous_typed && is_attribute_macro(last, &is_macro) && !is_macro(previous) {
                        previous
                    } else {
                        last
                    };
                    add(name, TYPE, *dead, *line);
                }
            }
        }
        names
    }
}

// ---------------------------------------------------------------------------------------------------
// The lexer.
// ---------------------------------------------------------------------------------------------------

/// The class of a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Word,
    Number,
    Literal,
    /// A punctuator: an ASCII byte, or one of the codes below for a punctuator of more bytes.
    Punct(u16),
}

const SCOPE: u16 = 256;
const ARROW: u16 = 257;
const ELLIPSIS: u16 = 258;
const AND_AND: u16 = 259;
const OR_OR: u16 = 260;
const SHIFT_LEFT: u16 = 261;
const COMPARE: u16 = 262;
const INCREMENT: u16 = 263;
const ASSIGN_OPERATOR: u16 = 264;
const MEMBER_POINTER: u16 = 265;
const PASTE: u16 = 266;

#[derive(Clone, Copy, Debug)]
struct Token {
    class: Class,
    start: u32,
    end: u32,
    line: u32,
    dead: bool,
}

/// The tokens of a source and what the directive lines give.
struct Lexed {
    tokens: Vec<Token>,
    includes: Vec<(Form, String)>,
    macro_bodies: Vec<(String, Body)>,
    traps: Traps,
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 0x80
}

fn is_identifier_start(byte: u8) -> bool {
    is_identifier_byte(byte) && !byte.is_ascii_digit()
}

/// The source with each line splice removed, and the offsets of the removed splices in the result.
/// A source with no splice is not copied. O(n) in the bytes.
fn without_splices(source: &[u8]) -> (std::borrow::Cow<'_, [u8]>, Vec<u32>) {
    let has_splice = source
        .windows(2)
        .any(|pair| pair[0] == b'\\' && matches!(pair[1], b'\n' | b'\r' | b' ' | b'\t'));
    if !has_splice {
        return (std::borrow::Cow::Borrowed(source), Vec::new());
    }
    let mut out = Vec::with_capacity(source.len());
    let mut splices = Vec::new();
    let mut i = 0;
    while i < source.len() {
        if source[i] == b'\\' {
            let mut next = i + 1;
            while matches!(source.get(next), Some(b' ' | b'\t')) {
                next += 1;
            }
            if source.get(next) == Some(&b'\r') {
                next += 1;
            }
            if source.get(next) == Some(&b'\n') {
                splices.push(u32::try_from(out.len()).unwrap_or(u32::MAX));
                i = next + 1;
                continue;
            }
        }
        out.push(source[i]);
        i += 1;
    }
    (std::borrow::Cow::Owned(out), splices)
}

struct Lexer<'a> {
    b: &'a [u8],
    at: usize,
    line: u32,
    /// The lines that a splice joined before `at`, which the line of a token adds.
    splices: &'a [u32],
    splice_index: usize,
    tokens: Vec<Token>,
    includes: Vec<(Form, String)>,
    macro_bodies: Vec<(String, Body)>,
    traps: Traps,
    /// The open conditionals: true for a group that `#if 0` makes dead.
    conditions: Vec<bool>,
}

impl Lexer<'_> {
    fn peek(&self, offset: usize) -> u8 {
        self.b.get(self.at + offset).copied().unwrap_or(0)
    }

    fn dead(&self) -> bool {
        self.conditions.iter().any(|dead| *dead)
    }

    /// The line of the byte at `at` in the lines of the original source. O(1) amortized, because the
    /// offsets only grow.
    fn original_line(&mut self) -> u32 {
        while self.splice_index < self.splices.len() && self.splices[self.splice_index] as usize <= self.at {
            self.splice_index += 1;
        }
        self.line + u32::try_from(self.splice_index).unwrap_or(u32::MAX)
    }

    fn newline_in(&mut self, from: usize, to: usize) {
        self.line += u32::try_from(self.b[from..to].iter().filter(|byte| **byte == b'\n').count()).unwrap_or(0);
    }

    /// Go past a block comment from its `/*`.
    fn skip_block_comment(&mut self) {
        let from = self.at;
        self.at += 2;
        while self.at < self.b.len() && !(self.b[self.at] == b'*' && self.peek(1) == b'/') {
            self.at += 1;
        }
        self.at = (self.at + 2).min(self.b.len());
        self.newline_in(from, self.at);
    }

    /// Go past a quoted literal from its quote. A literal with no closing quote ends at the line break.
    fn skip_quoted(&mut self, quote: u8) {
        self.at += 1;
        while self.at < self.b.len() {
            match self.b[self.at] {
                b'\n' => return,
                b'\\' => self.at += 2,
                byte if byte == quote => {
                    self.at += 1;
                    return;
                }
                _ => self.at += 1,
            }
        }
        self.at = self.at.min(self.b.len());
    }

    /// Go past a raw string literal from its `"`. A delimiter that [lex.string] p2 does not permit
    /// gives an ordinary literal.
    fn skip_raw_string(&mut self) {
        let open = self.at + 1;
        let delimiter_length = self.b[open..]
            .iter()
            .take(17)
            .position(|&byte| byte == b'(')
            .filter(|&length| {
                self.b[open..open + length].iter().all(|&byte| {
                    !matches!(
                        byte,
                        b' ' | b'(' | b')' | b'\\' | b'\t' | b'\x0b' | b'\x0c' | b'\n' | b'\r'
                    )
                })
            });
        let Some(length) = delimiter_length else {
            self.skip_quoted(b'"');
            return;
        };
        let delimiter = &self.b[open..open + length];
        let mut at = open + length + 1;
        while at < self.b.len() {
            if self.b[at] == b')'
                && self.b[at + 1..].starts_with(delimiter)
                && self.b.get(at + 1 + length) == Some(&b'"')
            {
                let from = self.at;
                self.at = at + length + 2;
                self.newline_in(from, self.at);
                return;
            }
            at += 1;
        }
        let from = self.at;
        self.at = self.b.len();
        self.newline_in(from, self.at);
    }

    fn skip_horizontal(&mut self) {
        while self.at < self.b.len() {
            match self.b[self.at] {
                b' ' | b'\t' | b'\r' | b'\x0b' | b'\x0c' => self.at += 1,
                b'/' if self.peek(1) == b'*' => self.skip_block_comment(),
                _ => return,
            }
        }
    }

    fn identifier(&mut self) -> &[u8] {
        let from = self.at;
        while self.at < self.b.len() && is_identifier_byte(self.b[self.at]) {
            self.at += 1;
        }
        &self.b[from..self.at]
    }

    /// Read a directive after its `#`, up to the line break that ends it.
    fn directive(&mut self) {
        self.skip_horizontal();
        let name = self.identifier().to_vec();
        match name.as_slice() {
            b"include" | b"include_next" | b"import" => {
                self.skip_horizontal();
                let (form, close) = match self.peek(0) {
                    b'<' => (Form::Angle, b'>'),
                    b'"' => (Form::Quote, b'"'),
                    _ => (Form::Angle, 0),
                };
                if close != 0 {
                    let from = self.at + 1;
                    let mut to = from;
                    while to < self.b.len() && self.b[to] != close && self.b[to] != b'\n' {
                        to += 1;
                    }
                    if to < self.b.len() && self.b[to] == close {
                        self.includes
                            .push((form, String::from_utf8_lossy(&self.b[from..to]).into_owned()));
                        self.at = to + 1;
                    }
                }
            }
            b"if" | b"elif" => {
                let zero = self.condition_is_zero();
                if name == b"if" {
                    self.conditions.push(zero);
                    self.traps.zero_groups += u32::from(zero);
                } else if let Some(top) = self.conditions.last_mut() {
                    *top = zero;
                }
            }
            b"ifdef" | b"ifndef" => self.conditions.push(false),
            b"else" | b"elifdef" | b"elifndef" => {
                if let Some(top) = self.conditions.last_mut() {
                    *top = false;
                }
            }
            b"endif" => {
                self.conditions.pop();
            }
            b"define" => {
                self.count_macro_body_class_keys();
                self.macro_body();
            }
            _ => {}
        }
        self.skip_directive_rest();
    }

    /// True when the text of an `#if` or `#elif` up to the line break is `0`, `(0)` or `false`.
    fn condition_is_zero(&mut self) -> bool {
        let mut text = Vec::new();
        let mut at = self.at;
        while at < self.b.len() && self.b[at] != b'\n' {
            if self.b[at] == b'/' && self.b.get(at + 1) == Some(&b'/') {
                break;
            }
            if self.b[at] == b'/' && self.b.get(at + 1) == Some(&b'*') {
                at += 2;
                while at < self.b.len() && !(self.b[at] == b'*' && self.b.get(at + 1) == Some(&b'/')) {
                    at += 1;
                }
                at += 2;
                continue;
            }
            if !self.b[at].is_ascii_whitespace() {
                text.push(self.b[at]);
            }
            at += 1;
        }
        matches!(text.as_slice(), b"0" | b"(0)" | b"false")
    }

    /// Record the class of the body of an object-like `#define`. The cursor does not move. O(n) in the
    /// bytes of the line.
    fn macro_body(&mut self) {
        let saved = self.at;
        let saved_line = self.line;
        self.skip_horizontal();
        let name = self.identifier().to_vec();
        if !name.is_empty()
            && self.peek(0) != b'('
            && let Ok(name) = String::from_utf8(name)
        {
            let body = self.body_class();
            self.macro_bodies.push((name, body));
        }
        self.at = saved;
        self.line = saved_line;
    }

    /// The class of the text from the cursor to the end of the directive line.
    ///
    /// ONE WORD IS A TAG WHEN IT HAS A LOWERCASE LETTER AND IS NO RESERVED NAME, AN ATTRIBUTE WHEN IT IS A
    /// RESERVED NAME WITH A LOWERCASE LETTER OR A SPECIFIER KEYWORD, AND AN ALIAS WHEN IT HAS NO LOWERCASE
    /// LETTER, because `#define CHAR8 UINT8` names a type. Sun's `#define BOOST_SYMBOL_VISIBLE __global` is
    /// an attribute and no tag. clang's `#define uint64_t __UINT64_TYPE__` names a type of the compiler, and
    /// an attribute class stripped `uint64_t` from every `uint64_t f(x)` of llvm-project. Two words or more are an attribute when each word is a reserved name, a specifier
    /// keyword or a macro-shaped name, and each group follows a word or is `[[...]]`.
    fn body_class(&mut self) -> Body {
        let mut words = 0;
        let mut attribute = true;
        let mut first_tag = false;
        let mut first_attribute = false;
        let mut first_word = Vec::new();
        loop {
            self.skip_horizontal();
            let byte = self.peek(0);
            if matches!(byte, b'\n' | 0) || (byte == b'/' && self.peek(1) == b'/') {
                break;
            }
            words += 1;
            if is_identifier_start(byte) {
                let word = self.identifier().to_vec();
                let reserved =
                    word.starts_with(b"__") || (word.len() > 1 && word[0] == b'_' && word[1].is_ascii_uppercase());
                let specifier = is_specifier(&word)
                    || is_group_specifier(&word)
                    || matches!(
                        word.as_slice(),
                        b"noexcept"
                            | b"explicit"
                            | b"virtual"
                            | b"friend"
                            | b"inline"
                            | b"static"
                            | b"constexpr"
                            | b"extern"
                            | b"thread_local"
                            | b"register"
                            | b"mutable"
                    );
                if words == 1 {
                    first_tag = word.iter().any(u8::is_ascii_lowercase) && !reserved && !is_keyword(&word);
                    first_attribute = reserved || specifier;
                    first_word = word.clone();
                }
                if !(reserved || specifier || is_macro_shaped(&word)) {
                    attribute = false;
                }
                self.skip_horizontal();
                if self.peek(0) == b'(' {
                    if !self.skip_line_group(b'(', b')') {
                        return Body::Other;
                    }
                    words += 1;
                }
                continue;
            }
            match byte {
                b'[' if self.peek(1) == b'[' => {
                    if !self.skip_line_group(b'[', b']') {
                        return Body::Other;
                    }
                }
                b'"' => self.skip_quoted(b'"'),
                _ => return Body::Other,
            }
        }
        match words {
            0 => Body::Attribute,
            1 if first_tag => Body::Tag,
            1 if first_attribute && !is_macro_shaped(&first_word) => Body::Attribute,
            1 if is_macro_shaped(&first_word) => Body::Alias(String::from_utf8_lossy(&first_word).into_owned()),
            1 => Body::Other,
            _ if attribute => Body::Attribute,
            _ => Body::Other,
        }
    }

    /// Go past a group from its opening byte to its matched closing byte, on the directive line. False
    /// when the line ends first.
    fn skip_line_group(&mut self, open: u8, close: u8) -> bool {
        let mut depth = 0usize;
        while self.at < self.b.len() {
            let byte = self.b[self.at];
            match byte {
                b'\n' => return false,
                b'"' | b'\'' => {
                    self.skip_quoted(byte);
                    continue;
                }
                _ if byte == open => depth += 1,
                _ if byte == close => {
                    depth -= 1;
                    if depth == 0 {
                        self.at += 1;
                        return true;
                    }
                }
                _ => {}
            }
            self.at += 1;
        }
        false
    }

    /// Count a class key and a name in the text of a `#define` line.
    fn count_macro_body_class_keys(&mut self) {
        let mut at = self.at;
        let mut previous_key = false;
        while at < self.b.len() && self.b[at] != b'\n' {
            if is_identifier_start(self.b[at]) {
                let from = at;
                while at < self.b.len() && is_identifier_byte(self.b[at]) {
                    at += 1;
                }
                let word = &self.b[from..at];
                if previous_key && !is_keyword(word) {
                    self.traps.macro_body_class_keys += 1;
                }
                previous_key = matches!(word, b"class" | b"struct" | b"union" | b"enum");
            } else {
                if !self.b[at].is_ascii_whitespace() {
                    previous_key = false;
                }
                at += 1;
            }
        }
    }

    /// Go past the rest of a directive line. A block comment that spans lines continues the directive.
    fn skip_directive_rest(&mut self) {
        while self.at < self.b.len() {
            match self.b[self.at] {
                b'\n' => return,
                b'/' if self.peek(1) == b'/' => {
                    while self.at < self.b.len() && self.b[self.at] != b'\n' {
                        self.at += 1;
                    }
                    return;
                }
                b'/' if self.peek(1) == b'*' => self.skip_block_comment(),
                b'"' | b'\'' => {
                    let quote = self.b[self.at];
                    self.skip_quoted(quote);
                }
                _ => self.at += 1,
            }
        }
    }

    fn push(&mut self, class: Class, start: usize, line: u32) {
        let dead = self.dead();
        self.tokens.push(Token {
            class,
            start: u32::try_from(start).unwrap_or(u32::MAX),
            end: u32::try_from(self.at).unwrap_or(u32::MAX),
            line,
            dead,
        });
    }

    fn punctuator(&mut self) -> u16 {
        let (a, b, c) = (self.peek(0), self.peek(1), self.peek(2));
        let (code, length) = match (a, b, c) {
            (b'.', b'.', b'.') => (ELLIPSIS, 3),
            (b'<', b'=', b'>') => (COMPARE, 3),
            (b'-', b'>', b'*') => (MEMBER_POINTER, 3),
            (b'<', b'<', b'=') => (ASSIGN_OPERATOR, 3),
            (b'>', b'>', b'=') => (ASSIGN_OPERATOR, 3),
            (b':', b':', _) => (SCOPE, 2),
            (b'-', b'>', _) => (ARROW, 2),
            (b'&', b'&', _) => (AND_AND, 2),
            (b'|', b'|', _) => (OR_OR, 2),
            (b'<', b'<', _) => (SHIFT_LEFT, 2),
            (b'=' | b'!' | b'<' | b'>', b'=', _) => (COMPARE, 2),
            (b'+', b'+', _) | (b'-', b'-', _) => (INCREMENT, 2),
            (b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^', b'=', _) => (ASSIGN_OPERATOR, 2),
            (b'.', b'*', _) => (MEMBER_POINTER, 2),
            (b'#', b'#', _) => (PASTE, 2),
            // `>>` is two tokens, so that it closes two template argument lists.
            _ => (u16::from(a), 1),
        };
        self.at += length;
        code
    }

    fn run(mut self) -> Lexed {
        let mut line_start = true;
        while self.at < self.b.len() {
            let byte = self.b[self.at];
            match byte {
                b'\n' => {
                    self.line += 1;
                    line_start = true;
                    self.at += 1;
                }
                b' ' | b'\t' | b'\r' | b'\x0b' | b'\x0c' | 0 => self.at += 1,
                b'/' if self.peek(1) == b'/' => {
                    while self.at < self.b.len() && self.b[self.at] != b'\n' {
                        self.at += 1;
                    }
                }
                b'/' if self.peek(1) == b'*' => self.skip_block_comment(),
                b'#' if line_start => {
                    self.at += 1;
                    line_start = false;
                    self.directive();
                }
                b'%' if line_start && self.peek(1) == b':' => {
                    self.at += 2;
                    line_start = false;
                    self.directive();
                }
                _ => {
                    line_start = false;
                    let start = self.at;
                    let line = self.original_line();
                    if byte == b'"' || byte == b'\'' {
                        self.skip_quoted(byte);
                        self.push(Class::Literal, start, line);
                    } else if byte.is_ascii_digit() || (byte == b'.' && self.peek(1).is_ascii_digit()) {
                        self.skip_number();
                        self.push(Class::Number, start, line);
                    } else if is_identifier_start(byte) {
                        let word = self.identifier();
                        let raw = matches!(word, b"R" | b"LR" | b"uR" | b"UR" | b"u8R");
                        let prefix = matches!(word, b"L" | b"u" | b"U" | b"u8");
                        if raw && self.peek(0) == b'"' {
                            self.skip_raw_string();
                            self.push(Class::Literal, start, line);
                        } else if prefix && matches!(self.peek(0), b'"' | b'\'') {
                            let quote = self.peek(0);
                            self.skip_quoted(quote);
                            self.push(Class::Literal, start, line);
                        } else {
                            self.push(Class::Word, start, line);
                        }
                    } else {
                        let code = self.punctuator();
                        self.push(Class::Punct(code), start, line);
                    }
                }
            }
        }
        Lexed {
            tokens: self.tokens,
            includes: self.includes,
            macro_bodies: self.macro_bodies,
            traps: self.traps,
        }
    }

    /// Go past a preprocessing number. A `'` between two digits is a digit separator.
    fn skip_number(&mut self) {
        self.at += 1;
        while self.at < self.b.len() {
            match self.b[self.at] {
                b'e' | b'E' | b'p' | b'P' if matches!(self.peek(1), b'+' | b'-') => self.at += 2,
                b'\'' if is_identifier_byte(self.peek(1)) => self.at += 2,
                b'.' => self.at += 1,
                byte if is_identifier_byte(byte) => self.at += 1,
                _ => return,
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The words.
// ---------------------------------------------------------------------------------------------------

/// The keywords of C and C++ and the reserved words of the compilers that can never name a declared
/// type or a declarator.
fn is_keyword(word: &[u8]) -> bool {
    matches!(
        word,
        b"alignas"
            | b"alignof"
            | b"and"
            | b"and_eq"
            | b"asm"
            | b"auto"
            | b"bitand"
            | b"bitor"
            | b"bool"
            | b"break"
            | b"case"
            | b"catch"
            | b"char"
            | b"char8_t"
            | b"char16_t"
            | b"char32_t"
            | b"class"
            | b"compl"
            | b"concept"
            | b"const"
            | b"consteval"
            | b"constexpr"
            | b"constinit"
            | b"const_cast"
            | b"continue"
            | b"co_await"
            | b"co_return"
            | b"co_yield"
            | b"decltype"
            | b"default"
            | b"delete"
            | b"do"
            | b"double"
            | b"dynamic_cast"
            | b"else"
            | b"enum"
            | b"explicit"
            | b"export"
            | b"extern"
            | b"false"
            | b"float"
            | b"for"
            | b"friend"
            | b"goto"
            | b"if"
            | b"inline"
            | b"int"
            | b"long"
            | b"mutable"
            | b"namespace"
            | b"new"
            | b"noexcept"
            | b"not"
            | b"not_eq"
            | b"nullptr"
            | b"operator"
            | b"or"
            | b"or_eq"
            | b"private"
            | b"protected"
            | b"public"
            | b"register"
            | b"reinterpret_cast"
            | b"requires"
            | b"return"
            | b"short"
            | b"signed"
            | b"sizeof"
            | b"static"
            | b"static_assert"
            | b"static_cast"
            | b"struct"
            | b"switch"
            | b"template"
            | b"this"
            | b"thread_local"
            | b"throw"
            | b"true"
            | b"try"
            | b"typedef"
            | b"typeid"
            | b"typename"
            | b"union"
            | b"unsigned"
            | b"using"
            | b"virtual"
            | b"void"
            | b"volatile"
            | b"wchar_t"
            | b"while"
            | b"xor"
            | b"xor_eq"
            | b"restrict"
            | b"_Alignas"
            | b"_Alignof"
            | b"_Atomic"
            | b"_Bool"
            | b"_Complex"
            | b"_Generic"
            | b"_Imaginary"
            | b"_Noreturn"
            | b"_Static_assert"
            | b"_Thread_local"
            | b"__attribute__"
            | b"__attribute"
            | b"__declspec"
            | b"__extension__"
            | b"__inline"
            | b"__inline__"
            | b"__restrict"
            | b"__restrict__"
            | b"__typeof__"
            | b"__typeof"
            | b"typeof"
            | b"__int8"
            | b"__int16"
            | b"__int32"
            | b"__int64"
            | b"__int128"
            | b"__asm__"
            | b"__asm"
            | b"__const"
            | b"__volatile__"
            | b"__signed__"
            | b"__cdecl"
            | b"__stdcall"
            | b"__fastcall"
            | b"__thiscall"
            | b"__vectorcall"
            | b"__ptr32"
            | b"__ptr64"
            | b"__w64"
            | b"_Nonnull"
            | b"_Nullable"
            | b"_Null_unspecified"
            | b"__underlying_type"
            | b"__thread"
    )
}

/// A keyword that names a type or a part of one.
fn is_builtin_type(word: &[u8]) -> bool {
    matches!(
        word,
        b"auto"
            | b"bool"
            | b"char"
            | b"char8_t"
            | b"char16_t"
            | b"char32_t"
            | b"double"
            | b"float"
            | b"int"
            | b"long"
            | b"short"
            | b"signed"
            | b"unsigned"
            | b"void"
            | b"wchar_t"
            | b"_Bool"
            | b"_Complex"
            | b"__int8"
            | b"__int16"
            | b"__int32"
            | b"__int64"
            | b"__int128"
            | b"__signed__"
    )
}

/// A keyword that can come before the type of a declaration, or between its type and its declarator.
fn is_specifier(word: &[u8]) -> bool {
    matches!(
        word,
        b"static"
            | b"extern"
            | b"inline"
            | b"constexpr"
            | b"constinit"
            | b"consteval"
            | b"const"
            | b"volatile"
            | b"virtual"
            | b"explicit"
            | b"friend"
            | b"mutable"
            | b"register"
            | b"thread_local"
            | b"_Thread_local"
            | b"__thread"
            | b"export"
            | b"__extension__"
            | b"_Noreturn"
            | b"__inline"
            | b"__inline__"
            | b"restrict"
            | b"__restrict"
            | b"__restrict__"
            | b"_Atomic"
            | b"__const"
            | b"__volatile__"
            | b"__cdecl"
            | b"__stdcall"
            | b"__fastcall"
            | b"__thiscall"
            | b"__vectorcall"
            | b"__ptr32"
            | b"__ptr64"
            | b"__w64"
            | b"_Nonnull"
            | b"_Nullable"
            | b"_Null_unspecified"
    )
}

/// A keyword that takes a parenthesized group and that can stand among the specifiers.
fn is_group_specifier(word: &[u8]) -> bool {
    matches!(
        word,
        b"__attribute__" | b"__attribute" | b"__declspec" | b"alignas" | b"_Alignas" | b"__asm__" | b"__asm" | b"asm"
    )
}

/// A keyword that gives a type from a parenthesized group.
fn is_type_operator(word: &[u8]) -> bool {
    matches!(
        word,
        b"decltype" | b"typeof" | b"__typeof__" | b"__typeof" | b"__underlying_type" | b"_Atomic"
    )
}

/// A concept of the standard library, [concepts] and [range.range] and [iterator.concepts].
fn is_standard_concept(word: &[u8]) -> bool {
    matches!(
        word,
        b"same_as"
            | b"derived_from"
            | b"convertible_to"
            | b"common_reference_with"
            | b"common_with"
            | b"integral"
            | b"signed_integral"
            | b"unsigned_integral"
            | b"floating_point"
            | b"assignable_from"
            | b"swappable"
            | b"swappable_with"
            | b"destructible"
            | b"constructible_from"
            | b"default_initializable"
            | b"move_constructible"
            | b"copy_constructible"
            | b"equality_comparable"
            | b"totally_ordered"
            | b"movable"
            | b"copyable"
            | b"semiregular"
            | b"regular"
            | b"invocable"
            | b"regular_invocable"
            | b"predicate"
            | b"relation"
            | b"equivalence_relation"
            | b"strict_weak_order"
            | b"range"
            | b"borrowed_range"
            | b"sized_range"
            | b"view"
            | b"input_range"
            | b"output_range"
            | b"forward_range"
            | b"bidirectional_range"
            | b"random_access_range"
            | b"contiguous_range"
            | b"common_range"
            | b"viewable_range"
            | b"constant_range"
            | b"input_iterator"
            | b"output_iterator"
            | b"forward_iterator"
            | b"bidirectional_iterator"
            | b"random_access_iterator"
            | b"contiguous_iterator"
            | b"sentinel_for"
            | b"sized_sentinel_for"
            | b"indirectly_readable"
            | b"weakly_incrementable"
            | b"incrementable"
            | b"input_or_output_iterator"
    )
}

/// A word with an uppercase letter and no lowercase letter, of two characters or more.
fn is_macro_shaped(word: &[u8]) -> bool {
    word.len() >= 2 && word.iter().any(u8::is_ascii_uppercase) && !word.iter().any(u8::is_ascii_lowercase)
}

// ---------------------------------------------------------------------------------------------------
// The reader.
// ---------------------------------------------------------------------------------------------------

/// Where a declaration stands, which decides the tokens that end a declarator.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    /// A statement, a member, or an item of a namespace.
    Statement,
    /// An item of a parameter list, which ends at its end.
    Parameter,
    /// An item of a template parameter list.
    TemplateParameter,
    /// The declaration of a condition or of the head of a `for`: `for (auto x : v)`, `if (T x = f())`.
    Condition,
}

/// A qualified name at a token: `a`, `a::b`, `a<T>::b<U>`, `a::~b`, `a::operator`.
struct Chain {
    /// The token after the name.
    next: usize,
    /// The token of the last name.
    leaf: usize,
    /// The token of the name before the last one, for `A::A`.
    previous: Option<usize>,
    /// The count of names.
    parts: usize,
    /// True when a template argument list follows a name.
    arguments: bool,
    destructor: bool,
    operator: bool,
}

struct Reader<'a> {
    b: &'a [u8],
    t: Vec<Token>,
    /// The token that closes or opens the bracket at each index, or `NONE`.
    partner: Vec<u32>,
    /// True for a token whose innermost matched bracket is a parenthesis or a square bracket.
    in_group: Vec<bool>,
    /// True for a token inside the parameter list of a template head.
    in_template_head: Vec<bool>,
    /// True for the token after the `>` of a template head.
    after_template_head: Vec<bool>,
    /// True for the `>` of a template head.
    template_close: Vec<bool>,
    /// True for a token where a declaration can start.
    starts: Vec<bool>,
    /// True for a `{` that opens the body of a class head.
    class_braces: Vec<bool>,
    /// The class names of the class head that each `{` opens.
    class_bodies: BTreeMap<usize, Vec<String>>,
    /// The open braces, with the names of their class head.
    scopes: Vec<(usize, Vec<String>, bool, bool)>,
    facts: FileFacts,
    c_file: bool,
    /// The nesting of parameter lists that the reader is in. A list deeper than `MAX_DEPTH` is not
    /// read, so that generated code with deep nesting cannot exhaust the stack of a thread.
    depth: usize,
}

/// The deepest parameter list that the reader reads.
const MAX_DEPTH: usize = 32;

const NONE: u32 = u32::MAX;

impl Reader<'_> {
    fn text(&self, i: usize) -> &[u8] {
        let token = self.t[i];
        &self.b[token.start as usize..token.end as usize]
    }

    fn string(&self, i: usize) -> String {
        String::from_utf8_lossy(self.text(i)).into_owned()
    }

    fn is_word(&self, i: usize) -> bool {
        i < self.t.len() && self.t[i].class == Class::Word
    }

    fn is_name(&self, i: usize) -> bool {
        self.is_word(i) && !is_keyword(self.text(i))
    }

    fn word_is(&self, i: usize, word: &[u8]) -> bool {
        self.is_word(i) && self.text(i) == word
    }

    fn punct(&self, i: usize) -> Option<u16> {
        match self.t.get(i)?.class {
            Class::Punct(code) => Some(code),
            _ => None,
        }
    }

    fn is(&self, i: usize, code: u16) -> bool {
        self.punct(i) == Some(code)
    }

    fn partner_of(&self, i: usize) -> Option<usize> {
        let partner = *self.partner.get(i)?;
        (partner != NONE).then_some(partner as usize)
    }

    /// Match the brackets `(`, `[` and `{`. An unmatched close matches the nearest open bracket of its
    /// kind and leaves the brackets between unmatched. O(n) in the tokens.
    fn match_brackets(&mut self) {
        let mut stack: Vec<usize> = Vec::new();
        for i in 0..self.t.len() {
            let Some(code) = self.punct(i) else { continue };
            let open = match code {
                0x29 => u16::from(b'('),
                0x5d => u16::from(b'['),
                0x7d => u16::from(b'{'),
                0x28 | 0x5b | 0x7b => {
                    stack.push(i);
                    continue;
                }
                _ => continue,
            };
            if let Some(depth) = stack.iter().rposition(|&opener| self.punct(opener) == Some(open)) {
                let opener = stack[depth];
                stack.truncate(depth);
                self.partner[opener] = u32::try_from(i).unwrap_or(NONE);
                self.partner[i] = u32::try_from(opener).unwrap_or(NONE);
            }
        }
        // THE INNERMOST MATCHED BRACKET OF EACH TOKEN. An unmatched bracket is left out, because an
        // `#if` branch can open a group that the other branch closes, and a bracket that is never closed
        // must not hide every statement after it.
        let mut open: Vec<usize> = Vec::new();
        for i in 0..self.t.len() {
            if matches!(self.punct(i), Some(0x29 | 0x5d | 0x7d))
                && let Some(opener) = self.partner_of(i)
                && let Some(depth) = open.iter().rposition(|&index| index == opener)
            {
                open.truncate(depth);
            }
            self.in_group[i] = open.last().is_some_and(|&opener| !self.is(opener, 0x7b));
            if matches!(self.punct(i), Some(0x28 | 0x5b | 0x7b)) && self.partner_of(i).is_some() {
                open.push(i);
            }
        }
    }

    /// The `>` that closes the template argument list opened at `open`, or None when the `<` is a
    /// less-than. A statement end, an unmatched close, and a logical operator that no `>` or `,`
    /// follows end the search.
    fn angle_close(&self, open: usize) -> Option<usize> {
        self.angle_close_with(open, false)
    }

    /// `angle_close`, and with `braces` a brace group is part of the list: the default argument of a
    /// template head, `template <class T, T V = T{'.'}>`. An expression keeps the brace as an end,
    /// because `f(a < b, [] { }, c > d)` holds no template argument list.
    fn angle_close_with(&self, open: usize, braces: bool) -> Option<usize> {
        let mut depth = 0usize;
        let mut j = open;
        while j < self.t.len() {
            match self.punct(j) {
                Some(0x3c) => depth += 1,
                Some(0x3e) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                Some(0x28 | 0x5b) => j = self.partner_of(j)?,
                Some(0x7b) if braces => j = self.partner_of(j)?,
                Some(0x3b | 0x7b | 0x7d | 0x29 | 0x5d | 0x3f | COMPARE) => return None,
                Some(AND_AND | OR_OR) if !matches!(self.punct(j + 1), Some(0x3e | 0x2c | 0x29)) => return None,
                _ => {}
            }
            j += 1;
        }
        None
    }

    /// The `>` that closes a template argument list in a type position opened at `open`. A logical
    /// operator, a comparison, a `?` and a brace group (`T{'.'}`) can stand in the list there, and only a
    /// `;`, an unmatched close or the end of the source ends the search.
    fn type_arguments_close(&self, open: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut j = open;
        while j < self.t.len() {
            match self.punct(j) {
                Some(0x3c) => depth += 1,
                Some(0x3e) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                Some(0x28 | 0x5b | 0x7b) => j = self.partner_of(j)?,
                Some(0x3b | 0x7d | 0x29 | 0x5d) => return None,
                _ => {}
            }
            j += 1;
        }
        None
    }

    /// The `>` that closes the template argument list of a base clause opened at `open`. A brace is
    /// part of the list here, and a `;` or an unmatched close ends the search.
    fn template_arguments_close(&self, open: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut j = open;
        while j < self.t.len() {
            match self.punct(j) {
                Some(0x3c) => depth += 1,
                Some(0x3e) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                Some(0x28 | 0x5b | 0x7b) => j = self.partner_of(j)?,
                Some(0x3b | 0x7d | 0x29 | 0x5d) => return None,
                _ => {}
            }
            j += 1;
        }
        None
    }

    /// The token after an attribute group at `i`: `[[...]]`, `__attribute__((...))`, `alignas(...)`.
    fn after_attribute(&self, i: usize) -> Option<usize> {
        if self.is(i, 0x5b) && self.is(i + 1, 0x5b) {
            return self.partner_of(i).map(|close| close + 1);
        }
        if self.is_word(i) && is_group_specifier(self.text(i)) {
            if self.is(i + 1, 0x28) {
                return self.partner_of(i + 1).map(|close| close + 1);
            }
            return Some(i + 1);
        }
        None
    }

    fn skip_attributes(&self, mut i: usize) -> usize {
        while let Some(next) = self.after_attribute(i) {
            i = next;
        }
        i
    }

    /// Read a qualified name at `k`. O(n) in its tokens.
    fn chain(&self, k: usize) -> Chain {
        let mut chain = Chain {
            next: k,
            leaf: k,
            previous: None,
            parts: 0,
            arguments: false,
            destructor: false,
            operator: false,
        };
        let mut j = k;
        loop {
            if self.word_is(j, b"template") {
                j += 1;
            }
            if self.is(j, u16::from(b'~')) {
                chain.destructor = true;
                j += 1;
            }
            if self.word_is(j, b"operator") {
                chain.operator = true;
                chain.next = j;
                return chain;
            }
            if !self.is_name(j) {
                break;
            }
            if chain.parts > 0 {
                chain.previous = Some(chain.leaf);
            }
            chain.leaf = j;
            chain.parts += 1;
            j += 1;
            if self.is(j, 0x3c)
                && let Some(close) = self.angle_close(j)
            {
                chain.arguments = true;
                j = close + 1;
            }
            if self.is(j, SCOPE) && (self.is_name(j + 1) || self.is(j + 1, u16::from(b'~')) || self.is_word(j + 1)) {
                j += 1;
                continue;
            }
            break;
        }
        chain.next = j;
        chain
    }

    /// `REFERENCE` when a `&` or a `&&` stands before the declarator that starts at `start`, past the
    /// cv-qualifiers, the attributes and the macro-shaped words, and 0 otherwise.
    fn reference_before(&self, start: usize) -> u32 {
        let mut j = start;
        while j > 0 {
            j -= 1;
            if self.is_word(j) && (is_specifier(self.text(j)) || is_macro_shaped(self.text(j))) {
                continue;
            }
            return if matches!(self.punct(j), Some(0x26 | AND_AND)) {
                REFERENCE
            } else {
                0
            };
        }
        0
    }

    fn add(&mut self, i: usize, bits: u32) {
        let token = self.t[i];
        let name = self.string(i);
        let bits = self.scoped(i, bits);
        self.facts.add(&name, bits, token.dead, token.line);
    }

    /// The bits with `OUTER` and `NAMESPACE` for an object or a function whose declarator name is at `i`,
    /// at the scope of the statement that the reader reads. `OUTER`: each brace of the scope stack opens a
    /// namespace, a linkage specification or a class body. `NAMESPACE`: each brace opens a namespace or a
    /// linkage specification, and no `::` stands before the name.
    fn scoped(&self, i: usize, bits: u32) -> u32 {
        if bits & (OBJECT | FUNCTION) == 0 {
            return bits;
        }
        let mut bits = bits;
        if self.scopes.iter().all(|(_, _, outer, _)| *outer) {
            bits |= OUTER;
        }
        if self.scopes.iter().all(|(_, _, _, namespace)| *namespace) && !(i > 0 && self.is(i - 1, SCOPE)) {
            bits |= NAMESPACE;
        }
        bits
    }

    /// True for a `{` that opens a namespace or a linkage specification: `namespace a::b {`,
    /// `inline namespace v1 {`, `namespace {`, `extern "C" {`.
    fn namespace_brace(&self, open: usize) -> bool {
        if open == 0 {
            return false;
        }
        let before = open - 1;
        if self.t[before].class == Class::Literal {
            return before > 0 && self.word_is(before - 1, b"extern");
        }
        let mut j = before;
        while j > 0 && (self.is_name(j) || self.is(j, SCOPE)) {
            j -= 1;
        }
        self.word_is(j, b"namespace")
    }

    /// True when the token at `i` is after a template head, past `friend`, `export`, attributes, a
    /// macro-shaped word and a requires clause.
    fn under_template(&self, i: usize) -> bool {
        let mut j = i;
        let mut steps = 0;
        while j > 0 && steps < 64 {
            steps += 1;
            let before = j - 1;
            if self.template_close[before] {
                return true;
            }
            if self.is(before, 0x5d) || self.is(before, 0x29) {
                match self.partner_of(before) {
                    Some(open) => {
                        j = open;
                        continue;
                    }
                    None => return false,
                }
            }
            if self.is_word(before) {
                let word = self.text(before);
                if matches!(word, b"friend" | b"export" | b"inline" | b"requires") || is_macro_shaped(word) {
                    j = before;
                    continue;
                }
                // The words of a requires clause: `requires std::integral<T>`.
                if self.find_requires(before) {
                    j = before;
                    continue;
                }
                return false;
            }
            if matches!(self.punct(before), Some(SCOPE | AND_AND | OR_OR | 0x3e | 0x3c | 0x21))
                && self.find_requires(before)
            {
                j = before;
                continue;
            }
            return false;
        }
        false
    }

    /// True when a `requires` keyword stands before `i` in the same declaration.
    fn find_requires(&self, i: usize) -> bool {
        let mut j = i;
        for _ in 0..32 {
            if j == 0 {
                return false;
            }
            j -= 1;
            if self.word_is(j, b"requires") {
                return true;
            }
            if matches!(self.punct(j), Some(0x3b | 0x7b | 0x7d)) {
                return false;
            }
        }
        false
    }

    /// True when the class key at `i` is part of a typedef declaration at the same brace level.
    fn in_typedef(&self, i: usize) -> bool {
        let mut j = i;
        while j > 0 {
            j -= 1;
            if self.word_is(j, b"typedef") {
                return true;
            }
            match self.punct(j) {
                Some(0x3b | 0x7b | 0x7d) => return false,
                Some(0x29 | 0x5d) => match self.partner_of(j) {
                    Some(open) => j = open,
                    None => return false,
                },
                _ => {}
            }
        }
        false
    }

    // -------------------------------------------------------------------------------------------
    // The type positions.
    // -------------------------------------------------------------------------------------------

    /// Read a class key at `i`. Give the token after the head and the body, for the reader of a
    /// declaration.
    fn class_key(&mut self, i: usize, record: bool) -> usize {
        let mut j = i + 1;
        if self.word_is(i, b"enum") && (self.word_is(j, b"class") || self.word_is(j, b"struct")) {
            j += 1;
        }
        if i > 0 && matches!(self.punct(i - 1), Some(0x2e | ARROW | SCOPE)) {
            return j;
        }
        if self.in_template_head[i] {
            if record {
                self.facts.traps.template_parameter_keys += 1;
            }
            return j;
        }
        j = self.skip_attributes(j);
        // `struct ::statvfs sfs;`: a name from the global namespace.
        if self.is(j, SCOPE) && self.is_name(j + 1) {
            j += 1;
        }
        let mut words: Vec<usize> = Vec::new();
        while self.is_name(j) {
            let word = self.text(j);
            if matches!(word, b"final" | b"sealed" | b"__final")
                && !words.is_empty()
                && (self.is(j + 1, u16::from(b'{')) || self.is(j + 1, u16::from(b':')))
            {
                j += 1;
                break;
            }
            let chain = self.chain(j);
            if chain.operator || chain.destructor || chain.parts == 0 {
                break;
            }
            // A macro with arguments in a class head is an attribute and no name:
            // `struct DECLSPEC_UUID("B41463C3-...") ISetupInstance`.
            // It is the first word, and its group holds arguments: `struct mallinfo mallinfo() __THROW {` is a
            // function. After macro-shaped words only, a macro-shaped call before a name with a lowercase
            // letter is one too: `class Q_CORE_EXPORT QT_DEPRECATED_VERSION_X_6_15("...") QVariantConstPointer`.
            let after_group = self.partner_of(chain.next).map_or(0, |close| close + 1);
            if (words.is_empty()
                || (words.iter().all(|&w| is_macro_shaped(self.text(w)))
                    && is_macro_shaped(self.text(chain.leaf))
                    && self.is_word(after_group)
                    && !is_macro_shaped(self.text(after_group))))
                && chain.parts == 1
                && !chain.arguments
                && self.is(chain.next, 0x28)
                && !self.is(chain.next + 1, 0x29)
                && self.is_name(after_group)
            {
                j = self.skip_attributes(self.partner_of(chain.next).map_or(chain.next, |close| close + 1));
                continue;
            }
            words.push(chain.leaf);
            j = self.skip_attributes(chain.next);
        }
        let follow_at = j;
        let follow = match self.punct(follow_at) {
            Some(0x7b) => Follow::Body,
            Some(0x3a) => {
                // A base clause or the underlying type of an enum: a body follows unless a `;` comes first.
                // The template arguments of a base can hold a brace: `named_constant<symbol_text{u8"m_t"}>`.
                let mut k = follow_at + 1;
                let mut body = None;
                while k < self.t.len() {
                    match self.punct(k) {
                        Some(0x7b) => {
                            body = Some(k);
                            break;
                        }
                        Some(0x3b | 0x7d) => break,
                        Some(0x28 | 0x5b) => k = self.partner_of(k).unwrap_or(k),
                        Some(0x3c) if self.is_name(k - 1) => {
                            if let Some(close) = self.template_arguments_close(k) {
                                k = close;
                            }
                        }
                        _ => {}
                    }
                    k += 1;
                }
                match body {
                    Some(brace) => {
                        j = brace;
                        Follow::Body
                    }
                    None => Follow::Semicolon,
                }
            }
            Some(0x3b) => Follow::Semicolon,
            Some(0x2a | 0x26 | AND_AND | 0x5e) => Follow::Pointer,
            Some(0x28) => Follow::Call,
            None if self.word_is(follow_at, b"const") || self.word_is(follow_at, b"volatile") => Follow::Pointer,
            _ => Follow::Other,
        };
        let after = if follow == Follow::Body {
            let brace = if self.is(j, u16::from(b'{')) { j } else { follow_at };
            self.partner_of(brace).map_or(brace + 1, |close| close + 1)
        } else {
            follow_at
        };
        if words.is_empty() || !record {
            if follow == Follow::Body && record {
                let brace = if self.is(j, u16::from(b'{')) { j } else { follow_at };
                self.class_bodies.insert(brace, Vec::new());
                self.class_braces[brace] = true;
            }
            return after;
        }
        if self.c_file && self.word_is(i, b"class") {
            self.facts.traps.c_file_class_keys += 1;
        }
        let template = self.under_template(i);
        let names: Vec<String> = words.iter().map(|&w| self.string(w)).collect();
        if follow == Follow::Body {
            let brace = if self.is(j, u16::from(b'{')) { j } else { follow_at };
            self.class_bodies.insert(brace, names.clone());
            self.class_braces[brace] = true;
        }
        let token = self.t[words[0]];
        // A class key after `friend`, past the attributes and the macro-shaped words.
        let friend = {
            let mut k = i;
            while k > 0 && self.is_word(k - 1) && is_macro_shaped(self.text(k - 1)) {
                k -= 1;
            }
            k > 0 && self.word_is(k - 1, b"friend")
        };
        // A TEMPLATE TEMPLATE PARAMETER AFTER A HEAD OUTSIDE A TEMPLATE PARAMETER LIST: range-v3 writes
        // `template(template<typename...> class ContT, typename Rng)` with a macro named `template`.
        let parameter = i > 0
            && self.template_close[i - 1]
            && words.len() == 1
            && matches!(self.punct(follow_at), Some(0x2c | 0x3e | 0x29 | 0x3d));
        if parameter {
            self.add(words[0], TYPE_PARAMETER);
            return after;
        }
        // `struct RANGES_STRUCT_WITH_ADL_BARRIER(view_closure_base)`: a macro call names the class.
        if words.len() == 1 && follow == Follow::Call && is_macro_shaped(self.text(words[0])) {
            return after;
        }
        if words.len() == 1 {
            let bits = match follow {
                Follow::Body => TYPE,
                _ => TYPE_USE,
            } | if template { TEMPLATE } else { 0 }
                | if friend { FRIEND } else { 0 };
            self.add(words[0], bits);
        } else {
            let typedef = self.in_typedef(i);
            let initializer = follow == Follow::Body && {
                let brace = if self.is(j, u16::from(b'{')) { j } else { follow_at };
                let close = self.partner_of(brace).unwrap_or(brace);
                !(brace..close).any(|index| self.is(index, 0x3b))
            };
            self.facts.heads.push(Head::Class {
                friend,
                words: names,
                follow,
                template,
                typedef,
                dead: token.dead,
                line: token.line,
                initializer,
                scope: self.scoped(*words.last().expect("a head has two words or more"), OBJECT) & (OUTER | NAMESPACE),
            });
        }
        after
    }

    /// Read a typedef at `i`: each declarator of its list gives a type.
    fn typedef(&mut self, i: usize) {
        if i > 0 && matches!(self.punct(i - 1), Some(0x2e | ARROW)) {
            return;
        }
        let mut end = i + 1;
        while end < self.t.len() {
            match self.punct(end) {
                Some(0x3b) => break,
                Some(0x7d | 0x29 | 0x5d) => return,
                Some(0x28 | 0x5b | 0x7b) => match self.partner_of(end) {
                    Some(close) => end = close,
                    None => return,
                },
                _ => {}
            }
            end += 1;
        }
        if end >= self.t.len() {
            return;
        }
        for (from, to) in self.split_with(i + 1, end, 0x2c, true) {
            self.typedef_declarator(from, to);
        }
    }

    /// Split the tokens `[from, to)` at each top-level separator. Template argument lists are skipped.
    fn split(&self, from: usize, to: usize, separator: u16) -> Vec<(usize, usize)> {
        self.split_with(from, to, separator, false)
    }

    /// `split`, and with `types` the template argument lists are matched as types: a typedef and a
    /// template head. `typedef std::conditional_t<A == B || C == D, X, Y> T;` holds one declarator.
    fn split_with(&self, from: usize, to: usize, separator: u16, types: bool) -> Vec<(usize, usize)> {
        let mut parts = Vec::new();
        let mut start = from;
        let mut j = from;
        while j < to {
            match self.punct(j) {
                Some(code) if code == separator => {
                    parts.push((start, j));
                    start = j + 1;
                }
                Some(0x28 | 0x5b | 0x7b) => {
                    if let Some(close) = self.partner_of(j).filter(|close| *close < to) {
                        j = close;
                    }
                }
                // In a type, a `<` after a `>` opens a list too: the union of two `#if` branches gives
                // `pack_options < hook_defaults, O1 > < hook_defaults, Options... > ::type`.
                Some(0x3c)
                    if j > from
                        && (self.is_name(j - 1)
                            || self.word_is(j - 1, b"template")
                            || (types && self.is(j - 1, 0x3e))) =>
                {
                    let close = if types {
                        self.type_arguments_close(j)
                    } else {
                        self.angle_close(j)
                    };
                    if let Some(close) = close.filter(|close| *close < to) {
                        j = close;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        parts.push((start, to));
        parts
    }

    /// True when the tokens `[from, to)` start a declarator group: `*`, `&`, `^`, a calling convention
    /// or a macro word and then a pointer, or `Class::*`.
    fn starts_declarator_group(&self, from: usize, to: usize) -> bool {
        let mut j = self.skip_attributes(from);
        while j < to && self.is_word(j) && (is_specifier(self.text(j)) || is_macro_shaped(self.text(j))) {
            j = self.skip_attributes(j + 1);
        }
        if matches!(self.punct(j), Some(0x2a | 0x26 | AND_AND | 0x5e)) {
            return true;
        }
        // `Class::*name` and `ns::Class::*name`.
        let mut k = j;
        while k + 1 < to && self.is_name(k) && self.is(k + 1, SCOPE) {
            k += 2;
            if self.is(k, 0x2a) {
                return true;
            }
        }
        false
    }

    /// The name of a typedef declarator in `[from, to)`, read from its right end.
    fn typedef_declarator(&mut self, mut from: usize, mut to: usize) {
        let mut after_suffix = false;
        for _ in 0..64 {
            // Trailing attributes and qualifiers.
            loop {
                if to <= from {
                    return;
                }
                let last = to - 1;
                // `__attribute__((x))` after the declarator, and `noexcept(false)` or `throw()` after a
                // parameter list: `typedef int foo6_t(double) noexcept(false);`.
                if self.is(last, 0x29)
                    && let Some(open) = self.partner_of(last)
                    && open > from
                    && self.is_word(open - 1)
                    && (is_group_specifier(self.text(open - 1)) || matches!(self.text(open - 1), b"noexcept" | b"throw"))
                {
                    to = open - 1;
                    continue;
                }
                // An attribute call after a parameter list and its qualifiers:
                // `typedef void (*sz_ptrty) (int, int) __arm_inout("za");`.
                if self.is(last, 0x29)
                    && let Some(open) = self.partner_of(last)
                    && open > from + 1
                    && self.is_word(open - 1)
                    && !is_keyword(self.text(open - 1))
                    && (self.is(open - 2, 0x29)
                        || self.word_is(open - 2, b"const")
                        || self.word_is(open - 2, b"volatile")
                        || matches!(self.punct(open - 2), Some(0x26 | AND_AND)))
                {
                    to = open - 1;
                    continue;
                }
                if self.is(last, 0x5d)
                    && let Some(open) = self.partner_of(last)
                    && self.is(open + 1, 0x5b)
                {
                    to = open;
                    continue;
                }
                if self.is_word(last)
                    && (is_specifier(self.text(last))
                        || matches!(self.text(last), b"noexcept" | b"override" | b"final"))
                {
                    to = last;
                    continue;
                }
                // A macro-shaped word after a parameter list or an array bound:
                // `typedef R (BOOST_BIND_CC *F) () BOOST_BIND_NOEXCEPT;` and
                // `typedef char must_be_a_complete_type[sizeof(T)] BOOST_ATTRIBUTE_UNUSED;`. After the group of a
                // type operator or of an attribute the word is the declarator: `typedef decltype(ds1) DS1;` and
                // `typedef _Float16 __attribute__((vector_size(16))) F16;`.
                if self.is_word(last)
                    && is_macro_shaped(self.text(last))
                    && last > from
                    && (self.is(last - 1, 0x5d)
                        || (self.is(last - 1, 0x29)
                            && self.partner_of(last - 1).is_some_and(|open| {
                                !(open > from
                                    && self.is_word(open - 1)
                                    && (is_type_operator(self.text(open - 1))
                                        || is_group_specifier(self.text(open - 1))))
                            })))
                {
                    to = last;
                    continue;
                }
                // A ref-qualifier after a parameter list and its cv-qualifiers: `typedef int (BASE::*F)() &;` and
                // `typedef T1 (T1::*F)(T1) const &&;`.
                if matches!(self.punct(last), Some(0x26 | AND_AND)) && last > from {
                    let mut k = last - 1;
                    while k > from && (self.word_is(k, b"const") || self.word_is(k, b"volatile")) {
                        k -= 1;
                    }
                    if self.is(k, 0x29) {
                        to = last;
                        continue;
                    }
                }
                break;
            }
            let last = to - 1;
            if self.is(last, 0x5d) {
                match self.partner_of(last) {
                    Some(open) if open >= from => {
                        to = open;
                        after_suffix = true;
                        continue;
                    }
                    _ => return,
                }
            }
            if self.is(last, 0x29) {
                let Some(open) = self.partner_of(last).filter(|open| *open >= from) else {
                    return;
                };
                // THE POSITION DECIDES, AND THE CONTENT ONLY AFTER A NAME. A group after `)` or `]` is a parameter
                // list: `(*F)(const MLAS_FP16* A)`. A group after a name is a declarator group only when a
                // pointer opens it: `typedef Foo (*F);`, and not `typedef void F(int *p);`. A group after a
                // suffix, or after no name, is a declarator group: `typedef void (__stdcall *F)(void);`.
                let before_close = open > from && (self.is(open - 1, 0x29) || self.is(open - 1, 0x5d));
                let before_name = open > from && self.is_name(open - 1);
                let direct_pointer = matches!(self.punct(open + 1), Some(0x2a | 0x26 | AND_AND | 0x5e));
                if after_suffix || (!before_close && !before_name) || (before_name && direct_pointer) {
                    from = open + 1;
                    to = last;
                    after_suffix = false;
                } else {
                    to = open;
                    after_suffix = true;
                }
                continue;
            }
            if self.is_name(last) {
                if last > from && self.is(last - 1, SCOPE) {
                    return;
                }
                let token = self.t[last];
                if last > from && self.is_name(last - 1) {
                    // `typedef typename X::type T29;`: a name after `::` is part of a type and no declarator.
                    let previous_typed =
                        last - 1 > from && !self.is(last - 2, SCOPE) && !self.word_is(last - 2, b"typedef");
                    self.facts.heads.push(Head::Typedef {
                        last: self.string(last),
                        previous: self.string(last - 1),
                        previous_typed,
                        dead: token.dead,
                        line: token.line,
                    });
                } else {
                    let reference = self.reference_before(last);
                    self.add(last, TYPE | reference);
                }
            }
            return;
        }
    }

    /// Read `using NAME = ...;` at `i`.
    fn using(&mut self, i: usize) {
        let name = i + 1;
        if !self.is_name(name) {
            return;
        }
        // `using __iter_distance_t _LIBCPP_NODEBUG = ...;`: attributes and macro-shaped words can stand
        // between the name and the `=`, and a macro-shaped word can take arguments:
        // `using MemorySpan V8_DEPRECATE_SOON("Use std::span instead.") = std::span<T>;`.
        let mut after = self.skip_attributes(name + 1);
        while self.is_word(after) && is_macro_shaped(self.text(after)) {
            after += 1;
            if self.is(after, 0x28) {
                match self.partner_of(after) {
                    Some(close) => after = close + 1,
                    None => return,
                }
            }
            after = self.skip_attributes(after);
        }
        if self.is(after, u16::from(b'=')) {
            let bits = if self.under_template(i) { TYPE | TEMPLATE } else { TYPE };
            self.add(name, bits);
        }
    }

    /// Read a template head at `i`, the keyword `template` before a `<`. Mark its parameter list and
    /// read each parameter.
    ///
    /// THE PARAMETER LIST IS A TYPE POSITION. Its default arguments can hold a comparison and a logical
    /// operator: `template <class TDst, class TSrc, bool same_rank = TDst::rank() == TSrc::rank()>`. A
    /// matcher for an expression stops there, and every `class` of the list then reads as a class key.
    fn template_head(&mut self, i: usize) {
        self.template_parameters(i + 1);
    }

    /// Read a template parameter list whose `<` is at `open`: after `template`, or after the `]` of a
    /// lambda introducer (`[]<class T>(T x)`).
    fn template_parameters(&mut self, open: usize) {
        let Some(close) = self.type_arguments_close(open) else {
            return;
        };
        for index in open..=close {
            self.in_template_head[index] = true;
        }
        self.template_close[close] = true;
        if close + 1 < self.after_template_head.len() {
            self.after_template_head[close + 1] = true;
        }
        for (from, to) in self.split_with(open + 1, close, 0x2c, true) {
            let mut j = self.skip_attributes(from);
            if j >= to {
                continue;
            }
            if self.word_is(j, b"template") && self.is(j + 1, 0x3c) {
                match self.type_arguments_close(j + 1) {
                    Some(inner) => j = inner + 1,
                    None => continue,
                }
            }
            if self.word_is(j, b"class") || self.word_is(j, b"typename") {
                j += 1;
                if self.is(j, ELLIPSIS) {
                    j += 1;
                }
                if j < to && self.is_name(j) {
                    self.add(j, TYPE_PARAMETER);
                }
                continue;
            }
            // `template <std::ranges::input_range Range>`: a constrained type parameter of a standard concept.
            if let Some(name) = self.standard_constrained_parameter(j, to) {
                self.add(name, TYPE_PARAMETER);
                continue;
            }
            self.declaration(from, to, Context::TemplateParameter);
        }
    }

    // -------------------------------------------------------------------------------------------
    // The value positions.
    // -------------------------------------------------------------------------------------------

    /// True when the `{` at `open` opens a body of statements or of members, and false when it opens a
    /// braced list: `verts.append({radius * float3(x)})`, `const P p[4] = {a * P(x)}`, `Foo{a * b(1)}`.
    ///
    /// THE TOKEN BEFORE THE BRACE DECIDES. `)`, `]`, `>`, `;`, `}`, a literal (`extern "C" {`) and a
    /// keyword (`else`, `try`, `do`, `mutable`, `noexcept`, `const`) open a body. A class head, a
    /// namespace, a macro-shaped word after `)` and an `{` of a body open a body. `(`, `,`, `=`, `return`,
    /// `[`, `?` and a plain name open a list, and a brace in a list opens a list.
    fn statement_brace(&self, open: usize) -> bool {
        if open == 0 || self.class_braces[open] {
            return true;
        }
        let before = open - 1;
        match self.t[before].class {
            // `case X: {` and `default: {` open a block.
            Class::Punct(0x29 | 0x5d | 0x3e | 0x3b | 0x7d | 0x3a) => true,
            Class::Punct(0x7b) => self.statement_brace(before),
            Class::Punct(_) => false,
            Class::Literal => before > 0 && self.word_is(before - 1, b"extern"),
            Class::Number => false,
            Class::Word => {
                let word = self.text(before);
                if word == b"return" || word == b"co_return" || word == b"co_yield" || word == b"throw" {
                    return false;
                }
                if is_keyword(word) || matches!(word, b"final" | b"override" | b"sealed") {
                    return true;
                }
                // `auto f() -> Foo {` and `-> ns::Foo<T>* {`: a trailing return type before a body.
                let mut k = before;
                while k > 0
                    && (self.is_name(k) || matches!(self.punct(k), Some(SCOPE | 0x2a | 0x26 | AND_AND | 0x3e | 0x3c)))
                {
                    k -= 1;
                }
                if self.is(k, ARROW) {
                    return true;
                }
                // `namespace a::b {`, `namespace {`, and a macro after a function head: `void f() LLVM_READONLY {`.
                // A lowercase word after `)` opens a body when its brace group holds a `;`, and `(void) Foo{a}`
                // stays an expression.
                let mut j = before;
                while j > 0 && (self.is_name(j) || self.is(j, SCOPE)) {
                    j -= 1;
                }
                self.word_is(j, b"namespace")
                    || self.word_is(j, b"inline") && j > 0 && self.word_is(j - 1, b"namespace")
                    || (before > 0
                        && self.is(before - 1, 0x29)
                        && (is_macro_shaped(word)
                            || self.partner_of(open).is_some_and(|close| (open..close).any(|index| self.is(index, 0x3b)))))
            }
        }
    }

    /// The name of a template parameter in `[from, to)` that a concept of the standard library constrains:
    /// `std::integral T`, `std::ranges::input_range R`, `std::same_as<int> auto`. None otherwise.
    fn standard_constrained_parameter(&self, from: usize, to: usize) -> Option<usize> {
        let mut j = from;
        if self.is(j, SCOPE) {
            j += 1;
        }
        if !self.word_is(j, b"std") || !self.is(j + 1, SCOPE) {
            return None;
        }
        j += 2;
        if self.word_is(j, b"ranges") && self.is(j + 1, SCOPE) {
            j += 2;
        }
        if !self.is_word(j) || !is_standard_concept(self.text(j)) {
            return None;
        }
        j += 1;
        if self.is(j, 0x3c) {
            j = self.type_arguments_close(j)? + 1;
        }
        if self.is(j, ELLIPSIS) {
            j += 1;
        }
        (j < to && self.is_name(j)).then_some(j)
    }

    /// True when a declaration can start at `i`. The caller fills `starts` in order.
    fn statement_start(&self, i: usize) -> bool {
        // `if (init; A && StringRef(x) == y)`: a statement cannot start inside a parenthesis or a bracket.
        if self.in_group[i] {
            return false;
        }
        if i == 0 || self.after_template_head[i] {
            return true;
        }
        let before = i - 1;
        match self.punct(before) {
            Some(0x7b) => return self.statement_brace(before),
            Some(0x3b | 0x7d) => return true,
            Some(0x3a) => {
                // An access specifier, a label, or a case label.
                let mut j = before;
                let mut words = 0;
                while j > 0 && !matches!(self.punct(j - 1), Some(0x3b | 0x7b | 0x7d)) {
                    j -= 1;
                    words += 1;
                    if words > 4 {
                        return false;
                    }
                }
                let first = self.text(j);
                return (words == 1 && self.is_word(j))
                    || (words == 2 && matches!(first, b"public" | b"private" | b"protected"))
                    || self.word_is(j, b"case");
            }
            Some(0x29) => {
                // A macro invocation with no semicolon, on a line of its own.
                if let Some(open) = self.partner_of(before)
                    && open > 0
                    && self.is_word(open - 1)
                    && self.starts[open - 1]
                    && self.t[i].line > self.t[before].line
                {
                    return true;
                }
            }
            None if self.is_word(before) => {
                if self.starts[before] && is_macro_shaped(self.text(before)) && self.t[i].line > self.t[before].line {
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    /// The innermost class head whose body holds `i`.
    fn enclosing_class(&self) -> Option<&Vec<String>> {
        self.scopes
            .iter()
            .rev()
            .map(|(_, names, _, _)| names)
            .find(|names| !names.is_empty())
    }

    /// True for the token that ends a declarator name in a context.
    ///
    /// A CONDITION DECLARES A NAME ONLY BEFORE `=`, `:` OR `{`: `if (T x = f())`, `for (auto& x : v)`,
    /// `while (T x{g()})`. Each other token after a name there ends an expression, and
    /// `if (flags & ModeFlags(APPEND))` is no declaration of `ModeFlags`.
    fn ends_declarator(&self, i: usize, end: usize, context: Context) -> bool {
        if i >= end {
            return matches!(context, Context::Parameter | Context::TemplateParameter);
        }
        if context == Context::Condition {
            // `if (T x{f()})` declares `x`, and `if (a && CFeeRate{x} < y)` holds a braced cast: the brace
            // group must end the condition.
            return match self.punct(i) {
                Some(0x3d | 0x3a) => true,
                Some(0x7b) => self.partner_of(i).is_some_and(|close| close + 1 >= end),
                _ => false,
            };
        }
        if self.is_word(i) && is_group_specifier(self.text(i)) {
            return true;
        }
        match self.punct(i) {
            // A parameter of a function type holds a parameter list: `int f(int g(int x))`. A group that
            // opens with a number, a literal or an operator is an argument list: `RT hw(a * b * RT(3));`.
            Some(0x28) if context == Context::Parameter => {
                self.is(i + 1, 0x29)
                    || self.is_word(i + 1)
                    || matches!(self.punct(i + 1), Some(0x2a | 0x26 | AND_AND | ELLIPSIS | SCOPE))
                    || (self.is(i + 1, 0x5b) && self.is(i + 2, 0x5b))
            }
            Some(0x28 | 0x5b | 0x3d | 0x3b | 0x2c | 0x7b) => true,
            Some(0x3a) => context != Context::TemplateParameter,
            _ => false,
        }
    }

    /// Read a declaration in `[s, end)`. Each declarator gives a value. O(n) in the tokens.
    fn declaration(&mut self, s: usize, end: usize, context: Context) {
        let end = end.min(self.t.len());
        let mut k = s;
        loop {
            if k >= end {
                return;
            }
            if let Some(next) = self.after_attribute(k) {
                k = next;
                continue;
            }
            if self.is_word(k) && is_specifier(self.text(k)) {
                k += 1;
                continue;
            }
            if self.word_is(k, b"extern")
                || (self.word_is(k.saturating_sub(1), b"extern") && self.t[k].class == Class::Literal)
            {
                k += 1;
                continue;
            }
            break;
        }
        let bits = match context {
            Context::Parameter => PARAMETER,
            Context::TemplateParameter => TEMPLATE_VALUE,
            Context::Statement | Context::Condition => OBJECT,
        };
        let mut typed = false;
        // A keyword type, a class key, a type operator or a pointer came before. A qualified name or a
        // template-id alone is a weaker sign, because `ns::f(*p);` and `f<T>(*p);` are calls.
        let mut strong = false;
        let mut pointer = false;
        let mut run: Vec<usize> = Vec::new();
        while k < end {
            if let Some(next) = self.after_attribute(k) {
                k = next;
                continue;
            }
            match self.t[k].class {
                Class::Word => {
                    let word = self.text(k);
                    if is_builtin_type(word) {
                        typed = true;
                        strong = true;
                        run.clear();
                        k += 1;
                    } else if is_specifier(word) || word == b"typename" {
                        k += 1;
                    } else if matches!(word, b"class" | b"struct" | b"union" | b"enum") {
                        let words_before = self.facts.heads.len();
                        let after = self.class_key(k, false);
                        let _ = words_before;
                        typed = true;
                        strong = true;
                        run.clear();
                        // `struct foo bar, *baz;`: the head record reads `bar`, and the list goes on.
                        k = after;
                        if self.is(k, 0x2c) {
                            self.declarator_list(k + 1, end, context, bits);
                            return;
                        }
                        // `static struct stat getstat(int x);`: the head record reads the function, and its
                        // parameters give values.
                        if self.is(k, 0x28) && k > 0 && self.is_name(k - 1) {
                            if let Some(following) = self.after_declarator(k, end, context) {
                                self.declarator_list(following, end, context, bits);
                            }
                            return;
                        }
                        if !self.is_name(k) && !matches!(self.punct(k), Some(0x2a | 0x26 | AND_AND | 0x5e | 0x28)) {
                            return;
                        }
                        if self.is_name(k) && self.class_words_run_on(k) {
                            return;
                        }
                    } else if is_type_operator(word) && self.is(k + 1, 0x28) {
                        match self.partner_of(k + 1) {
                            Some(close) => k = close + 1,
                            None => return,
                        }
                        typed = true;
                        strong = true;
                        run.clear();
                    } else if word == b"operator" {
                        self.operator_declarator(k, end, context);
                        return;
                    } else if is_keyword(word) {
                        return;
                    } else {
                        let chain = self.chain(k);
                        if chain.operator {
                            self.operator_declarator(chain.next, end, context);
                            return;
                        }
                        if chain.destructor {
                            return;
                        }
                        let next = chain.next;
                        // A name before a declarator group is a type: `static base::TimeTicks (*Now)();`. After a
                        // strong type the name is the declarator, unless a suffix follows the group:
                        // `void get(FT& lb) {` declares `get`, and `FT` is a type that has the shape of a macro.
                        let ends = self.ends_declarator(next, end, context)
                            && !(self.is(next, 0x28)
                                && self.partner_of(next).is_some_and(|close| {
                                    (!strong || matches!(self.punct(close + 1), Some(0x28 | 0x5b)))
                                        && self.starts_declarator_group(next + 1, close)
                                }));
                        let constructor = self.is(next, 0x28)
                            && ((chain.parts >= 2
                                && chain
                                    .previous
                                    .is_some_and(|previous| self.text(previous) == self.text(chain.leaf)))
                                || (chain.parts == 1
                                    && context == Context::Statement
                                    && self.enclosing_class().is_some_and(|names| {
                                        let leaf = self.text(chain.leaf);
                                        names.iter().any(|name| name.as_bytes() == leaf)
                                    })));
                        if constructor {
                            self.parameters(next);
                            return;
                        }
                        if ends && (typed || !run.is_empty()) {
                            let bits = if self.is(next, 0x28) && context == Context::Statement {
                                FUNCTION
                            } else {
                                bits
                            };
                            let bits = bits | self.reference_before(k);
                            // A RUN OF PLAIN NAMES BEFORE THE DECLARATOR GOES TO THE PROJECT, EVEN FOR A QUALIFIED
                            // DECLARATOR: `PB_DS_CLASS_T_DEC PB_DS_CLASS_C_DEC::trigger(float x)` is a constructor
                            // when the run holds macros only.
                            if run.is_empty() {
                                self.add(chain.leaf, bits);
                            } else {
                                run.push(chain.leaf);
                                let token = self.t[chain.leaf];
                                let words = run.iter().map(|&w| self.string(w)).collect();
                                self.facts.heads.push(Head::Run {
                                    words,
                                    typed,
                                    bits: self.scoped(chain.leaf, bits),
                                    dead: token.dead,
                                    line: token.line,
                                    qualifier: chain.previous.map(|previous| self.string(previous)),
                                });
                            }
                            if let Some(following) = self.after_declarator(next, end, context) {
                                self.declarator_list(
                                    following,
                                    end,
                                    context,
                                    bits & !REFERENCE & !FUNCTION | if bits & FUNCTION != 0 { OBJECT } else { 0 },
                                );
                            }
                            return;
                        }
                        if chain.parts >= 2 || chain.arguments {
                            typed = true;
                            run.clear();
                        } else {
                            run.push(chain.leaf);
                        }
                        k = next;
                    }
                }
                Class::Punct(0x2a | 0x26 | AND_AND | 0x5e) => {
                    if !typed && run.is_empty() {
                        return;
                    }
                    // `a * b * c`: a name between two pointer tokens is an operand, because a declarator
                    // puts its pointer tokens together (`T **p`).
                    if pointer && !run.is_empty() {
                        return;
                    }
                    pointer = true;
                    typed = true;
                    strong = true;
                    run.clear();
                    k += 1;
                }
                Class::Punct(SCOPE) => k += 1,
                Class::Punct(0x28) => {
                    let Some(close) = self.partner_of(k) else { return };
                    // `f(*p);` is a call and `Foo (*callback)(int);` a declaration: with no strong type, a
                    // parameter list or an array bound must follow the group.
                    let suffix = matches!(self.punct(close + 1), Some(0x28 | 0x5b));
                    if (strong || ((typed || !run.is_empty()) && suffix)) && self.starts_declarator_group(k + 1, close)
                    {
                        self.group_declarator(k + 1, close, bits);
                        if let Some(following) = self.after_declarator(close + 1, end, context) {
                            self.declarator_list(following, end, context, bits);
                        }
                    }
                    return;
                }
                _ => return,
            }
        }
    }

    /// True when the class key before `k` took the name at `k` among its words already.
    fn class_words_run_on(&self, k: usize) -> bool {
        k > 0 && self.is_name(k - 1)
    }

    /// Read the declarators after the first one: `int a, *b, c[3];`.
    fn declarator_list(&mut self, mut k: usize, end: usize, context: Context, bits: u32) {
        for _ in 0..4096 {
            k = self.skip_attributes(k);
            while k < end
                && (matches!(self.punct(k), Some(0x2a | 0x26 | AND_AND | 0x5e))
                    || (self.is_word(k) && is_specifier(self.text(k))))
            {
                k = self.skip_attributes(k + 1);
            }
            if k >= end {
                return;
            }
            if self.is(k, 0x28) {
                let Some(close) = self.partner_of(k) else { return };
                if !self.starts_declarator_group(k + 1, close) {
                    return;
                }
                self.group_declarator(k + 1, close, bits);
                match self.after_declarator(close + 1, end, context) {
                    Some(following) => k = following,
                    None => return,
                }
                continue;
            }
            if !self.is_name(k) {
                return;
            }
            let chain = self.chain(k);
            if chain.operator || chain.destructor || !self.ends_declarator(chain.next, end, context) {
                return;
            }
            let reference = self.reference_before(k);
            self.add(chain.leaf, bits | reference);
            match self.after_declarator(chain.next, end, context) {
                Some(following) => k = following,
                None => return,
            }
        }
    }

    /// Read the name in a declarator group `[from, to)`: `(*name)`, `(Class::*name[3])`.
    fn group_declarator(&mut self, from: usize, to: usize, bits: u32) {
        let mut k = self.skip_attributes(from);
        while k < to {
            if matches!(self.punct(k), Some(0x2a | 0x26 | AND_AND | 0x5e | SCOPE))
                || (self.is_word(k)
                    && (is_specifier(self.text(k)) || is_macro_shaped(self.text(k)) && !self.is(k + 1, 0x5d)))
                || (self.is_name(k) && self.is(k + 1, SCOPE))
            {
                if self.is_name(k)
                    && !self.is(k + 1, SCOPE)
                    && (k + 1 == to || matches!(self.punct(k + 1), Some(0x5b | 0x28)))
                {
                    break;
                }
                k += 1;
                continue;
            }
            break;
        }
        if k >= to {
            return;
        }
        if self.is(k, 0x28) {
            if let Some(close) = self.partner_of(k).filter(|close| *close <= to) {
                self.group_declarator(k + 1, close, bits);
            }
            return;
        }
        if self.is_name(k) {
            let reference = self.reference_before(k);
            self.add(k, (if bits == FUNCTION { OBJECT } else { bits }) | reference);
            let mut j = k + 1;
            while j < to && self.is(j, 0x5b) {
                j = self.partner_of(j).map_or(to, |close| close + 1);
            }
            if j < to && self.is(j, 0x28) {
                self.parameters(j);
            }
        }
    }

    /// Read an operator declarator from `operator` at `k`: its parameters give values.
    fn operator_declarator(&mut self, k: usize, end: usize, _context: Context) {
        let mut j = k + 1;
        if self.is(j, 0x28) && self.is(j + 1, 0x29) {
            j += 2;
        }
        while j < end && j < self.t.len() {
            match self.punct(j) {
                Some(0x28) => {
                    self.parameters(j);
                    return;
                }
                Some(0x3b | 0x7b | 0x7d) => return,
                Some(0x3c) => j = self.angle_close(j).unwrap_or(j) + 1,
                _ => j += 1,
            }
        }
    }

    /// Read a parameter list whose `(` is at `open`. Each parameter with a name gives a value.
    fn parameters(&mut self, open: usize) {
        let Some(close) = self.partner_of(open) else { return };
        if self.depth >= MAX_DEPTH {
            return;
        }
        self.depth += 1;
        for (from, to) in self.split(open + 1, close, 0x2c) {
            if to <= from || (to == from + 1 && (self.word_is(from, b"void") || self.is(from, ELLIPSIS))) {
                continue;
            }
            self.declaration(from, to, Context::Parameter);
        }
        self.depth -= 1;
    }

    /// Go past what follows a declarator name at `k`: a parameter list and the qualifiers after it, an
    /// array bound, an initializer, a bit-field width. Give the token after a `,` that starts the next
    /// declarator, or None at the end of the declaration.
    fn after_declarator(&mut self, mut k: usize, end: usize, context: Context) -> Option<usize> {
        if context == Context::Parameter || context == Context::TemplateParameter {
            if self.is(k, 0x28) {
                self.parameters(k);
            }
            return None;
        }
        if self.is(k, 0x28) {
            self.parameters(k);
            k = self.partner_of(k)? + 1;
            // The qualifiers and the attributes of a function declarator.
            for _ in 0..64 {
                if k >= end {
                    return None;
                }
                if let Some(next) = self.after_attribute(k) {
                    k = next;
                    continue;
                }
                if self.is_word(k) {
                    let word = self.text(k);
                    if matches!(
                        word,
                        b"const" | b"volatile" | b"override" | b"final" | b"mutable" | b"constexpr"
                    ) || is_macro_shaped(word)
                    {
                        k += 1;
                        continue;
                    }
                    if matches!(word, b"noexcept" | b"throw") {
                        k += 1;
                        if self.is(k, 0x28) {
                            k = self.partner_of(k)? + 1;
                        }
                        continue;
                    }
                    if self.is(k + 1, 0x28) && !is_keyword(word) {
                        // A macro with arguments after the declarator: `void f() LLVM_ATTRIBUTE(x);`.
                        k = self.partner_of(k + 1)? + 1;
                        continue;
                    }
                }
                if matches!(self.punct(k), Some(0x26 | AND_AND)) {
                    k += 1;
                    continue;
                }
                if self.is(k, ARROW) {
                    // A trailing return type, up to the body, the end, or an initializer. A type holds no
                    // top-level `,`, and its template arguments can hold a logical expression:
                    // `-> typename std::conditional<A::value && !B::value, pilfered<T>, T&&>::type`.
                    while k < end && !matches!(self.punct(k), Some(0x7b | 0x3b | 0x3d)) {
                        k = match self.punct(k) {
                            Some(0x28 | 0x5b) => self.partner_of(k)? + 1,
                            Some(0x3c) => self.type_arguments_close(k).map_or(k + 1, |close| close + 1),
                            _ => k + 1,
                        };
                    }
                    continue;
                }
                break;
            }
            if self.is(k, 0x3b) || self.is(k, 0x7b) || self.is(k, 0x3a) {
                return None;
            }
        }
        while self.is(k, 0x5b) {
            k = self.partner_of(k)? + 1;
        }
        if context == Context::Condition {
            return None;
        }
        k = self.skip_attributes(k);
        let start = k;
        let mut assigned = false;
        loop {
            if k >= end {
                return None;
            }
            match self.punct(k) {
                Some(0x2c) => return Some(k + 1),
                Some(0x3b | 0x7d) => return None,
                // A brace group that holds a `;` is a body when it neither follows the declarator directly nor
                // comes after a `=`: the declaration ends before it, and no `,` after it continues the list.
                Some(0x7b) if k != start && !assigned => {
                    let close = self.partner_of(k)?;
                    if (k..close).any(|index| self.is(index, 0x3b)) {
                        return None;
                    }
                    k = close + 1;
                }
                Some(0x3d) => {
                    assigned = true;
                    k += 1;
                }
                Some(0x28 | 0x5b | 0x7b) => k = self.partner_of(k)? + 1,
                Some(0x3c) if k > 0 && self.is_name(k - 1) => k = self.angle_close(k).map_or(k + 1, |close| close + 1),
                _ => {
                    if self.word_is(k, b"try") {
                        return None;
                    }
                    k += 1;
                }
            }
        }
    }

    /// Read the parenthesized head of `for`, `if`, `while`, `switch` or `catch` at `open`.
    fn condition(&mut self, keyword: usize, open: usize) {
        let Some(close) = self.partner_of(open) else { return };
        if self.word_is(keyword, b"catch") {
            self.parameters(open);
            return;
        }
        let parts = self.split(open + 1, close, 0x3b);
        if let Some(&(from, to)) = parts.first() {
            // A `for` head with no semicolon is a range-based for: `for (auto& x : v)`.
            self.declaration(from, to, Context::Condition);
        }
        if self.word_is(keyword, b"if") || self.word_is(keyword, b"switch") {
            if let Some(&(from, to)) = parts.get(1) {
                self.declaration(from, to, Context::Condition);
            }
        }
    }

    fn read(&mut self) {
        self.match_brackets();
        for i in 0..self.t.len() {
            let start = self.statement_start(i);
            self.starts[i] = start;
            if start && !self.in_template_head[i] {
                self.declaration(i, self.t.len(), Context::Statement);
            }
            match self.t[i].class {
                Class::Punct(0x7b) => {
                    let names = self.class_bodies.remove(&i).unwrap_or_default();
                    let namespace = self.namespace_brace(i);
                    let outer = self.class_braces[i] || namespace;
                    self.scopes.push((i, names, outer, namespace));
                }
                Class::Punct(0x7d) => {
                    if let Some(open) = self.partner_of(i)
                        && let Some(depth) = self.scopes.iter().rposition(|(brace, _, _, _)| *brace == open)
                    {
                        self.scopes.truncate(depth);
                    }
                }
                Class::Punct(0x5d) => {
                    // A lambda: `[captures](parameters)`. A subscript has a name or a bracket before its `[`.
                    // `return []<class T>()`: a keyword before the `[` starts no subscript.
                    let introducer = self.partner_of(i).is_some_and(|open| {
                        open == 0 || !(self.is_name(open - 1) || matches!(self.punct(open - 1), Some(0x29 | 0x5d)))
                    });
                    if introducer && self.is(i + 1, 0x28) {
                        self.parameters(i + 1);
                    }
                    // A generic lambda with a template head: `[]<class T>(T x)`.
                    if introducer
                        && self.is(i + 1, 0x3c)
                        && (self.word_is(i + 2, b"class")
                            || self.word_is(i + 2, b"typename")
                            || self.word_is(i + 2, b"template"))
                        && !self.in_template_head[i + 1]
                    {
                        self.template_parameters(i + 1);
                        if let Some(close) = self.type_arguments_close(i + 1)
                            && self.is(close + 1, 0x28)
                        {
                            self.parameters(close + 1);
                        }
                    }
                }
                Class::Word => {
                    let word = self.text(i);
                    match word {
                        b"template"
                            if self.is(i + 1, 0x3c)
                                && !(i > 0 && matches!(self.punct(i - 1), Some(0x2e | ARROW | SCOPE))) =>
                        {
                            if !self.in_template_head[i + 1] {
                                self.template_head(i);
                            }
                        }
                        b"class" | b"struct" | b"union" | b"enum" => {
                            if !(i > 0 && self.word_is(i - 1, b"enum")) {
                                self.class_key(i, true);
                            }
                        }
                        b"typedef" => self.typedef(i),
                        b"using" => self.using(i),
                        b"concept" if self.is_name(i + 1) => self.add(i + 1, CONCEPT),
                        b"for" | b"if" | b"while" | b"switch" | b"catch" if self.is(i + 1, 0x28) => {
                            self.condition(i, i + 1)
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

/// The name and the class of the body of each object-like `#define` of a source, in every branch. O(n)
/// in the bytes of the source.
pub fn macro_bodies(source: &[u8]) -> Vec<(String, Body)> {
    let (bytes, splices) = without_splices(source);
    Lexer {
        b: &bytes,
        at: 0,
        line: 1,
        splices: &splices,
        splice_index: 0,
        tokens: Vec::new(),
        includes: Vec::new(),
        macro_bodies: Vec::new(),
        traps: Traps::default(),
        conditions: Vec::new(),
    }
    .run()
    .macro_bodies
}

/// Read the facts of one source. `c_file` counts the class keys of a C file as a trap. O(n) in the
/// bytes of the source.
pub fn read(source: &[u8], c_file: bool) -> FileFacts {
    let (bytes, splices) = without_splices(source);
    let lexed = Lexer {
        b: &bytes,
        at: 0,
        line: 1,
        splices: &splices,
        splice_index: 0,
        tokens: Vec::new(),
        includes: Vec::new(),
        macro_bodies: Vec::new(),
        traps: Traps::default(),
        conditions: Vec::new(),
    }
    .run();
    let count = lexed.tokens.len();
    let mut reader = Reader {
        b: &bytes,
        t: lexed.tokens,
        partner: vec![NONE; count],
        in_group: vec![false; count],
        class_braces: vec![false; count],
        in_template_head: vec![false; count],
        after_template_head: vec![false; count + 1],
        template_close: vec![false; count],
        starts: vec![false; count],
        class_bodies: BTreeMap::new(),
        scopes: Vec::new(),
        facts: FileFacts {
            includes: lexed.includes,
            traps: lexed.traps,
            ..FileFacts::default()
        },
        c_file,
        depth: 0,
    };
    reader.read();
    reader.facts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The live bits of each name of a source, as words, with the heads resolved by `macros`, each of
    /// which the project defines with a body of the class given.
    fn facts_classes(source: &str, macros: &[(&str, Macro)]) -> Vec<(String, String)> {
        let facts = read(source.as_bytes(), false);
        facts
            .resolve(|name| {
                macros
                    .iter()
                    .find(|(macro_name, _)| *macro_name == name)
                    .map_or(Macro::None, |(_, class)| *class)
            })
            .into_iter()
            .filter(|(_, fact)| fact.live != 0)
            .map(|(name, fact)| (name, bits_text(fact.live & !(OUTER | NAMESPACE))))
            .collect()
    }

    /// The facts of a source with no macro, and with the bits `OUTER` and `NAMESPACE`.
    fn facts_scoped(source: &str) -> Vec<(String, String)> {
        read(source.as_bytes(), false)
            .resolve(|_| Macro::None)
            .into_iter()
            .filter(|(_, fact)| fact.live != 0)
            .map(|(name, fact)| (name, bits_text(fact.live)))
            .collect()
    }

    /// An object and a function at namespace scope or class scope give `OUTER`, and at namespace scope with
    /// no qualifier `NAMESPACE`. A parameter, a template value, a local, a range-for variable and a lambda
    /// parameter give neither.
    #[test]
    fn a_value_at_namespace_or_class_scope_is_outer() {
        assert_eq!(
            facts_scoped(
                "namespace n { int g; void f(int p) { int local; for (auto& e : v) {} auto l = [](int q) {}; } struct S { int m; void h(); }; }\ntemplate <int N> struct T {};\nextern \"C\" { int c_object; }\nstruct stat st;\nFoo bar;\nvoid k() { struct stat st2; Foo baz; }\nint n::S::count = 0;"
            ),
            pairs(&[
                ("N", "template-value"),
                ("S", "type"),
                ("T", "type,template"),
                ("bar", "object,outer,namespace"),
                ("baz", "object"),
                ("c_object", "object,outer,namespace"),
                ("count", "object,outer"),
                ("e", "object,reference"),
                ("f", "function,outer,namespace"),
                ("g", "object,outer,namespace"),
                ("h", "function,outer"),
                ("k", "function,outer,namespace"),
                ("l", "object"),
                ("local", "object"),
                ("m", "object,outer"),
                ("p", "parameter"),
                ("q", "parameter"),
                ("st", "object,outer,namespace"),
                ("st2", "object"),
                ("stat", "type-use")
            ])
        );
    }

    /// `facts_classes` with each macro of the class `Macro::Other`.
    fn facts_with(source: &str, macros: &[&str]) -> Vec<(String, String)> {
        let classes: Vec<(&str, Macro)> = macros.iter().map(|name| (*name, Macro::Other)).collect();
        facts_classes(source, &classes)
    }

    fn facts(source: &str) -> Vec<(String, String)> {
        facts_with(source, &[])
    }

    fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
        expected
            .iter()
            .map(|(name, bits)| ((*name).to_owned(), (*bits).to_owned()))
            .collect()
    }

    /// Each class key position gives a type, and the bits tell a definition from a forward declaration.
    #[test]
    fn a_class_key_and_a_name_give_a_type() {
        assert_eq!(
            facts(
                "class A {}; struct B; union C {}; enum D { X }; enum class E : int { Y }; enum struct F; friend class G;"
            ),
            pairs(&[
                ("A", "type"),
                ("B", "type-use"),
                ("C", "type"),
                ("D", "type"),
                ("E", "type"),
                ("F", "type-use"),
                ("G", "type-use,friend")
            ])
        );
        assert_eq!(
            facts("class ns::A final : public Base<int> { };"),
            pairs(&[("A", "type")])
        );
        assert_eq!(
            facts("template <> struct hash<Key> { };"),
            pairs(&[("hash", "type,template")])
        );
        // A macro with arguments in a class head is an attribute, and a brace in the template arguments
        // of a base does not open the body.
        assert_eq!(
            facts("struct DECLSPEC_UUID(\"B41463C3\") ISetupInstance : public IUnknown { };"),
            pairs(&[("ISetupInstance", "type")])
        );
        assert_eq!(
            facts(
                "class Q_CORE_EXPORT QT_DEPRECATED_VERSION_X_6_15(\"Use QVariant::ConstPointer instead.\") QVariantConstPointer\n{\npublic:\n    explicit QVariantConstPointer(QVariant variant);\n};"
            ),
            pairs(&[("QVariantConstPointer", "type"), ("variant", "parameter")])
        );
        assert_eq!(
            facts(
                "EXPORT struct mallinfo mallinfo() __THROW { struct mallinfo m; return m; } static struct stat getstat(int x);"
            ),
            pairs(&[
                ("getstat", "function"),
                ("m", "object"),
                ("mallinfo", "type-use,function"),
                ("stat", "type-use"),
                ("x", "parameter")
            ])
        );
        assert_eq!(
            facts(
                "inline constexpr struct molar_mass final : named_constant<symbol_text{u8\"M\", \"M\"}, mag<1> * kg> {} molar_mass;"
            ),
            pairs(&[("molar_mass", "type,object")])
        );
        // An elaborated type specifier still names a type.
        assert_eq!(
            facts("struct stat st; struct stat *pst; sizeof(struct foo); x = (struct bar *)p; struct ::statvfs sfs;"),
            pairs(&[
                ("bar", "type-use"),
                ("foo", "type-use"),
                ("pst", "object"),
                ("sfs", "object"),
                ("st", "object"),
                ("stat", "type-use"),
                ("statvfs", "type-use")
            ])
        );
    }

    /// Every declarator of a typedef list gives a type, and the name inside a declarator group too.
    #[test]
    fn each_declarator_of_a_typedef_gives_a_type() {
        assert_eq!(
            facts(
                "typedef int T1, *T2, T3[4]; typedef int (*T4)(int); typedef void T5(int); typedef struct { int m; } T6, *T7;"
            ),
            pairs(&[
                ("T1", "type"),
                ("T2", "type"),
                ("T3", "type"),
                ("T4", "type"),
                ("T5", "type"),
                ("T6", "type"),
                ("T7", "type"),
                ("m", "object")
            ])
        );
        // R3 of the per-translation-unit design: a reference typedef is a type.
        assert_eq!(
            facts(
                "typedef T& reference; typedef std::vector<int> V; typedef void (__stdcall *T8)(void); typedef int (Klass::*T9)(int);"
            ),
            pairs(&[
                ("T8", "type"),
                ("T9", "type"),
                ("V", "type"),
                ("reference", "type,reference")
            ])
        );
        // `typedef struct foo foo_t;`: the typedef reads `foo_t`, and `foo` names a type.
        assert_eq!(
            facts("typedef struct foo foo_t;"),
            pairs(&[("foo", "type-use"), ("foo_t", "type")])
        );
        // The last word of a typedef can be a macro of the project.
        assert_eq!(
            facts_with("typedef int foo_t PACKED;", &["PACKED"]),
            pairs(&[("foo_t", "type")])
        );
        assert_eq!(facts("typedef Foo Bar;"), pairs(&[("Bar", "type")]));
        assert_eq!(
            facts_with("typedef typename X::type T29; typedef Foo T30;", &["T29", "T30"]),
            pairs(&[("T29", "type"), ("T30", "type")])
        );
        assert_eq!(
            facts("typedef int (BASE::*PtrToMemberFuncLValueRef)() &; typedef void(Fn)(int a);"),
            pairs(&[("Fn", "type"), ("PtrToMemberFuncLValueRef", "type")])
        );
        assert_eq!(
            facts(
                "typedef void(Kernel_Fn)(const MLAS_FP16* A, size_t CountN); typedef Foo (*Pointer); typedef void Function(int *p);"
            ),
            pairs(&[("Function", "type"), ("Kernel_Fn", "type"), ("Pointer", "type")])
        );
    }

    /// An alias declaration gives a type, and a template head gives a template.
    #[test]
    fn an_alias_and_a_template_head_give_their_kinds() {
        assert_eq!(
            facts(
                "using A1 = int; template <class P> using A2 = P; template <class T, int N> struct S1 {}; template <class T> class S2; using namespace std; using std::string;"
            ),
            pairs(&[
                ("A1", "type"),
                ("A2", "type,template"),
                ("N", "template-value"),
                ("P", "type-parameter"),
                ("S1", "type,template"),
                ("S2", "template,type-use"),
                ("T", "type-parameter")
            ])
        );
        assert_eq!(
            facts(
                "using __iter_distance_t _LIBCPP_NODEBUG = int; _EXPORT_STD using nanoseconds = duration<long long, nano>;"
            ),
            pairs(&[("__iter_distance_t", "type"), ("nanoseconds", "type")])
        );
        assert_eq!(
            facts(
                "using AccessorNameSetterCallback  //\n    V8_DEPRECATE_SOON(\"Use V2 instead.\") =\n        void (*)(Local<Name> property);"
            ),
            pairs(&[("AccessorNameSetterCallback", "type")])
        );
        // A template template parameter and a constrained head.
        assert_eq!(
            facts("template <template <class> class TT, typename... Ts> requires Sortable<TT> struct S3 {};"),
            pairs(&[
                ("S3", "type,template"),
                ("TT", "type-parameter"),
                ("Ts", "type-parameter")
            ])
        );
        // A brace in a default argument of the head.
        assert_eq!(
            facts("template <class T, op Op, T MatchVal = T{'.'}> void bm(State& state) {}"),
            pairs(&[
                ("MatchVal", "template-value"),
                ("Op", "template-value"),
                ("T", "type-parameter"),
                ("bm", "function"),
                ("state", "parameter,reference")
            ])
        );
    }

    /// TRAP #280: the name of `class EXPORT_MACRO Name` and of `class Name FINAL_MACRO` is the word that
    /// the project does not define as a macro. With no macro row, a last word with no lowercase letter
    /// after a word with one is a macro, and otherwise the name is the last word, as the grammar reads it.
    #[test]
    fn the_macros_of_the_project_decide_the_name_of_a_class_head() {
        assert_eq!(facts("class EXPORT Name {};"), pairs(&[("Name", "type")]));
        assert_eq!(
            facts_with("class EXPORT Name {};", &["EXPORT"]),
            pairs(&[("Name", "type")])
        );
        // With no macro row, a last word with no lowercase letter after a word with one is a macro.
        assert_eq!(facts("class Name FINAL {};"), pairs(&[("Name", "type")]));
        assert_eq!(
            facts("class Name FINAL { int m; };"),
            pairs(&[("Name", "type"), ("m", "object")])
        );
        assert_eq!(
            facts_with("class Name FINAL : public Base {};", &["FINAL"]),
            pairs(&[("Name", "type")])
        );
        assert_eq!(
            facts_with("class Name FINAL {};", &["FINAL"]),
            pairs(&[("Name", "type")])
        );
    }

    /// `struct A B;` IS A FORWARD DECLARATION WHEN `A` IS A MACRO, AND AN OBJECT OF THE TYPE `A` WHEN IT
    /// IS NOT. The three readings of r47 give three answers.
    #[test]
    fn the_macros_of_the_project_decide_struct_a_b() {
        let source = "class META_VIS basic_string; struct CALL_CENTER g_call; class ABSL_A ABSL_B Ptr;";
        assert_eq!(
            facts_with(source, &["META_VIS", "ABSL_A", "ABSL_B"]),
            pairs(&[
                ("CALL_CENTER", "type-use"),
                ("Ptr", "type-use"),
                ("basic_string", "type-use"),
                ("g_call", "object")
            ])
        );
        // A macro whose body is one lowercase name is a tag: `#define flatbuffers_stat stat`. The test
        // helper reads a macro with a lowercase letter as a tag macro.
        assert_eq!(
            facts_classes(
                "struct flatbuffers_stat file_info;",
                &[("flatbuffers_stat", Macro::Tag)]
            ),
            pairs(&[("file_info", "object")])
        );
        // A brace initializer is no class body.
        assert_eq!(
            facts("void f() { struct stat source_descr{}; }"),
            pairs(&[("f", "function"), ("source_descr", "object"), ("stat", "type-use")])
        );
        assert_eq!(
            facts_with("class EXPORT Empty {}; class Named FINAL {};", &["EXPORT", "FINAL"]),
            pairs(&[("Empty", "type"), ("Named", "type")])
        );
        let bodies = macro_bodies(
            b"#define flatbuffers_stat stat\n#define timeval SceNetInetTimeval // x\n#define VIS __attribute__((visibility(\"default\")))\n#define EMPTY\n#define F(x) x\n#define TWO a b\n#define SUN __global\n#define CE constexpr\n#define DWORD unsigned long\n#define CHAR8 UINT8\n#define GPU __host__ __device__\n#define PFX std::filesystem::\n#define SPLIT(x) \\\n  x\n#define uint64_t __UINT64_TYPE__\n",
        );
        assert_eq!(
            bodies,
            [
                ("flatbuffers_stat".to_owned(), Body::Tag),
                ("timeval".to_owned(), Body::Tag),
                ("VIS".to_owned(), Body::Attribute),
                ("EMPTY".to_owned(), Body::Attribute),
                ("TWO".to_owned(), Body::Other),
                ("SUN".to_owned(), Body::Attribute),
                ("CE".to_owned(), Body::Attribute),
                ("DWORD".to_owned(), Body::Other),
                ("CHAR8".to_owned(), Body::Alias("UINT8".to_owned())),
                ("GPU".to_owned(), Body::Attribute),
                ("PFX".to_owned(), Body::Other),
                ("uint64_t".to_owned(), Body::Alias("__UINT64_TYPE__".to_owned())),
            ]
        );
    }

    /// A declarator gives a value: an object, a function, a parameter, a template value.
    #[test]
    fn a_declarator_gives_a_value() {
        assert_eq!(
            facts(
                "Foo bar; static const char *names[] = {0}; int a, *b, c[3]; std::vector<int> v{1, 2}; unsigned x : 3;"
            ),
            pairs(&[
                ("a", "object"),
                ("b", "object"),
                ("bar", "object"),
                ("c", "object"),
                ("names", "object"),
                ("v", "object"),
                ("x", "object")
            ])
        );
        assert_eq!(
            facts(
                "void (*callback)(int arg); int (Klass::*member)(double d); auto lambda = [&](Widget w, int n) { return w; }; try {} catch (const Error &e) {}"
            ),
            pairs(&[
                ("arg", "parameter"),
                ("callback", "object"),
                ("d", "parameter"),
                ("e", "parameter,reference"),
                ("lambda", "object"),
                ("member", "object"),
                ("n", "parameter"),
                ("w", "parameter")
            ])
        );
        assert_eq!(
            facts(
                "void K::m(Foo q) const { Bar local; for (auto &it : v) {} if (Baz *z = g()) {} for (int i = 0; i < n; ++i) {} }"
            ),
            pairs(&[
                ("i", "object"),
                ("it", "object,reference"),
                ("local", "object"),
                ("m", "function"),
                ("q", "parameter"),
                ("z", "object")
            ])
        );
    }

    /// R3: A NAME UNDER A REFERENCE DECLARATOR IS A VALUE. The tree collector records none of these.
    #[test]
    fn a_reference_declarator_gives_a_value() {
        assert_eq!(
            facts("const A& RefFn() const; A& RefDecl(); A& RefField; void g(A& RefParam);"),
            pairs(&[
                ("RefDecl", "function,reference"),
                ("RefField", "object,reference"),
                ("RefFn", "function,reference"),
                ("RefParam", "parameter,reference"),
                ("g", "function")
            ])
        );
    }

    /// R4: A DECLARATION WITH NO TYPE GIVES NO VALUE: a constructor, a destructor, a conversion function,
    /// and a constructor after a macro or after a template head. Their parameters still give values.
    #[test]
    fn a_constructor_gives_no_value() {
        assert_eq!(
            facts(
                "struct K { K(); explicit K(int a); DUCKDB_API K(const K &other); MOZ_IMPLICIT K(Foo f); ~K(); operator bool() const; template <class T> K(T t); };"
            ),
            pairs(&[
                ("K", "type"),
                ("T", "type-parameter"),
                ("a", "parameter"),
                ("f", "parameter"),
                ("other", "parameter,reference"),
                ("t", "parameter")
            ])
        );
        assert_eq!(
            facts(
                "K::K(int b) : x(b) {} template <class T> Box<T>::Box(T c) {} K::~K() {} inline StringBuilder::operator StringView() const { return {}; }"
            ),
            pairs(&[("T", "type-parameter"), ("b", "parameter"), ("c", "parameter")])
        );
    }

    /// The last name of a run that is no macro is the declarator.
    #[test]
    fn the_macros_of_the_project_decide_the_declarator_of_a_run() {
        let source = "LLVM_ABI Foo baz(); Foo qux GUARDED_BY(mu); LLVM_ABI Value *emit();";
        assert_eq!(
            facts_with(source, &["LLVM_ABI", "GUARDED_BY"]),
            pairs(&[("baz", "function"), ("emit", "function"), ("qux", "object")])
        );
        assert_eq!(facts("int qux GUARDED_BY(mu);"), pairs(&[("GUARDED_BY", "function")]));
        assert_eq!(
            facts_with("void f() { DWORD CreationDisposition = OPEN_EXISTING; }", &["DWORD"]),
            pairs(&[("CreationDisposition", "object"), ("f", "function")])
        );
        // A short lowercase macro of the project is no attribute, and the type before it stays a type.
        assert_eq!(
            facts_with("BOOST_CONSTEXPR FromTimePoint f(ms);", &["BOOST_CONSTEXPR", "f"]),
            pairs(&[("f", "function")])
        );
        // A macro of the project can have the name of a real declarator: bde defines `RT(rt)` in a C file.
        assert_eq!(
            facts_with("const Time RT(23, 22, 21, 209);", &["RT"]),
            pairs(&[("RT", "function")])
        );
        assert_eq!(facts_with("FromTimePoint f(ms);", &["f"]), pairs(&[("f", "function")]));
    }

    /// AN ATTRIBUTE MACRO GOES FROM A RUN, A DECLARATOR GROUP IS NO TYPE NAME, AND A TRAILING RETURN TYPE
    /// HOLDS NO DECLARATOR.
    #[test]
    fn the_corpus_shapes_of_the_first_collections() {
        let attributes = [
            ("BOOST_CXX14_CONSTEXPR", Macro::Attribute),
            ("BOOST_FUSION_GPU_ENABLED", Macro::Attribute),
            ("GUIDE", Macro::Attribute),
        ];
        assert_eq!(
            facts_classes(
                "template <class T> BOOST_CXX14_CONSTEXPR BOOST_FUSION_GPU_ENABLED deque(T&& t) : base(t) {}",
                &attributes
            ),
            pairs(&[("T", "type-parameter"), ("t", "parameter,reference")])
        );
        assert_eq!(
            facts_classes("template <class C> GUIDE sender(C, bool *) -> sender<C>;", &attributes),
            pairs(&[("C", "type-parameter")])
        );
        assert_eq!(
            facts("V8_EXPORT_PRIVATE static base::TimeTicks (*Now)();"),
            pairs(&[("Now", "object")])
        );
        assert_eq!(
            facts_with(
                "PB_DS_CLASS_T_DEC\nPB_DS_CLASS_C_DEC::hash_trigger(float load_min) {}",
                &["PB_DS_CLASS_T_DEC", "PB_DS_CLASS_C_DEC"]
            ),
            pairs(&[("load_min", "parameter")])
        );
        assert_eq!(
            facts(
                "template <class T> auto pilfer(T&& t) noexcept -> typename std::conditional<A<T>::value && !B<T>::value, pilfered<T>, T&&>::type { return t; }"
            ),
            pairs(&[
                ("T", "type-parameter"),
                ("pilfer", "function"),
                ("t", "parameter,reference")
            ])
        );
    }

    /// A TEMPLATE HEAD IS A TYPE POSITION, A LAMBDA CAN HAVE ONE, A TYPEDEF SPLITS AT ITS TOP LEVEL ONLY,
    /// AND A LONG CLASS HEAD HOLDS MACROS.
    #[test]
    fn the_template_heads_and_the_typedefs_of_the_oracle_rows() {
        assert_eq!(
            facts("template <class TDst, class TSrc, bool same_rank = TDst::rank() == TSrc::rank()> struct Assign {};"),
            pairs(&[
                ("Assign", "type,template"),
                ("TDst", "type-parameter"),
                ("TSrc", "type-parameter"),
                ("same_rank", "template-value")
            ])
        );
        assert_eq!(
            facts("auto f = []<class Item>(Item item) { return item; };"),
            pairs(&[("Item", "type-parameter"), ("f", "object"), ("item", "parameter")])
        );
        assert_eq!(
            facts("typedef std::conditional_t<R == Real || R == ImagPart, RealScalar, ComplexScalar> OutputScalar;"),
            pairs(&[("OutputScalar", "type")])
        );
        assert_eq!(
            facts_with(
                "template <class T> class ABSL_MUST_USE_RESULT ABSL_NULLABILITY_COMPATIBLE PROTOBUF_NULL_AFTER_MOVE UniquePtr;",
                &["ABSL_MUST_USE_RESULT", "PROTOBUF_NULL_AFTER_MOVE"]
            ),
            pairs(&[("T", "type-parameter"), ("UniquePtr", "template,type-use")])
        );
    }

    /// A FRIEND CLASS, A MACRO CALL AS A CLASS NAME, A TRAILING MACRO OF A TYPEDEF, TWO BRANCHES OF ONE
    /// TEMPLATE ARGUMENT LIST, AND A TEMPLATE TEMPLATE PARAMETER OF A MACRO.
    #[test]
    fn the_friend_and_the_branch_shapes_of_the_oracle_rows() {
        assert_eq!(
            facts("struct A { friend class B; };"),
            pairs(&[("A", "type"), ("B", "type-use,friend")])
        );
        assert_eq!(
            facts(
                "template <class T> struct A { template <class U> friend class ABSL_NULLABILITY_COMPATIBLE UniquePtr; };"
            ),
            pairs(&[
                ("A", "type,template"),
                ("T", "type-parameter"),
                ("U", "type-parameter"),
                ("UniquePtr", "template,type-use")
            ])
        );
        assert_eq!(
            facts("auto f() { return []<class Item>(Item item) { return item; }; }"),
            pairs(&[("Item", "type-parameter"), ("f", "function"), ("item", "parameter")])
        );
        assert_eq!(
            facts("struct Shape FLATBUFFERS_FINAL_CLASS : private ::flatbuffers::Table { };"),
            pairs(&[("Shape", "type")])
        );
        assert_eq!(
            facts("class MLAS_API MLAS_KERNEL { };"),
            pairs(&[("MLAS_KERNEL", "type")])
        );
        assert_eq!(
            facts("struct RANGES_STRUCT_WITH_ADL_BARRIER(view_closure_base) { };"),
            pairs(&[])
        );
        assert_eq!(
            facts("typedef R (BOOST_BIND_CC *F) () BOOST_BIND_NOEXCEPT;"),
            pairs(&[("F", "type")])
        );
        // The shapes of the rows that the text reader lost against the tree reader over the list.
        assert_eq!(
            facts(
                "typedef RT (MyClass::*CLFp)(int) const &;\ntypedef T1 (T1::*TestFunc1CR)(T1) const &&;\ntypedef decltype(ds1) DS1;\ntypedef _Float16 __attribute__((vector_size(16))) F16;\ntypedef int foo6_t(double)noexcept(false);\ntypedef char must_be_a_complete_type[sizeof(T)] BOOST_ATTRIBUTE_UNUSED;\ntypedef int __attribute__((mode(QI))) __attribute__((vector_size(8)))  VT_11;"
            ),
            pairs(&[
                ("CLFp", "type"),
                ("DS1", "type"),
                ("F16", "type"),
                ("TestFunc1CR", "type"),
                ("VT_11", "type"),
                ("foo6_t", "type"),
                ("must_be_a_complete_type", "type")
            ])
        );
        assert_eq!(
            facts("typedef void (*sz_ptrty) (int, int) __arm_inout(\"za\");\ntypedef void (C::*member)() const __arm_preserves(\"za\");"),
            pairs(&[("member", "type"), ("sz_ptrty", "type")])
        );
        assert_eq!(
            facts(
                "typedef typename pack_options\n#if 0\n< hook_defaults, O1 >\n#else\n< hook_defaults, Options... >\n#endif\n::type packed_options;"
            ),
            pairs(&[("packed_options", "type")])
        );
        assert_eq!(
            facts("template(template<typename...> class ContT, typename Rng)(requires x) auto to(Rng&& rng) {}"),
            // The function after the macro form of a head is not read, and no rule of this reader reaches it.
            pairs(&[("ContT", "type-parameter")])
        );
    }

    /// A BRACED LIST, A PRODUCT IN AN ARGUMENT LIST, A BRACED CAST IN A CONDITION AND A CONSTRAINED TYPE
    /// PARAMETER OF A STANDARD CONCEPT GIVE NO VALUE.
    #[test]
    fn the_lists_and_the_products_of_the_lost_rows() {
        assert_eq!(
            facts(
                "void f() { verts.append({radius * float3(c, s, 0)}); const P ps[4] = {p * P(x)}; auto v = Foo{a * b(1)}; }"
            ),
            pairs(&[("f", "function"), ("ps", "object"), ("v", "object")])
        );
        assert_eq!(
            facts("void f() { RT hw( phw*qhw*rhw * RT(3)); }"),
            pairs(&[("f", "function"), ("hw", "function")])
        );
        assert_eq!(
            facts("void f() { if (chain && CFeeRate{fee, 1000} < min) {} if (T x{g()}) {} }"),
            pairs(&[("f", "function"), ("x", "object")])
        );
        assert_eq!(
            facts("template <std::ranges::input_range Range, std::size_t N> void f(Range&& r);"),
            pairs(&[
                ("N", "template-value"),
                ("Range", "type-parameter"),
                ("f", "function"),
                ("r", "parameter,reference")
            ])
        );
        assert_eq!(
            facts("namespace a::b { int x; } extern \"C\" { int y; } void g() LLVM_READONLY { int z; }"),
            pairs(&[("g", "function"), ("x", "object"), ("y", "object"), ("z", "object")])
        );
        assert_eq!(
            facts(
                "auto h() -> Foo { Bar b; switch (c) { case 1: { Baz z; } } } struct S { void f() final { Qux q; } };"
            ),
            pairs(&[
                ("S", "type"),
                ("b", "object"),
                ("f", "function"),
                ("h", "function"),
                ("q", "object"),
                ("z", "object")
            ])
        );
    }

    /// A parameter type with the shape of a macro starts no declarator group after a strong type, and a
    /// function body ends the declarator list. CGAL gave `Bound` as an object from the member initializer
    /// of the next constructor.
    #[test]
    fn a_function_body_ends_the_declarator_list() {
        assert_eq!(
            facts("void get(FT& lb, FT& ub) const {\n}\nV<FT>::V(int idx) : Entry(x), Bound(lb, ub), C(y) {}"),
            pairs(&[
                ("get", "function"),
                ("idx", "parameter"),
                ("lb", "parameter,reference"),
                ("ub", "parameter,reference")
            ])
        );
        // A lowercase word after the parameters ends the qualifiers, and the body after it ends the list.
        assert_eq!(
            facts("void get() lower_attribute {\n  int q;\n}\nV::V(int idx) : Entry(x), Bound(lb, ub) {}"),
            pairs(&[("get", "function"), ("idx", "parameter"), ("q", "object")])
        );
        assert_eq!(
            facts(
                "static base::TimeTicks (*Now)(); void WINAPI (*fp)(int); int a{1}, b; int c = [] { return 1; }(), d;"
            ),
            pairs(&[
                ("Now", "object"),
                ("a", "object"),
                ("b", "object"),
                ("c", "object"),
                ("d", "object"),
                ("fp", "object")
            ])
        );
    }

    /// A statement that is no declaration gives no value.
    #[test]
    fn an_expression_gives_no_value() {
        assert_eq!(
            facts(
                "int main() { f(*p); ns::g(*q); free(x); a = b * c; x->y(z); std::cout << w; return v; foo<T>(*r); arr[i](s); if (a < b && c > d) {} }"
            ),
            pairs(&[("main", "function")])
        );
        assert_eq!(
            facts("int f(int flags) { if (flags & ModeFlags(APPEND)) {} while (a & b) {} return 0; }"),
            pairs(&[("f", "function"), ("flags", "parameter")])
        );
        // A statement does not start after the `;` of an if head, and a lambda body in a call is a body.
        assert_eq!(
            facts("void g() { if (const Arg *A = get(); A && StringRef(A->v) == \"x\") {} call([] { Foo local; }); }"),
            pairs(&[("A", "object"), ("g", "function"), ("local", "object")])
        );
    }

    /// THE FIVE THINGS THAT HIDE TEXT, AND A CLASS KEY IN A MACRO BODY.
    #[test]
    fn a_comment_a_literal_and_a_macro_body_declare_nothing() {
        let source = "#define DECL class Hidden { };\n/* class InComment {}; */ // class InLine {};\nconst char *s = \"class InString {}\";\nauto r = R\"x(class InRaw {};)x\";\nchar c = '{';\n#define LONG \\\n  struct Spliced {};\nclass Real {};";
        assert_eq!(
            facts(source),
            pairs(&[("Real", "type"), ("c", "object"), ("r", "object"), ("s", "object")])
        );
        assert_eq!(read(source.as_bytes(), false).traps.macro_body_class_keys, 2);
    }

    /// AN `#if 0` GROUP IS READ, AND ITS NAMES CARRY THE DEAD BITS.
    #[test]
    fn a_name_in_an_if_0_group_has_the_dead_bits() {
        let facts = read(
            b"#if 0\nclass Dead {};\n#else\nclass Live {};\n#endif\n#if 1\nclass One {};\n#endif\n",
            false,
        );
        let resolved = facts.resolve(|_| Macro::None);
        assert_eq!(resolved["Dead"].live, 0);
        assert_eq!(resolved["Dead"].dead, TYPE);
        assert_eq!(resolved["Live"].live, TYPE);
        assert_eq!(resolved["One"].live, TYPE);
        assert_eq!(facts.traps.zero_groups, 1);
    }

    /// `class` as a name in a C file, and `class` in a template parameter list, declare no class.
    #[test]
    fn class_as_a_name_and_class_in_a_template_head_declare_no_class() {
        assert_eq!(
            facts("void f(int class) { class = 1; x.class = 2; }"),
            pairs(&[("f", "function")])
        );
        let facts = read(b"template <class T, class = void> struct S {};", false);
        assert_eq!(facts.traps.template_parameter_keys, 2);
    }

    /// The include lines, in every branch, with their form.
    #[test]
    fn the_include_lines_are_read_in_every_branch() {
        let facts = read(b"#include <vector>\n#if 0\n#  include \"dead.h\"\n#endif\n// #include \"comment.h\"\n#include_next <next.h>\n", false);
        assert_eq!(
            facts.includes,
            vec![
                (Form::Angle, "vector".to_owned()),
                (Form::Quote, "dead.h".to_owned()),
                (Form::Angle, "next.h".to_owned())
            ]
        );
    }

    /// The line of a name counts the lines that a splice joins.
    #[test]
    fn the_line_of_a_name_counts_the_spliced_lines() {
        let facts = read(b"#define A \\\n  1\n\nclass Third {};\n", false);
        assert_eq!(facts.names["Third"].line, 4);
    }
}
