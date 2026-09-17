//! The names that the `#define` lines of a source define, read from the text of the source.
//!
//! THIS READER NEVER PARSES. The seed of a type reads a tree, and a misparse of the tree then writes
//! a wrong row into the seed: the fork reads `inline StringBuilder::operator StringView() const` as
//! a function named `StringView`, and WebKit lost `StringView` from its seed for that reason. A
//! `#define` line is a line of the preprocessor, and its text says the name and the shape of the
//! macro with no grammar at all. So a defect of the grammar cannot remove a macro from a seed, and a
//! commit that changes the grammar cannot change the macro rows of a seed.
//!
//! THE READER STRIPS WHAT HIDES A LINE FROM THE PREPROCESSOR. It removes a line splice, and it
//! passes a line comment, a block comment, a string literal, a character literal, a raw string
//! literal and the header name of an `#include`. A `#define` in any of them is text and no
//! directive. A block comment that spans lines is one space ([lex.phases] p1.3), so a `#` after it
//! starts a directive only when a line break comes before the comment ([cpp.pre] p1).
//!
//! THE READER REPRODUCES THE `#define` INDEX OF r58, which read the text of the same lines. Over the
//! source files of the 64 corpus projects on 2026-09-17, the index holds 626,204 pairs of a project
//! and a name, this reader gives 627,419, and 99.6% of the pairs are the same. The two classes of
//! difference were read one by one. The 521 pairs of the index alone are text that the index read
//! as a directive: a raw string of a test (`#define EXPLICIT explicit` in
//! clang-tidy/LexerUtilsTest.cpp), a block comment (`#define ULlong` in mozjs DoubleToString.cpp),
//! and a `\code` block of doxygen (`#define RAPIDJSON_SSE42` in rapidjson.h). The 1,736 pairs of
//! this reader alone come from files that the index did not read: the `.tcc`, `.icc` and `.cppm`
//! files, the `third_party/nccl` tree of pytorch, and names with a `$`.
//!
//! THE READER TAKES EVERY BRANCH OF A CONDITIONAL. `#if 0` and `#ifdef _WIN32` are not evaluated,
//! so a macro that one configuration defines is a macro of the project. The reader also cannot
//! tell a real definition from a test fixture: qtbase defines `QString()` in
//! tests/auto/tools/moc/parse-defines.h. A consumer of these names must read them as "a file of the
//! project defines this name", and never as "this name is a macro at this site".
//!
//! AN ALIAS HAS THE KINDS OF ITS TARGET. `#define P_ BSLIM_TESTUTIL_P_` in bde is object-like, and its
//! replacement list is one identifier that names a function-like macro. `P_(LINE)` then expands to
//! `BSLIM_TESTUTIL_P_(LINE)`, and the group after `P_` is an argument list ([cpp.rescan] p1). So a name
//! whose replacement list is exactly one identifier gives that identifier as its alias target, and
//! `inherit_alias_bits` gives the alias the kinds of each target at the end of its chain.

use std::collections::{BTreeMap, BTreeSet};

/// The shape of a `#define`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Shape {
    /// `#define NAME` or `#define NAME text`: no parameter list follows the name.
    Object,
    /// `#define NAME(`: a `(` comes immediately after the name, with no white space and no comment
    /// between the two. [cpp.replace] p10 reads that `(` as the start of a parameter list, and any
    /// other `(` as the start of the replacement list.
    Function,
}

/// One `#define` of a source.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Define {
    /// The name of the macro.
    pub name: String,
    /// The shape of the macro.
    pub shape: Shape,
    /// The replacement list of an object-like macro when that list is exactly one identifier, with no
    /// other token: `BSLIM_TESTUTIL_P_` of `#define P_ BSLIM_TESTUTIL_P_`. None for each other macro.
    pub alias: Option<String>,
    /// The first tokens of the replacement list, at most `BODY_TOKENS` of them. A string literal and a
    /// character literal give their quote only, because no shape of a body reads the text of a literal.
    pub body: Vec<String>,
    /// The last token of the replacement list, empty for a body of no token.
    pub body_last: String,
    /// The number of tokens of the replacement list.
    pub body_count: usize,
}

/// The tokens that `Define::body` keeps. A shape reads the first token, the last token or the count,
/// and a shape of a whole body reads the kept tokens and the count together.
pub const BODY_TOKENS: usize = 8;

