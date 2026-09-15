//! Measure the use of each fixed-width limit of the generated parser.
//!
//! The generator writes the tables of the parser into `src/parser.c`, and the runtime reads them with the
//! types of `src/tree_sitter/parser.h`. Each type holds a maximum value, and parser.h gives each limit next
//! to its type. `cargo xtask limits` prints the use of each limit, and a test fails when the parser uses
//! more than 90% of a limit. The limits come from vendor/tree-sitter-generate and vendor/tree-sitter, and a
//! change of a limit changes the two copies together.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// The maximum use of a limit that the test accepts, in percent.
const MAXIMUM_PERCENT: u64 = 90;

/// The use of one limit in the parser.
#[derive(Debug)]
pub struct Limit {
    /// The name of the limit and the type that holds it.
    pub name: &'static str,
    /// The value that the parser uses.
    pub used: u64,
    /// The maximum value that the type holds.
    pub maximum: u64,
}

impl Limit {
    /// True when the parser uses more than `MAXIMUM_PERCENT` of the limit.
    fn is_near(&self) -> bool {
        self.used * 100 > self.maximum * MAXIMUM_PERCENT
    }

    /// One line with the use in percent, the use, the maximum, and the name.
    fn describe(&self) -> String {
        let percent = 100.0 * self.used as f64 / self.maximum as f64;
        format!("{percent:>7.3}%  {:>13}  of {:>13}  {}", self.used, self.maximum, self.name)
    }
}

/// The part of `src/parser.c` that holds a line.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Other,
    ParseActions,
    ShapeSlots,
    ShapeNvar,
    ShapeOffset,
    ShapeConstOffset,
    StateShape,
    StateValueOffset,
    FieldMapSlices,
    FieldMapEntries,
    SupertypeMapSlices,
    Lex,
    KeywordLex,
}

/// The values that `measure_text` reads from the parser.
#[derive(Default)]
struct Counts {
    state_count: u64,
    symbol_count: u64,
    alias_count: u64,
    field_count: u64,
    production_id_count: u64,
    max_alias_sequence_length: u64,
    max_reserved_word_set_size: u64,
    action_slots: u64,
    actions_in_list: u64,
    child_count: u64,
    precedence_max: i64,
    precedence_min: i64,
    field_map_start: u64,
    field_map_length: u64,
    field_child_index: u64,
    supertype_map_start: u64,
    /// The next free slot of `ts_parse_actions`. The array is dense, and each entry takes its header
    /// slot and one slot for each of its actions.
    parse_action_slot: u64,
    shape_slot: u64,
    shape_nvar: u64,
    shape_symbol_start: u64,
    shape_const_start: u64,
    shape_id: u64,
    state_value_start: u64,
    lex_states: u64,
    keyword_lex_states: u64,
    external_lex_states: u64,
    reserved_word_sets: u64,
}

/// The number at the start of `text`, before the first character that is not a digit.
fn leading_number(text: &str) -> Option<u64> {
    let end = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
    text[..end].parse().ok()
}

/// The number immediately after the first `marker` in `line`.
fn number_after(line: &str, marker: &str) -> Option<u64> {
    line.find(marker).and_then(|at| leading_number(&line[at + marker.len()..]))
}

