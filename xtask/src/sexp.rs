//! Write the S-expression of a syntax tree with one node on each line, as `tree-sitter test` writes it.
//!
//! `tree_sitter::format_sexp` splits the text at each space and each `)`, and it keeps the state of a quote
//! after the quoted name of a MISSING node. The space before the next quoted name then became part of a name,
//! and the read of that name found a `)`. The function then closed all open nodes. After three MISSING nodes
//! at the same byte, it wrote the last MISSING node after the root. A name such as `")"` also lost its `)`.
//!
//! This module reads the names of MISSING and UNEXPECTED nodes as `Node::to_sexp` writes them, with the
//! quotes. The layout of the other nodes is the layout of `tree_sitter::format_sexp`. The functions read the
//! text one time and do not recurse. O(n) in the length of the text, plus the indentation of the output.

use std::io::{self, Write};

/// The start of the name of a node that the parser did not find, as `(MISSING ";")` or `(MISSING identifier)`.
const MISSING: &[u8] = b"MISSING ";
/// The start of the name of a character that no token starts with, as `(UNEXPECTED '@')` or `(UNEXPECTED 9474)`.
const UNEXPECTED: &[u8] = b"UNEXPECTED ";

/// The end of a name that starts at `start` and ends before a space, a `(`, or a `)`.
fn word_end(text: &[u8], start: usize) -> usize {
    text[start..]
        .iter()
        .position(|&byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b'(' | b')'))
        .map_or(text.len(), |offset| start + offset)
}

/// The end of a quoted name that starts with the quote at `start`.
///
/// `to_sexp` does not escape the quote in a name: `(MISSING """)` names the token `"`, and `(UNEXPECTED ''')`
/// the character `'`. The end is the first quote after the first character of the name that a `)` or the end
/// of the text follows. A name with no such quote ends as a word.
fn quoted_end(text: &[u8], start: usize) -> usize {
    let quote = text[start];
    (start + 2..text.len())
        .find(|&index| text[index] == quote && text.get(index + 1).is_none_or(|&next| next == b')'))
        .map_or_else(|| word_end(text, start), |index| index + 1)
}

/// The end of the name of a node after its `(`: a kind, a quoted kind, or a MISSING or UNEXPECTED form.
fn head_end(text: &[u8], start: usize) -> usize {
    let rest = &text[start..];
    let payload = if rest.starts_with(MISSING) {
        start + MISSING.len()
    } else if rest.starts_with(UNEXPECTED) {
        start + UNEXPECTED.len()
    } else {
        start
    };
    match text.get(payload) {
        Some(b'"' | b'\'') => quoted_end(text, payload),
        Some(_) => word_end(text, payload),
        None => text.len(),
    }
}

/// Write a line break and two spaces for each open node. O(n) in the depth.
fn indent(out: &mut impl Write, depth: usize) -> io::Result<()> {
    const SPACES: [u8; 256] = [b' '; 256];
    out.write_all(b"\n")?;
    let mut remaining = 2 * depth;
    while remaining > 0 {
        let count = remaining.min(SPACES.len());
        out.write_all(&SPACES[..count])?;
        remaining -= count;
    }
    Ok(())
}

/// Write an S-expression with one node on each line.
///
/// A child starts on a new line, two spaces deeper than its parent, with its field name before it. A `)`
/// comes after the last child. A `)` that closes no node is not written, as in `tree_sitter::format_sexp`.
/// A word that is not a field name or a node stays in the text after a space.
pub fn write(sexp: &str, out: &mut impl Write) -> io::Result<()> {
    let text = sexp.as_bytes();
    let mut depth = 0usize;
    let mut after_field = false;
    let mut index = 0;
    while index < text.len() {
        match text[index] {
            b' ' | b'\t' | b'\r' | b'\n' => index += 1,
            b')' => {
                if depth > 0 {
                    depth -= 1;
                    out.write_all(b")")?;
                }
                index += 1;
            }
            b'(' => {
                let end = head_end(text, index + 1);
                if !after_field && depth > 0 {
                    indent(out, depth)?;
                }
                after_field = false;
                out.write_all(&text[index..end])?;
                depth += 1;
                index = end;
            }
            _ => {
                let end = word_end(text, index);
                let word = &text[index..end];
                if word.ends_with(b":") {
                    indent(out, depth)?;
                    out.write_all(word)?;
                    out.write_all(b" ")?;
                    after_field = true;
                } else {
                    out.write_all(b" ")?;
                    out.write_all(word)?;
                }
                index = end;
            }
        }
    }
    Ok(())
}

