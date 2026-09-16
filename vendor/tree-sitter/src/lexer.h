#ifndef TREE_SITTER_LEXER_H_
#define TREE_SITTER_LEXER_H_

#ifdef __cplusplus
extern "C" {
#endif

#include "./length.h"
#include "./subtree.h"
#include "tree_sitter/api.h"
#include "./parser.h"

typedef struct {
  uint32_t value;
  bool valid;
} ColumnData;

// The character column of one byte position of one row (tree-sitter-cpp fork).
//
// `ts_lexer__get_column` reads the line again from its start, and a lexer that moves invalidates
// `column_data` at each step. A scanner that reads the column one time for each token of a long line
// then takes a time that grows with the square of the length of the line. The anchor holds the
// result of the last computation, and the next computation reads the line from the anchor. A parse
// that moves forward then reads each line one time.
//
// The anchor is a fact about the text and the included ranges, and not about the position of the
// lexer. `ts_lexer_goto` keeps it, and only a new input or a new list of included ranges clears it.
typedef struct {
  uint32_t row;
  uint32_t byte;
  uint32_t column;
  bool valid;
} ColumnAnchor;

typedef struct {
  TSLexer data;
  Length current_position;
  Length token_start_position;
  Length token_end_position;

  TSRange *included_ranges;
  const char *chunk;
  TSInput input;
  TSLogger logger;

  uint32_t included_range_count;
  uint32_t current_included_range_index;
  uint32_t chunk_start;
  uint32_t chunk_size;
  uint32_t lookahead_size;
  bool did_get_column;
  ColumnData column_data;
  ColumnAnchor column_anchor;

  char debug_buffer[TREE_SITTER_SERIALIZATION_BUFFER_SIZE];
} Lexer;

void ts_lexer_init(Lexer *self);
void ts_lexer_delete(Lexer *self);
void ts_lexer_set_input(Lexer *self, TSInput input);
void ts_lexer_reset(Lexer *self, Length position);
void ts_lexer_start(Lexer *self);
void ts_lexer_finish(Lexer *self, uint32_t *lookahead_end_byte);
void ts_lexer_mark_end(Lexer *self);
bool ts_lexer_set_included_ranges(Lexer *self, const TSRange *ranges, uint32_t count);
TSRange *ts_lexer_included_ranges(const Lexer *self, uint32_t *count);

#ifdef __cplusplus
}
#endif

#endif  // TREE_SITTER_LEXER_H_
