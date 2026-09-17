//! The tree-sitter grammar DSL in Rust.
//!
//! Each builder gives the same rule as the JavaScript function with the same name in
//! the `dsl.js` file of tree-sitter 0.26. `Grammar::to_json` gives the value that
//! `tree-sitter generate` writes to `src/grammar.json`, with the same key order.

use std::collections::HashSet;

use indexmap::IndexMap;
use serde_json::{Map, Value};

/// The JSON schema that each `grammar.json` names.
const SCHEMA: &str = "https://tree-sitter.github.io/tree-sitter/assets/schemas/grammar.schema.json";

/// One rule of a grammar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rule {
    /// The empty string.
    Blank,
    /// A literal string.
    String(String),
    /// A regular expression, with the flags of a JavaScript regular expression.
    Pattern { value: String, flags: Option<String> },
    /// A reference to a rule or to an external token.
    Symbol(String),
    /// The members, one after the other.
    Seq(Vec<Rule>),
    /// One of the members.
    Choice(Vec<Rule>),
    /// The content, zero or more times.
    Repeat(Box<Rule>),
    /// The content, one or more times.
    Repeat1(Box<Rule>),
    /// The content, as a child with a field name.
    Field { name: String, content: Box<Rule> },
    /// The content, with a different node name. A named alias gives a named node.
    Alias {
        content: Box<Rule>,
        named: bool,
        value: String,
    },
    /// The content, with a precedence.
    Prec {
        kind: PrecKind,
        value: i32,
        content: Box<Rule>,
    },
    /// The content as one token. An immediate token has no extras before it.
    Token { immediate: bool, content: Box<Rule> },
    /// The content, with the reserved words of a named word set.
    Reserved { context: String, content: Box<Rule> },
}

/// The kind of a precedence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrecKind {
    /// `prec`: selects a parse action when the parse table is built.
    Plain,
    /// `prec.left`: left associativity at equal precedence.
    Left,
    /// `prec.right`: right associativity at equal precedence.
    Right,
    /// `prec.dynamic`: selects among the parses that stay after a conflict, at parse time.
    Dynamic,
}

impl PrecKind {
    /// The `type` of the rule in `grammar.json`.
    fn json_type(self) -> &'static str {
        match self {
            Self::Plain => "PREC",
            Self::Left => "PREC_LEFT",
            Self::Right => "PREC_RIGHT",
            Self::Dynamic => "PREC_DYNAMIC",
        }
    }
}