/// THE SHAPE OF THE REPLACEMENT LIST OF A `#define`, which task 371 gives to a seed row.
///
/// A row of a seed says that a project defines a name as a macro, and format 2 says nothing about the
/// replacement list. A position of the text can then hold a macro of any expansion, and the grammar
/// must read it as the text permits. `#define __ masm->` of v8 and `#define nssv_noexcept noexcept` of
/// simdjson need the expansion, and the shape gives what the expansion looks like.
///
/// A SHAPE SAYS WHAT THE BODY LOOKS LIKE, AND NEVER WHAT IT MEANS. blender writes
/// `#define ccl_private thread` for the address space of Metal, and `thread` is also the name of a class
/// of blender. A reader of a shape asks its question at a position where the grammar already forbids
/// each other reading.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum BodyShape {
    /// No token: `#define NDEBUG`.
    Empty,
    /// One identifier that is no keyword: `#define hb_locale_t locale_t`, `#define P_ BSLIM_TESTUTIL_P_`.
    OneName,
    /// Specifier keywords, reserved names in lowercase and attribute groups only:
    /// `#define nssv_constexpr constexpr`, `#define MY_API __declspec(dllexport)`.
    Specifier,
    /// Type keywords and the punctuators of a type, with no name: `#define DWORD unsigned long`.
    TypeExpression,
    /// The last token is `::`: `#define _STD ::std::`.
    ScopePrefix,
    /// The last token is `->` or `.`: `#define __ masm->`.
    MemberPrefix,
    /// The first token is `=` or `{`: `#define _ZERO_OR_NO_INIT = 0`.
    Initializer,
    /// `NAME ( ... )`, with the name of the call: `#define __ ACCESS_MASM(masm)`. A reader resolves the
    /// shape of the body of that macro, because the expansion of the call is the expansion of the name.
    Call(String),
    /// More than one token, and the last token is a name: `#define A x FOO`.
    EndsWithName(String),
    /// Each other body: `#define PI 3.14`, `#define WORDS a + b`.
    Other,
}

/// The keywords that a declaration takes as a specifier and that name no type.
const SPECIFIER_KEYWORDS: [&str; 17] = [
    "constexpr", "consteval", "constinit", "inline", "static", "extern", "explicit", "virtual", "friend", "mutable",
    "thread_local", "register", "noexcept", "typename", "override", "final", "alignas",
];

/// The keywords that name a type, with the two cv-qualifiers, which a type expression takes.
const TYPE_KEYWORDS: [&str; 17] = [
    "void", "bool", "char", "char8_t", "char16_t", "char32_t", "wchar_t", "short", "int", "long", "signed",
    "unsigned", "float", "double", "auto", "const", "volatile",
];

/// The other keywords of C++. A body of one such keyword names no type and no macro.
const OTHER_KEYWORDS: [&str; 34] = [
    "class", "struct", "union", "enum", "template", "operator", "this", "nullptr", "true", "false", "return", "if",
    "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "goto", "sizeof", "decltype",
    "new", "delete", "throw", "try", "catch", "namespace", "using", "public", "private", "protected",
];

/// The punctuators that a type expression takes.
const TYPE_PUNCTUATORS: [&str; 5] = ["*", "&", "&&", "::", "..."];

/// The punctuators and the tokens that an attribute group takes.
const ATTRIBUTE_TOKENS: [&str; 6] = ["(", ")", "[", "]", ",", "\""];

/// True for a token that starts with a letter or `_`.
fn starts_name(token: &str) -> bool {
    token.starts_with(|byte: char| byte.is_ascii_alphabetic() || byte == '_')
}

/// True for a name that a `#define` line writes in the shape of a macro: no lowercase letter.
fn macro_shaped(token: &str) -> bool {
    starts_name(token) && !token.chars().any(|byte| byte.is_ascii_lowercase())
}

/// True for a keyword of C++, of any of the three lists above.
fn is_keyword(token: &str) -> bool {
    SPECIFIER_KEYWORDS.contains(&token) || TYPE_KEYWORDS.contains(&token) || OTHER_KEYWORDS.contains(&token)
}

/// True for a name that the standard reserves to the compiler: `__x`, `_X`.
fn reserved(token: &str) -> bool {
    token.starts_with("__")
        || (token.len() > 1 && token.starts_with('_') && token[1..2].chars().all(char::is_uppercase))
}

/// The shape of the replacement list of one `#define`. O(n) in the tokens that the reader kept.
///
/// A RESERVED NAME WITH A LOWERCASE LETTER IS A SPECIFIER, AND A RESERVED NAME IN UPPERCASE IS A NAME.
/// blender writes `#define ccl_private __private` for the address space of OpenCL, and Sun writes
/// `#define BOOST_SYMBOL_VISIBLE __global`. clang writes `#define uint64_t __UINT64_TYPE__`, which
/// names a type of the compiler. The text reader of the type rows takes the same two readings.
pub fn body_shape(define: &Define) -> BodyShape {
    let first = define.body.first().map(String::as_str).unwrap_or("");
    let last = define.body_last.as_str();
    if define.body_count == 0 {
        return BodyShape::Empty;
    }
    if define.body_count == 1 {
        if SPECIFIER_KEYWORDS.contains(&first) {
            return BodyShape::Specifier;
        }
        if TYPE_KEYWORDS.contains(&first) {
            return BodyShape::TypeExpression;
        }
        if OTHER_KEYWORDS.contains(&first) {
            return BodyShape::Other;
        }
        if reserved(first) && !macro_shaped(first) {
            return BodyShape::Specifier;
        }
        return if starts_name(first) { BodyShape::OneName } else { BodyShape::Other };
    }
    if last == "::" {
        return BodyShape::ScopePrefix;
    }
    if last == "->" || last == "." {
        return BodyShape::MemberPrefix;
    }
    if first == "=" || first == "{" {
        return BodyShape::Initializer;
    }
    // The two shapes below read every token, so a body of more tokens than the reader kept takes
    // neither of them.
    let whole = define.body_count <= define.body.len();
    let tokens: Vec<&str> = define.body.iter().map(String::as_str).collect();
    // A SPECIFIER BODY HOLDS SPECIFIERS OUTSIDE ITS GROUPS, AND ANYTHING INSIDE THEM. The argument of
    // an attribute is an attribute name and no specifier: `__declspec(dllexport)`,
    // `__attribute__((visibility("default")))`. So the rule reads the tokens at depth 0 only.
    let mut depth = 0;
    let mut outside = Vec::new();
    for token in &tokens {
        match *token {
            "(" | "[" => depth += 1,
            ")" | "]" => depth -= 1,
            _ if depth == 0 => outside.push(*token),
            _ => {}
        }
    }
    if whole
        && depth == 0
        && outside.iter().all(|token| SPECIFIER_KEYWORDS.contains(token) || reserved(token) || macro_shaped(token))
        && outside.iter().any(|token| SPECIFIER_KEYWORDS.contains(token) || reserved(token))
    {
        return BodyShape::Specifier;
    }
    if whole
        && tokens.iter().all(|token| TYPE_KEYWORDS.contains(token) || TYPE_PUNCTUATORS.contains(token))
        && tokens.iter().any(|token| TYPE_KEYWORDS.contains(token))
    {
        return BodyShape::TypeExpression;
    }
    if last == ")" && starts_name(first) && !is_keyword(first) {
        return BodyShape::Call(first.to_owned());
    }
    // A keyword at the end names no macro, so the group after such a body is no argument list.
    if starts_name(last) && !is_keyword(last) {
        return BodyShape::EndsWithName(last.to_owned());
    }
    BodyShape::Other
}

