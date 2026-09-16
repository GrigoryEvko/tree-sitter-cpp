//! Print the syntax tree of a file, or of the standard input, as an indented S-expression.
//!
//! The parse has the budget and the memory ceiling of `corpus`. A file that passes one of them
//! gives no tree and a message, as it does in a corpus run.
//!
//! With a seed, the first line of the output names the id of that seed, because the tree of a file
//! is a function of the file and of the seed. Refer to `seed`.

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
    const USAGE: &str = "usage: cargo xtask parse FILE [--seed SEED] (- reads the standard input)";
    let (path, seed) = match args {
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
    sexp::write(&tree.root_node().to_sexp(), &mut out)?;
    writeln!(out)?;
    out.flush()?;
    Ok(())
}
