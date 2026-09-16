//! Compare each dynamic precedence of the grammar with the parse table.
//!
//! A dynamic precedence selects one reading of an ambiguity after the parse. The generator puts the
//! value on each production of the rule that declares it, and it writes the value into each reduce
//! action of that production. A step of the generator can remove a production, and it gives no
//! report for such a removal. The value then has no effect, and the parser selects a reading by the
//! order of the symbols. No error count shows it.
//!
//! `cargo xtask precedence` reads `src/grammar.json` and `src/parser.c`, and it fails when the parse
//! table holds no reduce action with a value that the grammar declares. The test
//! `each_dynamic_precedence_of_the_grammar_is_in_the_parse_table` runs the same comparison.
//!
//! The comparison uses two rules of the generator:
//!
//! - The flattener takes the value with the largest absolute value of the wrappers of one production
//!   (`flatten_grammar.rs`, `FlattenState::dyn_prec`). A value inside a wrapper with a larger
//!   absolute value never reaches a production, and this command then reports it.
//! - `expand_repeats` moves a wrapper inside a `repeat` to an auxiliary rule. The value is then on
//!   `aux_sym_NAME_repeatN`, and the comparison reads the auxiliary symbols of the rule too.
//!
//! The comparison reports a rule of the inline list and does not compare it. Such a rule has no
//! symbol of its own, and its productions go into the rules that use it.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The dynamic precedences of one rule of the grammar, and the values in the parse table.
#[derive(Debug, PartialEq, Eq)]
pub struct Rule {
    /// The name of the rule in the grammar.
    pub name: String,
    /// The values that the rule declares.
    pub declared: BTreeSet<i32>,
    /// The values of the reduce actions of the symbol of the rule and of its auxiliary symbols.
    pub found: BTreeSet<i32>,
    /// True when the rule is in the inline list of the grammar.
    pub inlined: bool,
    /// True when the parser has no symbol for the rule.
    pub no_symbol: bool,
}

impl Rule {
    /// The declared values that no reduce action of the rule holds.
    fn lost(&self) -> Vec<i32> {
        if self.inlined {
            return Vec::new();
        }
        self.declared.iter().filter(|value| !self.found.contains(value)).copied().collect()
    }

    /// One line with the name, the declared values, and the values in the parse table.
    fn describe(&self) -> String {
        let list = |values: &BTreeSet<i32>| {
            values.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
        };
        let state = if self.inlined {
            "in the inline list, and the comparison skips it".to_owned()
        } else if self.no_symbol {
            "the parser has no symbol for the rule".to_owned()
        } else {
            format!("the parse table has {}", list(&self.found))
        };
        format!("{}: the grammar declares {}, and {state}", self.name, list(&self.declared))
    }
}

/// Add each dynamic precedence of a rule body to `values`. O(n) in the size of the body.
fn collect_values(node: &Value, values: &mut BTreeSet<i32>) {
    let Some(map) = node.as_object() else { return };
    if map.get("type").and_then(Value::as_str) == Some("PREC_DYNAMIC")
        && let Some(value) = map.get("value").and_then(Value::as_i64)
        && let Ok(value) = i32::try_from(value)
    {
        values.insert(value);
    }
    if let Some(content) = map.get("content") {
        collect_values(content, values);
    }
    if let Some(members) = map.get("members").and_then(Value::as_array) {
        for member in members {
            collect_values(member, values);
        }
    }
}

/// The symbol index of each identifier of `enum ts_symbol_identifiers` in a parser file.
///
/// The identifiers hold the names of the rules of the grammar, and `ts_symbol_names` holds the names
/// after the default aliases. The comparison needs the names of the rules. O(n) in the text.
fn symbol_indices(parser: &str) -> Result<BTreeMap<String, u32>, String> {
    let start = parser
        .find("enum ts_symbol_identifiers {")
        .ok_or("the parser file has no enum ts_symbol_identifiers")?;
    let end = parser[start..]
        .find("\n};")
        .ok_or("the enum ts_symbol_identifiers has no end")?;
    let mut indices = BTreeMap::new();
    for line in parser[start..start + end].lines() {
        let Some((name, value)) = line.split_once('=') else { continue };
        let name = name.trim();
        let value = value.trim().trim_end_matches(',').trim();
        if let Ok(index) = value.parse::<u32>() {
            indices.insert(name.to_owned(), index);
        }
    }
    if indices.is_empty() {
        return Err("the enum ts_symbol_identifiers has no entry".into());
    }
    Ok(indices)
}

