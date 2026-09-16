//! The invariants of the external scanner: of the source of `src/scanner.c`, and of what the
//! scanner does with the seed of a parse.
//!
//! A C compiler cannot compare the length of a string literal in an array with the value of a
//! `#define`, because `strlen` is not a constant expression. These tests read the source and do the
//! comparison, so an invariant that the file depends on cannot fall silently.
//!
//! THE TEST READS THE ARRAYS OF THE FILE AND NOT A LIST OF NAMES. An array that somebody adds next
//! year is covered on the day it is added. A hand-written list of the arrays would cover the arrays
//! of today only, which is the opposite of what these invariants are for.

use regex::Regex;

/// One array of names of `src/scanner.c`, with the entries that a word comparison reads.
struct NameList {
    name: String,
    entries: Vec<Entry>,
}

/// One string literal of an array, with the number of characters that it holds.
struct Entry {
    text: String,
    /// The characters of the entry. An escape such as `\\` is one character and not two.
    length: usize,
}

/// Read the entries of an array that starts at the `{` at `open`, and give the index of its `}`.
///
/// The scan reads a string literal, a character literal, a line comment and a block comment, so a
/// brace or a quotation mark inside any of them does not end the array early.
fn read_body(text: &str, open: usize) -> Option<(Vec<Entry>, usize)> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut entries = Vec::new();
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((entries, index));
                }
                index += 1;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                    index += 1;
                }
                index += 2;
            }
            b'\'' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'\'' {
                    index += if bytes[index] == b'\\' { 2 } else { 1 };
                }
                index += 1;
            }
            b'"' => {
                let start = index + 1;
                let mut escapes = 0usize;
                index += 1;
                while index < bytes.len() && bytes[index] != b'"' {
                    if bytes[index] == b'\\' {
                        escapes += 1;
                        index += 2;
                    } else {
                        index += 1;
                    }
                }
                let literal = &text[start..index];
                entries.push(Entry { text: literal.to_owned(), length: literal.chars().count() - escapes });
                index += 1;
            }
            _ => index += 1,
        }
    }
    None
}

/// Read every `static const char *const NAME[]` array of `text`, with its entries.
fn name_lists(text: &str) -> Vec<NameList> {
    let head = Regex::new(r"static const char \*const (\w+)\[\][^=;]*=\s*\{").expect("the pattern compiles");
    head.captures_iter(text)
        .filter_map(|capture| {
            let whole = capture.get(0).expect("the whole match exists");
            let (entries, _) = read_body(text, whole.end() - 1)?;
            Some(NameList { name: capture[1].to_owned(), entries })
        })
        .collect()
}

/// Read the value of a `#define NAME value` of `text`.
fn define(text: &str, name: &str) -> Option<usize> {
    text.lines().find_map(|line| {
        let value = line.trim().strip_prefix("#define")?.trim_start().strip_prefix(name)?;
        value.starts_with([' ', '\t']).then(|| value.split_whitespace().next())?.and_then(|value| value.parse().ok())
    })
}

#[cfg(test)]
mod tests {
    use super::{define, name_lists};
    use std::fs;

    /// The scan finds an entry that is as long as a cut word.
    ///
    /// The test of the real file passes because the file obeys the invariant, and a test that only
    /// ever sees a file that obeys it cannot show that it would find a file that does not. This test
    /// gives the scan a source of four-character entries with a `MACRO_WORD_SIZE` of 5, which is the
    /// condition that the invariant forbids, and reads the length that the scan gives back.
    #[test]
    fn the_scan_finds_an_entry_as_long_as_a_cut_word() {
        let text = "#define MACRO_WORD_SIZE 5\nstatic const char *const WORDS[] = {\"abcd\", \"abc\", NULL};\n";
        let size = define(text, "MACRO_WORD_SIZE").expect("the define reads");
        let lists = name_lists(text);
        assert_eq!(lists.len(), 1);
        let longest = lists[0].entries.iter().map(|entry| entry.length).max().expect("the list has entries");
        assert_eq!(longest, 4);
        assert!(!(longest < size - 1), "an entry of 4 characters and a cut word of 4 must not pass");
    }

    /// A brace, a quotation mark and a comment inside a body do not end the array early.
    #[test]
    fn the_scan_of_a_body_reads_past_a_comment_and_a_brace() {
        let text = "static const char *const WORDS[] = {\n    // a \" and a } in a line comment\n    \"a}b\", /* \" and } */ \"cd\",\n    NULL,\n};\nstatic const char *const MORE[] = {\"e\", NULL};\n";
        let lists = name_lists(text);
        assert_eq!(lists.len(), 2);
        let entries: Vec<&str> = lists[0].entries.iter().map(|entry| entry.text.as_str()).collect();
        assert_eq!(entries, ["a}b", "cd"]);
        assert_eq!(lists[1].name, "MORE");
    }

    /// An escape is one character of an entry and not two.
    #[test]
    fn an_escape_is_one_character_of_an_entry() {
        let text = r#"static const char *const WORDS[] = {"a\\b", "c\"d", NULL};"#;
        let lists = name_lists(text);
        let lengths: Vec<usize> = lists[0].entries.iter().map(|entry| entry.length).collect();
        assert_eq!(lengths, [3, 3]);
    }

