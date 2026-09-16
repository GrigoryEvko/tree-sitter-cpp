use std::{
    cmp,
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    mem::swap,
};

use rustc_hash::{FxHashMap, FxHashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::tables::{ActionListId, ActionListPool};

use super::{
    LANGUAGE_VERSION,
    build_tables::Tables,
    grammars::{LexicalGrammar, SyntaxGrammar, VariableType},
    nfa::CharacterSet,
    node_types::ChildType,
    rules::Alias,
    rules::{AliasMap, Symbol, SymbolType, TokenSet},
    strpool::{StrId, StrPool},
    tables::{
        AdvanceAction, FieldLocation, GotoAction, LexState, LexTable, ParseAction, ParseTable,
    },
};

const SMALL_STATE_THRESHOLD: usize = 64;
// The generator writes only the ABI of the tree-sitter-cpp fork: 32-bit parse table values and state
// ids. With an upstream version number, an upstream runtime reads these tables incorrectly.
pub const ABI_VERSION_MIN: usize = LANGUAGE_VERSION;
pub const ABI_VERSION_MAX: usize = LANGUAGE_VERSION;
const ABI_VERSION_WITH_RESERVED_WORDS: usize = 15;
// The generator wraps a large array of positional initializers at this column. A designator for each
// element costs more text than the element (tree-sitter-cpp fork).
const WRAP_COLUMN: usize = 100;

pub type RenderResult<T> = Result<T, RenderError>;

/// Put a line break in the buffer when the current line is 100 characters or longer.
///
/// `line_start` is the offset of the start of the current line. The function moves it to the new
/// line. The caller writes one array element and then calls this function.
fn wrap_array_line(buffer: &mut String, line_start: &mut usize) {
    if buffer.len() - *line_start >= WRAP_COLUMN {
        buffer.push('\n');
        *line_start = buffer.len();
    }
}

/// Put a line break after the last element of an array, when that line holds one element or more.
fn end_array_line(buffer: &mut String, line_start: usize) {
    if buffer.len() > line_start {
        buffer.push('\n');
    }
}

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum RenderError {
    #[error("Parse table action count {0} exceeds maximum value of {max}", max=u32::MAX)]
    ParseTable(usize),
    #[error(
        "This version of Tree-sitter can only generate parsers with ABI version {ABI_VERSION_MIN} - {ABI_VERSION_MAX}, not {0}"
    )]
    ABI(usize),
}

#[clippy::format_args]
macro_rules! add {
    ($this: tt, $($arg: tt)*) => {{
        $this.buffer.write_fmt(format_args!($($arg)*)).unwrap();
    }}
}

macro_rules! add_whitespace {
    ($this:tt) => {{
        for _ in 0..$this.indent_level {
            write!(&mut $this.buffer, "  ").unwrap();
        }
    }};
}

#[clippy::format_args]
macro_rules! add_line {
    ($this: tt, $($arg: tt)*) => {
        add_whitespace!($this);
        $this.buffer.write_fmt(format_args!($($arg)*)).unwrap();
        $this.buffer += "\n";
    }
}

macro_rules! indent {
    ($this:tt) => {
        $this.indent_level += 1;
    };
}

macro_rules! dedent {
    ($this:tt) => {
        assert_ne!($this.indent_level, 0);
        $this.indent_level -= 1;
    };
}

#[derive(Default)]
struct Generator {
    buffer: String,
    indent_level: usize,
    language_name: String,
    parse_table: ParseTable,
    main_lex_table: LexTable,
    keyword_lex_table: LexTable,
    large_character_sets: Vec<(Option<Symbol>, CharacterSet)>,
    large_character_set_info: Vec<LargeCharacterSetInfo>,
    large_state_count: usize,
    syntax_grammar: SyntaxGrammar,
    lexical_grammar: LexicalGrammar,
    default_aliases: AliasMap,
    symbol_order: FxHashMap<Symbol, usize>,
    symbol_ids: FxHashMap<Symbol, String>,
    alias_ids: FxHashMap<Alias, String>,
    unique_aliases: Vec<Alias>,
    symbol_map: FxHashMap<Symbol, Symbol>,
    reserved_word_sets: Vec<TokenSet>,
    reserved_word_set_ids_by_parse_state: Vec<usize>,
    field_names: Vec<StrId>,
    supertype_symbol_map: BTreeMap<Symbol, Vec<ChildType>>,
    supertype_map: BTreeMap<String, Vec<ChildType>>,
    abi_version: usize,
    metadata: Option<Metadata>,
    str_pool: StrPool,
}

struct LargeCharacterSetInfo {
    constant_name: String,
    is_used: bool,
}

#[derive(Clone, Copy, Default)]
struct Metadata {
    major: u8,
    minor: u8,
    patch: u8,
}

impl Generator {
    fn generate(mut self) -> RenderResult<String> {
        self.init();
        self.add_header();
        self.add_includes();
        self.add_pragmas();
        self.add_stats();
        self.add_symbol_enum();
        self.add_symbol_names_list();
        self.add_unique_symbol_map();
        self.add_symbol_metadata_list();

        if !self.field_names.is_empty() {
            self.add_field_name_enum();
            self.add_field_name_names_list();
            self.add_field_sequences();
        }

        if !self.parse_table.production_infos.is_empty() {
            self.add_alias_sequences();
        }

        self.add_non_terminal_alias_map();
        self.add_primary_state_id_list();

        if self.abi_version >= ABI_VERSION_WITH_RESERVED_WORDS && !self.supertype_map.is_empty() {
            self.add_supertype_map();
        }

        let buffer_offset_before_lex_functions = self.buffer.len();

        let mut main_lex_table = LexTable::default();
        swap(&mut main_lex_table, &mut self.main_lex_table);
        self.add_lex_function("ts_lex", main_lex_table);

        if self.syntax_grammar.word_token.is_some() {
            let mut keyword_lex_table = LexTable::default();
            swap(&mut keyword_lex_table, &mut self.keyword_lex_table);
            self.add_lex_function("ts_lex_keywords", keyword_lex_table);
        }

        // Once the lex functions are generated, and we've determined which large
        // character sets are actually used, we can generate the large character set
        // constants. Insert them into the output buffer before the lex functions.
        let lex_functions = self.buffer[buffer_offset_before_lex_functions..].to_string();
        self.buffer.truncate(buffer_offset_before_lex_functions);
        for ix in 0..self.large_character_sets.len() {
            self.add_character_set(ix);
        }
        self.buffer.push_str(&lex_functions);

        self.add_lex_modes();

        if self.abi_version >= ABI_VERSION_WITH_RESERVED_WORDS && self.reserved_word_sets.len() > 1
        {
            self.add_reserved_word_sets();
        }

        self.add_parse_table()?;

        if !self.syntax_grammar.external_tokens.is_empty() {
            self.add_external_token_enum();
            self.add_external_scanner_symbol_map();
            self.add_external_scanner_states_list();
        }

        self.add_parser_export();

        Ok(self.buffer)
    }

    fn init(&mut self) {
        let mut symbol_identifiers = FxHashSet::default();
        for i in 0..self.parse_table.symbols.len() {
            self.assign_symbol_id(self.parse_table.symbols[i], &mut symbol_identifiers);
        }
        self.symbol_ids.insert(
            Symbol::end_of_nonterminal_extra(),
            self.symbol_ids[&Symbol::end()].clone(),
        );

        self.symbol_map = FxHashMap::default();

        for symbol in &self.parse_table.symbols {
            let mut mapping = symbol;

            // There can be multiple symbols in the grammar that have the same name and kind,
            // due to simple aliases. When that happens, ensure that they map to the same
            // public-facing symbol. If one of the symbols is not aliased, choose that one
            // to be the public-facing symbol. Otherwise, pick the symbol with the lowest
            // numeric value.
            if let Some(alias) = self.default_aliases.get(symbol) {
                let kind = alias.kind();
                for other_symbol in &self.parse_table.symbols {
                    if let Some(other_alias) = self.default_aliases.get(other_symbol) {
                        if other_symbol < mapping && other_alias == alias {
                            mapping = other_symbol;
                        }
                    } else if self.metadata_for_symbol(*other_symbol) == (alias.value, kind) {
                        mapping = other_symbol;
                        break;
                    }
                }
            }
            // Two anonymous tokens with different flags but the same string value
            // should be represented with the same symbol in the public API. Examples:
            // * "<" and token(prec(1, "<"))
            // * "(" and token.immediate("(")
            else if symbol.is_terminal() {
                let metadata = self.metadata_for_symbol(*symbol);
                for other_symbol in &self.parse_table.symbols {
                    let other_metadata = self.metadata_for_symbol(*other_symbol);
                    if other_metadata == metadata {
                        if let Some(mapped) = self.symbol_map.get(other_symbol)
                            && mapped == symbol
                        {
                            break;
                        }
                        mapping = other_symbol;
                        break;
                    }
                }
            }

            self.symbol_map.insert(*symbol, *mapping);
        }

        for production_info in &self.parse_table.production_infos {
            // Build a list of all field names
            for &field_name in production_info.field_map.keys() {
                if let Err(i) = self.field_names.binary_search_by(|&sid| {
                    self.str_pool
                        .resolve(sid)
                        .cmp(self.str_pool.resolve(field_name))
                }) {
                    self.field_names.insert(i, field_name);
                }
            }

            // Generate a mapping from aliases to C identifiers.
            for &alias in production_info.alias_sequence.iter().flatten() {
                // Some aliases match an existing symbol in the grammar.
                let alias_id = if let Some(existing_symbol) = self.symbols_for_alias(alias).first()
                {
                    self.symbol_ids[&self.symbol_map[existing_symbol]].clone()
                }
                // Other aliases don't match any existing symbol, and need their own
                // identifiers.
                else {
                    if let Err(i) = self.unique_aliases.binary_search_by(|candidate| {
                        self.str_pool
                            .resolve(candidate.value)
                            .cmp(self.str_pool.resolve(alias.value))
                            .then_with(|| candidate.is_named.cmp(&alias.is_named))
                    }) {
                        self.unique_aliases.insert(i, alias);
                    }

                    if alias.is_named {
                        format!("alias_sym_{}", self.sanitize_identifier(alias.value))
                    } else {
                        format!("anon_alias_sym_{}", self.sanitize_identifier(alias.value))
                    }
                };

                self.alias_ids.entry(alias).or_insert(alias_id);
            }
        }

        for (ix, (symbol, _)) in self.large_character_sets.iter().enumerate() {
            let count = self.large_character_sets[0..ix]
                .iter()
                .filter(|(sym, _)| sym == symbol)
                .count()
                + 1;
            let constant_name = if let Some(symbol) = symbol {
                format!("{}_character_set_{}", self.symbol_ids[symbol], count)
            } else {
                format!("extras_character_set_{count}")
            };
            self.large_character_set_info.push(LargeCharacterSetInfo {
                constant_name,
                is_used: false,
            });
        }

        // Assign an id to each unique reserved word set
        self.reserved_word_sets.push(TokenSet::new());
        for state in &self.parse_table.states {
            let id = if let Some(ix) = self
                .reserved_word_sets
                .iter()
                .position(|set| *set == state.reserved_words)
            {
                ix
            } else {
                self.reserved_word_sets.push(state.reserved_words.clone());
                self.reserved_word_sets.len() - 1
            };
            self.reserved_word_set_ids_by_parse_state.push(id);
        }

        if self.abi_version >= ABI_VERSION_WITH_RESERVED_WORDS {
            for (supertype, subtypes) in &self.supertype_symbol_map {
                if let Some(supertype) = self.symbol_ids.get(supertype) {
                    self.supertype_map
                        .entry(supertype.clone())
                        .or_insert_with(|| subtypes.clone());
                }
            }

            self.supertype_symbol_map.clear();
        }

        // Determine which states should use the "small state" representation, and which should
        // use the normal array representation.
        let threshold = cmp::min(SMALL_STATE_THRESHOLD, self.parse_table.symbols.len() / 2);
        self.large_state_count = self
            .parse_table
            .states
            .iter()
            .enumerate()
            .take_while(|(i, s)| {
                *i <= 1 || s.terminal_entries.len() + s.nonterminal_entries.len() > threshold
            })
            .count();
    }