impl Rule {
    /// The JSON value of the rule, with the key order of `dsl.js`.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        let mut put = |key: &str, value: Value| {
            map.insert(key.to_owned(), value);
        };
        match self {
            Self::Blank => put("type", "BLANK".into()),
            Self::String(value) => {
                put("type", "STRING".into());
                put("value", value.as_str().into());
            }
            Self::Pattern { value, flags } => {
                put("type", "PATTERN".into());
                put("value", value.as_str().into());
                if let Some(flags) = flags {
                    put("flags", flags.as_str().into());
                }
            }
            Self::Symbol(name) => {
                put("type", "SYMBOL".into());
                put("name", name.as_str().into());
            }
            Self::Seq(members) | Self::Choice(members) => {
                put(
                    "type",
                    if matches!(self, Self::Seq(_)) { "SEQ" } else { "CHOICE" }.into(),
                );
                put("members", members.iter().map(Rule::to_json).collect());
            }
            Self::Repeat(content) | Self::Repeat1(content) => {
                put(
                    "type",
                    if matches!(self, Self::Repeat(_)) {
                        "REPEAT"
                    } else {
                        "REPEAT1"
                    }
                    .into(),
                );
                put("content", content.to_json());
            }
            Self::Field { name, content } => {
                put("type", "FIELD".into());
                put("name", name.as_str().into());
                put("content", content.to_json());
            }
            Self::Alias { content, named, value } => {
                put("type", "ALIAS".into());
                put("content", content.to_json());
                put("named", (*named).into());
                put("value", value.as_str().into());
            }
            Self::Prec { kind, value, content } => {
                put("type", kind.json_type().into());
                put("value", (*value).into());
                put("content", content.to_json());
            }
            Self::Token { immediate, content } => {
                put("type", if *immediate { "IMMEDIATE_TOKEN" } else { "TOKEN" }.into());
                put("content", content.to_json());
            }
            Self::Reserved { context, content } => {
                put("type", "RESERVED".into());
                put("content", content.to_json());
                put("context_name", context.as_str().into());
            }
        }
        Value::Object(map)
    }

    /// Call `visit` with the name of each symbol in the rule. O(n) in the size of the rule.
    pub fn visit_symbols<'a>(&'a self, visit: &mut impl FnMut(&'a str)) {
        match self {
            Self::Symbol(name) => visit(name),
            Self::Seq(members) | Self::Choice(members) => {
                for member in members {
                    member.visit_symbols(visit);
                }
            }
            Self::Repeat(content)
            | Self::Repeat1(content)
            | Self::Field { content, .. }
            | Self::Alias { content, .. }
            | Self::Prec { content, .. }
            | Self::Token { content, .. }
            | Self::Reserved { content, .. } => content.visit_symbols(visit),
            Self::Blank | Self::String(_) | Self::Pattern { .. } => {}
        }
    }

    /// The members of a sequence or a choice.
    ///
    /// # Panics
    ///
    /// If the rule is not a sequence or a choice. A grammar that changes the members of a
    /// rule with a different shape has an error in its source code.
    pub fn into_members(self) -> Vec<Rule> {
        match self {
            Self::Seq(members) | Self::Choice(members) => members,
            other => panic!("the rule is not a sequence or a choice: {other:?}"),
        }
    }
}