    /// A CUT WORD MUST NEVER EQUAL AN ENTRY OF A NAME LIST.
    ///
    /// `read_word` keeps the first `MACRO_WORD_SIZE - 1` characters of a word, so a word that it cut
    /// holds exactly that many characters. Every entry of every name list is shorter than that
    /// today, and the inequality is what makes `is_macro_name`, `is_type_trait`,
    /// `is_grammar_keyword`, `is_class_key` and every other caller of `word_in` unable to match a
    /// name that the text does not hold.
    ///
    /// THE MARGIN IS ONE CHARACTER AND NOTHING ELSE ENFORCES IT. The longest entry of the file is
    /// `__builtin_ge_synthesizes_from_spaceship` of TYPE_TRAITS at 39, and a cut word holds 40. C++
    /// adds type traits, and this fork adds keywords to GRAMMAR_KEYWORDS. The day an entry of 40
    /// characters arrives, the inequality becomes an equality, a truncated name of 200 bytes can
    /// equal that entry, and the name tests begin to match a name that does not exist. There is no
    /// ERROR node and no differ row for such a match.
    ///
    /// THE SEED HAS NO SUCH GUARANTEE AND CANNOT HAVE ONE. A name list is a closed set that this
    /// fork keeps short. The seed of src/seed.h is an open set whose entries run to exactly
    /// `TS_CPP_SEED_WORD_SIZE - 1`, so a cut word can equal an entry. The seed carries the cut flag
    /// of the `Reader` in the place of this inequality.
    #[test]
    fn no_entry_of_a_name_list_can_equal_a_cut_word() {
        let path = crate::repository().join("src").join("scanner.c");
        let text = fs::read_to_string(&path).expect("src/scanner.c reads");
        let size = define(&text, "MACRO_WORD_SIZE").unwrap_or_else(|| {
            panic!(
                "{} declares no numeric MACRO_WORD_SIZE. This test cannot compare the entries with a \
                 number it cannot read, and it must fail rather than pass in silence.",
                path.display()
            )
        });
        let cut = size - 1;
        let lists = name_lists(&text);

        // The extraction must fail loudly rather than find nothing and report success. A pattern
        // that stops matching after a change of style would otherwise retire the invariant.
        assert!(
            lists.len() >= 30,
            "the scan of {} found {} arrays of names and the file holds at least 30. The pattern no \
             longer matches the source, so the invariant is not checked.",
            path.display(),
            lists.len()
        );
        for expected in ["TYPE_TRAITS", "GRAMMAR_KEYWORDS"] {
            let list = lists.iter().find(|list| list.name == expected);
            assert!(
                list.is_some_and(|list| !list.entries.is_empty()),
                "the scan of {} found no entry of {expected}, so the invariant is not checked",
                path.display()
            );
        }

        for list in &lists {
            for entry in &list.entries {
                assert!(
                    entry.length < cut,
                    "`{}` of {} has {} characters, and `read_word` cuts a word at {cut}. A cut word \
                     holds exactly {cut} characters, so it would equal this entry. `is_macro_name`, \
                     `is_type_trait`, `is_grammar_keyword` and every other caller of `word_in` would \
                     then match a name that the text does not hold, with no ERROR node and no differ \
                     row. Keep the entry shorter than {cut} characters, or raise MACRO_WORD_SIZE and \
                     read the reason for its value at its declaration.",
                    entry.text,
                    list.name,
                    entry.length
                );
            }
        }
    }
}

