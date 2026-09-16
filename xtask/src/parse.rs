//! Print the syntax tree of a file, or of the standard input, as an indented S-expression.
//!
//! The parse has the budget and the memory ceiling of `corpus`. A file that passes one of them
//! gives no tree and a message, as it does in a corpus run.

use std::error::Error;
use std::fs;
use std::io::{BufWriter, Read as _, Write as _};

use tree_sitter::{Language, Parser};

use crate::corpus::parse_with_limit;
use crate::sexp;

/// Parse FILE, or the standard input for `-`, and print the tree.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let [path] = args else {
        return Err("usage: cargo xtask parse FILE (- reads the standard input)".into());
    };
    let source = if path == "-" {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes)?;
        bytes
    } else {
        fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?
    };
    let mut parser = Parser::new();
    parser.set_language(&Language::new(tree_sitter_cpp::LANGUAGE))?;
    let tree = parse_with_limit(&mut parser, &source, path)
        .map_err(|stop| format!("the parse of {path} stopped at {stop}. The file gets an error in a corpus run."))?;
    // The indentation of a deep tree makes a large text. The text goes to the output in parts.
    let mut out = BufWriter::new(std::io::stdout().lock());
    sexp::write(&tree.root_node().to_sexp(), &mut out)?;
    writeln!(out)?;
    out.flush()?;
    Ok(())
}
