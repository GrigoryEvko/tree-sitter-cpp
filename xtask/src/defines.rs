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
fn directive(text: &mut Text, found: &mut Vec<(String, Shape)>) {
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
    if let Ok(macro_name) = String::from_utf8(macro_name) {
        found.push((macro_name, shape));
    }
}

#[cfg(test)]
mod tests {
    use super::{Shape, defines, starts_with_directive};

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
}