impl From<&str> for Rule {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for Rule {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

/// The node name of an alias.
pub enum AliasTarget {
    /// A named node. `alias(x, $.name)` in `grammar.js`.
    Named(String),
    /// An anonymous node. `alias(x, 'text')` in `grammar.js`.
    Anonymous(String),
}

impl From<Rule> for AliasTarget {
    /// # Panics
    ///
    /// If the rule is not a symbol.
    fn from(rule: Rule) -> Self {
        match rule {
            Rule::Symbol(name) => Self::Named(name),
            other => panic!("the target of an alias must be a symbol or a string, not {other:?}"),
        }
    }
}

impl From<&str> for AliasTarget {
    fn from(value: &str) -> Self {
        Self::Anonymous(value.to_owned())
    }
}

impl From<String> for AliasTarget {
    fn from(value: String) -> Self {
        Self::Anonymous(value)
    }
}

/// A sequence: `seq![a, "b", c]`. Each member is a `Rule` or a string.
#[macro_export]
macro_rules! seq {
    ($($member:expr),* $(,)?) => {
        $crate::dsl::Rule::Seq(vec![$($crate::dsl::Rule::from($member)),*])
    };
}

/// A choice: `choice![a, "b", c]`. Each member is a `Rule` or a string.
#[macro_export]
macro_rules! choice {
    ($($member:expr),* $(,)?) => {
        $crate::dsl::Rule::Choice(vec![$($crate::dsl::Rule::from($member)),*])
    };
}

/// A symbol: `s!(expression)` is `$.expression` in `grammar.js`.
#[macro_export]
macro_rules! s {
    ($name:tt) => {
        $crate::dsl::sym(stringify!($name))
    };
}

/// The empty string.
pub fn blank() -> Rule {
    Rule::Blank
}

/// A reference to a rule or to an external token.
pub fn sym(name: &str) -> Rule {
    Rule::Symbol(name.to_owned())
}

/// A regular expression. The text is the source of a JavaScript regular expression.
pub fn re(pattern: &str) -> Rule {
    Rule::Pattern {
        value: pattern.to_owned(),
        flags: None,
    }
}

/// A sequence of the rules of an iterator.
pub fn seq_of(members: impl IntoIterator<Item = Rule>) -> Rule {
    Rule::Seq(members.into_iter().collect())
}

/// A choice among the rules of an iterator.
pub fn choice_of(members: impl IntoIterator<Item = Rule>) -> Rule {
    Rule::Choice(members.into_iter().collect())
}

/// The rule or the empty string.
pub fn optional(rule: impl Into<Rule>) -> Rule {
    Rule::Choice(vec![rule.into(), Rule::Blank])
}

/// The rule, zero or more times.
pub fn repeat(rule: impl Into<Rule>) -> Rule {
    Rule::Repeat(Box::new(rule.into()))
}

/// The rule, one or more times.
pub fn repeat1(rule: impl Into<Rule>) -> Rule {
    Rule::Repeat1(Box::new(rule.into()))
}

/// The rule, as a child with a field name.
pub fn field(name: &str, rule: impl Into<Rule>) -> Rule {
    Rule::Field {
        name: name.to_owned(),
        content: Box::new(rule.into()),
    }
}

/// The rule, with a different node name.
pub fn alias(rule: impl Into<Rule>, target: impl Into<AliasTarget>) -> Rule {
    let (named, value) = match target.into() {
        AliasTarget::Named(value) => (true, value),
        AliasTarget::Anonymous(value) => (false, value),
    };
    Rule::Alias {
        content: Box::new(rule.into()),
        named,
        value,
    }
}

/// The rule as one token.
pub fn token(rule: impl Into<Rule>) -> Rule {
    Rule::Token {
        immediate: false,
        content: Box::new(rule.into()),
    }
}

/// The rule as one token with no extras before it. `token.immediate` in `grammar.js`.
pub fn token_immediate(rule: impl Into<Rule>) -> Rule {
    Rule::Token {
        immediate: true,
        content: Box::new(rule.into()),
    }
}

/// The rule, with the reserved words of the word set `context`. `reserved(context, rule)` in
/// `grammar.js`.
///
/// A reserved word set applies only to the steps of the rule that are the word token. In a parse state
/// that expects the word token at such a step, the lexer gives a reserved word as its keyword, also when
/// the state has no action for the keyword.
pub fn reserved(context: &str, rule: impl Into<Rule>) -> Rule {
    Rule::Reserved {
        context: context.to_owned(),
        content: Box::new(rule.into()),
    }
}

/// A precedence rule of a kind.
fn precedence(kind: PrecKind, value: i32, rule: impl Into<Rule>) -> Rule {
    Rule::Prec {
        kind,
        value,
        content: Box::new(rule.into()),
    }
}

/// `prec(value, rule)`.
pub fn prec(value: i32, rule: impl Into<Rule>) -> Rule {
    precedence(PrecKind::Plain, value, rule)
}

/// `prec.left(value, rule)`. `prec.left(rule)` in `grammar.js` is `prec_left(0, rule)`.
pub fn prec_left(value: i32, rule: impl Into<Rule>) -> Rule {
    precedence(PrecKind::Left, value, rule)
}

/// `prec.right(value, rule)`. `prec.right(rule)` in `grammar.js` is `prec_right(0, rule)`.
pub fn prec_right(value: i32, rule: impl Into<Rule>) -> Rule {
    precedence(PrecKind::Right, value, rule)
}

/// `prec.dynamic(value, rule)`.
pub fn prec_dynamic(value: i32, rule: impl Into<Rule>) -> Rule {
    precedence(PrecKind::Dynamic, value, rule)
}

/// One or more of the rule, with a comma between each two.
pub fn comma_sep1(rule: impl Into<Rule>) -> Rule {
    let rule = rule.into();
    Rule::Seq(vec![rule.clone(), repeat(Rule::Seq(vec![",".into(), rule]))])
}

/// Zero or more of the rule, with a comma between each two.
pub fn comma_sep(rule: impl Into<Rule>) -> Rule {
    optional(comma_sep1(rule))
}

/// Owned names from string slices.
pub fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|&name| name.to_owned()).collect()
}