/// What the scanner does with the seed of a parse.
///
/// THE GATE RUNS NO SEED, so every count of a gate is zero for this whole feature. These tests are
/// the only evidence that the seed reaches the scanner and changes a tree. A count cannot carry it.
#[cfg(test)]
mod seeded {
    use std::fs;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};

    use tree_sitter::{InputEdit, Language, Parser, Point, Tree};

    use crate::seed::Seed;

    /// A seed file in a directory of its own. The directory goes at the end of the test.
    ///
    /// `Seed::read` takes a path, so a test that wants a seed writes one. The tests of
    /// `crate::seed` hold a helper of the same shape for the reader of the format. This one is
    /// separate because that one is private to its module.
    struct SeedFile(PathBuf);

    /// The counter of the directories of the tests. Two tests run at the same time, so each file
    /// takes a directory of its own.
    static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    impl SeedFile {
        fn new(text: &str) -> Self {
            let count = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!("xtask-scanner-{}-{count}", std::process::id()));
            fs::create_dir_all(&directory).expect("the directory of the test");
            let path = directory.join("project.seed");
            let mut file = fs::File::create(&path).expect("the file of the test");
            file.write_all(text.as_bytes()).expect("the text of the test");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for SeedFile {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0.parent().expect("the file is in a directory"));
        }
    }

    /// A parser of this grammar.
    fn parser() -> Parser {
        let mut parser = Parser::new();
        parser.set_language(&Language::new(tree_sitter_cpp::LANGUAGE)).expect("the grammar loads");
        parser
    }

    /// Parse `source` with no seed.
    fn bare(source: &str) -> Tree {
        parser().parse(source, None).expect("the parse ends")
    }

    /// `A(x)` in a position where a declaration cannot start. With `A` a type, the callee is a
    /// `type_identifier` and the call is a functional cast. With `A` a name the parse does not know,
    /// the callee is an `identifier` and the call is a call.
    const CAST: &str = "int f() { return A(x); }\n";

    /// True when the tree reads the callee of the call as a type.
    fn is_cast(tree: &Tree) -> bool {
        tree.root_node().to_sexp().contains("function: (type_identifier)")
    }

    /// THE SEED REACHES THE SCANNER THROUGH THE PUBLIC ENTRY POINT AND CHANGES A TREE.
    ///
    /// This is the one proof in the system that the runtime half and the scanner half are
    /// connected. A test that called the scanner's own setter would prove the reader and nothing
    /// about the runtime, so this one goes through `ts_parser_set_scanner_context` only.
    ///
    /// BOTH ORDERS AROUND `ts_parser_set_language` MUST WORK, and only one of them needs the
    /// runtime to give the context to the scanner right after `create`. A test of the other order
    /// alone would pass with that call missing.
    #[test]
    fn a_seed_changes_a_tree_through_the_public_entry_point_in_both_orders() {
        let file = SeedFile::new("A\ttype\n");
        let seed = Seed::read(file.path()).expect("the seed reads");
        assert_eq!(seed.names(), 1);

        let unseeded = bare(CAST);
        assert!(!is_cast(&unseeded), "with no seed the callee is a name and not a type");

        // The language first, then the context. The runtime already has a scanner, so
        // `ts_parser_set_scanner_context` gives the context to it.
        let mut after = Parser::new();
        after.set_language(&Language::new(tree_sitter_cpp::LANGUAGE)).expect("the grammar loads");
        // SAFETY: `seed` lives to the end of this test and every parse is inside it.
        unsafe { after.set_scanner_context(seed.as_context()) };
        let tree_after = after.parse(CAST, None).expect("the parse ends");

        // The context first, then the language. The parser has no scanner yet, so the runtime must
        // give the context at `create`, which happens inside the first parse.
        let mut before = Parser::new();
        // SAFETY: as above.
        unsafe { before.set_scanner_context(seed.as_context()) };
        before.set_language(&Language::new(tree_sitter_cpp::LANGUAGE)).expect("the grammar loads");
        let tree_before = before.parse(CAST, None).expect("the parse ends");

        assert!(is_cast(&tree_after), "the seed did not reach the scanner when the language came first");
        assert!(is_cast(&tree_before), "the seed did not reach the scanner when the context came first");
        assert_eq!(tree_after.root_node().to_sexp(), tree_before.root_node().to_sexp());
        assert_ne!(tree_after.root_node().to_sexp(), unseeded.root_node().to_sexp());

        // The tree of the seeded parse is the tree that the file gets when it declares the class
        // itself, which is what the seed is for.
        let declared = bare(&format!("struct A {{}};\n{CAST}"));
        assert!(is_cast(&declared));

        // A context of NULL gives exactly the unseeded reading again.
        // SAFETY: a null context reads nothing.
        unsafe { after.set_scanner_context(std::ptr::null()) };
        let again = after.parse(CAST, None).expect("the parse ends");
        assert_eq!(again.root_node().to_sexp(), unseeded.root_node().to_sexp());
    }

    /// A NAME THAT THE SEED DOES NOT HOLD KEEPS THE READING OF A CALL.
    #[test]
    fn a_name_the_seed_does_not_hold_keeps_the_reading_of_a_call() {
        let file = SeedFile::new("A\ttype\n");
        let seed = Seed::read(file.path()).expect("the seed reads");
        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let tree = parser.parse("int f() { return B(x); }\n", None).expect("the parse ends");
        assert!(!is_cast(&tree), "the seed holds `A` and not `B`");
    }

    /// A NAME LONGER THAN THE BUFFER MUST NOT TAKE A SEED ENTRY THAT IS ONLY ITS PREFIX.
    ///
    /// `read_word_sized` keeps the first `TS_CPP_SEED_WORD_SIZE - 1` bytes of a word, so without a
    /// guard a compare would take an entry of exactly that length as the whole name. The entry here
    /// is a real corpus name of 64 bytes, and the source name is that name with a tail. A name list
    /// needs no such guard, because a cut word is longer than every entry of every list. A seed is
    /// an open set whose entries reach the length of the buffer, so `is_seed_type_name` reads
    /// `word_cut` of the `Reader`. Without that read, the longer name here IS read as a type.
    #[test]
    fn a_name_longer_than_the_buffer_does_not_take_an_entry_that_is_its_prefix() {
        const NAME: &str = "BackForwardCacheBrowserTestWithNotRestoredReasonsMaskCrossOrigin";
        assert_eq!(NAME.len(), 64, "the name is the longest that the format takes");
        let file = SeedFile::new(&format!("{NAME}\ttype\n"));
        let seed = Seed::read(file.path()).expect("the seed reads");
        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };

        let exact = parser.parse(format!("int f() {{ return {NAME}(x); }}\n"), None).expect("the parse ends");
        assert!(is_cast(&exact), "a name of exactly the length of an entry must take that entry");

        let longer = parser.parse(format!("int f() {{ return {NAME}Tail(x); }}\n"), None).expect("the parse ends");
        assert!(!is_cast(&longer), "a longer name took a seed entry that is only its prefix");
    }

    /// THE SEED REACHES THE OPERAND OF `alignas`, THE THIRD SOURCE OF THAT ONE LOOKUP.
    ///
    /// The operand is a type-id or a constant expression and a bare name is both. The construct
    /// answers when a template head of the same declaration declares the name, and the file answers
    /// when the scanner recorded a class head of that name. THE PROJECT ANSWERS FOR 102 SITES IN 40
    /// FILES OF THE CORPUS WHERE THE FILE DECLARES THE NAME NOWHERE, and only a seed reaches those.
    ///
    /// THE GATE RUNS NO SEED, so this rule measures zero in every gate. This test is the only
    /// evidence that it does anything at all.
    #[test]
    fn a_seed_reaches_the_operand_of_alignas() {
        const SOURCE: &str = "struct S { alignas(Widget) char a; };\nstruct T { alignas(Other) char b; };\n";
        let file = SeedFile::new("Widget\ttype\n");
        let seed = Seed::read(file.path()).expect("the seed reads");

        let unseeded = bare(SOURCE);
        assert_eq!(
            unseeded.root_node().to_sexp().matches("(type_descriptor").count(),
            0,
            "with no seed the file declares neither name and both operands are expressions"
        );

        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let seeded = parser.parse(SOURCE, None).expect("the parse ends");
        let sexp = seeded.root_node().to_sexp();
        assert_ne!(sexp, unseeded.root_node().to_sexp(), "the seed did not reach the operand");
        // THE SEED NAMES ONE OF THE TWO. `Widget` becomes a type and `Other` stays an expression,
        // so the rule reads the seed and does not simply take every name in that position.
        assert_eq!(sexp.matches("(type_descriptor").count(), 1, "{sexp}");
        assert!(sexp.contains("(alignas_qualifier (type_descriptor type: (type_identifier)))"), "{sexp}");
        assert!(sexp.contains("(alignas_qualifier (identifier))"), "{sexp}");
    }

    /// THE CONTEXT SURVIVES A PARSE THAT REUSES A TREE.
    ///
    /// `serialize` and `deserialize` do not carry the context, and they must not. The buffer is 1 KB
    /// and it runs at every token. THE POINTER SURVIVES TODAY ONLY BECAUSE `deserialize` CLEARS EACH
    /// FIELD OF THE STATE BY NAME AND NEVER WRITES OVER THE WHOLE STRUCT. A `memset` there, which is
    /// the natural thing for the next hand to reach for, would drop the seed in the middle of a
    /// parse. There would be no ERROR node and no differ row, and only the trees would change.
    ///
    /// The source puts the cast after enough text that the reparse reuses the part before it and
    /// restores the scanner from its serialized state to read the rest.
    #[test]
    fn the_context_survives_a_parse_that_reuses_a_tree() {
        let file = SeedFile::new("A\ttype\n");
        let seed = Seed::read(file.path()).expect("the seed reads");
        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };

        let padding = "struct Pad {};\n".repeat(40);
        let first = format!("{padding}int f() {{ return A(x); }}\n");
        let second = format!("{padding}int f() {{ return A(xy); }}\n");
        let mut tree = parser.parse(&first, None).expect("the parse ends");
        assert!(is_cast(&tree), "the first parse reads the cast");

        // One byte goes in after the `x` of the argument. The row is the line of the cast and the
        // column is the offset of the new byte in that line.
        let line = "int f() { return A(x";
        let start = padding.len() + line.len();
        let row = padding.lines().count();
        let column = line.len();
        tree.edit(&InputEdit {
            start_byte: start,
            old_end_byte: start,
            new_end_byte: start + 1,
            start_position: Point::new(row, column),
            old_end_position: Point::new(row, column),
            new_end_position: Point::new(row, column + 1),
        });
        let again = parser.parse(&second, Some(&tree)).expect("the parse ends");
        assert!(
            is_cast(&again),
            "the reparse lost the seed. `deserialize` of src/scanner.c cleared the context, and the \
             trees of an incremental parse then differ from the trees of a full parse with no error \
             anywhere to show it."
        );
    }
}

