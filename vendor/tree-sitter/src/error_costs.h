#ifndef TREE_SITTER_ERROR_COSTS_H_
#define TREE_SITTER_ERROR_COSTS_H_

#include <stdint.h>

#define ERROR_STATE 0
#define ERROR_COST_PER_RECOVERY 500
#define ERROR_COST_PER_MISSING_TREE 110
#define ERROR_COST_PER_SKIPPED_TREE 100
#define ERROR_COST_PER_SKIPPED_LINE 30
#define ERROR_COST_PER_SKIPPED_CHAR 1

// An ERROR node has the first cost for each right brace with no left brace before it in the node.
// It has the second cost for each right parenthesis or right square bracket with no left one.
// After a syntax error, GCC and Clang skip tokens. They stop before a right bracket if they did
// not skip its left bracket (cp_parser_skip_to_end_of_statement in gcc/cp/parser.cc, and
// Parser::SkipUntil in clang/lib/Parse/Parser.cpp). An ERROR node with such a right brace keeps
// the construct of the brace open. The error region can then continue until the end of the file
// (tree-sitter-cpp fork).
#define ERROR_COST_PER_UNMATCHED_BRACE 10000
#define ERROR_COST_PER_UNMATCHED_PARENTHESIS 1000

// The error costs of subtrees and of stack versions stop at ERROR_COST_MAX. A cost of ERROR_COST_MAX
// tells that the full cost is ERROR_COST_MAX or more. A sum with a part of ERROR_COST_MAX is also
// ERROR_COST_MAX, and two such costs are equal. The sums use 64 bits before they stop, and a sum
// with no such part is exact. A sum of 32 bits can become smaller than one of its parts. An ERROR
// node then gets a cost that is less than the cost of its unmatched brackets (tree-sitter-cpp fork).
#define ERROR_COST_MAX UINT32_MAX

// Stop a cost of 64 bits at ERROR_COST_MAX (tree-sitter-cpp fork).
static inline uint32_t ts_error_cost_saturate(uint64_t cost) {
  return cost >= ERROR_COST_MAX ? ERROR_COST_MAX : (uint32_t)cost;
}

// Add an increment to an error cost, and stop the sum at ERROR_COST_MAX. The increment must be
// less than 2^63 (tree-sitter-cpp fork).
static inline uint32_t ts_error_cost_add(uint32_t cost, uint64_t increment) {
  return ts_error_cost_saturate((uint64_t)cost + increment);
}

#endif