/// Read the limits of a parser from the text of its `parser.c`.
///
/// The function reads the defines, the parse actions, the shape arrays of the parse tables, the field
/// and supertype maps, the lex functions, and the dimensions of the reserved words and of the external
/// scanner states, in the format of `render.rs`. It skips the symbols and the values of the shapes.
/// O(n) in the size of the text.
///
/// # Errors
///
/// If the text has no state count, no parse action, or no lex state. The format of the file is then
/// different from the format of the generator.
fn measure_text(text: &str) -> Result<Vec<Limit>, String> {
    let mut counts = Counts::default();
    let mut section = Section::Other;
    for line in text.lines() {
        if let Some(define) = line.strip_prefix("#define ") {
            let (name, value) = define.split_once(' ').unwrap_or((define, ""));
            let Ok(value) = value.trim().parse::<u64>() else { continue };
            match name {
                "STATE_COUNT" => counts.state_count = value,
                "SYMBOL_COUNT" => counts.symbol_count = value,
                "ALIAS_COUNT" => counts.alias_count = value,
                "FIELD_COUNT" => counts.field_count = value,
                "PRODUCTION_ID_COUNT" => counts.production_id_count = value,
                "MAX_ALIAS_SEQUENCE_LENGTH" => counts.max_alias_sequence_length = value,
                "MAX_RESERVED_WORD_SET_SIZE" => counts.max_reserved_word_set_size = value,
                _ => {}
            }
            continue;
        }
        if line.starts_with("static ") {
            section = Section::Other;
            if line.contains(" ts_parse_actions[") {
                section = Section::ParseActions;
            } else if line.contains(" ts_shape_slots[") {
                section = Section::ShapeSlots;
            } else if line.contains(" ts_shape_nvar[") {
                section = Section::ShapeNvar;
            } else if line.contains(" ts_shape_offset[") {
                section = Section::ShapeOffset;
            } else if line.contains(" ts_shape_const_offset[") {
                section = Section::ShapeConstOffset;
            } else if line.contains(" ts_state_shape[") {
                section = Section::StateShape;
            } else if line.contains(" ts_state_value_offset[") {
                section = Section::StateValueOffset;
            } else if line.contains(" ts_field_map_slices[") {
                section = Section::FieldMapSlices;
            } else if line.contains(" ts_field_map_entries[") {
                section = Section::FieldMapEntries;
            } else if line.contains(" ts_supertype_map_slices[") {
                section = Section::SupertypeMapSlices;
            } else if line.starts_with("static bool ts_lex(") {
                section = Section::Lex;
            } else if line.starts_with("static bool ts_lex_keywords(") {
                section = Section::KeywordLex;
            } else if let Some(rows) = number_after(line, " ts_reserved_words[") {
                counts.reserved_word_sets = rows;
            } else if let Some(rows) = number_after(line, " ts_external_scanner_states[") {
                counts.external_lex_states = rows;
            }
            continue;
        }
        if line.starts_with("};") {
            section = Section::Other;
            continue;
        }
        match section {
            Section::Other => {}
            Section::ParseActions => {
                // `E(2,1),S(5),R(123,3,-1,7),` with a line break at approximately 100 columns. One
                // entry stays on one line, and a line holds one entry or more.
                for entry in line.split("E(").skip(1) {
                    let Some(count) = leading_number(entry) else { continue };
                    counts.actions_in_list = counts.actions_in_list.max(count);
                    counts.parse_action_slot += 1 + count;
                    counts.action_slots = counts.parse_action_slot;
                    // A reduce action is `R(symbol,children,precedence,production)`. The text `R(` of
                    // the shift repeat action `SR(` has only one argument, and the match then fails.
                    for (at, _) in entry.match_indices("R(") {
                        let arguments: Vec<&str> =
                            entry[at + 2..].split(')').next().unwrap_or("").split(',').collect();
                        if let [_, children, precedence, _] = arguments[..] {
                            counts.child_count = counts.child_count.max(children.parse().unwrap_or(0));
                            let precedence: i64 = precedence.parse().unwrap_or(0);
                            counts.precedence_max = counts.precedence_max.max(precedence);
                            counts.precedence_min = counts.precedence_min.min(precedence);
                        }
                    }
                }
            }
            Section::ShapeSlots
            | Section::ShapeNvar
            | Section::ShapeOffset
            | Section::ShapeConstOffset
            | Section::StateShape
            | Section::StateValueOffset => {
                // A plain list of numbers, with a line break at approximately 100 columns.
                let mut maximum = 0;
                for value in line.split(',') {
                    if let Some(value) = leading_number(value.trim()) {
                        maximum = maximum.max(value);
                    }
                }
                let count = match section {
                    Section::ShapeSlots => &mut counts.shape_slot,
                    Section::ShapeNvar => &mut counts.shape_nvar,
                    Section::ShapeOffset => &mut counts.shape_symbol_start,
                    Section::ShapeConstOffset => &mut counts.shape_const_start,
                    Section::StateShape => &mut counts.shape_id,
                    _ => &mut counts.state_value_start,
                };
                *count = (*count).max(maximum);
            }
            Section::FieldMapSlices | Section::SupertypeMapSlices => {
                let (Some(start), Some(length)) = (number_after(line, ".index = "), number_after(line, ".length = "))
                else {
                    continue;
                };
                if section == Section::FieldMapSlices {
                    counts.field_map_start = counts.field_map_start.max(start);
                    counts.field_map_length = counts.field_map_length.max(length);
                } else {
                    counts.supertype_map_start = counts.supertype_map_start.max(start);
                }
            }
            Section::FieldMapEntries => {
                // `{field_declarator, 1},` or `{field_type, 0, .inherited = true},`
                if let Some(entry) = line.trim_start().strip_prefix("{field_")
                    && let Some(index) = number_after(entry, ", ")
                {
                    counts.field_child_index = counts.field_child_index.max(index);
                }
            }
            Section::Lex | Section::KeywordLex => {
                if let Some(case) = line.trim_start().strip_prefix("case ")
                    && let Some(state) = leading_number(case)
                {
                    let states = if section == Section::Lex {
                        &mut counts.lex_states
                    } else {
                        &mut counts.keyword_lex_states
                    };
                    *states = (*states).max(state + 1);
                }
            }
        }
    }
    if counts.state_count == 0 || counts.action_slots == 0 || counts.lex_states == 0 {
        return Err("the parser file has no STATE_COUNT, no parse action, or no lex state. \
                    Its format is different from the format of the generator."
            .to_owned());
    }
    let uint16 = u64::from(u16::MAX);
    let uint32 = u64::from(u32::MAX);
    Ok(vec![
        Limit {
            name: "parse action slots: the uint32_t values of the parse tables",
            used: counts.action_slots,
            maximum: uint32,
        },
        Limit {
            name: "actions for one state and one token: TSParseActionEntry.entry.count (uint8_t)",
            used: counts.actions_in_list,
            maximum: 255,
        },
        Limit {
            name: "states: TSStateId (uint32_t), with UINT32_MAX for no state",
            used: counts.state_count,
            maximum: uint32,
        },
        Limit {
            name: "shapes of the parse tables: ts_state_shape (uint32_t)",
            used: counts.shape_id + 1,
            maximum: uint32,
        },
        Limit {
            name: "slot of a group in a shape: ts_shape_slots (uint16_t)",
            used: counts.shape_slot,
            maximum: uint16,
        },
        Limit {
            name: "variable slots of a shape: ts_shape_nvar (uint16_t)",
            used: counts.shape_nvar,
            maximum: uint16,
        },
        Limit {
            name: "start of a shape: ts_shape_offset (uint32_t)",
            used: counts.shape_symbol_start,
            maximum: uint32,
        },
        Limit {
            name: "start of the constants of a shape: ts_shape_const_offset (uint32_t)",
            used: counts.shape_const_start,
            maximum: uint32,
        },
        Limit {
            name: "start of the values of a state: ts_state_value_offset (uint32_t)",
            used: counts.state_value_start,
            maximum: uint32,
        },
        Limit {
            name: "symbols and aliases: TSSymbol (uint16_t), with two error symbols",
            used: counts.symbol_count + counts.alias_count,
            maximum: uint16 - 1,
        },
        Limit {
            name: "fields: TSFieldId in 15 bits of the query analysis",
            used: counts.field_count,
            maximum: 32_767,
        },
        Limit {
            name: "start of a field map slice: TSMapSlice.index (uint16_t)",
            used: counts.field_map_start,
            maximum: uint16,
        },
        Limit {
            name: "field map entries of one production: TSMapSlice.length (uint16_t)",
            used: counts.field_map_length,
            maximum: uint16,
        },
        Limit {
            name: "child index of a field: TSFieldMapEntry.child_index (uint8_t)",
            used: counts.field_child_index,
            maximum: 255,
        },
        Limit {
            name: "children of a production: 7 bits of the query analysis",
            used: counts.child_count,
            maximum: 127,
        },
        Limit {
            name: "positive dynamic precedence: TSParseAction.reduce.dynamic_precedence (int16_t)",
            used: counts.precedence_max.unsigned_abs(),
            maximum: 32_767,
        },
        Limit {
            name: "negative dynamic precedence: TSParseAction.reduce.dynamic_precedence (int16_t)",
            used: counts.precedence_min.unsigned_abs(),
            maximum: 32_768,
        },
        Limit {
            name: "production ids: TSParseAction.reduce.production_id (uint16_t)",
            used: counts.production_id_count,
            maximum: uint16 + 1,
        },
        Limit {
            name: "alias sequence length: TSLanguage.max_alias_sequence_length (uint16_t)",
            used: counts.max_alias_sequence_length,
            maximum: uint16,
        },
        Limit {
            name: "start of a supertype map slice: TSMapSlice.index (uint16_t)",
            used: counts.supertype_map_start,
            maximum: uint16,
        },
        Limit {
            name: "lex states: TSLexerMode.lex_state (uint16_t), with UINT16_MAX for a non-terminal extra",
            used: counts.lex_states,
            maximum: uint16,
        },
        Limit {
            name: "keyword lex states: the uint16_t lex states of ADVANCE_MAP",
            used: counts.keyword_lex_states,
            maximum: uint16,
        },
        Limit {
            name: "external lex states: TSLexerMode.external_lex_state (uint16_t)",
            used: counts.external_lex_states,
            maximum: uint16 + 1,
        },
        Limit {
            name: "reserved word sets: TSLexerMode.reserved_word_set_id (uint16_t)",
            used: counts.reserved_word_sets,
            maximum: uint16 + 1,
        },
        Limit {
            name: "words in a reserved word set: TSLanguage.max_reserved_word_set_size (uint16_t)",
            used: counts.max_reserved_word_set_size,
            maximum: uint16,
        },
    ])
}

