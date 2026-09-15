#ifndef TREE_SITTER_LANGUAGE_H_
#define TREE_SITTER_LANGUAGE_H_

#ifdef __cplusplus
extern "C" {
#endif

#include "./subtree.h"
#include "./parser.h"

#define ts_builtin_sym_error_repeat (ts_builtin_sym_error - 1)

#define LANGUAGE_VERSION_WITH_RESERVED_WORDS 15
#define LANGUAGE_VERSION_WITH_PRIMARY_STATES 14
// The last upstream ABI version that the runtime reads (tree-sitter-cpp fork).
#define LANGUAGE_VERSION_UPSTREAM_MAX 15
// The first ABI of the tree-sitter-cpp fork: 32-bit values in the parse tables and in
// `primary_state_ids`, and the high 16 bits of a shift state in `TSParseAction.shift.state_high`.
#define LANGUAGE_VERSION_WITH_WIDE_TABLES 1015
// The second ABI of the tree-sitter-cpp fork: the parse tables in the shape layout. Refer to the nine
// shape pointers of TSLanguage in parser.h.
#define LANGUAGE_VERSION_WITH_SHAPE_TABLES 1016

// True when the runtime reads the ABI version: 13 thru 15, or one of the two versions of the
// tree-sitter-cpp fork.
static inline bool ts_language_version_is_supported(uint32_t version) {
  return
    (version >= TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION && version <= LANGUAGE_VERSION_UPSTREAM_MAX) ||
    version == LANGUAGE_VERSION_WITH_WIDE_TABLES ||
    version == LANGUAGE_VERSION_WITH_SHAPE_TABLES;
}

// True when the parse tables of the language are in the shape layout (tree-sitter-cpp fork). The
// runtime must read a shape pointer of TSLanguage only when this function is true, because a language
// of a lower ABI keeps each of them null.
static inline bool ts_language_has_shape_tables(const TSLanguage *self) {
  return self->abi_version >= LANGUAGE_VERSION_WITH_SHAPE_TABLES;
}

// True when the parse tables and the primary state ids of the language have 32-bit values
// (tree-sitter-cpp fork).
static inline bool ts_language_has_wide_tables(const TSLanguage *self) {
  return self->abi_version >= LANGUAGE_VERSION_WITH_WIDE_TABLES;
}

// The value at `index` of a parse table or of the primary state ids of the language, with the width of
// its ABI.
static inline uint32_t ts_language_table_value(const TSLanguage *self, const void *table, uint32_t index) {
  return ts_language_has_wide_tables(self)
    ? ((const uint32_t *)table)[index]
    : ((const uint16_t *)table)[index];
}

// The state after a shift action. ABI 13 thru 15 keep the state in `state_low`, and the bytes of
// `state_high` are padding.
static inline TSStateId ts_language_shift_state(const TSLanguage *self, const TSParseAction *action) {
  if (ts_language_has_wide_tables(self)) {
    return (TSStateId)action->shift.state_low | ((TSStateId)action->shift.state_high << 16);
  }
  return action->shift.state_low;
}

typedef struct {
  const TSParseAction *actions;
  uint32_t action_count;
  bool is_reusable;
} TableEntry;

typedef enum {
  LookaheadFresh,      // no `next()` yet
  LookaheadPositioned, // last `next()` returned true
  LookaheadDone,       // last `next()` returned false
} LookaheadPhase;

// An iterator of the valid symbols of a parse state (tree-sitter-cpp fork).
//
// For a language in the shape layout, `position` is the index of the current symbol in
// `shape_symbols` and `group_end` is the end of the shape. `shape` and `value_offset` then give the
// table value of each symbol.
//
// For a language of a lower ABI, `table` is a parse table of the language and `position` is an index
// in it: the start of the row of the state for a large state, and the index of the current value for
// a small state. Refer to `ts_language_table_value`.
typedef struct {
  const TSLanguage *language;
  const void *table;
  uint32_t position;
  uint32_t group_end;
  uint32_t table_value;
  uint32_t group_count;
  uint32_t shape;
  uint32_t value_offset;
  bool is_small_state;
  LookaheadPhase phase;

  const TSParseAction *actions;
  TSSymbol symbol;
  TSStateId next_state;
  uint16_t action_count;
} LookaheadIterator;

void ts_language_table_entry(const TSLanguage *self, TSStateId state, TSSymbol symbol, TableEntry *result);
TSLexerMode ts_language_lex_mode_for_state(const TSLanguage *self, TSStateId state);
bool ts_language_is_reserved_word(const TSLanguage *self, TSStateId state, TSSymbol symbol);
TSSymbolMetadata ts_language_symbol_metadata(const TSLanguage *self, TSSymbol symbol);
TSSymbol ts_language_public_symbol(const TSLanguage *self, TSSymbol symbol);
const TSLanguage *ts_language_copy_without_callbacks(const TSLanguage *self);
#ifdef __wasm__
uint32_t ts_language_current_context_id(void);
#endif

static inline const TSParseAction *ts_language_actions(
  const TSLanguage *self,
  TSStateId state,
  TSSymbol symbol,
  uint32_t *count
) {
  TableEntry entry;
  ts_language_table_entry(self, state, symbol, &entry);
  *count = entry.action_count;
  return entry.actions;
}

static inline bool ts_language_has_reduce_action(
  const TSLanguage *self,
  TSStateId state,
  TSSymbol symbol
) {
  TableEntry entry;
  ts_language_table_entry(self, state, symbol, &entry);
  return entry.action_count > 0 && entry.actions[0].type == TSParseActionTypeReduce;
}

// Lookup the table value for a given symbol and state.
//
// For non-terminal symbols, the table value represents a successor state.
// For terminal symbols, it represents an index in the actions table.
// For 'large' parse states, this is a direct lookup. For 'small' parse
// states, this requires searching through the symbol groups to find
// the given symbol.
//
// The runtime searches a small parse state with 16-bit or 32-bit values. The macro defines the search
// for one width. The search reads the groups of the state until a group holds the symbol. O(n) in the
// values of the state (tree-sitter-cpp fork).
#define TS_LANGUAGE_DEFINE_SMALL_STATE_LOOKUP(name, value_type)  \
  static inline uint32_t name(const value_type *data, TSSymbol symbol) { \
    uint32_t group_count = *(data++);                            \
    for (unsigned i = 0; i < group_count; i++) {                 \
      uint32_t section_value = *(data++);                        \
      uint32_t symbol_count = *(data++);                         \
      for (unsigned j = 0; j < symbol_count; j++) {              \
        if (*(data++) == symbol) return section_value;           \
      }                                                          \
    }                                                            \
    return 0;                                                    \
  }

TS_LANGUAGE_DEFINE_SMALL_STATE_LOOKUP(ts_language__small_state_lookup_16, uint16_t)
TS_LANGUAGE_DEFINE_SMALL_STATE_LOOKUP(ts_language__small_state_lookup_32, uint32_t)

#undef TS_LANGUAGE_DEFINE_SMALL_STATE_LOOKUP

// The index of `symbol` in `shape_symbols[start..end]`, or UINT32_MAX when the shape has no such
// symbol (tree-sitter-cpp fork). A large state keeps its symbols in ascending order and takes the
// binary search, O(log n). A small state keeps its symbols in group order and takes the linear scan,
// O(n) in the symbols of the state.
static inline uint32_t ts_language__shape_index(
  const TSSymbol *symbols,
  uint32_t start,
  uint32_t end,
  TSSymbol symbol,
  bool is_ascending
) {
  if (is_ascending) {
    uint32_t low = start;
    uint32_t high = end;
    while (low < high) {
      uint32_t middle = low + ((high - low) >> 1);
      if (symbols[middle] < symbol) {
        low = middle + 1;
      } else {
        high = middle;
      }
    }
    return (low < end && symbols[low] == symbol) ? low : UINT32_MAX;
  }
  for (uint32_t index = start; index < end; index++) {
    if (symbols[index] == symbol) return index;
  }
  return UINT32_MAX;
}

// The table value of one slot of a shape (tree-sitter-cpp fork). A slot below the number of variable
// slots reads the values of the state, and a higher slot reads the constants of the shape.
static inline uint32_t ts_language__shape_value(
  const TSLanguage *self,
  uint32_t shape,
  uint32_t value_offset,
  uint32_t slot
) {
  uint32_t variable_count = self->shape_nvar[shape];
  return slot < variable_count
    ? self->state_values[value_offset + slot]
    : self->shape_const[self->shape_const_offset[shape] + slot - variable_count];
}

static inline uint32_t ts_language_lookup(
  const TSLanguage *self,
  TSStateId state,
  TSSymbol symbol
) {
  if (ts_language_has_shape_tables(self)) {
    uint32_t shape = self->state_shape[state];
    uint32_t index = ts_language__shape_index(
      self->shape_symbols,
      self->shape_offset[shape],
      self->shape_offset[shape + 1],
      symbol,
      state < self->large_state_count
    );
    if (index == UINT32_MAX) return 0;
    return ts_language__shape_value(
      self, shape, self->state_value_offset[state], self->shape_slots[index]
    );
  }
  if (state >= self->large_state_count) {
    uint32_t index = self->small_parse_table_map[state - self->large_state_count];
    if (ts_language_has_wide_tables(self)) {
      return ts_language__small_state_lookup_32(&((const uint32_t *)self->small_parse_table)[index], symbol);
    }
    return ts_language__small_state_lookup_16(&((const uint16_t *)self->small_parse_table)[index], symbol);
  } else {
    return ts_language_table_value(self, self->parse_table, state * self->symbol_count + symbol);
  }
}

static inline bool ts_language_has_actions(
  const TSLanguage *self,
  TSStateId state,
  TSSymbol symbol
) {
  return ts_language_lookup(self, state, symbol) != 0;
}

// Iterate over all of the symbols that are valid in the given state.
//
// For 'large' parse states, this just requires iterating through
// all possible symbols and checking the parse table for each one.
// For 'small' parse states, this exploits the structure of the
// table to only visit the valid symbols.
static inline LookaheadIterator ts_language_lookaheads(
  const TSLanguage *self,
  TSStateId state
) {
  bool is_small_state = state >= self->large_state_count;
  const void *table = NULL;
  uint32_t position = 0;
  uint32_t group_end = 0;
  uint32_t group_count = 0;
  uint32_t shape = 0;
  uint32_t value_offset = 0;
  if (ts_language_has_shape_tables(self)) {
    // The symbols of the shape are already in the order of the iteration: ascending symbol for a
    // large state, and group order for a small state (tree-sitter-cpp fork).
    shape = self->state_shape[state];
    value_offset = self->state_value_offset[state];
    position = self->shape_offset[shape];
    group_end = self->shape_offset[shape + 1];
  } else if (is_small_state) {
    table = self->small_parse_table;
    position = self->small_parse_table_map[state - self->large_state_count];
    group_end = position + 1;
    group_count = ts_language_table_value(self, table, position);
  } else {
    // The start of the row of the state. Each step adds the symbol to it.
    table = self->parse_table;
    position = state * self->symbol_count;
  }
  return (LookaheadIterator) {
    .language = self,
    .table = table,
    .position = position,
    .group_end = group_end,
    .group_count = group_count,
    .shape = shape,
    .value_offset = value_offset,
    .is_small_state = is_small_state,
    .phase = LookaheadFresh,
    .symbol = UINT16_MAX,
    .next_state = 0,
  };
}

static inline bool ts_lookahead_iterator__next(LookaheadIterator *self) {
  if (self->phase == LookaheadDone) return false;
  const TSLanguage *language = self->language;

  // The shape of the state holds each valid symbol one time, in the order of the iteration, and the
  // slot of each symbol gives its table value with no search (tree-sitter-cpp fork).
  if (ts_language_has_shape_tables(language)) {
    uint32_t index = self->phase == LookaheadFresh ? self->position : self->position + 1;
    if (index >= self->group_end) {
      self->phase = LookaheadDone;
      return false;
    }
    self->position = index;
    self->symbol = language->shape_symbols[index];
    self->table_value = ts_language__shape_value(
      language, self->shape, self->value_offset, language->shape_slots[index]
    );
  }

  // For small parse states, valid symbols are listed explicitly,
  // grouped by their value. There's no need to look up the actions
  // again until moving to the next group.
  else if (self->is_small_state) {
    self->position++;
    if (self->position == self->group_end) {
      if (self->group_count == 0) {
        self->phase = LookaheadDone;
        return false;
      }
      self->group_count--;
      self->table_value = ts_language_table_value(language, self->table, self->position++);
      uint32_t symbol_count = ts_language_table_value(language, self->table, self->position++);
      self->group_end = self->position + symbol_count;
      self->symbol = (TSSymbol)ts_language_table_value(language, self->table, self->position);
    } else {
      self->symbol = (TSSymbol)ts_language_table_value(language, self->table, self->position);
      self->phase = LookaheadPositioned;
      return true;
    }
  }

  // For large parse states, iterate through every symbol until one
  // is found that has valid actions.
  else {
    // The row of the state starts at `position`, and each value has the width of the ABI of the
    // language (tree-sitter-cpp fork).
    uint32_t row = self->position;
    uint32_t symbol = self->phase == LookaheadFresh ? 0 : (uint32_t)self->symbol + 1;
    uint32_t value = 0;
    while (symbol < language->symbol_count) {
      value = ts_language_table_value(language, self->table, row + symbol);
      if (value) break;
      symbol++;
    }
    if (symbol >= language->symbol_count) {
      self->phase = LookaheadDone;
      return false;
    }
    self->symbol = (TSSymbol)symbol;
    self->table_value = value;
  }

  // Depending on if the symbol is terminal or non-terminal, the table value either
  // represents a list of actions or a successor state.
  if (self->symbol < self->language->token_count) {
    const TSParseActionEntry *entry = &self->language->parse_actions[self->table_value];
    self->action_count = entry->entry.count;
    self->actions = (const TSParseAction *)(entry + 1);
    self->next_state = 0;
  } else {
    self->action_count = 0;
    self->next_state = self->table_value;
  }
  self->phase = LookaheadPositioned;
  return true;
}

// Whether the state is a "primary state". If this returns false, it indicates that there exists
// another state that behaves identically to this one with respect to query analysis.
static inline bool ts_language_state_is_primary(
  const TSLanguage *self,
  TSStateId state
) {
  if (self->abi_version >= LANGUAGE_VERSION_WITH_PRIMARY_STATES) {
    return state == ts_language_table_value(self, self->primary_state_ids, state);
  } else {
    return true;
  }
}

static inline const bool *ts_language_enabled_external_tokens(
  const TSLanguage *self,
  unsigned external_scanner_state
) {
  if (external_scanner_state == 0) {
    return NULL;
  } else {
    return self->external_scanner.states + self->external_token_count * external_scanner_state;
  }
}

static inline const TSSymbol *ts_language_alias_sequence(
  const TSLanguage *self,
  uint32_t production_id
) {
  return production_id ?
    &self->alias_sequences[production_id * self->max_alias_sequence_length] :
    NULL;
}

static inline TSSymbol ts_language_alias_at(
  const TSLanguage *self,
  uint32_t production_id,
  uint32_t child_index
) {
  return production_id ?
    self->alias_sequences[production_id * self->max_alias_sequence_length + child_index] :
    0;
}

static inline void ts_language_field_map(
  const TSLanguage *self,
  uint32_t production_id,
  const TSFieldMapEntry **start,
  const TSFieldMapEntry **end
) {
  if (self->field_count == 0) {
    *start = NULL;
    *end = NULL;
    return;
  }

  TSMapSlice slice = self->field_map_slices[production_id];
  *start = &self->field_map_entries[slice.index];
  *end = &self->field_map_entries[slice.index] + slice.length;
}

static inline void ts_language_aliases_for_symbol(
  const TSLanguage *self,
  TSSymbol original_symbol,
  const TSSymbol **start,
  const TSSymbol **end
) {
  *start = &self->public_symbol_map[original_symbol];
  *end = *start + 1;

  unsigned idx = 0;
  for (;;) {
    TSSymbol symbol = self->alias_map[idx++];
    if (symbol == 0 || symbol > original_symbol) break;
    uint16_t count = self->alias_map[idx++];
    if (symbol == original_symbol) {
      *start = &self->alias_map[idx];
      *end = &self->alias_map[idx + count];
      break;
    }
    idx += count;
  }
}

static inline void ts_language_write_symbol_as_dot_string(
  const TSLanguage *self,
  FILE *f,
  TSSymbol symbol
) {
  const char *name = ts_language_symbol_name(self, symbol);
  for (const char *chr = name; *chr; chr++) {
    switch (*chr) {
      case '"':
      case '\\':
        fputc('\\', f);
        fputc(*chr, f);
        break;
      case '\n':
        fputs("\\n", f);
        break;
      case '\t':
        fputs("\\t", f);
        break;
      default:
        fputc(*chr, f);
        break;
    }
  }
}

#ifdef __cplusplus
}
#endif

#endif  // TREE_SITTER_LANGUAGE_H_
