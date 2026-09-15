//! The committed `src/grammar.json` must agree with the Rust grammar.

use tree_sitter_cpp_grammar::grammar;

#[test]
fn the_committed_grammar_json_agrees_with_the_rust_grammar() {
    let committed = include_str!("../../src/grammar.json");
    let generated = serde_json::to_string_pretty(&grammar().to_json()).expect("a grammar serializes to JSON");
    assert!(
        committed == generated,
        "src/grammar.json does not agree with the Rust grammar. Run `cargo xtask generate`."
    );
}

#[test]
fn the_grammar_defines_each_name_that_it_uses() {
    assert_eq!(grammar().undefined_names(), Vec::<String>::new());
}