/// THE ORDER OF THE SCANS OF A FUNCTION THAT TRIES MORE THAN ONE TOKEN.
///
/// The lexer of tree-sitter cannot go back. A scan that reads a character and then gives no token
/// leaves the reader where it stopped. Each scan after it, in the same call, reads from there.
///
/// A function that tries ONE candidate is safe. Its decline returns false to `scan_token`, and the
/// runtime resets the lexer. A function that tries MORE THAN ONE candidate in one call is not safe,
/// because the second candidate reads a position that the first candidate moved.
///
/// `scan_class_macro_mark` gave no token for every class that has a member, which is the common
/// form, and it left the reader in the body of the class. The scan of `MACRO_SCOPE_START` after it
/// read the first group of the body as the arguments of the head macro. The input
/// `class A("x") B { decltype(c)::type d; };` got a `macro_scope_specifier` and a MISSING `::`.
/// GCC and Clang accept that input, and NO FILE OF THE CORPUS HOLDS THE FORM. No count of a gate
/// moves for it. This reader is the only thing that finds it.
///
/// THE READER TAKES THE FUNCTIONS FROM THE FILE AND NOT FROM A LIST OF NAMES. A function that
/// somebody writes next year is covered on the day that it is written.
///
/// THE READER DOES NOT LOOK ACROSS THE CALL BOUNDARY, AND A REVIEWER MUST. It reads one function at
/// a time. It cannot tell whether the CALLER of a scan treats a given result as a stop or as "carry
/// on". A scan that reads characters and gives `INVOCATION_NONE` is safe where every caller stops
/// on that value and unsafe where one caller carries on with the same reader.
///
/// WHAT A REVIEWER MUST CHECK BY HAND, for a scan that reads characters and can give more than one
/// result: for EACH result that means "not this one", find every caller and read what it does next.
/// The result is safe only where every caller stops. Where one caller carries on, the scan must not
/// read a character before it gives that result, or it must restore the reader. Refer to
/// "THE SCANNER CAN REWIND THE LEXER, AT A PRICE" of PROTOCOL.txt.
mod order {
    use regex::Regex;
    use std::collections::BTreeSet;

    /// The two functions that move the reader. Each other name comes from the calls of the file.
    const MOVERS: [&str; 2] = ["step", "advance"];

    /// The function that reads horizontal blanks and no other character.
    ///
    /// A branch that calls this one and gives no token leaves the reader at the first character
    /// that is not a blank. `the_blank_scan_reads_blanks_only` holds this name to its body.
    const BLANK: &str = "skip_blanks";

    /// The functions that read past a gap before they decide.
    const GAP_SKIPS: [&str; 2] = ["skip_gap", "skip_blanks"];

    /// One function of the file.
    pub struct Function {
        pub name: String,
        /// The line of the signature, 0-based.
        pub start: usize,
        /// The line of the closing brace, 0-based.
        pub end: usize,
    }

    /// One branch at the top level of a function.
    pub struct Branch {
        /// The line of the `if`, 1-based.
        pub line: usize,
        /// The text of the `if`, for the message of a fault.
        pub text: String,
    }