/// Read the limits of the parser in the file `path`. Refer to `measure_text`.
///
/// # Errors
///
/// If the file cannot be read, or if `measure_text` gives an error.
pub fn measure(path: &Path) -> Result<Vec<Limit>, Box<dyn Error>> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(measure_text(&text).map_err(|e| format!("{}: {e}", path.display()))?)
}

/// Print the use of each limit of `src/parser.c`, or of the parser file in `args`.
///
/// # Errors
///
/// If the parser uses more than `MAXIMUM_PERCENT` of a limit, or if `measure` gives an error.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let path = match args {
        [] => repository.join("src").join("parser.c"),
        [path] => PathBuf::from(path),
        _ => return Err("usage: cargo xtask limits [PARSER_C]".into()),
    };
    let limits = measure(&path)?;
    for limit in &limits {
        println!("{}", limit.describe());
    }
    let near = limits.iter().filter(|limit| limit.is_near()).count();
    if near > 0 {
        return Err(format!(
            "the parser uses more than {MAXIMUM_PERCENT}% of {near} limits. Raise each limit in \
             vendor/tree-sitter-generate and vendor/tree-sitter together."
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small parser file in the format of the generator.
    const SAMPLE: &str = "\
#define STATE_COUNT 10
#define LARGE_STATE_COUNT 4
#define SYMBOL_COUNT 20
#define ALIAS_COUNT 2
#define FIELD_COUNT 3
#define MAX_ALIAS_SEQUENCE_LENGTH 5
#define MAX_RESERVED_WORD_SET_SIZE 7
#define PRODUCTION_ID_COUNT 9
static const TSMapSlice ts_field_map_slices[PRODUCTION_ID_COUNT] = {
  [1] = {.index = 0, .length = 2},
  [2] = {.index = 2, .length = 1},
};
static const TSFieldMapEntry ts_field_map_entries[] = {
  [0] =
    {field_declarator, 1},
    {field_type, 0, .inherited = true},
  [2] =
    {field_value, 4},
};
static const TSMapSlice ts_supertype_map_slices[] = {
  [sym_expression] = {.index = 0, .length = 6},
  [sym_statement] = {.index = 6, .length = 3},
};
static bool ts_lex(TSLexer *lexer, TSStateId state) {
  switch (state) {
    case 0:
      ADVANCE_MAP(
        'a', 1,
      );
    case 1:
      END_STATE();
  }
}
static bool ts_lex_keywords(TSLexer *lexer, TSStateId state) {
  switch (state) {
    case 0:
      END_STATE();
  }
}
static const TSSymbol ts_reserved_words[3][MAX_RESERVED_WORD_SET_SIZE] = {
};
static const TSSymbol ts_shape_symbols[7] = {
1,2,3,
1,4,5,6,
};
static const uint16_t ts_shape_slots[7] = {
0,1,1,
0,0,2,1,
};
static const uint32_t ts_shape_offset[3] = {
0,3,7,
};
static const uint16_t ts_shape_nvar[2] = {
2,1,
};
static const uint32_t ts_shape_const[2] = {
70000,4,
};
static const uint32_t ts_shape_const_offset[3] = {
0,0,2,
};
static const uint32_t ts_state_shape[10] = {
0,0,0,0,1,1,1,1,1,1,
};
static const uint32_t ts_state_value_offset[10] = {
0,2,4,6,8,9,10,11,12,13,
};
static const uint32_t ts_state_values[14] = {
5,6,5,6,5,6,5,6,7,7,7,7,7,7,
};
#define E(c, r) {.entry = {.count = c, .reusable = r}}
#define R(s, c, p, i) REDUCE(s, c, p, i)
static const TSParseActionEntry ts_parse_actions[] = {
E(0,0),E(2,1),S(5),R(12,3,-300,8),
E(1,1),SR(7),
E(1,1),R(13,2,200,1),
};
#undef E
#undef R
static const bool ts_external_scanner_states[6][EXTERNAL_TOKEN_COUNT] = {
};
";

    /// The value of the limit whose name starts with `prefix`.
    fn used(limits: &[Limit], prefix: &str) -> u64 {
        limits
            .iter()
            .find(|limit| limit.name.starts_with(prefix))
            .unwrap_or_else(|| panic!("no limit starts with {prefix}"))
            .used
    }

    /// Each section of the format of the generator gives its value.
    #[test]
    fn the_sample_gives_each_value() {
        let limits = measure_text(SAMPLE).unwrap();
        for (prefix, expected) in [
            ("parse action slots", 8),
            ("actions for one state", 2),
            ("states", 10),
            ("shapes of the parse tables", 2),
            ("slot of a group in a shape", 2),
            ("variable slots of a shape", 2),
            ("start of a shape", 7),
            ("start of the constants of a shape", 2),
            ("start of the values of a state", 13),
            ("symbols and aliases", 22),
            ("fields", 3),
            ("start of a field map slice", 2),
            ("field map entries of one production", 2),
            ("child index of a field", 4),
            ("children of a production", 3),
            ("positive dynamic precedence", 200),
            ("negative dynamic precedence", 300),
            ("production ids", 9),
            ("alias sequence length", 5),
            ("start of a supertype map slice", 6),
            ("lex states", 2),
            ("keyword lex states", 1),
            ("external lex states", 6),
            ("reserved word sets", 3),
            ("words in a reserved word set", 7),
        ] {
            assert_eq!(used(&limits, prefix), expected, "{prefix}");
        }
    }

    /// A text that is not in the format of the generator gives an error, and not a use of zero.
    #[test]
    fn a_text_in_a_different_format_gives_an_error() {
        assert!(measure_text("int main() {}\n").is_err());
    }

    /// A use of 90% is permitted, and a use of more than 90% is near the limit.
    #[test]
    fn a_use_of_more_than_90_percent_is_near_the_limit() {
        let limit = |used| Limit { name: "sample", used, maximum: 1000 };
        assert!(!limit(900).is_near());
        assert!(limit(901).is_near());
    }

    /// The generated parser uses a maximum of 90% of each limit of its tables.
    #[test]
    fn the_parser_uses_a_maximum_of_90_percent_of_each_limit() {
        let path = crate::repository().join("src").join("parser.c");
        let limits = measure(&path).unwrap();
        let near: Vec<String> = limits.iter().filter(|limit| limit.is_near()).map(Limit::describe).collect();
        assert!(
            near.is_empty(),
            "the parser uses more than {MAXIMUM_PERCENT}% of these limits. Raise each limit in \
             vendor/tree-sitter-generate and vendor/tree-sitter together:\n{}",
            near.join("\n")
        );
    }
}