/// The longest delimiter of a raw string literal, from [lex.string] p2.
const RAW_DELIMITER: usize = 16;

/// A cursor over the bytes of a source that removes the line splices as it reads.
#[derive(Clone)]
struct Text<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Text<'_> {
    /// Go past each line splice at the cursor. A splice is a `\` and a line break. The blanks between
    /// the two are part of the splice, as GCC and Clang read it with a warning.
    fn splice(&mut self) {
        while self.bytes.get(self.at) == Some(&b'\\') {
            let mut next = self.at + 1;
            while matches!(self.bytes.get(next), Some(b' ' | b'\t')) {
                next += 1;
            }
            if self.bytes.get(next) == Some(&b'\r') {
                next += 1;
            }
            if self.bytes.get(next) != Some(&b'\n') {
                return;
            }
            self.at = next + 1;
        }
    }

    /// The byte at the cursor, after the splices, or None at the end of the source.
    fn peek(&mut self) -> Option<u8> {
        self.splice();
        self.bytes.get(self.at).copied()
    }

    /// The byte after the byte at the cursor, after the splices. The cursor does not move.
    fn peek_second(&mut self) -> Option<u8> {
        self.splice();
        let mut ahead = self.clone();
        ahead.at += 1;
        ahead.peek()
    }

    /// Go past the byte at the cursor.
    fn bump(&mut self) {
        self.at += 1;
    }

    /// Go past a block comment from its `/*`. An unterminated comment ends at the end of the source.
    fn skip_block_comment(&mut self) {
        self.bump();
        self.bump();
        while let Some(byte) = self.peek() {
            self.bump();
            if byte == b'*' && self.peek() == Some(b'/') {
                self.bump();
                return;
            }
        }
    }

    /// Go past a line comment from its `//`, up to the line break. A splice continues the comment.
    fn skip_line_comment(&mut self) {
        while let Some(byte) = self.peek() {
            if byte == b'\n' {
                return;
            }
            self.bump();
        }
    }

    /// Go past a string literal or a character literal from its quote. A literal with no closing quote
    /// ends at the line break, and the line break stays for the caller.
    fn skip_quoted(&mut self, quote: u8) {
        self.bump();
        while let Some(byte) = self.peek() {
            match byte {
                b'\n' => return,
                b'\\' => {
                    self.bump();
                    if self.peek().is_some_and(|escaped| escaped != b'\n') {
                        self.bump();
                    }
                }
                _ if byte == quote => {
                    self.bump();
                    return;
                }
                _ => self.bump(),
            }
        }
    }

    /// Go past a raw string literal from its `"`. The splices of a raw string are part of its text,
    /// so this scan reads the bytes as they are. A delimiter that [lex.string] p2 does not permit
    /// gives an ordinary string literal. O(n) in the bytes of the literal.
    fn skip_raw_string(&mut self) {
        let open = self.at + 1;
        let Some(length) = self.bytes[open..]
            .iter()
            .take(RAW_DELIMITER + 1)
            .position(|&byte| byte == b'(')
            .filter(|&length| {
                self.bytes[open..open + length]
                    .iter()
                    .all(|&byte| !matches!(byte, b' ' | b'(' | b')' | b'\\' | b'\t' | b'\x0b' | b'\x0c' | b'\n' | b'\r'))
            })
        else {
            self.skip_quoted(b'"');
            return;
        };
        let delimiter = &self.bytes[open..open + length];
        let mut at = open + length + 1;
        while at < self.bytes.len() {
            if self.bytes[at] == b')'
                && self.bytes[at + 1..].starts_with(delimiter)
                && self.bytes.get(at + 1 + length) == Some(&b'"')
            {
                self.at = at + length + 2;
                return;
            }
            at += 1;
        }
        self.at = self.bytes.len();
    }

    /// Read an identifier at the cursor, with the splices removed. An identifier character is an
    /// ASCII letter, a digit, `_`, `$`, or a byte of a character that is not ASCII.
    fn identifier(&mut self) -> Vec<u8> {
        let mut word = Vec::new();
        while let Some(byte) = self.peek() {
            if !is_identifier_byte(byte) {
                break;
            }
            word.push(byte);
            self.bump();
        }
        word
    }

    /// Go past the blanks and the block comments of the line at the cursor. A line break stops the
    /// scan, and a line comment stops it too, because the comment runs to the line break.
    fn skip_line_space(&mut self) {
        while let Some(byte) = self.peek() {
            match byte {
                b' ' | b'\t' | b'\r' | b'\x0b' | b'\x0c' => self.bump(),
                b'/' if self.peek_second() == Some(b'*') => self.skip_block_comment(),
                _ => return,
            }
        }
    }
}

