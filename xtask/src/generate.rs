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
    /// The external tokens of the grammar and the enumerators of the scanner are two hand-written
    /// lists, in two files, that each reader of `valid_symbols` uses as ONE list. A new token appends
    /// to both, so two agents that add a token at the same time get a conflict in each file. A REBASE
    /// THAT KEEPS ONE ORDER IN ONE FILE AND THE OTHER ORDER IN THE OTHER GIVES THE SAME COUNT WITH TWO
    /// SLOTS EXCHANGED. `tree_sitter_cpp_external_scanner_create` compares the counts only, so it
    /// passes, and the scanner then answers for a token that the parser did not ask for. That is the
    /// failure of #272, where one parse of a 20 KB file took 152 GB, with a count that agrees. A GUARD
    /// THAT COMPARES A SUMMARY OF TWO THINGS PASSES EVERY DEFECT THAT KEEPS THE SUMMARY: a count sees
    /// an addition and a removal, and it is blind to an exchange. Five conflicts of that shape came
    /// from one day of work.
    #[test]
    fn the_external_tokens_of_the_scanner_agree_with_the_grammar() {
        let repository = crate::repository();
        let text = std::fs::read_to_string(repository.join("src").join("grammar.json"))
            .expect("the repository has src/grammar.json");
        let grammar: serde_json::Value = serde_json::from_str(&text).expect("src/grammar.json is JSON");
        let externals = grammar["externals"].as_array().expect("src/grammar.json has externals");
        let scanner = std::fs::read_to_string(repository.join("src").join("scanner.c"))
            .expect("the repository has src/scanner.c");
        let start = scanner.find("enum TokenType {").expect("src/scanner.c has enum TokenType");
        let end = start + scanner[start..].find("\n};").expect("the enum of src/scanner.c ends");
        let names: Vec<&str> = scanner[start..end]
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with("//"))
            .map(|line| line.trim_end_matches(','))
            .filter(|line| {
                !line.is_empty() && line.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            })
            .collect();
        assert_eq!(
            names.last().copied(),
            Some("TOKEN_TYPE_COUNT"),
            "TOKEN_TYPE_COUNT is the last enumerator of `TokenType`, because the check at the load of the \
             language reads it as the count of the tokens"
        );
        let tokens = &names[..names.len() - 1];
        assert_eq!(
            tokens.len(),
            externals.len(),
            "src/scanner.c holds {} tokens and src/grammar.json holds {} externals. A new token goes into \
             the enum of src/scanner.c AND into `g.externals` of grammar/src/cpp.rs, in the same place.",
            tokens.len(),
            externals.len()
        );
        for (slot, (external, name)) in externals.iter().zip(tokens.iter()).enumerate() {
            match external["name"].as_str() {
                Some(symbol) => {
                    let expected = symbol.trim_start_matches('_').to_uppercase();
                    assert_eq!(
                        expected.as_str(),
                        *name,
                        "slot {slot} holds `{symbol}` in src/grammar.json and `{name}` in src/scanner.c. \
                         Each token owns one slot in the two lists, and a slot with two names makes the \
                         scanner answer for a different token."
                    );
                }
                // A string external gives the token of a text, and it takes its name in the scanner
                // alone. The three values are here, so that a fourth one is a change of this test.
                None => {
                    let value = external["value"].as_str().unwrap_or_default();
                    assert!(
                        matches!(value, "-" | "+" | "["),
                        "slot {slot} of src/grammar.json is the string external `{value}`, which this test \
                         does not know. Write the value here, beside the enumerator `{name}` of the scanner."
                    );
                }
            }
        }
    }

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
        // ABI 1019 adds no field of the language struct, and a grammar of each earlier ABI of this
        // fork still loads by name. A version that no ABI of this fork uses is refused, which is what
        // stops a parser of a later generator from loading in this runtime.
        assert_eq!(tree_sitter::LANGUAGE_VERSION, 1019);
        assert!(tree_sitter::is_supported_language_version(tree_sitter::SCANNER_CONTEXT_LANGUAGE_VERSION));
        for version in tree_sitter::FORK_LANGUAGE_VERSIONS {
            assert!(tree_sitter::is_supported_language_version(version), "ABI {version} loads by name");
        }
        assert!(!tree_sitter::is_supported_language_version(1020));

        // THE TWO LISTS OF ACCEPTED VERSIONS MUST HOLD THE SAME VERSIONS. One is
        // `ts_language_version_is_supported` of vendor/tree-sitter/src/language.h, which the C
        // runtime applies. The other is `is_supported_language_version` of the Rust binding, which
        // `Parser::set_language` applies BEFORE it calls the runtime. A version that only the C list
        // holds is a parser that the runtime reads and that a Rust consumer cannot load, with
        // `LanguageError` and no other sign. The Rust list named the current version in the place of
        // its newest entry until ABI 1019, so each bump dropped the ABI before it from that side.
        let language_h = include_str!("../../vendor/tree-sitter/src/language.h");
        let defined = |name: &str| -> usize {
            language_h
                .lines()
                .find_map(|line| line.strip_prefix(&format!("#define {name} ")))
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or_else(|| panic!("language.h has no numeric define {name}"))
        };
        let mut from_c: Vec<usize> = language_h
            .lines()
            .skip_while(|line| !line.contains("ts_language_version_is_supported"))
            .take_while(|line| !line.trim_start().starts_with('}'))
            .filter_map(|line| line.split("version ==").nth(1))
            .map(|rest| defined(rest.trim().trim_end_matches(" ||").trim_end_matches(';')))
            .collect();
        from_c.sort_unstable();
        assert!(
            from_c.len() >= 4,
            "the scan of ts_language_version_is_supported found {} versions and it holds at least 4. \
             The pattern no longer matches language.h, so the two lists are not compared.",
            from_c.len()
        );
        let mut from_rust = tree_sitter::FORK_LANGUAGE_VERSIONS.to_vec();
        from_rust.sort_unstable();
        assert_eq!(
            from_c, from_rust,
            "ts_language_version_is_supported of language.h and FORK_LANGUAGE_VERSIONS of the Rust \
             binding hold different versions. A consumer of the Rust binding refuses a parser that \
             the C runtime reads."
        );
        assert_eq!(
            *from_rust.last().expect("the list is not empty"),
            tree_sitter::LANGUAGE_VERSION,
            "the newest ABI of the fork is not the version that the generator writes"
        );
    }

    /// The generator writes the entry point `<name>_external_scanner_set_context` only for a grammar
    /// whose grammar.json sets `external_scanner_set_context`. The scanner of an upstream grammar
    /// does not define the function, and a parser that names it does not link: 209 of 308 tests of the
    /// tree-sitter CLI failed with the fixtures that the generator of the fork wrote. This grammar sets
    /// the key, so its parser keeps the entry point.
    #[test]
    fn the_generator_names_the_scanner_context_entry_point_only_for_a_grammar_that_declares_it() {
        let grammar = |key: &str| {
            format!(
                r#"{{"name": "probe", "rules": {{"source": {{"type": "SYMBOL", "name": "token"}}}},
                "externals": [{{"type": "SYMBOL", "name": "token"}}]{key}}}"#
            )
        };
        let render = |json: &str| {
            let mut diagnostics = Vec::new();
            let (name, code) =
                tree_sitter_generate::generate_parser_for_grammar(
                    json,
                    None,
                    tree_sitter_generate::OptLevel::default(),
                    &mut diagnostics,
                )
                    .expect("the probe grammar generates");
            assert_eq!(name, "probe");
            code
        };
        let undeclared = render(&grammar(""));
        assert!(undeclared.contains("tree_sitter_probe_external_scanner_scan"), "the probe has a scanner");
        assert!(
            !undeclared.contains("_set_context"),
            "a grammar that does not declare the entry point must not name it"
        );
        let declared = render(&grammar(r#", "external_scanner_set_context": true"#));
        assert!(declared.contains("void tree_sitter_probe_external_scanner_set_context(void *, const void *);"));
        assert!(declared.contains(".external_scanner_set_context = tree_sitter_probe_external_scanner_set_context,"));

        let repository = crate::repository();
        let text = std::fs::read_to_string(repository.join("src").join("grammar.json"))
            .expect("the repository has src/grammar.json");
        let json: serde_json::Value = serde_json::from_str(&text).expect("src/grammar.json is JSON");
        assert_eq!(json["external_scanner_set_context"], serde_json::Value::Bool(true));
        let parser = std::fs::read_to_string(repository.join("src").join("parser.c"))
            .expect("the repository has src/parser.c");
        assert!(parser.contains(".external_scanner_set_context = tree_sitter_cpp_external_scanner_set_context,"));
    }
}
