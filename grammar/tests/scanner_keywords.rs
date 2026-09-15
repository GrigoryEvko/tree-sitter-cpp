//! The external scanner keeps a list of the identifier-shaped strings of the grammar. The list must
//! agree with the Rust grammar.

use std::collections::BTreeSet;

use serde_json::Value;
use tree_sitter_cpp_grammar::grammar;

/// Collect the values of the STRING rules that have the shape of an identifier. O(n) in the rules.
fn identifier_strings(rule: &Value, found: &mut BTreeSet<String>) {
    match rule {
        Value::Object(fields) => {
            if fields.get("type").and_then(Value::as_str) == Some("STRING")
                && let Some(text) = fields.get("value").and_then(Value::as_str)
                && text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                found.insert(text.to_owned());
            }
            fields.values().for_each(|value| identifier_strings(value, found));
        }
        Value::Array(items) => items.iter().for_each(|item| identifier_strings(item, found)),
        _ => {}
    }
}

#[test]
fn the_scanner_keywords_agree_with_the_grammar() {
    let scanner = include_str!("../../src/scanner.c");
    let start = scanner
        .find("GRAMMAR_KEYWORDS[] = {")
        .expect("src/scanner.c defines GRAMMAR_KEYWORDS");
    let end = start
        + scanner[start..]
            .find("};")
            .expect("the GRAMMAR_KEYWORDS list ends with `};`");
    let listed: Vec<&str> = scanner[start..end].split('"').skip(1).step_by(2).collect();

    let mut sorted = listed.clone();
    sorted.sort_unstable();
    assert_eq!(
        listed, sorted,
        "GRAMMAR_KEYWORDS in src/scanner.c is not in the order of `strcmp`."
    );

    let mut expected = BTreeSet::new();
    identifier_strings(&grammar().to_json(), &mut expected);
    let listed: BTreeSet<String> = listed.into_iter().map(str::to_owned).collect();
    assert_eq!(
        listed, expected,
        "GRAMMAR_KEYWORDS in src/scanner.c does not agree with the identifier-shaped strings of the grammar."
    );
}