    fn add_header(&mut self) {
        add_line!(self, "/* Automatically @generated by tree-sitter */");
        add_line!(self, "");
    }

    fn add_includes(&mut self) {
        add_line!(self, "#include \"tree_sitter/parser.h\"");
        add_line!(self, "");
    }

    fn add_pragmas(&mut self) {
        add_line!(self, "#if defined(__GNUC__) || defined(__clang__)");
        add_line!(
            self,
            "#pragma GCC diagnostic ignored \"-Wmissing-field-initializers\""
        );
        add_line!(self, "#endif");
        add_line!(self, "");

        // Keep the optimizer on for GCC and for clang (tree-sitter-cpp fork).
        //
        // `ADVANCE(state)` does `goto next_state`, which enters `switch (state)` again for each
        // character. With no optimizer the compiler writes that switch as a linear compare chain
        // and not a jump table. `ts_lex_keywords` of this grammar then holds 1983 compares that
        // descend from case 992 to case 1. The runtime enters it at state 0, and state 0 is the
        // last case. Each identifier of each file walks the whole chain before its first character.
        //
        // The measurement of 2026-09-16 used the C++ grammar of this fork, with 779 main lex
        // states, over 329,387 corpus files:
        //
        //   parse cycles         0.545 of the cycles with the pragma
        //   parse instructions   0.426
        //   corpus CPU time      34m44s with the pragma, and 17m00s without it
        //   trees --compare      0 changed hashes over 329,387 files and over 7,646 test files
        //
        // The price is the compile of src/parser.c, as the minimum of three compiles at -O2 with
        // no ccache, on one machine of 2026-09-16:
        //
        //   gcc 16.2.1     2.76 s and 436 MB with the pragma, and 5.30 s and 599 MB without it
        //   clang 22.1.8   2.44 s and 659 MB with the pragma, and 7.88 s and 660 MB without it
        //
        // The machine code is also smaller: 182,470 bytes to 140,230 for gcc, and 169,725 to
        // 134,637 for clang.
        //
        // THE NUMBERS ABOVE BELONG TO A LEX TABLE OF 779 STATES. A larger table gives a longer
        // compile, and nothing here warns. Measure the compile again before you make the table
        // larger.
        //
        // MSVC KEEPS THE PRAGMA, BECAUSE THIS FORK MEASURED NO MSVC. Do not remove that branch
        // without a measurement of your own.
        if self.main_lex_table.states.len() > 300 {
            add_line!(self, "#ifdef _MSC_VER");
            add_line!(self, "#pragma optimize(\"\", off)");
            add_line!(self, "#endif");
            add_line!(self, "");
        }
    }

    fn add_stats(&mut self) {
        let token_count = self
            .parse_table
            .symbols
            .iter()
            .filter(|symbol| {
                if symbol.is_terminal() || symbol.is_eof() {
                    true
                } else if symbol.is_external() {
                    self.syntax_grammar.external_tokens[symbol.index as usize]
                        .corresponding_internal_token
                        .is_none()
                } else {
                    false
                }
            })
            .count();

        add_line!(self, "#define LANGUAGE_VERSION {}", self.abi_version);
        add_line!(
            self,
            "#define STATE_COUNT {}",
            self.parse_table.states.len()
        );
        add_line!(self, "#define LARGE_STATE_COUNT {}", self.large_state_count);

        add_line!(
            self,
            "#define SYMBOL_COUNT {}",
            self.parse_table.symbols.len()
        );
        add_line!(self, "#define ALIAS_COUNT {}", self.unique_aliases.len());
        add_line!(self, "#define TOKEN_COUNT {token_count}");
        add_line!(
            self,
            "#define EXTERNAL_TOKEN_COUNT {}",
            self.syntax_grammar.external_tokens.len()
        );
        add_line!(self, "#define FIELD_COUNT {}", self.field_names.len());
        add_line!(
            self,
            "#define MAX_ALIAS_SEQUENCE_LENGTH {}",
            self.parse_table.max_aliased_production_length
        );
        add_line!(
            self,
            "#define MAX_RESERVED_WORD_SET_SIZE {}",
            self.reserved_word_sets
                .iter()
                .map(TokenSet::len)
                .max()
                .unwrap()
        );

        add_line!(
            self,
            "#define PRODUCTION_ID_COUNT {}",
            self.parse_table.production_infos.len()
        );
        add_line!(self, "#define SUPERTYPE_COUNT {}", self.supertype_map.len());
        add_line!(self, "");
    }

