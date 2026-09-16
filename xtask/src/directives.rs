//! Check that no node outside a directive begins on a directive line.
//!
//! THE INVARIANT. A code node does not begin on a line whose first character that is not a blank is
//! `#`. A node that breaks it holds a token that the preprocessor discards, so our tree gives a
//! position to text that no front end reads.
//!
//! THE FORM THAT THE INVARIANT FINDS. The tokens after `#endif` are extra, and the two front ends
//! discard them with a warning: GCC gives `-Wendif-labels` and Clang gives `-Wextra-tokens`. Our
//! tree gives the token to the code that follows it:
//!
//!     #ifdef USE_DML
//!     #endif USE_DML
//!     int y;
//!
//! gives `(preproc_ifdef ...) (macro_invocation name: (identifier)) (declaration ...)`. The
//! `macro_invocation` holds a token that no front end reads. The tree has no ERROR node, and the
//! comparison with Clang gives no row, because that comparison walks the facts of Clang and Clang
//! has no fact there.
//!
//! WHAT IS NOT A DEFECT, AND THE CHECK MUST NOT REPORT IT.
//! - A node that is PART of the directive: the condition of `#if 1 && 2`, the body of
//!   `#define f(x) x + 1`, the path of an `#include`. Each one has a `preproc_` ancestor.
//! - A comment. `#endif // NAME` is the common form, and a comment is an extra with no position of
//!   its own. In the corpus 89,819 comments begin on an `#endif` line.
//! - A line of a string that begins with `#`. The content of a raw string literal holds whole lines,
//!   and the tokens after the close of the string are on the line that the string ended:
//!   `#endif");`. The check reads the start of the line, and it reports nothing when that start is
//!   inside a string or a comment.
//!
//! THE BASELINE. The check fails on a site that the baseline does not hold, and it does NOT fail
//! when a site of the baseline is gone. A check that fails on a decrease blocks its own repair, and
//! a commit that removes a site removes it from the baseline in the same commit. The corpus held
//! nine sites, fork commit 228aada repaired each one, and the baseline is now EMPTY. The check is a
//! hard zero line, and each new site is a defect that no other report of the gate reads.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;

use rayon::prelude::*;
use tree_sitter::{Parser, Tree};

/// The maximum length of the text of the line in a site.
const LINE_BYTES: usize = 100;

/// One node that begins on a directive line and is not part of a directive.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Site {
    pub path: String,
    pub line: usize,
    pub kind: String,
    pub text: String,
}

impl Site {
    /// The key of a site in the baseline. The text is for a reader and it is not part of the key.
    fn key(&self) -> (&str, usize, &str) {
        (&self.path, self.line, &self.kind)
    }

    /// The line of the baseline file.
    fn row(&self) -> String {
        format!("{}\t{}\t{}\t{}", self.path, self.line, self.kind, self.text)
    }
}

/// The start of the line that holds `at`.
fn line_start(source: &[u8], at: usize) -> usize {
    source[..at].iter().rposition(|&c| c == b'\n').map_or(0, |i| i + 1)
}

/// True if the line that starts at `start` has `#` as its first character that is not a blank.
fn is_directive_line(source: &[u8], start: usize) -> bool {
    source[start..]
        .iter()
        .find(|c| !matches!(c, b' ' | b'\t' | b'\r'))
        .copied()
        == Some(b'#')
}

/// The nodes of one tree that begin on a directive line and are not part of a directive.
///
/// The walk carries two facts down the tree: a `preproc_` ancestor, which makes a node part of a
/// directive, and the ranges of the strings and the comments, which tell whether the start of a
/// line is inside one. Only a leaf gives a site, because the start of an inner node is the start of
/// its first leaf. O(n) in the number of nodes.
pub fn sites(path: &str, source: &[u8], tree: &Tree) -> Vec<Site> {
    let mut covers: Vec<(usize, usize)> = Vec::new();
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if matches!(node.kind(), "raw_string_literal" | "string_literal" | "comment") {
            covers.push((node.start_byte(), node.end_byte()));
        }
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    let inside = |at: usize| covers.iter().any(|&(s, e)| s <= at && at < e);

    let mut found = Vec::new();
    let mut stack = vec![(tree.root_node(), false)];
    while let Some((node, in_directive)) = stack.pop() {
        let kind = node.kind();
        let directive = in_directive || kind.starts_with("preproc_");
        if !directive && kind != "comment" && node.child_count() == 0 {
            let begin = node.start_byte();
            let start = line_start(source, begin);
            if is_directive_line(source, start) && !inside(start) {
                let end = source[start..]
                    .iter()
                    .position(|&c| c == b'\n')
                    .map_or(source.len(), |i| start + i);
                let mut text = String::from_utf8_lossy(&source[start..end]).replace(['\t', '\r'], " ");
                text.truncate(text.char_indices().nth(LINE_BYTES).map_or(text.len(), |(i, _)| i));
                found.push(Site {
                    path: path.to_owned(),
                    line: source[..begin].iter().filter(|&&c| c == b'\n').count() + 1,
                    kind: kind.to_owned(),
                    text: text.trim().to_owned(),
                });
            }
        }
        for child in node.children(&mut cursor) {
            stack.push((child, directive));
        }
    }
    found.sort();
    found
}