/// True for a byte that can be part of an identifier.
fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 0x80
}

/// True for a byte that can start an identifier.
fn is_identifier_start(byte: u8) -> bool {
    is_identifier_byte(byte) && !byte.is_ascii_digit()
}

/// Give the name and the shape of each `#define` of a source, in the order of the text. O(n) in the
/// bytes of the source.
///
/// A name that is not UTF-8 is left out, because a seed holds text.
pub fn defines(source: &[u8]) -> Vec<(String, Shape)> {
    define_lines(source).into_iter().map(|define| (define.name, define.shape)).collect()
}

/// Give each `#define` of a source with its alias target, in the order of the text. O(n) in the bytes
/// of the source.
pub fn define_lines(source: &[u8]) -> Vec<Define> {
    let mut text = Text { bytes: source, at: 0 };
    let mut found = Vec::new();
    // [cpp.pre] p1: a directive starts at the start of the source or after white space that holds a
    // line break. A comment is white space with no line break in it.
    let mut line_start = true;
    while let Some(byte) = text.peek() {
        match byte {
            b'\n' => {
                text.bump();
                line_start = true;
            }
            b' ' | b'\t' | b'\r' | b'\x0b' | b'\x0c' => text.bump(),
            b'/' if text.peek_second() == Some(b'/') => text.skip_line_comment(),
            b'/' if text.peek_second() == Some(b'*') => text.skip_block_comment(),
            b'#' if line_start => {
                text.bump();
                line_start = false;
                directive(&mut text, &mut found);
            }
            b'%' if line_start && text.peek_second() == Some(b':') => {
                text.bump();
                text.bump();
                line_start = false;
                directive(&mut text, &mut found);
            }
            b'"' | b'\'' => {
                line_start = false;
                text.skip_quoted(byte);
            }
            _ if is_identifier_start(byte) => {
                line_start = false;
                let word = text.identifier();
                // An encoding prefix and `R` start a raw string: `R"x(...)x"`, `u8R"(...)"`. The other
                // prefixes start an ordinary literal, which the next turn of the loop reads.
                if matches!(word.as_slice(), b"R" | b"LR" | b"uR" | b"UR" | b"u8R") && text.peek() == Some(b'"') {
                    text.skip_raw_string();
                }
            }
            _ if byte.is_ascii_digit() || (byte == b'.' && text.peek_second().is_some_and(|next| next.is_ascii_digit())) => {
                line_start = false;
                skip_number(&mut text);
            }
            _ => {
                line_start = false;
                text.bump();
            }
        }
    }
    found
}

/// The names of the directives that `starts_with_directive` takes as the first token of a header.
const DIRECTIVE_NAMES: [&[u8]; 15] = [
    b"if", b"ifdef", b"ifndef", b"elif", b"else", b"endif", b"define", b"undef", b"include", b"include_next",
    b"import", b"pragma", b"error", b"warning", b"line",
];

/// True when the first token of a source is a directive of `DIRECTIVE_NAMES`, after the blanks, the
/// line breaks, the comments and a byte order mark. O(n) in the bytes before that token.
///
/// A HEADER WITH NO EXTENSION STARTS WITH A DIRECTIVE, AND A SCRIPT DOES NOT. libstdc++ `vector` and
/// Eigen `Core` start with a comment and an `#ifndef` or a `#pragma`. A shell script starts with
/// `#!`, whose `!` names no directive. A Makefile starts with `# a comment`, whose word is no
/// directive name. A ChangeLog starts with text.
pub fn starts_with_directive(source: &[u8]) -> bool {
    let bytes = source.strip_prefix(b"\xef\xbb\xbf").unwrap_or(source);
    let mut text = Text { bytes, at: 0 };
    loop {
        match text.peek() {
            Some(b' ' | b'\t' | b'\r' | b'\n' | b'\x0b' | b'\x0c') => text.bump(),
            Some(b'/') if text.peek_second() == Some(b'/') => text.skip_line_comment(),
            Some(b'/') if text.peek_second() == Some(b'*') => text.skip_block_comment(),
            Some(b'#') => {
                text.bump();
                break;
            }
            Some(b'%') if text.peek_second() == Some(b':') => {
                text.bump();
                text.bump();
                break;
            }
            _ => return false,
        }
    }
    text.skip_line_space();
    let name = text.identifier();
    DIRECTIVE_NAMES.contains(&name.as_slice())
}

