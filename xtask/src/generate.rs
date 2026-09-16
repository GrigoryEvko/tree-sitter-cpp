//! Generate the parser from the Rust grammar.

use std::error::Error;
use std::fs;
use std::path::Path;

use tree_sitter_generate::{Diagnostic, OptLevel, generate_parser_in_directory};

/// The ABI version of the generated parser: the version that the runtime in vendor/tree-sitter reads.
/// The ABI of the fork has 32-bit parse table values and state ids.
const ABI_VERSION: usize = tree_sitter::LANGUAGE_VERSION;

/// Write `src/grammar.json` from the Rust grammar, then the files that tree-sitter makes from it.
///
/// With `--check`, compare `src/grammar.json` with the Rust grammar and write nothing. With
/// `--diagnostics`, print the full text of each diagnostic of the generator.
///
/// The command writes the files and then fails when the generator reports a conflict set that the
/// parse table builder does not use.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let (check, show_diagnostics) = match args {
        [] => (false, false),
        [flag] if flag == "--check" => (true, false),
        [flag] if flag == "--diagnostics" => (false, true),
        _ => return Err("usage: cargo xtask generate [--check | --diagnostics]".into()),
    };
    let grammar = tree_sitter_cpp_grammar::grammar();
    let undefined = grammar.undefined_names();
    if !undefined.is_empty() {
        return Err(format!(
            "the grammar uses these names, but does not define them: {}",
            undefined.join(", ")
        )
        .into());
    }
    let json = serde_json::to_string_pretty(&grammar.to_json())?;
    let path = repository.join("src").join("grammar.json");
    if check {
        let written = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if written != json {
            return Err(format!(
                "{} does not agree with the Rust grammar. Run `cargo xtask generate`.",
                path.display()
            )
            .into());
        }
        return Ok(());
    }
    fs::write(&path, &json).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let mut diagnostics = Vec::new();
    generate_parser_in_directory(
        repository,
        None::<&Path>,
        Some(&path),
        ABI_VERSION,
        None,
        None,
        true,
        OptLevel::default(),
        &mut diagnostics,
    )?;
    // The generator reports the conflicts that the grammar declares and does not use, and other
    // notes about the rules. The full text of the other notes can be long, so the default prints the
    // count.
    let unnecessary: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic, Diagnostic::UnnecessaryConflicts(_)))
        .collect();
    if show_diagnostics {
        for diagnostic in &diagnostics {
            println!("{diagnostic}");
        }
    } else {
        for diagnostic in &unnecessary {
            println!("{diagnostic}");
        }
        let count = diagnostics.len() - unnecessary.len();
        if count > 0 {
            let name = if count == 1 { "diagnostic" } else { "diagnostics" };
            println!(
                "the generator reports {count} more {name}. Run `cargo xtask generate --diagnostics` to read them."
            );
        }
    }
    // A conflict set that the parse table builder does not use stops the report of a later conflict
    // of the same symbols. The generator then makes a GLR split with no word, the parser selects one
    // reading by the symbol order, and a silent misparse follows. With the set removed, the
    // generator stops with an error and names the symbols. Declare only the sets that it names.
    if !unnecessary.is_empty() {
        let count: usize = unnecessary
            .iter()
            .map(|diagnostic| match diagnostic {
                Diagnostic::UnnecessaryConflicts(conflicts) => conflicts.len(),
                _ => 0,
            })
            .sum();
        let name = if count == 1 { "set" } else { "sets" };
        return Err(format!(
            "the grammar declares {count} conflict {name} that the parse table builder does not use. \
             Remove each set above from `g.conflicts`."
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The generator in vendor/tree-sitter-generate writes the header, the ABI version, and the table
    /// layout that the runtime in vendor/tree-sitter reads.
    #[test]
    fn the_generator_agrees_with_the_runtime() {
        assert!(
            tree_sitter_generate::PARSER_HEADER == tree_sitter::PARSER_HEADER,
            "vendor/tree-sitter-generate/src/parser.h.inc does not agree with vendor/tree-sitter/src/parser.h. \
             Copy the header of the runtime to the generator."
        );
        let api = include_str!("../../vendor/tree-sitter/include/tree_sitter/api.h");
        let version = |name: &str| -> usize {
            api.lines()
                .find_map(|line| line.strip_prefix(&format!("#define {name} ")))
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or_else(|| panic!("api.h has no numeric define {name}"))
        };
        assert_eq!(version("TREE_SITTER_LANGUAGE_VERSION"), tree_sitter::LANGUAGE_VERSION);
        assert_eq!(
            version("TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION"),
            tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION
        );
        // The generator writes only the ABI of the fork. The runtime also reads the upstream versions.
        assert_eq!(tree_sitter_generate::ABI_VERSION_MAX, tree_sitter::LANGUAGE_VERSION);
        assert_eq!(tree_sitter_generate::ABI_VERSION_MIN, tree_sitter::LANGUAGE_VERSION);
        assert!(tree_sitter::is_supported_language_version(14));
        assert!(tree_sitter::is_supported_language_version(15));
        assert!(!tree_sitter::is_supported_language_version(16));
        assert!(tree_sitter::is_supported_language_version(tree_sitter::LANGUAGE_VERSION));
    }
}
