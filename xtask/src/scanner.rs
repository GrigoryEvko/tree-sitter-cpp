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
            // The reader refuses a file with no format row, so the helper writes the row first.
            file.write_all(format!("{}\n{text}", crate::seed::format_row()).as_bytes()).expect("the text of the test");
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

    /// THE SEED REACHES THE TYPE BEFORE A DECLARATOR AND A MACRO, THE THIRD SOURCE OF THAT LOOKUP.
    ///
    /// `hb_codepoint_t glyph HB_UNUSED` reads as the attribute macro `hb_codepoint_t`, the type
    /// `glyph` and the declarator `HB_UNUSED` when no source declares the first name. The template
    /// head of the same declaration is the first source, the class heads of the file are the
    /// second, and a typedef is in neither. The seed of harfbuzz holds `hb_codepoint_t` as a type,
    /// and 159 sites of the corpus at 3977e59 have this name in this shape. Refer to
    /// `is_declared_type_name` in src/scanner.c.
    ///
    /// THE GATE RUNS NO SEED, so this source measures zero in every gate. This test is the only
    /// evidence in the test suite that it does anything at all.
    #[test]
    fn a_seed_reaches_the_type_before_a_declarator_and_a_macro() {
        const SOURCE: &str = "void f(hb_codepoint_t glyph HB_UNUSED, hb_other_t other HB_UNUSED);\n";
        let file = SeedFile::new("hb_codepoint_t\ttype\n");
        let seed = Seed::read(file.path()).expect("the seed reads");

        let unseeded = bare(SOURCE);
        assert_eq!(
            unseeded.root_node().to_sexp().matches("(attributed_declarator").count(),
            0,
            "with no seed the file declares neither name and each parameter reads the first name as a macro"
        );

        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let seeded = parser.parse(SOURCE, None).expect("the parse ends");
        let sexp = seeded.root_node().to_sexp();
        assert_ne!(sexp, unseeded.root_node().to_sexp(), "the seed did not reach the type");
        // THE SEED NAMES ONE OF THE TWO. `hb_codepoint_t` becomes the type, with `glyph` as the
        // declarator and `HB_UNUSED` as its attribute macro, and the second parameter keeps the
        // reading with the macro first, so the rule reads the seed and does not take every name.
        assert_eq!(sexp.matches("(attributed_declarator").count(), 1, "{sexp}");
        assert!(
            sexp.contains(
                "(parameter_declaration type: (type_identifier) declarator: (attributed_declarator (identifier) \
                 (attribute_macro name: (identifier))))"
            ),
            "{sexp}"
        );
        assert!(
            sexp.contains("(parameter_declaration (attribute_macro name: (identifier)) type: (type_identifier) declarator: (identifier))"),
            "{sexp}"
        );
    }

    /// A FUNCTION-MACRO ROW READS A NAME WITH A LOWERCASE LETTER AS A MACRO WHERE AN ITEM STARTS.
    ///
    /// A translation unit and a namespace body hold declarations and no statement, so `name(args);` there is
    /// no call. 00e3c7a reads the name with no lowercase letter as a macro, and the shape of the name is its
    /// evidence. `PyDoc_STRVAR(doc, "text");` and `TF_CALL_half(REGISTER);` hold a lowercase letter, and the
    /// row of the seed is the evidence in their place. Refer to `scan_macro_invocation` in src/scanner.c.
    ///
    /// EACH LINE OF THE SOURCE NAMES AN INPUT THAT ONE TEST OF THE RULE DECIDES:
    /// - The four lines with `PyDoc_STRVAR` and `TF_CALL_half` take the reading, at file scope, in a
    ///   namespace body, in a linkage body and in a conditional group. In the group, a first version of this
    ///   rule gave no token, because the branch of a call before the directive that ends a branch read the
    ///   group and returned false. That branch now keeps its read and lets this rule run.
    /// - `common();` keeps its call, because no row of the seed holds `common`. The corpus holds 38 such
    ///   sites in the boost type-traits tests, where `TT_TEST_BEGIN(...)` opens a function body in its
    ///   expansion and the call stands in that body.
    /// - `printf("good");` keeps its call, because the row is object-like and an object-like macro takes no
    ///   arguments ([cpp.replace] p10). The corpus holds 7 such sites, in the Clang interpreter tests.
    /// - `Widget(x);` keeps the reading of the base, because a class head of the file declares `Widget`.
    ///   The two lookups of 00e3c7a give that guard, and the one C++ reading of that text is a declaration
    ///   of `x`, which no commit reads yet.
    /// - `TF_CALL_half(*p);` keeps its declaration, because the group has the shape of a parenthesized
    ///   declarator.
    /// - `TF_CALL_half(REGISTER);` in a function body keeps its call, because no item starts there.
    ///
    /// THE GATE RUNS NO SEED, so this rule measures zero in the plain steps of the gate, and this test and a
    /// seeded run of `xtask trees --seeds` are the evidence.
    #[test]
    fn a_function_macro_row_reads_a_name_with_a_lowercase_letter_as_a_macro_where_an_item_starts() {
        const SOURCE: &str = "PyDoc_STRVAR(doc, \"text\");\nnamespace ns { TF_CALL_half(REGISTER); }\n\
                              extern \"C\" { TF_CALL_half(REGISTER); }\n#if GOOGLE_CUDA\n\
                              TF_CALL_half(REGISTER);\n#endif\n";
        // Each line here keeps the tree of a parse with no seed.
        const KEPT: &str = "common();\nprintf(\"good\");\nclass Widget { int m; };\nWidget(x);\n\
                            TF_CALL_half(*p);\nvoid f() { TF_CALL_half(REGISTER); }\n";
        // The rows of a seed ascend by the bytes of the name.
        let file = SeedFile::new(
            "PyDoc_STRVAR\tfunction-macro\nTF_CALL_half\tfunction-macro\nWidget\tfunction-macro\nprintf\tobject-macro\n",
        );
        let seed = Seed::read(file.path()).expect("the seed reads");
        let unseeded = bare(SOURCE).root_node().to_sexp();
        assert_eq!(unseeded.matches("(macro_invocation").count(), 0, "{unseeded}");

        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let seeded = parser.parse(SOURCE, None).expect("the parse ends");
        let sexp = seeded.root_node().to_sexp();
        assert!(!seeded.root_node().has_error(), "{sexp}");
        // The four sites with a function row take the reading, and no call expression stays.
        assert_eq!(sexp.matches("(macro_invocation").count(), 4, "{sexp}");
        assert_eq!(sexp.matches("(call_expression").count(), 0, "{sexp}");
        // Each line of KEPT gives the tree of a parse with no seed.
        let kept = parser.parse(KEPT, None).expect("the parse ends").root_node().to_sexp();
        assert_eq!(kept, bare(KEPT).root_node().to_sexp(), "a line of KEPT changed with the seed");
        assert_eq!(kept.matches("(expression_statement (call_expression").count(), 4, "{kept}");
        // `TF_CALL_half(*p);` keeps its declaration with a parenthesized declarator.
        assert!(
            kept.contains(
                "(declaration type: (type_identifier) declarator: (parenthesized_declarator declarator: (pointer_declarator declarator: (identifier))))"
            ),
            "{kept}"
        );
    }

    /// A MACRO ROW REACHES THE TYPE BEFORE A DECLARATOR AND A MACRO WHEN NO SOURCE DECLARES THE TYPE.
    ///
    /// `Mutex mu MOZ_UNANNOTATED;` reads as the attribute macro `Mutex`, the type `mu` and the
    /// declarator `MOZ_UNANNOTATED` when no source declares `Mutex`. The type rows of firefox hold no
    /// `Mutex`, because a template parameter of that name removes it. The macro row of
    /// `MOZ_UNANNOTATED` is the evidence at the position of the macro. Refer to
    /// `scan_template_parameter_declarator` in src/scanner.c.
    ///
    /// EACH LINE OF THE SOURCE NAMES AN INPUT THAT ONE TEST OF THE RULE DECIDES:
    /// - The field, the declaration and the parameter with `MOZ_UNANNOTATED` take the reading.
    /// - `Other o NOT_DEFINED;` keeps the reading of today, because `NOT_DEFINED` has no row.
    /// - `Kind k AS_TYPE;` keeps the reading of today, because a type row is no macro row.
    ///
    /// THE GATE RUNS NO SEED, so this rule measures zero in the plain steps of the gate, and this test
    /// and a seeded run of `xtask trees --seeds` are the evidence.
    #[test]
    fn a_macro_row_reaches_the_type_before_a_declarator_and_a_macro() {
        const SOURCE: &str = "class A {\n  Mutex mu MOZ_UNANNOTATED;\n  Other o NOT_DEFINED;\n  Kind k AS_TYPE;\n};\n\
                              Mutex global MOZ_UNANNOTATED;\n\
                              void f(Mutex m MOZ_UNANNOTATED);\n";
        let file = SeedFile::new("AS_TYPE\ttype\nMOZ_UNANNOTATED\tobject-macro\n");
        let seed = Seed::read(file.path()).expect("the seed reads");

        let unseeded = bare(SOURCE);
        let before = unseeded.root_node().to_sexp();
        assert_eq!(before.matches("(attributed_declarator").count(), 0, "{before}");

        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let seeded = parser.parse(SOURCE, None).expect("the parse ends");
        let sexp = seeded.root_node().to_sexp();
        assert!(!seeded.root_node().has_error(), "{sexp}");
        // Three sites with the macro row take the reading, and the two other fields do not.
        assert_eq!(sexp.matches("(attributed_declarator").count(), 3, "{sexp}");
        for kind in ["field_declaration", "declaration", "parameter_declaration"] {
            let name = if kind == "field_declaration" { "field_identifier" } else { "identifier" };
            assert!(
                sexp.contains(&format!(
                    "({kind} type: (type_identifier) declarator: (attributed_declarator ({name}) \
                     (attribute_macro name: (identifier))))"
                )),
                "{kind}: {sexp}"
            );
        }
    }

    /// THE TEXT AFTER THE MACROS ENDS THE DECLARATOR, AND A QUALIFIER ENDS IT ONLY AFTER A PARAMETER LIST.
    ///
    /// For each line with the reading, the tree with the macro rows is the tree of the same line with the
    /// macro names deleted, plus the macro nodes and the `attributed_declarator` of a variable. Refer to
    /// `scan_template_parameter_declarator` in src/scanner.c:
    /// - `static I min PREVENT () const` and `I f PREVENT (int x) noexcept`: a qualifier after a group
    ///   that holds no expression.
    /// - `I f PREVENT () &` and `virtual I f PREVENT () = 0;`: a ref-qualifier and a pure specifier.
    /// - `Agg good1 ABSL_ATTRIBUTE_UNUSED = {1, 2};` and `void c(T value ABSL_ATTRIBUTE_UNUSED = 1);`: a
    ///   `=` after the macro.
    /// - `Agg threadlocal_data_ CACHELINE_ALIGNED ATTR_INITIAL_EXEC;`: a second macro.
    ///
    /// EACH LINE WITHOUT THE READING NAMES AN INPUT THAT ONE TEST DECIDES, and it keeps the tree of a parse
    /// with no seed:
    /// - `T f PREVENT (x) const`: a group of expressions before `const` is the arguments of the macro and
    ///   no parameter list. Without the test, the line gets an ERROR node.
    /// - `T f PREVENT (x) &`: the same test for the ref-qualifier.
    /// - `void b(T value PREVENT x);`: a name after the macro ends no declarator.
    #[test]
    fn a_qualifier_ends_the_declarator_before_a_macro_only_after_a_parameter_list() {
        let file = SeedFile::new(
            "ABSL_ATTRIBUTE_UNUSED\tobject-macro\nATTR_INITIAL_EXEC\tobject-macro\nCACHELINE_ALIGNED\tobject-macro\n\
             PREVENT\tobject-macro\n",
        );
        let seed = Seed::read(file.path()).expect("the seed reads");
        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let macros = [" PREVENT", " ABSL_ATTRIBUTE_UNUSED", " CACHELINE_ALIGNED", " ATTR_INITIAL_EXEC"];
        for source in [
            "class A { static I min PREVENT () const { return 1; } };\n",
            "class A { I f PREVENT (int x) noexcept { return x; } };\n",
            "class A { I f PREVENT () & { return 1; } };\n",
            "class A { virtual I f PREVENT () = 0; };\n",
            "Agg good1 ABSL_ATTRIBUTE_UNUSED = {1, 2};\n",
            "void c(T value ABSL_ATTRIBUTE_UNUSED = 1);\n",
            "Agg threadlocal_data_ CACHELINE_ALIGNED ATTR_INITIAL_EXEC;\n",
        ] {
            let seeded = parser.parse(source, None).expect("the parse ends");
            let mut sexp = seeded.root_node().to_sexp();
            assert!(!seeded.root_node().has_error(), "{source}{sexp}");
            let deleted = macros.iter().fold(source.to_owned(), |text, name| text.replace(name, ""));
            let deleted = parser.parse(&deleted, None).expect("the parse ends").root_node().to_sexp();
            let count = macros.iter().filter(|name| source.contains(*name)).count();
            let markers = " (attribute_macro name: (identifier))".repeat(count);
            assert_eq!(sexp.matches(&markers).count(), 1, "{source}{sexp}");
            for name in ["identifier", "field_identifier"] {
                sexp = sexp.replace(&format!("(attributed_declarator ({name}){markers})"), &format!("({name})"));
            }
            // The macro of a function is a child of its `function_declarator`.
            assert_eq!(sexp.replace(&markers, ""), deleted, "{source}");
        }
        for source in [
            "class A { T f PREVENT (x) const { return 1; } };\n",
            "class A { T f PREVENT (x) & { return 1; } };\n",
            "void b(T value PREVENT x);\n",
        ] {
            let seeded = parser.parse(source, None).expect("the parse ends");
            assert!(!seeded.root_node().has_error(), "{source}{}", seeded.root_node().to_sexp());
            assert_eq!(seeded.root_node().to_sexp(), bare(source).root_node().to_sexp(), "{source}");
        }
    }

    /// A FIRST NAME WITH A MACRO ROW DOES NOT ENTER, SO ITS INVOCATION AND ITS CONSTRUCTOR MACRO STAY.
    ///
    /// The scan reads the blank after the first name before it can see the `(` of an invocation or
    /// the `::` of a constructor. A decline after that blank stops `scan_macro_start`, and no branch
    /// can give its empty token after the scan of a comparison marked the end after the name. So a
    /// first name with a macro row does not enter on the macro rows. `ClassDef (A, 1)` in a class body
    /// and `api Widget::Widget() {}` keep their macro with a seed that holds a row for `ClassDef` and
    /// for `api`. Without the test, 46 corpus files lost such a macro and 10 got an ERROR node.
    #[test]
    fn a_first_name_with_a_macro_row_keeps_its_invocation_and_its_constructor_macro() {
        const SOURCE: &str = "class A {\n  int x;\n  ClassDef (A, 1)\n};\napi Widget::Widget() {}\n";
        let file = SeedFile::new("ClassDef\tfunction-macro\nMOZ_UNANNOTATED\tobject-macro\napi\tobject-macro\n");
        let seed = Seed::read(file.path()).expect("the seed reads");
        let unseeded = bare(SOURCE).root_node().to_sexp();
        assert!(unseeded.contains("(macro_invocation"), "{unseeded}");
        assert_eq!(unseeded.matches("(attribute_macro name: (identifier))").count(), 1, "{unseeded}");

        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let seeded = parser.parse(SOURCE, None).expect("the parse ends");
        assert_eq!(seeded.root_node().to_sexp(), unseeded, "the seed changed the tree of a name with a macro row");
    }

    /// WITH NO MACRO ROW FOR ANY NAME, A SEED CHANGES NO TREE OF THIS CONSTRUCT, AND WITH NO SEED THE RULE
    /// DOES NOT ENTER.
    ///
    /// `has_seed` comes before the scan reads a character. Without it, the decline of each plain type
    /// name that a blank follows would stop the later scans of a parse with no seed.
    #[test]
    fn the_rule_of_the_macro_rows_reads_no_character_with_no_seed() {
        const SOURCE: &str = "class A {\n  Mutex mu MOZ_UNANNOTATED;\n  ClassDef (A, 1)\n};\napi Widget::Widget() {}\n";
        let unseeded = bare(SOURCE).root_node().to_sexp();
        assert!(unseeded.contains("(macro_invocation"), "{unseeded}");
        assert_eq!(unseeded.matches("(attributed_declarator").count(), 0, "{unseeded}");
        // A context that is no seed, as `test_a_scanner_context_does_not_reach_an_upstream_grammar`
        // gives it, reads as no seed.
        let marker = 42u32;
        let mut parser = parser();
        // SAFETY: `marker` lives to the end of this test, and the scanner reads its magic and stops.
        unsafe { parser.set_scanner_context(std::ptr::addr_of!(marker).cast()) };
        let other = parser.parse(SOURCE, None).expect("the parse ends").root_node().to_sexp();
        assert_eq!(other, unseeded);
    }

    /// AN OBJECT-LIKE MACRO OF THE SEED GIVES THE GROUP AFTER IT TO THE DECLARATOR, AS THE TEXT WITHOUT THE
    /// MACRO DOES.
    ///
    /// An object-like macro takes no arguments ([cpp.replace] p10). For each line with an object row, the
    /// tree with the macro is the tree of the same line with the macro name deleted, plus the macro node
    /// and its `attributed_declarator`. The test deletes the name and compares the two trees, so it pins
    /// no tree of its own:
    /// - `Mutex l1 MOZ_UNANNOTATED("autolock");` in a block is a variable with an initializer.
    /// - `Monitor monitor MOZ_UNANNOTATED(__func__);` in a block takes `NAME_INITIALIZER_IN_BLOCK`, and
    ///   outside a block it is a function with a parameter.
    /// - `Stream in MOZ_UNANNOTATED(0);` outside a block is a variable with an initializer.
    ///
    /// The lines with no object row keep the arguments of the macro: `GUARDED_BY` has a function row,
    /// `BOTH` has the two rows, and `NO_ROW` has no row. The name of 64 bytes with a tail keeps them too,
    /// because the seed holds only the first 64 bytes. EACH LINE THAT FOLLOWS KEEPS THE TREE WITH NO SEED,
    /// AND EACH ONE DECIDES A GUARD OF THE SCAN:
    /// - `max MOZ_UNANNOTATED ()` keeps the macro between the name and the parameter list, because an
    ///   empty group holds no initializer.
    /// - `swap MOZ_UNANNOTATED (T& a, T& b) { T c = a; }` keeps it too, because only a `;` or a `,` comes
    ///   after an object reading, and a body does not.
    /// - `isnan MOZ_UNANNOTATED(y))` in a condition keeps its reading, because a `)` does not come after an
    ///   object reading.
    /// - `Mutex l1 MOZ_UNANNOTATED GUARDED_BY(mu);` gives `(mu)` to `GUARDED_BY`, because the object reading
    ///   takes only the group directly after the name.
    /// - `Air::Opcode NODELETE opcodeForType(Width width);` of WebKit keeps its type macro with an object
    ///   row for `NODELETE`, because the object readings start at the name of the declarator.
    #[test]
    fn an_object_macro_of_the_seed_gives_the_group_to_the_declarator() {
        const LONG: &str = "OBJECT_MACRO_WITH_A_NAME_OF_SIXTY_FOUR_BYTES_AND_NO_PARAMETERS_1";
        assert_eq!(LONG.len(), 64, "the name is the longest that the format takes");
        let file = SeedFile::new(&format!(
            "BOTH\tobject-macro,function-macro\nGUARDED_BY\tfunction-macro\nMOZ_UNANNOTATED\tobject-macro\n\
             NODELETE\tobject-macro\n{LONG}\tobject-macro\n"
        ));
        let seed = Seed::read(file.path()).expect("the seed reads");
        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let wrapped = "(attributed_declarator (identifier) (attribute_macro name: (identifier)))";
        for source in [
            "void g() { Mutex l1 MOZ_UNANNOTATED(\"autolock\"); }\n",
            "void g() { Monitor monitor MOZ_UNANNOTATED(__func__); }\n",
            "Monitor monitor MOZ_UNANNOTATED(__func__);\n",
            "Stream in MOZ_UNANNOTATED(0);\n",
        ] {
            let seeded = parser.parse(source, None).expect("the parse ends").root_node().to_sexp();
            let deleted = parser
                .parse(source.replace(" MOZ_UNANNOTATED", ""), None)
                .expect("the parse ends")
                .root_node()
                .to_sexp();
            assert_eq!(seeded.matches(wrapped).count(), 1, "{source}{seeded}");
            assert_eq!(seeded.replace(wrapped, "(identifier)"), deleted, "{source}");
        }
        // The rule of the macro rows also makes an undeclared first name a type before a macro with a row
        // (refer to `a_macro_row_reaches_the_type_before_a_declarator_and_a_macro`), so a line with such a
        // name can differ from its tree with no seed. The lines here assert that no object reading comes.
        let object_readings = ["(init_declarator declarator: (attributed_declarator", "(function_declarator declarator: (attributed_declarator"];
        for source in [
            "int x GUARDED_BY(mu);\n".to_owned(),
            "int y BOTH(mu);\n".to_owned(),
            "int z NO_ROW(mu);\n".to_owned(),
            "void g() { Mutex l1 GUARDED_BY(\"autolock\"); }\n".to_owned(),
            format!("void g() {{ int l1 {LONG}X(\"autolock\"); }}\n"),
        ] {
            let seeded = parser.parse(&source, None).expect("the parse ends").root_node().to_sexp();
            assert!(seeded.contains("(attribute_macro name: (identifier) arguments: (argument_list"), "{source}{seeded}");
            assert!(!object_readings.iter().any(|reading| seeded.contains(reading)), "{source}{seeded}");
        }
        for source in [
            "class A { static int max MOZ_UNANNOTATED () { return 1; } };\n",
            "template <class T> void swap MOZ_UNANNOTATED (T& a, T& b) { T c = a; }\n",
            "void g() { int l1 MOZ_UNANNOTATED GUARDED_BY(mu); }\n",
            "Air::Opcode NODELETE opcodeForType(Width width);\n",
        ] {
            let seeded = parser.parse(source, None).expect("the parse ends");
            assert!(!seeded.root_node().has_error(), "{source}{}", seeded.root_node().to_sexp());
            assert_eq!(seeded.root_node().to_sexp(), bare(source).root_node().to_sexp(), "{source}");
        }
        // The condition has an error with and without the seed, and neither macro takes an object reading.
        let condition = "bool f(double x, double y) {\n  if (isnan MOZ_UNANNOTATED(x) || isnan MOZ_UNANNOTATED(y))\n    \
                         return false;\n  return true;\n}\n";
        let seeded = parser.parse(condition, None).expect("the parse ends").root_node().to_sexp();
        assert!(!object_readings.iter().any(|reading| seeded.contains(reading)), "{condition}{seeded}");
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

    /// A MACRO-SHAPED NAME THAT IS A WHOLE TEMPLATE PARAMETER READS AS A MACRO.
    ///
    /// `template <UNDIRECTED_GRAPH_PARAMS> class undirected_graph` of boost graph holds a macro where
    /// one parameter stands. The rule fires on the position and the shape of the name, so a parse with
    /// no seed reads it too, and it declines on the evidence of a declared type.
    ///
    /// EACH LINE OF THE SOURCE NAMES AN INPUT THAT ONE TEST OF THE RULE DECIDES:
    /// - `template <UNDIRECTED_GRAPH_PARAMS> class g {};` takes the reading, and the name stands alone.
    /// - `template <typename T, MACRO_TAG> struct S;` takes it after a parameter of the same list.
    /// - `FILE* BufType::* FileMemberPtr` of fmt ostream.h keeps its parameter, because the character
    ///   after the name is not `,` and not `>`.
    /// - `struct BR { const int &r; };` then `template <BR>` keeps its parameter, because a class head
    ///   of the file with a member declares the name. This is a non-type parameter of class type.
    /// - `template <T> void i() {}` keeps its parameter, because the name has one letter.
    /// - `template <ET> void h() {}` keeps its parameter when a type row of the seed holds `ET`.
    ///
    /// Refer to `scan_macro_start` in src/scanner.c.
    #[test]
    fn a_macro_shaped_name_that_is_a_whole_template_parameter_reads_as_a_macro() {
        const SOURCE: &str = "template <UNDIRECTED_GRAPH_PARAMS> class g {};\n\
                              template <typename T, MACRO_TAG> struct S;\n";
        // Each line here keeps the tree of a parse before this rule.
        const KEPT: &str = "template <typename Tag, typename BufType, FILE* BufType::* FileMemberPtr>\n\
                            struct a;\nstruct BR { const int &r; };\ntemplate <BR> void f() {}\n\
                            template <T> void i() {}\n";
        let tree = bare(SOURCE);
        let sexp = tree.root_node().to_sexp();
        assert!(!tree.root_node().has_error(), "{sexp}");
        assert_eq!(sexp.matches("(macro_invocation").count(), 2, "{sexp}");
        // A macro that gives a whole parameter with no arguments holds its name only.
        assert!(
            sexp.contains("(template_parameter_list (macro_invocation name: (identifier)))"),
            "{sexp}"
        );

        let kept = bare(KEPT);
        let kept_sexp = kept.root_node().to_sexp();
        assert!(!kept.root_node().has_error(), "{kept_sexp}");
        assert_eq!(kept_sexp.matches("(macro_invocation").count(), 0, "{kept_sexp}");
        assert_eq!(kept_sexp.matches("(parameter_declaration").count(), 3, "{kept_sexp}");

        // A type row of the seed declines the name, as a class head of the file does.
        const SEEDED: &str = "template <ET> void h() {}\n";
        let plain = bare(SEEDED).root_node().to_sexp();
        assert_eq!(plain.matches("(macro_invocation").count(), 1, "{plain}");
        let file = SeedFile::new("ET\ttype\n");
        let seed = Seed::read(file.path()).expect("the seed reads");
        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let seeded = parser.parse(SEEDED, None).expect("the parse ends").root_node().to_sexp();
        assert_eq!(seeded.matches("(macro_invocation").count(), 0, "{seeded}");
    }

    /// A BODY THAT ENDS IN `->` OR `.` READS THE TWO NAMES AND THE GROUP AS ONE MEMBER CALL.
    ///
    /// `#define __ masm->` of v8 makes `__ Mov(x29, sp);` the member call `masm->Mov(x29, sp);`
    /// ([expr.ref] p1, [expr.call] p1). The grammar reads the text as a declaration of `Mov` with the
    /// type `__`, and a member access operator can start no declaration. Refer to
    /// `_macro_member_statement` in grammar/src/cpp.rs.
    ///
    /// EACH LINE OF THE SOURCE NAMES A POSITION THAT THE RULE DECIDES: a block, a case body, a label,
    /// a file item, a namespace body and a branch of a conditional group.
    ///
    /// EACH LINE OF `KEPT` NAMES AN INPUT THAT ONE TEST OF THE RULE DECIDES, and each one keeps the
    /// tree of a parse with no seed:
    /// - `__ Get(i).Set(v);` and `__ out()[0] = w;`: a postfix operator after the group. The rule
    ///   holds the call and the `;` only, and the scan reads the `;` before it gives the token.
    /// - `int a[] = {__ Foo(x), 1};`: a `,` after the group, in a braced list and not in a block.
    /// - `MEMBER_AND_MORE Foo(x);`: a name whose bodies are a member prefix AND one other shape. The
    ///   expansion at the site is unknown, so the reading is not forced.
    /// - `A_TYPE Foo(x);`: a name that the seed also declares as a type.
    /// - `__ return(x);`: a keyword of the grammar in the place of the member.
    /// - `PLAIN Foo(x);`: a name with no body row.
    /// - `class C { __ Unreachable(); };`: a class body, where the rule is not valid.
    ///
    /// THE GATE RUNS NO SEED, so this rule measures zero in the plain steps of the gate. This test
    /// and a seeded run of `xtask trees --seeds` are the evidence.
    #[test]
    fn a_member_prefix_body_reads_a_statement_of_two_names_as_a_member_call() {
        const SOURCE: &str = "void f() {\n  __ Mov(x29, sp);\n  switch (n) { case 1: __ Nop(); }\n  \
                              here: __ Ret(0);\n}\n__ Init(0);\nnamespace ns { __ Start(1); }\n\
                              void g() {\n#if X\n  __ Stop(2);\n#endif\n}\n";
        // Each line here keeps the tree of a parse with no seed.
        const KEPT: &str = "void f() {\n  __ Get(i).Set(v);\n  __ out()[0] = w;\n  int a[] = {__ Foo(x), 1};\n  \
                            MEMBER_AND_MORE Foo(x);\n  A_TYPE Foo(x);\n  __ return(x);\n  PLAIN Foo(x);\n}\n\
                            class C { __ Unreachable(); };\n";
        // The rows of a seed ascend by the bytes of the name.
        let file = SeedFile::new(
            "A_TYPE\ttype,object-macro,body-member\nMEMBER_AND_MORE\tobject-macro,body-member,body-other\n\
             PLAIN\tobject-macro\n__\tobject-macro,body-member\n",
        );
        let seed = Seed::read(file.path()).expect("the seed reads");

        let unseeded = bare(SOURCE);
        let before = unseeded.root_node().to_sexp();
        assert!(!unseeded.root_node().has_error(), "{before}");
        assert_eq!(before.matches("(field_expression").count(), 0, "{before}");

        let mut parser = parser();
        // SAFETY: `seed` lives to the end of this test.
        unsafe { parser.set_scanner_context(seed.as_context()) };
        let seeded = parser.parse(SOURCE, None).expect("the parse ends");
        let sexp = seeded.root_node().to_sexp();
        assert!(!seeded.root_node().has_error(), "{sexp}");
        // The six positions take the reading, and no declaration stays.
        assert_eq!(
            sexp.matches(
                "(expression_statement (call_expression function: (field_expression argument: (identifier) \
                 field: (field_identifier)) arguments: (argument_list"
            )
            .count(),
            6,
            "{sexp}"
        );
        assert_eq!(sexp.matches("(declaration type:").count(), 0, "{sexp}");

        let kept = parser.parse(KEPT, None).expect("the parse ends");
        assert_eq!(kept.root_node().to_sexp(), bare(KEPT).root_node().to_sexp(), "a line of KEPT changed");
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

    /// True where the text gives a token, or calls a function that gives one.
    fn gives_token(stripped: &[String], text: &str, all: &[Function], givers: &BTreeSet<String>) -> bool {
        let _ = (stripped, all);
        text.contains("result_symbol") || givers.iter().any(|name| calls(text, name.as_str()))
    }

    /// The names of the functions that set a `result_symbol`.
    pub fn token_givers(stripped: &[String], all: &[Function]) -> BTreeSet<String> {
        let mut set: BTreeSet<String> = BTreeSet::new();
        loop {
            let mut grew = false;
            for function in all {
                if set.contains(&function.name) {
                    continue;
                }
                let body = stripped[function.start..=function.end].join("\n");
                if body.contains("result_symbol") || set.iter().any(|name| calls(&body, name.as_str())) {
                    set.insert(function.name.clone());
                    grew = true;
                }
            }
            if !grew {
                return set;
            }
        }
    }

    /// The branches that read a character and can then give no token, where a scan after them could
    /// have given one.
    ///
    /// THIS IS THE SECOND HALF OF THE LAW AND ITS DAMAGE IS DIFFERENT. A branch that is NOT ENTERED
    /// leaves the reader moved for the scans after it, which is rule one. A branch that IS ENTERED,
    /// reads characters and then gives no token STOPS THE WHOLE FUNCTION. The scans after it never
    /// run at all, so the cost is not a stale read. It is an absence.
    ///
    /// r47 found `P(CONFIG)` this way on 2026-09-16. A branch for a template parameter name read a
    /// word, found a `(` where it wanted a name, and gave no token. `scan_macro_start`, which is the
    /// last statement of `scan_word_start`, never ran, and the macro invocation got no token.
    ///
    /// THE REPAIR IS TO DECIDE BEFORE READING A CHARACTER. A test of `next`, the character that the
    /// function took before any scan, costs nothing and cannot consume.
    ///
    /// THIS IS AN INVENTORY AND NOT A GATE, AND THE MEASUREMENT IS WHY. The scan of the file gives
    /// 10 sites. Several of them are CORRECT BY DESIGN, among them the `CLASS_MACRO_MARK` guard,
    /// which stops the function on purpose after `scan_class_macro_mark` moved the reader, because
    /// no later scan is safe once the reader moved. A check that fails on correct sites is a check
    /// that somebody turns off.
    ///
    /// THE DISCRIMINATOR THAT r47 PROPOSED DOES NOT SEPARATE THEM, AND I MEASURED IT. "The condition
    /// reads the lookahead of the start of the call" holds for 1 of the 10 sites, and 9 that do not
    /// read it include both the correct and the defective. The row says which, so a reader can use
    /// it, and the test does not decide with it.
    ///
    /// THE INVENTORY GREW FROM 10 SITES TO 11 ON THE DAY IT LANDED, and the new row is the
    /// `ENUMERATOR_MACRO_NAME` branch of a rule that landed the same evening. A list that grows with
    /// the scanner is what an inventory is for.
    ///
    /// AN EXPOSURE COUNT IS NOT A DEFECT COUNT, MEASURED AT 24,347 TO 1. r47's runtime probe counted
    /// 24,347 declines at the `INITIALIZER_MACRO_START` site where the last statement of
    /// `scan_word_start` was live and never ran. Making the guard symmetric with its two siblings
    /// changed ONE clean tree over 329,387 files and added one error byte, so the asymmetry is real
    /// in the source and inert in the corpus. A GATE ON EXPOSURE WOULD FIRE 24,347 TIMES FOR ONE
    /// CHANGE THAT POINTS THE WRONG WAY.
    ///
    /// THE ONLY INSTRUMENT THAT MEASURES WHETHER A DECLINE COSTS A BETTER TREE IS A DRAFT AND
    /// `xtask trees`, ONE BUILD FOR EACH SITE. THAT IS A MEASUREMENT AND NOT A GATE, BECAUSE A GATE
    /// RUNS ON EVERY LANDING. So this stays an inventory that prints rows for a reader, and a reader
    /// resolves any row in one build.
    ///
    /// THE QUESTION IS STATIC AFTER ALL, AND IT IS IN A DIFFERENT FILE. "Can a later branch give a
    /// token AT THIS POSITION" is a question about `valid_symbols`, which the parser fills from
    /// `ts_external_scanner_states` of src/parser.c, ONE ROW FOR EACH EXTERNAL LEXER STATE. That
    /// table is generated source in this repository, so two external tokens are valid together
    /// exactly when one row holds both.
    /// THE ANSWER DOES NOT SEPARATE THE SITES. Every branch of this inventory shares a state with
    /// `_macro_line_start`, including the `CLASS_MACRO_MARK` site that is correct by design. So
    /// co-validity proves EXPOSURE for every row and decides nothing, which is the same lesson as
    /// the 24,347 above. `report_the_co_valid_tokens_of_each_stop_site` prints the pairs of each
    /// row from that table, without the recovery row that holds every token.
    /// A branch that cannot decide from `next` has no repair here except a rewind of the lexer,
    /// which this fork refused on price. Refer to "THE SCANNER CAN REWIND THE LEXER, AT A PRICE"
    /// of PROTOCOL.txt.
    ///
    /// THE PRICE OF EACH ROW, AT eb03d9e, OVER 329,387 CORPUS FILES AND 7,646 COMPILER TEST FILES.
    /// A yield draft is a build where the branch does not run wherever a later form is also live.
    /// A draft gives three numbers: the trees that change (reach), the files that lose their error
    /// (FIXED), and the files that get one (NEW ERROR). A reach with FIXED 0 is not a repair, and
    /// the trees say what it is.
    /// TWO CUTS OF THE TABLE GIVE THE COUNTS, AND A READER MUST NOT COMPARE ONE WITH THE OTHER. The
    /// 47 and the 48 are the states that a token shares with `_macro_line_start`. On that cut every
    /// narrow row shares one state, and that one is the recovery row, which holds every token. The
    /// counts of "real states" leave the recovery row out: ALIGNAS_TYPE_NAME has 1, ENUMERATOR_MACRO_NAME
    /// 4, DECLARATOR_MACRO_NAME 8, TRAILING_MACRO_NAME 20, and TEMPLATE_PARAMETER_TYPE_NAME 85.
    /// - `scan_word_start` TEMPLATE_PARAMETER_TYPE_NAME, 47 states with `_macro_line_start`: reach
    ///   32, FIXED 0, NEW ERROR 0. Each of the 32 is the row's own success undone: `T MACRO name`
    ///   with `T` from the template head, where the yield gives `attribute_macro T` and the type
    ///   `MACRO`. The pin "A macro after a type that the template head declares" falls with it.
    /// - `scan_macro_start` MACRO_SCOPE_START at the `:`, 48 states with `_macro_line_start`:
    ///   reach 9, FIXED 0, NEW ERROR 2, and 5 files with more error bytes. Each of the 9 is
    ///   `MACRO(args)::name`, the row's own success undone. The scan eats the first `:`, and no
    ///   later form of the function gave a token in any of the 9.
    /// - `scan_word_start` INITIALIZER_MACRO_START: 24,347 exposures to one neutral change, above.
    /// - `scan_token` ENUMERATOR_MACRO_NAME: reach 0. Its four real states hold no macro token, and
    ///   the forms after it want a `[`, a `#`, or a keyword, which the refused name is not.
    /// - `scan_word_start` CLASS_MACRO_MARK: correct by design. The stop is the purpose of the guard.
    /// - `scan_word_start` MACRO_SCOPE_START at the group: zero by construction. The block runs only
    ///   with no macro token live, and `macro && scan_macro_start` is the only statement after it.
    /// - `scan_trailing_macro_name`, 20 real states: zero by construction. `read_macro_name` refuses
    ///   the names that `is_macro_name` with two characters refuses, so the call name macro of a
    ///   shared state refuses them too, and every later statement of the function wants that name.
    /// - `scan_directive` at `##`: zero by construction. The branch gives no token only without
    ///   `pending`, and the later form of the function wants `pending`.
    /// - `scan_macro_start` at the group and at `if (call)`: one read that three decisions share,
    ///   so a yield of one is a yield of all three. Unmeasured.
    /// - `scan_word_start` ALIGNAS_TYPE_NAME, one real state: unmeasured. That state also holds
    ///   `functional_cast_name`, `macro_scope_start` and `concatenated_macro_start`, so the price is
    ///   a declared type name in `alignas(...)` that a `(` follows. One build prices it.
    ///
    /// THE INSTRUMENT THAT FINDS A REAL DEFECT IS A CONSTRUCT SOMEBODY READS, AND THE COUNTS ONLY
    /// SAY WHERE TO LOOK. Co-validity proves exposure for every row and separates none. The yield
    /// drafts priced the two widest rows at zero. A count of 22 clean trees changed would have read
    /// as a repair, and five trees read said the opposite. The one defect of this family with a
    /// price, 23 sites of valid C++, came from r47 reading four broken files.
    pub fn stop_sites(
        stripped: &[String],
        function: &Function,
        advances: &BTreeSet<String>,
        blanks: &BTreeSet<String>,
        givers: &BTreeSet<String>,
    ) -> Vec<String> {
        let mut out = Vec::new();
        let list = branches(stripped, function);
        let captured: Vec<String> = captured_lookahead(stripped, function).into_iter().map(|(name, _)| name).collect();
        for branch in &list {
            let start = branch.line - 1;
            let close = block_end(stripped, start, function.end);
            // A scan after this branch must be able to give a token, or nothing is lost.
            let after = stripped[close + 1..function.end].join("\n");
            if !gives_token(stripped, &after, &[], givers) {
                continue;
            }
            // The first line of the branch that reads a character.
            let mut read_at = None;
            for (index, line) in stripped.iter().enumerate().take(close + 1).skip(start) {
                if advances.iter().any(|name| !blanks.contains(name) && calls(line, name.as_str())) {
                    read_at = Some(index);
                    break;
                }
            }
            let Some(read_at) = read_at else {
                continue;
            };
            // A `return false` after that line stops the function with the reader moved.
            let stops = (read_at..=close).any(|index| stripped[index].contains("return false"));
            if !stops {
                continue;
            }
            // A branch that reads the lookahead of the start of the call in its CONDITION has
            // already refused the shapes that one character can refuse. r47 repaired its branch
            // that way: a test of `next` costs nothing and cannot consume.
            let text = condition(stripped, start, function.end);
            let decided = captured
                .iter()
                .any(|name| Regex::new(&format!(r"(^|[^A-Za-z0-9_]){name}([^A-Za-z0-9_]|$)")).expect("the pattern builds").is_match(&text));
            out.push(format!(
                "{}:{} reads a character at line {} and can then give no token, so the scans after \
                 it never run. The condition {} the lookahead of the start of the call: {}",
                function.name,
                branch.line,
                read_at + 1,
                if decided { "READS" } else { "DOES NOT READ" },
                branch.text
            ));
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

    /// The external tokens of each row of `ts_external_scanner_states` of src/parser.c.
    ///
    /// The parser fills `valid_symbols` from that table, one row for each external lexer state, so
    /// two tokens can be valid at one position exactly when one row holds both. A name comes back
    /// without the leading underscore of a hidden rule: `_macro_scope_start` reads as
    /// `macro_scope_start`, which is the C constant `MACRO_SCOPE_START` in lowercase. The table
    /// ends at the first `};` after its brace. O(n) in the length of the table.
    pub fn external_states(parser: &str) -> Vec<BTreeSet<String>> {
        let Some(start) = parser.find("static const bool ts_external_scanner_states") else {
            return Vec::new();
        };
        let Some(open) = parser[start..].find('{') else {
            return Vec::new();
        };
        let body = &parser[start + open..];
        let body = body.find("\n};").map_or(body, |end| &body[..end]);
        let row = Regex::new(r"^\s*\[\d+\]\s*=\s*\{").expect("the pattern builds");
        let entry = Regex::new(r"\[ts_external_token_([A-Za-z0-9_]+)\]\s*=\s*true").expect("the pattern builds");
        let mut states: Vec<BTreeSet<String>> = Vec::new();
        for line in body.lines() {
            if row.is_match(line) {
                states.push(BTreeSet::new());
            } else if let (Some(found), Some(state)) = (entry.captures(line), states.last_mut()) {
                state.insert(found[1].trim_start_matches('_').to_string());
            }
        }
        states
    }
}

#[cfg(test)]
mod order_tests {
    use super::order;
    use regex::Regex;
    use std::collections::BTreeSet;
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

    /// Print the branches that stop the function after they read a character.
    ///
    /// AN INVENTORY, NOT A GATE. `order::stop_sites` says why. The test asserts only that the scan
    /// still finds sites, because a zero here would mean the reader broke and not that the file is
    /// clean. I made exactly that mistake once today with a different rule of this module.
    #[test]
    fn report_the_stop_sites() {
        let path = crate::repository().join("src").join("scanner.c");
        let text = fs::read_to_string(&path).expect("src/scanner.c reads");
        let stripped = order::strip(&text);
        let all = order::functions(&stripped);
        let advances = order::advancing(&stripped, &all);
        let blanks = order::blank_only(&stripped, &all, &advances);
        let givers = order::token_givers(&stripped, &all);
        let mut found = Vec::new();
        for index in order::multiplexers(&stripped, &all) {
            found.extend(order::stop_sites(&stripped, &all[index], &advances, &blanks, &givers));
        }
        println!("STOP SITES: {}", found.len());
        for row in &found {
            println!("  {row}");
        }
        assert!(
            !found.is_empty(),
            "the scan of the stop sites found none. The file holds them, so a zero here means that \
             the reader broke. Run this test with --nocapture and read the rows."
        );
    }

    /// Print the tokens that share a parse state with the token of each stop site.
    ///
    /// AN INVENTORY, NOT A GATE. The table proves exposure for every row and decides nothing, and
    /// `order::stop_sites` says why. The recovery row holds every token and says nothing, so the
    /// test leaves it out. A row whose condition names no token prints that fact. The test asserts
    /// that the table reads and that the sites still exist, for the reason `report_the_stop_sites`
    /// gives.
    #[test]
    fn report_the_co_valid_tokens_of_each_stop_site() {
        let source = crate::repository().join("src").join("scanner.c");
        let text = fs::read_to_string(&source).expect("src/scanner.c reads");
        let stripped = order::strip(&text);
        let all = order::functions(&stripped);
        let advances = order::advancing(&stripped, &all);
        let blanks = order::blank_only(&stripped, &all, &advances);
        let givers = order::token_givers(&stripped, &all);
        let mut rows = Vec::new();
        for index in order::multiplexers(&stripped, &all) {
            rows.extend(order::stop_sites(&stripped, &all[index], &advances, &blanks, &givers));
        }
        assert!(!rows.is_empty(), "the scan of the stop sites found none. Refer to report_the_stop_sites.");
        let parser = crate::repository().join("src").join("parser.c");
        let table = fs::read_to_string(&parser).expect("src/parser.c reads");
        let states = order::external_states(&table);
        assert!(!states.is_empty(), "src/parser.c holds no ts_external_scanner_states table, so the reader broke");
        let full = states.iter().map(BTreeSet::len).max().unwrap_or(0);
        let real: Vec<&BTreeSet<String>> = states.iter().filter(|state| state.len() < full).collect();
        let symbol = Regex::new(r"valid_symbols\[([A-Z_0-9]+)\]").expect("the pattern builds");
        println!("CO-VALID TOKENS OF {} STOP SITES, over {} real states of {}", rows.len(), real.len(), states.len());
        for row in &rows {
            let site = row.split(" reads a character").next().unwrap_or(row);
            let names: BTreeSet<String> = symbol.captures_iter(row).map(|found| found[1].to_lowercase()).collect();
            if names.is_empty() {
                println!("  {site}: the condition names no token");
                continue;
            }
            for name in &names {
                let holding: Vec<&&BTreeSet<String>> = real.iter().filter(|state| state.contains(name)).collect();
                let mut shared: BTreeSet<&str> = BTreeSet::new();
                for state in &holding {
                    shared.extend(state.iter().map(String::as_str));
                }
                shared.remove(name.as_str());
                let list: Vec<&str> = shared.into_iter().collect();
                println!("  {site} {name}: {} real states, shared with {}", holding.len(), list.join(", "));
            }
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

    /// The scanner reads the state of the lexer through `TSLexer` and never through a copy of the
    /// private `Lexer` of the runtime.
    ///
    /// `TSLexer` is the whole contract between a runtime and a scanner. `Lexer` of
    /// vendor/tree-sitter/src/lexer.h is private, it starts with `TSLexer data`, and the members
    /// after it are the state of the lexer. A cast of a `TSLexer *` to a struct that names those
    /// members reads that state, and the read is correct only while the private layout stays as it
    /// was. Nothing tells such a cast that a member moved: it reads a different field and gives a
    /// number of the right type.
    ///
    /// The trace of the record of locals held such a copy until ABI 1019 gave `get_offset`. The
    /// accessor makes the same value part of the contract, and this test keeps the copy from coming
    /// back for the next value somebody wants.
    ///
    /// THE TEST ASSERTS A POSITIVE FIRST. A pattern that stops matching finds nothing and reads as a
    /// pass, so the test demands that the file holds the accessor call before it demands that the
    /// file holds no cast.
    #[test]
    fn the_scanner_reads_no_private_field_of_the_runtime_lexer() {
        let path = crate::repository().join("src").join("scanner.c");
        let text = fs::read_to_string(&path).expect("src/scanner.c reads");

        let calls = text.matches("lexer->get_offset(lexer)").count();
        assert!(
            calls > 0,
            "{} calls `lexer->get_offset(lexer)` nowhere. Either the byte offset of ABI 1019 has no \
             reader left, and this test checks nothing, or the call is spelled differently and the \
             pattern must follow it.",
            path.display()
        );

        // A cast of a lexer pointer to a named type, with or without `const`. The `regex` crate has
        // no look-ahead, so the pattern captures the type and the code drops a cast to `TSLexer`,
        // which is the contract and not a private field.
        let cast = Regex::new(r"\(\s*(?:const\s+)?(\w+)\s*\*\s*\)\s*(?:_?lexer|_self)\b")
            .expect("the pattern compiles");
        // A struct whose first member is a `TSLexer` by value, which is the shape of the private
        // `Lexer` and the only reason to write such a struct in a scanner.
        let mirror = Regex::new(r"\{\s*\n\s*TSLexer\s+\w+\s*;").expect("the pattern compiles");
        let mut found: Vec<String> = Vec::new();
        for (number, line) in text.lines().enumerate() {
            for capture in cast.captures_iter(line) {
                if &capture[1] != "TSLexer" {
                    found.push(format!("  src/scanner.c:{}: {}", number + 1, line.trim()));
                }
            }
        }
        if mirror.is_match(&text) {
            found.push("  src/scanner.c declares a struct whose first member is a TSLexer".to_string());
        }
        assert!(
            found.is_empty(),
            "{} reads a private field of the runtime lexer. `TSLexer` is the whole contract, and a \
             copy of `Lexer` is correct only while its private layout does not move. Add an accessor \
             to `TSLexer` with an ABI version, as `get_offset` of ABI 1019 does.\n{}",
            path.display(),
            found.join("\n")
        );
    }
}
