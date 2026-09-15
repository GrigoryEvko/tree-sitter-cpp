//! Parse the snippets in `test/syntax`, and report each snippet with a parse error.
//!
//! A syntax file holds snippets. A line `### NAME` starts a snippet, and the lines after it,
//! up to the next such line, are its source code. Each snippet must parse with no ERROR node
//! and no MISSING node. The snippets cover the C++ standards from C++98 to C++2d and the
//! vendor extensions.

use std::error::Error;
use std::fs;
use std::path::Path;

use tree_sitter::{Language, Node, Parser};

use crate::sexp;

/// One snippet of a syntax file.
pub(crate) struct Snippet {
    pub(crate) name: String,
    pub(crate) source: String,
}

/// The snippets of a syntax file, in file order. The text before the first header is ignored.
pub(crate) fn snippets(content: &str) -> Vec<Snippet> {
    let mut out: Vec<Snippet> = Vec::new();
    for line in content.split_inclusive('\n') {
        if let Some(name) = line.strip_prefix("### ") {
            out.push(Snippet {
                name: name.trim().to_owned(),
                source: String::new(),
            });
        } else if let Some(last) = out.last_mut() {
            last.source.push_str(line);
        }
    }
    out
}

/// The first ERROR or MISSING node in document order.
///
/// The walk goes only into subtrees that have an error. O(n) in the visited nodes.
fn first_error(root: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if node.is_error() || node.is_missing() {
            return Some(node);
        }
        if node.has_error() && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return None;
            }
        }
    }
}

/// Parse each snippet. With a NAME, parse only the snippets whose name contains it, and print their trees.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let filter = match args {
        [] => None,
        [name] => Some(name.as_str()),
        _ => return Err("usage: cargo xtask syntax [NAME]".into()),
    };
    let directory = repository.join("test").join("syntax");
    let mut paths: Vec<_> = fs::read_dir(&directory)
        .map_err(|e| format!("cannot read {}: {e}", directory.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    paths.retain(|path| path.extension().is_some_and(|extension| extension == "txt"));
    paths.sort();
    let mut parser = Parser::new();
    parser.set_language(&Language::new(tree_sitter_cpp::LANGUAGE))?;

    let (mut total, mut failed) = (0usize, 0usize);
    for path in &paths {
        let content = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        for snippet in snippets(&content) {
            if filter.is_some_and(|name| !snippet.name.contains(name)) {
                continue;
            }
            total += 1;
            let tree = parser
                .parse(&snippet.source, None)
                .expect("a parser with a language and no time limit gives a tree");
            if let Some(error) = first_error(tree.root_node()) {
                failed += 1;
                let position = error.start_position();
                let line = snippet.source.lines().nth(position.row).unwrap_or_default().trim();
                let kind = if error.is_missing() {
                    format!("MISSING {}", error.kind())
                } else {
                    "ERROR".to_owned()
                };
                println!(
                    "✗ {}  {}:{} {kind}  {line}",
                    snippet.name,
                    position.row + 1,
                    position.column + 1
                );
            }
            if filter.is_some() {
                println!("{}\n", sexp::format(&tree.root_node().to_sexp()));
            }
        }
    }
    println!("\n{} of {total} snippets parse with no error", total - failed);
    if failed == 0 {
        Ok(())
    } else {
        Err(format!("{failed} snippets have a parse error").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_starts_a_snippet_and_the_lines_after_it_are_its_source() {
        let found = snippets("ignored\n### a\nint x;\n### b\nint y;\nint z;\n");
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].name.as_str(), found[0].source.as_str()), ("a", "int x;\n"));
        assert_eq!(
            (found[1].name.as_str(), found[1].source.as_str()),
            ("b", "int y;\nint z;\n")
        );
    }
}
