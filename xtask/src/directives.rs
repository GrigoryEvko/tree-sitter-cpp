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

/// True if the line that starts at `start` has `#` as its first character that is not a blank.
fn is_directive_line(source: &[u8], start: usize) -> bool {
    source[start..]
        .iter()
        .find(|c| !matches!(c, b' ' | b'\t' | b'\r'))
        .copied()
        == Some(b'#')
}

/// The byte range of each line whose first character that is not a blank is `#`, in the order of the
/// source. The end is the newline of the line, or the end of the source.
///
/// One forward pass over the source gives every directive line. A corpus file holds 15,457 bytes and
/// 18 directive lines on average, so a search of these few ranges costs far less than a scan back to
/// the start of the line for each leaf of the tree. O(n) in the bytes of the source.
fn directive_lines(source: &[u8]) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    while start < source.len() {
        let end = source[start..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(source.len(), |i| start + i);
        if is_directive_line(source, start) {
            lines.push((start, end));
        }
        start = end + 1;
    }
    lines
}

/// The directive line that holds a byte, if a directive line does. O(log n) in the directive lines.
fn directive_line_of(lines: &[(usize, usize)], at: usize) -> Option<(usize, usize)> {
    let index = lines.partition_point(|&(start, _)| start <= at);
    let &(start, end) = lines.get(index.checked_sub(1)?)?;
    (at <= end).then_some((start, end))
}

/// The nodes of one tree that begin on a directive line and are not part of a directive.
///
/// The walk carries a `preproc_` ancestor down the tree, which makes a node part of a directive.
/// Only a leaf gives a site, because the start of an inner node is the start of its first leaf.
///
/// A file with no directive line gives no site and needs no walk. The ranges of the strings and the
/// comments tell whether the start of a line is inside one, and only a file that holds a candidate
/// pays for them. O(n) in the bytes and the nodes.
pub fn sites(path: &str, source: &[u8], tree: &Tree) -> Vec<Site> {
    let lines = directive_lines(source);
    if lines.is_empty() {
        return Vec::new();
    }

    let mut cursor = tree.walk();
    let mut candidates: Vec<(String, usize, usize, usize)> = Vec::new();
    let mut stack = vec![(tree.root_node(), false)];
    while let Some((node, in_directive)) = stack.pop() {
        let kind = node.kind();
        let directive = in_directive || kind.starts_with("preproc_");
        if !directive && kind != "comment" && node.child_count() == 0 {
            let begin = node.start_byte();
            if let Some((start, end)) = directive_line_of(&lines, begin) {
                candidates.push((kind.to_owned(), begin, start, end));
            }
        }
        for child in node.children(&mut cursor) {
            stack.push((child, directive));
        }
    }
    if candidates.is_empty() {
        return Vec::new();
    }

    // The second walk runs only for a file that holds a candidate, and almost no file does.
    let mut covers: Vec<(usize, usize)> = Vec::new();
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
    for (kind, begin, start, end) in candidates {
        if inside(start) {
            continue;
        }
        let mut text = String::from_utf8_lossy(&source[start..end]).replace(['\t', '\r'], " ");
        text.truncate(text.char_indices().nth(LINE_BYTES).map_or(text.len(), |(i, _)| i));
        found.push(Site {
            path: path.to_owned(),
            line: source[..begin].iter().filter(|&&c| c == b'\n').count() + 1,
            kind,
            text: text.trim().to_owned(),
        });
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
            let Ok(tree) = crate::corpus::parse_with_limit(parser, &source, path) else { return Vec::new() };
            // A file with a parse error is the subject of the corpus report, and not of this check.
            if tree.root_node().has_error() {
                return Vec::new();
            }
            sites(path, &source, &tree)
        })
        .flatten()
        .collect();
    report(found, baseline)
}