/// A parser of the C++ grammar.
fn new_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_cpp::LANGUAGE.into())
        .expect("the grammar loads");
    parser
}

/// Read the baseline file. A line that is empty or that starts with `#` is a comment of the file.
fn read_baseline(path: &Path) -> Result<Vec<Site>, Box<dyn Error>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display()).into()),
    };
    let mut sites = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.splitn(4, '\t');
        let (Some(path), Some(line_number), Some(kind)) = (fields.next(), fields.next(), fields.next()) else {
            return Err(format!("the baseline line has fewer than four fields: {line}").into());
        };
        sites.push(Site {
            path: path.to_owned(),
            line: line_number.parse().map_err(|e| format!("the line number {line_number} is not a number: {e}"))?,
            kind: kind.to_owned(),
            text: fields.next().unwrap_or("").to_owned(),
        });
    }
    sites.sort();
    Ok(sites)
}

/// Run the check over a list of files, and compare the sites with a baseline.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let [root, list, baseline] = args else {
        return Err("usage: cargo xtask directives ROOT LIST BASELINE".into());
    };
    let (root, baseline) = (Path::new(root), Path::new(baseline));
    let text = fs::read_to_string(list).map_err(|e| format!("cannot read {list}: {e}"))?;
    let paths: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();

    let found: Vec<Site> = paths
        .par_iter()
        .map_init(new_parser, |parser, path| {
            let Ok(source) = fs::read(root.join(path)) else { return Vec::new() };
            let Some(tree) = parser.parse(&source, None) else { return Vec::new() };
            // A file with a parse error is the subject of the corpus report, and not of this check.
            if tree.root_node().has_error() {
                return Vec::new();
            }
            sites(path, &source, &tree)
        })
        .flatten()
        .collect();
    let mut found = found;
    found.sort();

    let recorded = read_baseline(baseline)?;
    let keys: BTreeSet<_> = recorded.iter().map(Site::key).collect();
    let found_keys: BTreeSet<_> = found.iter().map(Site::key).collect();
    let new: Vec<&Site> = found.iter().filter(|s| !keys.contains(&s.key())).collect();
    let gone: Vec<&Site> = recorded.iter().filter(|s| !found_keys.contains(&s.key())).collect();

    for site in &new {
        println!("new site: {}", site.row());
    }
    // A site that is gone is a repair. The check prints it so that the agent removes it from the
    // baseline in the same commit, and it does not fail.
    for site in &gone {
        println!("gone: {}", site.row());
    }
    println!(
        "directives: {} sites, {} new, {} gone",
        found.len(),
        new.len(),
        gone.len()
    );
    if !new.is_empty() {
        return Err(format!(
            "{} node(s) begin on a directive line and are not in {}",
            new.len(),
            baseline.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Site, new_parser, sites};

    /// The sites of a snippet, as `line:kind` for each one.
    fn scan(source: &str) -> Vec<String> {
        let mut parser = new_parser();
        let tree = parser.parse(source, None).expect("the snippet parses");
        sites("x.cc", source.as_bytes(), &tree)
            .iter()
            .map(|s: &Site| format!("{}:{}", s.line, s.kind))
            .collect()
    }

    /// THE POSITIVE CONTROL OF THIS CHECK NO LONGER HAS A SOURCE, AND THAT IS THE RESULT.
    ///
    /// The control was `#ifndef A` / `#endif A` / `int y;`, which gave the site `2:identifier`.
    /// Fork commit 228aada reads the extra tokens of an `#endif` line and of an `#else` line, so
    /// that snippet now gives no site. No directive form that the grammar reads gives one: I ran
    /// `#ifdef A B`, `#else X`, `#undef A B`, `#pragma once X`, `#line 5 "f" X`, `#error a b`,
    /// `#endif }`, `#endif 1+2`, `#endif ;`, `#endif (x)`, `#endif "s"`, `#elif 2 X`, and an
    /// `#endif` with a line splice. Each one gives zero sites. `#include <a.h> X` gives an ERROR
    /// node, which the corpus report reads and this check skips.
    ///
    /// So the check is a hard zero, and the two tests that follow take the place of the positive
    /// control. Each one proves that one condition of `sites` carries weight, because a test that
    /// only shows an empty result cannot tell a working check from a broken one.
    #[test]
    fn the_predicates_read_a_directive_line() {
        let source = b"  #  endif X\nint y;\n";
        assert_eq!(super::line_start(source, 5), 0, "the start of the first line");
        assert!(super::is_directive_line(source, 0), "a blank before the `#` keeps the directive");
        let second = super::line_start(source, 15);
        assert_eq!(second, 13, "the start of the second line");
        assert!(!super::is_directive_line(source, second), "a line of code is not a directive line");
    }

    /// The exclusion of a string carries weight. Without it the tokens after the close of a raw
    /// string whose content ends a line give sites, because the line begins with `#`.
    #[test]
    fn the_tokens_after_a_raw_string_do_begin_on_a_line_that_starts_with_a_hash() {
        let source = "const char *s = R\"cc(\n#endif)cc\";\nint y;\n";
        let mut parser = new_parser();
        let tree = parser.parse(source, None).expect("the snippet parses");
        let mut cursor = tree.walk();
        let mut stack = vec![tree.root_node()];
        let mut on_a_hash_line = 0;
        while let Some(node) = stack.pop() {
            if node.child_count() == 0 && node.kind() != "comment" {
                let start = super::line_start(source.as_bytes(), node.start_byte());
                if super::is_directive_line(source.as_bytes(), start) {
                    on_a_hash_line += 1;
                }
            }
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }
        assert_eq!(
            on_a_hash_line, 4,
            "the close of the string and the `;` sit on a line that starts with `#`, and the exclusion of a string keeps each one out"
        );
        assert!(scan(source).is_empty(), "and the check reports none of them");
    }

    /// A node that is part of a directive is correct, and the check reports none of them.
    #[test]
    fn a_node_of_a_directive_gives_no_site() {
        let source = "#define f(x) x + 1\n#if 1 && 2\nint a;\n#endif\n#include <stddef.h>\nint b;\n";
        assert!(scan(source).is_empty(), "the nodes of a directive are not sites");
    }

    /// The test above is not vacuous. Each node of a directive DOES begin on a directive line, and
    /// only the `preproc_` ancestor keeps it out of the list. A later change that removes that
    /// condition makes this test fail.
    #[test]
    fn the_nodes_of_a_directive_do_begin_on_a_directive_line() {
        let source = "#define f(x) x + 1\n#if 1 && 2\nint a;\n#endif\n#include <stddef.h>\nint b;\n";
        let mut parser = new_parser();
        let tree = parser.parse(source, None).expect("the snippet parses");
        let mut cursor = tree.walk();
        let mut stack = vec![tree.root_node()];
        let mut on_a_directive_line = 0;
        while let Some(node) = stack.pop() {
            if node.child_count() == 0 && node.kind() != "comment" {
                let begin = node.start_byte();
                let start = super::line_start(source.as_bytes(), begin);
                if super::is_directive_line(source.as_bytes(), start) {
                    on_a_directive_line += 1;
                }
            }
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }
        assert_eq!(
            on_a_directive_line, 14,
            "the snippet holds 14 leaves on a directive line, and the ancestor condition keeps each one out"
        );
    }

    /// The line of a raw string that begins with `#` is not a directive line, and the tokens after
    /// the close of the string are not sites.
    #[test]
    fn a_line_of_a_raw_string_is_not_a_directive_line() {
        assert!(scan("const char *s = R\"cc(\n#endif)cc\";\nint y;\n").is_empty());
    }
}