/// Owned conflict sets from string slices.
pub fn conflict_sets(sets: &[&[&str]]) -> Vec<Vec<String>> {
    sets.iter().map(|set| names(set)).collect()
}

/// A tree-sitter grammar.
#[derive(Clone, Debug)]
pub struct Grammar {
    pub name: String,
    /// The name of the grammar that this grammar extends.
    pub inherits: Option<String>,
    /// The rule for keyword extraction.
    pub word: Option<String>,
    /// The rules in the order of `grammar.json`. The first rule is the root.
    pub rules: IndexMap<String, Rule>,
    pub extras: Vec<Rule>,
    pub conflicts: Vec<Vec<String>>,
    pub precedences: Vec<Vec<Rule>>,
    pub externals: Vec<Rule>,
    pub inline: Vec<String>,
    pub supertypes: Vec<String>,
    pub reserved: IndexMap<String, Vec<Rule>>,
    /// True when the external scanner defines `tree_sitter_<name>_external_scanner_set_context`, the
    /// entry point that takes the context of the parser. `grammar.json` then holds the key
    /// `external_scanner_set_context`, and the generator of the fork writes the entry point into the
    /// language struct. A grammar without the key gets a null entry point, so a scanner that does not
    /// define the function still links.
    pub external_scanner_set_context: bool,
}