    fn add_symbol_enum(&mut self) {
        add_line!(self, "enum ts_symbol_identifiers {{");
        indent!(self);
        self.symbol_order.insert(Symbol::end(), 0);
        let mut i = 1;
        for symbol in &self.parse_table.symbols {
            if *symbol != Symbol::end() {
                self.symbol_order.insert(*symbol, i);
                add_line!(self, "{} = {i},", self.symbol_ids[symbol]);
                i += 1;
            }
        }
        for alias in &self.unique_aliases {
            add_line!(self, "{} = {i},", self.alias_ids[alias]);
            i += 1;
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_symbol_names_list(&mut self) {
        add_line!(self, "static const char * const ts_symbol_names[] = {{");
        indent!(self);
        for symbol in &self.parse_table.symbols {
            let name = self.sanitize_string(
                self.default_aliases
                    .get(symbol)
                    .map_or_else(|| self.metadata_for_symbol(*symbol).0, |alias| alias.value),
            );
            add_line!(self, "[{}] = \"{name}\",", self.symbol_ids[symbol]);
        }
        for alias in &self.unique_aliases {
            add_line!(
                self,
                "[{}] = \"{}\",",
                self.alias_ids[alias],
                self.sanitize_string(alias.value)
            );
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_unique_symbol_map(&mut self) {
        add_line!(self, "static const TSSymbol ts_symbol_map[] = {{");
        indent!(self);
        for symbol in &self.parse_table.symbols {
            add_line!(
                self,
                "[{}] = {},",
                self.symbol_ids[symbol],
                self.symbol_ids[&self.symbol_map[symbol]],
            );
        }

        for alias in &self.unique_aliases {
            add_line!(
                self,
                "[{}] = {},",
                self.alias_ids[alias],
                self.alias_ids[alias],
            );
        }

        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_field_name_enum(&mut self) {
        add_line!(self, "enum ts_field_identifiers {{");
        indent!(self);
        for (i, &field_name) in self.field_names.iter().enumerate() {
            add_line!(
                self,
                "{} = {},",
                Self::field_id(self.str_pool.resolve(field_name)),
                i + 1
            );
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_field_name_names_list(&mut self) {
        add_line!(self, "static const char * const ts_field_names[] = {{");
        indent!(self);
        add_line!(self, "[0] = NULL,");
        for &field_name in &self.field_names {
            let field_name = self.str_pool.resolve(field_name);
            add_line!(self, "[{}] = \"{field_name}\",", Self::field_id(field_name));
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_symbol_metadata_list(&mut self) {
        add_line!(
            self,
            "static const TSSymbolMetadata ts_symbol_metadata[] = {{"
        );
        indent!(self);
        for symbol in &self.parse_table.symbols {
            add_line!(self, "[{}] = {{", self.symbol_ids[symbol]);
            indent!(self);
            if let Some(Alias { is_named, .. }) = self.default_aliases.get(symbol) {
                add_line!(self, ".visible = true,");
                add_line!(self, ".named = {is_named},");
            } else {
                match self.metadata_for_symbol(*symbol).1 {
                    VariableType::Named => {
                        add_line!(self, ".visible = true,");
                        add_line!(self, ".named = true,");
                    }
                    VariableType::Anonymous => {
                        add_line!(self, ".visible = true,");
                        add_line!(self, ".named = false,");
                    }
                    VariableType::Hidden => {
                        add_line!(self, ".visible = false,");
                        add_line!(self, ".named = true,");
                        if self.syntax_grammar.supertype_symbols.contains(symbol) {
                            add_line!(self, ".supertype = true,");
                        }
                    }
                    VariableType::Auxiliary => {
                        add_line!(self, ".visible = false,");
                        add_line!(self, ".named = false,");
                    }
                }
            }
            dedent!(self);
            add_line!(self, "}},");
        }
        for alias in &self.unique_aliases {
            add_line!(self, "[{}] = {{", self.alias_ids[alias]);
            indent!(self);
            add_line!(self, ".visible = true,");
            add_line!(self, ".named = {},", alias.is_named);
            dedent!(self);
            add_line!(self, "}},");
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_alias_sequences(&mut self) {
        add_line!(
            self,
            "static const TSSymbol ts_alias_sequences[PRODUCTION_ID_COUNT][MAX_ALIAS_SEQUENCE_LENGTH] = {{",
        );
        indent!(self);
        for (i, production_info) in self.parse_table.production_infos.iter().enumerate() {
            if production_info.alias_sequence.is_empty() {
                // Work around MSVC's intolerance of empty array initializers by
                // explicitly zero-initializing the first element.
                if i == 0 {
                    add_line!(self, "[0] = {{0}},");
                }
                continue;
            }

            add_line!(self, "[{i}] = {{");
            indent!(self);
            for (j, alias) in production_info.alias_sequence.iter().enumerate() {
                if let Some(alias) = alias {
                    add_line!(self, "[{j}] = {},", self.alias_ids[alias]);
                }
            }
            dedent!(self);
            add_line!(self, "}},");
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_non_terminal_alias_map(&mut self) {
        let mut alias_ids_by_symbol = FxHashMap::default();
        for i in 0..self.syntax_grammar.variables.len() {
            for prod_id in self.syntax_grammar.variable_prod_ids(i) {
                for step in self.syntax_grammar.production(prod_id).steps {
                    if let Some(alias) = step.alias()
                        && step.symbol().is_non_terminal()
                        && Some(alias) != self.default_aliases.get(&step.symbol()).copied()
                        && self.symbol_ids.contains_key(&step.symbol())
                        && let Some(alias_id) = self.alias_ids.get(&alias)
                    {
                        let alias_ids = alias_ids_by_symbol
                            .entry(step.symbol())
                            .or_insert(Vec::new());
                        if let Err(i) = alias_ids.binary_search(&alias_id) {
                            alias_ids.insert(i, alias_id);
                        }
                    }
                }
            }
        }

        let mut alias_ids_by_symbol = alias_ids_by_symbol.iter().collect::<Vec<_>>();
        alias_ids_by_symbol.sort_unstable_by_key(|e| e.0);

        add_line!(
            self,
            "static const uint16_t ts_non_terminal_alias_map[] = {{"
        );
        indent!(self);
        for (symbol, alias_ids) in alias_ids_by_symbol {
            let symbol_id = &self.symbol_ids[symbol];
            let public_symbol_id = &self.symbol_ids[&self.symbol_map[symbol]];
            add_line!(self, "{symbol_id}, {},", 1 + alias_ids.len());
            indent!(self);
            add_line!(self, "{public_symbol_id},");
            for alias_id in alias_ids {
                add_line!(self, "{alias_id},");
            }
            dedent!(self);
        }
        add_line!(self, "0,");
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    /// Produces a list of the "primary state" for every state in the grammar.
    ///
    /// The "primary state" for a given state is the first encountered state that behaves
    /// identically with respect to query analysis. We derive this by keeping track of the `core_id`
    /// for each state and treating the first state with a given `core_id` as primary.
    fn add_primary_state_id_list(&mut self) {
        add_line!(
            self,
            "static const TSStateId ts_primary_state_ids[STATE_COUNT] = {{"
        );
        // The array is dense and in state order, so the initializers are positional.
        let mut line_start = self.buffer.len();
        let mut first_state_for_each_core_id = FxHashMap::default();
        for (idx, state) in self.parse_table.states.iter().enumerate() {
            let primary_state = *first_state_for_each_core_id
                .entry(state.core_id)
                .or_insert(idx);
            add!(self, "{primary_state},");
            wrap_array_line(&mut self.buffer, &mut line_start);
        }
        end_array_line(&mut self.buffer, line_start);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_field_sequences(&mut self) {
        let mut flat_field_maps = vec![];
        let mut next_flat_field_map_index = 0;
        Self::get_field_map_id(
            Vec::new(),
            &mut flat_field_maps,
            &mut next_flat_field_map_index,
        );

        let mut field_map_ids = Vec::with_capacity(self.parse_table.production_infos.len());
        for production_info in &self.parse_table.production_infos {
            if production_info.field_map.is_empty() {
                field_map_ids.push((0, 0));
            } else {
                let mut flat_field_map = Vec::with_capacity(production_info.field_map.len());
                for (field_name, locations) in &production_info.field_map {
                    for location in locations {
                        flat_field_map.push((*field_name, *location));
                    }
                }
                flat_field_map.sort_by(|(a, _), (b, _)| {
                    self.str_pool.resolve(*a).cmp(self.str_pool.resolve(*b))
                });
                let field_map_len = flat_field_map.len();
                field_map_ids.push((
                    Self::get_field_map_id(
                        flat_field_map,
                        &mut flat_field_maps,
                        &mut next_flat_field_map_index,
                    ),
                    field_map_len,
                ));
            }
        }

        add_line!(
            self,
            "static const TSMapSlice ts_field_map_slices[PRODUCTION_ID_COUNT] = {{",
        );
        indent!(self);
        for (production_id, (row_id, length)) in field_map_ids.into_iter().enumerate() {
            if length > 0 {
                add_line!(
                    self,
                    "[{production_id}] = {{.index = {row_id}, .length = {length}}},",
                );
            }
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");

        add_line!(
            self,
            "static const TSFieldMapEntry ts_field_map_entries[] = {{",
        );
        indent!(self);
        for (row_index, field_pairs) in flat_field_maps.into_iter().skip(1) {
            add_line!(self, "[{row_index}] =");
            indent!(self);
            for (field_name, location) in field_pairs {
                add_whitespace!(self);
                add!(
                    self,
                    "{{{}, {}",
                    Self::field_id(self.str_pool.resolve(field_name)),
                    location.index
                );
                if location.inherited {
                    add!(self, ", .inherited = true");
                }
                add!(self, "}},\n");
            }
            dedent!(self);
        }

        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_supertype_map(&mut self) {
        add_line!(
            self,
            "static const TSSymbol ts_supertype_symbols[SUPERTYPE_COUNT] = {{"
        );
        indent!(self);
        for supertype in self.supertype_map.keys() {
            add_line!(self, "{supertype},");
        }
        dedent!(self);
        add_line!(self, "}};\n");

        add_line!(
            self,
            "static const TSMapSlice ts_supertype_map_slices[] = {{",
        );
        indent!(self);
        let mut row_id = 0;
        let mut supertype_ids = vec![0];
        let mut supertype_string_map = BTreeMap::new();
        for (supertype, subtypes) in &self.supertype_map {
            supertype_string_map.insert(
                supertype,
                subtypes
                    .iter()
                    .flat_map(|s| match s {
                        ChildType::Normal(symbol) => vec![self.symbol_ids.get(symbol).cloned()],
                        ChildType::Aliased(alias) => {
                            self.alias_ids.get(alias).cloned().map_or_else(
                                || {
                                    self.symbols_for_alias(*alias)
                                        .into_iter()
                                        .map(|s| self.symbol_ids.get(&s).cloned())
                                        .collect()
                                },
                                |a| vec![Some(a)],
                            )
                        }
                    })
                    .flatten()
                    .collect::<BTreeSet<String>>(),
            );
        }
        for (supertype, subtypes) in &supertype_string_map {
            let length = subtypes.len();
            add_line!(
                self,
                "[{supertype}] = {{.index = {row_id}, .length = {length}}},",
            );
            row_id += length;
            supertype_ids.push(row_id);
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");

        add_line!(
            self,
            "static const TSSymbol ts_supertype_map_entries[] = {{",
        );
        indent!(self);
        for (i, (_, subtypes)) in supertype_string_map.iter().enumerate() {
            let row_index = supertype_ids[i];
            add_line!(self, "[{row_index}] =");
            indent!(self);
            for subtype in subtypes {
                add_whitespace!(self);
                add!(self, "{subtype},\n");
            }
            dedent!(self);
        }

        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_lex_function(&mut self, name: &str, lex_table: LexTable) {
        // A lex function takes a 16-bit lex state, as `TSLanguage.lex_fn` says (tree-sitter-cpp fork).
        add_line!(
            self,
            "static bool {name}(TSLexer *lexer, uint16_t state) {{",
        );
        indent!(self);

        add_line!(self, "START_LEXER();");
        add_line!(self, "eof = lexer->eof(lexer);");
        add_line!(self, "switch (state) {{");

        indent!(self);
        for (i, state) in lex_table.states.into_iter().enumerate() {
            add_line!(self, "case {i}:");
            indent!(self);
            self.add_lex_state(i, state);
            dedent!(self);
        }

        add_line!(self, "default:");
        indent!(self);
        add_line!(self, "return false;");
        dedent!(self);

        dedent!(self);
        add_line!(self, "}}");

        dedent!(self);
        add_line!(self, "}}");
        add_line!(self, "");
    }

    fn add_lex_state(&mut self, _state_ix: usize, state: LexState) {
        if let Some(accept_action) = state.accept_action {
            add_line!(self, "ACCEPT_TOKEN({});", self.symbol_ids[&accept_action]);
        }

        if let Some(eof_action) = state.eof_action {
            add_line!(self, "if (eof) ADVANCE({});", eof_action.state);
        }

        let mut chars_copy = CharacterSet::empty();
        let mut large_set = CharacterSet::empty();
        let mut ruled_out_chars = CharacterSet::empty();

        // The transitions in a lex state are sorted with the single-character
        // transitions first. If there are many single-character transitions,
        // then implement them using an array of (lookahead character, state)
        // pairs, instead of individual if statements, in order to reduce compile
        // time.
        let mut leading_simple_transition_count = 0;
        let mut leading_simple_transition_range_count = 0;
        for (chars, action) in &state.advance_actions {
            if action.in_main_token
                && chars.ranges().all(|r| {
                    let start = *r.start() as u32;
                    let end = *r.end() as u32;
                    end <= start + 1 && u16::try_from(end).is_ok()
                })
            {
                leading_simple_transition_count += 1;
                leading_simple_transition_range_count += chars.range_count();
            } else {
                break;
            }
        }

        if leading_simple_transition_range_count >= 8 {
            add_line!(self, "ADVANCE_MAP(");
            indent!(self);
            for (chars, action) in &state.advance_actions[0..leading_simple_transition_count] {
                for range in chars.ranges() {
                    add_whitespace!(self);
                    self.add_character(*range.start());
                    add!(self, ", {},\n", action.state);
                    if range.end() > range.start() {
                        add_whitespace!(self);
                        self.add_character(*range.end());
                        add!(self, ", {},\n", action.state);
                    }
                }
                ruled_out_chars = ruled_out_chars.add(chars);
            }
            dedent!(self);
            add_line!(self, ");");
        } else {
            leading_simple_transition_count = 0;
        }

        for (chars, action) in &state.advance_actions[leading_simple_transition_count..] {
            add_whitespace!(self);

            // The lex state's advance actions are represented with disjoint
            // sets of characters. When translating these disjoint sets into a
            // sequence of checks, we don't need to re-check conditions that
            // have already been checked due to previous transitions.
            //
            // Note that this simplification may result in an empty character set.
            // That means that the transition is guaranteed (nothing further needs to
            // be checked), not that this transition is impossible.
            let simplified_chars = chars.simplify_ignoring(&ruled_out_chars);

            // For large character sets, find the best matching character set from
            // a pre-selected list of large character sets, which are based on the
            // state transitions for individual tokens. This transition may not exactly
            // match one of the pre-selected character sets. In that case, determine
            // the additional checks that need to be performed to match this transition.
            let mut best_large_char_set: Option<(usize, CharacterSet, CharacterSet)> = None;
            if simplified_chars.range_count() >= super::build_tables::LARGE_CHARACTER_RANGE_COUNT {
                for (ix, (_, set)) in self.large_character_sets.iter().enumerate() {
                    chars_copy.assign(&simplified_chars);
                    large_set.assign(set);
                    let intersection = chars_copy.remove_intersection(&mut large_set);
                    if !intersection.is_empty() {
                        let additions = chars_copy.simplify_ignoring(&ruled_out_chars);
                        let removals = large_set.simplify_ignoring(&ruled_out_chars);
                        let total_range_count = additions.range_count() + removals.range_count();
                        if total_range_count >= simplified_chars.range_count() {
                            continue;
                        }
                        if let Some((_, best_additions, best_removals)) = &best_large_char_set {
                            let best_range_count =
                                best_additions.range_count() + best_removals.range_count();
                            if best_range_count < total_range_count {
                                continue;
                            }
                        }
                        best_large_char_set = Some((ix, additions, removals));
                    }
                }
            }

            // Add this transition's character set to the set of ruled out characters,
            // which don't need to be checked for subsequent transitions in this state.
            ruled_out_chars = ruled_out_chars.add(chars);

            let mut large_char_set_ix = None;
            let mut asserted_chars = simplified_chars;
            let mut negated_chars = CharacterSet::empty();
            if let Some((char_set_ix, additions, removals)) = best_large_char_set {
                asserted_chars = additions;
                negated_chars = removals;
                large_char_set_ix = Some(char_set_ix);
            }

            let line_break = format!("\n{}", "  ".repeat(self.indent_level + 2));

            let has_positive_condition = large_char_set_ix.is_some() || !asserted_chars.is_empty();
            let has_negative_condition = !negated_chars.is_empty();
            let has_condition = has_positive_condition || has_negative_condition;
            if has_condition {
                add!(self, "if (");
                if has_positive_condition && has_negative_condition {
                    add!(self, "(");
                }
            }

            if let Some(large_char_set_ix) = large_char_set_ix {
                let large_set = &self.large_character_sets[large_char_set_ix].1;

                // If the character set contains the null character, check that we
                // are not at the end of the file.
                let check_eof = large_set.contains('\0');
                if check_eof {
                    add!(self, "(!eof && ");
                }

                let char_set_info = &mut self.large_character_set_info[large_char_set_ix];
                char_set_info.is_used = true;
                // The bitmap of the code points 0 to 127 comes with the ranges. Refer to
                // `set_contains_ascii` in parser.h.inc (tree-sitter-cpp fork).
                add!(
                    self,
                    "set_contains_ascii({}, {}, {}_ascii, lookahead)",
                    char_set_info.constant_name,
                    large_set.range_count(),
                    char_set_info.constant_name,
                );
                if check_eof {
                    add!(self, ")");
                }
            }

            if !asserted_chars.is_empty() {
                if large_char_set_ix.is_some() {
                    add!(self, " ||{line_break}");
                }

                // If the character set contains the max character, then it probably
                // corresponds to a negated character class in a regex, so it will be more
                // concise and readable to express it in terms of negated ranges.
                let is_included = !asserted_chars.contains(char::MAX);
                if !is_included {
                    asserted_chars = asserted_chars.negate().add_char('\0');
                }

                self.add_character_range_conditions(&asserted_chars, is_included, &line_break);
            }

            if has_negative_condition {
                if has_positive_condition {
                    add!(self, ") &&{line_break}");
                }
                self.add_character_range_conditions(&negated_chars, false, &line_break);
            }

            if has_condition {
                add!(self, ") ");
            }

            self.add_advance_action(action);
            add!(self, "\n");
        }

        add_line!(self, "END_STATE();");
    }

    fn add_character_range_conditions(
        &mut self,
        characters: &CharacterSet,
        is_included: bool,
        line_break: &str,
    ) {
        for (i, range) in characters.ranges().enumerate() {
            let start = *range.start();
            let end = *range.end();
            if is_included {
                if i > 0 {
                    add!(self, " ||{line_break}");
                }

                if start == '\0' {
                    add!(self, "(!eof && ");
                    if end == '\0' {
                        add!(self, "lookahead == 0");
                    } else {
                        add!(self, "lookahead <= ");
                    }
                    self.add_character(end);
                    add!(self, ")");
                } else if end == start {
                    add!(self, "lookahead == ");
                    self.add_character(start);
                } else if end as u32 == start as u32 + 1 {
                    add!(self, "lookahead == ");
                    self.add_character(start);
                    add!(self, " ||{line_break}lookahead == ");
                    self.add_character(end);
                } else {
                    add!(self, "(");
                    self.add_character(start);
                    add!(self, " <= lookahead && lookahead <= ");
                    self.add_character(end);
                    add!(self, ")");
                }
            } else {
                if i > 0 {
                    add!(self, " &&{line_break}");
                }
                if end == start {
                    add!(self, "lookahead != ");
                    self.add_character(start);
                } else if end as u32 == start as u32 + 1 {
                    add!(self, "lookahead != ");
                    self.add_character(start);
                    add!(self, " &&{line_break}lookahead != ");
                    self.add_character(end);
                } else if start != '\0' {
                    add!(self, "(lookahead < ");
                    self.add_character(start);
                    add!(self, " || ");
                    self.add_character(end);
                    add!(self, " < lookahead)");
                } else {
                    add!(self, "lookahead > ");
                    self.add_character(end);
                }
            }
        }
    }

    fn add_character_set(&mut self, ix: usize) {
        let characters = self.large_character_sets[ix].1.clone();
        if !self.large_character_set_info[ix].is_used {
            return;
        }
        let constant_name = self.large_character_set_info[ix].constant_name.clone();

        add_line!(self, "static const TSCharacterRange {constant_name}[] = {{");

        indent!(self);
        for (ix, range) in characters.ranges().enumerate() {
            let column = ix % 8;
            if column == 0 {
                if ix > 0 {
                    add!(self, "\n");
                }
                add_whitespace!(self);
            } else {
                add!(self, " ");
            }
            add!(self, "{{");
            self.add_character(*range.start());
            add!(self, ", ");
            self.add_character(*range.end());
            add!(self, "}},");
        }
        add!(self, "\n");
        dedent!(self);
        add_line!(self, "}};");

        // The bitmap of the code points 0 to 127 of the set (tree-sitter-cpp fork). The lexer reads
        // one bit of it in the place of a binary search of the ranges. Refer to `set_contains_ascii`
        // in parser.h.inc.
        let mut ascii = [0u64; 2];
        for code_point in 0u32..128 {
            if characters.contains(char::from_u32(code_point).expect("a code point below 128 is a character")) {
                ascii[(code_point >> 6) as usize] |= 1u64 << (code_point & 63);
            }
        }
        add_line!(self, "static const uint64_t {constant_name}_ascii[2] = {{");
        indent!(self);
        add_line!(self, "0x{:016x}ULL, 0x{:016x}ULL,", ascii[0], ascii[1]);
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_advance_action(&mut self, action: &AdvanceAction) {
        if action.in_main_token {
            add!(self, "ADVANCE({});", action.state);
        } else {
            add!(self, "SKIP({});", action.state);
        }
    }

    fn add_lex_modes(&mut self) {
        let has_reserved_words = self.abi_version >= ABI_VERSION_WITH_RESERVED_WORDS;
        add_line!(
            self,
            "static const {} ts_lex_modes[STATE_COUNT] = {{",
            if has_reserved_words {
                "TSLexerMode"
            } else {
                "TSLexMode"
            }
        );
        // The initializers are positional. The field order of the two structs in parser.h is the lex
        // state, the external lex state, and for TSLexerMode the reserved word set id.
        let mut line_start = self.buffer.len();
        for i in 0..self.parse_table.states.len() {
            // The lex state has 16 bits, and a state id has 32 bits (tree-sitter-cpp fork). The lex
            // state 65535 identifies the end of a non-terminal extra.
            let is_end_of_extra = self.parse_table.states[i].is_end_of_non_terminal_extra();
            let (lex_state, external_lex_state) = if is_end_of_extra {
                (u32::from(u16::MAX), 0)
            } else {
                let state = &self.parse_table.states[i];
                (state.lex_state_id, state.external_lex_state_id)
            };
            if has_reserved_words {
                let reserved_word_set_id = if is_end_of_extra {
                    0
                } else {
                    self.reserved_word_set_ids_by_parse_state[i]
                };
                add!(
                    self,
                    "{{{lex_state},{external_lex_state},{reserved_word_set_id}}},"
                );
            } else {
                add!(self, "{{{lex_state},{external_lex_state}}},");
            }
            wrap_array_line(&mut self.buffer, &mut line_start);
        }
        end_array_line(&mut self.buffer, line_start);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_reserved_word_sets(&mut self) {
        add_line!(
            self,
            "static const TSSymbol ts_reserved_words[{}][MAX_RESERVED_WORD_SET_SIZE] = {{",
            self.reserved_word_sets.len(),
        );
        indent!(self);
        for (id, set) in self.reserved_word_sets.iter().enumerate() {
            if id == 0 {
                continue;
            }
            add_line!(self, "[{id}] = {{");
            indent!(self);
            for token in set.iter() {
                add_line!(self, "{},", self.symbol_ids[&token]);
            }
            dedent!(self);
            add_line!(self, "}},");
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_external_token_enum(&mut self) {
        add_line!(self, "enum ts_external_scanner_symbol_identifiers {{");
        indent!(self);
        for i in 0..self.syntax_grammar.external_tokens.len() {
            add_line!(self, "{} = {i},", self.external_token_id(i));
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_external_scanner_symbol_map(&mut self) {
        add_line!(
            self,
            "static const TSSymbol ts_external_scanner_symbol_map[EXTERNAL_TOKEN_COUNT] = {{"
        );
        indent!(self);
        for i in 0..self.syntax_grammar.external_tokens.len() {
            let token = &self.syntax_grammar.external_tokens[i];
            let id_token = token
                .corresponding_internal_token
                .unwrap_or_else(|| Symbol::external(i));
            add_line!(
                self,
                "[{}] = {},",
                self.external_token_id(i),
                self.symbol_ids[&id_token],
            );
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    fn add_external_scanner_states_list(&mut self) {
        add_line!(
            self,
            "static const bool ts_external_scanner_states[{}][EXTERNAL_TOKEN_COUNT] = {{",
            self.parse_table.external_lex_states.len(),
        );
        indent!(self);
        for i in 0..self.parse_table.external_lex_states.len() {
            if !self.parse_table.external_lex_states[i].is_empty() {
                add_line!(self, "[{i}] = {{");
                indent!(self);
                for token in self.parse_table.external_lex_states[i].iter() {
                    add_line!(
                        self,
                        "[{}] = true,",
                        self.external_token_id(token.index as usize)
                    );
                }
                dedent!(self);
                add_line!(self, "}},");
            }
        }
        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    /// The numeric id of a symbol in `enum ts_symbol_identifiers`.
    ///
    /// The end of a non-terminal extra takes the id of the end of the input. `init` gives the two the
    /// same identifier, and the parse tables keep them in the same cell.
    fn symbol_number(&self, symbol: Symbol) -> u16 {
        if symbol == Symbol::end_of_nonterminal_extra() {
            return 0;
        }
        self.symbol_order[&symbol] as u16
    }

    /// Write one array of numbers, with a line break at approximately 100 columns.
    ///
    /// C has no array of zero elements, so an empty array takes one element with the value 0. Each
    /// offset of such an array is 0, and the runtime reads no element of it.
    fn add_value_array<T: std::fmt::Display>(&mut self, ctype: &str, name: &str, values: &[T]) {
        add_line!(self, "static const {ctype} {name}[{}] = {{", values.len().max(1));
        let mut line_start = self.buffer.len();
        if values.is_empty() {
            add!(self, "0,");
        }
        for value in values {
            add!(self, "{value},");
            wrap_array_line(&mut self.buffer, &mut line_start);
        }
        end_array_line(&mut self.buffer, line_start);
        add_line!(self, "}};");
        add_line!(self, "");
    }

    /// Write the parse tables in the shape layout (tree-sitter-cpp fork).
    ///
    /// A GROUP of a state is a maximal set of symbols that share one table value. The SHAPE of a
    /// state is the ordered list of its symbols together with the group of each symbol, in the order
    /// in which the runtime reads them. Many states share one shape, and a column of a shape is
    /// frequently one value for each state of that shape. The arrays hold each shape one time, and
    /// for each state only the values of the columns that are not constant.
    ///
    /// The order of the symbols of a shape is the order of the iteration of the runtime: ascending
    /// symbol for a large state, and group order for a small state. `ts_language_lookaheads` reads
    /// the symbols in that order, so a change of the order changes a tree.
    ///
    /// O(n) in the entries of the parse tables, plus O(g) in the groups for the constant columns.
    fn add_parse_table(&mut self) -> RenderResult<()> {
        let mut parse_table_entries = FxHashMap::default();
        let mut next_parse_action_list_index = 0u32;

        // Parse action lists zero is for the default value, when a symbol is not valid.
        // `canonicalize` guarantees pool index 0 is the empty list.
        Self::get_parse_action_list_id(
            ActionListId::new(0, false),
            &self.parse_table.action_lists,
            &mut parse_table_entries,
            &mut next_parse_action_list_index,
        );

        let state_count = self.parse_table.states.len();

        // ---- 1. the entries of each state, in the order in which the runtime reads them ----
        // A value has 32 bits: an index in `ts_parse_actions` for a terminal, or the next state for
        // a non-terminal (tree-sitter-cpp fork).
        let mut entry_symbol: Vec<u16> = Vec::new();
        let mut entry_value: Vec<u32> = Vec::new();
        let mut entry_offset: Vec<u32> = Vec::with_capacity(state_count + 1);
        let mut terminal_entries = Vec::new();
        let mut row: Vec<(u16, u32)> = Vec::new();
        let mut symbols_by_value = FxHashMap::<(u32, SymbolType), Vec<Symbol>>::default();

        for (i, state) in self.parse_table.states.iter().enumerate() {
            entry_offset.push(entry_symbol.len() as u32);
            row.clear();

            // The entries come from a hash map, so each order below is an explicit sort.
            terminal_entries.clear();
            terminal_entries.extend(state.terminal_entries.iter());
            terminal_entries.sort_unstable_by_key(|e| self.symbol_order.get(e.0));

            if i < self.large_state_count {
                // A large state was a dense row, and the runtime read it in ascending symbol order.
                for (symbol, action) in &state.nonterminal_entries {
                    let value = match action {
                        GotoAction::Goto(next) => *next,
                        GotoAction::ShiftExtra => i as u32,
                    };
                    row.push((self.symbol_number(*symbol), value));
                }
                for (symbol, id) in &terminal_entries {
                    let entry_id = Self::get_parse_action_list_id(
                        **id,
                        &self.parse_table.action_lists,
                        &mut parse_table_entries,
                        &mut next_parse_action_list_index,
                    );
                    row.push((self.symbol_number(**symbol), entry_id));
                }
                // The dense row of a large state took one cell for each symbol, so a symbol with two
                // entries kept the value that the text wrote last. The sort is stable, and the order
                // of the two entries is the order in which the text wrote them.
                row.sort_by_key(|entry| entry.0);
                row.dedup_by(|later, earlier| {
                    if later.0 == earlier.0 {
                        earlier.1 = later.1;
                        return true;
                    }
                    false
                });
                // A cell of the dense row with the value 0 was a cell with no entry. The large branch
                // of `ts_lookahead_iterator__next` reads a cell and steps to the next symbol while
                // the value is 0, so it gives no symbol for such a cell. The row drops the entry.
                // `GotoAction::ShiftExtra` of the state 0 gives four cells of that kind.
                //
                // The small branch of the same function reads each symbol of each group and tests no
                // value, so it gives a symbol whose group holds the value 0. A small state below
                // keeps such an entry. The two rules come from the two branches of the runtime, and
                // not from the grammar of today.
                row.retain(|entry| entry.1 != 0);
            } else {
                // A small state keeps its symbols in groups, and the runtime reads the groups in the
                // order that this sort gives them. Many lookahead symbols of one state have the same
                // value, and one group holds all of them.
                symbols_by_value.clear();
                for (symbol, id) in &terminal_entries {
                    let entry_id = Self::get_parse_action_list_id(
                        **id,
                        &self.parse_table.action_lists,
                        &mut parse_table_entries,
                        &mut next_parse_action_list_index,
                    );
                    symbols_by_value
                        .entry((entry_id, SymbolType::Terminal))
                        .or_default()
                        .push(**symbol);
                }
                for (symbol, action) in &state.nonterminal_entries {
                    let value = match action {
                        GotoAction::Goto(next) => *next,
                        GotoAction::ShiftExtra => i as u32,
                    };
                    symbols_by_value
                        .entry((value, SymbolType::NonTerminal))
                        .or_default()
                        .push(*symbol);
                }
                let mut values_with_symbols = symbols_by_value.drain().collect::<Vec<_>>();
                values_with_symbols.sort_unstable_by_key(|((value, kind), symbols)| {
                    (symbols.len(), *kind, *value, symbols[0])
                });
                // A small state keeps a symbol with two entries in two groups, and the search of the
                // runtime gives the value of the group that comes first. It also keeps an entry with
                // the value 0, because the runtime gives a symbol for it. Refer to the large branch
                // above.
                for ((value, _), symbols) in &mut values_with_symbols {
                    symbols.sort_unstable();
                    for symbol in symbols.iter() {
                        row.push((self.symbol_number(*symbol), *value));
                    }
                }
            }

            for &(symbol, value) in &row {
                entry_symbol.push(symbol);
                entry_value.push(value);
            }
        }
        entry_offset.push(entry_symbol.len() as u32);

        // ---- 2. the shape of each state ----
        // The key of a shape is the symbols of the state and the group of each of them. The group
        // comes from the first use of a value, in the order of the iteration. The shape ids come
        // from a first-use walk over the states in index order, and never from the order of a hash
        // map, so that two runs of the generator write the same file.
        let mut shape_of_key = FxHashMap::<Vec<u16>, u32>::default();
        let mut shape_symbol_list: Vec<Vec<u16>> = Vec::new();
        let mut shape_group_list: Vec<Vec<u16>> = Vec::new();
        let mut state_shape: Vec<u32> = Vec::with_capacity(state_count);
        let mut state_group_value: Vec<Vec<u32>> = Vec::with_capacity(state_count);
        let mut group_of_value = FxHashMap::<u32, u16>::default();
        let mut key: Vec<u16> = Vec::new();

        for i in 0..state_count {
            let start = entry_offset[i] as usize;
            let end = entry_offset[i + 1] as usize;
            group_of_value.clear();
            let mut values: Vec<u32> = Vec::new();
            key.clear();
            key.extend_from_slice(&entry_symbol[start..end]);
            for &value in &entry_value[start..end] {
                let group = *group_of_value.entry(value).or_insert_with(|| {
                    let group = values.len() as u16;
                    values.push(value);
                    group
                });
                key.push(group);
            }
            let shape = if let Some(&shape) = shape_of_key.get(&key) {
                shape
            } else {
                let shape = shape_symbol_list.len() as u32;
                shape_of_key.insert(key.clone(), shape);
                shape_symbol_list.push(entry_symbol[start..end].to_vec());
                shape_group_list.push(key[end - start..].to_vec());
                shape
            };
            state_shape.push(shape);
            state_group_value.push(values);
        }

        // ---- 3. the constant columns of each shape ----
        // A column of a shape is constant when each state of that shape has one value there. The
        // variable columns take the slots 0 thru nvar-1 in column order, and the constant columns
        // take the slots that follow, also in column order.
        let shape_count = shape_symbol_list.len();
        let mut states_of_shape: Vec<Vec<u32>> = vec![Vec::new(); shape_count];
        for (i, &shape) in state_shape.iter().enumerate() {
            states_of_shape[shape as usize].push(i as u32);
        }
        let mut shape_nvar: Vec<u16> = Vec::with_capacity(shape_count);
        let mut shape_slot_of_group: Vec<Vec<u16>> = Vec::with_capacity(shape_count);
        let mut shape_constant: Vec<Vec<u32>> = Vec::with_capacity(shape_count);
        for states in &states_of_shape {
            let first = states[0] as usize;
            let group_count = state_group_value[first].len();
            let mut is_variable = vec![false; group_count];
            for &state in &states[1..] {
                let values = &state_group_value[state as usize];
                for group in 0..group_count {
                    if values[group] != state_group_value[first][group] {
                        is_variable[group] = true;
                    }
                }
            }
            let mut slot_of_group = vec![0u16; group_count];
            let mut nvar = 0u16;
            for group in 0..group_count {
                if is_variable[group] {
                    slot_of_group[group] = nvar;
                    nvar += 1;
                }
            }
            let mut constants = Vec::new();
            let mut slot = nvar;
            for group in 0..group_count {
                if !is_variable[group] {
                    slot_of_group[group] = slot;
                    slot += 1;
                    constants.push(state_group_value[first][group]);
                }
            }
            shape_nvar.push(nvar);
            shape_slot_of_group.push(slot_of_group);
            shape_constant.push(constants);
        }

        // ---- 4. the nine arrays ----
        let mut shape_symbols: Vec<u16> = Vec::new();
        let mut shape_slots: Vec<u16> = Vec::new();
        let mut shape_offset: Vec<u32> = Vec::with_capacity(shape_count + 1);
        let mut shape_const: Vec<u32> = Vec::new();
        let mut shape_const_offset: Vec<u32> = Vec::with_capacity(shape_count + 1);
        shape_offset.push(0);
        shape_const_offset.push(0);
        for shape in 0..shape_count {
            shape_symbols.extend_from_slice(&shape_symbol_list[shape]);
            for &group in &shape_group_list[shape] {
                shape_slots.push(shape_slot_of_group[shape][group as usize]);
            }
            shape_offset.push(shape_symbols.len() as u32);
            shape_const.extend_from_slice(&shape_constant[shape]);
            shape_const_offset.push(shape_const.len() as u32);
        }
        let mut state_value_offset: Vec<u32> = Vec::with_capacity(state_count);
        let mut state_values: Vec<u32> = Vec::new();
        for i in 0..state_count {
            state_value_offset.push(state_values.len() as u32);
            let shape = state_shape[i] as usize;
            let nvar = shape_nvar[shape] as usize;
            let base = state_values.len();
            state_values.resize(base + nvar, 0);
            for (group, &value) in state_group_value[i].iter().enumerate() {
                let slot = shape_slot_of_group[shape][group] as usize;
                if slot < nvar {
                    state_values[base + slot] = value;
                }
            }
        }

        add_line!(
            self,
            "// The parse tables in the shape layout. `ts_state_shape` gives the shape of a state,"
        );
        add_line!(
            self,
            "// and `ts_shape_offset` gives the range of that shape in `ts_shape_symbols` and in"
        );
        add_line!(
            self,
            "// `ts_shape_slots`. A slot below `ts_shape_nvar` reads `ts_state_values`, and a higher"
        );
        add_line!(
            self,
            "// slot reads `ts_shape_const`. Refer to `ts_language_lookup` in language.h."
        );
        self.add_value_array("TSSymbol", "ts_shape_symbols", &shape_symbols);
        self.add_value_array("uint16_t", "ts_shape_slots", &shape_slots);
        self.add_value_array("uint32_t", "ts_shape_offset", &shape_offset);
        self.add_value_array("uint16_t", "ts_shape_nvar", &shape_nvar);
        self.add_value_array("uint32_t", "ts_shape_const", &shape_const);
        self.add_value_array("uint32_t", "ts_shape_const_offset", &shape_const_offset);
        self.add_value_array("uint32_t", "ts_state_shape", &state_shape);
        self.add_value_array("uint32_t", "ts_state_value_offset", &state_value_offset);
        self.add_value_array("uint32_t", "ts_state_values", &state_values);

        // The parse tables keep an index in `ts_parse_actions` in 32 bits (tree-sitter-cpp fork).
        // `get_parse_action_list_id` stops the count at u32::MAX.
        if next_parse_action_list_index == u32::MAX {
            Err(RenderError::ParseTable(
                next_parse_action_list_index as usize,
            ))?;
        }

        let mut parse_table_entries = parse_table_entries
            .into_iter()
            .map(|(id, i)| (i, id))
            .collect::<Vec<_>>();
        parse_table_entries.sort_by_key(|(index, _)| *index);
        self.add_parse_action_list(parse_table_entries);

        Ok(())
    }

    fn add_parse_action_list(&mut self, parse_table_entries: Vec<(u32, ActionListId)>) {
        // A short macro for each parse action macro of parser.h. The array holds millions of
        // actions, and the short name saves approximately one half of the text.
        add_line!(self, "#define E(c, r) {{.entry = {{.count = c, .reusable = r}}}}");
        add_line!(self, "#define S(s) SHIFT(s)");
        add_line!(self, "#define SR(s) SHIFT_REPEAT(s)");
        add_line!(self, "#define SX() SHIFT_EXTRA()");
        add_line!(self, "#define R(s, c, p, i) REDUCE(s, c, p, i)");
        add_line!(self, "#define RC() RECOVER()");
        add_line!(self, "#define AC() ACCEPT_INPUT()");
        add_line!(self, "");
        add_line!(
            self,
            "static const TSParseActionEntry ts_parse_actions[] = {{"
        );
        // `get_parse_action_list_id` gives the indexes in order, so the initializers are positional.
        let mut line_start = self.buffer.len();
        for (_, id) in parse_table_entries {
            let actions = self.parse_table.action_lists.get(id);
            add!(
                self,
                "E({},{}),",
                actions.len(),
                u8::from(id.reusable()),
            );
            for action in actions {
                match *action {
                    ParseAction::Accept => add!(self, "AC()"),
                    ParseAction::Recover => add!(self, "RC()"),
                    ParseAction::ShiftExtra => add!(self, "SX()"),
                    ParseAction::Shift {
                        state,
                        is_repetition,
                    } => {
                        if is_repetition {
                            add!(self, "SR({state})");
                        } else {
                            add!(self, "S({state})");
                        }
                    }
                    ParseAction::Reduce {
                        symbol,
                        child_count,
                        dynamic_precedence,
                        production_id,
                        ..
                    } => {
                        // The numeric symbol id, because the symbol name costs much more text.
                        add!(
                            self,
                            "R({},{child_count},{dynamic_precedence},{production_id})",
                            self.symbol_order[&symbol]
                        );
                    }
                }
                add!(self, ",");
            }
            wrap_array_line(&mut self.buffer, &mut line_start);
        }
        end_array_line(&mut self.buffer, line_start);
        add_line!(self, "}};");
        add_line!(self, "");
        add_line!(self, "#undef E");
        add_line!(self, "#undef S");
        add_line!(self, "#undef SR");
        add_line!(self, "#undef SX");
        add_line!(self, "#undef R");
        add_line!(self, "#undef RC");
        add_line!(self, "#undef AC");
        add_line!(self, "");
    }

    fn add_parser_export(&mut self) {
        let language_function_name = format!("tree_sitter_{}", self.language_name);
        let external_scanner_name = format!("{language_function_name}_external_scanner");

        add_line!(self, "#ifdef __cplusplus");
        add_line!(self, r#"extern "C" {{"#);
        add_line!(self, "#endif");

        if !self.syntax_grammar.external_tokens.is_empty() {
            add_line!(self, "void *{external_scanner_name}_create(void);");
            add_line!(self, "void {external_scanner_name}_destroy(void *);");
            add_line!(
                self,
                "bool {external_scanner_name}_scan(void *, TSLexer *, const bool *);",
            );
            add_line!(
                self,
                "unsigned {external_scanner_name}_serialize(void *, char *);",
            );
            add_line!(
                self,
                "void {external_scanner_name}_deserialize(void *, const char *, unsigned);",
            );
            add_line!(self, "");
        }

        add_line!(self, "#ifdef TREE_SITTER_HIDE_SYMBOLS");
        add_line!(self, "#define TS_PUBLIC");
        add_line!(self, "#elif defined(_WIN32)");
        add_line!(self, "#define TS_PUBLIC __declspec(dllexport)");
        add_line!(self, "#else");
        add_line!(
            self,
            "#define TS_PUBLIC __attribute__((visibility(\"default\")))"
        );
        add_line!(self, "#endif");
        add_line!(self, "");

        add_line!(
            self,
            "TS_PUBLIC const TSLanguage *{language_function_name}(void) {{",
        );
        indent!(self);
        add_line!(self, "static const TSLanguage language = {{");
        indent!(self);
        add_line!(self, ".abi_version = LANGUAGE_VERSION,");

        // Quantities
        add_line!(self, ".symbol_count = SYMBOL_COUNT,");
        add_line!(self, ".alias_count = ALIAS_COUNT,");
        add_line!(self, ".token_count = TOKEN_COUNT,");
        add_line!(self, ".external_token_count = EXTERNAL_TOKEN_COUNT,");
        add_line!(self, ".state_count = STATE_COUNT,");
        add_line!(self, ".large_state_count = LARGE_STATE_COUNT,");
        add_line!(self, ".production_id_count = PRODUCTION_ID_COUNT,");
        if self.abi_version >= ABI_VERSION_WITH_RESERVED_WORDS {
            add_line!(self, ".supertype_count = SUPERTYPE_COUNT,");
        }
        add_line!(self, ".field_count = FIELD_COUNT,");
        add_line!(
            self,
            ".max_alias_sequence_length = MAX_ALIAS_SEQUENCE_LENGTH,"
        );

        // Parse tables. `parse_table`, `small_parse_table` and `small_parse_table_map` stay null,
        // because the shape layout replaces them (tree-sitter-cpp fork).
        add_line!(self, ".parse_actions = ts_parse_actions,");

        // Metadata
        add_line!(self, ".symbol_names = ts_symbol_names,");
        if !self.field_names.is_empty() {
            add_line!(self, ".field_names = ts_field_names,");
            add_line!(self, ".field_map_slices = ts_field_map_slices,");
            add_line!(self, ".field_map_entries = ts_field_map_entries,");
        }
        if !self.supertype_map.is_empty() && self.abi_version >= ABI_VERSION_WITH_RESERVED_WORDS {
            add_line!(self, ".supertype_map_slices = ts_supertype_map_slices,");
            add_line!(self, ".supertype_map_entries = ts_supertype_map_entries,");
            add_line!(self, ".supertype_symbols = ts_supertype_symbols,");
        }
        add_line!(self, ".symbol_metadata = ts_symbol_metadata,");
        add_line!(self, ".public_symbol_map = ts_symbol_map,");
        add_line!(self, ".alias_map = ts_non_terminal_alias_map,");
        if !self.parse_table.production_infos.is_empty() {
            add_line!(self, ".alias_sequences = &ts_alias_sequences[0][0],");
        }

        // Lexing
        add_line!(self, ".lex_modes = (const void*)ts_lex_modes,");
        add_line!(self, ".lex_fn = ts_lex,");
        if let Some(keyword_capture_token) = self.syntax_grammar.word_token {
            add_line!(self, ".keyword_lex_fn = ts_lex_keywords,");
            add_line!(
                self,
                ".keyword_capture_token = {},",
                self.symbol_ids[&keyword_capture_token]
            );
        }

        if !self.syntax_grammar.external_tokens.is_empty() {
            add_line!(self, ".external_scanner = {{");
            indent!(self);
            add_line!(self, "&ts_external_scanner_states[0][0],");
            add_line!(self, "ts_external_scanner_symbol_map,");
            add_line!(self, "{external_scanner_name}_create,");
            add_line!(self, "{external_scanner_name}_destroy,");
            add_line!(self, "{external_scanner_name}_scan,");
            add_line!(self, "{external_scanner_name}_serialize,");
            add_line!(self, "{external_scanner_name}_deserialize,");
            dedent!(self);
            add_line!(self, "}},");
        }

        add_line!(self, ".primary_state_ids = ts_primary_state_ids,");

        if self.abi_version >= ABI_VERSION_WITH_RESERVED_WORDS {
            add_line!(self, ".name = \"{}\",", self.language_name);

            if self.reserved_word_sets.len() > 1 {
                add_line!(self, ".reserved_words = &ts_reserved_words[0][0],");
            }

            add_line!(
                self,
                ".max_reserved_word_set_size = {},",
                self.reserved_word_sets
                    .iter()
                    .map(TokenSet::len)
                    .max()
                    .unwrap()
            );

            let metadata = self.metadata.unwrap_or_default();

            add_line!(self, ".metadata = {{");
            indent!(self);
            add_line!(self, ".major_version = {},", metadata.major);
            add_line!(self, ".minor_version = {},", metadata.minor);
            add_line!(self, ".patch_version = {},", metadata.patch);
            dedent!(self);
            add_line!(self, "}},");
        }

        // The parse tables in the shape layout, the last fields of TSLanguage (tree-sitter-cpp fork).
        add_line!(self, ".shape_symbols = ts_shape_symbols,");
        add_line!(self, ".shape_slots = ts_shape_slots,");
        add_line!(self, ".shape_offset = ts_shape_offset,");
        add_line!(self, ".shape_nvar = ts_shape_nvar,");
        add_line!(self, ".shape_const = ts_shape_const,");
        add_line!(self, ".shape_const_offset = ts_shape_const_offset,");
        add_line!(self, ".state_shape = ts_state_shape,");
        add_line!(self, ".state_value_offset = ts_state_value_offset,");
        add_line!(self, ".state_values = ts_state_values,");

        dedent!(self);
        add_line!(self, "}};");
        add_line!(self, "return &language;");
        dedent!(self);
        add_line!(self, "}}");
        add_line!(self, "#ifdef __cplusplus");
        add_line!(self, "}}");
        add_line!(self, "#endif");
    }

    fn get_parse_action_list_id(
        id: ActionListId,
        pool: &ActionListPool,
        parse_action_list_offsets: &mut FxHashMap<ActionListId, u32>,
        next_parse_action_list_index: &mut u32,
    ) -> u32 {
        if let Some(&index) = parse_action_list_offsets.get(&id) {
            index
        } else {
            let result = *next_parse_action_list_index;
            parse_action_list_offsets.insert(id, result);
            // The count stops at u32::MAX, and `add_parse_table` then rejects the tables
            // (tree-sitter-cpp fork).
            *next_parse_action_list_index =
                next_parse_action_list_index.saturating_add(1 + pool.get(id).len() as u32);
            result
        }
    }

    fn get_field_map_id(
        flat_field_map: Vec<(StrId, FieldLocation)>,
        flat_field_maps: &mut Vec<(usize, Vec<(StrId, FieldLocation)>)>,
        next_flat_field_map_index: &mut usize,
    ) -> usize {
        if let Some((index, _)) = flat_field_maps.iter().find(|(_, e)| *e == *flat_field_map) {
            return *index;
        }

        let result = *next_flat_field_map_index;
        *next_flat_field_map_index += flat_field_map.len();
        flat_field_maps.push((result, flat_field_map));
        result
    }

    fn external_token_id(&self, token_idx: usize) -> String {
        let token = &self.syntax_grammar.external_tokens[token_idx];
        format!("ts_external_token_{}", self.sanitize_identifier(token.name))
    }

    fn assign_symbol_id(&mut self, symbol: Symbol, used_identifiers: &mut FxHashSet<String>) {
        let mut id;
        if symbol == Symbol::end() {
            id = "ts_builtin_sym_end".to_string();
        } else {
            let (name, kind) = self.metadata_for_symbol(symbol);
            id = match kind {
                VariableType::Auxiliary => format!("aux_sym_{}", self.sanitize_identifier(name)),
                VariableType::Anonymous => format!("anon_sym_{}", self.sanitize_identifier(name)),
                VariableType::Hidden | VariableType::Named => {
                    format!("sym_{}", self.sanitize_identifier(name))
                }
            };

            let mut suffix_number = 1;
            let mut suffix = String::new();
            while used_identifiers.contains(&id) {
                id.drain(id.len() - suffix.len()..);
                suffix_number += 1;
                suffix = suffix_number.to_string();
                id += &suffix;
            }
        }

        used_identifiers.insert(id.clone());
        self.symbol_ids.insert(symbol, id);
    }

    fn field_id(field_name: &str) -> String {
        format!("field_{field_name}")
    }

    fn metadata_for_symbol(&self, symbol: Symbol) -> (StrId, VariableType) {
        let symbol_index = symbol.index as usize;
        match symbol.kind {
            SymbolType::End | SymbolType::EndOfNonTerminalExtra => {
                (StrPool::END_NAME_ID, VariableType::Hidden)
            }
            SymbolType::NonTerminal => {
                let variable = &self.syntax_grammar.variables[symbol_index];
                (variable.name, variable.kind)
            }
            SymbolType::Terminal => {
                let variable = &self.lexical_grammar.variables[symbol_index];
                (variable.name, variable.kind)
            }
            SymbolType::External => {
                let token = &self.syntax_grammar.external_tokens[symbol_index];
                (token.name, token.kind)
            }
        }
    }

    fn symbols_for_alias(&self, alias: Alias) -> Vec<Symbol> {
        self.parse_table
            .symbols
            .iter()
            .copied()
            .filter(move |symbol| {
                self.default_aliases.get(symbol).map_or_else(
                    || {
                        let (name, kind) = self.metadata_for_symbol(*symbol);
                        name == alias.value && kind == alias.kind()
                    },
                    |&default_alias| default_alias == alias,
                )
            })
            .collect()
    }

    fn sanitize_identifier(&self, name: StrId) -> String {
        let name = self.str_pool.resolve(name);
        let mut result = String::with_capacity(name.len());
        for c in name.chars() {
            if c.is_ascii_alphanumeric() || c == '_' {
                result.push(c);
            } else {
                'special_chars: {
                    let replacement = match c {
                        ' ' if name.len() == 1 => "SPACE",
                        '~' => "TILDE",
                        '`' => "BQUOTE",
                        '!' => "BANG",
                        '@' => "AT",
                        '#' => "POUND",
                        '$' => "DOLLAR",
                        '%' => "PERCENT",
                        '^' => "CARET",
                        '&' => "AMP",
                        '*' => "STAR",
                        '(' => "LPAREN",
                        ')' => "RPAREN",
                        '-' => "DASH",
                        '+' => "PLUS",
                        '=' => "EQ",
                        '{' => "LBRACE",
                        '}' => "RBRACE",
                        '[' => "LBRACK",
                        ']' => "RBRACK",
                        '\\' => "BSLASH",
                        '|' => "PIPE",
                        ':' => "COLON",
                        ';' => "SEMI",
                        '"' => "DQUOTE",
                        '\'' => "SQUOTE",
                        '<' => "LT",
                        '>' => "GT",
                        ',' => "COMMA",
                        '.' => "DOT",
                        '?' => "QMARK",
                        '/' => "SLASH",
                        '\n' => "LF",
                        '\r' => "CR",
                        '\t' => "TAB",
                        '\0' => "NULL",
                        '\u{0001}' => "SOH",
                        '\u{0002}' => "STX",
                        '\u{0003}' => "ETX",
                        '\u{0004}' => "EOT",
                        '\u{0005}' => "ENQ",
                        '\u{0006}' => "ACK",
                        '\u{0007}' => "BEL",
                        '\u{0008}' => "BS",
                        '\u{000b}' => "VTAB",
                        '\u{000c}' => "FF",
                        '\u{000e}' => "SO",
                        '\u{000f}' => "SI",
                        '\u{0010}' => "DLE",
                        '\u{0011}' => "DC1",
                        '\u{0012}' => "DC2",
                        '\u{0013}' => "DC3",
                        '\u{0014}' => "DC4",
                        '\u{0015}' => "NAK",
                        '\u{0016}' => "SYN",
                        '\u{0017}' => "ETB",
                        '\u{0018}' => "CAN",
                        '\u{0019}' => "EM",
                        '\u{001a}' => "SUB",
                        '\u{001b}' => "ESC",
                        '\u{001c}' => "FS",
                        '\u{001d}' => "GS",
                        '\u{001e}' => "RS",
                        '\u{001f}' => "US",
                        '\u{007F}' => "DEL",
                        '\u{FEFF}' => "BOM",
                        '\u{0080}'..='\u{FFFF}' => {
                            write!(result, "u{:04x}", c as u32).unwrap();
                            break 'special_chars;
                        }
                        '\u{10000}'..='\u{10FFFF}' => {
                            write!(result, "U{:08x}", c as u32).unwrap();
                            break 'special_chars;
                        }
                        '0'..='9' | 'a'..='z' | 'A'..='Z' | '_' => unreachable!(),
                        ' ' => break 'special_chars,
                    };
                    if !result.is_empty() && !result.ends_with('_') {
                        result.push('_');
                    }
                    result += replacement;
                }
            }
        }
        result
    }

    fn sanitize_string(&self, name: StrId) -> String {
        let name = self.str_pool.resolve(name);
        let mut result = String::with_capacity(name.len());
        for c in name.chars() {
            match c {
                '\"' => result += "\\\"",
                '?' => result += "\\?",
                '\\' => result += "\\\\",
                '\u{0007}' => result += "\\a",
                '\u{0008}' => result += "\\b",
                '\u{000b}' => result += "\\v",
                '\u{000c}' => result += "\\f",
                '\n' => result += "\\n",
                '\r' => result += "\\r",
                '\t' => result += "\\t",
                '\0' => result += "\\0",
                '\u{0001}'..='\u{001f}' => write!(result, "\\x{:02x}", c as u32).unwrap(),
                '\u{007F}'..='\u{FFFF}' => write!(result, "\\u{:04x}", c as u32).unwrap(),
                '\u{10000}'..='\u{10FFFF}' => write!(result, "\\U{:08x}", c as u32).unwrap(),
                _ => result.push(c),
            }
        }
        result
    }

    fn add_character(&mut self, c: char) {
        match c {
            '\'' => add!(self, "'\\''"),
            '\\' => add!(self, "'\\\\'"),
            '\u{000c}' => add!(self, "'\\f'"),
            '\n' => add!(self, "'\\n'"),
            '\t' => add!(self, "'\\t'"),
            '\r' => add!(self, "'\\r'"),
            _ => {
                if c == '\0' {
                    add!(self, "0");
                } else if c == ' ' || c.is_ascii_graphic() {
                    add!(self, "'{c}'");
                } else {
                    add!(self, "0x{:02x}", c as u32);
                }
            }
        }
    }
}

/// Returns a String of C code for the given components of a parser.
///
/// # Arguments
///
/// * `name` - A string slice containing the name of the language
/// * `parse_table` - The generated parse table for the language
/// * `main_lex_table` - The generated lexing table for the language
/// * `keyword_lex_table` - The generated keyword lexing table for the language
/// * `keyword_capture_token` - A symbol indicating which token is used for keyword capture, if any.
/// * `syntax_grammar` - The syntax grammar extracted from the language's grammar
/// * `lexical_grammar` - The lexical grammar extracted from the language's grammar
/// * `default_aliases` - A map describing the global rename rules that should apply. the keys are
///   symbols that are *always* aliased in the same way, and the values are the aliases that are
///   applied to those symbols.
/// * `str_pool` - A backing pool for `StrId`-identified strings within `syntax_grammar`,
///   `lexical_grammar`, and `default_aliases`.
/// * `abi_version` - The language ABI version that should be generated. Usually you want
///   Tree-sitter's current version, but right after making an ABI change, it may be useful to
///   generate code with the previous ABI.
#[expect(
    clippy::too_many_arguments,
    reason = "all parameters are required for code generation"
)]
pub fn render_c_code(
    name: StrId,
    tables: Tables,
    syntax_grammar: SyntaxGrammar,
    lexical_grammar: LexicalGrammar,
    default_aliases: AliasMap,
    str_pool: StrPool,
    abi_version: usize,
    semantic_version: Option<(u8, u8, u8)>,
    supertype_symbol_map: BTreeMap<Symbol, Vec<ChildType>>,
) -> RenderResult<String> {
    if !(ABI_VERSION_MIN..=ABI_VERSION_MAX).contains(&abi_version) {
        Err(RenderError::ABI(abi_version))?;
    }

    Generator {
        language_name: str_pool.resolve(name).to_string(),
        parse_table: tables.parse_table,
        main_lex_table: tables.main_lex_table,
        keyword_lex_table: tables.keyword_lex_table,
        large_character_sets: tables.large_character_sets,
        large_character_set_info: Vec::new(),
        syntax_grammar,
        lexical_grammar,
        default_aliases,
        abi_version,
        metadata: semantic_version.map(|(major, minor, patch)| Metadata {
            major,
            minor,
            patch,
        }),
        supertype_symbol_map,
        str_pool,
        ..Default::default()
    }
    .generate()
}
