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