/// One reduce action of a parser file: the symbol, the child count, and the dynamic precedence.
type Reduce = (u32, u32, i32);

/// The reduce actions of a parser file.
///
/// The generator writes a reduce action as `R(symbol, child count, dynamic precedence, production)`.
/// O(n) in the text.
fn reduce_actions(parser: &str) -> BTreeSet<Reduce> {
    let mut actions = BTreeSet::new();
    let bytes = parser.as_bytes();
    let mut at = 0usize;
    while let Some(found) = parser[at..].find("R(") {
        let start = at + found;
        at = start + 2;
        // `R(` is the start of a name only after a character that no name holds. `SR(` is a shift of
        // a repetition, and the two have different fields.
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            continue;
        }
        let Some(close) = parser[at..].find(')') else { break };
        let fields: Vec<&str> = parser[at..at + close].split(',').collect();
        if fields.len() != 4 {
            continue;
        }
        let (Ok(symbol), Ok(children), Ok(precedence)) = (
            fields[0].trim().parse::<u32>(),
            fields[1].trim().parse::<u32>(),
            fields[2].trim().parse::<i32>(),
        ) else {
            continue;
        };
        actions.insert((symbol, children, precedence));
    }
    actions
}

/// Compare the dynamic precedences of a grammar with the reduce actions of a parser file.
///
/// The result holds one entry for each rule of the grammar that declares a dynamic precedence, in
/// the order of the names. O(n) in the size of the grammar and of the parser file.
///
/// # Errors
///
/// If the grammar is not an object with rules, or if the parser file has no symbol enum.
pub fn compare(grammar: &str, parser: &str) -> Result<Vec<Rule>, String> {
    let grammar: Value = serde_json::from_str(grammar).map_err(|e| format!("the grammar is not JSON: {e}"))?;
    let rules = grammar
        .get("rules")
        .and_then(Value::as_object)
        .ok_or("the grammar has no rules")?;
    let inline: BTreeSet<&str> = grammar
        .get("inline")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let indices = symbol_indices(parser)?;
    let actions = reduce_actions(parser);
    let mut precedences: BTreeMap<u32, BTreeSet<i32>> = BTreeMap::new();
    for &(symbol, _, precedence) in &actions {
        precedences.entry(symbol).or_default().insert(precedence);
    }

    let mut out = Vec::new();
    for (name, body) in rules {
        let mut declared = BTreeSet::new();
        collect_values(body, &mut declared);
        if declared.is_empty() {
            continue;
        }
        // The symbol of the rule, and the auxiliary symbols that `expand_repeats` made from it.
        let mut found = BTreeSet::new();
        let mut no_symbol = true;
        for (identifier, index) in &indices {
            let Some(rest) = identifier
                .strip_prefix("sym_")
                .or_else(|| identifier.strip_prefix("aux_sym_"))
                .or_else(|| identifier.strip_prefix("alias_sym_"))
            else {
                continue;
            };
            if rest == name || rest.strip_prefix(name.as_str()).is_some_and(|tail| tail.starts_with("_repeat")) {
                no_symbol = false;
                if let Some(values) = precedences.get(index) {
                    found.extend(values.iter().copied());
                }
            }
        }
        out.push(Rule {
            name: name.clone(),
            declared,
            found,
            inlined: inline.contains(name.as_str()),
            no_symbol,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// The reduce actions of a parser file that have no child and a dynamic precedence.
///
/// `ts_subtree_dynamic_precedence` of vendor/tree-sitter/src/subtree.h gives 0 for a node with no
/// child, so the runtime reads no such value. The generator gives no report for it.
///
/// # Errors
///
/// If the parser file has no symbol enum.
pub fn empty_reductions(parser: &str) -> Result<Vec<String>, String> {
    let indices = symbol_indices(parser)?;
    let names: BTreeMap<u32, &str> = indices.iter().map(|(name, index)| (*index, name.as_str())).collect();
    let mut out: Vec<String> = reduce_actions(parser)
        .iter()
        .filter(|&&(_, children, precedence)| children == 0 && precedence != 0)
        .map(|&(symbol, _, precedence)| {
            let name = names.get(&symbol).copied().unwrap_or("an unknown symbol");
            format!("{name} with the dynamic precedence {precedence}")
        })
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

/// Read `src/grammar.json` and a parser file, and compare their dynamic precedences.
///
/// # Errors
///
/// If a file cannot be read, or if `compare` gives an error.
pub fn measure(repository: &Path, parser: &Path) -> Result<Vec<Rule>, Box<dyn Error>> {
    let grammar_path = repository.join("src").join("grammar.json");
    let grammar =
        fs::read_to_string(&grammar_path).map_err(|e| format!("cannot read {}: {e}", grammar_path.display()))?;
    let text = fs::read_to_string(parser).map_err(|e| format!("cannot read {}: {e}", parser.display()))?;
    Ok(compare(&grammar, &text).map_err(|e| format!("{}: {e}", parser.display()))?)
}

/// Print the dynamic precedences of the grammar and of the parse table.
///
/// # Errors
///
/// If the parse table holds no reduce action with a value that the grammar declares, or if `measure`
/// gives an error.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let path = match args {
        [] => repository.join("src").join("parser.c"),
        [path] => PathBuf::from(path),
        _ => return Err("usage: cargo xtask precedence [PARSER_C]".into()),
    };
    let rules = measure(repository, &path)?;
    for rule in &rules {
        println!("{}", rule.describe());
    }
    let lost: Vec<&Rule> = rules.iter().filter(|rule| !rule.lost().is_empty()).collect();
    if !lost.is_empty() {
        let names: Vec<String> = lost
            .iter()
            .map(|rule| format!("{} ({:?})", rule.name, rule.lost()))
            .collect();
        let name = if lost.len() == 1 { "precedence" } else { "precedences" };
        return Err(format!(
            "the parse table holds no reduce action with {} dynamic {name} of the grammar: {}. \
             A value with no reduce action selects no reading.",
            lost.len(),
            names.join(", ")
        )
        .into());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let empty = empty_reductions(&text)?;
    if !empty.is_empty() {
        let name = if empty.len() == 1 { "action" } else { "actions" };
        return Err(format!(
            "{} reduce {name} of the parse table has no child and a dynamic precedence: {}. \
             `ts_subtree_dynamic_precedence` gives 0 for a node with no child, and the runtime \
             reads no such value.",
            empty.len(),
            empty.join(", ")
        )
        .into());
    }
    println!("{} rules declare a dynamic precedence, and the parse table holds each value", rules.len());
    println!("no reduce action with a dynamic precedence has a child count of 0");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grammar with a dynamic precedence on a unit rule, on a rule with more steps, and in a
    /// repeat.
    const GRAMMAR: &str = r#"{
      "name": "sample",
      "inline": ["_inlined"],
      "rules": {
        "unit": {"type": "CHOICE", "members": [
          {"type": "PREC_DYNAMIC", "value": 3, "content": {"type": "SYMBOL", "name": "a"}},
          {"type": "PREC_DYNAMIC", "value": 1, "content": {"type": "SYMBOL", "name": "b"}}
        ]},
        "pair": {"type": "SEQ", "members": [
          {"type": "SYMBOL", "name": "a"},
          {"type": "PREC_DYNAMIC", "value": -10, "content": {"type": "SYMBOL", "name": "b"}}
        ]},
        "many": {"type": "REPEAT1", "content":
          {"type": "PREC_DYNAMIC", "value": -1000, "content": {"type": "SYMBOL", "name": "a"}}
        },
        "_inlined": {"type": "PREC_DYNAMIC", "value": 7, "content": {"type": "SYMBOL", "name": "a"}}
      }
    }"#;

    /// A parser file with the reduce actions of `GRAMMAR`. The repeat of `many` is an auxiliary
    /// symbol, and the value of `many` is on it.
    const PARSER: &str = "\
enum ts_symbol_identifiers {
  sym_a = 1,
  sym_b = 2,
  sym_unit = 3,
  sym_pair = 4,
  sym_many = 5,
  aux_sym_many_repeat1 = 6,
};

static const TSParseActionEntry ts_parse_actions[] = {
E(1,1),R(3,1,3,0),E(1,1),R(3,1,1,0),E(1,1),R(4,2,-10,0),E(1,1),R(5,1,0,0),
E(1,1),R(6,1,-1000,0),E(1,1),R(6,2,-1000,0),
};
";

    /// The comparison finds each value of the grammar in the parse table.
    #[test]
    fn the_sample_parser_holds_each_value_of_the_sample_grammar() {
        let rules = compare(GRAMMAR, PARSER).expect("the sample compares");
        let names: Vec<&str> = rules.iter().map(|rule| rule.name.as_str()).collect();
        assert_eq!(names, ["_inlined", "many", "pair", "unit"]);
        let of = |name: &str| rules.iter().find(|rule| rule.name == name).expect("the rule is in the result");
        assert_eq!(of("unit").declared, BTreeSet::from([1, 3]));
        assert_eq!(of("unit").found, BTreeSet::from([1, 3]));
        assert_eq!(of("pair").found, BTreeSet::from([-10]));
        // `expand_repeats` puts the value of a repeat on the auxiliary symbol.
        assert_eq!(of("many").declared, BTreeSet::from([-1000]));
        assert_eq!(of("many").found, BTreeSet::from([0, -1000]));
        assert!(of("_inlined").inlined);
        for rule in &rules {
            assert!(rule.lost().is_empty(), "{}", rule.describe());
        }
    }

    /// A reduce action with no child and a dynamic precedence has no effect in the runtime.
    #[test]
    fn a_reduce_action_with_no_child_and_a_precedence_is_reported() {
        assert!(empty_reductions(PARSER).expect("the sample has a symbol enum").is_empty());
        let parser = PARSER.replace("R(5,1,0,0)", "R(5,0,-7,0)");
        let found = empty_reductions(&parser).expect("the sample has a symbol enum");
        assert_eq!(found, vec!["sym_many with the dynamic precedence -7".to_owned()]);
    }

    /// The parse table of the repository has no reduce action with no child and a dynamic
    /// precedence.
    ///
    /// `ts_subtree_dynamic_precedence` of vendor/tree-sitter/src/subtree.h gives 0 for a node with
    /// no child. Such a value is in the table, and the runtime never reads it.
    #[test]
    fn no_reduce_action_of_the_parse_table_has_no_child_and_a_precedence() {
        let path = crate::repository().join("src").join("parser.c");
        let parser = fs::read_to_string(&path).expect("the repository has src/parser.c");
        let found = empty_reductions(&parser).expect("src/parser.c has a symbol enum");
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    /// A parse table with no reduce action of a value gives a lost value.
    #[test]
    fn a_value_with_no_reduce_action_is_lost() {
        let parser = PARSER.replace("R(3,1,3,0)", "R(3,1,0,0)");
        let rules = compare(GRAMMAR, &parser).expect("the sample compares");
        let unit = rules.iter().find(|rule| rule.name == "unit").expect("the rule is in the result");
        assert_eq!(unit.lost(), vec![3]);
    }

    /// The parse table of the repository holds each dynamic precedence of the grammar.
    ///
    /// The generator writes the value of a production into each of its reduce actions
    /// (`build_parse_table.rs`, the `ParseAction::Reduce` of a finished item). A step that removes a
    /// production gives no report, and the value then selects no reading. This test finds that.
    #[test]
    fn each_dynamic_precedence_of_the_grammar_is_in_the_parse_table() {
        let repository = crate::repository();
        let rules = measure(&repository, &repository.join("src").join("parser.c"))
            .expect("the grammar and the parser of the repository compare");
        assert!(
            rules.len() > 50,
            "the grammar of the repository declares {} dynamic precedences",
            rules.len()
        );
        let lost: Vec<String> = rules
            .iter()
            .filter(|rule| !rule.lost().is_empty())
            .map(Rule::describe)
            .collect();
        assert!(lost.is_empty(), "{}", lost.join("\n"));
    }
}
