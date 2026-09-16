//! Print the syntax tree of a file, or of the standard input, as an indented S-expression.
//!
//! The parse has the budget and the memory ceiling of `corpus`. A file that passes one of them
//! gives no tree and a message, as it does in a corpus run.
//!
//! With a seed, the first line of the output names the id of that seed, because the tree of a file
//! is a function of the file and of the seed. Refer to `seed`.
//!
//! With `--text`, the output holds one named node for each line, with the field of the node, the
//! text of each leaf, and the line and the column of each node. A reader of an S-expression reads
//! the node kinds and infers the text, and the fork records that this inference was wrong more than
//! one time. The text form shows the tree and the text together.

use std::error::Error;
use std::fs;
use std::io::{BufWriter, Read as _, Write as _};
use std::path::Path;

use tree_sitter::{Language, Parser};

use crate::corpus::parse_with_limit;
use crate::seed::Seed;
use crate::sexp;

/// Parse FILE, or the standard input for `-`, and print the tree.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    const USAGE: &str = "usage: cargo xtask parse FILE [--seed SEED] [--text] (- reads the standard input)";
    let text = args.iter().any(|a| a == "--text");
    let args: Vec<String> = args.iter().filter(|a| *a != "--text").cloned().collect();
    let (path, seed) = match args.as_slice() {
        [path] => (path, None),
        [path, flag, seed] if flag == "--seed" => (path, Some(seed)),
        _ => return Err(USAGE.into()),
    };
    let source = if path == "-" {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes)?;
        bytes
    } else {
        fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?
    };
    let seed = seed.map(|path| Seed::read(Path::new(path))).transpose()?;
    let mut parser = Parser::new();
    parser.set_language(&Language::new(tree_sitter_cpp::LANGUAGE))?;
    if let Some(seed) = &seed {
        // SAFETY: the seed lives to the end of this function, and the parse is inside it.
        unsafe { parser.set_scanner_context(seed.as_context()) };
    }
    let tree = parse_with_limit(&mut parser, &source, path)
        .map_err(|stop| format!("the parse of {path} stopped at {stop}. The file gets an error in a corpus run."))?;
    // The indentation of a deep tree makes a large text. The text goes to the output in parts.
    let mut out = BufWriter::new(std::io::stdout().lock());
    if let Some(seed) = &seed {
        writeln!(out, "# seed {} {} {} names", seed.id(), seed.path().display(), seed.names())?;
    }
    if text {
        let mut cursor = tree.walk();
        write_text(&mut cursor, &source, 0, &mut out)?;
        out.flush()?;
        return Ok(());
    }
    sexp::write(&tree.root_node().to_sexp(), &mut out)?;
    writeln!(out)?;
    out.flush()?;
    Ok(())
}

/// Write the named nodes under the cursor, one for each line, with the text of each leaf.
///
/// A leaf is a named node with no named child. Its text is the first line of its source, cut at
/// 80 characters. An ERROR node and a MISSING node are in the output, because a reader of a
/// changed file looks for them first. An anonymous node adds no line and no depth. O(n) in the
/// nodes of the tree.
fn write_text(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    depth: usize,
    out: &mut impl std::io::Write,
) -> Result<(), Box<dyn Error>> {
    let node = cursor.node();
    if node.is_named() || node.is_error() || node.is_missing() {
        let field = cursor.field_name().map(|f| format!("{f}: ")).unwrap_or_default();
        let mut leaf = true;
        if cursor.goto_first_child() {
            loop {
                if cursor.node().is_named() {
                    leaf = false;
                    break;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
        let start = node.start_position();
        if leaf {
            let bytes = &source[node.start_byte()..node.end_byte()];
            let text = String::from_utf8_lossy(bytes);
            let text: String = text.lines().next().unwrap_or("").chars().take(80).collect();
            writeln!(out, "{}{field}({} {:?}) @{}:{}", "  ".repeat(depth), node.kind(), text, start.row + 1, start.column + 1)?;
            return Ok(());
        }
        writeln!(out, "{}{field}({}) @{}:{}", "  ".repeat(depth), node.kind(), start.row + 1, start.column + 1)?;
    }
    let next = if node.is_named() || node.is_error() { depth + 1 } else { depth };
    if cursor.goto_first_child() {
        loop {
            write_text(cursor, source, next, out)?;
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    Ok(())
}