impl Grammar {
    /// An empty grammar. White space is the only extra, as in `dsl.js`.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            inherits: None,
            word: None,
            rules: IndexMap::new(),
            extras: vec![re(r"\s")],
            conflicts: Vec::new(),
            precedences: Vec::new(),
            externals: Vec::new(),
            inline: Vec::new(),
            supertypes: Vec::new(),
            reserved: IndexMap::new(),
            external_scanner_set_context: false,
        }
    }

    /// A grammar that starts with all the parts of a base grammar.
    pub fn extend_from(base: &Grammar, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            inherits: Some(base.name.clone()),
            ..base.clone()
        }
    }

    /// Set a rule. A new rule goes to the end. A rule that exists keeps its position.
    pub fn define(&mut self, name: &str, rule: impl Into<Rule>) {
        self.rules.insert(name.to_owned(), rule.into());
    }

    /// Replace a rule with the rule that `build` makes from it.
    ///
    /// `build` receives the rule as it is at the time of the call. This is the `original`
    /// argument of a rule function in `grammar.js` when the rule is changed only one time.
    ///
    /// # Panics
    ///
    /// If the grammar has no rule with the name.
    pub fn redefine(&mut self, name: &str, build: impl FnOnce(Rule) -> Rule) {
        let original = self
            .rules
            .get(name)
            .unwrap_or_else(|| panic!("the grammar has no rule `{name}`"))
            .clone();
        self.rules.insert(name.to_owned(), build(original));
    }

    /// The names that the grammar uses but does not define, in the order of first use.
    ///
    /// A rule or an external symbol defines a name. O(n) in the size of the grammar.
    pub fn undefined_names(&self) -> Vec<String> {
        let mut used: Vec<&str> = Vec::new();
        let rules = self
            .rules
            .values()
            .chain(&self.extras)
            .chain(self.precedences.iter().flatten())
            .chain(&self.externals);
        for rule in rules {
            rule.visit_symbols(&mut |name| used.push(name));
        }
        used.extend(self.conflicts.iter().flatten().map(String::as_str));
        used.extend(self.inline.iter().map(String::as_str));
        used.extend(self.supertypes.iter().map(String::as_str));
        used.extend(self.word.as_deref());
        let mut defined: HashSet<&str> = self.rules.keys().map(String::as_str).collect();
        for external in &self.externals {
            if let Rule::Symbol(name) = external {
                defined.insert(name);
            }
        }
        let mut reported = HashSet::new();
        used.into_iter()
            .filter(|name| !defined.contains(name) && reported.insert(*name))
            .map(str::to_owned)
            .collect()
    }

    /// The `grammar.json` value of the grammar, with the key order of `dsl.js`.
    pub fn to_json(&self) -> Value {
        let strings = |list: &[String]| list.iter().map(|name| Value::from(name.as_str())).collect::<Value>();
        let rules = |list: &[Rule]| list.iter().map(Rule::to_json).collect::<Value>();
        let mut map = Map::new();
        map.insert("$schema".into(), SCHEMA.into());
        map.insert("name".into(), self.name.as_str().into());
        if let Some(inherits) = &self.inherits {
            map.insert("inherits".into(), inherits.as_str().into());
        }
        if let Some(word) = &self.word {
            map.insert("word".into(), word.as_str().into());
        }
        map.insert(
            "rules".into(),
            Value::Object(
                self.rules
                    .iter()
                    .map(|(name, rule)| (name.clone(), rule.to_json()))
                    .collect(),
            ),
        );
        map.insert("extras".into(), rules(&self.extras));
        map.insert(
            "conflicts".into(),
            self.conflicts.iter().map(|set| strings(set)).collect(),
        );
        map.insert(
            "precedences".into(),
            self.precedences.iter().map(|list| rules(list)).collect(),
        );
        map.insert("externals".into(), rules(&self.externals));
        if self.external_scanner_set_context {
            map.insert("external_scanner_set_context".into(), true.into());
        }
        map.insert("inline".into(), strings(&self.inline));
        map.insert("supertypes".into(), strings(&self.supertypes));
        map.insert(
            "reserved".into(),
            Value::Object(
                self.reserved
                    .iter()
                    .map(|(name, words)| (name.clone(), rules(words)))
                    .collect(),
            ),
        );
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compact JSON text of a rule. The text shows the key order.
    fn text(rule: &Rule) -> String {
        serde_json::to_string(&rule.to_json()).expect("a rule serializes to JSON")
    }

    #[test]
    fn optional_is_a_choice_with_blank() {
        assert_eq!(
            text(&optional("x")),
            r#"{"type":"CHOICE","members":[{"type":"STRING","value":"x"},{"type":"BLANK"}]}"#
        );
    }

    #[test]
    fn a_rule_has_the_key_order_of_dsl_js() {
        assert_eq!(
            text(&prec_left(2, field("f", alias(re("[a-z]"), sym("n"))))),
            r#"{"type":"PREC_LEFT","value":2,"content":{"type":"FIELD","name":"f","content":{"type":"ALIAS","content":{"type":"PATTERN","value":"[a-z]"},"named":true,"value":"n"}}}"#
        );
    }

    #[test]
    fn a_reserved_rule_has_the_key_order_of_dsl_js() {
        assert_eq!(
            text(&reserved("type_name", sym("identifier"))),
            r#"{"type":"RESERVED","content":{"type":"SYMBOL","name":"identifier"},"context_name":"type_name"}"#
        );
    }

    #[test]
    fn an_alias_to_a_symbol_is_named_and_an_alias_to_a_string_is_anonymous() {
        assert!(matches!(alias(sym("a"), sym("b")), Rule::Alias { named: true, .. }));
        assert!(matches!(alias(sym("a"), "b"), Rule::Alias { named: false, .. }));
    }

    #[test]
    fn a_redefined_rule_keeps_its_position_and_a_new_rule_goes_to_the_end() {
        let mut g = Grammar::new("t");
        g.define("a", "1");
        g.define("b", "2");
        g.redefine("a", |original| choice![original, "3"]);
        g.define("c", "4");
        assert_eq!(g.rules.keys().collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(g.rules["a"], choice!["1", "3"]);
    }

    #[test]
    fn a_name_is_undefined_when_no_rule_and_no_external_has_it() {
        let mut g = Grammar::new("t");
        g.define("a", seq![sym("b"), sym("a"), sym("d")]);
        g.inline = names(&["c", "b"]);
        g.externals = vec![sym("d")];
        assert_eq!(g.undefined_names(), ["b", "c"]);
    }
}