/// Go past a preprocessing number, [lex.ppnumber]. A `'` between two digits of a number is a digit
/// separator and starts no character literal: `1'000'000`.
fn skip_number(text: &mut Text) {
    text.bump();
    while let Some(byte) = text.peek() {
        match byte {
            b'e' | b'E' | b'p' | b'P' => {
                text.bump();
                if matches!(text.peek(), Some(b'+' | b'-')) {
                    text.bump();
                }
            }
            b'\'' if text.peek_second().is_some_and(is_identifier_byte) => {
                text.bump();
                text.bump();
            }
            b'.' => text.bump(),
            _ if is_identifier_byte(byte) => text.bump(),
            _ => return,
        }
    }
}

/// Read a directive after its `#`, and record a `#define`. The cursor stops after the name of the
/// macro, or after the name of the directive, and the caller reads the rest of the line as text.
fn directive(text: &mut Text, found: &mut Vec<Define>) {
    text.skip_line_space();
    let name = text.identifier();
    match name.as_slice() {
        b"define" => {}
        // A header name is no string literal and no comment: `#include <sys/*.h>` holds no comment.
        b"include" | b"include_next" | b"import" => {
            text.skip_line_space();
            if text.peek() == Some(b'<') {
                while let Some(byte) = text.peek() {
                    if byte == b'\n' {
                        return;
                    }
                    text.bump();
                    if byte == b'>' {
                        return;
                    }
                }
            }
            return;
        }
        _ => return,
    }
    text.skip_line_space();
    if !text.peek().is_some_and(is_identifier_start) {
        return;
    }
    let macro_name = text.identifier();
    let shape = if text.peek() == Some(b'(') { Shape::Function } else { Shape::Object };
    let alias = if shape == Shape::Object { alias_target(text) } else { None };
    let (body, body_last, body_count) = if shape == Shape::Object {
        body_tokens(text)
    } else {
        // The replacement list of a function-like macro starts after its parameter list. The list
        // holds names, commas and `...`, and it ends at the first `)` of the line.
        let mut ahead = text.clone();
        let mut depth = 0;
        loop {
            match ahead.peek() {
                None | Some(b'\n') => break,
                Some(b'(') => {
                    depth += 1;
                    ahead.bump();
                }
                Some(b')') => {
                    depth -= 1;
                    ahead.bump();
                    if depth == 0 {
                        break;
                    }
                }
                Some(_) => ahead.bump(),
            }
        }
        body_tokens(&ahead)
    };
    if let Ok(name) = String::from_utf8(macro_name) {
        found.push(Define { name, shape, alias, body, body_last, body_count });
    }
}

/// The tokens of the replacement list of an object-like macro, from the cursor to the end of the
/// directive line. The cursor does not move, because the caller reads the rest of the line as text.
///
/// The first tokens, the last token and the count come back. A comment is white space, and a line
/// comment ends the list. A string literal, a character literal and a raw string literal give their
/// quote, because no class of a body reads the text of a literal. O(n) in the bytes of the line.
fn body_tokens(text: &Text) -> (Vec<String>, String, usize) {
    /// The punctuators of more than one byte that a body can end with or start with, longest first.
    const PUNCTUATORS: [&[u8]; 20] = [
        b"->*", b"...", b"<<=", b">>=", b"->", b"::", b"##", b"<<", b">>", b"&&", b"||", b"==", b"!=", b"<=", b">=",
        b"+=", b"-=", b"*=", b"/=", b".*",
    ];
    let mut ahead = text.clone();
    let mut first = Vec::new();
    let mut last = String::new();
    let mut count = 0;
    loop {
        ahead.skip_line_space();
        let Some(byte) = ahead.peek() else { break };
        if byte == b'\n' || (byte == b'/' && ahead.peek_second() == Some(b'/')) {
            break;
        }
        let token = if is_identifier_start(byte) {
            let word = ahead.identifier();
            // An encoding prefix and `R` start a raw string, whose text is no token of a body.
            if matches!(word.as_slice(), b"R" | b"LR" | b"uR" | b"UR" | b"u8R") && ahead.peek() == Some(b'"') {
                ahead.skip_raw_string();
                "\"".to_owned()
            } else {
                String::from_utf8_lossy(&word).into_owned()
            }
        } else if byte == b'"' || byte == b'\'' {
            ahead.skip_quoted(byte);
            (byte as char).to_string()
        } else if byte.is_ascii_digit() {
            let mut number = Vec::new();
            while let Some(next) = ahead.peek() {
                if !is_identifier_byte(next) && next != b'.' {
                    break;
                }
                number.push(next);
                ahead.bump();
            }
            String::from_utf8_lossy(&number).into_owned()
        } else {
            let rest: Vec<u8> = {
                let mut copy = ahead.clone();
                let mut bytes = Vec::new();
                for _ in 0..3 {
                    match copy.peek() {
                        Some(next) if next != b'\n' => {
                            bytes.push(next);
                            copy.bump();
                        }
                        _ => break,
                    }
                }
                bytes
            };
            let width = PUNCTUATORS.iter().find(|p| rest.starts_with(p)).map_or(1, |p| p.len());
            let mut punctuator = Vec::new();
            for _ in 0..width {
                if let Some(next) = ahead.peek() {
                    punctuator.push(next);
                    ahead.bump();
                }
            }
            String::from_utf8_lossy(&punctuator).into_owned()
        };
        count += 1;
        if first.len() < BODY_TOKENS {
            first.push(token.clone());
        }
        last = token;
    }
    (first, last, count)
}

