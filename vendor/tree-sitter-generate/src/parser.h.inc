#ifndef TREE_SITTER_PARSER_H_
#define TREE_SITTER_PARSER_H_

#ifdef __cplusplus
extern "C" {
#endif

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#define ts_builtin_sym_error ((TSSymbol)-1)
#define ts_builtin_sym_end 0
#define TREE_SITTER_SERIALIZATION_BUFFER_SIZE 1024

#ifndef TREE_SITTER_API_H_
// A parse state id. The runtime and the parsers of the tree-sitter-cpp fork keep a state in 32 bits. A
// parser of the fork has a maximum of 4,294,967,295 states, because the runtime uses UINT32_MAX for a
// subtree with no state. A parser of ABI 13 thru 15 keeps a state in 16 bits.
typedef uint32_t TSStateId;
// A symbol id. A parser has a maximum of 65,534 symbols and aliases, because the two largest values
// are the error symbols.
typedef uint16_t TSSymbol;
// A field id. A parser has a maximum of 32,767 fields, because the query analysis keeps a field id in
// 15 bits.
typedef uint16_t TSFieldId;
typedef struct TSLanguage TSLanguage;
typedef struct TSLanguageMetadata {
  uint8_t major_version;
  uint8_t minor_version;
  uint8_t patch_version;
} TSLanguageMetadata;
#endif

typedef struct {
  TSFieldId field_id;
  // The index of the child in the production: a maximum of 255.
  uint8_t child_index;
  bool inherited;
} TSFieldMapEntry;

// Used to index the field and supertype maps.
// The start of a slice has a maximum of 65,535, and a slice has a maximum of 65,535 entries.
typedef struct {
  uint16_t index;
  uint16_t length;
} TSMapSlice;

typedef struct {
  bool visible;
  bool named;
  bool supertype;
} TSSymbolMetadata;

typedef struct TSLexer TSLexer;

struct TSLexer {
  int32_t lookahead;
  TSSymbol result_symbol;
  void (*advance)(TSLexer *, bool);
  void (*mark_end)(TSLexer *);
  uint32_t (*get_column)(TSLexer *);
  bool (*is_at_included_range_start)(const TSLexer *);
  bool (*eof)(const TSLexer *);
  void (*log)(const TSLexer *, const char *, ...);
};

typedef enum {
  TSParseActionTypeShift,
  TSParseActionTypeReduce,
  TSParseActionTypeAccept,
  TSParseActionTypeRecover,
} TSParseActionType;

typedef union {
  // A shift action keeps the low 16 bits of the state at the offset of the 16-bit state of ABI 13 thru
  // 15, and the high 16 bits after the flags, where ABI 13 thru 15 have padding. The flags have the same
  // offsets in each ABI. The runtime reads the state with `ts_language_shift_state` (tree-sitter-cpp
  // fork).
  struct {
    uint8_t type;
    uint16_t state_low;
    bool extra;
    bool repetition;
    uint16_t state_high;
  } shift;
  struct {
    uint8_t type;
    // A production has a maximum of 255 children. The query analysis keeps this count in 7 bits, and
    // a production in a query has a maximum of 127 children.
    uint8_t child_count;
    TSSymbol symbol;
    // The dynamic precedence of a production: -32,768 thru 32,767.
    int16_t dynamic_precedence;
    // A parser has a maximum of 65,536 production ids.
    uint16_t production_id;
  } reduce;
  uint8_t type;
} TSParseAction;

typedef struct {
  uint16_t lex_state;
  uint16_t external_lex_state;
} TSLexMode;

// A parser has a maximum of 65,535 lex states, because the lex state UINT16_MAX identifies the end of
// a non-terminal extra. A parser has a maximum of 65,536 external lex states and 65,536 reserved word
// sets.
typedef struct {
  uint16_t lex_state;
  uint16_t external_lex_state;
  uint16_t reserved_word_set_id;
} TSLexerMode;

typedef union {
  TSParseAction action;
  struct {
    // The actions for one state and one token: a maximum of 255.
    uint8_t count;
    bool reusable;
  } entry;
} TSParseActionEntry;

typedef struct {
  int32_t start;
  int32_t end;
} TSCharacterRange;

struct TSLanguage {
  uint32_t abi_version;
  uint32_t symbol_count;
  uint32_t alias_count;
  uint32_t token_count;
  uint32_t external_token_count;
  uint32_t state_count;
  uint32_t large_state_count;
  uint32_t production_id_count;
  uint32_t field_count;
  uint16_t max_alias_sequence_length;
  // The parse tables of ABI 13 thru 1015. A value has 16 bits in ABI 13 thru 15, and 32 bits in ABI
  // 1015 of the tree-sitter-cpp fork. For a token, the value is the index of its actions in
  // `parse_actions`, and for a non-terminal, the value is the next state. `small_parse_table` also
  // keeps the counts of its groups and symbols. The runtime reads the values with the width of the ABI
  // of the language. A parser of the fork has a maximum of 4,294,967,294 action slots. The index of a
  // large state in `parse_table` is state * symbol_count + symbol, in 32 bits.
  // ABI 1016 keeps these three pointers null and uses the shape layout at the end of this struct.
  const void *parse_table;
  const void *small_parse_table;
  const uint32_t *small_parse_table_map;
  const TSParseActionEntry *parse_actions;
  const char * const *symbol_names;
  const char * const *field_names;
  const TSMapSlice *field_map_slices;
  const TSFieldMapEntry *field_map_entries;
  const TSSymbolMetadata *symbol_metadata;
  const TSSymbol *public_symbol_map;
  const uint16_t *alias_map;
  const TSSymbol *alias_sequences;
  const TSLexerMode *lex_modes;
  // A lex function takes a 16-bit lex state in each ABI (tree-sitter-cpp fork).
  bool (*lex_fn)(TSLexer *, uint16_t);
  bool (*keyword_lex_fn)(TSLexer *, uint16_t);
  TSSymbol keyword_capture_token;
  struct {
    const bool *states;
    const TSSymbol *symbol_map;
    void *(*create)(void);
    void (*destroy)(void *);
    bool (*scan)(void *, TSLexer *, const bool *symbol_whitelist);
    unsigned (*serialize)(void *, char *);
    void (*deserialize)(void *, const char *, unsigned);
  } external_scanner;
  // A state id has 16 bits in ABI 14 and 15, and 32 bits in the ABI of the tree-sitter-cpp fork.
  const void *primary_state_ids;
  const char *name;
  const TSSymbol *reserved_words;
  // A reserved word set has a maximum of 65,535 words.
  uint16_t max_reserved_word_set_size;
  uint32_t supertype_count;
  const TSSymbol *supertype_symbols;
  const TSMapSlice *supertype_map_slices;
  const TSSymbol *supertype_map_entries;
  TSLanguageMetadata metadata;
  // The parse tables of ABI 1016, in the shape layout (tree-sitter-cpp fork). A GROUP of a state is a
  // maximal set of symbols that share one table value. The SHAPE of a state is the ordered list of its
  // symbols together with the group of each symbol. Many states share one shape, and a column of a
  // shape is frequently one value for each state of that shape.
  //
  // `state_shape[state]` gives the shape. The range `shape_offset[shape]` thru `shape_offset[shape+1]`
  // of `shape_symbols` and of `shape_slots` holds the symbols of that shape and the storage slot of
  // the group of each symbol. The symbols are in the order in which the runtime reads them: ascending
  // symbol for a state below `large_state_count`, and group order for a state above it. A slot below
  // `shape_nvar[shape]` reads `state_values[state_value_offset[state] + slot]`, and a higher slot
  // reads `shape_const[shape_const_offset[shape] + slot - shape_nvar[shape]]`.
  //
  // A language of a lower ABI version keeps these nine pointers null. The runtime must read one of
  // them only after a test of the ABI version. Refer to `ts_language_lookup` in language.h.
  const TSSymbol *shape_symbols;
  const uint16_t *shape_slots;
  const uint32_t *shape_offset;
  const uint16_t *shape_nvar;
  const uint32_t *shape_const;
  const uint32_t *shape_const_offset;
  const uint32_t *state_shape;
  const uint32_t *state_value_offset;
  const uint32_t *state_values;
  // The entry point of the external scanner that takes the context of the parser, the last field
  // of the struct (tree-sitter-cpp fork, ABI 1017). `ts_parser_set_scanner_context` stores an
  // opaque pointer on the parser, and the runtime gives it to this function right after
  // `external_scanner.create`, and again when the context changes while a scanner exists. A
  // language with no such function keeps the field null, and a language of a lower ABI has no
  // such field at all. The runtime must read it only after a test of the ABI version. Refer to
  // `ts_language_scanner_set_context` in language.h.
  void (*external_scanner_set_context)(void *payload, const void *context);
};

static inline bool set_contains(const TSCharacterRange *ranges, uint32_t len, int32_t lookahead) {
  uint32_t index = 0;
  uint32_t size = len - index;
  while (size > 1) {
    uint32_t half_size = size / 2;
    uint32_t mid_index = index + half_size;
    const TSCharacterRange *range = &ranges[mid_index];
    if (lookahead >= range->start && lookahead <= range->end) {
      return true;
    } else if (lookahead > range->end) {
      index = mid_index;
    }
    size -= half_size;
  }
  const TSCharacterRange *range = &ranges[index];
  return (lookahead >= range->start && lookahead <= range->end);
}

// True when a character set holds the character (tree-sitter-cpp fork).
//
// `ascii` holds one bit for each of the code points 0 to 127, and the generator writes it beside the
// ranges of the set. Nearly each character of a source is in that range, and the lexer then reads
// one bit in the place of a binary search of the ranges. A set of the identifier characters of
// Unicode has 687 ranges, so the search takes ten steps for each letter of each name.
//
// A lookahead that is not in the range 0 to 127, and a lookahead that is negative, take the search.
static inline bool set_contains_ascii(
  const TSCharacterRange *ranges,
  uint32_t len,
  const uint64_t *ascii,
  int32_t lookahead
) {
  uint32_t code_point = (uint32_t)lookahead;
  if (code_point < 128) {
    return (ascii[code_point >> 6] >> (code_point & 63)) & 1;
  }
  return set_contains(ranges, len, lookahead);
}

/*
 *  Lexer Macros
 */

#ifdef _MSC_VER
#define UNUSED __pragma(warning(suppress : 4101))
#else
#define UNUSED __attribute__((unused))
#endif

#define START_LEXER()           \
  bool result = false;          \
  bool skip = false;            \
  UNUSED                        \
  bool eof = false;             \
  int32_t lookahead;            \
  goto start;                   \
  next_state:                   \
  lexer->advance(lexer, skip);  \
  start:                        \
  skip = false;                 \
  lookahead = lexer->lookahead;

#define ADVANCE(state_value) \
  {                          \
    state = state_value;     \
    goto next_state;         \
  }

#define ADVANCE_MAP(...)                                              \
  {                                                                   \
    static const uint16_t map[] = { __VA_ARGS__ };                    \
    for (uint32_t i = 0; i < sizeof(map) / sizeof(map[0]); i += 2) {  \
      if (map[i] == lookahead) {                                      \
        state = map[i + 1];                                           \
        goto next_state;                                              \
      }                                                               \
    }                                                                 \
  }

#define SKIP(state_value) \
  {                       \
    skip = true;          \
    state = state_value;  \
    goto next_state;      \
  }

#define ACCEPT_TOKEN(symbol_value)     \
  result = true;                       \
  lexer->result_symbol = symbol_value; \
  lexer->mark_end(lexer);

#define END_STATE() return result;

/*
 *  Parse Table Macros
 */

#define SMALL_STATE(id) ((id) - LARGE_STATE_COUNT)

#define STATE(id) id

#define ACTIONS(id) id

#define SHIFT(state_value)                          \
  {{                                                \
    .shift = {                                      \
      .type = TSParseActionTypeShift,               \
      .state_low = (uint16_t)(state_value),         \
      .state_high = (uint16_t)((state_value) >> 16) \
    }                                               \
  }}

#define SHIFT_REPEAT(state_value)                    \
  {{                                                 \
    .shift = {                                       \
      .type = TSParseActionTypeShift,                \
      .state_low = (uint16_t)(state_value),          \
      .state_high = (uint16_t)((state_value) >> 16), \
      .repetition = true                             \
    }                                                \
  }}

#define SHIFT_EXTRA()                 \
  {{                                  \
    .shift = {                        \
      .type = TSParseActionTypeShift, \
      .extra = true                   \
    }                                 \
  }}

#define REDUCE(symbol_name, children, precedence, prod_id) \
  {{                                                       \
    .reduce = {                                            \
      .type = TSParseActionTypeReduce,                     \
      .symbol = symbol_name,                               \
      .child_count = children,                             \
      .dynamic_precedence = precedence,                    \
      .production_id = prod_id                             \
    },                                                     \
  }}

#define RECOVER()                    \
  {{                                 \
    .type = TSParseActionTypeRecover \
  }}

#define ACCEPT_INPUT()              \
  {{                                \
    .type = TSParseActionTypeAccept \
  }}

#ifdef __cplusplus
}
#endif

#endif  // TREE_SITTER_PARSER_H_