/// An S-expression with one node on each line. Refer to [`write`].
pub fn format(sexp: &str) -> String {
    let mut out = Vec::with_capacity(sexp.len() * 2);
    write(sexp, &mut out).expect("a write to a Vec does not fail");
    // The output has the bytes of the input slices at ASCII delimiters, and ASCII spaces and line breaks.
    String::from_utf8(out).expect("the output of UTF-8 input is UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_starts_a_line_with_its_field_name() {
        assert_eq!(
            format("(translation_unit (declaration type: (primitive_type) declarator: (identifier)))"),
            "(translation_unit\n  (declaration\n    type: (primitive_type)\n    declarator: (identifier)))"
        );
    }

    /// The tree of the corpus test "Unclosed blocks of a switch and a loop at the end of the input".
    #[test]
    fn three_missing_braces_at_one_byte_stay_in_their_blocks() {
        let sexp = "(translation_unit (function_definition body: (compound_statement (switch_statement \
                    body: (compound_statement (case_statement (for_statement body: (compound_statement \
                    (MISSING \"}\")))) (MISSING \"}\"))) (MISSING \"}\"))))";
        let expected = "\
(translation_unit
  (function_definition
    body: (compound_statement
      (switch_statement
        body: (compound_statement
          (case_statement
            (for_statement
              body: (compound_statement
                (MISSING \"}\"))))
          (MISSING \"}\")))
      (MISSING \"}\"))))";
        assert_eq!(format(sexp), expected);
        let old = tree_sitter::format_sexp(sexp, 0);
        assert!(
            old.ends_with(")(MISSING \"}\")"),
            "tree_sitter::format_sexp gives {old}"
        );
    }

    #[test]
    fn a_quoted_name_keeps_its_quotes_brackets_and_spaces() {
        assert_eq!(
            format("(a (MISSING \")\") (MISSING \"\"\") (UNEXPECTED ')') (UNEXPECTED ''') (b))"),
            "(a\n  (MISSING \")\")\n  (MISSING \"\"\")\n  (UNEXPECTED ')')\n  (UNEXPECTED ''')\n  (b))"
        );
        assert_eq!(
            format("(a (UNEXPECTED ' ') (UNEXPECTED '\\n') (UNEXPECTED 9474) (MISSING identifier) (b))"),
            "(a\n  (UNEXPECTED ' ')\n  (UNEXPECTED '\\n')\n  (UNEXPECTED 9474)\n  (MISSING identifier)\n  (b))"
        );
    }

    /// With no MISSING or UNEXPECTED node, the layout is the layout of `tree_sitter::format_sexp`, which
    /// wrote the expected trees of the corpus tests.
    #[test]
    fn the_layout_agrees_with_tree_sitter_for_trees_with_no_quoted_name() {
        let samples = [
            "(translation_unit)",
            "(translation_unit (comment) (declaration type: (primitive_type) declarator: (init_declarator \
             declarator: (identifier) value: (call_expression function: (identifier) arguments: \
             (argument_list (number_literal))))))",
            "(a (b (c (d)) (e)) f: (g) (h i: (j (k))))",
            "(ERROR (identifier) (ERROR (primitive_type)))",
        ];
        for sample in samples {
            assert_eq!(format(sample), tree_sitter::format_sexp(sample, 0), "{sample}");
        }
    }

    #[test]
    fn a_line_has_two_spaces_for_each_open_node() {
        let depth = 300;
        let sexp = format!("{}(x){}", "(a ".repeat(depth), ")".repeat(depth));
        let formatted = format(&sexp);
        let last = formatted.lines().last().expect("the text has lines");
        assert_eq!(last, format!("{}(x){}", " ".repeat(2 * depth), ")".repeat(depth)));
    }

    /// The runtime writes the text of a deep tree with no recursion. The source is the form of
    /// `hunt2/10-scanner-runtime/perf/mod_lines_32000.cpp`.
    #[test]
    fn the_text_of_a_tree_with_a_depth_of_32000_has_no_stack_overflow() {
        let lines = 32_000;
        let source = format!("int f(int a, int b) {{ return {}a % b; }}\n", "a % b +\n".repeat(lines));
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::new(tree_sitter_cpp::LANGUAGE))
            .expect("the grammar has the ABI of the runtime");
        let tree = parser
            .parse(&source, None)
            .expect("a parse with no time limit gives a tree");
        let sexp = tree.root_node().to_sexp();
        assert_eq!(sexp.matches("(binary_expression").count(), 2 * lines + 1);
        assert!(!tree.root_node().has_error());
    }
}