    /// Remove the comments, the string literals and the character literals of the whole text.
    ///
    /// A brace inside `'{'` or inside a comment must not count when the reader looks for the end of
    /// a block, and the name of a function never stands in a literal. One stripped text serves the
    /// count of the braces and the search for the calls.
    pub fn strip(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut in_comment = false;
        for line in text.lines() {
            let bytes = line.as_bytes();
            let mut kept = String::with_capacity(line.len());
            let mut index = 0;
            while index < bytes.len() {
                if in_comment {
                    if bytes[index] == b'*' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
                        in_comment = false;
                        index += 2;
                    } else {
                        index += 1;
                    }
                    continue;
                }
                let byte = bytes[index];
                if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
                    break;
                }
                if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
                    in_comment = true;
                    index += 2;
                    continue;
                }
                if byte == b'"' || byte == b'\'' {
                    kept.push(' ');
                    index += 1;
                    while index < bytes.len() {
                        if bytes[index] == b'\\' {
                            index += 2;
                        } else if bytes[index] == byte {
                            index += 1;
                            break;
                        } else {
                            index += 1;
                        }
                    }
                    continue;
                }
                kept.push(if byte.is_ascii() { byte as char } else { ' ' });
                index += 1;
            }
            out.push(kept);
        }
        out
    }

    /// True where the text calls `name`.
    fn calls(text: &str, name: &str) -> bool {
        let pattern = Regex::new(&format!(r"(^|[^A-Za-z0-9_]){name}\s*\(")).expect("the pattern builds");
        pattern.is_match(text)
    }

    /// The name of the function that a signature line declares.
    ///
    /// A REGULAR EXPRESSION CANNOT READ THIS. A pattern of a type and then a name backtracks to the
    /// shortest name that still reaches the `(`, which is the last letter of the real name. The
    /// reader takes the identifier that stands immediately before the first `(` instead.
    fn signature_name(line: &str) -> Option<String> {
        if line.is_empty() || line.starts_with([' ', '\t', '}', '#']) {
            return None;
        }
        let paren = line.find('(')?;
        let before = &line[..paren];
        let name: String = before.chars().rev().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        let name: String = name.chars().rev().collect();
        if !name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') {
            return None;
        }
        // A declaration has a type before the name. A call at column 0 does not, and a macro does
        // not, so the text before the name must hold one.
        let rest = before[..before.len() - name.len()].trim();
        if rest.is_empty() || rest.ends_with([',', '(', '=']) {
            return None;
        }
        Some(name)
    }

    /// The functions of the file. A function starts at column 0 and its closing brace is alone at
    /// column 0.
    pub fn functions(stripped: &[String]) -> Vec<Function> {
        let mut out = Vec::new();
        let mut index = 0;
        while index < stripped.len() {
            if let Some(found) = signature_name(&stripped[index]) {
                let mut open = index;
                while open < stripped.len() && !stripped[open].trim_end().ends_with('{') && open - index < 8 {
                    open += 1;
                }
                if open < stripped.len() && stripped[open].trim_end().ends_with('{') {
                    let mut close = open;
                    while close < stripped.len() && stripped[close] != "}" {
                        close += 1;
                    }
                    if close < stripped.len() {
                        out.push(Function { name: found, start: index, end: close });
                        index = close;
                    }
                }
            }
            index += 1;
        }
        out
    }

    /// The names of the functions that read a character, and of the functions that reach one.
    pub fn advancing(stripped: &[String], all: &[Function]) -> BTreeSet<String> {
        let mut set: BTreeSet<String> = MOVERS.iter().map(|name| name.to_string()).collect();
        loop {
            let mut grew = false;
            for function in all {
                if set.contains(&function.name) {
                    continue;
                }
                let body = stripped[function.start..=function.end].join("\n");
                if set.iter().any(|name| calls(&body, name)) {
                    set.insert(function.name.clone());
                    grew = true;
                }
            }
            if !grew {
                return set;
            }
        }
    }

    /// The names of the functions whose only advance is over horizontal blanks.
    pub fn blank_only(stripped: &[String], all: &[Function], advances: &BTreeSet<String>) -> BTreeSet<String> {
        let mut set: BTreeSet<String> = BTreeSet::new();
        set.insert(BLANK.to_string());
        loop {
            let mut grew = false;
            for function in all {
                if set.contains(&function.name) || !advances.contains(&function.name) {
                    continue;
                }
                let body = stripped[function.start..=function.end].join("\n");
                let called: Vec<&String> =
                    advances.iter().filter(|name| *name != &function.name && calls(&body, name)).collect();
                if !called.is_empty() && called.iter().all(|name| set.contains(*name)) {
                    set.insert(function.name.clone());
                    grew = true;
                }
            }
            if !grew {
                return set;
            }
        }
    }

    /// The last line of the block that starts at `start`, by a count of the braces.
    fn block_end(stripped: &[String], start: usize, limit: usize) -> usize {
        let mut depth = 0i32;
        let mut seen = false;
        let mut index = start;
        while index < limit {
            for character in stripped[index].chars() {
                if character == '{' {
                    depth += 1;
                    seen = true;
                } else if character == '}' {
                    depth -= 1;
                }
            }
            if seen && depth <= 0 {
                return index;
            }
            if !seen && stripped[index].trim_end().ends_with(';') {
                return index;
            }
            index += 1;
        }
        limit
    }

    /// The first line of each statement at the top level of the block of `start`.
    fn statements(stripped: &[String], start: usize, end: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut depth = 0i32;
        for (index, line) in stripped.iter().enumerate().take(end + 1).skip(start) {
            let before = depth;
            for character in line.chars() {
                if character == '{' {
                    depth += 1;
                } else if character == '}' {
                    depth -= 1;
                }
            }
            let text = line.trim();
            if index > start && before == 1 && !text.is_empty() && !text.starts_with('}') {
                out.push(index);
            }
        }
        out
    }

    /// True where the block saves the budget of the reader and gives no token where it changed.
    ///
    /// The budget counts one for each character that the reader takes. An equal budget is the
    /// evidence that a scan read no character. This is the guard of `CLASS_MACRO_MARK`.
    fn has_budget_guard(body: &str) -> bool {
        let save = Regex::new(r"(\w+)\s*=\s*reader\.budget\s*;").expect("the pattern builds");
        let Some(found) = save.captures(body) else {
            return false;
        };
        let name = &found[1];
        let test = Regex::new(&format!(r"reader\.budget\s*!=\s*{name}")).expect("the pattern builds");
        // THE GUARD RETURNS ANY "NO TOKEN" VALUE AND NOT ONLY `false`. A scan that gives an
        // enumerated result says "not this one" with a name of its own.
        test.is_match(body) && body.contains("return")
    }

    /// The branches at the top level of a function.
    pub fn branches(stripped: &[String], function: &Function) -> Vec<Branch> {
        let opener = Regex::new(r"^    (\} else )?if \(").expect("the pattern builds");
        let mut out = Vec::new();
        let mut index = function.start + 1;
        while index < function.end {
            if !opener.is_match(&stripped[index]) {
                index += 1;
                continue;
            }
            let end = block_end(stripped, index, function.end);
            out.push(Branch { line: index + 1, text: stripped[index].trim().chars().take(70).collect() });
            index = end + 1;
        }
        out
    }

    /// The functions that try more than one token in one call.
    ///
    /// A function that gives one symbol reads its text one time and then classifies what it read.
    /// A function that gives two or more tries a candidate, and then another one, and the second
    /// candidate reads the position that the first one left.
    pub fn multiplexers(stripped: &[String], all: &[Function]) -> Vec<usize> {
        let symbol = Regex::new(r"result_symbol\s*=\s*([A-Za-z_][A-Za-z0-9_]*)").expect("the pattern builds");
        let mut out = Vec::new();
        for (index, function) in all.iter().enumerate() {
            let body = stripped[function.start..=function.end].join("\n");
            let names: BTreeSet<&str> =
                symbol.captures_iter(&body).map(|found| found.get(1).expect("the group").as_str()).collect();
            if names.len() >= 2 {
                out.push(index);
            }
        }
        out
    }

    /// The names that hold the lookahead of the start of the scan.
    ///
    /// `scan_word_start` writes `int32_t next = lexer->lookahead;` before it tries any candidate.
    /// That value is the character after the word AT THE START OF THE CALL. A scan that reads
    /// characters and then gives no token makes the value stale, and a branch after it that reads
    /// the name decides on text that the reader already passed.
    fn captured_lookahead(stripped: &[String], function: &Function) -> Vec<(String, usize)> {
        let capture = Regex::new(r"(?:^|[^A-Za-z0-9_])([a-z_][a-z0-9_]*)\s*=\s*lexer->lookahead\s*;").expect("the pattern builds");
        let mut out = Vec::new();
        for (index, line) in stripped.iter().enumerate().take(function.end + 1).skip(function.start) {
            if let Some(found) = capture.captures(line) {
                out.push((found[1].to_string(), index));
            }
        }
        out
    }

    /// The condition of the `if` that starts at `line`, as one text.
    fn condition(stripped: &[String], line: usize, limit: usize) -> String {
        let mut depth = 0i32;
        let mut text = String::new();
        let mut index = line;
        while index < limit {
            for character in stripped[index].chars() {
                if character == '(' {
                    depth += 1;
                } else if character == ')' {
                    depth -= 1;
                }
            }
            text.push_str(&stripped[index]);
            text.push(' ');
            if depth <= 0 && text.contains('(') {
                break;
            }
            index += 1;
        }
        text
    }

    /// The line of the first scan that reads a character on the path that leaves the block with no
    /// token.
    ///
    /// THE SEARCH IS BLIND TO THE RETURN VALUE OF THE SCAN, and it must be. A scan that gives an
    /// enumerated result says "not this one" with a name such as `INVOCATION_NONE`, and the caller
    /// carries on with the same reader. The damage comes from the characters that the scan read and
    /// never from the name of the value. r48 lost 8 corpus tests and 11 snippets to that shape on
    /// 2026-09-16. A search for `return false` finds none of it.
    ///
    /// Three kinds of statement can move the reader on that path.
    /// - A statement of the block. Every path takes it.
    /// - The condition of a branch. The path where the condition is false takes it, and the body of
    ///   the branch did not run, so the body cannot repair it.
    /// - A branch that can fall through. The search goes into it.
    ///
    /// A branch that returns on every path contributes nothing. The path that leaves the block is
    /// the path where its condition was false, and nothing in it ran. A block that saves and tests
    /// `reader.budget` gives no token on that path, so the search does not go into it either.
    fn first_fall_through_advance(
        stripped: &[String],
        start: usize,
        end: usize,
        after: usize,
        advances: &BTreeSet<String>,
        blanks: &BTreeSet<String>,
    ) -> Option<(usize, String)> {
        let opener = Regex::new(r"^\s*(\} else )?if \(").expect("the pattern builds");
        let moving = |text: &str| -> Option<String> {
            let names: Vec<&str> = advances
                .iter()
                .filter(|name| !blanks.contains(*name) && calls(text, name))
                .map(|name| name.as_str())
                .collect();
            (!names.is_empty()).then(|| names.join(", "))
        };
        for index in statements(stripped, start, end) {
            // THE SEARCH STARTS AFTER THE LINE THAT TOOK THE LOOKAHEAD. `scan_word_start` calls
            // `read_word_sized` before it takes `next`, and that call reads the whole word. A search
            // from the top of the function stops at that call, which is correct and reads nothing
            // stale, and it would hide every scan after it.
            if index <= after {
                continue;
            }
            if !opener.is_match(&stripped[index]) {
                if let Some(names) = moving(&stripped[index]) {
                    return Some((index + 1, names));
                }
                continue;
            }
            let close = block_end(stripped, index, end);
            if let Some(names) = moving(&condition(stripped, index, end)) {
                return Some((index + 1, names));
            }
            let body = stripped[index..=close].join("\n");
            if has_budget_guard(&body) {
                continue;
            }
            let inner = statements(stripped, index, close);
            let returns = match inner.last() {
                Some(last) => stripped[*last].trim().starts_with("return"),
                None => false,
            };
            if returns {
                continue;
            }
            if let Some(found) = first_fall_through_advance(stripped, index, close, after, advances, blanks) {
                return Some(found);
            }
        }
        None
    }

    /// The advancing functions that a block calls on the path that leaves it with no token.
    ///
    /// A call inside a branch that always returns is not on that path. A call that stands as a
    /// statement of the block is.
    fn fall_through_advances(stripped: &[String], start: usize, end: usize, advances: &BTreeSet<String>) -> BTreeSet<String> {
        let opener = Regex::new(r"^\s*(\} else )?if \(").expect("the pattern builds");
        let mut out = BTreeSet::new();
        for index in statements(stripped, start, end) {
            if opener.is_match(&stripped[index]) {
                let close = block_end(stripped, index, end);
                let text = condition(stripped, index, end);
                for name in advances.iter().filter(|name| calls(&text, name)) {
                    out.insert(name.clone());
                }
                let inner = statements(stripped, index, close);
                let returns = match inner.last() {
                    Some(last) => stripped[*last].trim().starts_with("return"),
                    None => false,
                };
                if !returns {
                    out.extend(fall_through_advances(stripped, index, close, advances));
                }
            } else {
                for name in advances.iter().filter(|name| calls(&stripped[index], name)) {
                    out.insert(name.clone());
                }
            }
        }
        out
    }

    /// The faults of one function.
    ///
    /// THE READER ENFORCES THE TWO SHAPES THAT GAVE REAL DEFECTS, AND NOT A GENERAL RULE.
    ///
    /// It cannot decide in general whether the text after a branch is written for a reader that
    /// moved. `scan_macro_start` reads a group in a condition and then continues on purpose, and
    /// every line after it reads the `Arguments` and the `Gap` that the scan filled. The old
    /// `scan_word_start` did the same thing and then read `next`, a character that it took before
    /// any scan ran. The two look identical to a reader of the source. Only the use of the stale
    /// value tells them apart, so that use is what the first rule finds.
    ///
    /// Rule one: a name that holds the lookahead of the start of the call must not be read after a
    /// branch whose condition reads a character.
    ///
    /// Rule two: a branch that reads horizontal blanks and gives no token arms a rule for the rest
    /// of the function. THE SAFETY OF SUCH A BRANCH IS A PROPERTY OF THE SCANS THAT FOLLOW IT, and
    /// nothing holds those scans to it. Each branch after it must skip its own gap before it reads
    /// the lookahead.
    pub fn faults(
        stripped: &[String],
        function: &Function,
        advances: &BTreeSet<String>,
        blanks: &BTreeSet<String>,
    ) -> Vec<String> {
        let mut out = Vec::new();
        for (name, written) in captured_lookahead(stripped, function) {
            let Some((line, callee)) =
                first_fall_through_advance(stripped, function.start, function.end, written, advances, blanks)
            else {
                continue;
            };
            let read = Regex::new(&format!(r"(^|[^A-Za-z0-9_]){name}([^A-Za-z0-9_]|$)")).expect("the pattern builds");
            for (index, text) in stripped.iter().enumerate().take(function.end).skip(line) {
                if read.is_match(text) {
                    out.push(format!(
                        "{}:{} reads `{name}`, the lookahead of line {}, after the scan of line {line} ({callee}) \
                         read characters and could give no token",
                        function.name,
                        index + 1,
                        written + 1,
                    ));
                    break;
                }
            }
        }
        let list = branches(stripped, function);
        let mut armed: Option<usize> = None;
        for branch in &list {
            let close = block_end(stripped, branch.line - 1, function.end);
            let moves = fall_through_advances(stripped, branch.line - 1, close, advances);
            if moves.is_empty() || !moves.iter().all(|name| blanks.contains(name)) {
                continue;
            }
            armed = Some(branch.line);
            break;
        }
        let Some(armed) = armed else {
            return out;
        };
        for branch in &list {
            if branch.line <= armed {
                continue;
            }
            let close = block_end(stripped, branch.line - 1, function.end);
            let body = stripped[branch.line - 1..=close].join("\n");
            let Some(read) = body.find("lookahead") else {
                continue;
            };
            let skipped = GAP_SKIPS.iter().filter_map(|name| body.find(&format!("{name}("))).min();
            if skipped.is_none_or(|at| at > read) {
                out.push(format!(
                    "{}:{} reads the lookahead after the blank scan of line {armed} and skips no gap first: {}",
                    function.name, branch.line, branch.text
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod order_tests {
    use super::order;
    use std::fs;

    /// A source with the movers and a multiplexer of the given body.
    ///
    /// THE MULTIPLEXER READS ITS WORD BEFORE THE BODY, as `scan_word_start` does with
    /// `read_word_sized`. That call reads characters and it is correct, because every line after it
    /// is written for it. A reader that searched from the top of the function would stop there and
    /// hide every scan that follows. This preamble is what holds the search to the right place.
    fn source(body: &str) -> String {
        format!(
            "static void step(Reader *reader) {{\n    reader->budget--;\n    advance(reader->lexer);\n}}\n\
             static void skip_blanks(Reader *reader) {{\n    while (reader->lexer->lookahead == ' ') {{\n        step(reader);\n    }}\n}}\n\
             static void read_word(Reader *reader, char *word) {{\n    step(reader);\n}}\n\
             static bool skip_group(Reader *reader) {{\n    step(reader);\n    return false;\n}}\n\
             static bool scan_two(Reader *reader, const bool *valid_symbols) {{\n\
             \x20   char word[64];\n    read_word(reader, word);\n{body}\n    return false;\n}}\n"
        )
    }

    /// Read a source and give the faults of its last function.
    fn faults(text: &str) -> Vec<String> {
        let stripped = order::strip(text);
        let all = order::functions(&stripped);
        let advances = order::advancing(&stripped, &all);
        let blanks = order::blank_only(&stripped, &all, &advances);
        let last = all.last().expect("the source holds a function");
        order::faults(&stripped, last, &advances, &blanks)
    }

    /// THE READER FINDS A BRANCH THAT DECIDES ON A LOOKAHEAD THAT A SCAN MADE STALE.
    ///
    /// The file obeys the invariant today, so a test of the file alone cannot show that the reader
    /// would find a file that does not. This source holds the shape that the invariant forbids, and
    /// it is the shape that `scan_word_start` had: a name that takes the lookahead before any scan,
    /// a scan in a condition that reads characters and can give no token, and then a branch that
    /// decides on that name.
    #[test]
    fn the_reader_finds_a_stale_lookahead_after_a_scan_that_reads() {
        let text = source(
            "    int32_t next = lexer->lookahead;\n\
             \x20   if (valid_symbols[0] && skip_group(reader)) {\n\
             \x20       lexer->result_symbol = A;\n\
             \x20       return true;\n    }\n\
             \x20   if (valid_symbols[1] && next == '(') {\n\
             \x20       lexer->result_symbol = B;\n\
             \x20       return true;\n    }",
        );
        let found = faults(&text);
        assert_eq!(found.len(), 1, "the reader gives one fault, and gave {found:?}");
        assert!(found[0].contains("skip_group"), "the fault names the scan that reads: {found:?}");
        assert!(found[0].contains("next"), "the fault names the stale lookahead: {found:?}");
    }

    /// THE READER FINDS A SCAN WHOSE RESULT IS AN ENUMERATED VALUE AND NOT `false`.
    ///
    /// r48 hit this on 2026-09-16. Its probe read a word and gave `INVOCATION_NONE` on failure,
    /// which is the value that the branch around it already gave, so the value looked harmless. The
    /// CALLER treats `INVOCATION_NONE` as "carry on" and scans on with the same reader, and the
    /// reader had eaten a name. 8 corpus tests and 11 snippets failed at once, among them
    /// `STACK_OF(X509) sk;` and `NS_IMETHOD_(void) Unlink(void *p) = 0;`.
    ///
    /// THE DAMAGE WAS THE CONSUMPTION AND NOT THE VALUE. A search for `return false` finds none of
    /// this, so the reader looks at what a scan READ and never at what it gave back.
    #[test]
    fn the_reader_finds_a_scan_that_gives_an_enumerated_value() {
        let text = source(
            "    int32_t next = lexer->lookahead;\n\
             \x20   Invocation result = skip_group(reader);\n\
             \x20   if (result == INVOCATION_TOKEN) {\n\
             \x20       lexer->result_symbol = A;\n\
             \x20       return true;\n    }\n\
             \x20   if (valid_symbols[1] && next == '(') {\n\
             \x20       lexer->result_symbol = B;\n\
             \x20       return true;\n    }",
        );
        let found = faults(&text);
        assert_eq!(found.len(), 1, "the reader gives one fault, and gave {found:?}");
        assert!(found[0].contains("skip_group"), "the fault names the scan that reads: {found:?}");
    }

    /// A block that saves the budget and gives no token where it changed is correct.
    ///
    /// This is the repair of `scan_class_macro_mark`. The scan still reads the head of the class,
    /// and the branch after it still reads `next`, but no path reaches that branch with a reader
    /// that moved.
    #[test]
    fn the_reader_accepts_a_branch_that_guards_the_budget() {
        let text = source(
            "    int32_t next = lexer->lookahead;\n\
             \x20   if (valid_symbols[0]) {\n\
             \x20       uint32_t start_budget = reader.budget;\n\
             \x20       if (skip_group(reader)) {\n\
             \x20           lexer->result_symbol = A;\n\
             \x20           return true;\n        }\n\
             \x20       if (reader.budget != start_budget) {\n\
             \x20           return false;\n        }\n    }\n\
             \x20   if (valid_symbols[1] && next == '(') {\n\
             \x20       lexer->result_symbol = B;\n\
             \x20       return true;\n    }",
        );
        assert!(faults(&text).is_empty(), "the guard makes the branch correct: {:?}", faults(&text));
    }

    /// THE SAFETY OF A BLANK SCAN IS A PROPERTY OF THE SCANS THAT FOLLOW IT.
    ///
    /// A branch that reads horizontal blanks and falls through is permitted, because every scan of
    /// this file skips its own gap. That safety lives in the CALLERS THAT COME AFTER, and nothing
    /// holds them to it. The reader holds them to it.
    #[test]
    fn the_reader_finds_a_lookahead_read_after_a_blank_scan() {
        let text = source(
            "    if (valid_symbols[0]) {\n\
             \x20       skip_blanks(reader);\n\
             \x20       if (lexer->lookahead != '(') {\n\
             \x20           return true;\n        }\n    }\n\
             \x20   if (valid_symbols[1] && lexer->lookahead == '<') {\n\
             \x20       lexer->result_symbol = B;\n\
             \x20       return true;\n    }",
        );
        let found = faults(&text);
        assert_eq!(found.len(), 1, "the reader gives one fault, and gave {found:?}");
        assert!(found[0].contains("skips no gap first"), "the fault names the rule: {found:?}");
    }

    /// A branch after a blank scan that skips its own gap first is correct.
    #[test]
    fn the_reader_accepts_a_gap_skip_before_the_lookahead() {
        let text = source(
            "    if (valid_symbols[0]) {\n\
             \x20       skip_blanks(reader);\n\
             \x20       if (lexer->lookahead != '(') {\n\
             \x20           return true;\n        }\n    }\n\
             \x20   if (valid_symbols[1]) {\n\
             \x20       skip_gap(reader);\n\
             \x20       if (lexer->lookahead == '<') {\n\
             \x20           return true;\n        }\n    }",
        );
        assert!(faults(&text).is_empty(), "a gap skip before the read is correct: {:?}", faults(&text));
    }

    /// `skip_blanks` reads spaces and tabs and no other character.
    ///
    /// `order::BLANK` names this function, and the name is the one declaration of the reader. This
    /// test holds the name to the body, so a `skip_blanks` that grows a second reader operation
    /// fails here and not in the trees.
    #[test]
    fn the_blank_scan_reads_blanks_only() {
        let path = crate::repository().join("src").join("scanner.c");
        let text = fs::read_to_string(&path).expect("src/scanner.c reads");
        let stripped = order::strip(&text);
        let all = order::functions(&stripped);
        let function = all
            .iter()
            .find(|function| function.name == "skip_blanks")
            .expect("src/scanner.c holds skip_blanks");
        let body: Vec<&str> = text.lines().take(function.end + 1).skip(function.start).collect();
        let body = body.join("\n");
        assert!(body.contains("== ' '"), "skip_blanks reads a space: {body}");
        assert!(body.contains("== '\\t'"), "skip_blanks reads a tab: {body}");
        for name in ["skip_group", "read_word", "skip_gap", "read_token"] {
            assert!(!body.contains(&format!("{name}(")), "skip_blanks must not call {name}: {body}");
        }
    }

    /// NO SCAN OF THE FILE READS A CHARACTER AND THEN LETS ANOTHER SCAN READ FROM THERE.
    ///
    /// The reader takes the functions that try more than one token from the file itself. A
    /// multiplexer that somebody writes next year is covered on the day that it is written.
    #[test]
    fn no_scan_of_the_file_reads_a_character_and_falls_through() {
        let path = crate::repository().join("src").join("scanner.c");
        let text = fs::read_to_string(&path).expect("src/scanner.c reads");
        let stripped = order::strip(&text);
        let all = order::functions(&stripped);
        let advances = order::advancing(&stripped, &all);
        let blanks = order::blank_only(&stripped, &all, &advances);
        let mut found = Vec::new();
        for index in order::multiplexers(&stripped, &all) {
            found.extend(order::faults(&stripped, &all[index], &advances, &blanks));
        }
        assert!(
            found.is_empty(),
            "a scan reads a character and then gives no token, and the scan after it reads from \
             where that scan stopped. The lexer cannot go back, so the second scan gives a token \
             for text that is not its own. Refer to the module comment of `order`.\n{}",
            found.join("\n")
        );
    }
}