/// Compare the sites of a pass with a baseline, print the report line, and fail on a new site.
///
/// `sites` is a pure function of a source and a tree, so a pass that already parses the corpus can
/// collect the sites and call this. The check then costs no second parse of 329,387 files. Refer to
/// `cargo xtask trees --directives`.
///
/// A new site fails. A site that is gone does not, so a repair is never blocked by the check that
/// measures it.
pub fn report(mut found: Vec<Site>, baseline: &Path) -> Result<(), Box<dyn Error>> {
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
        let lines = super::directive_lines(source);
        assert_eq!(lines, vec![(0, 12)], "a blank before the `#` keeps the directive, and line 2 is code");
        assert_eq!(
            super::directive_line_of(&lines, 5),
            Some((0, 12)),
            "a byte of the first line is on the directive line"
        );
        assert_eq!(
            super::directive_line_of(&lines, 15),
            None,
            "a byte of the second line is on no directive line"
        );
        assert!(super::is_directive_line(source, 0), "a blank before the `#` keeps the directive");
        assert!(!super::is_directive_line(source, 13), "a line of code is not a directive line");
    }

    /// A directive line at the end of a source with no line end still has a range, and the last byte
    /// of the source is on it.
    #[test]
    fn a_directive_line_with_no_line_end_has_a_range() {
        let source = b"int y;\n#endif X";
        let lines = super::directive_lines(source);
        assert_eq!(lines, vec![(7, 15)], "the range ends at the end of the source");
        assert_eq!(super::directive_line_of(&lines, 14), Some((7, 15)));
        assert_eq!(super::directive_line_of(&lines, 3), None, "a byte before the directive line");
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
        let lines = super::directive_lines(source.as_bytes());
        let mut on_a_hash_line = 0;
        while let Some(node) = stack.pop() {
            if node.child_count() == 0
                && node.kind() != "comment"
                && super::directive_line_of(&lines, node.start_byte()).is_some()
            {
                on_a_hash_line += 1;
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
        let lines = super::directive_lines(source.as_bytes());
        let mut on_a_directive_line = 0;
        while let Some(node) = stack.pop() {
            if node.child_count() == 0
                && node.kind() != "comment"
                && super::directive_line_of(&lines, node.start_byte()).is_some()
            {
                on_a_directive_line += 1;
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

    /// A baseline file of a test, with one line for each site.
    fn baseline_file(name: &str, rows: &[&str]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("directive-sites-{name}-{}.txt", std::process::id()));
        std::fs::write(&path, rows.join("\n")).expect("the test writes its baseline");
        path
    }

    /// A site that the baseline does not hold fails the check.
    ///
    /// No directive form of C++ gives a site, so this test builds the site instead of parsing one.
    /// It holds the failing path of `report`, which no corpus file can reach.
    #[test]
    fn a_site_that_the_baseline_does_not_hold_fails() {
        let path = baseline_file("new", &[]);
        let found = vec![Site {
            path: "a/b.cc".to_owned(),
            line: 2,
            kind: "identifier".to_owned(),
            text: "#endif GUARD".to_owned(),
        }];
        let error = super::report(found, &path).expect_err("a new site fails the check");
        assert!(
            error.to_string().starts_with("1 node(s) begin on a directive line"),
            "the message names the count of new sites: {error}"
        );
        std::fs::remove_file(&path).ok();
    }

    /// A site of the baseline that the pass does not find is a repair, and it does not fail.
    #[test]
    fn a_site_of_the_baseline_that_is_gone_does_not_fail() {
        let path = baseline_file("gone", &["a/b.cc\t2\tidentifier\t#endif GUARD"]);
        super::report(Vec::new(), &path).expect("a site that is gone does not fail the check");
        std::fs::remove_file(&path).ok();
    }

    /// A site that the baseline holds passes, and the baseline keeps it.
    #[test]
    fn a_site_that_the_baseline_holds_passes() {
        let path = baseline_file("held", &["a/b.cc\t2\tidentifier\t#endif GUARD"]);
        let found = vec![Site {
            path: "a/b.cc".to_owned(),
            line: 2,
            kind: "identifier".to_owned(),
            // The text is not part of the key, and a different text still matches.
            text: "#endif OTHER".to_owned(),
        }];
        super::report(found, &path).expect("a site of the baseline does not fail the check");
        std::fs::remove_file(&path).ok();
    }
}
