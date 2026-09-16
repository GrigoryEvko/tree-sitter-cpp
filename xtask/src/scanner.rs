//! The invariants of the source of `src/scanner.c` that no compiler checks.
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