/// The replacement list of an object-like macro when it is exactly one identifier. The cursor does not
/// move, because the caller reads the rest of the line as text. A comment is white space, and a line
/// comment runs to the line break. O(n) in the bytes of the line.
fn alias_target(text: &Text) -> Option<String> {
    let mut ahead = text.clone();
    ahead.skip_line_space();
    if !ahead.peek().is_some_and(is_identifier_start) {
        return None;
    }
    let target = ahead.identifier();
    ahead.skip_line_space();
    let ends = match ahead.peek() {
        None | Some(b'\n') => true,
        Some(b'/') => ahead.peek_second() == Some(b'/'),
        Some(_) => false,
    };
    if ends { String::from_utf8(target).ok() } else { None }
}

/// Give each alias the macro bits of each name that its chain of aliases reaches. `bits` maps a name to
/// the bits of its `#define` lines, and `aliases` maps a name to the targets of its alias lines. A name
/// can have a target in one branch and a different body in another, so it keeps its own bits and adds
/// the bits of the targets. A chain that comes back to a name ends there. O(a * c) in the aliases and
/// the length of the longest chain.
pub fn inherit_alias_bits(bits: &mut BTreeMap<String, u16>, aliases: &BTreeMap<String, BTreeSet<String>>) {
    let mut inherited = Vec::new();
    for name in aliases.keys() {
        let mut seen = BTreeSet::new();
        let mut stack = vec![name.as_str()];
        let mut sum = 0;
        while let Some(current) = stack.pop() {
            if !seen.insert(current) {
                continue;
            }
            if current != name {
                sum |= bits.get(current).copied().unwrap_or(0);
            }
            if let Some(targets) = aliases.get(current) {
                stack.extend(targets.iter().map(String::as_str));
            }
        }
        inherited.push((name, sum));
    }
    for (name, sum) in inherited {
        if sum != 0 {
            *bits.entry(name.clone()).or_default() |= sum;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{BodyShape, Shape, body_shape, define_lines, defines, inherit_alias_bits, starts_with_directive};

    /// The alias target of each `#define` of a source.
    fn aliases(source: &str) -> Vec<(String, Option<String>)> {
        define_lines(source.as_bytes()).into_iter().map(|define| (define.name, define.alias)).collect()
    }

    fn alias(name: &str, target: Option<&str>) -> Vec<(String, Option<String>)> {
        vec![(name.to_owned(), target.map(str::to_owned))]
    }

    /// The replacement list of an object-like macro is an alias when it is exactly one identifier. The
    /// first line is the macro of bde that #370 found.
    #[test]
    fn a_replacement_list_of_one_identifier_is_an_alias() {
        assert_eq!(aliases("#define P_ BSLIM_TESTUTIL_P_\n"), alias("P_", Some("BSLIM_TESTUTIL_P_")));
        assert_eq!(aliases("#define A B // a comment\n"), alias("A", Some("B")));
        assert_eq!(aliases("#define A /* x */ B /* y */\r\n"), alias("A", Some("B")));
        assert_eq!(aliases("#define A \\\n  B\n"), alias("A", Some("B")));
        assert_eq!(aliases("#define A B"), alias("A", Some("B")));
        // A list with a second token, a literal, a raw string, or no token is no alias.
        assert_eq!(aliases("#define A B C\n"), alias("A", None));
        assert_eq!(aliases("#define A B(x)\n"), alias("A", None));
        assert_eq!(aliases("#define A __attribute__((unused))\n"), alias("A", None));
        assert_eq!(aliases("#define A u8\"x\"\n"), alias("A", None));
        assert_eq!(aliases("#define A R\"(x)\"\n"), alias("A", None));
        assert_eq!(aliases("#define A 1\n"), alias("A", None));
        assert_eq!(aliases("#define A\n"), alias("A", None));
        assert_eq!(aliases("#define A /* nothing */\n"), alias("A", None));
        // A function-like macro has parameters, and its list is no alias.
        assert_eq!(aliases("#define A(x) B\n"), alias("A", None));
        // The rest of the line stays text for the reader: the raw string hides the next `#define`.
        assert_eq!(aliases("#define A R\"(\n#define B\n)\"\n"), alias("A", None));
    }

    /// An alias gets the bits of each name at the end of its chain, and a cycle ends.
    #[test]
    fn an_alias_inherits_the_bits_of_its_chain() {
        let mut bits: BTreeMap<String, u16> =
            [("P_", 4), ("MID", 4), ("CALL", 8), ("LOOP_A", 4), ("LOOP_B", 4), ("PLAIN", 4), ("TWO", 4), ("ONE", 4)]
                .into_iter()
                .map(|(name, bit)| (name.to_owned(), bit))
                .collect();
        let aliases: BTreeMap<String, BTreeSet<String>> = [
            ("P_", vec!["MID"]),
            ("MID", vec!["CALL"]),
            ("LOOP_A", vec!["LOOP_B"]),
            ("LOOP_B", vec!["LOOP_A"]),
            ("TWO", vec!["ONE", "CALL"]),
            ("SELF", vec!["SELF"]),
            ("TO_NO_MACRO", vec!["int"]),
        ]
        .into_iter()
        .map(|(name, targets)| (name.to_owned(), targets.into_iter().map(str::to_owned).collect()))
        .collect();
        inherit_alias_bits(&mut bits, &aliases);
        assert_eq!(bits["P_"], 12, "the chain P_, MID, CALL ends at a function-like macro");
        assert_eq!(bits["MID"], 12);
        assert_eq!(bits["CALL"], 8);
        assert_eq!(bits["LOOP_A"], 4, "a cycle ends, and the names keep their bits");
        assert_eq!(bits["LOOP_B"], 4);
        assert_eq!(bits["TWO"], 12, "a name with two targets gets the bits of the two");
        assert_eq!(bits["PLAIN"], 4);
        assert!(!bits.contains_key("SELF"), "an alias of itself adds no bit");
        assert!(!bits.contains_key("TO_NO_MACRO"), "a target that is no macro adds no bit");
    }

    /// A header with no extension starts with a directive, and a script, a Makefile and a document
    /// do not. The texts are the first lines of corpus files.
    #[test]
    fn a_header_with_no_extension_starts_with_a_directive() {
        assert!(starts_with_directive(b"// <vector> -*- C++ -*-\n\n/** @file */\n\n#ifndef _GLIBCXX_VECTOR\n"));
        assert!(starts_with_directive(b"\xef\xbb\xbf#pragma once\n"));
        assert!(starts_with_directive(b"/* x\n */\n  #  include <a>\n"));
        assert!(starts_with_directive(b"%:define A\n"));
        assert!(!starts_with_directive(b"#! /bin/sh\n# Guess values for system-dependent variables\n#define A\n"));
        assert!(!starts_with_directive(b"# Common Makefile\n#define name\n"));
        assert!(!starts_with_directive(b"2024-01-01  Somebody\n#define CAT\n"));
        assert!(!starts_with_directive(b"#%Module\n"));
        assert!(!starts_with_directive(b""));
        assert!(!starts_with_directive(b"#"));
    }

    fn names(source: &str) -> Vec<(String, Shape)> {
        defines(source.as_bytes())
    }

    fn one(name: &str, shape: Shape) -> Vec<(String, Shape)> {
        vec![(name.to_owned(), shape)]
    }

    /// The shape is the character right after the name. [cpp.replace] p10.
    #[test]
    fn a_parenthesis_right_after_the_name_gives_a_function_macro() {
        assert_eq!(names("#define A(x) x\n"), one("A", Shape::Function));
        assert_eq!(names("#define A (x) x\n"), one("A", Shape::Object));
        assert_eq!(names("#define A\n"), one("A", Shape::Object));
        assert_eq!(names("#define A 1"), one("A", Shape::Object));
        // A comment is white space, so the `(` after it starts the replacement list.
        assert_eq!(names("#define A/**/(x) x\n"), one("A", Shape::Object));
        // A splice is removed before the directive is read, so the `(` is right after the name.
        assert_eq!(names("#define A\\\n(x) x\n"), one("A", Shape::Function));
        assert_eq!(names("#define VA(...) __VA_ARGS__\n"), one("VA", Shape::Function));
    }

    /// The spellings of a directive that the preprocessor reads.
    #[test]
    fn the_blanks_the_comments_and_the_digraph_of_a_directive_are_read() {
        assert_eq!(names("  #  define A\n"), one("A", Shape::Object));
        assert_eq!(names("#\tdefine\tA\n"), one("A", Shape::Object));
        assert_eq!(names("/* x */ # /* y */ define /* z */ A\n"), one("A", Shape::Object));
        assert_eq!(names("%:define A\n"), one("A", Shape::Object));
        assert_eq!(names("#de\\\nfine A\n"), one("A", Shape::Object));
        assert_eq!(names("\r\n#define A\r\n#define B(x)\r\n"), vec![
            ("A".to_owned(), Shape::Object),
            ("B".to_owned(), Shape::Function)
        ]);
        // A name with a character that is not ASCII.
        assert_eq!(names("#define Ä\n"), one("Ä", Shape::Object));
    }

    /// A `#` that is not the first token of a line starts no directive.
    #[test]
    fn a_hash_after_a_token_of_the_same_line_is_no_directive() {
        assert!(names("int x; #define A\n").is_empty());
        assert!(names("#define S(x) #x\n#define T(x) a #define B\n").iter().all(|(name, _)| name != "B"));
        // A splice joins the two lines, so the `#` follows a token.
        assert!(names("int x; \\\n#define A\n").is_empty());
        // A comment is one space with no line break, so the `#` follows `x`.
        assert!(names("x /*\n*/ #define A\n").is_empty());
        // A line break before the comment starts a line.
        assert_eq!(names("x\n/*\n*/ #define A\n"), one("A", Shape::Object));
        assert!(names("#defined A\n#definer B\n").is_empty());
        assert!(names("#undef A\n#ifdef B\n").is_empty());
    }

    /// The five things that hide a line from the preprocessor.
    #[test]
    fn a_define_in_a_comment_a_literal_or_a_raw_string_is_no_directive() {
        assert!(names("/*\n#define A\n*/\n").is_empty());
        assert!(names("// x \\\n#define A\n").is_empty());
        assert!(names("const char *s = \"\\\n#define A\";\n").is_empty());
        assert!(names("auto s = R\"(\n#define A\n)\";\n").is_empty());
        assert!(names("auto s = u8R\"sql(\n)\"\n#define A\n)sql\";\n").is_empty());
        // A literal with no closing quote ends at the line break: `#error` text with an apostrophe.
        assert_eq!(names("#error don't\n#define A\n"), one("A", Shape::Object));
        // A digit separator starts no character literal. As a literal,  would close on the
        // quote in the comment, and the comment would not hide the .
        assert!(names("int a = 1'000; /* '\n#define A\n*/\n").is_empty());
        assert_eq!(names("int x = 1'000;\n#define A\nint y = '\\'';\n#define B\n"), vec![
            ("A".to_owned(), Shape::Object),
            ("B".to_owned(), Shape::Object)
        ]);
        // A header name is no comment.
        assert_eq!(names("#include <sys/*.h>\n#define A\n"), one("A", Shape::Object));
        // A comment that the text of a directive opens hides the next line.
        assert!(names("#define A 1 /* x\n#define B\n*/\n").iter().all(|(name, _)| name != "B"));
    }

    /// Every branch of a conditional is read, and an incomplete directive gives nothing.
    #[test]
    fn every_branch_is_read_and_an_incomplete_directive_gives_nothing() {
        assert_eq!(names("#if 0\n#define A\n#else\n#define A(x)\n#endif\n"), vec![
            ("A".to_owned(), Shape::Object),
            ("A".to_owned(), Shape::Function)
        ]);
        assert!(names("#define\n#define 1A\n#define\n").is_empty());
        assert!(names("#define /* A */\nA\n").is_empty());
        assert!(names("").is_empty());
        assert!(names("#").is_empty());
        assert!(names("\"").is_empty());
        assert!(names("R\"x(").is_empty());
        assert!(names("/*").is_empty());
        assert!(names("\\").is_empty());
    }

    /// The shape of the replacement list of each `#define` of a source.
    fn shapes(source: &str) -> Vec<BodyShape> {
        define_lines(source.as_bytes()).iter().map(body_shape).collect()
    }

    /// The shape of the one `#define` of a source.
    fn shape(source: &str) -> BodyShape {
        shapes(source).pop().expect("the source holds one #define")
    }

    /// THE SHAPE OF A BODY READS THE FIRST TOKEN, THE LAST TOKEN AND THE COUNT.
    ///
    /// The lines come from the corpus: `__` of v8 and of hhvm, `nssv_noexcept` of simdjson,
    /// `hb_locale_t` of harfbuzz, `_ZERO_OR_NO_INIT` of the STL, `_STD` of the STL, and
    /// `ccl_private` of blender.
    #[test]
    fn the_shape_of_a_body_reads_its_first_token_its_last_token_and_its_count() {
        assert_eq!(shape("#define NDEBUG\n"), BodyShape::Empty);
        assert_eq!(shape("#define hb_locale_t locale_t\n"), BodyShape::OneName);
        assert_eq!(shape("#define P_ BSLIM_TESTUTIL_P_\n"), BodyShape::OneName);
        assert_eq!(shape("#define nssv_noexcept noexcept\n"), BodyShape::Specifier);
        assert_eq!(shape("#define MY_API __declspec(dllexport)\n"), BodyShape::Specifier);
        assert_eq!(shape("#define BOOST_SYMBOL_VISIBLE __global\n"), BodyShape::Specifier);
        assert_eq!(shape("#define uint64_t __UINT64_TYPE__\n"), BodyShape::OneName);
        assert_eq!(shape("#define hb_locale_t void *\n"), BodyShape::TypeExpression);
        assert_eq!(shape("#define DWORD unsigned long\n"), BodyShape::TypeExpression);
        assert_eq!(shape("#define _STD ::std::\n"), BodyShape::ScopePrefix);
        assert_eq!(shape("#define __ masm->\n"), BodyShape::MemberPrefix);
        assert_eq!(shape("#define __ basm_.\n"), BodyShape::MemberPrefix);
        assert_eq!(shape("#define _ZERO_OR_NO_INIT = 0\n"), BodyShape::Initializer);
        assert_eq!(shape("#define EMPTY_BRACES {}\n"), BodyShape::Initializer);
        assert_eq!(shape("#define __ ACCESS_MASM(masm)\n"), BodyShape::Call("ACCESS_MASM".to_owned()));
        assert_eq!(shape("#define A x FOO\n"), BodyShape::EndsWithName("FOO".to_owned()));
        assert_eq!(shape("#define PI 3.14\n"), BodyShape::Other);
        assert_eq!(shape("#define SCOPE class\n"), BodyShape::Other);
        assert_eq!(shape("#define WORDS a + b\n"), BodyShape::EndsWithName("b".to_owned()));
        assert_eq!(shape("#define SHIFT x << 2\n"), BodyShape::Other);
        // A function-like macro has the shape of its own replacement list, which starts after the
        // parameter list. A call body resolves through it.
        assert_eq!(shape("#define ACCESS_MASM(masm) masm->\n"), BodyShape::MemberPrefix);
        assert_eq!(shape("#define CAT(a, b) a##b\n"), BodyShape::EndsWithName("b".to_owned()));
        // A body of more tokens than the reader keeps takes no shape of a whole body.
        assert_eq!(shape("#define LONG const unsigned long int const unsigned long int const\n"), BodyShape::Other);
    }
}
