#include "tree_sitter/alloc.h"
#include "tree_sitter/parser.h"

#include "seed.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <wctype.h>

/// The external tokens, in the order of `g.externals` in `grammar/src/cpp.rs`.
enum TokenType {
    RAW_STRING_DELIMITER,
    RAW_STRING_CONTENT,
    DECAY_COPY_AUTO,
    MACRO_LINE_START,
    MACRO_BLOCK_START,
    MACRO_CALL_START,
    MACRO_ENUMERATOR_START,
    TRAILING_MACRO_NAME,
    CALL_MACRO_NAME,
    QT_EMIT_MARKER,
    QT_FOREACH_MARKER,
    TYPE_TRAIT_MARKER,
    MS_ASM_CODE,
    VA_ARG_MARKER,
    MEMBER_POINTER_START,
    PREPROC_LINE_END,
    PREPROC_DIRECTIVE,
    PREPROC_DEFINE,
    PREPROC_INCLUDE,
    PREPROC_IF,
    PREPROC_IFDEF,
    PREPROC_IFNDEF,
    PREPROC_ELIF,
    PREPROC_ELIFDEF,
    PREPROC_ELIFNDEF,
    PREPROC_ELSE,
    PREPROC_ENDIF,
    PREPROC_CONDITIONAL,
    PREPROC_SKIPPED,
    /// A token that the scanner never gives. It is valid only in a string literal or a character literal.
    PREPROC_LITERAL_MARKER,
    /// The `#embed` directive where an expression can start, and a declaration or a statement cannot.
    PREPROC_EMBED,
    /// The `auto` of a structured binding declaration: `auto& [a, b] = p;`.
    STRUCTURED_BINDING_AUTO,
    /// The directive of the last line of an `#elif` or `#else` branch of a structured group, in the place of
    /// `PREPROC_DIRECTIVE`, `PREPROC_DEFINE`, or `PREPROC_INCLUDE`.
    PREPROC_FINAL_DIRECTIVE,
    PREPROC_FINAL_DEFINE,
    PREPROC_FINAL_INCLUDE,
    /// The first directive of a structured group with no case label at its top level, in the place of
    /// `PREPROC_IF`, `PREPROC_IFDEF`, or `PREPROC_IFNDEF`. A case body reads such a group.
    PREPROC_IF_IN_CASE,
    PREPROC_IFDEF_IN_CASE,
    PREPROC_IFNDEF_IN_CASE,
    /// An empty extra before the class key of a class head with a body. The scanner records the name of
    /// the class.
    CLASS_HEAD_MARK,
    /// An empty token before the macros of a constructor or a destructor.
    CONSTRUCTOR_MACRO_START,
    /// A token that the scanner never gives. It comes after a `>=` where a template argument list can end.
    UNREACHABLE_TOKEN,
    /// An empty token before a function-like macro in the place of an attribute, before the specifiers of
    /// a declaration.
    MACRO_CALL_ATTRIBUTE_START,
    /// The same token before a macro with arguments that are not expressions.
    MACRO_CALL_ATTRIBUTE_TOKENS_START,
    /// A name before a `<` that starts a comparison and not a template argument list:
    /// `x < 0 || x > (n - 1)`. The token holds the name.
    COMPARISON_NAME,
    /// A token that the scanner never gives. It is valid only after the `{` of a braced list.
    INITIALIZER_LIST_MARKER,
    /// The name of a macro after the `(` of a grouping, in the place of a calling convention:
    /// `typedef void (WINAPI *F)(void *);`. The token holds the name.
    GROUPING_CALL_MACRO_NAME,
    /// The `(` of a C-style cast with a name as its type: `(Value) * x`, `(DWORD)(x)`.
    CAST_PAREN,
    /// The `(` of a parenthesized expression that holds a name: `(x) * y`, `(f)(a, b)`, `sizeof(x)`.
    NAME_EXPRESSION_PAREN,
    /// The `(` of the operand of `sizeof` or `typeid` that is a type name: `sizeof(Value)`, `typeid(A<B>)`.
    OPERAND_TYPE_PAREN,
    /// The name of a macro after the declarator of a variable or a data member.
    DECLARATOR_MACRO_NAME,
    /// The name of a macro after the name of an enumerator.
    ENUMERATOR_MACRO_NAME,
    /// An empty token before a statement macro with arguments and a body: `FOREACH(item, items) {`. The token is
    /// valid where a statement can start and a function definition cannot: in a block, a case body, a label, or a
    /// substatement.
    MACRO_STATEMENT_START,
    /// The `...` of a C++26 pack index that a comment, a line splice, or a directive line divides from its `[`:
    /// `T...`, a `#pragma` line, and `[0]`. The lexer reads the other forms as one `...[` token.
    PACK_INDEX_ELLIPSIS,
    /// An empty token before a function-like macro that gives the type of a declaration, in the place of a
    /// type specifier: `STACK_OF(X509) *sk;`.
    MACRO_TYPE_START,
    /// The same token where the declaration is a parameter: `void f(BOOST_FWD_REF(T) x);`. A `)` then also
    /// ends the declarator.
    PARAMETER_MACRO_TYPE_START,
    /// The `-` before a number that has a ud-suffix: `-1_k`. The token is the `-` of the grammar.
    MINUS_BEFORE_SUFFIXED_NUMBER,
    /// The `+` before a number that has a ud-suffix: `+1_k`.
    PLUS_BEFORE_SUFFIXED_NUMBER,
    /// The `(` of the operand of the GNU `alignof` that is a type name: `__alignof__(locale)`.
    ALIGNOF_TYPE_PAREN,
    /// The `[:` that opens a C++26 splice. The scanner gives no token for the `[` of `a[::b]`.
    SPLICE_OPEN,
    /// The first `[` of an attribute that a comment, a line splice, or a directive line divides from the
    /// second `[`. The lexer reads the other forms as one `[[` token.
    ATTRIBUTE_OPEN_BRACKET,
    /// The first `]` of an attribute, with the same gap before the second `]`.
    ATTRIBUTE_CLOSE_BRACKET,
    /// An empty token before a macro in the place of an attribute of a statement:
    /// `MUST_TAIL_CALL return g(a);`.
    STATEMENT_ATTRIBUTE_MACRO_START,
    /// The name of a macro after the `*` or the `&` of a declarator: `char * BROTLI_RESTRICT buf;`.
    POINTER_CALL_MACRO_NAME,
    /// The name of a macro between the name of a declarator and its parameter list:
    /// `const P& min BOOST_PREVENT_MACRO_SUBSTITUTION () const;`.
    DECLARATOR_NAME_MACRO_NAME,
    /// An empty token before the name of a compiler trait that gives a type: `__underlying_type(E)`.
    /// The token is valid where a type specifier can start.
    TYPE_TRAIT_TYPE_MARKER,
    /// The `[` of the grammar, for the digraph `<:`.
    OPEN_BRACKET,
    /// An empty token before the `(` of an attribute-argument-clause that is not an expression list.
    ATTRIBUTE_TOKENS_MARKER,
    /// The text of a directive line: the value of a `#define`, the argument of a `#pragma`, and the
    /// parameters of an `#embed`.
    PREPROC_ARG,
    /// A mark before the `(` of the parameter list of a function-like macro. The scanner gives no such
    /// token, and it reads the mark in `valid_symbols` only.
    PREPROC_PARAMS_MARK,
    /// The name of a class head that the scanner recorded, before the `(` of a functional cast: `A(x)`.
    /// The token holds the name.
    FUNCTIONAL_CAST_NAME,
    /// An empty token before the name of a macro invocation line that a storage class specifier comes
    /// before: `static Q_LOGGING_CATEGORY(log, "qtc", QtWarningMsg)`. The scan gives it only for a name
    /// with arguments.
    MACRO_LINE_AFTER_SPECIFIERS,
    /// An empty mark before the name of a macro in the head of a class that has no member:
    /// `struct LLVM_ABI A;`, `class BASE_EXPORT C {};`. The parser reads the mark after a class key
    /// only, and the scanner gives it only when a macro-shaped name comes first.
    CLASS_MACRO_MARK,
    /// An empty mark before the extra tokens of an `#endif` or an `#else` line. The scanner gives the
    /// mark only when the rest of that line holds a token.
    PREPROC_EXTRA_MARK,
    /// The name of a macro between the name of a function and its argument list, in an expression:
    /// `c_policies::acosh BOOST_MATH_PREVENT_MACRO_SUBSTITUTION(x)`. The token holds the name.
    CALL_NAME_MACRO_NAME,
    /// An empty token before the name of a macro at the head of an element of a braced list:
    /// `{ PyVarObject_HEAD_INIT(nullptr, 0) "n", 0 }`. The scan gives it when an element comes after
    /// the arguments of the name and no comma divides the two.
    INITIALIZER_MACRO_START,
    /// An empty token before the name of a macro that is an attribute of the statement after it, and
    /// whose arguments are not an expression list: `SkDEBUGCODE(bool found =) find(1);`.
    STATEMENT_ATTRIBUTE_MACRO_TOKENS_START,
    /// An empty token before the name of a macro call that gives a scope:
    /// `BOOST_MPL_AUX_VALUE_WKND(N)::value`. The scan gives it when a balanced group and a `::` come
    /// after the name.
    MACRO_SCOPE_START,
    /// The name of a macro between the TYPE of a declaration and its declarator:
    /// `typedef unsigned int CV_DECL_ALIGNED(1) unaligned_uint;`. The token holds the name.
    TYPE_ATTRIBUTE_MACRO_NAME,
    /// An empty token before the word `template`. The scan reads the template head and records the
    /// names that it declares as TYPE parameters. Refer to `scan_template_head`.
    TEMPLATE_HEAD_MARK,
    /// The operand of `alignas`, where a source of the parse declares the name as a type:
    /// `template <typename T> struct S { alignas(T) char storage[sizeof(T)]; };`. The token covers
    /// the name and the grammar reads it as a `type_descriptor`. Refer to `is_alignas_type_name`.
    ALIGNAS_TYPE_NAME,
    /// An empty token before the name of a macro call that is a part of a concatenation:
    /// `"arena." STRINGIFY(MALLCTL_ARENAS_ALL) ".purge"`. The scan gives it when a balanced group
    /// and a string literal come after the name.
    CONCATENATED_MACRO_START,
    /// The count of the external tokens, and not a token. `tree_sitter_cpp_external_scanner_create`
    /// compares it with the count of the parser.
    TOKEN_TYPE_COUNT,
};

/// The language of src/parser.c. The scanner reads its count of external tokens.
const TSLanguage *tree_sitter_cpp(void);

/// The maximum number of characters that the scanner reads in the arguments of a macro.
#define MAX_MACRO_ARGUMENTS_LENGTH 4096

/// The spec limits delimiters to 16 chars
#define MAX_DELIMITER_LENGTH 16

/// The maximum number of characters that the function reads to find the comma of a Qt `foreach`.
#define MAX_QT_FOREACH_SCAN 1024

/// The maximum number of characters that the member pointer lookahead reads.
#define MAX_MEMBER_POINTER_LOOKAHEAD 512

/// The length of the longest name that the scanner compares with a directive name or a keyword.
#define MAX_NAME_LENGTH 16

/// The maximum nesting of conditional groups that the scanner records. A deeper group is a line group.
#define MAX_GROUPS 64

/// The kind of an open conditional group.
typedef enum {
    /// A group that the grammar reads as a `preproc_if` or a `preproc_ifdef`.
    GROUP_STRUCTURED = 1,
    /// A group of directive lines in which the parser reads a branch as code.
    GROUP_LINE_CHOSEN = 2,
    /// A group of directive lines in which all branches up to this point are false, as in `#if 0`.
    GROUP_LINE_PENDING = 3,
    /// A group of directive lines in which the parser reads a later branch (`select_line_branch`), and skips the
    /// branches before it. The value `GROUP_LINE_SELECTED + n` tells that n more `#elif` or `#else` directives come
    /// before the `#elif` or `#else` of that branch.
    GROUP_LINE_SELECTED = 4,
} GroupKind;

/// The largest index of a branch that `select_line_branch` can select. The kind of the group holds the index.
#define MAX_SELECTED_BRANCH (255 - GROUP_LINE_SELECTED)

/// True for a group of directive lines in which the parser skips the current branch.
static inline bool is_pending_group(uint8_t kind) { return kind == GROUP_LINE_PENDING || kind >= GROUP_LINE_SELECTED; }

/// The number of class names that the scanner records. A new name removes the oldest name.
#define MAX_CLASSES 32

typedef struct {
    uint8_t delimiter_length;
    wchar_t delimiter[MAX_DELIMITER_LENGTH];
    /// The open conditional groups, the innermost last.
    uint8_t group_count;
    uint8_t groups[MAX_GROUPS];
    /// The number of open groups that are deeper than MAX_GROUPS. Each one is a group of directive lines, and its
    /// `#endif` closes it.
    uint16_t deep_groups;
    /// The number of recorded class names.
    uint8_t class_count;
    /// True when the last `#endif` or `#else` of the scanner has a token after it on its line.
    ///
    /// The mark of those extra tokens is empty, so a scan of the text cannot find the line again: the
    /// parser asks for the mark at the position of the next token, which is on the line that follows.
    /// The scan of the directive line reads the answer, and the mark reads it here.
    bool preproc_extra_tokens;
    /// The hashes of the names of the last class heads with a body, the most recent last.
    uint32_t classes[MAX_CLASSES];
    /// MEASUREMENT OF TASK 240. The number of names of the second record.
    uint8_t loose_count;
    /// MEASUREMENT OF TASK 240. The hashes of the names of the last class heads, with no condition on
    /// the body. Only the functional cast reads this record. The constructor rules read `classes`.
    uint32_t loose[MAX_CLASSES];
    /// The number of recorded template type parameter names.
    uint8_t template_count;
    /// The hashes of the names that the last template heads declare as TYPE parameters, the most
    /// recent last. `alignas(T)` reads them, and nothing else does.
    ///
    /// THE RECORD IS NOT SCOPED. A template parameter leaves scope at the end of its declaration and
    /// this record keeps it. The measurement of task 291 over the 1,173 `alignas` sites of the
    /// corpus with a bare name: 200 sites name a parameter of their OWN declaration, and a record
    /// filled as the scan goes forward reaches all 200 and evicts none. That is the shape of the
    /// construct and not luck: the template head of an in-scope site is immediately before it, so
    /// its parameter is the most recent entry, and an eviction would need more than MAX_CLASSES
    /// distinct parameter names between the head and the operand of its own `alignas`.
    ///
    /// 6 sites name a parameter of the file that is NOT of their own declaration, 4 of those have it
    /// declared earlier and a forward record can hold it, AND ALL 4 WOULD BE RIGHT ANYWAY, because
    /// the file or the project declares the name as a type as well. Exposure 4, wrong 0.
    uint32_t templates[MAX_CLASSES];
    /// The context of the parser, the seed of #275, or NULL. The runtime gives it through
    /// `tree_sitter_cpp_external_scanner_set_context`, and `serialize` and `deserialize` do not
    /// carry it, so `reset` and `deserialize` must not clear it.
    const void *context;
} Scanner;

/// The traits of GCC (gcc/cp/cp-trait.def) and Clang (clang/include/clang/Basic/BuiltinTraits.td)
/// that give a value, in the order of `strcmp`.
///
/// The table holds no trait that gives a type, for example `__underlying_type`. Those traits are in
/// `TYPE_TRAIT_TYPES`, with the token `TYPE_TRAIT_TYPE_MARKER` of their own. An older comment said
/// that the fork reads no such trait, because a marker at each start of a type splits many parser
/// states, and nested template argument lists then take more parser versions than the parser keeps.
/// The runtime merges those versions. Refer to upstream-patches/11-merge-finished-version.patch.
static const char *const TYPE_TRAITS[] = {
    "__array_extent",
    "__array_rank",
    "__builtin_bit_cast",
    "__builtin_ge_synthesizes_from_spaceship",
    "__builtin_gt_synthesizes_from_spaceship",
    "__builtin_is_cpp_trivially_relocatable",
    "__builtin_is_implicit_lifetime",
    "__builtin_is_structural",
    "__builtin_is_virtual_base_of",
    "__builtin_le_synthesizes_from_spaceship",
    "__builtin_lt_synthesizes_from_spaceship",
    "__builtin_omp_required_simd_align",
    "__builtin_ptrauth_type_discriminator",
    "__builtin_structured_binding_size",
    "__builtin_type_order",
    "__builtin_vectorelements",
    "__can_pass_in_regs",
    "__datasizeof",
    "__has_nothrow_assign",
    "__has_nothrow_constructor",
    "__has_nothrow_copy",
    "__has_nothrow_move_assign",
    "__has_trivial_assign",
    "__has_trivial_constructor",
    "__has_trivial_copy",
    "__has_trivial_destructor",
    "__has_trivial_move_assign",
    "__has_trivial_move_constructor",
    "__has_unique_object_representations",
    "__has_virtual_destructor",
    "__is_abstract",
    "__is_aggregate",
    "__is_arithmetic",
    "__is_array",
    "__is_assignable",
    "__is_base_of",
    "__is_bitwise_cloneable",
    "__is_bounded_array",
    "__is_class",
    "__is_complete_type",
    "__is_compound",
    "__is_const",
    "__is_constructible",
    "__is_convertible",
    "__is_convertible_to",
    "__is_destructible",
    "__is_empty",
    "__is_enum",
    "__is_final",
    "__is_floating_point",
    "__is_function",
    "__is_fundamental",
    "__is_integral",
    "__is_interface_class",
    "__is_invocable",
    "__is_layout_compatible",
    "__is_literal",
    "__is_literal_type",
    "__is_lvalue_expr",
    "__is_lvalue_reference",
    "__is_member_function_pointer",
    "__is_member_object_pointer",
    "__is_member_pointer",
    "__is_nothrow_assignable",
    "__is_nothrow_constructible",
    "__is_nothrow_convertible",
    "__is_nothrow_destructible",
    "__is_nothrow_invocable",
    "__is_object",
    "__is_pod",
    "__is_pointer",
    "__is_pointer_interconvertible_base_of",
    "__is_polymorphic",
    "__is_reference",
    "__is_rvalue_expr",
    "__is_rvalue_reference",
    "__is_same",
    "__is_same_as",
    "__is_scalar",
    "__is_scoped_enum",
    "__is_sealed",
    "__is_signed",
    "__is_standard_layout",
    "__is_trivial",
    "__is_trivially_assignable",
    "__is_trivially_constructible",
    "__is_trivially_copyable",
    "__is_trivially_destructible",
    "__is_trivially_equality_comparable",
    "__is_trivially_relocatable",
    "__is_unbounded_array",
    "__is_union",
    "__is_unsigned",
    "__is_void",
    "__is_volatile",
    "__reference_binds_to_temporary",
    "__reference_constructs_from_temporary",
    "__reference_converts_from_temporary",
};

/// The number of traits in `TYPE_TRAITS`.
#define TYPE_TRAIT_COUNT (sizeof TYPE_TRAITS / sizeof TYPE_TRAITS[0])

/// The traits of GCC (the DEFTRAIT_TYPE rows of gcc/cp/cp-trait.def) and of Clang (the
/// TransformTypeTrait rows of clang/include/clang/Basic/BuiltinTraits.td) that give a type, in the
/// order of `strcmp`.
///
/// Each of these traits takes one type between parentheses. The table holds the union of the two
/// front ends: `__bases` and `__direct_bases` are of GCC only, and `__make_signed`,
/// `__make_unsigned`, `__remove_const`, `__remove_reference_t`, `__remove_restrict`, and
/// `__remove_volatile` are of Clang only.
///
/// `__type_pack_element` is a DEFTRAIT_TYPE row, and it is not in this table. It takes template
/// arguments between angle brackets, and the grammar reads it as a template type
/// (cp_lexer_peek_trait in GCC parser.cc).
static const char *const TYPE_TRAIT_TYPES[] = {
    "__add_lvalue_reference",
    "__add_pointer",
    "__add_rvalue_reference",
    "__bases",
    "__decay",
    "__direct_bases",
    "__make_signed",
    "__make_unsigned",
    "__remove_all_extents",
    "__remove_const",
    "__remove_cv",
    "__remove_cvref",
    "__remove_extent",
    "__remove_pointer",
    "__remove_reference",
    "__remove_reference_t",
    "__remove_restrict",
    "__remove_volatile",
    "__underlying_type",
};

/// The number of traits in `TYPE_TRAIT_TYPES`.
#define TYPE_TRAIT_TYPE_COUNT (sizeof TYPE_TRAIT_TYPES / sizeof TYPE_TRAIT_TYPES[0])

// The loop guard of a debug build.
//
// A time limit of the parser cannot stop a scanner that does not return. In a build without NDEBUG, the scanner
// counts the characters that it reads. Each loop whose number of steps depends on the input calls LOOP_STEP at the
// start of each step. When a scan does more than MAX_IDLE_STEPS loop steps after its last read, the assertion of
// LOOP_STEP stops the process at that loop. A test with a long time limit then also finds a loop with no end.

/// The maximum number of loop steps of one scan after the last read of a character.
#define MAX_IDLE_STEPS 4096

#ifndef NDEBUG
/// The number of characters that the scans of this thread read.
static _Thread_local uint64_t read_count;
/// The value of `read_count` at the last loop step.
static _Thread_local uint64_t read_count_at_step;
/// The loop steps of the current scan since the last read.
static _Thread_local uint32_t idle_steps;

/// Start the loop guard for a new scan.
static inline void start_loop_guard(void) {
    read_count_at_step = read_count;
    idle_steps = 0;
}

/// Record one loop step. Return false when the scan did more than MAX_IDLE_STEPS steps after its last read.
static inline bool loop_step(void) {
    if (read_count != read_count_at_step) {
        read_count_at_step = read_count;
        idle_steps = 0;
    }
    return ++idle_steps <= MAX_IDLE_STEPS;
}

#define LOOP_STEP() assert(loop_step() && "A scanner loop does not read the input.")
#else
static inline void start_loop_guard(void) {}

#define LOOP_STEP() ((void)0)
#endif

/// Count a read of a character for the loop guard. At the end of the input, the lexer does not move, and the
/// count does not change.
static inline void count_read(const TSLexer *lexer) {
#ifndef NDEBUG
    if (!lexer->eof(lexer)) {
        read_count++;
    }
#else
    (void)lexer;
#endif
}

// The token guard of a debug build.
//
// The runtime ends a token at the position of the last `mark_end` of the scan. A scan with no `mark_end` ends the
// token at the position of the lexer when the scan returns, and a `mark_end` of an earlier part of the same scan also
// sets the end. A branch that returns a token then depends on the reads of the parts before it. For this reason,
// each branch that returns a token calls `mark_end`. In a build without NDEBUG, the scanner records the length of
// the token at each `mark_end`. The assertion of `tree_sitter_cpp_external_scanner_scan` stops the process when a
// scan returns a token with no `mark_end`, or an empty token that `can_be_empty` does not permit.

#ifndef NDEBUG
/// The characters that `advance` read after the start of the token of the current scan.
static _Thread_local uint32_t token_length;
/// The length of the token at the last `mark_end` of the current scan, or TOKEN_NOT_MARKED.
static _Thread_local uint32_t marked_length;

/// The value of `marked_length` before the first `mark_end` of a scan.
#define TOKEN_NOT_MARKED UINT32_MAX

/// Start the token guard for a new scan.
static inline void start_token_guard(void) {
    token_length = 0;
    marked_length = TOKEN_NOT_MARKED;
}
#else
static inline void start_token_guard(void) {}
#endif

static inline void advance(TSLexer *lexer) {
#ifndef NDEBUG
    if (!lexer->eof(lexer)) {
        token_length++;
    }
#endif
    count_read(lexer);
    lexer->advance(lexer, false);
}

/// Read a character as white space. The token then starts after the character. When `mark_end` came before the
/// character, the runtime moves the start of the token back to that end, and the token is empty.
static inline void skip(TSLexer *lexer) {
#ifndef NDEBUG
    token_length = 0;
    if (marked_length != TOKEN_NOT_MARKED) {
        marked_length = 0;
    }
#endif
    count_read(lexer);
    lexer->advance(lexer, true);
}

/// End the token at the position of the lexer.
static inline void mark_end(TSLexer *lexer) {
#ifndef NDEBUG
    marked_length = token_length;
#endif
    lexer->mark_end(lexer);
}

static bool advance_space(TSLexer *lexer, const Scanner *scanner);

static inline void reset(Scanner *scanner) {
    scanner->delimiter_length = 0;
    memset(scanner->delimiter, 0, sizeof scanner->delimiter);
}

/// True for a line break: a line feed or a carriage return.
///
/// Phase 1 of [lex.phases] gives each line break the same form. libcpp `_cpp_clean_line`
/// (gcc/libcpp/lex.cc:877) ends a logical line at each of the two characters, and it writes a line feed
/// there (lex.cc:1042). Clang `Lexer::LexTokenInternal` (clang/lib/Lex/Lexer.cpp:3928) reads a carriage
/// return as a line break, and it gives the token `eod` of a directive line for each of the two.
///
/// A carriage return and a line feed after it are one line break. A scan that counts the line breaks of
/// its text reads the two characters as one break.
static inline bool is_line_break(int32_t c) { return c == '\n' || c == '\r'; }

/// True for the white space that a line break does not end: a space, a tab, a form feed, or a vertical
/// tab.
static inline bool is_horizontal_space(int32_t c) {
    return c == ' ' || c == '\t' || c == '\f' || c == '\v';
}

/// True for the white space that can come between the backslash and the line break of a line splice: a space, a
/// tab, a form feed, or a vertical tab (libcpp `is_nvspace`, Clang `isHorizontalWhitespace`).
static inline bool is_splice_space(int32_t c) { return c == ' ' || c == '\t' || c == '\f' || c == '\v'; }

/// The text after a backslash, as `skip_backslash` and `step_backslash` read it.
typedef enum {
    /// A character that is not white space, or the end of the input or of the budget. That character is the
    /// lookahead.
    BACKSLASH_CHARACTER,
    /// White space that a line feed does not end. The character after the white space is the lookahead.
    BACKSLASH_SPACE,
    /// A line splice. The character after its line feed is the lookahead.
    BACKSLASH_SPLICE,
} Backslash;

/// Go past a backslash, and past the line splice or the white space after it. The lookahead is the backslash. With
/// `whitespace`, the characters are white space before a token, and not characters of the token.
///
/// Phase 2 of [lex.phases] deletes each line splice before tokenization (libcpp `_cpp_clean_line`, Clang
/// `Lexer::getEscapedNewLineSize`). A line splice is a backslash, white space of `is_splice_space`, and a line break.
/// The line break is a line feed, a carriage return, or a carriage return and a line feed. `LINE_SPLICE` in
/// grammar/src/c.rs gives the same text to the grammar. Each scanner loop reads a backslash in text with this
/// function or with `step_backslash`. O(n) in the length of the white space.
///
/// A line feed and a carriage return after it are two line breaks, as libcpp `_cpp_clean_line` reads them. Clang
/// `Lexer::getEscapedNewLineSize` reads that order as one line break, and the two front ends disagree here.
static Backslash skip_backslash(TSLexer *lexer, bool whitespace) {
    void (*next)(TSLexer *) = whitespace ? skip : advance;
    Backslash text = BACKSLASH_CHARACTER;
    next(lexer);
    while (is_splice_space(lexer->lookahead)) {
        LOOP_STEP();
        next(lexer);
        text = BACKSLASH_SPACE;
    }
    if (lexer->lookahead == '\r') {
        next(lexer);
        if (lexer->lookahead == '\n') {
            next(lexer);
        }
        return BACKSLASH_SPLICE;
    }
    if (lexer->lookahead != '\n') {
        return text;
    }
    next(lexer);
    return BACKSLASH_SPLICE;
}

/// Go past a backslash in a string literal or a character literal, and past the character that it escapes or the
/// line splice after it (`skip_backslash`). After a backslash and white space, the lookahead can be the quote that
/// ends the literal, as in `"\ "`, and the function does not go past that quote.
static void skip_literal_backslash(TSLexer *lexer) {
    if (skip_backslash(lexer, false) == BACKSLASH_CHARACTER) {
        advance(lexer);
    }
}

/// Scan the raw string delimiter in R"delimiter(content)delimiter"
static bool scan_raw_string_delimiter(Scanner *scanner, TSLexer *lexer) {
    if (scanner->delimiter_length > 0) {
        // Closing delimiter: must exactly match the opening delimiter.
        // We already checked this when scanning content, but this is how we
        // know when to stop. We can't stop at ", because R"""hello""" is valid.
        for (int i = 0; i < scanner->delimiter_length; ++i) {
            if (lexer->lookahead != scanner->delimiter[i]) {
                return false;
            }
            advance(lexer);
        }
        reset(scanner);
        mark_end(lexer);
        return true;
    }

    // Opening delimiter: record the d-char-sequence up to (.
    // A d-char is a basic character that is not a parenthesis, a backslash, or white space.
    // The `(` check comes before the length check, because a delimiter can have 16 characters.
    for (;;) {
        LOOP_STEP();
        if (lexer->lookahead == '(') {
            // Rather than create a token for an empty delimiter, we fail and
            // let the grammar fall back to a delimiter-less rule.
            mark_end(lexer);
            return scanner->delimiter_length > 0;
        }
        if (scanner->delimiter_length >= MAX_DELIMITER_LENGTH || lexer->eof(lexer) || lexer->lookahead == '\\' ||
            lexer->lookahead == ')' || iswspace(lexer->lookahead)) {
            return false;
        }
        scanner->delimiter[scanner->delimiter_length++] = lexer->lookahead;
        advance(lexer);
    }
}

/// Scan the raw string content in R"delimiter(content)delimiter"
static bool scan_raw_string_content(Scanner *scanner, TSLexer *lexer) {
    // The progress made through the delimiter since the last ')'.
    // The delimiter may not contain ')' so a single counter suffices.
    for (int delimiter_index = -1;;) {
        LOOP_STEP();
        // If we hit EOF, consider the content to terminate there.
        // This forms an incomplete raw_string_literal, and models the code
        // well.
        if (lexer->eof(lexer)) {
            mark_end(lexer);
            return true;
        }

        if (delimiter_index >= 0) {
            if (delimiter_index == scanner->delimiter_length) {
                if (lexer->lookahead == '"') {
                    return true;
                }
                delimiter_index = -1;
            } else {
                if (lexer->lookahead == scanner->delimiter[delimiter_index]) {
                    delimiter_index += 1;
                } else {
                    delimiter_index = -1;
                }
            }
        }

        if (delimiter_index == -1 && lexer->lookahead == ')') {
            // The content doesn't include the )delimiter" part.
            // We must still scan through it, but exclude it from the token.
            mark_end(lexer);
            delimiter_index = 0;
        }

        advance(lexer);
    }
}

/// Scan the `auto` of a C++23 decay-copy, `auto(x)` or `auto{x}`: the word `auto` with a `(` or
/// a `{` immediately after it. The scan starts after the word, and the token ends there. The scan
/// fails for a function type with a trailing return type, `function<auto(int)->void>`, where `->`
/// follows the balanced parentheses. For other text the scan also fails, and the grammar reads
/// `auto` as a keyword or as an identifier. The scan does not allocate, and it reads at most to
/// the token after the closing parenthesis. `scanner` holds the open groups.
static bool scan_decay_copy_auto(TSLexer *lexer, const Scanner *scanner) {
    mark_end(lexer);
    if (lexer->lookahead == '{') {
        lexer->result_symbol = DECAY_COPY_AUTO;
        return true;
    }
    if (lexer->lookahead != '(') {
        return false;
    }
    // Skip the balanced parentheses. A string literal or a character literal can contain one.
    unsigned depth = 0;
    do {
        LOOP_STEP();
        int32_t quote = lexer->lookahead;
        if (lexer->eof(lexer)) {
            return false;
        }
        if (quote == '"' || quote == '\'') {
            advance(lexer);
            while (!lexer->eof(lexer) && lexer->lookahead != quote && !is_line_break(lexer->lookahead)) {
                LOOP_STEP();
                if (lexer->lookahead == '\\') {
                    skip_literal_backslash(lexer);
                } else {
                    advance(lexer);
                }
            }
        } else if (quote == '(') {
            depth++;
        } else if (quote == ')') {
            depth--;
        }
        advance(lexer);
    } while (depth > 0);
    if (advance_space(lexer, scanner) && lexer->lookahead == '-') {
        advance(lexer);
        if (lexer->lookahead == '>') {
            return false;
        }
    }
    lexer->result_symbol = DECAY_COPY_AUTO;
    return true;
}

// Macro invocations as statements, declarations, and members.
//
// The three macro tokens are empty. The scanner emits one of them at the start of an item, before
// the name of a macro invocation, and only reads ahead after that position. The rule is the rule
// of clang-format for a macro with no semicolon (UnwrappedLineParser::parseStructuralElement):
// an uppercase name, optional parenthesized arguments, and a line break before a token that can
// start a line (tokenCanStartNewLine). The scan uses no heap memory.

/// The maximum number of characters that one macro scan reads after the start of the name.
#define MACRO_SCAN_LIMIT 0x10000
/// The maximum nesting of brackets that a macro scan follows.
#define MACRO_MAX_DEPTH 64
/// The size of the buffer for a word or an operator that the scan compares with a list. A longer
/// word keeps its first 40 characters.
///
/// THE VALUE CARRIES AN INVARIANT: EVERY ENTRY OF EVERY NAME LIST OF THIS FILE IS SHORTER THAN 40
/// CHARACTERS. A cut word holds exactly 40, so no cut word can equal an entry. That inequality is
/// what makes `is_macro_name`, `is_type_trait`, `is_grammar_keyword`, `is_class_key` and every
/// other caller of `word_in` unable to match a name that the text does not hold. Such a match has
/// no ERROR node and no differ row.
///
/// THE MARGIN IS ONE CHARACTER. The longest entry of the 40 lists of this file is
/// `__builtin_ge_synthesizes_from_spaceship` of TYPE_TRAITS, at 39 characters. C++ adds type
/// traits, and this fork adds keywords, so ordinary work closes the margin.
/// `no_entry_of_a_name_list_can_equal_a_cut_word` of xtask/src/scanner.rs reads this file and fails
/// at the entry that crosses it.
///
/// THE SEED OF src/seed.h CANNOT CARRY THIS INVARIANT. A name list is a closed set that this fork
/// keeps short. A seed is an open set whose entries reach exactly `TS_CPP_SEED_WORD_SIZE - 1`
/// bytes, so a cut word can equal one of them. `is_seed_type_name` reads `word_cut` of the `Reader`
/// in the place of this inequality. Two lookups, two guards, and neither guard works on the other.
#define MACRO_WORD_SIZE 41
/// The minimum length of a macro name with no arguments, as in clang-format.
#define MACRO_MIN_BARE_LENGTH 5

/// The lexer of a lookahead scan, with the number of characters that the scan can still read, and the conditional
/// groups that the scan is in.
typedef struct {
    TSLexer *lexer;
    uint32_t budget;
    /// The hash of the full text of the last word that `read_word` read. The value is never 0.
    uint32_t word_hash;
    /// True when the buffer of the last word was too small and the word lost characters.
    ///
    /// A COMPARISON WITH A NAME LIST DOES NOT READ THIS, because a cut word holds `MACRO_WORD_SIZE
    /// - 1` characters and every entry of every list is shorter, so a cut word can equal none of
    /// them. A SEED LOOKUP MUST READ IT. A seed is an open set whose entries reach the length of
    /// the buffer, so a cut word can equal an entry there, and the seed would then give a name that
    /// the text does not hold.
    bool word_cut;
    /// The scanner, which holds the open groups at the start of the scan, or NULL.
    const Scanner *scanner;
    /// The number of the open groups of `scanner` with an `#endif` that the scan did not read.
    uint32_t outer_groups;
    /// The number of the groups that start in the text of the scan and are open.
    uint32_t group_count;
    /// For each group of `group_count`, the innermost last: true when the scan reads a branch of the group, false
    /// when all branches up to the current branch are false, as in `#if 0`.
    bool groups[MAX_GROUPS];
} Reader;

/// Start a scan at the lookahead of the lexer. `scanner` holds the open groups, or it is NULL.
static inline Reader start_reader(TSLexer *lexer, uint32_t budget, const Scanner *scanner) {
    Reader reader;
    reader.lexer = lexer;
    reader.budget = budget;
    reader.word_hash = 0;
    reader.word_cut = false;
    reader.scanner = scanner;
    reader.outer_groups = scanner != NULL ? scanner->group_count : 0;
    reader.group_count = 0;
    return reader;
}

/// The space between two tokens.
typedef struct {
    /// The line breaks, which include the line breaks in block comments.
    unsigned newlines;
    /// A blank line, a comment, or the first directive of a conditional group is after the first line break. A group
    /// that starts there usually starts new items: `PROFILE_SCOPE` before `#ifdef DEBUG` and `log("x");`. The other
    /// directive lines and the text of a skipped branch do not divide the lines: `HANDLE` before `#else`,
    /// `pthread_t`, `#endif`, and `thread;`.
    bool separated;
    /// A blank line is after the first line break. `separated` is also true.
    bool blank_line;
    /// The gap holds a comment, a line splice, or a directive line, and not white space only.
    bool text;
    /// The scan stopped in a token that cannot start a line, for example `/` or `\`.
    bool blocked;
    /// The scan stopped after a `/` that does not start a comment. `blocked` is also true.
    bool slash;
    /// The scan stopped after the name of a directive that the parser does not read as a directive line: an `#elif`,
    /// `#else`, or `#endif` of a structured group, or an `#embed`. The scan also stops at the end of its budget in
    /// a directive line or after it. `blocked` is also true.
    bool directive;
    /// A directive line that divides the lines is after the first line break: each directive except the `#elif`,
    /// `#else`, and `#endif` of a group around the scan. `separated` is also true.
    bool directive_line;
} Gap;

static bool skip_directive(Reader *reader, Gap *gap);

/// The facts about the top level of a bracketed group.
typedef struct {
    /// A token shows that the group is not a parameter list.
    bool not_parameters;
    /// A `;` or a statement keyword shows that the group holds statements.
    bool statements;
    /// A token shows that an argument in the group is not an expression.
    bool not_expressions;
    /// Two words are in sequence in an argument, or the word of a type starts an argument. The tokens of
    /// the group are then the tokens of no expression list: `(a b c)` and `(const char*)`.
    bool word_sequence;
    /// The group has no token.
    bool empty;
    /// The number of the arguments at the top level of the group.
    unsigned argument_count;
    /// The group has one argument, and the tokens of that argument are the tokens of a type-id.
    bool one_type_id;
    /// An argument is one name that is not the word of a type: `(alias)`, and not `(void)`.
    bool bare_name;
    /// The group has one argument with the shape of a parenthesized declarator: a `*`, a `&`, or a
    /// `&&` first, and then the tokens of a type-id: `(*p)`, `(&r)`, `(*const p)`.
    bool pointer_declarator;
    /// An argument of a nested group that must hold parameters starts with a literal or an operator.
    /// The nested group is then a call, and no argument of the group is a parameter declaration:
    /// `MATCHER_P(IsNode, height, absl::StrCat("height ", height))`. A default argument holds a call
    /// of its own, and the scan does not read the groups after a `=`.
    bool call_arguments;
} Arguments;

/// The kind of the line that comes after a macro invocation.
typedef enum {
    /// The first token cannot start a line.
    NEXT_BLOCKED,
    /// The line starts with a function or variable declarator, and a type can come before it.
    NEXT_DECLARATOR,
    /// The line starts with the declarator of a constructor or a destructor, which has no type.
    NEXT_CONSTRUCTOR,
    /// The line starts with a different token that can start a line.
    NEXT_OTHER,
} NextLine;

/// The words that cannot be the first word of a parameter declaration.
static const char *const NOT_PARAMETER_WORDS[] = {
    "true",        "false",        "nullptr",    "sizeof",           "alignof", "typeid", "new", "delete",
    "co_await",    "static_cast", "dynamic_cast", "const_cast", "reinterpret_cast", "throw",   NULL,
};

/// The keywords that start a statement and are not part of an expression or a declaration.
static const char *const STATEMENT_KEYWORDS[] = {
    "return", "break", "continue", "goto",  "if",      "else",      "for",       "while",    "do",
    "switch", "case",  "try",      "catch", "using",   "typedef",   "namespace", "co_return", "co_yield",
    NULL,
};

/// The words of a type. An argument that starts with one of them is not an expression, unless
/// `(` or `{` comes after it, as in the functional cast `int(x)`.
static const char *const TYPE_WORDS[] = {
    "const",    "volatile", "struct",  "class",    "union",    "enum",     "unsigned", "signed",  "short",
    "long",     "auto",     "static",  "extern",   "register", "inline",   "mutable",  "constexpr", "bool",
    "char",     "int",      "float",   "double",   "void",     "wchar_t",  "char8_t",  "char16_t", "char32_t",
    "size_t",   "ssize_t",  "ptrdiff_t", "intptr_t", "uintptr_t", "int8_t", "int16_t", "int32_t", "int64_t",
    "uint8_t",  "uint16_t", "uint32_t", "uint64_t", "__interface", NULL,
};

/// The keywords that can come immediately before a word in an expression.
static const char *const PREFIX_WORDS[] = {
    "new",   "delete", "sizeof", "alignof", "typeid", "throw",  "co_await", "co_yield", "not",    "compl",
    "and",   "or",     "xor",    "bitand",  "bitor",  "not_eq", "and_eq",   "or_eq",    "xor_eq", "typename",
    "template", "operator", "return", NULL,
};

/// The operators that can start an expression.
static const char *const PREFIX_OPERATORS[] = {"+", "-", "!", "~", "*", "&", "&&", "++", "--", "::", NULL};

/// The operators that can come after the first word of a parameter declaration.
static const char *const DECLARATOR_OPERATORS[] = {"*", "&", "&&", "<", "=", NULL};

/// The two-character operators that the scan reads as one token.
static const char *const OPERATOR_PAIRS[] = {
    "::", "->", "==", "!=", "<=", ">=", "&&", "||", "++", "--", "<<", ">>", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=",
    NULL,
};

/// The keywords that continue the line before them.
static const char *const CONTINUATION_KEYWORDS[] = {
    "noexcept", "and", "or", "bitand", "bitor", "xor", "not_eq", "and_eq", "or_eq", "xor_eq", "override", "final",
    "__asm__",  "__asm", NULL,
};

/// The calling conventions of Microsoft C++, which come between a return type and a declarator.
static const char *const CALL_MODIFIERS[] = {
    "__cdecl", "__clrcall", "__stdcall", "__fastcall", "__thiscall", "__vectorcall", NULL,
};

/// The keywords that can start a line, except `operator`.
static const char *const LINE_KEYWORDS[] = {
    "alignas",   "auto",      "bool",      "char",      "class",        "concept",   "const",     "consteval",
    "constexpr", "constinit", "decltype",  "default",   "delete",       "double",    "enum",      "explicit",
    "export",    "extern",    "float",     "friend",    "inline",       "int",       "long",      "mutable",
    "namespace", "private",   "protected", "public",    "register",     "requires",  "short",     "signed",
    "static",    "static_assert", "struct", "template", "thread_local", "typedef",   "typename",  "union",
    "unsigned",  "using",     "virtual",   "void",      "volatile",     "wchar_t",   "return",    "break",
    "continue",  "goto",      "if",        "else",      "for",          "while",     "do",        "switch",
    "case",      "try",       "catch",     "throw",     "new",          "sizeof",    "co_return", "co_yield",
    "co_await",  "this",      "true",      "false",     "nullptr",      "__if_exists", "__if_not_exists",
    "_Static_assert", "__label__", NULL,
};

/// The prefixes of a raw string literal ([lex.string]). A `"` after one of these words starts a literal whose
/// content holds each line break, each `#`, and each `/*` up to its closing delimiter.
static const char *const RAW_STRING_PREFIXES[] = {"R", "LR", "uR", "UR", "u8R", NULL};

/// True if the word is in the list. O(n) in the length of the list.
static bool word_in(const char *word, const char *const *list) {
    for (; *list; list++) {
        if (strcmp(word, *list) == 0) {
            return true;
        }
    }
    return false;
}

static inline bool is_word_start(int32_t c) {
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_' || c == '$' || c >= 0x80;
}

static inline bool is_word_char(int32_t c) { return is_word_start(c) || (c >= '0' && c <= '9'); }

static inline bool is_digit(int32_t c) { return c >= '0' && c <= '9'; }

/// True if the scan can read the current character.
static inline bool readable(const Reader *reader) {
    return reader->budget > 0 && !reader->lexer->eof(reader->lexer);
}

/// Read the current character, and go to the next one. At the end of the budget, the scan reads no more
/// characters, and the lexer does not move. The decisions of a scan then use no text after its budget.
static inline void step(Reader *reader) {
    if (reader->budget == 0) {
        return;
    }
    reader->budget--;
    advance(reader->lexer);
}

/// The FNV-1a offset basis of 32 bits.
#define HASH_START 2166136261u

/// Add a character to an FNV-1a hash of 32 bits.
static inline uint32_t hash_character(uint32_t hash, int32_t c) {
    for (unsigned shift = 0; shift < 32; shift += 8) {
        hash = (hash ^ (((uint32_t)c >> shift) & 0xff)) * 16777619u;
    }
    return hash;
}

/// The hash of a name as the scanner records it. The value 0 means no name, so a hash of 0 becomes 1.
static inline uint32_t finish_hash(uint32_t hash) { return hash == 0 ? 1 : hash; }

static Backslash step_backslash(Reader *reader);

/// Read a word into a buffer of `size` bytes. Keep its start in `word`, with a NUL at the end, and
/// a `?` for each character that is not ASCII. Set `has_lower` if the word has a lowercase ASCII
/// letter. Keep the hash of the full word in `reader->word_hash`.
///
/// THE HASH READS THE WHOLE WORD AND `size` DOES NOT BOUND IT. Two buffers of different sizes over
/// the same text give the same `reader->word_hash`, so the record of class names reads the same
/// hash whichever size the caller asks for.
///
/// A line splice inside the word is not part of the word. Phase 2 of [lex.phases] deletes each
/// splice before tokenization, so `Q\` and `_OBJECT` on two lines are the one word `Q_OBJECT`
/// (libcpp `_cpp_clean_line`, gcc/libcpp/lex.cc:877, and Clang `Lexer::LexIdentifierContinue`,
/// clang/lib/Lex/Lexer.cpp:2039). The word, its hash, and its shape then hold the spliced text, and
/// the tables of macro names read the same word as the lexer of the parser.
static void read_word_sized(Reader *reader, char *word, unsigned size, bool *has_lower) {
    unsigned length = 0;
    uint32_t hash = HASH_START;
    reader->word_cut = false;
    for (;;) {
        while (readable(reader) && is_word_char(reader->lexer->lookahead)) {
            LOOP_STEP();
            int32_t c = reader->lexer->lookahead;
            if (c >= 'a' && c <= 'z') {
                *has_lower = true;
            }
            if (length < size - 1) {
                word[length++] = c < 0x80 ? (char)c : '?';
            } else {
                reader->word_cut = true;
            }
            hash = hash_character(hash, c);
            step(reader);
        }
        // Only a backslash inside a word can continue the word. A word starts at a character of
        // `is_word_start`, so the scan reads no backslash before the first character.
        if (length == 0 || !readable(reader) || reader->lexer->lookahead != '\\') {
            break;
        }
        LOOP_STEP();
        if (step_backslash(reader) == BACKSLASH_SPLICE) {
            continue;
        }
        // The backslash starts a universal character name of the identifier, and not a splice:
        // `jalapeño` is one word. The word holds the backslash, so that the scan reads the one
        // identifier that the lexer of the parser reads.
        if (length < size - 1) {
            word[length++] = '\\';
        } else {
            reader->word_cut = true;
        }
        hash = hash_character(hash, '\\');
    }
    word[length] = '\0';
    reader->word_hash = finish_hash(hash);
}

/// Read a word into a buffer of `MACRO_WORD_SIZE` bytes, which is what a comparison with a list
/// takes. The signature does not change, so `is_region_macro_name` reads a word that is cut at
/// exactly the same character as before.
static void read_word(Reader *reader, char word[MACRO_WORD_SIZE], bool *has_lower) {
    read_word_sized(reader, word, MACRO_WORD_SIZE, has_lower);
}

/// Go past spaces and tabs.
static void skip_blanks(Reader *reader) {
    while (readable(reader) && (reader->lexer->lookahead == ' ' || reader->lexer->lookahead == '\t')) {
        LOOP_STEP();
        step(reader);
    }
}

/// Go past a backslash, and past the line splice or the white space after it, in a lookahead scan (`skip_backslash`).
/// Each character is in the budget of the scan, and the scan compares no character after its budget. O(n) in the
/// length of the white space.
static Backslash step_backslash(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    Backslash text = BACKSLASH_CHARACTER;
    step(reader);
    while (readable(reader) && is_splice_space(lexer->lookahead)) {
        LOOP_STEP();
        step(reader);
        text = BACKSLASH_SPACE;
    }
    if (readable(reader) && lexer->lookahead == '\r') {
        step(reader);
        if (readable(reader) && lexer->lookahead == '\n') {
            step(reader);
        }
        return BACKSLASH_SPLICE;
    }
    if (!readable(reader) || lexer->lookahead != '\n') {
        return text;
    }
    step(reader);
    return BACKSLASH_SPLICE;
}

/// Go past the rest of a line comment in a lookahead scan, as `skip_line_comment` does. A line splice continues the
/// comment. The line break stays unread. O(n) in the length of the comment.
static void step_line_comment(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    while (readable(reader) && !is_line_break(lexer->lookahead)) {
        LOOP_STEP();
        if (lexer->lookahead == '\\') {
            step_backslash(reader);
        } else {
            step(reader);
        }
    }
}

/// Go past white space, line splices, comments, and directive lines, and record them in `gap`.
///
/// The preprocessor removes a directive line before the parser gets the tokens (libcpp `_cpp_lex_token`, Clang
/// `Lexer::LexTokenInternal`). A `#` starts a directive line only at the start of a line, after white space and
/// comments. `skip_directive` gives the directive lines that the scan goes past. A line splice does not start a line.
/// O(n) in the length of the text that the scan reads.
static void skip_gap(Reader *reader, Gap *gap) {
    TSLexer *lexer = reader->lexer;
    bool line_is_blank = false;
    bool line_start = false;
    bool carriage_return = false;
    while (readable(reader)) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        bool after_carriage_return = carriage_return;
        carriage_return = c == '\r';
        if (is_line_break(c)) {
            // A carriage return and the line feed after it are one line break.
            if (!(after_carriage_return && c == '\n')) {
                if (gap->newlines > 0 && line_is_blank) {
                    gap->separated = true;
                    gap->blank_line = true;
                }
                gap->newlines++;
            }
            line_is_blank = true;
            line_start = true;
            step(reader);
        } else if (c == '#' && line_start) {
            bool skipped = skip_directive(reader, gap);
            if (!skipped || reader->budget == 0) {
                // A scan that cannot read past the directives stops there, as before a structured group.
                gap->directive |= reader->budget == 0;
                gap->blocked = true;
                return;
            }
            line_is_blank = false;
            line_start = false;
            gap->text = true;
        } else if (is_horizontal_space(c)) {
            step(reader);
        } else if (c == '\\') {
            if (step_backslash(reader) != BACKSLASH_SPLICE) {
                gap->blocked = true;
                return;
            }
            gap->text = true;
        } else if (c == '/') {
            step(reader);
            gap->text = true;
            if (lexer->lookahead == '/') {
                gap->separated |= gap->newlines > 0;
                line_is_blank = false;
                step_line_comment(reader);
            } else if (lexer->lookahead == '*') {
                gap->separated |= gap->newlines > 0;
                line_is_blank = false;
                step(reader);
                bool star = false;
                bool comment_carriage_return = false;
                while (readable(reader) && !(star && lexer->lookahead == '/')) {
                    LOOP_STEP();
                    int32_t inner = lexer->lookahead;
                    star = inner == '*';
                    // A carriage return and the line feed after it are one line break.
                    if (is_line_break(inner) && !(comment_carriage_return && inner == '\n')) {
                        gap->newlines++;
                    }
                    comment_carriage_return = inner == '\r';
                    step(reader);
                }
                step(reader);
            } else {
                gap->blocked = true;
                gap->slash = true;
                return;
            }
        } else {
            return;
        }
    }
}

/// Go past a string or character literal. An unterminated literal ends at the end of its line.
static void skip_quoted(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    int32_t quote = lexer->lookahead;
    step(reader);
    while (readable(reader) && lexer->lookahead != quote && !is_line_break(lexer->lookahead)) {
        LOOP_STEP();
        if (lexer->lookahead != '\\') {
            step(reader);
        } else if (step_backslash(reader) == BACKSLASH_CHARACTER) {
            // The character that the backslash escapes. After white space, the lookahead can be the quote: `"\ "`.
            step(reader);
        }
    }
    if (lexer->lookahead == quote) {
        step(reader);
    }
}

/// Go past a raw string literal from its `"`: R"delimiter(content)delimiter".
///
/// A d-char is each character of the basic character set except a space, a `(`, a `)`, a `\`, and a
/// control character ([lex.string] p1). A `"` IS a d-char, and `R""""(x)""""` holds the delimiter
/// `"""`. A scan that stops at the `"` reads the rest of the literal as ordinary tokens, and the
/// group around it then ends at the wrong place. `skip_raw_string_literal` reads the same delimiter.
static void skip_raw_string(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    int32_t delimiter[MAX_DELIMITER_LENGTH];
    unsigned length = 0;
    step(reader);
    while (readable(reader) && lexer->lookahead != '(') {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        if (length == MAX_DELIMITER_LENGTH || c == ')' || c == '\\' || iswspace(c)) {
            return;
        }
        delimiter[length++] = c;
        step(reader);
    }
    step(reader);
    while (readable(reader)) {
        LOOP_STEP();
        if (lexer->lookahead != ')') {
            step(reader);
            continue;
        }
        step(reader);
        unsigned matched = 0;
        while (matched < length && readable(reader) && lexer->lookahead == delimiter[matched]) {
            step(reader);
            matched++;
        }
        if (matched == length && lexer->lookahead == '"') {
            step(reader);
            return;
        }
    }
}

/// Go past a number literal, with its digit separators, exponent signs, and suffix.
static void skip_number(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    int32_t previous = 0;
    while (readable(reader)) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        bool exponent_sign =
            (c == '+' || c == '-') && (previous == 'e' || previous == 'E' || previous == 'p' || previous == 'P');
        if (!is_word_char(c) && c != '.' && c != '\'' && !exponent_sign) {
            return;
        }
        previous = c;
        step(reader);
    }
}

/// The class of one token in a bracketed group.
typedef enum {
    TOKEN_WORD,
    TOKEN_LITERAL,
    TOKEN_COMMA,
    TOKEN_SEMICOLON,
    /// `::` or `...`, which can start a parameter declaration.
    TOKEN_SCOPE,
    /// `*`, `&`, `&&`, `<`, or `=`, which can come after the first word of a parameter declaration.
    TOKEN_DECLARATOR,
    TOKEN_OPERATOR,
    TOKEN_OPEN,
    TOKEN_CLOSE,
} TokenClass;

/// Read one token, and classify it. The scan is at a token that is not white space or a comment.
/// `word` receives the start of a word, or the text of an operator. An opening or closing bracket
/// stays unread.
static TokenClass read_token(Reader *reader, char word[MACRO_WORD_SIZE]) {
    TSLexer *lexer = reader->lexer;
    int32_t c = lexer->lookahead;
    if (is_word_start(c)) {
        bool has_lower = false;
        read_word(reader, word, &has_lower);
        bool prefix = strcmp(word, "L") == 0 || strcmp(word, "u") == 0 || strcmp(word, "U") == 0 ||
                      strcmp(word, "u8") == 0;
        bool raw_prefix = strcmp(word, "R") == 0 || strcmp(word, "LR") == 0 || strcmp(word, "uR") == 0 ||
                          strcmp(word, "UR") == 0 || strcmp(word, "u8R") == 0;
        if (lexer->lookahead == '"' && raw_prefix) {
            skip_raw_string(reader);
            return TOKEN_LITERAL;
        }
        if ((lexer->lookahead == '"' || lexer->lookahead == '\'') && prefix) {
            skip_quoted(reader);
            return TOKEN_LITERAL;
        }
        return TOKEN_WORD;
    }
    if (is_digit(c)) {
        skip_number(reader);
        return TOKEN_LITERAL;
    }
    if (c == '"' || c == '\'') {
        skip_quoted(reader);
        return TOKEN_LITERAL;
    }
    if (c == '(' || c == '[' || c == '{') {
        return TOKEN_OPEN;
    }
    if (c == ')' || c == ']' || c == '}') {
        return TOKEN_CLOSE;
    }
    step(reader);
    int32_t next = lexer->lookahead;
    word[0] = (char)c;
    word[1] = '\0';
    if (c == '.' && is_digit(next)) {
        skip_number(reader);
        return TOKEN_LITERAL;
    }
    if (c == '.' && next == '.') {
        step(reader);
        if (lexer->lookahead != '.') {
            return TOKEN_OPERATOR;
        }
        step(reader);
        memcpy(word, "...", 4);
        return TOKEN_SCOPE;
    }
    if (next > 0 && next < 0x80) {
        word[1] = (char)next;
        word[2] = '\0';
        if (word_in(word, OPERATOR_PAIRS)) {
            step(reader);
        } else {
            word[1] = '\0';
        }
    }
    if (strcmp(word, ",") == 0) {
        return TOKEN_COMMA;
    }
    if (strcmp(word, ";") == 0) {
        return TOKEN_SEMICOLON;
    }
    if (strcmp(word, "::") == 0) {
        return TOKEN_SCOPE;
    }
    return word_in(word, DECLARATOR_OPERATORS) ? TOKEN_DECLARATOR : TOKEN_OPERATOR;
}

/// The state of the argument that a group scan reads.
typedef struct {
    /// The tokens at the top level of the argument.
    unsigned tokens;
    /// The `(` and `{` groups at the top level of the argument.
    unsigned groups;
    /// The template argument lists that are open.
    unsigned angles;
    bool first_is_word;
    bool first_is_type;
    bool first_is_decltype;
    bool first_is_typename;
    /// The first token is a `*`, a `&`, or a `&&`.
    bool first_is_pointer;
    /// The last token is a word that is not a prefix keyword.
    bool after_word;
    /// The last token is the keyword `operator`.
    bool after_operator_keyword;
    /// The last token closed a template argument list.
    bool after_angle;
    /// The open template argument list has a logical or comparison operator, as a comparison has:
    /// `a < b || c > d`.
    bool angle_logic;
    /// The last token is an operator that needs an operand after it.
    bool after_operator;
    /// A `=` is at the top level of the argument, and a default argument comes after it.
    bool after_assign;
    /// The argument starts with `(`, and that group has no token yet.
    bool empty_parenthesis;
    /// The argument has a group in `()` at its top level.
    bool parenthesis_group;
    /// A token of the argument cannot be a token of a type-id.
    bool not_type_id;
    /// The last token of the argument can end a type-id: a word, a `*`, a `&`, a `&&`, or a template
    /// argument list.
    bool type_id_end;
    /// The argument was a type-id before a `(` opened at its top level, and that group is the parameter
    /// list of an abstract function declarator: `void()`, `void(::boost::system::error_code)`.
    bool function_parameters;
} Argument;

/// Record the facts about an argument at its end.
///
/// An argument is not an expression if it is empty, if it is only the word of a type, if it is
/// `decltype` and its operand, if it starts with `typename` and has no group, or if it ends with an
/// operator.
///
/// The group holds one type-id when the first argument starts with a word, has the other tokens of a
/// type-id, and no second argument comes after it: `STACK_OF(X509)`, `BOOST_RV_REF(const A<B>&)`. A
/// type-id never starts with a pointer or reference operator, and `U_SUCCESS(*code)` is an expression.
///
/// The group has the shape of a parenthesized declarator when its one argument starts with a pointer
/// or reference operator, and the other tokens are the tokens of a type-id: `(*p)`, `(&r)`. In a
/// class body, `Foo (*p);` declares the pointer `p` of type `Foo`.
static void end_argument(Arguments *args, const Argument *arg) {
    args->one_type_id =
        args->argument_count == 0 && arg->first_is_word && !arg->not_type_id && arg->type_id_end;
    args->pointer_declarator = args->argument_count == 0 && arg->first_is_pointer && arg->tokens >= 2 &&
                               !arg->not_type_id && arg->type_id_end;
    args->bare_name |= arg->tokens == 1 && arg->first_is_word && !arg->first_is_type;
    args->argument_count++;
    if (arg->tokens == 0) {
        args->not_parameters = true;
        args->not_expressions = true;
    } else if ((arg->tokens == 1 && arg->first_is_type) || (arg->first_is_decltype && arg->tokens == 2) ||
               (arg->first_is_typename && arg->groups == 0) || arg->after_operator) {
        args->not_expressions = true;
    }
}

/// Go past a balanced group from its opening bracket to the character after its closing bracket,
/// and record the facts about its top level in `args`. Return false if the brackets do not
/// balance before the end of the scan.
///
/// A group is not a parameter list if an argument does not start with a word, `::`, `...`, or
/// `[[`, or if the token after the first word of an argument cannot come after a type name.
/// An argument is not an expression if an operator that cannot start an expression or an empty
/// `()` starts it, if the word of a type starts it, if two words are in sequence, or if a word
/// comes after a template argument list. `end_argument` gives the rules at the end of an argument.
///
/// A `<` after a word opens a template argument list. In it, a comma does not end an argument,
/// and the words do not mark the argument. A comparison with `<` stops these marks until the end.
static bool skip_group(Reader *reader, Arguments *args) {
    TSLexer *lexer = reader->lexer;
    char closers[MACRO_MAX_DEPTH];
    // A group that opens after a name is the parameter list of a declarator, and its own arguments
    // must be parameters. `parameter_group` marks each such group, and `inner_start` and `inner_word`
    // hold the position of the scan in the argument of the group. The top level uses `arg`.
    bool parameter_group[MACRO_MAX_DEPTH];
    bool inner_start[MACRO_MAX_DEPTH];
    bool inner_word[MACRO_MAX_DEPTH];
    unsigned depth = 0;
    bool empty = true;
    Argument arg = {0};
    char word[MACRO_WORD_SIZE];
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        if (!readable(reader) || gap.directive) {
            return false;
        }
        bool top = depth == 1;
        word[0] = '\0';
        TokenClass token = gap.blocked ? TOKEN_OPERATOR : read_token(reader, word);
        if (token == TOKEN_OPEN) {
            int32_t c = lexer->lookahead;
            if (depth == MACRO_MAX_DEPTH) {
                return false;
            }
            closers[depth++] = c == '(' ? ')' : c == '[' ? ']' : '}';
            step(reader);
            // No parameter declaration starts with a braced list: `data::make({ 100, 8000 })`.
            if (depth >= 3 && c == '{' && parameter_group[depth - 2] && inner_start[depth - 2]) {
                args->call_arguments = true;
            }
            bool declarator_group = false;
            if (depth == 2) {
                declarator_group = c == '(' && arg.after_word && arg.angles == 0 && !arg.after_assign;
            } else if (depth > 2) {
                declarator_group = c == '(' && parameter_group[depth - 2] && inner_word[depth - 2];
                inner_start[depth - 2] = false;
                inner_word[depth - 2] = false;
            }
            parameter_group[depth - 1] = declarator_group;
            inner_start[depth - 1] = true;
            inner_word[depth - 1] = false;
            if (!top) {
                arg.empty_parenthesis = false;
                continue;
            }
            empty = false;
            arg.after_word = false;
            arg.after_operator_keyword = false;
            arg.after_angle = false;
            arg.after_operator = false;
            if (arg.angles > 0) {
                continue;
            }
            // White space can come between the two brackets of an attribute: `[ [nodiscard] ]`.
            if (c == '[') {
                Gap brackets = {0};
                skip_gap(reader, &brackets);
            }
            bool attribute = c == '[' && lexer->lookahead == '[';
            if ((arg.tokens == 0 && !attribute) || (arg.tokens == 1 && arg.first_is_word && c == '{')) {
                args->not_parameters = true;
            }
            if (arg.tokens == 1 && arg.first_is_type && c == '[') {
                args->not_expressions = true;
            }
            // In a group in braces, a body after a parameter list is a function definition, and the group holds
            // declarations: `NAME { inline void f(int x) { g(); } }`. A braced list has no such element.
            if (c == '{' && closers[0] == '}' && arg.parenthesis_group) {
                args->statements = true;
            }
            // A `(` after the word of a type opens the parameter list of an abstract function
            // declarator, and the argument stays a type-id:
            // `BOOST_ASIO_COMPLETION_TOKEN_FOR(void(error_code, size_t))` of boost/libs/asio. A name
            // that is no keyword of a type keeps its call, because only name lookup tells a function
            // type from a call: `BOOST_TEST(isnanq(x)) && BOOST_TEST(y);` of
            // boost/libs/charconv/test/test_float128.cpp:583 stays an expression. Each other group
            // gives tokens that no type-id holds.
            arg.function_parameters = c == '(' && arg.tokens == 1 && arg.first_is_type;
            arg.parenthesis_group |= c == '(';
            arg.empty_parenthesis = arg.tokens == 0 && c == '(';
            arg.groups += c != '[';
            arg.not_type_id |= !arg.function_parameters;
            arg.type_id_end = false;
            arg.tokens++;
            continue;
        }
        if (token == TOKEN_CLOSE) {
            int32_t c = lexer->lookahead;
            if (depth == 0 || closers[depth - 1] != c) {
                return false;
            }
            step(reader);
            depth--;
            if (depth == 0) {
                if (!empty) {
                    end_argument(args, &arg);
                }
                args->empty = empty;
                return true;
            }
            if (depth >= 2) {
                inner_start[depth - 1] = false;
                inner_word[depth - 1] = false;
            }
            if (depth == 1 && arg.empty_parenthesis) {
                args->not_expressions = true;
            }
            // The parameter list of an abstract function declarator closed, and the argument is a
            // type-id again: `void(int)` ends with the `)` of its parameter list.
            if (depth == 1 && arg.function_parameters) {
                arg.function_parameters = false;
                arg.type_id_end = true;
            }
            arg.empty_parenthesis = false;
            continue;
        }
        if (!top) {
            arg.empty_parenthesis = false;
            if (depth >= 2) {
                unsigned group = depth - 1;
                if (token == TOKEN_COMMA) {
                    inner_start[group] = true;
                    inner_word[group] = false;
                } else {
                    // No parameter declaration starts with a literal or an operator. A parenthesized
                    // declarator starts with a `*`, a `&`, or a `&&`, which are declarator tokens. The
                    // `^` of a block pointer starts one too: `INTERCEPTOR(void, f, void (^work)(void))`.
                    bool block_pointer = token == TOKEN_OPERATOR && strcmp(word, "^") == 0;
                    if (parameter_group[group] && inner_start[group] && !block_pointer &&
                        (token == TOKEN_LITERAL || token == TOKEN_OPERATOR)) {
                        args->call_arguments = true;
                    }
                    inner_start[group] = false;
                    inner_word[group] = token == TOKEN_WORD;
                }
            }
            // A `;` in parentheses or brackets, outside all braces, is not a token of an expression. The group holds
            // statements: `EMIT_BINARY(BLOCK(add32(x);))`. In braces, a `;` ends a statement of a lambda or of a GNU
            // statement expression: `f([] { g(); })`, `f(({ g(); }))`.
            if (token == TOKEN_SEMICOLON && memchr(closers, '}', depth) == NULL) {
                args->not_parameters = true;
                args->statements = true;
            }
            continue;
        }
        empty = false;
        bool is_word = token == TOKEN_WORD;
        bool prefix_word = is_word && word_in(word, PREFIX_WORDS);
        bool opens_angle = token == TOKEN_DECLARATOR && strcmp(word, "<") == 0 && arg.after_word;
        bool closes_angle = token == TOKEN_OPERATOR && (strcmp(word, ">") == 0 || strcmp(word, ">>") == 0);
        bool pointer_operator = token == TOKEN_DECLARATOR && (strcmp(word, "*") == 0 || strcmp(word, "&") == 0 ||
                                                              strcmp(word, "&&") == 0);
        if (token == TOKEN_SEMICOLON || (is_word && word_in(word, STATEMENT_KEYWORDS))) {
            args->not_parameters = true;
            args->statements = true;
        }
        if (arg.angles > 0) {
            // A template argument list can end an expression, as a variable template does:
            // `is_same_v<A, B>`. Only a word after it marks the argument.
            static const char *const LOGIC_OPERATORS[] = {"||", "&&", "?", "==", "!=", "<=", ">=", NULL};
            if (opens_angle) {
                arg.angles++;
            } else if (closes_angle) {
                unsigned closed = word[1] == '>' ? 2 : 1;
                arg.angles = arg.angles > closed ? arg.angles - closed : 0;
            } else if (word_in(word, LOGIC_OPERATORS)) {
                arg.angle_logic = true;
            }
            arg.after_word = is_word && !prefix_word;
            arg.after_angle = arg.angles == 0 && !arg.angle_logic;
            arg.after_operator = false;
            // A template argument list ends a type-id, and a comparison in it does not: `A<B>`.
            arg.type_id_end = arg.after_angle;
            continue;
        }
        if (token == TOKEN_COMMA) {
            end_argument(args, &arg);
            arg = (Argument){0};
            continue;
        }
        if (arg.tokens == 0) {
            arg.first_is_word = is_word;
            arg.first_is_type = is_word && word_in(word, TYPE_WORDS);
            arg.first_is_decltype = is_word && strcmp(word, "decltype") == 0;
            arg.first_is_typename = is_word && strcmp(word, "typename") == 0;
            arg.first_is_pointer = pointer_operator;
            if (is_word ? word_in(word, NOT_PARAMETER_WORDS) : token != TOKEN_SCOPE) {
                args->not_parameters = true;
            }
            if (!is_word && token != TOKEN_LITERAL && !word_in(word, PREFIX_OPERATORS)) {
                args->not_expressions = true;
            }
        } else {
            if (arg.tokens == 1 && arg.first_is_word && token != TOKEN_WORD && token != TOKEN_SCOPE &&
                token != TOKEN_DECLARATOR) {
                args->not_parameters = true;
            }
            bool adjacent = is_word && !prefix_word && (arg.after_word || arg.after_angle);
            if ((arg.tokens == 1 && arg.first_is_type) || adjacent) {
                args->not_expressions = true;
                args->word_sequence = true;
            }
        }
        if (opens_angle) {
            arg.angles = 1;
            arg.angle_logic = false;
        }
        // A parameter declaration holds an operator of an expression only in its default argument. A
        // macro takes an expression: `BOOST_DATA_TEST_CASE(test1, data::make(s1) + s2, index)`. A
        // pointer or reference operator, a `<`, and a `=` are declarator tokens and stay permitted.
        if (token == TOKEN_OPERATOR && arg.tokens > 0 && !arg.after_assign) {
            args->call_arguments = true;
        }
        bool operator_token = token == TOKEN_OPERATOR || token == TOKEN_DECLARATOR || strcmp(word, "::") == 0;
        bool increment = strcmp(word, "++") == 0 || strcmp(word, "--") == 0;
        arg.after_operator = operator_token && !increment && !arg.after_operator_keyword;
        arg.after_assign |= token == TOKEN_DECLARATOR && strcmp(word, "=") == 0;
        arg.after_operator_keyword = is_word && strcmp(word, "operator") == 0;
        arg.after_word = is_word && !prefix_word;
        arg.after_angle = false;
        // The tokens of a type-id: a name, a `::`, a pointer or reference operator, the `<` of a
        // template argument list, and the `...` of a pack expansion after a type-id:
        // `BOOST_ASIO_COMPLETION_TOKEN_FOR(Signatures...)` of boost/libs/asio. A name that starts a
        // statement or an expression is not a type.
        bool type_name = is_word && !word_in(word, STATEMENT_KEYWORDS) && !word_in(word, NOT_PARAMETER_WORDS);
        bool scope = token == TOKEN_SCOPE && strcmp(word, "::") == 0;
        bool pack_ellipsis = strcmp(word, "...") == 0 && arg.type_id_end;
        arg.not_type_id |= !(type_name || pointer_operator || scope || opens_angle || pack_ellipsis);
        arg.type_id_end = type_name || pointer_operator || pack_ellipsis;
        arg.tokens++;
    }
}

/// Go past a template parameter list from its `<` to the character after its `>`. Return false
/// at a `;`, `{`, or `}` before the `>`.
static bool skip_angles(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    unsigned depth = 0;
    while (readable(reader)) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        if (c == ';' || c == '{' || c == '}') {
            return false;
        }
        step(reader);
        if (c == '<') {
            depth++;
        } else if (c == '>' && --depth == 0) {
            return true;
        }
    }
    return false;
}

static bool scan_constructor_parameters(Reader *reader);
static bool is_class_name(const Scanner *scanner, uint32_t name);

/// Classify the line after a macro invocation, from its first tokens.
///
/// The scan starts at the first token of the line. `skip_gap` went past the directive lines and the skipped branches
/// before it, and the first line of code after them is the line that the parser reads after the macro
/// (GCC `cp_parser_constructor_declarator_p` and Clang `isConstructorDeclarator` also read the tokens after the
/// directives). A declarator is a name before `(`, or one name before `;` or `,`. The name can be qualified.
/// One uppercase name before `(` is a macro call. A constructor or a destructor has no type: `A::A(`,
/// `A::~A(`, `~A(`, and the name of a recorded class head before `(` and parameters. After arguments
/// that can be a parameter list, `try` continues a function definition. Where no statement can start,
/// `throw (` also continues the declarator. `classes` holds the recorded class names, or it is NULL
/// where a member cannot start. With `after_directive`, a directive line comes between the macro and the line,
/// and `__asm__` starts a basic asm declaration, as in `__asm__(".cfi_startproc");` after `#if CPU(ARM64)`.
static NextLine classify_next_line(Reader *reader, bool after_parameters, bool statement, const Scanner *classes,
                                   bool after_directive) {
    TSLexer *lexer = reader->lexer;
    if (lexer->eof(lexer)) {
        return NEXT_OTHER;
    }
    int32_t c = lexer->lookahead;
    if (c == '}' || c == '#') {
        return NEXT_OTHER;
    }
    if (c == '[') {
        // An attribute can start a line. A different `[` continues an expression: `f(x)\n[i] = 0;`. White space
        // can come between the two brackets of an attribute.
        step(reader);
        Gap brackets = {0};
        skip_gap(reader, &brackets);
        return !brackets.blocked && lexer->lookahead == '[' ? NEXT_OTHER : NEXT_BLOCKED;
    }
    bool qualified = false;
    if (c == ':') {
        step(reader);
        if (lexer->lookahead != ':') {
            return NEXT_BLOCKED;
        }
        step(reader);
        skip_blanks(reader);
        qualified = true;
    }
    bool destructor = false;
    if (lexer->lookahead == '~') {
        step(reader);
        skip_blanks(reader);
        destructor = true;
    }
    if (!readable(reader) || !is_word_start(lexer->lookahead)) {
        return qualified || destructor ? NEXT_OTHER : NEXT_BLOCKED;
    }
    char word[MACRO_WORD_SIZE];
    bool has_lower = false;
    read_word(reader, word, &has_lower);
    if (!qualified && !destructor) {
        if (after_directive && (strcmp(word, "__asm__") == 0 || strcmp(word, "__asm") == 0)) {
            return NEXT_OTHER;
        }
        if (word_in(word, CONTINUATION_KEYWORDS) || (after_parameters && strcmp(word, "try") == 0)) {
            return NEXT_BLOCKED;
        }
        if (after_parameters && !statement && strcmp(word, "throw") == 0) {
            // A dynamic exception specification continues a member declarator: `S(...)\nthrow (int);`.
            // In a block, `throw (e);` is a statement after the macro.
            skip_blanks(reader);
            if (lexer->lookahead == '(') {
                return NEXT_BLOCKED;
            }
        }
        if (after_parameters && strcmp(word, "requires") == 0) {
            // A trailing requires-clause continues the declarator ([dcl.decl.general]): `S(int x)` on
            // one line, and `requires C<T>` on the next line.
            return NEXT_BLOCKED;
        }
        if (strcmp(word, "operator") == 0 || word_in(word, CALL_MODIFIERS)) {
            return NEXT_DECLARATOR;
        }
        if (strcmp(word, "__attribute__") == 0 || strcmp(word, "__attribute") == 0) {
            // A GNU attribute continues a function declarator when a body, `;`, `:`, or `=` comes
            // after it: `S()\n__attribute__((nothrow)) {`. A different attribute starts a declaration.
            Gap gap = {0};
            skip_gap(reader, &gap);
            Arguments attribute = {0};
            if (lexer->lookahead != '(' || !skip_group(reader, &attribute)) {
                return NEXT_BLOCKED;
            }
            skip_gap(reader, &gap);
            int32_t after = lexer->lookahead;
            return after == '{' || after == ';' || after == ':' || after == '=' ? NEXT_BLOCKED : NEXT_OTHER;
        }
        if (word_in(word, LINE_KEYWORDS)) {
            return NEXT_OTHER;
        }
    }
    bool single = !qualified && !destructor;
    bool macro_shaped = single && !has_lower;
    bool constructor = false;
    bool class_constructor = single && is_class_name(classes, reader->word_hash);
    for (;;) {
        LOOP_STEP();
        skip_blanks(reader);
        if (lexer->lookahead != ':') {
            break;
        }
        step(reader);
        if (lexer->lookahead != ':') {
            return NEXT_OTHER;
        }
        step(reader);
        skip_blanks(reader);
        if (lexer->lookahead == '~') {
            step(reader);
            destructor = true;
        }
        if (!readable(reader) || !is_word_start(lexer->lookahead)) {
            return NEXT_OTHER;
        }
        char previous[MACRO_WORD_SIZE];
        memcpy(previous, word, sizeof previous);
        read_word(reader, word, &has_lower);
        constructor |= strcmp(previous, word) == 0;
        single = false;
        macro_shaped = false;
        class_constructor = false;
    }
    int32_t next = lexer->lookahead;
    if (next == '(') {
        if (class_constructor) {
            return scan_constructor_parameters(reader) ? NEXT_CONSTRUCTOR : NEXT_DECLARATOR;
        }
        if (constructor || destructor) {
            return NEXT_CONSTRUCTOR;
        }
        return macro_shaped ? NEXT_OTHER : NEXT_DECLARATOR;
    }
    if (single && (next == ';' || next == ',')) {
        return NEXT_DECLARATOR;
    }
    return NEXT_OTHER;
}

/// True if a word that `read_word` read has the shape of a macro name, as clang-format reads it: an
/// uppercase letter, no lowercase letter, and only ASCII letters, digits, and `_`.
static bool is_macro_name(const char *word, bool has_lower) {
    bool has_upper = false;
    for (const char *c = word; *c; ++c) {
        has_upper |= *c >= 'A' && *c <= 'Z';
    }
    return has_upper && !has_lower && !strchr(word, '?') && !strchr(word, '$');
}

/// The first parts of the names of the SAL annotations of MSVC (sal.h, specstrings_strict.h, concurrencysal.h,
/// and driverspecs.h). A prefix with more parts, as `Field_size`, names a family that shares its first part with
/// names that are not annotations: `_Field_default_instance_` of protobuf.
static const char *const SAL_NAME_PREFIXES[] = {
    "Acquires",        "Always",         "Analysis",           "At",
    "Benign_race",     "COM_Outptr",     "Called_from_function_class", "Check_return",
    "Create_lock_level", "Deref",        "Deref2",             "Dispatch_type",
    "Enum_is_bitflag", "Field_range",    "Field_size",         "Field_z",
    "Frees_ptr",       "Function_class", "Guarded_by",         "Has_lock",
    "IRQL",            "In",             "Inexpressible",      "Inout",
    "Interlocked_operand", "Kernel",     "Literal",            "Lock_level_order",
    "Maybenull",       "Maybevalid",     "Must_inspect",       "No_competing_thread",
    "Notliteral",      "Notnull",        "Notvalid",           "NullNull_terminated",
    "Null_terminated", "Nullterm_length", "On_failure",        "Out",
    "Outptr",          "Outref",         "Points_to_data",     "Post",
    "Pre",             "Prepost",        "Printf_format_string", "Readable",
    "Releases",        "Requires",       "Reserved",           "Result",
    "Ret",             "Return_type_success", "Satisfies",     "Scanf",
    "Strict_type_match", "Struct_size",  "Success",            "Translates",
    "Unchanged",       "Use_decl_annotations", "When",         "Writable",
    "Write_guarded_by", NULL,
};

/// True if a word that `read_word` read is the name of a SAL annotation of MSVC: `_In_`, `_Out_writes_bytes_`,
/// `_Success_`. The name has only ASCII letters, digits, and single `_` characters. It starts with `_` and a prefix
/// of `SAL_NAME_PREFIXES` that a `_` ends, and it ends with `_`. sal.h defines these names as macros, empty by
/// default. O(n) in the number of prefixes.
static bool is_sal_name(const char *word) {
    size_t length = strlen(word);
    if (length < 3 || word[0] != '_' || word[length - 1] != '_' || strstr(word, "__") != NULL) {
        return false;
    }
    for (const char *c = word; *c; ++c) {
        if (!(is_word_char(*c) && *c != '$' && *c != '?')) {
            return false;
        }
    }
    for (const char *const *prefix = SAL_NAME_PREFIXES; *prefix; ++prefix) {
        size_t prefix_length = strlen(*prefix);
        if (strncmp(word + 1, *prefix, prefix_length) == 0 && word[1 + prefix_length] == '_') {
            return true;
        }
    }
    return false;
}

/// The parts of a macro name, between the `_` characters, that name a region or a pragma macro:
/// `QT_BEGIN_NAMESPACE`, `AUD_NAMESPACE_END`, `QT_WARNING_PUSH`, `HEDLEY_DIAGNOSTIC_POP`.
static const char *const REGION_MACRO_PARTS[] = {
    "BEGIN", "END", "PUSH", "POP", "NAMESPACE", "WARNING", "WARNINGS", "DIAGNOSTIC", "DIAGNOSTICS", NULL,
};

/// The starts of the names of macros that expand to declarations: `NS_DECL_ISUPPORTS`, `DECLARE_X`.
static const char *const DECLARATION_MACRO_PREFIXES[] = {"DECLARE_", "NS_DECL_", NULL};

/// The Qt macros that expand to declarations and an access specifier.
static const char *const QT_DECLARATION_MACROS[] = {"Q_OBJECT", "Q_GADGET", "Q_NAMESPACE", NULL};

/// True if a macro name has the shape of a macro that opens or closes a region, changes the state of
/// the pragmas, or expands to declarations. Such a macro is not an attribute. In the corpus, the
/// definitions of these names expand to `namespace x {`, `}`, `_Pragma(...)`, or declarations, and an
/// attribute macro almost never has such a name. `word` is a word that `read_word` read. A part at the
/// end of a cut word is not compared. O(n) in the length of the word.
static bool is_region_macro_name(const char *word) {
    if (word_in(word, QT_DECLARATION_MACROS)) {
        return true;
    }
    for (const char *const *prefix = DECLARATION_MACRO_PREFIXES; *prefix; prefix++) {
        if (strncmp(word, *prefix, strlen(*prefix)) == 0) {
            return true;
        }
    }
    bool cut = strlen(word) >= MACRO_WORD_SIZE - 1;
    for (const char *start = word;;) {
        const char *end = strchr(start, '_');
        if (end == NULL && cut) {
            return false;
        }
        size_t length = end == NULL ? strlen(start) : (size_t)(end - start);
        for (const char *const *part = REGION_MACRO_PARTS; *part; part++) {
            if (strlen(*part) == length && strncmp(start, *part, length) == 0) {
                return true;
            }
        }
        if (end == NULL) {
            return false;
        }
        start = end + 1;
    }
}

// Constructors after macros.
//
// A constructor has no type, and the name of its declarator is the name of its class (GCC
// `cp_parser_constructor_declarator_p`, Clang `isConstructorDeclarator`). In a class body, the
// front ends compare the name with the class that they define. Outside the class, `A::A` and `A::~A`
// always name the constructor and the destructor ([class.qual], DR 147). The scanner does similar
// comparisons: it records the names of the last class heads with a body, and it reads the words after
// a macro up to such a name.
//
// The scanner does not keep a stack of the open class bodies. Its state goes into each external token,
// and the parser merges two parse versions only when their states are equal. The tokens of a stack, at
// the braces of a class body, differ between the versions that read `class API A {` as a class and as
// a function, and between the versions of error recovery. The recorded names come from an extra token
// before each class key, which each version reads in the same place.

/// The identifier-shaped strings of the grammar, in the order of `strcmp`. The lexer gives such a
/// string as a keyword where the grammar accepts it. A macro name is never one of them.
static const char *const GRAMMAR_KEYWORDS[] = {
    "FALSE", "NULL", "Q_EMIT", "Q_FOREACH", "Q_FOREVER", "Q_SIGNALS", "Q_SLOTS", "TRUE", "_Alignas", "_Alignof",
    "_Atomic", "_Complex", "_Generic", "_Nonnull", "_Noreturn", "_Null_unspecified", "_Nullable",
    "_Nullable_result", "_Pragma", "_Static_assert", "__alignof", "__alignof__", "__asm", "__asm__", "__attribute",
    "__attribute__",
    "__based",
    "__bool",
    "__builtin_available", "__builtin_offsetof", "__builtin_va_arg", "__catch", "__cdecl", "__clrcall",
    "__complex__",
    "__const", "__const__", "__declspec", "__device__", "__except", "__extension__", "__fastcall", "__finally",
    "__forceinline", "__global__", "__host__", "__if_exists", "__if_not_exists", "__imag", "__imag__",
    "__inline", "__inline__",
    "__interface", "__label__", "__leave", "__nonnull", "__nullable", "__pixel", "__pragma", "__ptr32",
    "__ptr64", "__real", "__real__",
    "__regcall", "__restrict", "__restrict__", "__signed", "__signed__", "__sptr", "__stdcall", "__thiscall",
    "__thread", "__try", "__typeof", "__typeof__", "__typeof_unqual__", "__unaligned", "__uptr", "__vector",
    "__vectorcall",
    "__volatile", "__volatile__", "__w64", "_alignof", "_unaligned", "abstract", "alignas", "alignof", "and", "and_eq",
    "asm", "auto", "bitand", "bitor", "bool", "break", "case", "catch", "char", "char16_t", "char32_t", "char64_t",
    "char8_t", "charptr_t", "class", "co_await", "co_return", "co_yield", "compl", "concept", "const", "const_cast",
    "consteval", "constexpr", "constinit", "continue", "contract_assert", "decltype", "default", "defined", "delete",
    "do", "double", "dynamic_cast", "else", "emit", "enum", "explicit", "export", "extern", "false", "final", "float",
    "for", "foreach", "forever", "friend", "goto", "if", "import", "inline", "int", "int16_t", "int32_t", "int64_t",
    "int8_t", "intptr_t", "long", "max_align_t", "module", "mutable", "namespace", "new", "noexcept", "noreturn",
    "not", "not_eq", "nullptr", "nullptr_t", "offsetof", "operator", "or", "or_eq", "override", "post", "pre",
    "private", "protected", "ptrdiff_t", "public", "register", "reinterpret_cast", "replaceable_if_eligible",
    "requires", "restrict", "return", "sealed", "short", "signals", "signed", "size_t", "sizeof", "slots", "ssize_t",
    "static", "static_assert", "static_cast", "struct", "switch", "template", "this", "thread_local", "throw",
    "trivially_relocatable_if_eligible", "true",
    "try", "typedef", "typeid", "typename", "typeof", "typeof_unqual", "uint16_t", "uint32_t", "uint64_t",
    "uint8_t", "uintptr_t", "union", "unsigned", "using", "va_arg", "virtual", "void", "volatile", "while", "xor",
    "xor_eq",
};

/// The number of words in `GRAMMAR_KEYWORDS`.
#define GRAMMAR_KEYWORD_COUNT (sizeof GRAMMAR_KEYWORDS / sizeof GRAMMAR_KEYWORDS[0])

/// Compare a word with a keyword, for `bsearch`.
static int compare_keyword(const void *word, const void *keyword) {
    return strcmp((const char *)word, *(const char *const *)keyword);
}

/// True if a word that `read_word` read is in `GRAMMAR_KEYWORDS`. O(log n) in the number of keywords.
static bool is_grammar_keyword(const char *word) {
    return bsearch(word, GRAMMAR_KEYWORDS, GRAMMAR_KEYWORD_COUNT, sizeof GRAMMAR_KEYWORDS[0], compare_keyword) != NULL;
}

/// The keywords of `_constructor_specifiers` in the grammar, which can come between the macros before
/// a constructor.
static const char *const CONSTRUCTOR_SPECIFIER_WORDS[] = {
    "extern",           "static",     "register",   "inline",      "__inline",      "__inline__", "__forceinline",
    "thread_local",     "__thread",   "const",      "constexpr",   "volatile",      "restrict",   "__restrict__",
    "__extension__",    "_Atomic",    "_Noreturn",  "noreturn",    "_Nonnull",      "mutable",    "constinit",
    "consteval",        "_Nullable",  "_Null_unspecified",         "__nullable",    "__nonnull",  "_Complex",
    "__complex__",      "__restrict", "__const",    "__const__",   "__volatile",    "__volatile__",
    "_Nullable_result", "virtual",    "__host__",   "__device__",  "__global__",    "explicit",   "alignas",
    "_Alignas",         "__attribute__",            "__attribute", "__declspec",    NULL,
};

/// The specifier keywords that have an argument list: `explicit(true)`, `alignas(8)`, `__attribute__((x))`.
static const char *const PARENTHESIZED_SPECIFIER_WORDS[] = {
    "explicit", "alignas", "_Alignas", "__attribute__", "__attribute", "__declspec", NULL,
};

/// The reserved words of C++ ([lex.key]) and the alternative tokens ([lex.digraph]). A class head has
/// none of them. A word with a special meaning in some places, as `final` or `module`, can be a name.
static const char *const RESERVED_WORDS[] = {
    "alignas",   "alignof",      "and",          "and_eq",        "asm",          "auto",       "bitand",
    "bitor",     "bool",         "break",        "case",          "catch",        "char",       "char8_t",
    "char16_t",  "char32_t",     "class",        "compl",         "concept",      "const",      "const_cast",
    "consteval", "constexpr",    "constinit",    "continue",      "co_await",     "co_return",  "co_yield",
    "decltype",  "default",      "delete",       "do",            "double",       "dynamic_cast", "else",
    "enum",      "explicit",     "export",       "extern",        "false",        "float",      "for",
    "friend",    "goto",         "if",           "inline",        "int",          "long",       "mutable",
    "namespace", "new",          "noexcept",     "not",           "not_eq",       "nullptr",    "operator",
    "or",        "or_eq",        "private",      "protected",     "public",       "register",   "reinterpret_cast",
    "requires",  "return",       "short",        "signed",        "sizeof",       "static",     "static_assert",
    "static_cast", "struct",     "switch",       "template",      "this",         "thread_local", "throw",
    "true",      "try",          "typedef",      "typeid",        "typename",     "union",      "unsigned",
    "using",     "virtual",      "void",         "volatile",      "wchar_t",      "while",      "xor",
    "xor_eq",    NULL,
};

/// The words that can come after the name of a class in its head, before the base clause or the body.
static const char *const CLASS_VIRT_SPECIFIER_WORDS[] = {
    "final", "sealed", "abstract", "trivially_relocatable_if_eligible", "replaceable_if_eligible", NULL,
};

/// True if the scanner recorded a class head with the name of the hash. With no scanner, false. O(n) in
/// MAX_CLASSES.
static bool is_class_name(const Scanner *scanner, uint32_t name) {
    if (scanner == NULL) {
        return false;
    }
    for (unsigned i = 0; i < scanner->class_count; i++) {
        if (scanner->classes[i] == name) {
            return true;
        }
    }
    return false;
}

/// Record a name in a record of MAX_CLASSES names as the most recent name. Return false if it
/// already is the most recent name. O(n) in MAX_CLASSES.
///
/// The record keeps the last MAX_CLASSES distinct names and gives up the oldest. A name that it
/// holds already moves to the most recent end rather than taking a second place.
static bool record_name(uint32_t *names, uint8_t *count, uint32_t name) {
    if (*count > 0 && names[*count - 1] == name) {
        return false;
    }
    unsigned kept = 0;
    for (unsigned i = 0; i < *count; i++) {
        if (names[i] != name) {
            names[kept++] = names[i];
        }
    }
    if (kept == MAX_CLASSES) {
        memmove(names, &names[1], (MAX_CLASSES - 1) * sizeof(uint32_t));
        kept--;
    }
    names[kept++] = name;
    *count = (uint8_t)kept;
    return true;
}

/// Record the name of a class head as the most recent name. Return false if it already is the most
/// recent name. O(n) in MAX_CLASSES.
static bool record_class_name(Scanner *scanner, uint32_t name) {
    return record_name(scanner->classes, &scanner->class_count, name);
}

/// Record the name of a template type parameter as the most recent name. Return false if it already
/// is the most recent name.
static bool record_template_name(Scanner *scanner, uint32_t name) {
    return record_name(scanner->templates, &scanner->template_count, name);
}

/// True when a template head recorded the name as a TYPE parameter. O(n) in MAX_CLASSES.
static bool is_template_parameter(const Scanner *scanner, uint32_t name) {
    if (scanner == NULL) {
        return false;
    }
    for (unsigned i = 0; i < scanner->template_count; i++) {
        if (scanner->templates[i] == name) {
            return true;
        }
    }
    return false;
}

/// MEASUREMENT OF TASK 240. True if the second record holds the name. O(n) in MAX_CLASSES.
static bool is_loose_name(const Scanner *scanner, uint32_t name) {
    if (scanner == NULL) {
        return false;
    }
    for (unsigned i = 0; i < scanner->loose_count; i++) {
        if (scanner->loose[i] == name) {
            return true;
        }
    }
    return false;
}

/// Give the kinds of `name` in `seed`, or 0 when the seed holds no such name. `length` is the bytes
/// of `name`, with no NUL.
///
/// A BINARY SEARCH OVER THE BYTES OF THE NAME. The entries ascend in the order of `memcmp` and the
/// length is the tie break, which src/seed.h declares and `cargo xtask seed collect` writes. There
/// is no hash of the name. A hash makes two names one entry, and the reading that such a collision
/// gives has no ERROR node and no differ row. O(log n) compares of O(`length`) each.
///
/// The bounds check reads a seed that a caller built by hand and that names bytes outside its own
/// text block. The runtime never reads the target, so this scan is the only reader of those bytes.
static uint16_t seed_kinds(const TSCppSeed *seed, const char *name, uint32_t length) {
    if (seed == NULL || seed->magic != TS_CPP_SEED_MAGIC || seed->version != TS_CPP_SEED_VERSION) {
        return 0;
    }
    uint32_t low = 0;
    uint32_t high = seed->count;
    while (low < high) {
        uint32_t middle = low + (high - low) / 2;
        const TSCppSeedEntry *entry = &seed->entries[middle];
        if (entry->offset > seed->text_size || entry->length > seed->text_size - entry->offset) {
            return 0;
        }
        uint32_t shortest = entry->length < length ? entry->length : length;
        int order = memcmp(seed->text + entry->offset, name, shortest);
        if (order == 0) {
            if (entry->length == length) {
                return entry->kinds;
            }
            order = entry->length < length ? -1 : 1;
        }
        if (order < 0) {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    return 0;
}

/// True when a source of the parse declares the operand of an `alignas` as a type.
///
/// ONE LOOKUP WITH SEVERAL SOURCES AND A STATED ORDER OF AUTHORITY, and not several call sites that
/// can disagree. The construct, the file and the project are three sources of ONE fact, and a
/// disagreement between them is where a silent wrong tree would live.
///
///   1. THE CONSTRUCT. A name that a template head of the same declaration declares as a type
///      parameter IS a type, by the grammar. No record and no artifact can go stale under it.
///   2. THE FILE. The class heads, the aliases and the typedefs that the scanner recorded.
///   3. THE PROJECT. The seed of the parse.
///
/// THE ORDER IS MEASURED AND NOT ASSUMED, over the 1,173 sites of the corpus whose operand is a bare
/// name. The construct disagrees with the file at 10 sites and with the project at 3, and THE
/// CONSTRUCT IS RIGHT AT EVERY ONE: in llvm-project PointerIntPair.h the name `Ptr` is the type
/// parameter of the enclosing template and also a function parameter of a different class in the
/// same file. THE FILE AND THE PROJECT NEVER DISAGREE, 0 sites of 1,173 in either direction, so the
/// second half of the order is stated here and no site of the corpus exercises it.
///
/// Task 291 adds the sources one step at a time, and each step states its own count of tree changes.
static bool is_alignas_type_name(const Scanner *scanner, const Reader *reader) {
    if (scanner == NULL) {
        return false;
    }
    // 1. THE CONSTRUCT, task 291 step 2a. 200 sites in 150 files.
    if (is_template_parameter(scanner, reader->word_hash)) {
        return true;
    }
    // 2. THE FILE, task 291 step 2b. The class heads that `scan_class_head` recorded, which is what
    //    the functional cast reads at its own position.
    //
    //    THIS READS CLASS HEADS AND NOTHING ELSE, which is what the record holds. Of the 217 sites
    //    that the file decides and the construct does not, a class head declares 119 and the ring
    //    reaches 111 of those. The other 98 rest on a `using` alias or a typedef, and the scanner
    //    records neither. A record of those names is a separate step with a count of its own.
    return is_class_name(scanner, reader->word_hash) || is_loose_name(scanner, reader->word_hash);
}

/// True when the seed of the parse names `name` as a type or as a template.
///
/// A CUT WORD GIVES FALSE, AND THAT GUARD IS REQUIRED. `name` holds the first
/// `TS_CPP_SEED_WORD_SIZE - 1` bytes of a longer word, and a compare would then take a seed entry
/// that is only a prefix of the word in the text. A name list needs no such guard, because a cut
/// word is longer than every entry of every list and can equal none of them. A seed is an open set
/// whose entries reach exactly the length of the buffer, so the inequality that protects a list
/// becomes an equality here. The corpus holds a type name of 260 bytes and two of 244 and 243, and
/// each of them would take a seed entry that shares its first 64 bytes.
///
/// A NAME THAT IS NOT ASCII GIVES FALSE AND THAT IS CORRECT. `read_word_sized` writes one `?` for
/// each character that is not ASCII, and the seed holds the bytes of the name, so the compare gives
/// a miss. No C++ identifier holds a `?`, so no entry can be reached by the replacement. A miss
/// loses a repair and accepts nothing.
static bool is_seed_type_name(const Scanner *scanner, const char *name, const Reader *reader) {
    if (scanner == NULL || reader->word_cut) {
        return false;
    }
    uint16_t kinds = seed_kinds((const TSCppSeed *)scanner->context, name, (uint32_t)strlen(name));
    return (kinds & (TS_CPP_SEED_TYPE | TS_CPP_SEED_TEMPLATE)) != 0;
}

/// MEASUREMENT OF TASK 240. Record a name in the second record. Return false if it already is the most
/// recent name. O(n) in MAX_CLASSES.
static bool record_loose_name(Scanner *scanner, uint32_t name) {
    if (scanner->loose_count > 0 && scanner->loose[scanner->loose_count - 1] == name) {
        return false;
    }
    unsigned kept = 0;
    for (unsigned i = 0; i < scanner->loose_count; i++) {
        if (scanner->loose[i] != name) {
            scanner->loose[kept++] = scanner->loose[i];
        }
    }
    if (kept == MAX_CLASSES) {
        memmove(scanner->loose, &scanner->loose[1], (MAX_CLASSES - 1) * sizeof(uint32_t));
        kept--;
    }
    scanner->loose[kept++] = name;
    scanner->loose_count = (uint8_t)kept;
    return true;
}

/// Read a template argument list from its `<`, and the gap after it. Return false if the list does not
/// end before a `;`, `{`, or `}`.
static bool skip_template_arguments(Reader *reader, Gap *gap) {
    if (!skip_angles(reader)) {
        return false;
    }
    *gap = (Gap){0};
    skip_gap(reader, gap);
    return !gap->blocked;
}

/// The words after a parameter list that a constructor cannot have: the cv-qualifiers of a member
/// function and the virt-specifiers ([class.ctor.general], [class.virtual]).
static const char *const NOT_CONSTRUCTOR_SUFFIX_WORDS[] = {"const", "volatile", "override", "final", NULL};

/// Read the rest of a parameter list from a place at `depth` in its parentheses, and the start of the
/// token after the list. Return false if the token is a word of `NOT_CONSTRUCTOR_SUFFIX_WORDS` or the
/// `&` of a ref-qualifier: `MemoryType MemType() const`. A `->` can start the type of a deduction guide:
/// `KOKKOS_DEDUCTION_GUIDE Foo(T) -> Foo<T>;`. O(n) in the length of the list.
static bool scan_constructor_parameters_end(Reader *reader, unsigned depth) {
    TSLexer *lexer = reader->lexer;
    char word[MACRO_WORD_SIZE];
    while (depth > 0) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        TokenClass token = read_token(reader, word);
        if (token == TOKEN_OPEN || token == TOKEN_CLOSE) {
            depth = token == TOKEN_OPEN ? depth + 1 : depth - 1;
            step(reader);
        }
    }
    Gap gap = {0};
    skip_gap(reader, &gap);
    int32_t c = lexer->lookahead;
    if (gap.blocked) {
        return true;
    }
    if (c == '&') {
        return false;
    }
    if (!is_word_start(c)) {
        return true;
    }
    bool has_lower = false;
    read_word(reader, word, &has_lower);
    return !word_in(word, NOT_CONSTRUCTOR_SUFFIX_WORDS);
}

/// Read the parameters of a constructor from the `(` after its name. Return true if they can be the
/// parameters of a constructor, as GCC `cp_parser_constructor_declarator_p` and Clang
/// `isConstructorDeclarator` decide without name lookup.
///
/// An empty list, `...`, an attribute, a keyword of a type, or a name that starts a parameter makes a
/// constructor. A `*`, `&`, or `(` after the `(` starts a parenthesized declarator: `A (*f)();`. A name
/// before `(` or `[` is also a declarator: `A (f)(int);`, `A (a)[4];`. A qualifier after the list makes
/// a member function (`scan_constructor_parameters_end`).
static bool scan_constructor_parameters(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    step(reader);
    Gap gap = {0};
    skip_gap(reader, &gap);
    int32_t c = lexer->lookahead;
    if (gap.blocked || !readable(reader)) {
        return false;
    }
    if (c == ')' || c == '.' || c == ':') {
        return scan_constructor_parameters_end(reader, 1);
    }
    if (c == '[') {
        step(reader);
        Gap brackets = {0};
        skip_gap(reader, &brackets);
        return !brackets.blocked && lexer->lookahead == '[' && scan_constructor_parameters_end(reader, 2);
    }
    if (!is_word_start(c)) {
        return false;
    }
    char word[MACRO_WORD_SIZE];
    bool has_lower = false;
    read_word(reader, word, &has_lower);
    if (word_in(word, NOT_PARAMETER_WORDS)) {
        return false;
    }
    if (is_grammar_keyword(word)) {
        return scan_constructor_parameters_end(reader, 1);
    }
    for (;;) {
        LOOP_STEP();
        gap = (Gap){0};
        skip_gap(reader, &gap);
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        c = lexer->lookahead;
        if (c == '<') {
            if (!skip_angles(reader)) {
                return false;
            }
            continue;
        }
        if (c != ':') {
            break;
        }
        step(reader);
        if (lexer->lookahead != ':') {
            return scan_constructor_parameters_end(reader, 1);
        }
        step(reader);
        gap = (Gap){0};
        skip_gap(reader, &gap);
        if (!is_word_start(lexer->lookahead)) {
            return false;
        }
        read_word(reader, word, &has_lower);
    }
    if (c == '(' || c == '[') {
        return false;
    }
    if (c == ')') {
        step(reader);
        gap = (Gap){0};
        skip_gap(reader, &gap);
        return lexer->lookahead != '(' && lexer->lookahead != '[' && scan_constructor_parameters_end(reader, 0);
    }
    return scan_constructor_parameters_end(reader, 1);
}

/// Read a qualified name from the `::` after its first component, whose hash is `previous`. Return
/// true if the name is `A::A` or `A::~A` before `(`: the name of a constructor or a destructor.
static bool scan_qualified_constructor_name(Reader *reader, uint32_t previous) {
    TSLexer *lexer = reader->lexer;
    char word[MACRO_WORD_SIZE];
    bool has_lower = false;
    for (;;) {
        LOOP_STEP();
        step(reader);
        if (lexer->lookahead != ':') {
            return false;
        }
        step(reader);
        Gap gap = {0};
        skip_gap(reader, &gap);
        bool destructor = !gap.blocked && lexer->lookahead == '~';
        if (destructor) {
            step(reader);
            skip_gap(reader, &gap);
        }
        if (gap.blocked || !readable(reader) || !is_word_start(lexer->lookahead)) {
            return false;
        }
        read_word(reader, word, &has_lower);
        uint32_t name = reader->word_hash;
        gap = (Gap){0};
        skip_gap(reader, &gap);
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        if (lexer->lookahead == '(') {
            return name == previous;
        }
        if (destructor || (lexer->lookahead == '<' && !skip_template_arguments(reader, &gap)) ||
            lexer->lookahead != ':') {
            return false;
        }
        previous = name;
    }
}

/// The declaration that comes after the arguments of a macro call.
typedef enum {
    /// No declaration.
    AFTER_CALL_NONE,
    /// Specifiers or a type, and a declarator.
    AFTER_CALL_DECLARATION,
    /// The name of a constructor or a destructor.
    AFTER_CALL_CONSTRUCTOR,
    /// A declarator with no type before it. The macro gives the type.
    AFTER_CALL_TYPE,
    /// The same, with a pointer or a reference operator between the macro and the declarator:
    /// `STACK_OF(X509) **sk;`. The text is also the product of a call and a name, `MAX(a, b) * c;`,
    /// and the macro is no attribute of a statement there.
    AFTER_CALL_TYPE_POINTER,
    /// The keyword of a statement. The macro is an attribute of that statement.
    AFTER_CALL_STATEMENT,
    /// An expression statement, which no declaration can be: `MACRO(a, b) find(1);`. The macro is an
    /// attribute of that statement, and it gives no type.
    AFTER_CALL_EXPRESSION,
    /// One name and a `;` or an assignment. The macro gives the type of a declaration, or it is an
    /// attribute of an expression statement. `args.one_type_id` tells the two apart.
    AFTER_CALL_TYPE_OR_STATEMENT,
    /// More macro invocations and the end of the line. The call is a macro invocation.
    AFTER_CALL_MACRO_LINE,
} AfterCall;

/// The keywords that start the type of a declaration, or a declaration of a different kind, after the
/// specifiers: the fundamental types, the size keywords, the class keys, and the words that start a
/// placeholder type, a friend, or a conversion function. A typedef has no attribute macros in the
/// grammar.
static const char *const DECLARATION_START_WORDS[] = {
    "void",      "bool",      "char",     "int",      "float",       "double",    "short",     "long",
    "signed",    "unsigned",  "size_t",   "ssize_t",  "ptrdiff_t",   "intptr_t",  "uintptr_t", "charptr_t",
    "nullptr_t", "max_align_t", "int8_t", "int16_t",  "int32_t",     "int64_t",   "uint8_t",   "uint16_t",
    "uint32_t",  "uint64_t",  "char8_t",  "char16_t", "char32_t",    "char64_t",  "auto",      "decltype",
    "typename",  "struct",    "class",    "union",    "__interface", "enum",      "friend",    "operator",
    NULL,
};

/// The keywords that start a statement and can come after the attributes of that statement. A
/// declaration never starts with one of them.
///
/// [stmt.pre] puts an attribute-specifier-seq before each statement, and a selection statement and an
/// iteration statement take one: `[[likely]] if (x) {}`. GCC `cp_parser_statement` and Clang
/// `ParseStatementOrDeclarationAfterAttributes` read the attributes before the keyword.
static const char *const STATEMENT_ATTRIBUTE_WORDS[] = {
    "return", "co_return", "co_yield", "break", "continue", "goto",  "throw",
    "delete", "if",        "for",      "while", "switch",   "do",    "try",    NULL,
};

/// Go past the qualifiers, the groups, and the macros after the parameter list of a function
/// declarator, to the token that ends the declarator: `f() const`, `f() NOEXCEPT`, `f() noexcept(B)`.
/// Return false at the end of the budget or at a token that cannot start a line.
/// O(n) in the length of the text that it reads.
static bool scan_declarator_suffix(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    char word[MACRO_WORD_SIZE];
    bool has_lower = false;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        if (lexer->lookahead == '(') {
            Arguments group = {0};
            if (!skip_group(reader, &group)) {
                return false;
            }
            continue;
        }
        if (!is_word_start(lexer->lookahead)) {
            return true;
        }
        read_word(reader, word, &has_lower);
    }
}

/// True if the token at the scan ends the declarator of a declaration whose type comes from a macro
/// call. A `=` that starts an initializer moves the scan past it.
///
/// Two names in sequence are never an expression, and each token that ends a declarator then also ends
/// the declaration: `void f(BOOST_RV_REF(T) value, int n)`. After a pointer or reference operator the
/// text can also be a product or a bit operation, and a `)` or a `[` ends such an expression:
/// `if (MASK(x) & FLAG)`, `if (OK(x) && a[0])`. Only a parameter takes a `)` after such an operator:
/// `f(STACK_OF(X509) *sk)`. A `==` is a comparison, and a `=` starts an initializer.
///
/// A `>` ends the last parameter of a template parameter list:
/// `template <BOOST_ASIO_COMPLETION_TOKEN_FOR(Signatures...) CompletionToken>` of
/// boost/libs/asio/include/boost/asio/deferred.hpp:111. A `>>` ends two such lists, and a `>=` is a
/// comparison. Only a parameter takes that token, and a declaration keeps `MASK(x) y >= n`.
static bool ends_macro_type_declarator(Reader *reader, bool pointer, bool parameter) {
    TSLexer *lexer = reader->lexer;
    int32_t c = lexer->lookahead;
    if (c == ';' || c == ',' || c == '{') {
        return true;
    }
    if (c == '=') {
        step(reader);
        return lexer->lookahead != '=';
    }
    if (c == '>' && parameter) {
        step(reader);
        return lexer->lookahead != '=';
    }
    if (c == ')') {
        return !pointer || parameter;
    }
    return c == '[' && !pointer;
}

/// Read the text after the arguments of a macro call, and tell if a declaration comes after the call.
/// The scan starts at the token after the arguments. `classes` holds the recorded class names, or it is
/// NULL where a member cannot start. `sal` tells if the macro is a SAL annotation (`is_sal_name`).
///
/// Attributes, specifiers, and more macro calls can come first. A keyword of
/// `DECLARATION_START_WORDS`, or two names in sequence, as in `T x`, start a declaration. A name before
/// `*`, `&`, or a calling convention is the type of a declarator. A recorded class name before a parameter
/// list, `~` and a recorded class name, `A::A`, and `A::~A` start a constructor or a destructor.
/// After a type, a macro name before `(` or `{` is not a declarator: `STRUCT(8) Block FINAL_CLASS {`.
/// O(n) in the length of the text that it reads.
///
/// With no type after the call, a declarator gives `AFTER_CALL_TYPE`, and the macro gives the type:
/// pointer and reference operators and a name, as in `STACK_OF(X509) *sk;`, a `...` and a name, as in
/// `BOOST_FWD_REF(Args) ...args`, or one name and a parameter list, as in
/// `NS_IMETHOD_(void) Unlink(void *p);`. `ends_macro_type_declarator` gives the tokens that end such a
/// declarator, and `parameter` tells if the declaration can be a parameter.
///
/// Each token up to the decision is on the line of the call. After a SAL annotation, the declaration can
/// start on the next line, and one name before `,` or `)` is the type of a parameter with no name:
/// `_In_reads_(n) T)`. A SAL annotation is never a statement macro.
static AfterCall scan_after_macro_call(Reader *reader, const Scanner *classes, bool sal, bool next_line,
                                       bool parameter, bool macro_line) {
    TSLexer *lexer = reader->lexer;
    char word[MACRO_WORD_SIZE];
    bool has_lower = false;
    // A name that can be a type came before the current token, and that name has the shape of a macro.
    bool type = false;
    bool macro_type = false;
    // A pointer or reference operator came after the arguments, before the name of the declarator.
    bool pointer = false;
    // More macro invocations came after the call: `P_(LINE) P(X)`.
    bool chain = false;
    // The chain of macro invocations went past a line break, and it continues on a line of its own:
    // `P_(LINE) P_(STR)` and `P_(EXP_GT) P_(EXP_GE)` in the test drivers of bde. Only a macro line can
    // come out of the scan after that, because a declaration on a line of its own takes the names
    // before it as its attribute macros.
    bool crossed_line = false;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        // A line that holds only macro invocations ends at a `}`, at a `;`, or at the end of the input.
        // Each name of the line is a macro invocation. The first name of this scan has arguments, and
        // `MYTYPE OBJ(ARG);` is no such line, because its first name has none. A line break does not end
        // such a line, because a declaration on the next line takes the names as its attribute macros:
        // `ATTR_WARN_UNUSED_RESULT ATTR_NONNULL(1, 2)` and `bool g(int a, int b);`.
        if (chain && !gap.blocked && (!readable(reader) || lexer->lookahead == '}' || lexer->lookahead == ';')) {
            return AFTER_CALL_MACRO_LINE;
        }
        if (gap.blocked || !readable(reader)) {
            return AFTER_CALL_NONE;
        }
        if (gap.newlines > 0 && !next_line) {
            // A chain of macro invocations continues on the next line. Each name of that line is a macro
            // invocation with arguments, and no other text comes before the end of the chain.
            if (!chain || !is_word_start(lexer->lookahead)) {
                return AFTER_CALL_NONE;
            }
            crossed_line = true;
        }
        int32_t c = lexer->lookahead;
        if (c == '[') {
            step(reader);
            skip_gap(reader, &gap);
            Arguments attribute = {0};
            if (gap.blocked || lexer->lookahead != '[' || !skip_group(reader, &attribute)) {
                return AFTER_CALL_NONE;
            }
            skip_gap(reader, &gap);
            if (gap.blocked || lexer->lookahead != ']') {
                return AFTER_CALL_NONE;
            }
            step(reader);
            continue;
        }
        if (c == '~') {
            step(reader);
            skip_gap(reader, &gap);
            if (gap.blocked || (gap.newlines > 0 && !next_line) || !is_word_start(lexer->lookahead)) {
                return AFTER_CALL_NONE;
            }
            read_word(reader, word, &has_lower);
            bool name = is_class_name(classes, reader->word_hash);
            skip_gap(reader, &gap);
            return name && !gap.blocked && lexer->lookahead == '(' ? AFTER_CALL_CONSTRUCTOR : AFTER_CALL_NONE;
        }
        if (!is_word_start(c)) {
            if (c == '*' || c == '&') {
                if (type) {
                    return AFTER_CALL_DECLARATION;
                }
                // A pointer or reference declarator after a macro that gives the type:
                // `STACK_OF(X509) **sk;`. The name of the declarator comes after the operators, and
                // `SIZE(x) * 2;` keeps its expression.
                pointer = true;
                while (readable(reader) && (lexer->lookahead == '*' || lexer->lookahead == '&')) {
                    LOOP_STEP();
                    step(reader);
                    skip_gap(reader, &gap);
                    if (gap.blocked || gap.newlines > 0) {
                        return AFTER_CALL_NONE;
                    }
                }
                continue;
            }
            // The `...` of a pack after a macro that gives the type: `BOOST_FWD_REF(Args) ...args`.
            if (type || pointer || c != '.') {
                return AFTER_CALL_NONE;
            }
            for (unsigned dot = 0; dot < 3; ++dot) {
                if (lexer->lookahead != '.') {
                    return AFTER_CALL_NONE;
                }
                step(reader);
            }
            skip_gap(reader, &gap);
            if (gap.blocked || gap.newlines > 0) {
                return AFTER_CALL_NONE;
            }
            c = lexer->lookahead;
            if (c == ')' || c == ',') {
                return AFTER_CALL_TYPE;
            }
            if (!is_word_start(c)) {
                return AFTER_CALL_NONE;
            }
        }
        has_lower = false;
        read_word(reader, word, &has_lower);
        uint32_t name = reader->word_hash;
        size_t length = strlen(word);
        // Only the name of a macro continues a chain that went past a line break. Each other word
        // there starts a declaration, and the names before it are its attribute macros:
        // `ATTR_WARN_UNUSED_RESULT ATTR_NONNULL(1, 2)` and `bool g(int a, int b);` on the next line.
        if (crossed_line && !is_macro_name(word, has_lower)) {
            return AFTER_CALL_NONE;
        }
        // The keyword of a statement starts no declaration, and the macro is an attribute of that
        // statement: `MACRO(x) return g(a);`.
        if (word_in(word, STATEMENT_ATTRIBUTE_WORDS)) {
            return AFTER_CALL_STATEMENT;
        }
        if (word_in(word, CONSTRUCTOR_SPECIFIER_WORDS)) {
            skip_gap(reader, &gap);
            Arguments arguments = {0};
            if (gap.directive || (gap.newlines > 0 && !next_line) ||
                (lexer->lookahead == '(' && word_in(word, PARENTHESIZED_SPECIFIER_WORDS) &&
                 !skip_group(reader, &arguments))) {
                return AFTER_CALL_NONE;
            }
            continue;
        }
        // `typedef` is a specifier of a declaration, and a macro before it is an attribute of that
        // typedef: `DEPRECATED("m") typedef int T;`. The word stays out of DECLARATION_START_WORDS,
        // because the scan of a bare macro reads that list as the names of the types of a
        // declaration (`name_of_a_type`), and `typedef` names no type.
        if (word_in(word, DECLARATION_START_WORDS) || strcmp(word, "typedef") == 0) {
            return AFTER_CALL_DECLARATION;
        }
        // A calling convention comes between the type and the declarator: `BOOL __stdcall f(`.
        if (word_in(word, CALL_MODIFIERS)) {
            if (type) {
                return AFTER_CALL_DECLARATION;
            }
            continue;
        }
        if (is_grammar_keyword(word)) {
            return AFTER_CALL_NONE;
        }
        if (pointer) {
            // After a pointer or reference operator the declarator is a name, which can have a scope,
            // and an optional parameter list. A template argument list after the name makes an
            // expression: `while (OK(e) && pos < s->length())`. A parameter list with one bare name is
            // a call: `for (; OK(s) && next(alias); )`, and not `STACK_OF(X)* f(const SSL *s)`.
            for (;;) {
                LOOP_STEP();
                gap = (Gap){0};
                skip_gap(reader, &gap);
                if (gap.blocked || gap.newlines > 0 || !readable(reader)) {
                    return AFTER_CALL_NONE;
                }
                if (lexer->lookahead != ':') {
                    break;
                }
                step(reader);
                if (lexer->lookahead != ':') {
                    return AFTER_CALL_NONE;
                }
                step(reader);
                gap = (Gap){0};
                skip_gap(reader, &gap);
                if (gap.blocked || gap.newlines > 0 || !is_word_start(lexer->lookahead)) {
                    return AFTER_CALL_NONE;
                }
                read_word(reader, word, &has_lower);
            }
            if (lexer->lookahead == '(') {
                Arguments parameters = {0};
                if (!skip_group(reader, &parameters) || parameters.not_parameters || parameters.bare_name) {
                    return AFTER_CALL_NONE;
                }
                if (!scan_declarator_suffix(reader)) {
                    return AFTER_CALL_NONE;
                }
            }
            return ends_macro_type_declarator(reader, true, parameter) ? AFTER_CALL_TYPE_POINTER : AFTER_CALL_NONE;
        }
        // The rest of a qualified name, and its template arguments.
        uint32_t previous = 0;
        for (;;) {
            LOOP_STEP();
            gap = (Gap){0};
            skip_gap(reader, &gap);
            if (gap.blocked || (gap.newlines > 0 && !next_line) || !readable(reader)) {
                return AFTER_CALL_NONE;
            }
            if (lexer->lookahead == '<') {
                if (!skip_angles(reader)) {
                    return AFTER_CALL_NONE;
                }
                continue;
            }
            if (lexer->lookahead != ':') {
                break;
            }
            step(reader);
            if (lexer->lookahead != ':') {
                return type ? AFTER_CALL_DECLARATION : AFTER_CALL_NONE;
            }
            step(reader);
            skip_gap(reader, &gap);
            bool destructor = !gap.blocked && lexer->lookahead == '~';
            if (destructor) {
                step(reader);
                skip_gap(reader, &gap);
            }
            if (gap.blocked || (gap.newlines > 0 && !next_line) || !is_word_start(lexer->lookahead)) {
                return AFTER_CALL_NONE;
            }
            previous = name;
            read_word(reader, word, &has_lower);
            name = reader->word_hash;
            if (destructor) {
                skip_gap(reader, &gap);
                bool call = !gap.blocked && lexer->lookahead == '(';
                return call && name == previous ? AFTER_CALL_CONSTRUCTOR : AFTER_CALL_NONE;
            }
        }
        c = lexer->lookahead;
        // A member access after the name is no part of a declarator, and the text is an expression
        // statement: `TEST_CYCLE() dst.setTo(val);` in opencv, `SkDEBUGCODE(bool found =)
        // fCache.find(key, nullptr);` in skia. [dcl.decl] gives a declarator no `.` and no `->`.
        if (!type && !pointer && (c == '.' || c == '-')) {
            if (c == '-') {
                step(reader);
                if (lexer->lookahead != '>') {
                    return AFTER_CALL_NONE;
                }
            }
            return AFTER_CALL_EXPRESSION;
        }
        // A name of one character with arguments can be an element of a line of macro invocations:
        // `P_(LINE) P(X)` in the test drivers of bde.
        bool short_macro = macro_line && !type && c == '(' && is_macro_name(word, has_lower);
        bool macro = previous == 0 && ((is_macro_name(word, has_lower) && length >= 2) || is_sal_name(word) ||
                                       (short_macro && previous == 0));
        if (c == '(') {
            if (previous != 0 && name == previous) {
                return AFTER_CALL_CONSTRUCTOR;
            }
            if (previous == 0 && is_class_name(classes, name)) {
                return scan_constructor_parameters(reader) ? AFTER_CALL_CONSTRUCTOR : AFTER_CALL_DECLARATION;
            }
            if (!macro) {
                if (type) {
                    return AFTER_CALL_DECLARATION;
                }
                // A parameter list after one name is a function declarator, and the macro gives the
                // return type: `NS_IMETHOD_(void) Unlink(void *p) = 0;`.
                Arguments parameters = {0};
                if (!skip_group(reader, &parameters)) {
                    return AFTER_CALL_NONE;
                }
                if (parameters.not_parameters) {
                    // The group cannot be a parameter list, so the name and the group are a call, and
                    // a `;` after them ends an expression statement: `MACRO(a, b) find(1);`.
                    gap = (Gap){0};
                    skip_gap(reader, &gap);
                    return !gap.blocked && lexer->lookahead == ';' ? AFTER_CALL_EXPRESSION : AFTER_CALL_NONE;
                }
                if (!scan_declarator_suffix(reader)) {
                    return AFTER_CALL_NONE;
                }
                return ends_macro_type_declarator(reader, false, parameter) ? AFTER_CALL_TYPE : AFTER_CALL_NONE;
            }
            // A macro call after a name that is not a macro, as in `T MID() (`, is not a declarator.
            Arguments arguments = {0};
            if ((type && !macro_type) || !skip_group(reader, &arguments)) {
                return AFTER_CALL_NONE;
            }
            chain = macro_line;
            continue;
        }
        if (is_word_start(c)) {
            type = true;
            macro_type = macro;
            continue;
        }
        if (c == '*' || c == '&') {
            return AFTER_CALL_DECLARATION;
        }
        if (sal && (c == ',' || c == ')')) {
            return AFTER_CALL_DECLARATION;
        }
        if (type) {
            // A macro name before `{` after a type names a class: `STRUCT(8) Block FINAL_CLASS {`.
            return c == '{' && macro ? AFTER_CALL_NONE : AFTER_CALL_DECLARATION;
        }
        // One name after the call is the declarator, and the macro gives the type: `STACK_OF(X509) sk;`,
        // `void f(BOOST_RV_REF(T) value, int n)`.
        //
        // A `;` or an assignment after that name also ends an expression statement, and the macro can
        // be the attribute of that statement: `MACRO(a, b) type_check;`. A `{`, a `,`, a `)`, or a `[`
        // ends no statement, and the name belongs to a declaration or to the head of a macro with a
        // body: `TEST(Mutex, Bug) ABSL_NO_THREAD_SAFETY_ANALYSIS {`.
        int32_t end = lexer->lookahead;
        if (!ends_macro_type_declarator(reader, false, parameter)) {
            return AFTER_CALL_NONE;
        }
        return end == ';' || end == '=' ? AFTER_CALL_TYPE_OR_STATEMENT : AFTER_CALL_TYPE;
    }
}

/// True if a name can be the name of a constructor in a class body: a recorded class name, or with
/// `latest`, the most recent recorded class name. With no scanner, false.
static bool is_constructor_name(const Scanner *classes, bool latest, uint32_t name) {
    if (!latest) {
        return is_class_name(classes, name);
    }
    return classes != NULL && classes->class_count > 0 && classes->classes[classes->class_count - 1] == name;
}

/// Read the words after the first macro of a declaration, and return true if a constructor or a
/// destructor follows the macros. The scan starts at the first token after the macro and its
/// arguments. `classes` holds the recorded class names, or it is NULL where a member cannot start.
/// With `latest`, only the most recent name can be the name of a constructor in a class body.
///
/// Macros, attributes, and the keywords of `_constructor_specifiers` can come before the name. A macro
/// after the first macro has the shape of a macro name and more than one character: in
/// `inline F Seq(F f)`, `F` is a type. The name is a recorded class name before `(` and parameters that
/// `scan_constructor_parameters` accepts, `~` and a recorded class name before `(`, or a qualified name
/// that `scan_qualified_constructor_name` accepts. O(n) in the length of the text that it reads.
static bool scan_constructor_after_macro(Reader *reader, const Scanner *classes, bool latest, bool *statement,
                                         bool *macro_line) {
    TSLexer *lexer = reader->lexer;
    char word[MACRO_WORD_SIZE];
    // A macro of the chain has arguments. Only an attributed statement takes such a chain.
    bool arguments_in_chain = false;
    // The last macro of the chain has arguments. A line of macro invocations ends with such a macro.
    bool last_arguments = false;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        // A line that holds only macro invocations ends at a `}` or at the end of the input:
        // `T_ P_(LINE) P(X)` in the test drivers of bde. A line break does not end such a line, because a
        // declaration on the next line takes the names as its attribute macros.
        if (last_arguments && !gap.blocked && (!readable(reader) || lexer->lookahead == '}')) {
            *macro_line = true;
            return false;
        }
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        int32_t c = lexer->lookahead;
        if (c == '[') {
            Arguments attribute = {0};
            if (!skip_group(reader, &attribute)) {
                return false;
            }
            continue;
        }
        if (c == '~') {
            step(reader);
            skip_gap(reader, &gap);
            if (classes == NULL || gap.blocked || !is_word_start(lexer->lookahead)) {
                return false;
            }
            bool has_lower = false;
            read_word(reader, word, &has_lower);
            if (arguments_in_chain || !is_constructor_name(classes, latest, reader->word_hash)) {
                return false;
            }
            skip_gap(reader, &gap);
            return !gap.blocked && lexer->lookahead == '(';
        }
        if (!is_word_start(c)) {
            return false;
        }
        bool has_lower = false;
        read_word(reader, word, &has_lower);
        uint32_t name = reader->word_hash;
        // The keyword of a statement starts no declaration, and the macros before it are the
        // attributes of that statement: `MUST_TAIL_CALL return g(a);`.
        if (word_in(word, STATEMENT_ATTRIBUTE_WORDS)) {
            *statement = true;
            return false;
        }
        gap = (Gap){0};
        skip_gap(reader, &gap);
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        c = lexer->lookahead;
        if (c == '(' && is_constructor_name(classes, latest, name)) {
            return !arguments_in_chain && scan_constructor_parameters(reader);
        }
        if (word_in(word, CONSTRUCTOR_SPECIFIER_WORDS)) {
            if (c == '(' && word_in(word, PARENTHESIZED_SPECIFIER_WORDS)) {
                Arguments arguments = {0};
                if (!skip_group(reader, &arguments)) {
                    return false;
                }
            }
            continue;
        }
        if (is_grammar_keyword(word)) {
            return false;
        }
        if (c == '<') {
            if (!skip_template_arguments(reader, &gap) || lexer->lookahead != ':') {
                return false;
            }
            c = ':';
        }
        if (c == ':') {
            return !arguments_in_chain && scan_qualified_constructor_name(reader, name);
        }
        // A name of one character with arguments can be an element of a line of macro invocations.
        if (!is_macro_name(word, has_lower) || (strlen(word) < 2 && c != '(')) {
            return false;
        }
        if (c == '(') {
            // A macro with arguments continues the chain only for an attributed statement or for a line
            // of macro invocations: `SUPPRESS_A SUPPRESS_B(x) return g(a);`, `T_ P_(LINE) P(X)`.
            Arguments arguments = {0};
            if (!skip_group(reader, &arguments) || arguments.not_expressions || arguments.statements) {
                return false;
            }
            arguments_in_chain = true;
        }
        last_arguments = c == '(';
    }
}

/// The words of the attributes that can come after a class key, before the name of the class.
static const char *const CLASS_ATTRIBUTE_WORDS[] = {
    "__attribute__", "__attribute", "alignas", "_Alignas", "__declspec", NULL,
};

/// Read the rest of a class head and its body up to the first member. Return true if the body has a `;`
/// or a braced group at its top level. The scan starts after the name of the class, or in its base
/// clause.
///
/// A member declaration ends with `;`, and a member function definition has a body. An enumerator list
/// has neither: `enum class E : int { A, B };`. A braced initializer has neither in most code:
/// `struct timespec t{1, 0};`. O(n) in the length of the text that the scan reads.
static bool class_body_has_member(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    char word[MACRO_WORD_SIZE];
    bool body = false;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        int32_t c = lexer->lookahead;
        if (c == ';' || (c == '{' && body)) {
            return body;
        }
        if (c == '{') {
            step(reader);
            body = true;
            continue;
        }
        if (c == '(' || c == '[') {
            Arguments group = {0};
            if (!skip_group(reader, &group)) {
                return false;
            }
            continue;
        }
        if (c == ')' || c == ']' || c == '}') {
            return false;
        }
        read_token(reader, word);
    }
}

/// Scan the head of a class after its class key. When a base clause, a virt-specifier, or a body comes
/// after the head, and the body has a member, record the name of the class. Return true if the recorded
/// names change. The token is the empty extra CLASS_HEAD_MARK before the class key.
///
/// The name of the class is the last name before these tokens. A name can be qualified, and template
/// arguments can come after it: `ns::A<int> {`. The names and the macro calls before it are macros:
/// `class LLVM_ABI A final {`, `struct TSA_CAPABILITY("mutex") M {`. Attributes can come before the
/// first name. A head with no name, `struct {`, records no name. Other text, as in `class T>` of a
/// template parameter, or in `struct stat *p;`, records no name. The key of `enum class E {` gives the
/// same scan, and `class_body_has_member` rejects its body. O(n) in the length of the text that the scan
/// reads.
static bool scan_class_head(Scanner *scanner, Reader *reader) {
    TSLexer *lexer = reader->lexer;
    uint32_t name = 0;
    bool after_name = false;
    bool named = false;
    char word[MACRO_WORD_SIZE];
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        int32_t c = lexer->lookahead;
        if (c == '{') {
            break;
        }
        if (c == ':') {
            step(reader);
            if (lexer->lookahead != ':') {
                break;
            }
            step(reader);
            after_name = false;
            named = true;
            continue;
        }
        if (c == '[' && !named) {
            Arguments attribute = {0};
            if (!skip_group(reader, &attribute)) {
                return false;
            }
            continue;
        }
        if (c == '<' && after_name) {
            if (!skip_angles(reader)) {
                return false;
            }
            continue;
        }
        if (c == '(' && after_name) {
            Arguments arguments = {0};
            if (!skip_group(reader, &arguments)) {
                return false;
            }
            name = 0;
            after_name = false;
            continue;
        }
        if (!is_word_start(c)) {
            return false;
        }
        bool has_lower = false;
        read_word(reader, word, &has_lower);
        if (after_name && word_in(word, CLASS_VIRT_SPECIFIER_WORDS)) {
            break;
        }
        if (!named && word_in(word, CLASS_ATTRIBUTE_WORDS)) {
            skip_gap(reader, &gap);
            Arguments attribute = {0};
            if (gap.blocked || lexer->lookahead != '(' || !skip_group(reader, &attribute)) {
                return false;
            }
            continue;
        }
        if (word_in(word, RESERVED_WORDS)) {
            return false;
        }
        name = reader->word_hash;
        after_name = true;
        named = true;
    }
    if (!after_name) {
        return false;
    }
    // MEASUREMENT OF TASK 240. The second record takes the name whatever the body holds. The first
    // record keeps its condition, because the constructor rules read it.
    bool member = class_body_has_member(reader);
    if (reader->budget == 0) {
        return false;
    }
    bool changed = record_loose_name(scanner, name);
    if (member) {
        changed = record_class_name(scanner, name) || changed;
    }
    if (!changed) {
        return false;
    }
    lexer->result_symbol = CLASS_HEAD_MARK;
    return true;
}

/// Scan a template head after the word `template`, and record the names that it declares as TYPE
/// parameters. Return true if the recorded names change. The token is the empty extra
/// TEMPLATE_HEAD_MARK before the word.
///
/// THE TOKEN IS GIVEN ONLY WHEN THE RECORD CHANGES, as `scan_class_head` does. An empty token that a
/// scan gives again at the same position is a token with no width that the parser reads forever: one
/// such loop of 2026-09-16 took 152 GB for a file of 20 KB.
///
/// ONLY `typename` AND `class` GIVE A NAME. A constrained parameter, `template <std::integral T>`,
/// and a non-type parameter, `template <int N>`, have the SAME SHAPE, a name and then a name, and
/// only the declaration of the first name tells a concept from a type. This scan holds no
/// declaration. A parameter that it does not read gives no entry, which is a miss and never a wrong
/// tree. A template template parameter, `template <template <class> class C>`, is not a type
/// ([temp.param]), so it gives no entry either.
///
/// O(n) in the length of the head.
static bool scan_template_head(Scanner *scanner, Reader *reader) {
    TSLexer *lexer = reader->lexer;
    Gap gap = {0};
    skip_gap(reader, &gap);
    if (gap.blocked || !readable(reader) || lexer->lookahead != '<') {
        return false;
    }
    step(reader);
    char word[MACRO_WORD_SIZE];
    unsigned depth = 1;
    bool changed = false;
    // True at the first character of a parameter, which is where a `typename` or a `class` can be.
    bool parameter_start = true;
    while (depth > 0) {
        LOOP_STEP();
        Gap inner = {0};
        skip_gap(reader, &inner);
        if (inner.blocked || !readable(reader)) {
            return false;
        }
        int32_t c = lexer->lookahead;
        if (c == '<') {
            depth++;
            step(reader);
            parameter_start = false;
            continue;
        }
        if (c == '>') {
            depth--;
            step(reader);
            parameter_start = depth == 1;
            continue;
        }
        if (c == ',') {
            step(reader);
            parameter_start = depth == 1;
            continue;
        }
        // A default argument holds a group of its own, and a `>` inside it closes nothing.
        if (c == '(' || c == '[') {
            Arguments group = {0};
            if (!skip_group(reader, &group) || reader->budget == 0) {
                return false;
            }
            parameter_start = false;
            continue;
        }
        if (parameter_start && depth == 1 && is_word_start(c)) {
            bool has_lower = false;
            read_word(reader, word, &has_lower);
            parameter_start = false;
            if (strcmp(word, "typename") != 0 && strcmp(word, "class") != 0) {
                continue;
            }
            // A pack comes before the name, `typename... Ts`, or after the keyword with a blank.
            skip_blanks(reader);
            while (readable(reader) && lexer->lookahead == '.') {
                LOOP_STEP();
                step(reader);
            }
            Gap after = {0};
            skip_gap(reader, &after);
            if (after.blocked || !readable(reader) || !is_word_start(lexer->lookahead)) {
                continue;
            }
            bool lower = false;
            read_word(reader, word, &lower);
            if (word[0] != '\0') {
                changed = record_template_name(scanner, reader->word_hash) || changed;
            }
            continue;
        }
        step(reader);
        parameter_start = false;
    }
    if (!changed) {
        return false;
    }
    lexer->result_symbol = TEMPLATE_HEAD_MARK;
    return true;
}

/// True if the word is a class key: `class`, `struct`, `union`, or the MSVC `__interface`.
static bool is_class_key(const char *word) {
    return strcmp(word, "class") == 0 || strcmp(word, "struct") == 0 || strcmp(word, "union") == 0 ||
           strcmp(word, "__interface") == 0;
}

/// Read a word, and scan the head of a class after it when the word is a class key. The token is the
/// empty CLASS_HEAD_MARK before the word.
static bool scan_class_key(Scanner *scanner, TSLexer *lexer) {
    mark_end(lexer);
    Reader reader = start_reader(lexer, MACRO_SCAN_LIMIT, scanner);
    char word[MACRO_WORD_SIZE];
    bool has_lower = false;
    read_word(&reader, word, &has_lower);
    return is_class_key(word) && scan_class_head(scanner, &reader);
}

/// Scan a macro in the head of a class that has no member: `struct LLVM_ABI A;`,
/// `class BASE_EXPORT C {};`. The token is the empty CLASS_MACRO_MARK before the name of the macro,
/// and the parser reads it after a class key only. The scan starts after that name.
///
/// THE SAME TEXT HAS A SECOND READING. `struct stat st;` declares a variable of an elaborated type,
/// and the two readings hold the same three tokens. A member in the body ends the second reading,
/// and a head with no member gives no such evidence. Only the shape of the FIRST name separates
/// them, and `is_macro_name` reads that shape.
///
/// THE SHAPE OF THE SECOND NAME SEPARATES NOTHING. Of the 17,189 class heads in the corpus that
/// hold a macro and a member, 1,366 give the class a name that starts with a lowercase letter:
/// `class FMT_API file {`, `struct FMT_API pipe {`, `class EASTL_API fixed_allocator :`. A rule on
/// the second name looks safe and it is not.
///
/// THE STOP SET IS THE SAFETY ARGUMENT. A `,`, a `=`, a `(` after the last name, a `<`, a `:` or a
/// body with a member stops the mark, so `struct STAT st = {};` and `class A B, C;` keep the
/// variable reading. The scan gives the mark for `;` and for an empty `{}` only.
///
/// MEASUREMENT OF 329,387 FILES, 2026-09-16. The macro reading is correct in 986 forward
/// declarations of 500 files and in 70 empty bodies of 44 files. It is incorrect in approximately
/// 41 lines, where the macro gives a TYPE and the second name is a variable:
/// `struct STATFSSTRUCT stats;`, `struct STAT sbuf;`, `struct STAT statInfo {};`. The corpus test
/// "A class head with a macro and no member" pins those lines. O(n) in the length of the head.
static bool scan_class_macro_mark(Reader *reader, const char *word, bool has_lower) {
    TSLexer *lexer = reader->lexer;
    if (strlen(word) < 2 || !is_macro_name(word, has_lower) || is_grammar_keyword(word)) {
        return false;
    }
    // The names that the scan read. The last name is the name of the class, and the names before it
    // are macros, so a head needs two names.
    unsigned names = 1;
    bool after_name = true;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        int32_t c = lexer->lookahead;
        if (c == '(') {
            // The arguments of a macro call: `class TSA_CAPABILITY("mutex") M;`. A `(` after the
            // last name belongs to an initializer of a variable, and the scan stops at the token
            // that follows the group.
            Arguments arguments = {0};
            if (!after_name || !skip_group(reader, &arguments)) {
                return false;
            }
            after_name = false;
            continue;
        }
        if (c == ';') {
            return names >= 2 && after_name;
        }
        if (c == '{') {
            if (names < 2 || !after_name) {
                return false;
            }
            step(reader);
            Gap body = {0};
            skip_gap(reader, &body);
            return !body.blocked && readable(reader) && lexer->lookahead == '}';
        }
        if (!is_word_start(c)) {
            return false;
        }
        char next[MACRO_WORD_SIZE];
        bool next_lower = false;
        read_word(reader, next, &next_lower);
        // A virt-specifier and a reserved word end the head that this scan reads. The rule of a
        // class head with a body takes those forms.
        if (word_in(next, CLASS_VIRT_SPECIFIER_WORDS) || word_in(next, RESERVED_WORDS)) {
            return false;
        }
        names++;
        after_name = true;
    }
}

/// True when a string literal starts at the lookahead of the scan. The scan reads no character.
///
/// A concatenation holds a string literal beside each part, and an encoding prefix can come before
/// the quote: `L"x"`, `u8"x"`, `R"(x)"` ([lex.string] p1). The scan reads the prefix with
/// `read_identifier` into a buffer that it drops, so the lexer moves only where the answer is true.
static bool starts_string_literal(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    int32_t c = lexer->lookahead;
    if (c == '"') {
        return true;
    }
    if (c != 'L' && c != 'u' && c != 'U' && c != 'R') {
        return false;
    }
    // A prefix has at most three characters, `u8R`. The scan reads them and stops at the quote.
    char prefix[4] = {0};
    unsigned length = 0;
    while (length < 3 && is_word_char(lexer->lookahead)) {
        prefix[length++] = (char)lexer->lookahead;
        step(reader);
    }
    if (lexer->lookahead != '"') {
        return false;
    }
    static const char *const PREFIXES[] = {"L", "u", "U", "R", "u8", "LR", "uR", "UR", "u8R", NULL};
    return word_in(prefix, PREFIXES);
}

/// True when a `::` comes at the lookahead of the scan. The scan reads the two characters.
///
/// A macro call that gives a scope has a `::` after its argument list:
/// `BOOST_MPL_AUX_VALUE_WKND(N)::value` of boost/libs/mpl. THE POSITION IS THE EVIDENCE AND THE
/// SPELLING OF THE NAME PROVES NOTHING. `A(b)::c::v` with no definition of `A` gives 4 errors in GCC
/// and 4 in Clang, and 0 errors in each with `#define NS(x) A`. A `::` cannot come after an
/// expression (GCC `cp_parser_nested_name_specifier_opt`, Clang `ParseOptionalCXXScopeSpecifier`),
/// so the text has one reading.
static bool scope_follows(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    if (lexer->lookahead != ':') {
        return false;
    }
    step(reader);
    return readable(reader) && lexer->lookahead == ':';
}

/// The result of `scan_macro_invocation`.
typedef enum {
    /// The scan selected a token.
    INVOCATION_TOKEN,
    /// The scan selected no token. A constructor can still come after the macro.
    INVOCATION_NONE,
    /// The scan selected no token, and no constructor comes after the macro.
    INVOCATION_STOP,
    /// The scan selected no token, and it found a constructor or a destructor on the next line.
    INVOCATION_CONSTRUCTOR,
} Invocation;

/// Scan for the start of a macro invocation, after the name and its arguments. The token is empty, and
/// it comes before the name.
///
/// The line form is an uppercase name with 5 or more characters, or an uppercase name with an
/// argument list, before a line break and a token that can start a line. Where a member starts, a
/// name of each shape with an argument list also takes the line form and the call form, because a
/// member with no type is a constructor, and `scan_macro_start` reads the class name. A long name
/// can have its argument list on the next line. When a name with no arguments can be the return type of a
/// declarator on the next line, the line form needs a blank line or a comment between them. The
/// same rule applies to a constructor or a destructor on the next line, where a declaration can have
/// one. Nearly all such names in real code are attributes: `KOKKOS_FUNCTION` before `View(int n)`. A
/// name with the shape of a region or a pragma macro (`is_region_macro_name`) stays a macro invocation
/// before a constructor: `QT_BEGIN_NAMESPACE` before `A::A() {}`, `Q_OBJECT` before `A();`.
///
/// The line form also includes an uppercase name of each length, with or without an argument list, before a `}`
/// on the same line: `{ Q_UNUSED(v) }`, `{MONGO_UNREACHABLE}`. The expansion of such a macro is a full statement
/// or member with its `;`. No other statement and no member ends before `}` without `;`. Where a braced list can
/// also start, as in the arguments `f({ FLAG })` or on the next line, a name before `}` is an element of the list.
///
/// The block form is an uppercase name and `{` on the same line. The name has an argument list
/// that cannot be a parameter list, with an optional template parameter list and parameter list
/// after it, or the name has no arguments and the block holds statements.
///
/// The statement form is an uppercase name with an argument list and a body, where a statement can start and a
/// function definition cannot: in a block, a case body, a label, or a substatement. The arguments can be a parameter
/// list, and the `{` can come after line breaks and directive lines: `MACRO_IF(x)`, a line break, and `{`. An `else`
/// clause can come after the body. After a blank line, the name is a macro invocation line, and the `{` starts a
/// block.
///
/// The call form is an uppercase name with arguments that are not expressions, before `;`. In a
/// statement, the call form includes the `;`. In a member, the line form takes the call when its
/// arguments also cannot be a parameter list, and the `;` stays after it.
///
/// `name` is the name, and `length` is its length. `same_line` tells if no line break comes between the
/// name and its arguments. `call` tells if the name has arguments, and `args` holds their facts. `gap`
/// is the space after the name and its arguments. `classes` holds the recorded class names, or it is
/// NULL where a member cannot start.
static Invocation scan_macro_invocation(Reader *reader, const char *name, size_t length, bool same_line, bool call,
                                        const Arguments *args, const Gap *gap, const bool *valid_symbols,
                                        const Scanner *classes) {
    TSLexer *lexer = reader->lexer;
    // After a directive that ends a branch of a structured group, the lookahead is not a token of the line.
    int32_t c = gap->directive ? 0 : lexer->lookahead;
    // A storage class specifier can come before a macro invocation line, and that line has its own
    // token. After a specifier the macro takes two arguments or more, because the other shapes there
    // give a keyword or an attribute of the declaration on the next line. A bare name does this:
    // `extern BOOST_ASIO_DECL` before `const error_category& get_netdb_category();`. One argument does
    // it too: `static QT_FUNCTION_TARGET(ARCH_SKYLAKE_AVX512)` before
    // `void qFloatToFloat16_tail_avx256(...)`, and `static SWIFT_CC(swift)` before
    // `void destroyTestActor(...)`. A macro that gives a full declaration needs the name of the
    // declaration and its value: `static Q_LOGGING_CATEGORY(log, "qtc", QtWarningMsg)`.
    bool after_specifiers_line = valid_symbols[MACRO_LINE_AFTER_SPECIFIERS] && call && args->argument_count >= 2;
    bool line_start = valid_symbols[MACRO_LINE_START] || after_specifiers_line;
    TSSymbol line_symbol = valid_symbols[MACRO_LINE_START] ? MACRO_LINE_START : MACRO_LINE_AFTER_SPECIFIERS;
    if (valid_symbols[MACRO_ENUMERATOR_START]) {
        // In an enumerator list, a call before `,`, `}`, or a line break is a macro invocation. An
        // enumerator never has arguments. A name with no arguments is an enumerator.
        bool end = c == ',' || c == '}' || lexer->eof(lexer) || gap->directive ||
                   (gap->newlines > 0 && (is_word_start(c) || c == '#'));
        if (!call || !end) {
            return INVOCATION_STOP;
        }
        lexer->result_symbol = MACRO_ENUMERATOR_START;
        return INVOCATION_TOKEN;
    }

    if (call && c == ';') {
        // A statement accepts the call form. A member has no call form, and it takes the call as a
        // macro invocation when the arguments cannot be a parameter list.
        if (valid_symbols[MACRO_CALL_START]) {
            if (!args->statements && !args->not_expressions) {
                return INVOCATION_STOP;
            }
            lexer->result_symbol = MACRO_CALL_START;
            return INVOCATION_TOKEN;
        }
        if (line_start && args->not_parameters) {
            lexer->result_symbol = line_symbol;
            return INVOCATION_TOKEN;
        }
        return INVOCATION_STOP;
    }

    if (call && same_line && c == '{' && valid_symbols[MACRO_STATEMENT_START]) {
        // Where the statement token is valid, a function definition cannot start, and a body after the arguments is
        // the body of a statement macro. The body can come after line breaks. After a blank line or a directive
        // line, the name is a macro invocation line, and the `{` starts a block: `FOR_EACH_ARITH_UNARY_OP(CASE)`,
        // `#undef CASE`, and a block of the case labels of the macro.
        bool line = gap->blank_line || gap->directive_line;
        lexer->result_symbol = line ? MACRO_LINE_START : MACRO_STATEMENT_START;
        return valid_symbols[lexer->result_symbol] ? INVOCATION_TOKEN : INVOCATION_STOP;
    }

    if (same_line && valid_symbols[MACRO_BLOCK_START] && !gap->directive) {
        bool block = false;
        if (call && args->not_parameters && lexer->lookahead == '<') {
            // A generic body: `NAME(args) <typename T>(T x) {`.
            if (!skip_angles(reader)) {
                return INVOCATION_STOP;
            }
            Gap after = {0};
            skip_gap(reader, &after);
            if (lexer->lookahead == '(') {
                Arguments parameters = {0};
                if (!skip_group(reader, &parameters)) {
                    return INVOCATION_STOP;
                }
                after = (Gap){0};
                skip_gap(reader, &after);
            }
            if (after.blocked || lexer->lookahead != '{') {
                return INVOCATION_STOP;
            }
            block = true;
        } else if (call && (args->not_parameters || args->call_arguments)) {
            // A call in an argument gives a body to the macro, and no function definition starts:
            // `MATCHER_P(IsNode, height, absl::StrCat("height ", height)) {`.
            block = lexer->lookahead == '{';
        } else if (!call && lexer->lookahead == '{') {
            // A block of statements after a name: `SCOPE_EXIT { f(); };`.
            Arguments body = {0};
            if (!skip_group(reader, &body) || !body.statements) {
                return INVOCATION_STOP;
            }
            block = true;
        }
        if (block) {
            if (reader->budget == 0) {
                return INVOCATION_STOP;
            }
            lexer->result_symbol = MACRO_BLOCK_START;
            return INVOCATION_TOKEN;
        }
    }

    if (line_start && c == '}') {
        // The `{` of a GNU statement expression can also start a braced list. The list then takes the name.
        if (valid_symbols[INITIALIZER_LIST_MARKER]) {
            return INVOCATION_STOP;
        }
        if (gap->newlines == 0) {
            lexer->result_symbol = line_symbol;
            return INVOCATION_TOKEN;
        }
    }
    if (!line_start || (!call && length < MACRO_MIN_BARE_LENGTH)) {
        return INVOCATION_NONE;
    }
    if (!lexer->eof(lexer)) {
        if (gap->newlines == 0) {
            return INVOCATION_NONE;
        }
        // Only a statement accepts the call form. Its token is valid where a statement can start. The end of a
        // branch of a structured group also ends the item, as a `}` does.
        bool new_item = gap->directive_line && (!call || valid_symbols[MACRO_CALL_START]);
        NextLine next = gap->directive ? NEXT_OTHER
                                       : classify_next_line(reader, call && !args->not_parameters,
                                                            valid_symbols[MACRO_CALL_START], classes, new_item);
        // After a directive line that divides the lines, a token that starts an item and does not continue a
        // construct starts a new item, as before the directives: `FOR_EACH_OP(CASE_LABEL)`, `#undef CASE_LABEL`, `{`.
        bool item_start = c == '{' || c == '(' || c == '*' || c == '!' || c == '~' || c == '[' || c == '"' ||
                          c == '\'' || c == ';' || is_digit(c);
        if (next == NEXT_BLOCKED && (c == '+' || c == '-') && lexer->lookahead == c) {
            // `++k;` starts a statement. A different `+` or `-` continues an expression.
            step(reader);
            item_start = lexer->lookahead == c;
        }
        if (next == NEXT_BLOCKED && new_item && item_start) {
            next = NEXT_OTHER;
        }
        if (next == NEXT_BLOCKED || (reader->budget == 0 && !gap->directive)) {
            return INVOCATION_STOP;
        }
        if (next == NEXT_DECLARATOR && !call && !gap->separated) {
            return INVOCATION_STOP;
        }
        if (next == NEXT_CONSTRUCTOR && !call && !gap->separated && valid_symbols[CONSTRUCTOR_MACRO_START] &&
            !is_region_macro_name(name)) {
            return INVOCATION_CONSTRUCTOR;
        }
    }
    lexer->result_symbol = line_symbol;
    return INVOCATION_TOKEN;
}

/// Scan for the start of a macro invocation, or for the start of the macros before a constructor. The
/// token is empty, and it comes before the name.
///
/// `scan_macro_invocation` gives the forms of a macro invocation. The constructor form is a name that
/// is not a keyword, before more macros and the name of a constructor or a destructor: `LLVM_ABI A();`
/// after the class head of `A`, `simdjson_inline A::A() {}`. `scan_constructor_after_macro` reads the
/// text after the first macro. A macro invocation comes first: `Q_OBJECT` on its own line before
/// `A();` is a macro invocation when a blank line or a comment separates them.
///
/// The name of a recorded class head gives a constructor only where a statement cannot start. After
/// specifiers, a member and a namespace member are possible, and the first macro has the shape of a
/// macro name and more than one character: `inline T relaxed_epsilon(T const& factor)` is a function. At
/// the start of a member, a first macro with a different shape needs the most recent class name:
/// `simdjson_inline array();` in `class array`, but `ut Bob();` in `struct tt`. In a function body,
/// `QPoint Point(1, 2);` declares a variable.
///
/// The scan starts after the name, which `read_word` read. `reader` holds the number of characters
/// that the scan can still read, and the token ends before the name.
static bool scan_macro_start(Reader reader, const char *name, bool has_lower, const bool *valid_symbols,
                             const Scanner *scanner) {
    TSLexer *lexer = reader.lexer;
    size_t length = strlen(name);
    // TRUE, FALSE, and NULL are tokens of the grammar. Q_EMIT, Q_FOREACH, and Q_FOREVER start Qt
    // statements.
    static const char *const GRAMMAR_WORDS[] = {"TRUE", "FALSE", "NULL", "Q_EMIT", "Q_FOREACH", "Q_FOREVER", NULL};
    bool macro_name = is_macro_name(name, has_lower) && !word_in(name, GRAMMAR_WORDS);
    // A SAL annotation with arguments, `_Out_writes_(n)`, is an attribute macro, and never a statement macro.
    bool sal = !macro_name && is_sal_name(name);
    bool member_start = valid_symbols[MACRO_LINE_START] && !valid_symbols[MACRO_CALL_START];
    // A class name before a constructor is a type, not a macro.
    bool class_name = is_class_name(scanner, reader.word_hash);
    // Where a member starts, a name with an argument list and no type before it is a macro
    // invocation, and the shape of the name has no effect: `ClassDefOverride(A,0)` of ROOT. A member
    // declaration with no type is a constructor, a destructor, or a conversion function. The two
    // front ends reject each other name there (GCC `cp_parser_member_declaration`, Clang
    // `ParseCXXClassMemberDeclaration`), and they accept `class A { int x; ClassDefOverride(A,0) };`
    // only with the macro. A recorded class name is the name of a constructor: `Foo(Foo&& other)`
    // before `V8_NOEXCEPT = default;` on the next line. A name in uppercase keeps its reading of a
    // macro there, because a macro can have the name of a class: `struct D {};`, `#define D(n)`, and
    // `D(1)` in a class body (g++.dg/cpp0x/pr85462.C). A SAL annotation is an attribute of the next
    // declaration. The arguments decide the rest, as for a name in uppercase: `Foo(int x);` and
    // `Foo(x);` can hold a parameter list, and they stay declarations.
    bool member_name = member_start && !class_name && !sal && !word_in(name, GRAMMAR_WORDS) &&
                       !is_grammar_keyword(name);
    bool invocation = (macro_name || member_name) &&
                      (valid_symbols[MACRO_LINE_START] || valid_symbols[MACRO_BLOCK_START] ||
                       valid_symbols[MACRO_CALL_START] || valid_symbols[MACRO_ENUMERATOR_START] ||
                       valid_symbols[MACRO_LINE_AFTER_SPECIFIERS]);
    bool after_specifiers = !valid_symbols[MACRO_LINE_START] && !valid_symbols[MACRO_CALL_START];
    const Scanner *classes = member_start || (after_specifiers && macro_name && length >= 2) ? scanner : NULL;
    bool constructor = valid_symbols[CONSTRUCTOR_MACRO_START] && !is_grammar_keyword(name) && !class_name;
    bool macro_type = valid_symbols[MACRO_TYPE_START] || valid_symbols[PARAMETER_MACRO_TYPE_START];
    bool attribute_call =
        (valid_symbols[MACRO_CALL_ATTRIBUTE_START] || valid_symbols[MACRO_CALL_ATTRIBUTE_TOKENS_START] ||
         macro_type) &&
        ((macro_name && length >= 2) || sal);
    // A macro before a statement keyword is an attribute of that statement.
    bool statement_attribute =
        (valid_symbols[STATEMENT_ATTRIBUTE_MACRO_START] || valid_symbols[STATEMENT_ATTRIBUTE_MACRO_TOKENS_START]) &&
        macro_name && length >= 2;
    // A line that holds only macro invocations, where a statement can start: `T_ P_(LINE) P(X)`.
    bool macro_line_possible = valid_symbols[MACRO_LINE_START] && valid_symbols[MACRO_CALL_START] && macro_name;
    if (!invocation && !constructor && !attribute_call && !statement_attribute) {
        return false;
    }

    // A directive that ends a branch of a structured group ends the gap. `scan_macro_invocation` reads it as the end
    // of the item.
    Gap gap = {0};
    skip_gap(&reader, &gap);
    if (gap.directive ? false : reader.budget == 0 || gap.blocked) {
        return false;
    }
    bool same_line = gap.newlines == 0;
    Arguments args = {0};
    bool call = false;
    if ((macro_name || sal || member_name) && !gap.directive && lexer->lookahead == '(' &&
        (same_line || length >= MACRO_MIN_BARE_LENGTH)) {
        if (!skip_group(&reader, &args)) {
            return false;
        }
        call = true;
        gap = (Gap){0};
        skip_gap(&reader, &gap);
        if (gap.directive ? false : reader.budget == 0 || gap.blocked) {
            return false;
        }
    }
    // A macro call that gives a scope: `typedef BOOST_MPL_AUX_NESTED_TYPE_WKND(T)::type type;`. The
    // scan already read the group, and a `::` after it gives the scope token.
    //
    // `scope_follows` READS THE FIRST `:` AND THE LEXER CANNOT GO BACK, so a scan that reads it and
    // then declines gives no token at all. A second reading of the same text would use a reader that
    // moved. The scan takes that step only where a constructor cannot start, because the initializer
    // list of a constructor is the other construct with a `:` after a parameter list:
    // `MD5 () : OpenSSLDigest(EVP_md5()) { }` of ceph. Refer to `macro_scope_specifier`.
    if (call && valid_symbols[MACRO_SCOPE_START] && !valid_symbols[CONSTRUCTOR_MACRO_START] &&
        !gap.directive && lexer->lookahead == ':') {
        if (!scope_follows(&reader)) {
            return false;
        }
        lexer->result_symbol = MACRO_SCOPE_START;
        return true;
    }
    // A name that is a macro only by its place must have the argument list that makes it one. A group
    // with the shape of a parenthesized declarator declares a member: `Foo (*p);`, `Foo (&r);`.
    if (invocation && (macro_name || (call && !args.pointer_declarator))) {
        Invocation result =
            scan_macro_invocation(&reader, name, length, same_line, call, &args, &gap, valid_symbols, classes);
        if (result == INVOCATION_TOKEN || result == INVOCATION_STOP) {
            return result == INVOCATION_TOKEN;
        }
        if (result == INVOCATION_CONSTRUCTOR) {
            lexer->result_symbol = CONSTRUCTOR_MACRO_START;
            return constructor;
        }
    }
    if (gap.directive) {
        return false;
    }
    if (call) {
        // A call before a declaration on the same line is an attribute. A call before a constructor
        // starts the macros of the constructor. A SAL annotation can be on the line before the declaration.
        // Arguments with a statement keyword, as in `_Success_(return != 0)`, are not expressions.
        //
        // After a template head and after the specifiers of a declaration, a macro invocation line is
        // not valid, and the declaration can start on the line after the call:
        // `template <class R>`, `BSLSTL_MAP_REQUIRES_CONTAINER_COMPATIBLE_RANGE(R, T)`, `map(R r);`.
        // A blank line, a comment, or a directive line between them ends the macro.
        bool next_line = sal || (after_specifiers && !gap.separated);
        // A name of ONE character can be the first element of a line of macro invocations:
        // `P(LINE) P(TEXT) P(NUM)` in the test drivers of bde. The rest of that line must hold macro
        // invocations, and a line of macro invocations is the only result that such a name can give.
        bool short_line = macro_line_possible && length < 2;
        if (!same_line || (gap.newlines > 0 && !next_line) || !((macro_name && length >= 2) || sal || short_line)) {
            return false;
        }
        AfterCall after = scan_after_macro_call(&reader, classes, sal, next_line,
                                               valid_symbols[PARAMETER_MACRO_TYPE_START], macro_line_possible);
        if (after == AFTER_CALL_MACRO_LINE) {
            lexer->result_symbol = MACRO_LINE_START;
            return reader.budget > 0;
        }
        if (after == AFTER_CALL_NONE || reader.budget == 0 || short_line) {
            return false;
        }
        bool tokens = args.not_expressions || args.statements;
        // The arguments of an attribute of a statement are a token tree when they are no expression
        // list. Refer to `_statement_attribute_macro_tokens`.
        TSSymbol statement_symbol = tokens ? STATEMENT_ATTRIBUTE_MACRO_TOKENS_START : STATEMENT_ATTRIBUTE_MACRO_START;
        if (after == AFTER_CALL_STATEMENT || after == AFTER_CALL_EXPRESSION) {
            lexer->result_symbol = statement_symbol;
            return statement_attribute && valid_symbols[statement_symbol];
        }
        if (after == AFTER_CALL_TYPE || after == AFTER_CALL_TYPE_POINTER || after == AFTER_CALL_TYPE_OR_STATEMENT) {
            // The macro gives the type, and its one argument is a type-id. A recorded class name is
            // the declarator of a constructor: `HAMT(const HAMT& h) V8_NOEXCEPT = default;`.
            if (args.one_type_id && macro_type && !class_name) {
                lexer->result_symbol =
                    valid_symbols[PARAMETER_MACRO_TYPE_START] ? PARAMETER_MACRO_TYPE_START : MACRO_TYPE_START;
                return true;
            }
            // The macro gives no type, because its arguments are no type-id. Where a statement can
            // start, the text after the macro is an expression statement, and the macro is an
            // attribute of that statement: `BOOST_ASIO_WRITE_HANDLER_CHECK(H, handler) type_check;`.
            if (after == AFTER_CALL_TYPE_OR_STATEMENT && statement_attribute && valid_symbols[statement_symbol]) {
                lexer->result_symbol = statement_symbol;
                return true;
            }
            return false;
        }
        if (after == AFTER_CALL_CONSTRUCTOR && constructor && !tokens) {
            lexer->result_symbol = CONSTRUCTOR_MACRO_START;
            return true;
        }
        lexer->result_symbol = tokens ? MACRO_CALL_ATTRIBUTE_TOKENS_START : MACRO_CALL_ATTRIBUTE_START;
        return valid_symbols[lexer->result_symbol];
    }
    // The scan of the macros before a constructor reads the same chain of macro names as the scan of
    // the attributes of a statement. One scan reads the text for the two, because the lexer cannot
    // move back.
    bool statement = false;
    bool macro_line = false;
    bool latest = !macro_name || length < 2;
    bool found = (constructor || statement_attribute || macro_line_possible) &&
                 scan_constructor_after_macro(&reader, classes, latest, &statement, &macro_line);
    if (statement) {
        lexer->result_symbol = STATEMENT_ATTRIBUTE_MACRO_START;
        return statement_attribute && reader.budget > 0;
    }
    if (macro_line) {
        lexer->result_symbol = MACRO_LINE_START;
        return macro_line_possible && reader.budget > 0;
    }
    if (!constructor || !found || reader.budget == 0) {
        return false;
    }
    lexer->result_symbol = CONSTRUCTOR_MACRO_START;
    return true;
}

/// Go past a balanced group in parentheses, from its `(` to the character after its `)`.
///
/// White space, comments, line splices, and the directive lines of a line group are the gap between two
/// tokens of the group (`skip_gap`), and each literal is one token (`read_token`). [lex.phases] p1.3
/// replaces each comment with one space before the parser gets the tokens, so a `(` or a `)` in a comment
/// is no bracket of the group. A `/*` or a `//` in the content of a literal starts no comment, and a `'`
/// between two digits is a digit separator ([lex.icon]) that starts no character literal.
///
/// Return false where the brackets do not balance in the budget of the scan, and where a directive of a
/// structured group stops the scan. O(n) in the length of the group.
static bool skip_parentheses(Reader *reader) {
    Arguments arguments = {0};
    return skip_group(reader, &arguments);
}

/// Read the name of a macro: two or more characters, an uppercase letter, and no lowercase letter,
/// as clang-format reads the name of a macro. No C++ keyword has this shape.
static bool read_macro_name(TSLexer *lexer) {
    unsigned length = 0;
    bool has_uppercase = false;
    for (;; ++length) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        if (c >= 'A' && c <= 'Z') {
            has_uppercase = true;
        } else if (c == '_' || (c >= '0' && c <= '9' && length > 0)) {
            // A digit or an underscore is part of the name.
        } else if ((c >= 'a' && c <= 'z') || c >= 0x80 || c == '$' || c == '\\') {
            // A lowercase letter or a different identifier character: not the name of a macro.
            return false;
        } else {
            break;
        }
        advance(lexer);
    }
    return length >= 2 && has_uppercase;
}

/// Read an identifier. Return the number of its characters, or 0 if no identifier starts here.
/// Copy the first characters into `text`, which has `capacity` characters.
static unsigned read_identifier(TSLexer *lexer, char *text, unsigned capacity) {
    unsigned length = 0;
    for (;;) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        bool is_start = c == '_' || c == '$' || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c >= 0x80;
        if (!is_start && !(length > 0 && c >= '0' && c <= '9')) {
            return length;
        }
        if (length < capacity) {
            text[length] = (char)c;
        }
        ++length;
        advance(lexer);
    }
}

/// The keywords that a `(` can follow at the start of a type or a declarator. After a macro, such
/// a keyword does not start the name of a function: `INLINE decltype(auto) f();`.
static const char *const KEYWORDS_BEFORE_PARENTHESIS[] = {
    "decltype", "typeof", "__typeof__", "__typeof", "typeof_unqual", "__typeof_unqual__", "alignas",
    "_Alignas", "sizeof", "alignof", "noexcept", "throw", "static_assert", "_Static_assert", "__attribute__",
    "__attribute", "__declspec", "explicit", "requires",
};

/// The qualifiers that can come between the macro of a pointer declarator and the name of that
/// declarator: `uint8_t* WEBP_RESTRICT const dst`.
static const char *const POINTER_QUALIFIER_WORDS[] = {
    "const",    "volatile",  "restrict", "__restrict", "__restrict__",      "_Nonnull",
    "_Nullable", "__nonnull", "__nullable", "_Null_unspecified", "_Nullable_result", NULL,
};

/// True if the `length` characters of `text` are a keyword of KEYWORDS_BEFORE_PARENTHESIS.
static bool is_keyword_before_parenthesis(const char *text, unsigned length) {
    for (size_t i = 0; i < sizeof KEYWORDS_BEFORE_PARENTHESIS / sizeof KEYWORDS_BEFORE_PARENTHESIS[0]; ++i) {
        if (strlen(KEYWORDS_BEFORE_PARENTHESIS[i]) == length && memcmp(KEYWORDS_BEFORE_PARENTHESIS[i], text, length) == 0) {
            return true;
        }
    }
    return false;
}

/// Go past the name of a macro between the name of a declarator and its parameter list:
/// `BOOST_MATH_PREVENT_MACRO_SUBSTITUTION` in
/// `double BOOST_MATH_TR1_DECL boost_acosh BOOST_MATH_PREVENT_MACRO_SUBSTITUTION(double x);`.
///
/// The name has the shape that `read_macro_name` reads, and the `(` after it is the parameter list of
/// the declarator. The scanner gives that name its own token (`DECLARATOR_NAME_MACRO_NAME`), and the
/// scan of the macro before the declarator goes past it to find the parameter list. Return true when
/// the scan read such a name and the gap after it. O(n) in the length of the name and the gap.
static bool skip_declarator_name_macro(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    if (!is_word_start(lexer->lookahead) || !read_macro_name(lexer)) {
        return false;
    }
    Gap gap = {0};
    skip_gap(reader, &gap);
    return !gap.blocked;
}

/// The operator names that are words ([over.oper.general], [lex.digraph]). A word after `operator` is
/// one of these names, or the type of a conversion function. No name of a type is in this list.
static const char *const WORD_OPERATOR_NAMES[] = {
    "new",    "delete", "co_await", "and",    "and_eq", "bitand", "bitor",
    "compl",  "not",    "not_eq",   "or",     "or_eq",  "xor",    "xor_eq", NULL,
};

/// Scan the name of a macro between the type and the declarator, in the place of a calling
/// convention: `WINAPI` in `DWORD WINAPI f(LPVOID p);`, `NODELETE` in `bool NODELETE f() const;`.
/// The scan starts after the name, and the token ends there.
///
/// After the name come a `*`, an operator function, or the name of a function, its parameters,
/// and a token that is not `:`. The name of the function can be qualified, a second macro can come
/// between that name and the parameter list, and line breaks can come before and after the macro. In
/// `const int MAX_SIZE = 8;` and `int FLAGS(int);` the uppercase name is the declarator.
///
/// The name of an operator function can be a word: `operator new`, `operator delete`, `operator
/// co_await`, and each alternative token ([lex.digraph]). The name of a conversion function is also a
/// word, and `WORD_OPERATOR_NAMES` tells the two apart.
///
/// The parser can have a second stack version in which the name is a second macro before the
/// type or before a constructor. That version cannot shift this token, and it stops. The scanner
/// then rejects the shapes of that version: a constructor with an initializer list,
/// `A B C(int x) : x(x) {}`, and a conversion function, `A B operator T();`.
///
/// Comments and directive lines can come between the tokens (`skip_gap`). `scanner` holds the open groups.
/// `macro_shaped` tells if the name has the shape of a macro name (`is_macro_name`). A name with a
/// different shape is a macro by its place only, after the `*` or the `&` of a declarator, and a name
/// must come after it.
static bool scan_call_macro_name(TSLexer *lexer, const Scanner *scanner, bool pointer, bool macro_shaped,
                                 bool type_attribute) {
    lexer->result_symbol = pointer ? POINTER_CALL_MACRO_NAME : CALL_MACRO_NAME;
    mark_end(lexer);
    Reader reader = start_reader(lexer, MACRO_SCAN_LIMIT, scanner);
    Gap gap = {0};
    skip_gap(&reader, &gap);
    if (gap.blocked) {
        return false;
    }
    // An attribute macro between the type and the declarator can take an argument list:
    // `typedef unsigned int CV_DECL_ALIGNED(1) unaligned_uint;`. The declarator comes after it.
    // AN ARGUMENT LIST AFTER THE NAME ENDS THIS SCAN, AND ONLY THE ATTRIBUTE OF A TYPE COMES OUT OF IT.
    // A calling convention takes no arguments, so this function gave no token for such a name before.
    // The macro between the type of a declaration and the declarator of an OBJECT does take them:
    // `typedef unsigned int CV_DECL_ALIGNED(1) unaligned_uint;`.
    if (lexer->lookahead == '(') {
        if (!type_attribute || pointer) {
            return false;
        }
        Arguments arguments = {0};
        if (!skip_group(&reader, &arguments) || arguments.statements) {
            return false;
        }
        gap = (Gap){0};
        skip_gap(&reader, &gap);
        if (gap.blocked || gap.newlines > 0) {
            return false;
        }
        // The declarator of an object, with the pointer and reference operators before its name.
        while (lexer->lookahead == '*' || lexer->lookahead == '&') {
            LOOP_STEP();
            step(&reader);
            skip_gap(&reader, &gap);
            if (gap.blocked) {
                return false;
            }
        }
        char name[MACRO_WORD_SIZE];
        bool name_has_lower = false;
        if (!is_word_start(lexer->lookahead)) {
            return false;
        }
        read_word(&reader, name, &name_has_lower);
        if (!name_has_lower || is_grammar_keyword(name) || word_in(name, DECLARATION_START_WORDS)) {
            return false;
        }
        gap = (Gap){0};
        skip_gap(&reader, &gap);
        if (gap.blocked) {
            return false;
        }
        if (lexer->lookahead == '[') {
            Arguments extent = {0};
            if (!skip_group(&reader, &extent)) {
                return false;
            }
            gap = (Gap){0};
            skip_gap(&reader, &gap);
            if (gap.blocked) {
                return false;
            }
        }
        int32_t end = lexer->lookahead;
        if (end == '=') {
            advance(lexer);
            if (lexer->lookahead == '=') {
                return false;
            }
        } else if (end != ';' && end != ',') {
            // A `{` after the name also ends the head of a class, where the name is the class and no
            // declarator stands: `class LIBCPP_ABI LIBCPP_CAPABILITY("mutex") mutex {`.
            return false;
        }
        lexer->result_symbol = TYPE_ATTRIBUTE_MACRO_NAME;
        return true;
    }
    if (lexer->lookahead == '*') {
        // A `*` of a pointer declarator, and not the operator `*=` in `*L1 *= 2;`. Only a name with the
        // shape of a macro takes this form. With a different name, `x * y * z` is an expression.
        if (!macro_shaped) {
            return false;
        }
        advance(lexer);
        return lexer->lookahead != '=';
    }
    char text[24];
    bool qualified = false;
    for (;;) {
        LOOP_STEP();
        unsigned length = read_identifier(lexer, text, sizeof text);
        if (length == 0 || (length <= sizeof text && is_keyword_before_parenthesis(text, length))) {
            return false;
        }
        gap = (Gap){0};
        skip_gap(&reader, &gap);
        if (length == 8 && memcmp(text, "operator", 8) == 0) {
            // An operator function has a symbol after `operator`, and a conversion function a type. The
            // symbol can be `/`, which stops the gap.
            int32_t c = lexer->lookahead;
            if (gap.blocked) {
                return gap.slash;
            }
            if (is_word_start(c)) {
                // An operator name can be a word: `static void* U_EXPORT2 operator new(size_t);`. A
                // conversion function has the name of a type there, and no name of a type is an
                // operator name.
                char name[MACRO_WORD_SIZE];
                unsigned name_length = read_identifier(lexer, name, sizeof name - 1);
                if (name_length >= sizeof name - 1) {
                    return false;
                }
                name[name_length] = '\0';
                return word_in(name, WORD_OPERATOR_NAMES);
            }
            return true;
        }
        if (gap.blocked) {
            return false;
        }
        if (lexer->lookahead == ':') {
            advance(lexer);
            if (lexer->lookahead != ':') {
                return false;
            }
            advance(lexer);
            gap = (Gap){0};
            skip_gap(&reader, &gap);
            if (gap.blocked) {
                return false;
            }
            qualified = true;
            continue;
        }
        bool has_lower = false;
        for (unsigned i = 0; i < length && i < sizeof text; ++i) {
            has_lower |= text[i] >= 'a' && text[i] <= 'z';
        }
        bool keyword = false;
        if (length < sizeof text) {
            text[length] = '\0';
            keyword = is_grammar_keyword(text);
        }
        // After a pointer or reference operator the name that follows the macro is the declarator of
        // an object, and the token after it ends the declarator: `char * BROTLI_RESTRICT buffer;`,
        // `void f(int * RESTRICT p, int n)`. A `==` is a comparison, and `a * MASK == b` is no
        // declaration.
        if (pointer) {
            int32_t c = lexer->lookahead;
            // A qualifier comes between the macro and the name of the declarator.
            if (length < sizeof text && word_in(text, POINTER_QUALIFIER_WORDS)) {
                continue;
            }
            // A name that is a macro by its place only comes before the name of the declarator. A name
            // with the shape of a macro, or a keyword, after it is a token after the declarator:
            // `const char* name ABSL_ATTRIBUTE_UNUSED = "x";` and `void* p __asm__("r1");` declare
            // `name` and `p`.
            if (!macro_shaped && (!has_lower || keyword)) {
                return false;
            }
            if (c == ';' || c == ',' || c == '[' || c == ')' || c == '{') {
                return true;
            }
            if (c == '=') {
                advance(lexer);
                return lexer->lookahead != '=';
            }
        }
        bool name_of_a_type = length < sizeof text && word_in(text, DECLARATION_START_WORDS);
        // A second macro in the place of a calling convention can come before the declarator:
        // `void WINAPI QT_WIN_CALLBACK qt_fast_timer_proc(uint timerId, DWORD_PTR user)` of
        // qtbase/src/corelib/kernel/qeventdispatcher_win.cpp:82. The scan gives no token for the first
        // of the two names, because no lexical signal tells that shape from
        // `LLVM_ABI LLVM_READNONE LLT getLCMType(LLT OrigTy, LLT TargetTy);` of
        // llvm-project/llvm/include/llvm/CodeGen/GlobalISel/Utils.h:379, where the two macros come
        // before the type and `specifier_prefix` reads them. The scan sees the same three names in the
        // two shapes, and only name lookup tells the type from the macro. The measurement of
        // 329,387 files gives 6 files of the first shape and 38 of the second.
        //
        // The declarator after the macro can be a template-id, in the explicit specialization and in
        // the explicit instantiation of a function template:
        // `template <> void MLASCALL MlasComputeExp<float>(const float* Input, float* Output, size_t N)`
        // of onnxruntime/onnxruntime/core/mlas/lib/compute.cpp:236. The scan goes past the template
        // argument list to the parameter list. The name has the same three conditions.
        //
        // The macro before such a declarator can also be the type of the declaration, with a macro among
        // the specifiers before it: `MLAS_FORCEINLINE MLAS_FLOAT16X8 PoolInit16x8<MaxPoolAggregation>()`
        // of onnxruntime/onnxruntime/core/mlas/lib/pooling_fp16.cpp:65 gives the type `MLAS_FORCEINLINE`
        // and the macro `MLAS_FLOAT16X8`. The scan sees the same two names in the two shapes, and only
        // name lookup tells the type from the macro. A second external token with a lower dynamic
        // precedence does not select the other reading, because a token of this scanner takes the place
        // of the name and the reading with the name does not continue.
        if (lexer->lookahead == '<' && !qualified && !name_of_a_type && has_lower) {
            gap = (Gap){0};
            if (!skip_template_arguments(&reader, &gap)) {
                return false;
            }
        } else if (lexer->lookahead != '(') {
            // A macro can come between the name of the declarator and its parameter list, and the scan
            // goes past it: `double DECL acosh BOOST_MATH_PREVENT_MACRO_SUBSTITUTION(double x);`. The
            // name before that macro is the declarator, and it has the same three conditions.
            if (qualified || name_of_a_type || !has_lower || !skip_declarator_name_macro(&reader)) {
                return false;
            }
        }
        if (lexer->lookahead != '(' || !skip_parentheses(&reader)) {
            return false;
        }
        // Skip the qualifiers and the specifiers after the parameters: `const`, `&`,
        // `noexcept(true)`, and macros.
        bool after_word = false;
        for (;;) {
            LOOP_STEP();
            gap = (Gap){0};
            skip_gap(&reader, &gap);
            if (gap.blocked) {
                break;
            }
            if (lexer->lookahead == '&') {
                advance(lexer);
                after_word = false;
            } else if (lexer->lookahead == '(') {
                if (!skip_parentheses(&reader)) {
                    return false;
                }
                after_word = false;
            } else {
                unsigned word_length = read_identifier(lexer, text, sizeof text);
                if (word_length == 0) {
                    break;
                }
                // A type keyword starts a declaration, and the parentheses were the arguments of a
                // macro call: `A B\n    C(1) void *f();`.
                if (word_length < sizeof text) {
                    text[word_length] = '\0';
                    if (word_in(text, DECLARATION_START_WORDS)) {
                        return false;
                    }
                }
                after_word = true;
            }
        }
        // A name before `*` is also the type of a declaration: `A C(1) T *f();`.
        if (gap.blocked) {
            return true;
        }
        if (after_word && lexer->lookahead == '*') {
            return false;
        }
        if (lexer->lookahead != ':') {
            return true;
        }
        advance(lexer);
        return lexer->lookahead == ':';
    }
}

/// The maximum number of characters that the scan of a macro in a grouping reads after the name.
#define MAX_GROUPING_LOOKAHEAD 4096

/// Scan the name of a macro after the `(` of a grouping, in the place of a calling convention: `WINAPI` in
/// `typedef void (WINAPI *F)(void *);`, `STDMETHODCALLTYPE` in `HRESULT (STDMETHODCALLTYPE I::*f)();`. The
/// scan starts after the name, and the token ends there.
///
/// Clang `ParseParenDeclarator` reads a calling convention after the `(`, and the calling convention applies
/// to a function type. For this reason, a declarator, the `)` of the grouping, and a parameter list come
/// after the name. The declarator starts with `*`, `&`, `&&`, `^`, or a name that is not a keyword. It holds
/// names, pointer operators, `::`, `...`, template argument lists after a name, and balanced groups in `()`
/// or `[]`: `(WINAPI *table[4])(int)`, `(WINAPI CreateHardLinkW_t)(`. An abstract declarator can be a
/// pointer operator only: `LONG (WINAPI *)(PVOID)`. In the call `f(SIZE * n);`, no parameter list comes
/// after the `)`, and the name stays an identifier. A name before `::` is a scope: `RET (OBJ::*)(ARGS...)`.
/// O(n) in the characters that the scan reads.
static bool scan_grouping_call_macro_name(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    lexer->result_symbol = GROUPING_CALL_MACRO_NAME;
    mark_end(lexer);
    if (reader->budget > MAX_GROUPING_LOOKAHEAD) {
        reader->budget = MAX_GROUPING_LOOKAHEAD;
    }
    char word[MACRO_WORD_SIZE];
    bool first = true;
    bool after_word = false;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        if (gap.blocked || !readable(reader)) {
            return false;
        }
        int32_t c = lexer->lookahead;
        if (c == ')') {
            if (first) {
                return false;
            }
            step(reader);
            gap = (Gap){0};
            skip_gap(reader, &gap);
            return !gap.blocked && readable(reader) && lexer->lookahead == '(';
        }
        if (c == '(' || c == '[') {
            Arguments group = {0};
            if (first || !skip_group(reader, &group)) {
                return false;
            }
            after_word = false;
            continue;
        }
        if (c == '<') {
            if (!after_word || !skip_angles(reader)) {
                return false;
            }
            after_word = false;
            continue;
        }
        // `read_token` does not read a `{`, a `}`, or a `]`, and it gives no text for most literals. A
        // declarator in a grouping has none of these tokens.
        word[0] = '\0';
        TokenClass token = read_token(reader, word);
        if (token == TOKEN_OPEN || token == TOKEN_CLOSE) {
            return false;
        }
        if (token == TOKEN_WORD) {
            bool keyword = is_grammar_keyword(word) || word_in(word, RESERVED_WORDS);
            if ((first && keyword) || word_in(word, NOT_PARAMETER_WORDS) || word_in(word, STATEMENT_KEYWORDS)) {
                return false;
            }
            after_word = true;
        } else if (strcmp(word, "*") == 0 || strcmp(word, "&") == 0 || strcmp(word, "&&") == 0 ||
                   strcmp(word, "^") == 0 || (strcmp(word, "::") == 0 && !first) || strcmp(word, "...") == 0) {
            after_word = false;
        } else {
            return false;
        }
        first = false;
    }
}

/// The kind of a word that can come after a trailing macro on a different line.
typedef enum {
    /// A word that does not continue the attributes after a declarator.
    TRAILING_WORD_OTHER,
    /// A name with the shape that `read_macro_name` reads.
    TRAILING_WORD_MACRO,
    /// `__attribute__` or `__attribute`.
    TRAILING_WORD_ATTRIBUTE,
    /// `override` or `final`.
    TRAILING_WORD_VIRT_SPECIFIER,
} TrailingWord;

/// Read a word, and classify it for `scan_trailing_macro_name`. O(n) in the length of the word.
static TrailingWord read_trailing_word(TSLexer *lexer) {
    char text[16];
    unsigned length = 0;
    bool has_upper = false;
    bool macro_shape = is_word_start(lexer->lookahead);
    while (is_word_char(lexer->lookahead)) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        has_upper |= c >= 'A' && c <= 'Z';
        macro_shape &= (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_';
        if (length < sizeof text - 1) {
            text[length] = (char)c;
        }
        ++length;
        advance(lexer);
    }
    if (macro_shape && has_upper && length >= 2) {
        return TRAILING_WORD_MACRO;
    }
    if (length >= sizeof text) {
        return TRAILING_WORD_OTHER;
    }
    text[length] = '\0';
    if (strcmp(text, "__attribute__") == 0 || strcmp(text, "__attribute") == 0) {
        return TRAILING_WORD_ATTRIBUTE;
    }
    if (strcmp(text, "override") == 0 || strcmp(text, "final") == 0) {
        return TRAILING_WORD_VIRT_SPECIFIER;
    }
    return TRAILING_WORD_OTHER;
}

/// Scan the name of a macro between the name of a function and its argument list:
/// `BOOST_MATH_PREVENT_MACRO_SUBSTITUTION` in `return c_policies::acosh
/// BOOST_MATH_PREVENT_MACRO_SUBSTITUTION(x);`. The macro expands to nothing, and it stops the expansion
/// of a macro that has the name of the function. The scan starts after the name, and the token ends
/// there.
///
/// A balanced group in parentheses comes after the name on the same line. The grammar takes the token
/// after a qualified name only, and two plain names before a `(` keep the reading of the scope macro:
/// `CGAL_NTS abs(a)`. O(n) in the length of the group.
static bool scan_call_name_macro(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    lexer->result_symbol = CALL_NAME_MACRO_NAME;
    mark_end(lexer);
    Gap gap = {0};
    skip_gap(reader, &gap);
    if (gap.blocked || gap.newlines > 0 || lexer->lookahead != '(') {
        return false;
    }
    Arguments arguments = {0};
    return skip_group(reader, &arguments);
}

/// Scan the name of a macro after a declarator, in the place of a GNU attribute: `PURE` in
/// `virtual void f() PURE;`, `TSA_REQUIRES` in `void g() TSA_REQUIRES(mu);`.
///
/// The name has the shape that `read_macro_name` reads. A name on a different line is a macro
/// only if `;`, `{`, `=`, or `:` follows the name and its arguments. More macros, GNU attributes,
/// and virt-specifiers can come before that token: `REQUIRES(mu) EXCLUDES(mu2);`. GCC reads a
/// sequence of attributes after a member declarator (cp_parser_member_declaration), and Clang
/// reads a virt-specifier after them (ParseCXXMemberDeclaratorBeforeInitializer). A statement
/// macro with no `;` then does not continue the declarator on the line before it. `line_break`
/// tells if a line break comes before the name. O(n) in the length of the sequence.
///
/// With `declarator`, the name comes after the declarator of a variable or a data member, and the
/// token is DECLARATOR_MACRO_NAME. The rule for a different line then applies also on the same line,
/// a `,` can also come after the macros, and a virt-specifier cannot come among them. The arguments of
/// a macro are expressions, and they are not empty. In `T min BOOST_PREVENT_MACRO_SUBSTITUTION () const;`
/// and `double f PREVENT (double x);`, the parentheses hold the parameters of the function.
///
/// Comments and directive lines can come between the tokens (`skip_gap`). `scanner` holds the open groups.
static bool scan_trailing_macro_name(TSLexer *lexer, const Scanner *scanner, bool line_break, bool declarator,
                                     bool name_macro) {
    if (!read_macro_name(lexer)) {
        return false;
    }
    lexer->result_symbol = declarator ? DECLARATOR_MACRO_NAME : TRAILING_MACRO_NAME;
    mark_end(lexer);
    if (!line_break && !declarator) {
        return true;
    }
    Reader reader = start_reader(lexer, MACRO_SCAN_LIMIT, scanner);
    // A macro name can have arguments. A virt-specifier and the arguments of an attribute cannot.
    bool after_macro = true;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(&reader, &gap);
        if (gap.blocked) {
            return false;
        }
        if (after_macro && lexer->lookahead == '(') {
            // The group can be the parameter list of the declarator.
            bool parameter_list = false;
            if (declarator) {
                Arguments arguments = {0};
                if (!skip_group(&reader, &arguments)) {
                    return false;
                }
                parameter_list = name_macro && !arguments.not_parameters;
                if (arguments.empty || arguments.not_expressions || arguments.statements) {
                    // The group cannot be the arguments of a macro, and it is the parameter list of the
                    // declarator. The macro then comes between the name of the declarator and that list:
                    // `const P& min BOOST_PREVENT_MACRO_SUBSTITUTION () const;`.
                    if (parameter_list) {
                        lexer->result_symbol = DECLARATOR_NAME_MACRO_NAME;
                        return true;
                    }
                    return false;
                }
            } else if (!skip_parentheses(&reader)) {
                return false;
            }
            gap = (Gap){0};
            skip_gap(&reader, &gap);
            if (gap.blocked) {
                return false;
            }
            if (parameter_list && lexer->lookahead == '{') {
                // A body of statements after the group: `bool signbit NO_MACRO_EXPAND(T)` and `{ return false; }`
                // in boost/math/tr1.hpp. The arguments of an attribute macro of a variable have no body after
                // them, and the group is the parameter list of the declarator. A braced initializer holds no
                // statement, and `T v GUARDED_BY(mu) {};` keeps the macro of the variable.
                Arguments body = {0};
                if (!skip_group(&reader, &body)) {
                    return false;
                }
                if (body.statements) {
                    lexer->result_symbol = DECLARATOR_NAME_MACRO_NAME;
                }
                return true;
            }
        }
        switch (lexer->lookahead) {
        case ';':
        case '{':
        case '=':
            return true;
        case ',':
        case ')':
            // A `)` ends the declarator of the last parameter: `void f(int a DEFAULT(nullptr));`.
            if (declarator) {
                return true;
            }
            break;
        case ':':
            advance(lexer);
            return lexer->lookahead != ':';
        default:
            break;
        }
        TrailingWord word = read_trailing_word(lexer);
        after_macro = word == TRAILING_WORD_MACRO;
        if (word == TRAILING_WORD_ATTRIBUTE) {
            Gap after = {0};
            skip_gap(&reader, &after);
            if (after.blocked || lexer->lookahead != '(' || !skip_parentheses(&reader)) {
                return false;
            }
        } else if (word == TRAILING_WORD_OTHER || (declarator && word == TRAILING_WORD_VIRT_SPECIFIER)) {
            return false;
        }
    }
}

// Preprocessor directives.

/// The white space before a token.
typedef struct {
    /// A line break comes before the token. A line splice is not a line break.
    bool line_break;
    /// The number of spaces and tabs after the last line break.
    uint32_t spaces;
    /// A backslash that does not start a line splice stopped the scan.
    bool blocked;
} Space;

/// Skip the white space and the line splices before a token, and record them in the result.
static Space skip_space_before_token(TSLexer *lexer) {
    Space space = {false, 0, false};
    for (;;) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        if (c == '\\') {
            // A line splice continues the line, as the extras of the grammar do.
            if (skip_backslash(lexer, true) != BACKSLASH_SPLICE) {
                space.blocked = true;
                return space;
            }
            continue;
        }
        if (is_line_break(c)) {
            space.line_break = true;
            space.spaces = 0;
        } else if (is_horizontal_space(c)) {
            space.spaces++;
        } else {
            return space;
        }
        skip(lexer);
    }
}

/// Scan the end of a directive line: the line break after the last token of the line.
///
/// The end of the file also ends a directive line, with an empty token. A carriage return with no line feed after
/// it is also a line break, and the token is then also empty (libcpp `_cpp_clean_line`, Clang
/// `Lexer::LexTokenInternal` at the case of the carriage return).
static bool scan_line_end(TSLexer *lexer) {
    // A line splice continues the directive line on the next line, also when that line is empty.
    for (;;) {
        LOOP_STEP();
        if (lexer->lookahead == '\r') {
            skip(lexer);
            if (lexer->lookahead != '\n') {
                lexer->result_symbol = PREPROC_LINE_END;
                mark_end(lexer);
                return true;
            }
        } else if (is_horizontal_space(lexer->lookahead)) {
            skip(lexer);
        } else if (lexer->lookahead != '\\') {
            break;
        } else if (skip_backslash(lexer, true) != BACKSLASH_SPLICE) {
            return false;
        }
    }
    lexer->result_symbol = PREPROC_LINE_END;
    if (lexer->eof(lexer)) {
        mark_end(lexer);
        return true;
    }
    if (lexer->lookahead != '\n') {
        return false;
    }
    advance(lexer);
    mark_end(lexer);
    return true;
}

// ---------------------------------------------------------------------------------------------------
// Names, comments, and literals, as the scans of conditional groups read them.
// ---------------------------------------------------------------------------------------------------

/// A name of up to MAX_NAME_LENGTH characters. A longer name has the length MAX_NAME_LENGTH + 1.
typedef struct {
    char text[MAX_NAME_LENGTH + 1];
    uint32_t length;
    /// True when the full name has the shape that `is_macro_name` reads: an uppercase letter, and only uppercase
    /// ASCII letters, digits, and `_`.
    bool macro_shaped;
} Name;

static bool name_is(const Name *name, const char *text) {
    return name->length <= MAX_NAME_LENGTH && strcmp(name->text, text) == 0;
}

static bool name_is_one_of(const Name *name, const char *const *list) {
    for (; *list; list++) {
        if (name_is(name, *list)) {
            return true;
        }
    }
    return false;
}

/// Read a name. The first character is the lookahead.
static void read_name(TSLexer *lexer, Name *name) {
    name->length = 0;
    bool has_upper = false;
    bool only_macro_characters = true;
    while (is_word_char(lexer->lookahead)) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        has_upper |= c >= 'A' && c <= 'Z';
        only_macro_characters &= (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_';
        if (name->length < MAX_NAME_LENGTH) {
            name->text[name->length] = (char)lexer->lookahead;
        }
        if (name->length <= MAX_NAME_LENGTH) {
            name->length++;
        }
        advance(lexer);
    }
    name->text[name->length <= MAX_NAME_LENGTH ? name->length : MAX_NAME_LENGTH] = '\0';
    name->macro_shaped = has_upper && only_macro_characters;
}

/// Skip a quoted literal. The lookahead is the quote. Return false when a line break comes before the
/// closing quote.
static bool skip_quoted_literal(TSLexer *lexer) {
    int32_t quote = lexer->lookahead;
    advance(lexer);
    for (;;) {
        LOOP_STEP();
        if (lexer->eof(lexer) || is_line_break(lexer->lookahead)) {
            return false;
        }
        if (lexer->lookahead == '\\') {
            skip_literal_backslash(lexer);
            continue;
        }
        if (lexer->lookahead == quote) {
            advance(lexer);
            return true;
        }
        advance(lexer);
    }
}

/// Skip a raw string literal. The lookahead is the quote after the prefix. Return false at the end of the
/// file or at an incorrect delimiter.
static bool skip_raw_string_literal(TSLexer *lexer) {
    advance(lexer);
    int32_t delimiter[MAX_DELIMITER_LENGTH];
    uint32_t length = 0;
    while (lexer->lookahead != '(') {
        LOOP_STEP();
        if (lexer->eof(lexer) || length == MAX_DELIMITER_LENGTH || is_line_break(lexer->lookahead)) {
            return false;
        }
        delimiter[length++] = lexer->lookahead;
        advance(lexer);
    }
    advance(lexer);
    for (;;) {
        LOOP_STEP();
        if (lexer->eof(lexer)) {
            return false;
        }
        if (lexer->lookahead != ')') {
            advance(lexer);
            continue;
        }
        advance(lexer);
        uint32_t matched = 0;
        while (matched < length && lexer->lookahead == delimiter[matched]) {
            advance(lexer);
            matched++;
        }
        if (matched == length && lexer->lookahead == '"') {
            advance(lexer);
            return true;
        }
    }
}

/// Skip a preprocessing number, with its digit separators, exponent signs, and suffix. The lookahead is its first
/// digit.
///
/// A digit separator is not the start of a character literal: `1'000` is one token ([lex.ppnumber], libcpp
/// `lex_number`). The scan reads the same text as `skip_number` of a lookahead scan.
static void skip_number_literal(TSLexer *lexer) {
    for (;;) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        if (c == 'e' || c == 'E' || c == 'p' || c == 'P') {
            advance(lexer);
            if (lexer->lookahead == '+' || lexer->lookahead == '-') {
                advance(lexer);
            }
            continue;
        }
        if (is_word_char(c) || c == '.' || c == '\'') {
            advance(lexer);
            continue;
        }
        return;
    }
}

/// Skip a block comment. The lookahead is the `*` after the `/`. Set `line_break` when the comment holds a line
/// break. Return true when the comment has its end `*/`, and false at the end of the file.
static bool skip_block_comment_text(TSLexer *lexer, bool *line_break) {
    advance(lexer);
    for (;;) {
        LOOP_STEP();
        if (lexer->eof(lexer)) {
            return false;
        }
        if (is_line_break(lexer->lookahead)) {
            *line_break = true;
        }
        if (lexer->lookahead == '*') {
            advance(lexer);
            if (lexer->lookahead == '/') {
                advance(lexer);
                return true;
            }
            continue;
        }
        advance(lexer);
    }
}

/// Skip a block comment. The lookahead is the `*` after the `/`. Return true when the comment has a line break.
static bool skip_block_comment(TSLexer *lexer) {
    bool line_break = false;
    skip_block_comment_text(lexer, &line_break);
    return line_break;
}

/// Go past a block comment of a directive line. The lookahead is the `*` after the `/`.
///
/// A block comment is white space of the line, also when it holds a line break. The directive line
/// ends after the comment, and not at the line break inside it. libcpp `_cpp_lex_direct` goes past
/// such a comment with `_cpp_skip_block_comment` (gcc/libcpp/lex.cc:1849, from lex.cc:4081), and it
/// gives the end of the directive line after it. Clang `Lexer::SkipBlockComment`
/// (clang/lib/Lex/Lexer.cpp:2974) gives no `eod` token for a line break inside a comment.
///
/// A comment with no end reads to the end of the file, and the scan of the caller stops there.
static void skip_directive_block_comment(TSLexer *lexer) {
    bool line_break = false;
    skip_block_comment_text(lexer, &line_break);
}

/// Skip the rest of a line comment. The lookahead is the character after `//`. A line splice continues
/// the comment on the next line. The line break is not skipped.
static void skip_line_comment(TSLexer *lexer) {
    while (!lexer->eof(lexer) && !is_line_break(lexer->lookahead)) {
        LOOP_STEP();
        if (lexer->lookahead == '\\') {
            skip_backslash(lexer, false);
        } else {
            advance(lexer);
        }
    }
}

/// The result of the scan of the text of a directive line.
typedef enum {
    /// The line has only white space before its line break. The lexer is at that line break, and the scan of the
    /// end of the line reads it.
    ARG_LINE_END,
    /// The scan gives the token PREPROC_ARG.
    ARG_TEXT,
    /// The text starts with a comment, and the scan gives no token. The lexer is not at the start of the text,
    /// and the scanner returns false. The lexer of the parser then reads the comment, which is an extra.
    ARG_STOP,
} ArgScan;

/// Scan the text of a directive line: the value of a `#define`, the argument of a `#pragma`, and the parameters
/// of an `#embed`.
///
/// The text is the sequence of preprocessing tokens up to the line break of the line ([cpp.replace]). libcpp
/// `_cpp_lex_direct` (lex.cc:3888) lexes each token of the line. A comment is white space in that text, and the
/// token ends before it, so that the tree has a node for the comment. A string literal, a character literal, and
/// a raw string literal are one token each, and a `/*` or a `//` in the content of such a literal starts no
/// comment. A raw string literal holds each line break of its content: libcpp `get_fresh_line_impl`
/// (lex.cc:3806) takes a new line in a directive for `lex_raw_string` (lex.cc:2550) only. A digit separator is
/// part of a number (libcpp `lex_number`, lex.cc:2325), and it starts no character literal. A line splice
/// continues the line (libcpp `_cpp_clean_line`, lex.cc:877).
///
/// Give the empty mark before the extra tokens of an `#endif` or an `#else` line. The lookahead is the
/// character after the directive name.
///
/// An `#endif` and an `#else` take a fixed number of tokens, and the preprocessor gives a warning and
/// removes the rest of the line: libcpp `check_eol_endif_labels` (gcc/libcpp/directives.cc:250) from
/// `do_else` (:2648) and `do_endif` (:2791), and Clang `Preprocessor::CheckEndOfDirective`
/// (clang/lib/Lex/PPDirectives.cpp:465) from `HandleElseDirective` (:3661) and `HandleEndifDirective`
/// (:3635). The two front ends compile such a line with a warning only.
///
/// The mark is empty, and the rule takes it as an option. A line with no extra token then takes no mark
/// and no line end, and the node of the branch or of the group keeps the range that it had. Only the
/// mark makes the text of the line valid in this position.
///
/// The scan gives no mark before a comment, so that `#endif // c` keeps the comment as a node. A
/// declined scan reads the horizontal space of the line only. The scan of the white space that follows
/// reads the line break after that space, and a line break resets the count of the spaces. O(n) in the
/// length of the horizontal space.
static bool scan_preproc_extra_mark(Scanner *scanner, TSLexer *lexer) {
    if (!scanner->preproc_extra_tokens) {
        return false;
    }
    scanner->preproc_extra_tokens = false;
    mark_end(lexer);
    lexer->result_symbol = PREPROC_EXTRA_MARK;
    return true;
}

/// True when the rest of a directive line holds a token. The lookahead is the character after the
/// directive name. The scan reads no character of the token of the directive, because `scan_directive`
/// ended that token before it.
///
/// A comment is no token of the line, so `#endif // c` keeps the comment as a node of the tree. A line
/// splice continues the line. O(n) in the length of the rest of the line.
static bool line_has_extra_tokens(TSLexer *lexer) {
    for (;;) {
        LOOP_STEP();
        if (is_splice_space(lexer->lookahead)) {
            advance(lexer);
            continue;
        }
        if (lexer->lookahead == '\\' && skip_backslash(lexer, false) == BACKSLASH_SPLICE) {
            continue;
        }
        break;
    }
    if (lexer->eof(lexer) || is_line_break(lexer->lookahead) || lexer->lookahead == '\\') {
        return false;
    }
    if (lexer->lookahead != '/') {
        return true;
    }
    // A comment is white space of the line. A `/` that starts no comment is a token.
    advance(lexer);
    return lexer->lookahead != '/' && lexer->lookahead != '*';
}

/// The token ends at the last character that is not white space. White space before the text, and a line splice
/// before the text, are not part of the token. A backslash that starts no line splice is also white space there,
/// and the token then starts after it. O(n) in the length of the line.
static ArgScan scan_preproc_arg(TSLexer *lexer) {
    for (;;) {
        LOOP_STEP();
        if (is_splice_space(lexer->lookahead)) {
            skip(lexer);
            continue;
        }
        if (lexer->lookahead != '\\' || skip_backslash(lexer, true) != BACKSLASH_SPLICE) {
            break;
        }
    }
    bool has_text = false;
    for (;;) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        if (lexer->eof(lexer) || is_line_break(c)) {
            break;
        }
        if (is_splice_space(c)) {
            advance(lexer);
            continue;
        }
        if (c == '\\') {
            if (skip_backslash(lexer, false) == BACKSLASH_SPLICE) {
                continue;
            }
        } else if (c == '/') {
            advance(lexer);
            if (lexer->lookahead == '*' || lexer->lookahead == '/') {
                if (has_text) {
                    break;
                }
                // The comment comes before the text of the line, and the lexer of the parser reads it as an
                // extra. A block comment with no end holds the rest of the file, and that lexer has no token
                // for it. The text of the line then holds the comment.
                bool line_break = false;
                if (lexer->lookahead == '/' || skip_block_comment_text(lexer, &line_break)) {
                    return ARG_STOP;
                }
            }
        } else if (c == '"' || c == '\'') {
            skip_quoted_literal(lexer);
        } else if (is_digit(c)) {
            skip_number_literal(lexer);
        } else if (is_word_start(c)) {
            Name name;
            read_name(lexer, &name);
            if (lexer->lookahead == '"' && name_is_one_of(&name, RAW_STRING_PREFIXES)) {
                skip_raw_string_literal(lexer);
            }
        } else {
            advance(lexer);
        }
        mark_end(lexer);
        has_text = true;
    }
    if (!has_text) {
        return ARG_LINE_END;
    }
    lexer->result_symbol = PREPROC_ARG;
    return ARG_TEXT;
}

/// Skip to the end of a line, over line splices, comments, and literals. The line break is not skipped.
///
/// A digit separator is part of a number, and it does not start a character literal.
///
/// With `skipped_text`, the text is the text of a branch that the parser skips as one token. The scan then reads
/// each token as the preprocessor reads it, and a raw string literal holds each line break and each `#` of its
/// content (libcpp `_cpp_lex_token` lexes each token and drops it, Clang
/// `Preprocessor::SkipExcludedConditionalBlock` lexes each token in raw mode). The scan stops at the end of the
/// line of the closing delimiter of that literal. Without `skipped_text`, the parser reads the tokens of the line,
/// and its token of a directive line ends at the line break. A raw string literal then also ends there.
static void skip_rest_of_line(TSLexer *lexer, bool skipped_text) {
    for (;;) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        if (lexer->eof(lexer) || is_line_break(c)) {
            return;
        }
        if (c == '\\') {
            skip_backslash(lexer, false);
            continue;
        }
        if (c == '/') {
            advance(lexer);
            if (lexer->lookahead == '*') {
                skip_block_comment(lexer);
            } else if (lexer->lookahead == '/') {
                advance(lexer);
                skip_line_comment(lexer);
            }
            continue;
        }
        if (is_digit(c)) {
            skip_number_literal(lexer);
            continue;
        }
        if (is_word_start(c)) {
            Name name;
            read_name(lexer, &name);
            if (skipped_text && lexer->lookahead == '"' && name_is_one_of(&name, RAW_STRING_PREFIXES)) {
                skip_raw_string_literal(lexer);
            }
            continue;
        }
        if (c == '"' || c == '\'') {
            skip_quoted_literal(lexer);
            continue;
        }
        advance(lexer);
    }
}

/// The kind of a conditional directive name.
typedef enum { COND_NONE, COND_IF, COND_ELSE, COND_ENDIF } ConditionalKind;

static ConditionalKind conditional_kind(const Name *name) {
    if (name_is(name, "if") || name_is(name, "ifdef") || name_is(name, "ifndef")) {
        return COND_IF;
    }
    if (name_is(name, "elif") || name_is(name, "elifdef") || name_is(name, "elifndef") || name_is(name, "else")) {
        return COND_ELSE;
    }
    if (name_is(name, "endif")) {
        return COND_ENDIF;
    }
    return COND_NONE;
}

/// Read the `#` or `%:` of a directive, the spaces after it, and its name. The lookahead is `#` or `%`.
/// Return false when the text is not a directive start.
static bool read_directive_name(TSLexer *lexer, Name *name) {
    if (lexer->lookahead == '%') {
        advance(lexer);
        if (lexer->lookahead != ':') {
            return false;
        }
    }
    advance(lexer);
    while (is_horizontal_space(lexer->lookahead)) {
        LOOP_STEP();
        advance(lexer);
    }
    read_name(lexer, name);
    return true;
}

/// Read the text of a directive line after the directive name. Return true when the line has only spaces
/// and comments.
static bool condition_is_empty(TSLexer *lexer) {
    for (;;) {
        LOOP_STEP();
        // A carriage return also ends the line, with or without a line feed after it.
        while (is_splice_space(lexer->lookahead)) {
            LOOP_STEP();
            advance(lexer);
        }
        if (lexer->eof(lexer) || is_line_break(lexer->lookahead)) {
            return true;
        }
        if (lexer->lookahead != '/') {
            return false;
        }
        advance(lexer);
        if (lexer->lookahead == '/') {
            return true;
        }
        if (lexer->lookahead != '*') {
            return false;
        }
        skip_directive_block_comment(lexer);
    }
}

/// Read the condition of a `#if` or `#elif` after the directive name. Return true when the condition is
/// the one token `0` or `false`, as the clangd `DirectiveTree` branch chooser reads it. The scan stops
/// at the first token after the condition, or at the end of the line.
static bool condition_is_false(TSLexer *lexer) {
    while (is_horizontal_space(lexer->lookahead)) {
        LOOP_STEP();
        advance(lexer);
    }
    Name value;
    if (lexer->lookahead >= '0' && lexer->lookahead <= '9') {
        read_name(lexer, &value);
        if (!name_is(&value, "0")) {
            return false;
        }
    } else if (is_word_char(lexer->lookahead)) {
        read_name(lexer, &value);
        if (!name_is(&value, "false")) {
            return false;
        }
    } else {
        return false;
    }
    for (;;) {
        LOOP_STEP();
        // A carriage return also ends the line, with or without a line feed after it.
        while (is_splice_space(lexer->lookahead)) {
            LOOP_STEP();
            advance(lexer);
        }
        if (lexer->eof(lexer) || is_line_break(lexer->lookahead)) {
            return true;
        }
        if (lexer->lookahead != '/') {
            return false;
        }
        advance(lexer);
        if (lexer->lookahead == '/') {
            return true;
        }
        if (lexer->lookahead != '*') {
            return false;
        }
        skip_directive_block_comment(lexer);
    }
}

// ---------------------------------------------------------------------------------------------------
// Directive lines in the lookahead scans.
// ---------------------------------------------------------------------------------------------------

/// Go past the rest of a line in a lookahead scan, over line splices, comments, and literals, as `skip_rest_of_line`
/// does. The line break stays unread. With `skipped_text`, a raw string literal holds each line break of its
/// content. O(n) in the length of the line.
static void skip_line(Reader *reader, bool skipped_text) {
    TSLexer *lexer = reader->lexer;
    while (readable(reader) && !is_line_break(lexer->lookahead)) {
        LOOP_STEP();
        int32_t c = lexer->lookahead;
        if (c == '\\') {
            step_backslash(reader);
        } else if (c == '/') {
            step(reader);
            if (lexer->lookahead == '*') {
                step(reader);
                bool star = false;
                while (readable(reader) && !(star && lexer->lookahead == '/')) {
                    LOOP_STEP();
                    star = lexer->lookahead == '*';
                    step(reader);
                }
                step(reader);
            } else if (lexer->lookahead == '/') {
                step_line_comment(reader);
            }
        } else if (c == '"' || c == '\'') {
            skip_quoted(reader);
        } else if (is_digit(c)) {
            skip_number(reader);
        } else if (is_word_start(c)) {
            // A raw string literal holds each line break and each `#` of its content. The word before its `"` is
            // not the name of a macro, and the hash of the last name stays.
            char word[MACRO_WORD_SIZE];
            bool has_lower = false;
            uint32_t hash = reader->word_hash;
            read_word(reader, word, &has_lower);
            reader->word_hash = hash;
            if (skipped_text && lexer->lookahead == '"' && word_in(word, RAW_STRING_PREFIXES)) {
                skip_raw_string(reader);
            }
        } else {
            step(reader);
        }
    }
}

/// Read the name of a directive in a lookahead scan. The lookahead is the character after the `#`.
static void read_reader_directive_name(Reader *reader, Name *name) {
    TSLexer *lexer = reader->lexer;
    while (readable(reader) && is_horizontal_space(lexer->lookahead)) {
        LOOP_STEP();
        step(reader);
    }
    name->length = 0;
    name->text[0] = '\0';
    name->macro_shaped = false;
    if (readable(reader)) {
        read_name(lexer, name);
        reader->budget = reader->budget > name->length ? reader->budget - name->length : 0;
    }
}

/// Go past the text of a branch that the parser does not read as code, from a directive name in the branch. Stop
/// after the name of the next `#elif`, `#else`, or `#endif` of the group, or with `stop_at_branch` false, after the
/// name of its `#endif`. Return false at the end of the file or of the budget. The parser reads this text as a
/// `preproc_skipped` token (`scan_skipped_text`). O(n) in the length of the text.
static bool skip_branch_text(Reader *reader, bool stop_at_branch, Name *name) {
    TSLexer *lexer = reader->lexer;
    uint32_t depth = 0;
    for (;;) {
        LOOP_STEP();
        skip_line(reader, true);
        if (!readable(reader)) {
            return false;
        }
        step(reader);
        while (readable(reader) && is_horizontal_space(lexer->lookahead)) {
            LOOP_STEP();
            step(reader);
        }
        if (!readable(reader) || lexer->lookahead != '#') {
            continue;
        }
        step(reader);
        read_reader_directive_name(reader, name);
        ConditionalKind kind = conditional_kind(name);
        if (kind == COND_IF) {
            depth++;
        } else if (depth == 0 && (kind == COND_ENDIF || (kind == COND_ELSE && stop_at_branch))) {
            return true;
        } else if (kind == COND_ENDIF) {
            depth--;
        }
    }
}

/// Go past a directive line in a lookahead scan, from its `#` at the start of a line, up to the line break at its
/// end. Return false when the scan stops: the parser does not read the line as a directive line, or the text after
/// it does not come next in the tokens that the parser gets.
///
/// The scan goes past a directive line that is not conditional, and past the null directive. An `#embed` gives
/// tokens in an expression, and the scan stops after its name. The conditional directives follow the rules of the
/// line groups (`scan_directive`):
/// - An `#if`, `#ifdef`, or `#ifndef` opens a group. The scan reads its first branch that is not false, as in `#if 0`
///   (`condition_is_false`), and it goes past the text of the false branches.
/// - After a branch that the scan read, an `#elif` or `#else` starts the text up to the `#endif`, which the scan goes
///   past.
/// - An `#elif`, `#else`, or `#endif` of an open group of the scanner, outside the groups of the scan, is a directive
///   of a line group or of a structured group. The scan goes past the directives of a line group. A structured group
///   is a node, and its branch ends at the directive, as a block ends at its `}`. The scan stops after the name, and
///   it sets `gap->directive`.
///
/// Each group of the scan is a line group. A structured group starts only where an item starts. After a macro
/// invocation line, the first line of its first branch is also the first line that the parser reads. The scan does
/// not select a later branch as `select_line_branch` does. O(n) in the length of the text that the scan reads.
static bool skip_directive(Reader *reader, Gap *gap) {
    TSLexer *lexer = reader->lexer;
    step(reader);
    if (lexer->lookahead == '#') {
        // `##` is the paste operator of a macro body, not a directive.
        return false;
    }
    Name name;
    read_reader_directive_name(reader, &name);
    for (;;) {
        LOOP_STEP();
        ConditionalKind kind = conditional_kind(&name);
        const Scanner *scanner = reader->scanner;
        uint8_t outer = scanner != NULL && reader->outer_groups > 0 ? scanner->groups[reader->outer_groups - 1] : 0;
        bool to_group_end = false;
        switch (kind) {
            case COND_NONE:
                if (name_is(&name, "embed")) {
                    gap->directive = true;
                    return false;
                }
                gap->directive_line |= gap->newlines > 0;
                gap->separated |= gap->newlines > 0;
                skip_line(reader, false);
                return true;
            case COND_IF:
                if (reader->group_count == MAX_GROUPS) {
                    return false;
                }
                gap->directive_line |= gap->newlines > 0;
                gap->separated |= gap->newlines > 0;
                if (name_is(&name, "if") && condition_is_false(lexer)) {
                    reader->groups[reader->group_count++] = false;
                    break;
                }
                reader->groups[reader->group_count++] = true;
                skip_line(reader, false);
                return true;
            case COND_ELSE:
                if (reader->group_count > 0) {
                    bool *read = &reader->groups[reader->group_count - 1];
                    if (*read) {
                        to_group_end = true;
                    } else if (!(name_is(&name, "elif") && condition_is_false(lexer))) {
                        *read = true;
                        skip_line(reader, false);
                        return true;
                    }
                } else if (outer == GROUP_LINE_CHOSEN) {
                    to_group_end = true;
                } else {
                    gap->directive = true;
                    return false;
                }
                break;
            case COND_ENDIF:
                if (reader->group_count > 0) {
                    reader->group_count--;
                } else if (outer == GROUP_LINE_CHOSEN) {
                    reader->outer_groups--;
                } else {
                    gap->directive = true;
                    return false;
                }
                skip_line(reader, false);
                return true;
        }
        if (!skip_branch_text(reader, !to_group_end, &name)) {
            return false;
        }
        if (to_group_end) {
            if (reader->group_count > 0) {
                reader->group_count--;
            } else {
                reader->outer_groups--;
            }
            skip_line(reader, false);
            return true;
        }
    }
}

/// Read the rest of the current line, and the blank lines and the comment lines after it. Return true and the
/// directive name when the next line with a token starts a directive. Otherwise return false, with the lexer at
/// the first token of that line or at the end of the file.
///
/// A `#` after a comment on its line is a directive only where a declaration can start. After the end of a
/// construct the scanner does not read it as a directive, and this function returns false for it. O(n) in the
/// length of the lines that the function reads.
///
/// A directive line can start with the digraph `%:`, and the scan reads the `%` of a line that starts with a
/// different `%` token. `percent_read` is then true, and the lexer is after that `%`.
static bool read_to_next_directive_line(TSLexer *lexer, Name *name, bool *percent_read) {
    skip_rest_of_line(lexer, false);
    for (;;) {
        LOOP_STEP();
        if (lexer->eof(lexer)) {
            return false;
        }
        // The lookahead is the line break at the end of a line.
        advance(lexer);
        bool after_comment = false;
        for (;;) {
            LOOP_STEP();
            while (is_horizontal_space(lexer->lookahead)) {
                LOOP_STEP();
                advance(lexer);
            }
            if (lexer->lookahead != '/') {
                break;
            }
            advance(lexer);
            if (lexer->lookahead == '/') {
                advance(lexer);
                skip_line_comment(lexer);
                break;
            }
            if (lexer->lookahead != '*') {
                return false;
            }
            skip_block_comment(lexer);
            after_comment = true;
        }
        if (lexer->eof(lexer)) {
            return false;
        }
        if (is_line_break(lexer->lookahead)) {
            continue;
        }
        if (after_comment || (lexer->lookahead != '#' && lexer->lookahead != '%')) {
            return false;
        }
        bool read = read_directive_name(lexer, name);
        *percent_read = !read;
        return read;
    }
}

/// Read the lines of a group up to its `#endif`, from the end of a directive name in the group. Return false at
/// the end of the file. The lexer is then after the name `endif`. O(n) in the length of the lines.
static bool read_to_group_end(TSLexer *lexer) {
    uint32_t depth = 0;
    Name name;
    for (;;) {
        LOOP_STEP();
        skip_rest_of_line(lexer, true);
        if (lexer->eof(lexer)) {
            return false;
        }
        advance(lexer);
        while (is_horizontal_space(lexer->lookahead)) {
            LOOP_STEP();
            advance(lexer);
        }
        if ((lexer->lookahead != '#' && lexer->lookahead != '%') || !read_directive_name(lexer, &name)) {
            continue;
        }
        ConditionalKind kind = conditional_kind(&name);
        if (kind == COND_IF) {
            depth++;
        } else if (kind == COND_ENDIF) {
            if (depth == 0) {
                return true;
            }
            depth--;
        }
    }
}

/// Read the text of the false branches of a group, from the condition of its `#if 0`, as the parser skips them
/// (`scan_skipped_text`). Stop after the name of the `#endif` of the group, or after the condition of its first `#elif`
/// or `#else` that is not false. Return false at the end of the file. O(n) in the length of the lines.
static bool skip_false_branches(TSLexer *lexer) {
    uint32_t depth = 0;
    Name name;
    for (;;) {
        LOOP_STEP();
        skip_rest_of_line(lexer, true);
        if (lexer->eof(lexer)) {
            return false;
        }
        advance(lexer);
        while (is_horizontal_space(lexer->lookahead)) {
            LOOP_STEP();
            advance(lexer);
        }
        if ((lexer->lookahead != '#' && lexer->lookahead != '%') || !read_directive_name(lexer, &name)) {
            continue;
        }
        ConditionalKind kind = conditional_kind(&name);
        if (kind == COND_IF) {
            depth++;
        } else if (kind == COND_ENDIF && depth > 0) {
            depth--;
        } else if (depth == 0 && (kind == COND_ENDIF || (kind == COND_ELSE && !(name_is(&name, "elif") &&
                                                                               condition_is_false(lexer))))) {
            return true;
        }
    }
}

/// Return true when the parser reads no token between the end of the directive line and an `#elif`, `#else`, or
/// `#endif` of a structured group. The lexer is after the directive name.
///
/// Blank lines, comment lines, and the conditional directives of line groups can come before that directive. The
/// parser reads them as extras. For a line group, the function reads the text after the `#else` or `#elif` of the
/// branch that the parser reads as skipped text, up to the `#endif` of the group. O(n) in the length of the lines
/// that the function reads.
static bool branch_ends_after_line(const Scanner *scanner, TSLexer *lexer) {
    // The scanner has no kind for a group that is deeper than MAX_GROUPS. The line then is not the last line of a
    // branch of a structured group.
    if (scanner->deep_groups > 0) {
        return false;
    }
    uint32_t count = scanner->group_count;
    Name name;
    bool percent_read = false;
    while (count > 0 && read_to_next_directive_line(lexer, &name, &percent_read)) {
        LOOP_STEP();
        ConditionalKind kind = conditional_kind(&name);
        uint8_t group = scanner->groups[count - 1];
        if (kind != COND_ELSE && kind != COND_ENDIF) {
            return false;
        }
        if (group == GROUP_STRUCTURED) {
            return true;
        }
        if (group != GROUP_LINE_CHOSEN || (kind == COND_ELSE && !read_to_group_end(lexer))) {
            return false;
        }
        count--;
    }
    return false;
}

// ---------------------------------------------------------------------------------------------------
// The check of a conditional group that the grammar can read as a structured node.
// ---------------------------------------------------------------------------------------------------

/// The way that a token can end the text before a directive.
typedef enum {
    /// No token: the branch is empty, or it holds only directive lines.
    END_NONE,
    /// A `;`, or the `:` of a label or an access specifier.
    END_ITEM,
    /// A `}` after a declaration, a statement, a label, or `{`: the end of a block or a body.
    END_BRACE,
    /// A macro invocation line with no `;`, as `scan_macro_start` reads it: `Q_OBJECT` or `DECLARE_TYPE(T)` before
    /// a line break, at the start of an item.
    END_MACRO,
    /// A `,`. It ends an item only in an enumerator list.
    END_COMMA,
    /// An identifier, a literal, `)`, `]`, or a braced list: the end of an expression. It ends an item only as the
    /// last enumerator of an enumerator list.
    END_VALUE,
    /// A token that leaves a construct incomplete: `=`, `if (x)`, `template <class T>`, `{`.
    END_OPEN,
} EndKind;

/// The state of a macro invocation line that starts an item, as `scan_macro_start` reads it.
typedef enum {
    /// The last token is not a part of a macro invocation line.
    MACRO_NONE,
    /// The last token is a macro name with MACRO_MIN_BARE_LENGTH or more characters.
    MACRO_LONG_NAME,
    /// The last token is a shorter macro name. The name starts an invocation only with arguments on its line.
    MACRO_SHORT_NAME,
    /// The scan is in the arguments of the macro.
    MACRO_ARGUMENTS,
    /// The last token is the `)` of the arguments.
    MACRO_CALL,
} MacroLine;

/// The flags of a group in the group that a scan examines.
enum { NESTED_CHOSEN = 1, NESTED_SKIPPED = 2 };

/// The state of a scan over the text of a conditional group.
typedef struct {
    /// The depth of parentheses, brackets, and braces, from the start of the group.
    int32_t depth[3];
    /// The flags of each open group in the group.
    uint8_t nested[MAX_GROUPS];
    uint32_t nested_count;
    /// The number of open groups in the group whose current branch the scan skips.
    uint32_t skipped_count;
    /// A bit for each open parenthesis: 1 when a control keyword or an attribute keyword is before it.
    uint64_t control_parens;
    /// A bit for each open parenthesis: 1 when `if`, `while`, `for`, or `switch` is before it. A statement comes
    /// after its `)`.
    uint64_t statement_parens;
    uint32_t parens;
    /// The way that the last token ends the text before the next branch directive.
    EndKind end;
    /// A bit for the EndKind of each branch that the scan completed.
    uint32_t branch_ends;
    /// The number of branches that the scan completed.
    uint32_t branch_count;
    /// The way that the first branch ends.
    EndKind first_end;
    /// The index of the first branch after branch 0 that ends with END_VALUE and has a condition that is not false,
    /// or 0.
    uint32_t value_branch;
    /// True when the condition of the current branch is `0` or `false`.
    bool branch_false;
    /// True when the scan cannot record the branches: the groups in the group are too deep.
    bool stopped;
    /// True when a branch ends with a macro invocation line with only names in its arguments, as in
    /// `TEST(Suite, Name)`. A block after it is the body of a function. A block after a call with other arguments is
    /// the body of the macro (`scan_macro_invocation`), or a block after a statement macro:
    /// `BOOST_IF_CONSTEXPR (std::is_integral<T>::value)`.
    bool call_end;
    /// True when the arguments of the macro invocation line of the scan have a token that is not a name or a `,`.
    bool macro_not_names;
    /// True when the group is in an enumerator list.
    bool enumerators;
    /// True until the first token of a branch.
    bool at_branch_start;
    /// True when a line break comes before the current token.
    bool line_break;
    /// True when the current token can start a declaration, a statement, or an enumerator.
    bool at_item_start;
    /// The macro invocation line that the last tokens are a part of.
    MacroLine macro;
    /// The depth of parentheses outside the arguments of the macro.
    int32_t macro_parens;
    /// The number of `?` operators that have no `:` in the branch.
    uint32_t conditionals;
    /// The number of `do` keywords outside all brackets that have no `while` in the branch.
    uint32_t open_do;
    /// True when a token since the start of the item shows that a `{` opens a body: `)`, `namespace`, `class`,
    /// `struct`, `union`, `__interface`, `enum`, `extern`, `else`, `do`, `try`, or `__asm`.
    bool before_body;
    /// A bit for each open brace: 1 when the brace opens a body.
    uint64_t body_braces;
    uint32_t braces;
    /// True when the last token was a control keyword.
    bool after_control_keyword;
    /// True when the last token was `if`, `while`, `for`, or `switch`.
    bool after_statement_keyword;
    /// True when the last token was `]`.
    bool after_bracket;
    /// True when the last token was `default`.
    bool after_default;
    /// True when the group has a `case` or `default` label outside all brackets.
    bool has_case_label;
    /// True when a check failed.
    bool failed;
} GroupScan;

/// The keywords that come before a parenthesized part that does not end a construct: `if (x)` needs a body.
static const char *const CONTROL_KEYWORDS[] = {
    "if",       "while",    "for",      "switch",   "catch",         "__attribute__", "__attribute",
    "alignas",  "_Alignas", "__declspec", "decltype", "noexcept",    "throw",         "sizeof",
    "alignof",  "typeid",   "static_assert", "requires", "explicit", "__typeof__",    "typeof",
    "__if_exists", "__if_not_exists", "_Static_assert", NULL,
};

/// The reserved words that do not end a declaration or a statement.
static const char *const OPEN_KEYWORDS[] = {
    "if",        "else",         "for",          "while",        "do",          "switch",      "case",
    "return",    "goto",         "throw",        "co_return",    "co_yield",    "co_await",    "new",
    "delete",    "sizeof",       "alignof",      "typeid",       "decltype",    "noexcept",    "static_cast",
    "dynamic_cast", "const_cast", "reinterpret_cast", "static",   "extern",      "inline",      "virtual",
    "explicit",  "friend",       "typedef",      "using",        "namespace",   "template",    "typename",
    "class",     "struct",       "union",        "enum",         "const",       "volatile",    "mutable",
    "register",  "thread_local", "constexpr",    "constinit",    "consteval",   "auto",        "signed",
    "unsigned",  "short",        "long",         "int",          "char",        "bool",        "float",
    "double",    "void",         "wchar_t",      "char8_t",      "char16_t",    "char32_t",    "operator",
    "public",    "private",      "protected",    "requires",     "concept",     "try",         "__interface",
    "__if_exists", "__if_not_exists", NULL,
};

/// The reserved words that do not start a declaration or a statement.
static const char *const CONTINUING_KEYWORDS[] = {"else", "catch", NULL};

/// The reserved words after which a `{` of the same item opens a body, not a braced list.
static const char *const BODY_KEYWORDS[] = {
    "namespace", "class", "struct", "union", "__interface", "enum", "extern", "else", "do", "try", "__asm", NULL,
};

/// The words with the shape of a macro name that `scan_macro_start` does not read as a macro invocation: the
/// grammar tokens TRUE, FALSE, and NULL, and the Qt statement words.
static const char *const NOT_MACRO_WORDS[] = {"TRUE", "FALSE", "NULL", "Q_EMIT", "Q_FOREACH", "Q_FOREVER", NULL};

/// The keywords before a parenthesized condition or loop head, after which a statement comes.
static const char *const STATEMENT_HEAD_KEYWORDS[] = {"if", "while", "for", "switch", NULL};

/// Record a token that ends the text before a directive in the way END. Fail when the token is the first
/// token of a branch and it continues a construct.
static void add_token(GroupScan *scan, EndKind end, bool continues_construct) {
    if (scan->at_branch_start && continues_construct) {
        scan->failed = true;
    }
    scan->at_branch_start = false;
    scan->end = end;
    scan->line_break = false;
    scan->at_item_start = false;
    scan->after_control_keyword = false;
    scan->after_statement_keyword = false;
    scan->after_bracket = false;
    scan->after_default = false;
}

/// True when the scan is outside all parentheses, brackets, and braces that the group opened.
static bool at_top_level(const GroupScan *scan) {
    return scan->depth[0] == 0 && scan->depth[1] == 0 && scan->depth[2] == 0;
}

/// True when the scan is outside all parentheses and brackets that the group opened. An item can start there.
static bool outside_parentheses(const GroupScan *scan) {
    return scan->depth[0] == 0 && scan->depth[1] == 0;
}

/// Record the start of a token whose first character is C, before the token itself.
///
/// A `(` after a macro name opens its arguments. A long name can have its arguments on the next line. A line
/// break after a macro name or its arguments ends a macro invocation line, as in `scan_macro_start`. The token
/// after that line break starts an item. In an enumerator list, a name with no arguments is an enumerator.
static void start_token(GroupScan *scan, int32_t c) {
    bool arguments = scan->macro == MACRO_LONG_NAME || (scan->macro == MACRO_SHORT_NAME && !scan->line_break);
    if (c == '(' && arguments) {
        scan->macro = MACRO_ARGUMENTS;
        scan->macro_parens = scan->depth[0];
        return;
    }
    if (scan->macro == MACRO_ARGUMENTS) {
        return;
    }
    bool line_end = scan->macro == MACRO_CALL || (scan->macro == MACRO_LONG_NAME && !scan->enumerators);
    if (scan->line_break && line_end) {
        scan->end = END_MACRO;
        scan->at_item_start = true;
        scan->before_body = false;
    }
    scan->macro = MACRO_NONE;
}

/// Record a closing bracket of kind INDEX, and fail when it closes a bracket that the group did not open.
static void close_bracket(GroupScan *scan, int index) {
    scan->depth[index]--;
    if (scan->depth[index] < 0) {
        scan->failed = true;
    }
}

/// Record a `{` or `<%`. The brace opens a body when a token since the start of the item shows it, as `)` in
/// `void f() {` or `namespace` in `namespace n {`.
static void add_open_brace(GroupScan *scan) {
    bool body = scan->before_body;
    add_token(scan, END_OPEN, false);
    if (scan->braces < 64) {
        scan->body_braces = (scan->body_braces << 1) | (body ? 1 : 0);
    }
    scan->braces++;
    scan->depth[2]++;
    scan->at_item_start = outside_parentheses(scan);
    scan->before_body = false;
}

/// Record a `}` or `%>`. The braces of a body close a body. Other braces close a braced list when an expression
/// or a `,` comes before the `}`, as in `{1, 2}`. Otherwise they close a block, as in `{ f(); }`.
static void add_close_brace(GroupScan *scan) {
    bool body = false;
    if (scan->braces > 0) {
        scan->braces--;
        if (scan->braces < 64) {
            body = (scan->body_braces & 1) != 0;
            scan->body_braces >>= 1;
        }
    }
    bool list = !body && (scan->end == END_VALUE || scan->end == END_COMMA);
    add_token(scan, list ? END_VALUE : END_BRACE, false);
    close_bracket(scan, 2);
    scan->at_item_start = outside_parentheses(scan);
    scan->before_body = false;
}

/// Read one punctuator and record it. The lookahead is the first character of the punctuator.
static void add_punctuator(TSLexer *lexer, GroupScan *scan) {
    int32_t c = lexer->lookahead;
    advance(lexer);
    int32_t next = lexer->lookahead;
    switch (c) {
        case '(': {
            // add_token clears the flags of the keyword, so the flags are read first.
            bool control = scan->after_control_keyword;
            bool statement = scan->after_statement_keyword;
            add_token(scan, END_OPEN, false);
            if (scan->parens < 64) {
                scan->control_parens = (scan->control_parens << 1) | (control ? 1 : 0);
                scan->statement_parens = (scan->statement_parens << 1) | (statement ? 1 : 0);
            }
            scan->parens++;
            scan->depth[0]++;
            return;
        }
        case ')': {
            bool control = false;
            bool statement = false;
            if (scan->parens > 0) {
                scan->parens--;
                if (scan->parens < 64) {
                    control = (scan->control_parens & 1) != 0;
                    statement = (scan->statement_parens & 1) != 0;
                    scan->control_parens >>= 1;
                    scan->statement_parens >>= 1;
                }
            }
            add_token(scan, control ? END_OPEN : END_VALUE, false);
            close_bracket(scan, 0);
            if (scan->macro == MACRO_ARGUMENTS && scan->depth[0] == scan->macro_parens) {
                scan->macro = MACRO_CALL;
            }
            scan->before_body = true;
            // The statement of `if (x)` can be a macro invocation line: `if (x)`, `CHECK(y)`.
            scan->at_item_start = statement && outside_parentheses(scan);
            return;
        }
        case '[':
            add_token(scan, END_OPEN, false);
            scan->depth[1]++;
            return;
        case ']': {
            bool attribute_end = scan->after_bracket;
            add_token(scan, attribute_end ? END_OPEN : END_VALUE, false);
            scan->after_bracket = true;
            close_bracket(scan, 1);
            return;
        }
        case '{':
            add_open_brace(scan);
            return;
        case '}':
            add_close_brace(scan);
            return;
        case ';':
            add_token(scan, END_ITEM, false);
            scan->at_item_start = outside_parentheses(scan);
            scan->before_body = false;
            return;
        case ':':
            if (next == ':') {
                advance(lexer);
                add_token(scan, END_OPEN, false);
            } else if (next == '>') {
                advance(lexer);
                add_token(scan, END_VALUE, false);
                close_bracket(scan, 1);
            } else if (scan->conditionals > 0) {
                // The `:` of a conditional operator needs its third operand.
                scan->conditionals--;
                add_token(scan, END_OPEN, true);
            } else {
                if (scan->after_default && at_top_level(scan)) {
                    scan->has_case_label = true;
                }
                add_token(scan, END_ITEM, true);
                scan->at_item_start = outside_parentheses(scan);
            }
            return;
        case '<':
            if (next == '%') {
                advance(lexer);
                add_open_brace(scan);
                return;
            }
            if (next == ':') {
                advance(lexer);
                if (lexer->lookahead == ':' || lexer->lookahead == '>') {
                    // `<::` is `<` and `::`.
                    add_token(scan, END_OPEN, true);
                    return;
                }
                add_token(scan, END_OPEN, false);
                scan->depth[1]++;
                return;
            }
            add_token(scan, END_OPEN, true);
            while (lexer->lookahead == '<' || lexer->lookahead == '=') {
                LOOP_STEP();
                advance(lexer);
            }
            return;
        case '%':
            if (next == '>') {
                advance(lexer);
                add_close_brace(scan);
                return;
            }
            add_token(scan, END_OPEN, true);
            return;
        case '-':
            if (next == '>' || next == '-' || next == '=') {
                advance(lexer);
                add_token(scan, END_OPEN, next == '>');
                return;
            }
            add_token(scan, END_OPEN, false);
            return;
        case '+':
        case '*':
        case '&':
        case '!':
        case '~':
            if ((c == '+' || c == '&') && next == c) {
                advance(lexer);
            }
            add_token(scan, END_OPEN, c == '&' && next == '&');
            if (lexer->lookahead == '=') {
                advance(lexer);
            }
            return;
        case '.':
            add_token(scan, END_OPEN, true);
            return;
        case ',':
            add_token(scan, END_COMMA, true);
            scan->at_item_start = scan->enumerators && outside_parentheses(scan);
            return;
        default:
            // `=`, `==`, `!=`, `>`, `>=`, `>>`, `|`, `||`, `^`, `?`, `#` in a macro, and the compound assignments.
            add_token(scan, END_OPEN, c != '#' && c != '@' && c != '`');
            if (c == '?') {
                scan->conditionals++;
            }
            // A `{` after `=` opens a braced list, also in `struct S s = {1, 2}`.
            if (c == '=' && next != '=') {
                scan->before_body = false;
            }
            while (lexer->lookahead == '=' || lexer->lookahead == '>' || lexer->lookahead == '|') {
                LOOP_STEP();
                advance(lexer);
            }
            return;
    }
}

/// Read one token that starts with an identifier character, and record it.
static void add_word(TSLexer *lexer, GroupScan *scan) {
    Name name;
    read_name(lexer, &name);
    if (lexer->lookahead == '"' && name_is_one_of(&name, RAW_STRING_PREFIXES)) {
        if (!skip_raw_string_literal(lexer)) {
            scan->failed = true;
        }
        add_token(scan, END_VALUE, false);
        return;
    }
    if ((lexer->lookahead == '"' || lexer->lookahead == '\'') &&
        (name_is(&name, "L") || name_is(&name, "u") || name_is(&name, "U") || name_is(&name, "u8"))) {
        if (!skip_quoted_literal(lexer)) {
            scan->failed = true;
        }
        add_token(scan, END_VALUE, false);
        return;
    }
    bool open = name_is_one_of(&name, OPEN_KEYWORDS);
    bool continuing = name_is_one_of(&name, CONTINUING_KEYWORDS);
    bool control = name_is_one_of(&name, CONTROL_KEYWORDS);
    bool item_start = scan->at_item_start;
    add_token(scan, open ? END_OPEN : END_VALUE, continuing);
    scan->after_control_keyword = control;
    scan->after_statement_keyword = name_is_one_of(&name, STATEMENT_HEAD_KEYWORDS);
    scan->at_item_start = (name_is(&name, "else") || name_is(&name, "do")) && outside_parentheses(scan);
    scan->after_default = name_is(&name, "default");
    if (name_is_one_of(&name, BODY_KEYWORDS) && outside_parentheses(scan)) {
        scan->before_body = true;
    }
    if (name_is(&name, "case") && at_top_level(scan)) {
        scan->has_case_label = true;
    }
    if (item_start && name.macro_shaped && !name_is_one_of(&name, NOT_MACRO_WORDS)) {
        scan->macro = name.length >= MACRO_MIN_BARE_LENGTH ? MACRO_LONG_NAME : MACRO_SHORT_NAME;
        scan->macro_not_names = false;
    }
    // A `do` statement ends with the `;` after its `while` condition.
    if (name_is(&name, "do") && at_top_level(scan)) {
        scan->open_do++;
    } else if (name_is(&name, "while") && at_top_level(scan) && scan->open_do > 0) {
        scan->open_do--;
    }
}

/// Read a preprocessing number, with digit separators and exponent signs.
static void add_number(TSLexer *lexer, GroupScan *scan) {
    skip_number_literal(lexer);
    add_token(scan, END_VALUE, false);
}

/// Check the text before a branch directive of the group: its last token, the depths, and the `do` statements.
/// Record the way that the branch ends. The directive line is a line break after a macro invocation.
static void end_branch(GroupScan *scan) {
    if (scan->macro == MACRO_CALL || (scan->macro == MACRO_LONG_NAME && !scan->enumerators)) {
        scan->end = END_MACRO;
        scan->call_end |= scan->macro == MACRO_CALL && !scan->macro_not_names;
    }
    if (scan->end == END_OPEN || !at_top_level(scan) || scan->open_do > 0) {
        scan->failed = true;
    }
    if (scan->branch_count == 0) {
        scan->first_end = scan->end;
    } else if (scan->end == END_VALUE && !scan->branch_false && scan->value_branch == 0) {
        scan->value_branch = scan->branch_count;
    }
    scan->branch_count++;
    scan->branch_ends |= 1U << scan->end;
    scan->end = END_NONE;
    scan->at_branch_start = true;
    scan->at_item_start = true;
    scan->before_body = false;
    scan->macro = MACRO_NONE;
    scan->conditionals = 0;
    scan->after_control_keyword = false;
    scan->after_bracket = false;
    scan->after_default = false;
}

/// Record a conditional directive of a group in the group. The lexer is after the directive name.
static void add_nested_directive(TSLexer *lexer, GroupScan *scan, const Name *name) {
    switch (conditional_kind(name)) {
        case COND_IF: {
            if (scan->nested_count == MAX_GROUPS) {
                scan->failed = true;
                scan->stopped = true;
                return;
            }
            uint8_t flags;
            if (scan->skipped_count > 0) {
                flags = NESTED_CHOSEN | NESTED_SKIPPED;
            } else if (name_is(name, "if") && condition_is_false(lexer)) {
                flags = NESTED_SKIPPED;
            } else {
                flags = NESTED_CHOSEN;
            }
            scan->nested[scan->nested_count++] = flags;
            if (flags & NESTED_SKIPPED) {
                scan->skipped_count++;
            }
            return;
        }
        case COND_ELSE: {
            uint8_t *flags = &scan->nested[scan->nested_count - 1];
            bool was_skipped = (*flags & NESTED_SKIPPED) != 0;
            bool skip_now;
            if (*flags & NESTED_CHOSEN) {
                skip_now = true;
            } else {
                skip_now = name_is(name, "elif") && condition_is_false(lexer);
                if (!skip_now) {
                    *flags |= NESTED_CHOSEN;
                }
            }
            // A group in a skipped branch stays skipped: its flags have both bits.
            if (was_skipped && !skip_now) {
                *flags &= (uint8_t)~NESTED_SKIPPED;
                scan->skipped_count--;
            } else if (!was_skipped && skip_now) {
                *flags |= NESTED_SKIPPED;
                scan->skipped_count++;
            }
            return;
        }
        case COND_ENDIF:
            if (scan->nested[scan->nested_count - 1] & NESTED_SKIPPED) {
                scan->skipped_count--;
            }
            scan->nested_count--;
            return;
        case COND_NONE:
            return;
    }
}

/// The kind of the first token after a group.
typedef enum {
    /// A token that can start a declaration, a statement, or an enumerator, or the end of the file.
    AFTER_ITEM_START,
    /// `{` or `<%`, which can start a block, or continue the construct before it with a body or a braced list.
    AFTER_OPEN_BRACE,
    /// `;`, which can also end the construct of a branch that ends with a brace: `struct A {}`.
    AFTER_SEMICOLON,
    /// `}` or `%>`.
    AFTER_CLOSE_BRACE,
    /// A token that cannot start a declaration or a statement, and cannot come after a `;`. It continues the construct
    /// before it: `noexcept`, `:`, `,`, `=`, `]`, `?`, `.`, `->`, or a binary operator.
    AFTER_CONTINUATION,
    /// `else` or `catch`, which continue a statement after its `;` or its `}`.
    AFTER_CONTINUING_KEYWORD,
    /// `)`, which continues a construct, also after the `;` of a `for` head.
    AFTER_CLOSE_PARENTHESIS,
} TokenAfter;

/// Read the first token after a group, and return its kind. The lexer is after the name of the `#endif`.
///
/// Directive lines, blank lines, and comment lines can come before that token. The `#elif`, `#else`, or `#endif`
/// of a structured group around the group ends the branch, as a `}` ends a block. After the `#elif` or `#else`
/// of a line group around the group, the parser skips the text up to the `#endif`. In a group after the group, the
/// parser reads the first branch that is not false, as in `#if 0` (`skip_false_branches`). The list of the tokens that
/// continue a construct is the list of clang-format `tokenCanStartNewLine`, with `,`, `)`, `]`, `?`, and the binary
/// operators. O(n) in the length of the lines that the function reads.
static TokenAfter read_token_after_group(const Scanner *scanner, TSLexer *lexer) {
    uint32_t count = scanner->group_count;
    Name name;
    bool percent_read = false;
    while (read_to_next_directive_line(lexer, &name, &percent_read)) {
        LOOP_STEP();
        ConditionalKind kind = conditional_kind(&name);
        if (kind == COND_IF && name_is(&name, "if") && condition_is_false(lexer) && !skip_false_branches(lexer)) {
            return AFTER_ITEM_START;
        }
        if (kind == COND_IF || kind == COND_NONE) {
            continue;
        }
        if (count == 0) {
            return AFTER_ITEM_START;
        }
        if (scanner->groups[count - 1] == GROUP_STRUCTURED) {
            return AFTER_CLOSE_BRACE;
        }
        if (kind == COND_ELSE && !read_to_group_end(lexer)) {
            return AFTER_ITEM_START;
        }
        count--;
    }
    if (lexer->eof(lexer)) {
        return AFTER_ITEM_START;
    }
    // The scan read the `%` of a line that starts with the digraph `%>`, which is the token `}`.
    if (percent_read && lexer->lookahead == '>') {
        return AFTER_CLOSE_BRACE;
    }
    int32_t c = lexer->lookahead;
    if (is_word_start(c)) {
        read_name(lexer, &name);
        if (name_is_one_of(&name, CONTINUING_KEYWORDS)) {
            return AFTER_CONTINUING_KEYWORD;
        }
        return name_is(&name, "noexcept") ? AFTER_CONTINUATION : AFTER_ITEM_START;
    }
    advance(lexer);
    int32_t next = lexer->lookahead;
    switch (c) {
        case ';':
            return AFTER_SEMICOLON;
        case '{':
            return AFTER_OPEN_BRACE;
        case '}':
            return AFTER_CLOSE_BRACE;
        case '<':
            // `<%` is `{`, and `<:` is `[`, which can start an attribute.
            return next == '%' ? AFTER_OPEN_BRACE : next == ':' ? AFTER_ITEM_START : AFTER_CONTINUATION;
        case '%':
            // `%>` is `}`, and `%:` is `#`.
            return next == '>' ? AFTER_CLOSE_BRACE : next == ':' ? AFTER_ITEM_START : AFTER_CONTINUATION;
        case ':':
            return next == ':' ? AFTER_ITEM_START : AFTER_CONTINUATION;
        case '.':
            // `.5` is a number.
            return is_digit(next) ? AFTER_ITEM_START : AFTER_CONTINUATION;
        case '-':
            return next == '>' || next == '=' ? AFTER_CONTINUATION : AFTER_ITEM_START;
        case '+':
        case '*':
        case '&':
        case '!':
            return next == '=' ? AFTER_CONTINUATION : AFTER_ITEM_START;
        case ')':
            return AFTER_CLOSE_PARENTHESIS;
        case ',':
        case '=':
        case ']':
        case '?':
        case '>':
        case '/':
        case '|':
        case '^':
            return AFTER_CONTINUATION;
        default:
            return AFTER_ITEM_START;
    }
}

/// Return true when the parser can read the branch ends of a group before the token after the group.
///
/// In an enumerator list, a branch ends with `,`, with a macro invocation, or with no token. The last enumerator
/// has no `,` only before the `}` of the list (GCC `cp_parser_enumerator_list`, Clang `ParseEnumBody`).
/// Elsewhere a branch ends with a complete item, or with no token: `;`, a label, a block or a body, or a macro
/// invocation line. The `;` after a braced construct continues that construct, as in `struct A {}`.
///
/// In a structured group, the macro scan reads a macro invocation line before the `#elif`, `#else`, or `#endif` of
/// the group as a complete item, as before a `}`. The macro scan goes past the directives of a line group, and it
/// reads the token after the group. A `;` after the macro then ends the call, and a `{` after its arguments starts its
/// block or the body of a function (`scan_macro_invocation`, clang-format `tokenCanStartNewLine`). For this reason, a
/// branch that ends with a macro invocation line does not fit before `;`. A branch that ends with a macro call with
/// only names in its arguments, as `TEST(Suite, Name)`, does not fit before `{`. The group is then a line group, as
/// for the preprocessor. A macro name with no arguments before a block, as `RANGES_DIAGNOSTIC_PUSH` before `{`, is a
/// pragma or a statement macro. A macro call with other arguments before a block, as
/// `BOOST_IF_CONSTEXPR (x::value)`, is also a statement macro.
static bool branch_ends_fit(const GroupScan *scan, TokenAfter after) {
    uint32_t ends = scan->branch_ends;
    if (after == AFTER_CONTINUATION || after == AFTER_CONTINUING_KEYWORD || after == AFTER_CLOSE_PARENTHESIS) {
        return false;
    }
    if (scan->enumerators) {
        if (ends & ((1U << END_ITEM) | (1U << END_BRACE))) {
            return false;
        }
        return !(ends & (1U << END_VALUE)) || after == AFTER_CLOSE_BRACE;
    }
    if (ends & ((1U << END_COMMA) | (1U << END_VALUE))) {
        return false;
    }
    if ((after == AFTER_SEMICOLON && (ends & (1U << END_MACRO))) || (after == AFTER_OPEN_BRACE && scan->call_end)) {
        return false;
    }
    return after != AFTER_SEMICOLON || !(ends & (1U << END_BRACE));
}

/// Return the index of the branch that the parser reads as code in a group of directive lines. 0 is the first branch.
///
/// The preprocessor keeps one branch, and the tokens after the group continue that branch. The parser reads the first
/// branch when it fits the token after the group. A branch that ends with `;` or a label does not fit before a `{` in
/// a class, a namespace, or at the top level, and never fits before a `:`, a `,`, an `=`, or a binary operator. Before
/// a `;`, it leaves an empty declaration or an empty statement, and in `if (x) x = !x; ; else` the `else` has no
/// `if`. A branch that ends with a name, a literal, `)`, or `]` fits before these tokens: a class head or a function
/// head before its body, `struct key_compare : iless` before `{` in Boost.Beast. When the first branch ends with `;`
/// or a label before such a token, the parser reads the first later branch that ends with an expression and has a
/// condition that is not false. The other branches stay `preproc_skipped` nodes. clang-format also parses the branches
/// with the tokens after the group (`UnwrappedLineParser::parse`). O(1).
static uint32_t select_line_branch(const GroupScan *scan, TokenAfter after) {
    if (scan->enumerators || scan->first_end != END_ITEM || scan->value_branch > MAX_SELECTED_BRANCH) {
        return 0;
    }
    bool first_fits = after != AFTER_OPEN_BRACE && after != AFTER_CONTINUATION && after != AFTER_SEMICOLON;
    return first_fits ? 0 : scan->value_branch;
}

/// Read the condition of an `#elif` after the directive name. Return true when the condition is the one token `0` or
/// `false` (`condition_is_false`). Set `empty` when the condition has only spaces and comments (`condition_is_empty`).
static bool elif_condition_is_false(TSLexer *lexer, bool *empty) {
    while (is_horizontal_space(lexer->lookahead)) {
        LOOP_STEP();
        advance(lexer);
    }
    if (is_word_char(lexer->lookahead)) {
        return condition_is_false(lexer);
    }
    *empty = condition_is_empty(lexer);
    return false;
}

/// Scan the text of a conditional group from the end of its first directive name.
///
/// Return true when the parser can read the group as a structured node. A structured node puts the tokens of
/// each branch into complete items of the construct around the group. The preprocessor gives the parser the
/// tokens of one branch and the tokens after the group together (libcpp `_cpp_lex_token`, Clang
/// `SkipExcludedConditionalBlock`). For this reason, a structured node is correct only when each branch, with the
/// tokens after the group, is a sequence of complete items. clang-format also parses each branch with the
/// tokens after the group (`UnwrappedLineParser::parse`). Where one branch ends in a construct, the directives
/// are lines, and the tokens after the `#endif` continue the construct, as for the preprocessor.
/// - The brackets of each branch balance, and no branch closes a bracket that it did not open.
/// - Each branch ends with a complete item or with no token (`branch_ends_fit`). A `;`, a label, a body, a block,
///   and a macro invocation line end an item. An expression, for example `void f(int a)`, `HANDLE`, `extern "C"`,
///   or the braced list `{1, 2}`, does not. A macro invocation line is an uppercase name at the start of an item
///   or after `if (x)`, with or without arguments, before a line break, as `scan_macro_start` reads it.
/// - No branch starts with a token that continues a construct, for example `else`, `,`, or `: base`.
/// - The token after the group can come after each branch end (`read_token_after_group`). An `else`, a
///   `catch`, a `:`, or a `,` continues the construct of the last branch.
/// - Each literal ends on its line, and the group has its `#endif`.
/// A group in the group adds the text of the branch that a line group reads as code: the first branch
/// that is not `#if 0`. The scanner reads such a group as lines or as a node when the parser gets to it.
/// `enumerators` is true for a group in an enumerator list. `scanner` holds the groups around the group. The
/// function sets `has_case_label` to true when a branch has a `case` or `default` label outside all brackets.
///
/// For a group that is not structured, the scan continues to the `#endif`, and it sets `selected_branch` to the index
/// of the branch that the parser reads as code (`select_line_branch`). O(n) in the length of the group.
static bool group_is_structured(const Scanner *scanner, TSLexer *lexer, bool enumerators, bool *has_case_label,
                                uint32_t *selected_branch) {
    GroupScan scan = {0};
    scan.at_branch_start = true;
    scan.at_item_start = true;
    scan.enumerators = enumerators;
    skip_rest_of_line(lexer, false);
    bool line_start = false;
    for (;;) {
        LOOP_STEP();
        if (scan.stopped || lexer->eof(lexer)) {
            return false;
        }
        int32_t c = lexer->lookahead;
        if (is_line_break(c)) {
            advance(lexer);
            line_start = true;
            scan.line_break = true;
            continue;
        }
        if (is_horizontal_space(c)) {
            advance(lexer);
            continue;
        }
        if (c == '\\') {
            skip_backslash(lexer, false);
            continue;
        }
        if (c == '/') {
            advance(lexer);
            if (lexer->lookahead == '*') {
                if (skip_block_comment(lexer)) {
                    line_start = false;
                    scan.line_break = true;
                }
                continue;
            }
            if (lexer->lookahead == '/') {
                advance(lexer);
                skip_line_comment(lexer);
                continue;
            }
            if (scan.skipped_count == 0) {
                start_token(&scan, c);
                add_token(&scan, END_OPEN, true);
            }
            line_start = false;
            continue;
        }
        if (line_start && (c == '#' || c == '%')) {
            line_start = false;
            Name name;
            if (!read_directive_name(lexer, &name)) {
                if (scan.skipped_count == 0) {
                    start_token(&scan, c);
                    add_token(&scan, END_OPEN, true);
                }
                continue;
            }
            ConditionalKind kind = conditional_kind(&name);
            if (scan.nested_count == 0 && kind == COND_ELSE) {
                end_branch(&scan);
                // The grammar has no `preproc_elif` without a condition. After a branch that they read, GCC
                // `do_elif` and Clang `HandleElifFamilyDirective` ignore that condition, and it can be empty.
                bool empty = false;
                scan.branch_false = name_is(&name, "elif") && elif_condition_is_false(lexer, &empty);
                if (empty) {
                    scan.failed = true;
                }
            } else if (scan.nested_count == 0 && kind == COND_ENDIF) {
                end_branch(&scan);
                *has_case_label = scan.has_case_label;
                TokenAfter after = read_token_after_group(scanner, lexer);
                bool structured = !scan.failed && branch_ends_fit(&scan, after);
                *selected_branch = structured ? 0 : select_line_branch(&scan, after);
                return structured;
            } else {
                add_nested_directive(lexer, &scan, &name);
            }
            // The token of a directive line of the parser ends at the line break, also in the body of a macro
            // with a raw string literal: `#define M(D) \` and `D(x, R"(` in ClickHouse src/Core/Settings.cpp.
            // The scan reads the lines after such a line as the parser reads them.
            skip_rest_of_line(lexer, false);
            continue;
        }
        line_start = false;
        if (scan.skipped_count > 0) {
            if (c == '"' || c == '\'') {
                skip_quoted_literal(lexer);
            } else if (is_word_start(c)) {
                // The parser reads a nested structured group with all its branches. A case label in a skipped
                // branch then also ends a case body. The scan does not count brackets in this text.
                Name name;
                read_name(lexer, &name);
                if (name_is(&name, "default")) {
                    while (is_horizontal_space(lexer->lookahead)) {
                        LOOP_STEP();
                        advance(lexer);
                    }
                    if (lexer->lookahead == ':') {
                        advance(lexer);
                        scan.has_case_label = scan.has_case_label || lexer->lookahead != ':';
                    }
                } else if (name_is(&name, "case")) {
                    scan.has_case_label = true;
                }
            } else {
                advance(lexer);
            }
            continue;
        }
        // MACRO_ARGUMENTS starts after the `(` of the arguments of a macro, and the `)` that closes them ends it.
        bool in_arguments = scan.macro == MACRO_ARGUMENTS;
        start_token(&scan, c);
        bool closes_arguments = c == ')' && scan.depth[0] == scan.macro_parens + 1;
        scan.macro_not_names |= in_arguments && !(is_word_start(c) || c == ',' || closes_arguments);
        if (c == '"' || c == '\'') {
            if (!skip_quoted_literal(lexer)) {
                scan.failed = true;
            }
            add_token(&scan, END_VALUE, false);
        } else if (c >= '0' && c <= '9') {
            add_number(lexer, &scan);
        } else if (is_word_char(c)) {
            add_word(lexer, &scan);
        } else {
            add_punctuator(lexer, &scan);
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The directive tokens.
// ---------------------------------------------------------------------------------------------------

/// Record a new open conditional group.
///
/// The scanner records the kind of the first MAX_GROUPS groups. It counts each group that is deeper, and it reads
/// the directives of such a group as lines. The count stops at UINT16_MAX, and a text with more groups than that
/// closes the groups of the array too early.
static void push_group(Scanner *scanner, GroupKind kind) {
    if (scanner->group_count < MAX_GROUPS) {
        scanner->groups[scanner->group_count++] = (uint8_t)kind;
    } else if (scanner->deep_groups < UINT16_MAX) {
        scanner->deep_groups++;
    }
}

/// The kind of the innermost open group, or 0 when no group is open.
static uint8_t innermost_group(const Scanner *scanner) {
    if (scanner->deep_groups > 0) {
        return GROUP_LINE_CHOSEN;
    }
    return scanner->group_count > 0 ? scanner->groups[scanner->group_count - 1] : 0;
}

/// Scan the text of the branches that a line group does not read as code, from the end of a directive
/// name or from the start of a line.
///
/// With STOP_AT_BRANCH, the text ends before the next `#elif`, `#else`, or `#endif` of the group. Otherwise
/// it ends before the `#endif` of the group. The line of that directive is not in the token.
static void scan_skipped_text(TSLexer *lexer, bool stop_at_branch, uint32_t depth) {
    skip_rest_of_line(lexer, true);
    mark_end(lexer);
    for (;;) {
        LOOP_STEP();
        if (lexer->eof(lexer)) {
            mark_end(lexer);
            return;
        }
        // The lookahead is the line break at the end of a line.
        advance(lexer);
        mark_end(lexer);
        while (is_horizontal_space(lexer->lookahead)) {
            LOOP_STEP();
            advance(lexer);
        }
        if (lexer->lookahead == '#' || lexer->lookahead == '%') {
            Name name;
            if (read_directive_name(lexer, &name)) {
                ConditionalKind kind = conditional_kind(&name);
                if (kind == COND_IF) {
                    depth++;
                } else if (depth == 0 && (kind == COND_ENDIF || (kind == COND_ELSE && stop_at_branch))) {
                    return;
                } else if (kind == COND_ENDIF) {
                    depth--;
                }
            }
        }
        skip_rest_of_line(lexer, true);
    }
}

/// Scan a directive at the start of a line, or the skipped text of a line group.
///
/// A `#` starts a directive only as the first token of a line (libcpp `_cpp_handle_directive`, Clang
/// `Preprocessor::HandleDirective`). The token is the `#` or `%:`, the spaces, and the name. For the
/// null directive the token is the `#` alone. `space` holds the white space before the token.
///
/// The token of a conditional directive depends on its group. The directives of a structured group
/// have the tokens of `preproc_if`, also where the parse state has no action for them: the parser then
/// recovers at the directive. The directives of a line group are a `preproc_call` or a `preproc_skipped`.
static bool scan_directive(Scanner *scanner, TSLexer *lexer, const bool *valid_symbols, Space space) {
    bool after_line_break = space.line_break;
    uint32_t spaces = space.spaces;

    // In the false branch of a line group, each text up to the next directive of the group is skipped.
    bool pending = is_pending_group(innermost_group(scanner)) && valid_symbols[PREPROC_SKIPPED];
    // At the end of the input, a false branch has no more text to skip. An empty skipped text is an extra that does
    // not change the parse state, and the runtime ignores it.
    if (lexer->eof(lexer)) {
        return false;
    }
    // A `#` after a comment on its line is also a directive where a declaration can start, as in
    // `/*<-*/ #include "x.hpp"`. In a string literal a declaration cannot start.
    bool item_position = valid_symbols[PREPROC_IF];
    bool is_directive = false;
    bool at_line_start = false;
    if (lexer->lookahead == '#') {
        at_line_start = after_line_break || lexer->get_column(lexer) == spaces;
        is_directive = at_line_start || item_position;
    } else if (lexer->lookahead == '%') {
        advance(lexer);
        // Only `%:` is a directive. The `:` comes before the column, because `get_column` reads the
        // line of the token, and a `%` of a modulo operator then reads the line for nothing.
        if (lexer->lookahead == ':') {
            at_line_start = after_line_break || lexer->get_column(lexer) == spaces + 1;
            is_directive = at_line_start || item_position;
        }
    }
    if (!is_directive && !pending) {
        return false;
    }

    Name name = {0};
    if (is_directive) {
        advance(lexer);
        if (lexer->lookahead == '#' || lexer->lookahead == '%') {
            // `##` or `%:%:` is the paste operator of a macro body, not a directive.
            if (!pending) {
                return false;
            }
            is_directive = false;
        } else {
            mark_end(lexer);
            while (is_horizontal_space(lexer->lookahead)) {
                LOOP_STEP();
                advance(lexer);
            }
            read_name(lexer, &name);
            if (!at_line_start && name.length == 0 && !pending) {
                return false;
            }
        }
    }
    ConditionalKind kind = is_directive ? conditional_kind(&name) : COND_NONE;
    if (pending && kind != COND_ELSE && kind != COND_ENDIF) {
        scan_skipped_text(lexer, true, kind == COND_IF ? 1 : 0);
        lexer->result_symbol = PREPROC_SKIPPED;
        return true;
    }
    if (name.length > 0) {
        mark_end(lexer);
    }

    {
        enum TokenType type;
        uint8_t group = innermost_group(scanner);
        switch (kind) {
            case COND_IF: {
                enum TokenType structured = name_is(&name, "if")      ? PREPROC_IF
                                            : name_is(&name, "ifdef") ? PREPROC_IFDEF
                                                                      : PREPROC_IFNDEF;
                enum TokenType in_case = name_is(&name, "if")      ? PREPROC_IF_IN_CASE
                                         : name_is(&name, "ifdef") ? PREPROC_IFDEF_IN_CASE
                                                                   : PREPROC_IFNDEF_IN_CASE;
                bool is_false = name_is(&name, "if") && condition_is_false(lexer);
                // A case body reads a group with no case label. A group with a case label ends the case statement.
                // In error recovery each token is valid, and the scanner gives the usual token. Only an item of an
                // enumerator list can start with the macro enumerator token.
                bool error_recovery = valid_symbols[RAW_STRING_DELIMITER] && valid_symbols[RAW_STRING_CONTENT];
                bool enumerators = valid_symbols[MACRO_ENUMERATOR_START] && !error_recovery;
                bool has_case_label = false;
                uint32_t selected_branch = 0;
                // Where no item can start, the group is a line group, and its scan only selects a branch.
                bool item = valid_symbols[structured] || valid_symbols[in_case];
                bool is_structured = scanner->group_count < MAX_GROUPS && (item || !is_false) &&
                                     group_is_structured(scanner, lexer, enumerators, &has_case_label,
                                                         &selected_branch) &&
                                     item;
                if (is_structured && valid_symbols[in_case] && !has_case_label && !error_recovery) {
                    push_group(scanner, GROUP_STRUCTURED);
                    type = in_case;
                } else if (is_structured && valid_symbols[structured]) {
                    push_group(scanner, GROUP_STRUCTURED);
                    type = structured;
                } else if (!is_structured && !is_false && selected_branch > 0) {
                    // The branch directives before the selected branch start skipped text.
                    push_group(scanner, (GroupKind)(GROUP_LINE_SELECTED + selected_branch - 1));
                    type = PREPROC_CONDITIONAL;
                } else {
                    push_group(scanner, is_false ? GROUP_LINE_PENDING : GROUP_LINE_CHOSEN);
                    type = PREPROC_CONDITIONAL;
                }
                break;
            }
            case COND_ELSE: {
                if (group == GROUP_STRUCTURED) {
                    type = name_is(&name, "else")      ? PREPROC_ELSE
                           : name_is(&name, "elif")    ? PREPROC_ELIF
                           : name_is(&name, "elifdef") ? PREPROC_ELIFDEF
                                                       : PREPROC_ELIFNDEF;
                } else if (group == GROUP_LINE_PENDING) {
                    // The first branch that is not false is the branch that the parser reads.
                    if (!(name_is(&name, "elif") && condition_is_false(lexer))) {
                        scanner->groups[scanner->group_count - 1] = GROUP_LINE_CHOSEN;
                    }
                    type = PREPROC_CONDITIONAL;
                } else if (group >= GROUP_LINE_SELECTED) {
                    // The scan of the group at its `#if` selected a later branch (`select_line_branch`).
                    scanner->groups[scanner->group_count - 1] =
                        group == GROUP_LINE_SELECTED ? (uint8_t)GROUP_LINE_CHOSEN : (uint8_t)(group - 1);
                    type = PREPROC_CONDITIONAL;
                } else {
                    scan_skipped_text(lexer, false, 0);
                    type = PREPROC_SKIPPED;
                }
                break;
            }
            case COND_ENDIF:
                // A group that is deeper than MAX_GROUPS closes before the groups of the array.
                if (scanner->deep_groups > 0) {
                    scanner->deep_groups--;
                } else if (scanner->group_count > 0) {
                    scanner->group_count--;
                }
                type = group == GROUP_STRUCTURED ? PREPROC_ENDIF : PREPROC_CONDITIONAL;
                break;
            default:
                type = name_is(&name, "define")    ? PREPROC_DEFINE
                       : name_is(&name, "include") ? PREPROC_INCLUDE
                                                   : PREPROC_DIRECTIVE;
                // Where a declaration or a statement can start, an `#embed` is a directive line.
                if (name_is(&name, "embed") && !valid_symbols[PREPROC_IF]) {
                    type = PREPROC_EMBED;
                }
                if (!valid_symbols[type]) {
                    type = PREPROC_DIRECTIVE;
                }
                if (!valid_symbols[type]) {
                    return false;
                }
                // The last directive line of an `#elif` or `#else` branch is the last child of the branch. In
                // error recovery each token is valid, and the scanner gives the usual token.
                {
                    enum TokenType final_type = type == PREPROC_DEFINE      ? PREPROC_FINAL_DEFINE
                                                : type == PREPROC_INCLUDE   ? PREPROC_FINAL_INCLUDE
                                                : type == PREPROC_DIRECTIVE ? PREPROC_FINAL_DIRECTIVE
                                                                            : type;
                    bool error_recovery = valid_symbols[RAW_STRING_DELIMITER] && valid_symbols[RAW_STRING_CONTENT];
                    if (final_type != type && valid_symbols[final_type] && !error_recovery &&
                        branch_ends_after_line(scanner, lexer)) {
                        type = final_type;
                    }
                }
                break;
        }
        // An `#endif` and an `#else` take a fixed number of tokens. The rest of their line is extra
        // tokens, and the mark of those tokens reads this answer. The scan comes after `mark_end` of
        // the directive token, so it reads no character of that token.
        if (type == PREPROC_ENDIF || type == PREPROC_ELSE) {
            scanner->preproc_extra_tokens = line_has_extra_tokens(lexer);
        }
        lexer->result_symbol = type;
        return true;
    }
}

// Pointers to members.

/// Read the white space, the comments, and the directive lines in the lookahead of a member pointer
/// (`skip_gap`). Return false when a token that cannot start a line stops the gap.
static bool read_member_pointer_gap(Reader *reader) {
    Gap gap = {0};
    skip_gap(reader, &gap);
    return !gap.blocked;
}

/// Read a group in `<>`, `()`, or `[]`, and the groups in it. Return false if the group does not
/// close in the budget, or if it contains `;`, `{`, or `}`. A `>` in an expression, as in `->`,
/// closes a group too early. The scope of a member pointer almost never contains such a `>`.
static bool read_group(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    int depth = 0;
    do {
        LOOP_STEP();
        if (!readable(reader)) {
            return false;
        }
        int32_t c = lexer->lookahead;
        if (c == '<' || c == '(' || c == '[') {
            ++depth;
        } else if (c == '>' || c == ')' || c == ']') {
            --depth;
        } else if (c == ';' || c == '{' || c == '}') {
            return false;
        }
        step(reader);
    } while (depth > 0);
    return true;
}

/// Read the cv-qualifiers, the attributes, and the more `*` and `&` operators after the `*` of a
/// member pointer. Return true if a name or the `...` of a pack follows them: `A::** ip`,
/// `C::*const& p`, `C::*[[a]] p`, `C::* __attribute__((a)) p`, `Cs::*...ps`, `Cs::*...`. An abstract
/// declarator such as `(C::*)` or `int C::* const` has no name, and the grammar reads it with a rule
/// that has no external token.
static bool scan_member_pointer_name(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    for (;;) {
        LOOP_STEP();
        if (!read_member_pointer_gap(reader)) {
            return false;
        }
        if (readable(reader) && (lexer->lookahead == '*' || lexer->lookahead == '&')) {
            step(reader);
            continue;
        }
        if (readable(reader) && lexer->lookahead == '[') {
            // An attribute-specifier-seq can follow each ptr-operator ([dcl.decl.general]).
            step(reader);
            Arguments attribute = {0};
            if (!read_member_pointer_gap(reader) || lexer->lookahead != '[' || !skip_group(reader, &attribute) ||
                !read_member_pointer_gap(reader) || lexer->lookahead != ']') {
                return false;
            }
            step(reader);
            continue;
        }
        if (lexer->lookahead == '.') {
            // The declarator of a pack has `...` before its name ([dcl.decl.general]).
            for (int dots = 0; dots < 3; ++dots) {
                if (!readable(reader) || lexer->lookahead != '.') {
                    return false;
                }
                step(reader);
            }
            return true;
        }
        if (!readable(reader) || !is_word_start(lexer->lookahead)) {
            return false;
        }
        char word[16];
        int length = 0;
        bool ascii = true;
        while (readable(reader) && is_word_char(lexer->lookahead)) {
            LOOP_STEP();
            ascii = ascii && lexer->lookahead < 0x80;
            if (length < (int)sizeof word - 1) {
                word[length] = (char)lexer->lookahead;
            }
            ++length;
            step(reader);
        }
        if (!ascii || length >= (int)sizeof word) {
            return true;
        }
        word[length] = '\0';
        if (strcmp(word, "__attribute__") == 0 || strcmp(word, "__attribute") == 0) {
            // GCC `cp_parser_declarator` and Clang `ParseTypeQualifierListOpt` read GNU attributes here.
            Arguments attribute = {0};
            if (!read_member_pointer_gap(reader) || lexer->lookahead != '(' || !skip_group(reader, &attribute)) {
                return false;
            }
            continue;
        }
        if (strcmp(word, "const") != 0 && strcmp(word, "volatile") != 0) {
            return true;
        }
    }
}

/// Read a name in the scope of a member pointer. Return true if the name is `decltype`.
static bool read_scope_name(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    static const char decltype_word[] = "decltype";
    const int decltype_length = (int)sizeof decltype_word - 1;
    int length = 0;
    bool is_decltype = true;
    while (readable(reader) && is_word_char(lexer->lookahead)) {
        LOOP_STEP();
        is_decltype = is_decltype && length < decltype_length && lexer->lookahead == decltype_word[length];
        ++length;
        step(reader);
    }
    return is_decltype && length == decltype_length;
}

/// Read the scope of a member pointer after its first name, and then the `*` and the name after
/// the `*`. `is_decltype` tells if the first name is `decltype`. O(n) in the budget.
static bool scan_member_pointer_scope(Reader *reader, bool is_decltype) {
    TSLexer *lexer = reader->lexer;
    for (;;) {
        LOOP_STEP();
        if (!read_member_pointer_gap(reader)) {
            return false;
        }
        if (lexer->lookahead == '<' || (is_decltype && lexer->lookahead == '(')) {
            if (!read_group(reader) || !read_member_pointer_gap(reader)) {
                return false;
            }
        }
        if (lexer->lookahead != ':') {
            return false;
        }
        step(reader);
        if (lexer->lookahead != ':') {
            return false;
        }
        step(reader);
        if (!read_member_pointer_gap(reader)) {
            return false;
        }
        if (lexer->lookahead == '*') {
            step(reader);
            return scan_member_pointer_name(reader);
        }
        if (!readable(reader) || !is_word_start(lexer->lookahead)) {
            return false;
        }
        is_decltype = read_scope_name(reader);
    }
}

/// Scan the start of the `ptr-operator` of a pointer to a member with a name or a pack after it:
/// `C::*p`, `::ns::C<T>::* const p`, `decltype(x)::*p`, or `Cs::*...ps`. The token has no width. The
/// scanner reads the scope only to find the `::*` and the name or the `...` after it. The grammar
/// then reads the scope and the `*` as usual tokens. Comments and directive lines can come between
/// the tokens (`skip_gap`).
///
/// This scan starts at a `::`, and `scan_word_start` reads a scope that starts with a name. The
/// scan reads a maximum of `MAX_MEMBER_POINTER_LOOKAHEAD` characters. `scanner` holds the open groups.
static bool scan_member_pointer_start(TSLexer *lexer, const Scanner *scanner) {
    lexer->result_symbol = MEMBER_POINTER_START;
    mark_end(lexer);
    Reader reader = start_reader(lexer, MAX_MEMBER_POINTER_LOOKAHEAD, scanner);
    step(&reader);
    if (lexer->lookahead != ':') {
        return false;
    }
    step(&reader);
    if (!read_member_pointer_gap(&reader) || !is_word_start(lexer->lookahead)) {
        return false;
    }
    bool is_decltype = read_scope_name(&reader);
    return scan_member_pointer_scope(&reader, is_decltype);
}

// Qt statements, compiler built-ins with type arguments, and MS inline assembly.

/// Move the lexer across the white space, the comments, and the directive lines after the end of the token
/// (`skip_gap`). Return false when a token that cannot start a line stops the gap. `scanner` holds the open groups.
static bool advance_space(TSLexer *lexer, const Scanner *scanner) {
    Reader reader = start_reader(lexer, MACRO_SCAN_LIMIT, scanner);
    Gap gap = {0};
    skip_gap(&reader, &gap);
    return !gap.blocked;
}

/// Read the text after the Qt word `emit` or `Q_EMIT`.
///
/// Qt defines the word as empty, and a signal call follows it: `emit changed(x);`. The function
/// returns true only when an identifier follows the word. A call `emit(x);` gets no token.
static bool scan_qt_emit_marker(TSLexer *lexer, const Scanner *scanner) {
    return advance_space(lexer, scanner) && is_word_start(lexer->lookahead);
}

/// Read the text after the Qt word `foreach` or `Q_FOREACH`.
///
/// Qt defines the word as a loop: `foreach (const QString &s, list) f(s);`. The function returns
/// true only when a list in parentheses follows the word, the list has exactly one comma at its top
/// level, and no `;` follows the list. A call `foreach(x);` or `foreach(0, n, f);` gets no token.
/// The function reads a maximum of `MAX_QT_FOREACH_SCAN` characters. O(n) in these characters.
static bool scan_qt_foreach_marker(TSLexer *lexer, const Scanner *scanner) {
    if (!advance_space(lexer, scanner) || lexer->lookahead != '(') {
        return false;
    }
    advance(lexer);
    unsigned depth = 1;
    unsigned commas = 0;
    for (unsigned count = 0; count < MAX_QT_FOREACH_SCAN && !lexer->eof(lexer); ++count) {
        int32_t character = lexer->lookahead;
        advance(lexer);
        switch (character) {
            case '"':
            case '\'':
                // A comma or a bracket in a quote does not count. A quote stops at the line end.
                while (!lexer->eof(lexer) && lexer->lookahead != character && !is_line_break(lexer->lookahead) &&
                       count < MAX_QT_FOREACH_SCAN) {
                    if (lexer->lookahead == '\\') {
                        skip_literal_backslash(lexer);
                    } else {
                        advance(lexer);
                    }
                    ++count;
                }
                advance(lexer);
                break;
            case '(':
            case '[':
            case '{':
                ++depth;
                break;
            case ')':
            case ']':
            case '}':
                if (--depth == 0) {
                    // A Qt `foreach` has two arguments, and a statement follows the list.
                    bool gap = advance_space(lexer, scanner);
                    return commas == 1 && (!gap || lexer->lookahead != ';');
                }
                break;
            case ',':
                if (depth == 1) {
                    ++commas;
                }
                break;
            case ';':
                if (depth == 1) {
                    return false;
                }
                break;
            default:
                break;
        }
    }
    return false;
}

/// Compare a word with the name of a trait, for `bsearch`.
static int compare_type_trait(const void *word, const void *name) {
    return strcmp((const char *)word, *(const char *const *)name);
}

/// True if the word is the name of a compiler trait in `TYPE_TRAITS`. O(log n) in the number of
/// traits.
static bool is_type_trait(const char *word) {
    return bsearch(word, TYPE_TRAITS, TYPE_TRAIT_COUNT, sizeof TYPE_TRAITS[0], compare_type_trait) != NULL;
}

/// True if the word is the name of a compiler trait in `TYPE_TRAIT_TYPES`. O(log n) in the number
/// of traits.
static bool is_type_trait_type(const char *word) {
    return bsearch(word, TYPE_TRAIT_TYPES, TYPE_TRAIT_TYPE_COUNT, sizeof TYPE_TRAIT_TYPES[0],
                   compare_type_trait) != NULL;
}

/// Read the text after `va_arg` or after the name of a compiler trait.
///
/// `va_arg` and a trait take a type: `va_arg(ap, const char *)`, `__is_same(T, int)`. The function
/// returns true only when `(` follows the word. The macro argument in `INSTKEYWORD(va_arg, VAArg)`
/// and a library template with the name of a trait, `__is_arithmetic<T>`, get no token.
static bool scan_parenthesis_after_word(TSLexer *lexer, const Scanner *scanner) {
    return advance_space(lexer, scanner) && lexer->lookahead == '(';
}

// Structured bindings.

/// The maximum number of characters that the scan of a structured binding reads after `auto`.
#define MAX_STRUCTURED_BINDING_LOOKAHEAD 1024

/// The specifier words that can come between `auto` and the `[` of a structured binding: the
/// cv-qualifiers and the storage specifiers that [dcl.pre] permits, with their GNU spellings.
static const char *const BINDING_SPECIFIER_WORDS[] = {
    "const",   "volatile", "static",     "thread_local", "constexpr",    "constinit",
    "__const", "__const__", "__volatile", "__volatile__", "__thread", NULL,
};

/// Read the text after the `auto` of a structured binding declaration. The token holds the word.
///
/// The scan reads specifier words, attributes, and one `&` or `&&`. It returns true at a `[` that a
/// second `[` does not follow, as GCC `cp_parser_simple_declaration` selects a binding:
/// `auto [a, b]`, `auto const& [a, b]`, `auto&& [...rest]`. For `auto x`, `auto& [[a]] x`, and
/// `auto [[a]] f()`, it returns false, and the grammar reads `auto` as a keyword. The scan reads a
/// maximum of `MAX_STRUCTURED_BINDING_LOOKAHEAD` characters.
static bool scan_structured_binding_auto(TSLexer *lexer, const Scanner *scanner) {
    mark_end(lexer);
    lexer->result_symbol = STRUCTURED_BINDING_AUTO;
    Reader reader = start_reader(lexer, MAX_STRUCTURED_BINDING_LOOKAHEAD, scanner);
    bool reference = false;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(&reader, &gap);
        if (gap.blocked || !readable(&reader)) {
            return false;
        }
        int32_t c = lexer->lookahead;
        if (c == '[') {
            step(&reader);
            gap = (Gap){0};
            skip_gap(&reader, &gap);
            if (gap.blocked || !readable(&reader)) {
                return false;
            }
            int32_t first = lexer->lookahead;
            if (first != '[') {
                // A binding list starts with a name or `...`. An empty list is an error of the
                // declaration, and not a different construct.
                return is_word_start(first) || first == '.' || first == ']';
            }
            // An attribute of the type. Clang also reads an attribute after the ref-qualifier:
            // `auto &[[maybe_unused]] [a, b]` (ParseDeclaratorInternal).
            Arguments attribute = {0};
            if (!skip_group(&reader, &attribute)) {
                return false;
            }
            gap = (Gap){0};
            skip_gap(&reader, &gap);
            if (gap.blocked || !readable(&reader) || lexer->lookahead != ']') {
                return false;
            }
            step(&reader);
        } else if (c == '&' && !reference) {
            step(&reader);
            if (lexer->lookahead == '&') {
                step(&reader);
            }
            reference = true;
        } else if (is_word_start(c)) {
            char word[MACRO_WORD_SIZE];
            bool has_lower = false;
            read_word(&reader, word, &has_lower);
            if (strcmp(word, "__attribute__") == 0 || strcmp(word, "__attribute") == 0) {
                // Clang also reads a GNU attribute after the ref-qualifier: `auto &__attribute__((used)) [a, b]`.
                gap = (Gap){0};
                skip_gap(&reader, &gap);
                Arguments attribute = {0};
                if (gap.blocked || lexer->lookahead != '(' || !skip_group(&reader, &attribute)) {
                    return false;
                }
            } else if (reference || !word_in(word, BINDING_SPECIFIER_WORDS)) {
                return false;
            }
        } else {
            return false;
        }
    }
}

// The `...` of a pack index.

/// Scan the `...` of a C++26 pack index that a comment, a line splice, or a directive line divides from its `[`:
/// `T...`, a `#pragma probe` line, and `[0];`. The token holds the `...`, and the grammar reads the `[` after it.
///
/// The preprocessor removes a directive line and a comment before the parser gets the tokens, and `T...[0]` and
/// `T... [0]` are the same tokens (GCC `cp_parser_pack_index`). The lexer reads the forms with white space as one
/// `...[` token (`pack_index_open` in the grammar), and this scan gives no token for them. O(n) in the characters
/// of the gap.
static bool scan_pack_index_ellipsis(TSLexer *lexer, const Scanner *scanner) {
    for (unsigned i = 0; i < 3; i++) {
        if (lexer->lookahead != '.') {
            return false;
        }
        advance(lexer);
    }
    mark_end(lexer);
    Reader reader = start_reader(lexer, MACRO_SCAN_LIMIT, scanner);
    Gap gap = {0};
    skip_gap(&reader, &gap);
    lexer->result_symbol = PACK_INDEX_ELLIPSIS;
    return gap.text && !gap.blocked && readable(&reader) && lexer->lookahead == '[';
}

// The arguments of an attribute.

/// Scan the empty token before the `(` of an attribute-argument-clause whose tokens are not an expression
/// list. The lookahead is the `(`.
///
/// [dcl.attr.grammar]: an attribute-argument-clause is a balanced token sequence. For an attribute that it
/// does not know, GCC reads balanced tokens (cp_parser_std_attribute, gcc/cp/parser.cc) and Clang skips to
/// the closing parenthesis (ParseCXX11AttributeArgs, clang/lib/Parse/ParseDeclCXX.cpp). Without name lookup,
/// the tokens of the clause decide: `[[vendor::foo(a b c)]]` is no expression list, and `[[deprecated("x")]]`
/// is one. O(n) in the characters of the clause.
static bool scan_attribute_tokens_marker(TSLexer *lexer, const Scanner *scanner) {
    mark_end(lexer);
    Reader reader = start_reader(lexer, MAX_MACRO_ARGUMENTS_LENGTH, scanner);
    Arguments args = {0};
    if (!skip_group(&reader, &args)) {
        return false;
    }
    lexer->result_symbol = ATTRIBUTE_TOKENS_MARKER;
    return args.word_sequence || args.statements;
}

// The digraphs.

/// Scan the digraph `<:`, which is the token `[` ([lex.digraph]). The lookahead is the `<`.
///
/// [lex.pptoken]p3.2: `<::` is the two tokens `<` and `::` when the character after it is neither a `:`
/// nor a `>`. `std::vector<::std::string>` keeps its tokens for this reason. GCC reads the same
/// characters in the `<` case of `_cpp_lex_direct` (libcpp/lex.cc), and Clang in `LexTokenInternal`
/// (clang/lib/Lex/Lexer.cpp). The lexer reads the other digraphs, which have no such exception.
static bool scan_less_than_digraph(TSLexer *lexer) {
    advance(lexer);
    if (lexer->lookahead != ':') {
        return false;
    }
    advance(lexer);
    mark_end(lexer);
    if (lexer->lookahead == ':') {
        advance(lexer);
        if (lexer->lookahead != ':' && lexer->lookahead != '>') {
            return false;
        }
    }
    lexer->result_symbol = OPEN_BRACKET;
    return true;
}

// The brackets of a splice and of an attribute.

/// Scan the first bracket of an attribute that a comment, a line splice, or a directive line divides from
/// the second bracket: `[`, a `#pragma probe` line, and `[nodiscard]]`. The token holds one bracket, and
/// the grammar reads the second bracket after it. The caller read the first bracket.
///
/// [dcl.attr.grammar]: an attribute-specifier is the tokens `[ [ ... ] ]`. The preprocessor removes a
/// directive line and a comment before the parser gets the tokens, and `[ [a] ]` and `[[a]]` are the same
/// tokens. The lexer reads the forms with white space as one token (`open` and `close` in the grammar),
/// and this scan gives no token for them. O(n) in the characters of the gap.
static bool scan_attribute_bracket(TSLexer *lexer, const Scanner *scanner, int32_t bracket, unsigned symbol) {
    mark_end(lexer);
    Reader reader = start_reader(lexer, MACRO_SCAN_LIMIT, scanner);
    Gap gap = {0};
    skip_gap(&reader, &gap);
    lexer->result_symbol = symbol;
    return gap.text && !gap.blocked && readable(&reader) && lexer->lookahead == bracket;
}

/// Scan the `[` of a splice or of an attribute. The lookahead is `[`, and one of the two tokens is valid.
///
/// [lex.pptoken]p4.3: `[` is a token by itself before `::` when the character after the `::` is not a `:`, and
/// before `:>`. The digraphs make no `[:` token. GCC reads the same characters in the `[` case of
/// `_cpp_lex_direct` (libcpp/lex.cc). A `[` token and a `::` token then give the subscript of `a[::b]`.
static bool scan_open_bracket(TSLexer *lexer, const Scanner *scanner, const bool *valid_symbols) {
    advance(lexer);
    if (lexer->lookahead != ':') {
        return valid_symbols[ATTRIBUTE_OPEN_BRACKET] &&
               scan_attribute_bracket(lexer, scanner, '[', ATTRIBUTE_OPEN_BRACKET);
    }
    if (!valid_symbols[SPLICE_OPEN]) {
        return false;
    }
    advance(lexer);
    mark_end(lexer);
    if (lexer->lookahead == '>') {
        return false;
    }
    if (lexer->lookahead == ':') {
        advance(lexer);
        if (lexer->lookahead != ':') {
            return false;
        }
    }
    lexer->result_symbol = SPLICE_OPEN;
    return true;
}

// A name before the `<` of a comparison.

/// The maximum number of characters that the scan of a comparison reads after the name.
#define MAX_COMPARISON_LOOKAHEAD 2048

/// The words of the grammar that cannot be the name before the `<` of a comparison, and that the
/// lists of keywords do not contain.
static const char *const NOT_COMPARISON_NAMES[] = {
    "NULL", "TRUE", "FALSE", "safe_cast", "typeof", "__typeof", "__typeof__", "static_assert", NULL,
};

/// One token of the text after a name, for `scan_comparison_name`.
typedef struct {
    TokenClass kind;
    /// The text of an operator or a bracket, or the start of a word.
    char text[MACRO_WORD_SIZE];
    /// The token is a number literal.
    bool number;
} TextToken;

/// The facts about the text from the `<` after a name to a token, at the top level of the text. The
/// top level is outside `()`, `[]`, and `{}`.
typedef struct {
    /// A `&&`, `||`, `and`, or `or` of an expression.
    bool logical;
    /// A `,`.
    bool comma;
    /// A `<`.
    bool less;
    /// A `,` before the last `<`.
    bool comma_before_less;
    /// A `&&`, `||`, `and`, or `or` after the last `<`.
    bool logical_after_less;
    /// The tokens before the last `<` end with a name, or with names that `::`, `.`, or `->` connect.
    /// The first of these names comes after an operator or at the start of the text.
    bool name_before_less;
} AngleFacts;

/// The kind of the last token of the text at the top level.
typedef enum {
    LAST_NONE,
    LAST_WORD,
    /// `::`, `.`, or `->`.
    LAST_LINK,
    LAST_OPERATOR,
    LAST_OTHER,
} LastToken;

/// The rule that `scan_comparison_name` applies to the tokens after a `>` at the top level.
typedef enum {
    CLOSE_NONE,
    /// `x < 0 || x > (n - 1)`: an operand comes after the `>`.
    CLOSE_OPERAND,
    /// `i < n >> 1`: an operand comes after the `>>`.
    CLOSE_SHIFT,
    /// `i < std::get<0>(t)`: a call group and a token of an expression come after the `>`.
    CLOSE_CALL,
} CloseRule;

/// The state of the rule for the tokens after a `>`.
typedef struct {
    CloseRule rule;
    /// The name before `<` also comes before the `>`, after `&&` or `||`. The name is then a variable.
    bool same;
    /// The depth of the groups in `()`, `[]`, and `{}` after the `>`, or 0.
    unsigned group_depth;
    bool group_empty;
    /// A `,` outside the inner groups of the group.
    bool group_comma;
    /// The last token of the group outside its inner groups is `*`, `&`, or `&&`.
    bool group_declarator;
    /// The number of tokens after the call group of `CLOSE_CALL`, to a maximum of 2. A name after `::` has
    /// the role of the call group.
    unsigned after_group;
    /// A `::` comes immediately after the `>` of `CLOSE_CALL`, and the name of the qualified name comes next.
    bool scope_link;
    /// The number of `<` after the call group that no `>` closes.
    unsigned angle_depth;
} Close;

/// The result of one token for the rule of a `>`.
typedef enum {
    /// The rule needs more tokens.
    FEED_MORE,
    /// The name before `<` is an operand of a comparison.
    FEED_MARK,
    /// The rule does not apply.
    FEED_STOP,
} FeedResult;

/// Read the next token after white space and comments. A bracket is a token here, and a `>` is one
/// character, so that the caller can find `>=` and `>>`. Return false at the end of the file or of
/// the budget, at a `#`, and at a backslash that does not continue the line.
static bool read_text_token(Reader *reader, TextToken *token) {
    TSLexer *lexer = reader->lexer;
    // After a false result, the token is an operator with no text, and not the bytes of an earlier stack frame.
    *token = (TextToken){.kind = TOKEN_OPERATOR};
    Gap gap = {0};
    skip_gap(reader, &gap);
    if (gap.slash) {
        // The operator `/` or `/=`. A `=` after the end of the budget is not part of the token.
        bool assignment = readable(reader) && lexer->lookahead == '=';
        token->kind = TOKEN_OPERATOR;
        token->number = false;
        strcpy(token->text, assignment ? "/=" : "/");
        if (assignment) {
            step(reader);
        }
        return true;
    }
    if (gap.blocked || !readable(reader) || lexer->lookahead == '#') {
        return false;
    }
    int32_t c = lexer->lookahead;
    token->text[0] = c < 0x80 ? (char)c : '?';
    token->text[1] = '\0';
    token->number = is_digit(c);
    if (c == '(' || c == '[' || c == '{') {
        token->kind = TOKEN_OPEN;
        step(reader);
        return true;
    }
    if (c == ')' || c == ']' || c == '}') {
        token->kind = TOKEN_CLOSE;
        step(reader);
        return true;
    }
    if (c == '>') {
        token->kind = TOKEN_OPERATOR;
        step(reader);
        return true;
    }
    token->kind = read_token(reader, token->text);
    // `read_token` writes `.` for the literal `.5`, and no text for the other literals.
    token->number |= token->kind == TOKEN_LITERAL && token->text[0] == '.';
    return true;
}

/// True if the token can start an operand after a `>`: a word, a literal, `(`, `::`, or a prefix
/// operator.
static bool starts_operand(const TextToken *token) {
    const char *t = token->text;
    return token->kind == TOKEN_WORD || token->kind == TOKEN_LITERAL || strcmp(t, "(") == 0 ||
           strcmp(t, "::") == 0 || word_in(t, PREFIX_OPERATORS);
}

/// True if the token is an assignment operator. The `=` of `<<=` is a different token here.
static bool is_assignment(const TextToken *token) {
    const char *t = token->text;
    return strcmp(t, "=") == 0 || (strlen(t) == 2 && t[1] == '=' && strchr("+-*/%&|^", t[0]) != NULL);
}

/// Apply `CLOSE_CALL` to a token after the call group. The `next` character comes after the token.
/// O(1).
///
/// At the end of the expression, the name is an operand of the comparison. A `;` or a bracket that
/// closes an outer group is the end. These tokens keep the template-id, because an outer template-id
/// or a declaration can contain it:
/// - A word or a literal immediately after the group: `AnyInvocable<StatusOr<T>() const>`.
/// - A `,`, `...`, `{`, or an assignment operator: `index_sequence<f<I>()...>`.
/// - A `>` that closes no `<` after the group: `std::bitset<static_cast<int>(n) + 1>`.
static FeedResult feed_after_call(Close *close, const TextToken *token, int32_t next) {
    const char *t = token->text;
    bool first = close->after_group == 1;
    close->after_group = 2;
    if (token->kind == TOKEN_SEMICOLON || token->kind == TOKEN_CLOSE) {
        return FEED_MARK;
    }
    bool logical_word = strcmp(t, "and") == 0 || strcmp(t, "or") == 0;
    if (first && ((token->kind == TOKEN_WORD && !logical_word) || token->kind == TOKEN_LITERAL)) {
        return FEED_STOP;
    }
    if (token->kind == TOKEN_COMMA || strcmp(t, "...") == 0 || strcmp(t, "{") == 0 || is_assignment(token)) {
        return FEED_STOP;
    }
    if (token->kind == TOKEN_OPEN) {
        close->group_depth = 1;
        return FEED_MORE;
    }
    if (strcmp(t, "<") == 0) {
        close->angle_depth++;
    } else if (strcmp(t, ">") == 0 && next != '=') {
        unsigned count = next == '>' ? 2 : 1;
        if (close->angle_depth < count) {
            return FEED_STOP;
        }
        close->angle_depth -= count;
    }
    return FEED_MORE;
}

/// Apply the rule of a `>` to the next token after it. The `next` character comes after the token.
/// O(1).
///
/// - `CLOSE_OPERAND`: a group in `()` that is not empty and has no `,`, `-`, `+`, `!`, `~`, `++`, `--`,
///   or a number. The template-id reading continues with a call or a binary operator, and real code
///   has the comparison. When the name is a variable, each operand and each group that is not empty.
/// - `CLOSE_SHIFT`: a number. When the name is a variable, each operand.
/// - `CLOSE_CALL`: a group in `()` that is not an abstract declarator such as `(*)`, or `::` and a name,
///   and after it the end of the expression. Refer to `feed_after_call`.
static FeedResult feed_close(Close *close, const TextToken *token, int32_t next) {
    const char *t = token->text;
    if (close->group_depth > 0) {
        if (token->kind == TOKEN_CLOSE && --close->group_depth == 0) {
            if (close->rule != CLOSE_CALL) {
                return !close->group_empty && (close->same || !close->group_comma) ? FEED_MARK : FEED_STOP;
            }
            if (close->after_group == 0) {
                // A call argument cannot end with `*`, `&`, or `&&`: `F<R (*)(A)>`.
                if (close->group_declarator) {
                    return FEED_STOP;
                }
                close->after_group = 1;
            }
            return FEED_MORE;
        }
        close->group_empty = false;
        if (token->kind == TOKEN_OPEN) {
            close->group_depth++;
        } else if (close->group_depth == 1) {
            close->group_comma |= token->kind == TOKEN_COMMA;
            close->group_declarator = strcmp(t, "*") == 0 || strcmp(t, "&") == 0 || strcmp(t, "&&") == 0;
        }
        return FEED_MORE;
    }
    if (close->after_group > 0) {
        return feed_after_call(close, token, next);
    }
    // A template-id before `::` is the scope of a qualified name: `i < A<N + 1>::value`. The name after `::`
    // has the role of the call group, and the end of the expression comes after it. `template` can come
    // before the name: `i < A<N>::template f<2>(x)`.
    if (close->rule == CLOSE_CALL && close->scope_link) {
        if (token->kind != TOKEN_WORD) {
            return FEED_STOP;
        }
        close->scope_link = strcmp(t, "template") == 0;
        close->after_group = close->scope_link ? 0 : 1;
        return FEED_MORE;
    }
    if (close->rule == CLOSE_CALL && token->kind == TOKEN_SCOPE && strcmp(t, "::") == 0) {
        close->scope_link = true;
        return FEED_MORE;
    }
    if (strcmp(t, "(") == 0 && close->rule != CLOSE_SHIFT) {
        close->group_depth = 1;
        close->group_empty = true;
        return FEED_MORE;
    }
    if (close->same) {
        return close->rule != CLOSE_CALL && starts_operand(token) ? FEED_MARK : FEED_STOP;
    }
    if (close->rule == CLOSE_SHIFT) {
        return token->number ? FEED_MARK : FEED_STOP;
    }
    bool prefix = strcmp(t, "-") == 0 || strcmp(t, "+") == 0 || strcmp(t, "!") == 0 || strcmp(t, "~") == 0 ||
                  strcmp(t, "++") == 0 || strcmp(t, "--") == 0;
    return close->rule == CLOSE_OPERAND && (token->number || prefix) ? FEED_MARK : FEED_STOP;
}

/// Scan a name that is an operand of a comparison, and not the name of a template-id. The token
/// holds the name.
///
/// Only name lookup tells a variable from a template before `<` (GCC `cp_parser_template_name`,
/// Clang `Sema::isTemplateName`). Without the token, the dynamic precedences of the template
/// arguments give these forms to a template-id:
///
/// - `x < 0 || x > (n - 1)`: the text up to the first `>` has `&&` or `||` and no `,` or `<`.
/// - `i < n >> 1`: the text up to the first `>` has no `,` or `<`, and the `>` is the first `>` of `>>`.
/// - `i < std::get<0>(t)` and `x < 0 || !isUInt<32>(x)`: a name comes before the last `<` of the text
///   up to the first `>`, and no `,` comes before that `<`. A `::` and a name after the `>` have the role
///   of the call group: `i < A<N + 1>::value`.
/// - `x < a.b<c>() || x > *p`: a `>` comes after the name, and a `&&` or `||` comes before the name.
///   A template name is not an operand, and the name is then a variable. A `<` or a `,` before the
///   name does not start an operand: `Result<unique_ptr<Result>> f()`.
///
/// `feed_close` gives the tokens after the `>` for each form. Other tokens after the `>` keep the
/// template-id: `C<a && !b>::f(d)`, `X<a && b> c;`, `enable_if_t<a && b>* = nullptr`. A `&&` before
/// `>`, `,`, or `...` is a reference in a type: `std::forward<T&&>(x)`.
///
/// The scan starts after the name, which `read_word` read. It stops at the end of the expression or
/// at an assignment operator, and it reads a maximum of `MAX_COMPARISON_LOOKAHEAD` characters. O(n)
/// in the characters that it reads.
static bool scan_comparison_name(Reader *reader, const char *name, bool comparison_valid, bool cast_name) {
    TSLexer *lexer = reader->lexer;
    lexer->result_symbol = COMPARISON_NAME;
    mark_end(lexer);
    if (reader->budget > MAX_COMPARISON_LOOKAHEAD) {
        reader->budget = MAX_COMPARISON_LOOKAHEAD;
    }
    skip_blanks(reader);
    if (!readable(reader)) {
        return false;
    }
    // A blank between the name of a recorded class and the `(` gives a functional cast: `A (x)`. White
    // space has no meaning between the two tokens. The GCC dump gives one `aggr_init_expr ctor:1` for
    // `A (6)` and the same node for `A(6)`.
    //
    // `scan_word_start` gives the token for `A(x)`. It sends the form with a blank here, because the
    // scan of a comparison cannot go back to the name.
    //
    // THIS BLANK IS NOT THE BLANK OF THE TEMPLATE-ID BELOW. This one comes after a plain NAME. No `<`
    // follows it, so no comparison and no shift can start. The other blank comes after the `>` of the
    // argument list. That one keeps the comparison of `a<b> >(c)`, and the comment there tells you to
    // keep it. The two conditions look the same and they are for different text. Do not make them one
    // condition.
    //
    // `skip_blanks` reads a space and a tab only. A newline or a comment before the `(` gives no token.
    // The guard of `cast_name` in `scan_word_start` stops the token at each position where a declaration
    // or a type-id can start. `A (gx);`, `A (*gp)[3];` and `void p(A (px));` keep the tree that they
    // have today.
    if (cast_name && lexer->lookahead == '(') {
        lexer->result_symbol = FUNCTIONAL_CAST_NAME;
        return true;
    }
    if (lexer->lookahead != '<') {
        return false;
    }
    step(reader);
    int32_t c = lexer->lookahead;
    if (c == '<' || c == '=' || c == ':' || c == '%') {
        return false;
    }
    AngleFacts facts = {0};
    Close close = {0};
    TextToken token;
    unsigned depth = 0;
    // The nesting of the angle brackets. The scan stepped past the first `<`, so the count starts at 1.
    // Only the functional cast reads this count. The comparison reads `closed` as before. The count
    // stops at the close of the first group: a later `<` opens the brackets of a different name, as in
    // `ModuloImpl<A, B>::template apply<R>(a, b)`, and the token holds the first name only.
    unsigned angles = 1;
    bool angles_done = false;
    bool closed = false;
    bool pending_logical = false;
    LastToken last = LAST_NONE;
    LastToken before_last = LAST_NONE;
    bool chain = false;
    // The last tokens are `&&`, `||`, `and`, or `or`, and then prefix operators: `|| *`.
    bool operand_start = false;
    // The chain of names that ends at the last token starts where `operand_start` is true.
    bool operand_chain = false;
    bool last_is_name = false;
    while (read_text_token(reader, &token)) {
        LOOP_STEP();
        const char *t = token.text;
        if (pending_logical) {
            bool reference = strcmp(t, ">") == 0 || strcmp(t, "...") == 0 || strcmp(t, "=") == 0 ||
                             token.kind == TOKEN_COMMA || token.kind == TOKEN_CLOSE;
            if (!reference) {
                facts.logical = true;
                facts.logical_after_less |= facts.less;
            }
            pending_logical = false;
        }
        if (close.rule != CLOSE_NONE) {
            FeedResult result = feed_close(&close, &token, lexer->lookahead);
            if (result == FEED_MARK) {
                return comparison_valid;
            }
            if (result == FEED_STOP) {
                close.rule = CLOSE_NONE;
            }
        }
        if (token.kind == TOKEN_SEMICOLON || strcmp(t, "{") == 0 || strcmp(t, "}") == 0) {
            return false;
        }
        bool was_operand_start = operand_start;
        operand_start = false;
        if (token.kind == TOKEN_OPEN || token.kind == TOKEN_CLOSE) {
            if (token.kind == TOKEN_CLOSE && depth == 0) {
                return false;
            }
            if (depth == 0) {
                before_last = last;
                last = LAST_OTHER;
                chain = false;
            }
            depth += token.kind == TOKEN_OPEN ? 1 : -1;
            continue;
        }
        if (depth > 0) {
            continue;
        }
        // A comparison is not the left operand of an assignment. A `>` before the assignment operator
        // closes a template-id: `std::same_as<C<char>> auto c = f<C>(r);`.
        if (is_assignment(&token)) {
            return false;
        }
        if (strcmp(t, ">") == 0) {
            if (lexer->lookahead == '=') {
                step(reader);
                before_last = last;
                last = LAST_OPERATOR;
                chain = false;
                continue;
            }
            bool split = lexer->lookahead == '>';
            if (split) {
                step(reader);
                if (lexer->lookahead == '=') {
                    return false;
                }
            }
            unsigned levels = split ? 2 : 1;
            // True when this `>` closes the last open bracket of the first group. A `>` after that
            // group is an operator: the second `>` of `a<b> >(c)` compares two values.
            bool closes = !angles_done && angles > 0 && angles <= levels;
            if (!angles_done) {
                angles = angles > levels ? angles - levels : 0;
                angles_done = angles == 0;
            }
            // A functional cast to a class template: `B<int>(x)`, `time_point<_Clock, _To>(t)`.
            //
            // The condition has five parts. The `>` closes the last open bracket. The name is in the
            // record, and a type specifier cannot start at the name (`cast_name`). No logical operator
            // is in the brackets. The `(` of the argument list comes IMMEDIATELY after the `>`, with
            // no blank. The last two parts keep a comparison of values: `x < 0 || x > (n - 1)` has the
            // operator, and `a<b> > (c)` has the blank.
            //
            // The count of the brackets is exact, so a nested list gives the same answer with one `>>`
            // token and with two `>` tokens: `B<C<int>>(x)` and `B<C<int> >(x)` are both casts.
            //
            // DO NOT REMOVE THE CONDITION OF THE BLANK. It carries two jobs, and only the first one
            // gives a correct tree:
            // - It keeps ONE comparison in `a<b> >(c)`. The template-id `a<b>` is the left operand.
            //   The first `>` closes the argument list, and the second `>` is the operator. The text
            //   has no reading with two comparisons. This program gives that tree, and the two front
            //   ends accept it:
            //       template<class T> int a; struct b { }; int c;
            //       bool f() { return a<b> >(c); }
            //   With `a`, `b` and `c` as ints the two front ends reject the text. Without the
            //   condition, that text becomes a cast. The parser then wants an argument list after
            //   `a<b>`, and the tree gets an ERROR node on a program that the two front ends compile.
            //   The acceptance rule forbids that at any site count. The count below is no reason to
            //   remove the condition.
            // - It also declines `B<int> (x)`, which is a correct functional cast, because white space
            //   has no meaning between the two tokens. This is a KNOWN loss and not a defect. In the
            //   329,387 corpus files 3503 sites of a template-id of a recorded class have no blank and
            //   1035 sites have one. The 1035 keep the tree of a call.
            //
            // THE RULE IS INCORRECT FOR ONE FORM, and it is the form of `is_class_name` in
            // `scan_word_start`. A name that hides a class gives the record a class that the name does
            // not denote. `struct A { int m; }; int A = 3; return A < b > (c);` is two comparisons,
            // and GCC accepts it. The blank before the `(` keeps the comparison there, and the
            // measurement found no corpus site of this form with no blank. Refer to the comment of the
            // functional cast in `scan_word_start` for the record and for [basic.scope.scope].
            if (closes && cast_name && !facts.logical && lexer->lookahead == '(') {
                lexer->result_symbol = FUNCTIONAL_CAST_NAME;
                return true;
            }
            bool same = last == LAST_WORD && last_is_name && chain && operand_chain;
            if (close.rule == CLOSE_NONE) {
                close = (Close){0};
                close.same = same;
                if (same) {
                    close.rule = split ? CLOSE_SHIFT : CLOSE_OPERAND;
                } else if (!closed && facts.less) {
                    bool call = !split && facts.name_before_less && !facts.comma_before_less && !facts.logical_after_less;
                    close.rule = call ? CLOSE_CALL : CLOSE_NONE;
                } else if (!closed && !facts.comma) {
                    close.rule = split ? CLOSE_SHIFT : facts.logical ? CLOSE_OPERAND : CLOSE_NONE;
                }
            }
            closed = true;
            before_last = last;
            last = LAST_OTHER;
            chain = false;
            continue;
        }
        if (token.kind == TOKEN_WORD && strcmp(t, "and") != 0 && strcmp(t, "or") != 0) {
            if (last != LAST_LINK) {
                chain = last == LAST_NONE || last == LAST_OPERATOR;
                operand_chain = was_operand_start;
            }
            last_is_name = strcmp(t, name) == 0;
            before_last = last;
            last = LAST_WORD;
            continue;
        }
        before_last = last;
        if (token.kind == TOKEN_WORD) {
            facts.logical = true;
            facts.logical_after_less |= facts.less;
            last = LAST_OPERATOR;
            chain = false;
            operand_start = true;
        } else if (strcmp(t, "&&") == 0 || strcmp(t, "||") == 0) {
            pending_logical = true;
            last = LAST_OPERATOR;
            chain = false;
            operand_start = true;
        } else if (token.kind == TOKEN_COMMA) {
            facts.comma = true;
            last = LAST_OTHER;
            chain = false;
        } else if (strcmp(t, "::") == 0 || strcmp(t, ".") == 0 || strcmp(t, "->") == 0) {
            if (last != LAST_WORD) {
                // A chain can start with `::`: `::std::get`.
                chain = strcmp(t, "::") == 0 && (last == LAST_NONE || last == LAST_OPERATOR);
                operand_chain = was_operand_start;
            }
            last = LAST_LINK;
        } else if (strcmp(t, "<") == 0) {
            if (!angles_done) {
                angles++;
            }
            facts.less = true;
            facts.name_before_less = before_last == LAST_WORD && chain;
            facts.comma_before_less = facts.comma;
            facts.logical_after_less = false;
            last = LAST_OPERATOR;
            chain = false;
        } else {
            if (strcmp(t, "<=") == 0 && lexer->lookahead == '>') {
                step(reader);
            }
            last = token.kind == TOKEN_OPERATOR || token.kind == TOKEN_DECLARATOR ? LAST_OPERATOR : LAST_OTHER;
            chain = false;
            operand_start = was_operand_start && word_in(t, PREFIX_OPERATORS);
        }
    }
    return false;
}

// A name in parentheses.
//
// Only name lookup tells a type from an expression in `(T) * x`, `(T)(x)`, and `sizeof(T)`. GCC tries to
// parse a type-id (`cp_parser_cast_expression`, `cp_parser_sizeof_operand`), and Clang uses
// `isTypeIdInParens` in `ParseParenExpression`. The two compilers find the declarations of the names.
// Without the declarations, the two readings have the same dynamic precedences, and the sequence of the
// parse versions selects one.
//
// This scan selects the reading from the shape of the name and from the token after the parentheses.
// A measurement of 329,387 files of 64 codebases compared each name with the declarations of its
// codebase. In `sizeof`, 92% of the names with an uppercase first letter and a lowercase letter are
// types, and 86% of the names with a lowercase first letter are variables. Before `(x)`, `-`, and `&`,
// 94% to 100% of the names with an uppercase first letter are types. The codebases of LLVM and ROOT
// name many variables with an uppercase first letter, and there the scan selects a type for them.

/// The maximum number of characters that the scan of a name in parentheses reads.
#define MAX_PARENTHESIZED_NAME_LOOKAHEAD 4096

/// The size of the buffer for one identifier of a name in parentheses.
#define NAME_WORD_SIZE 128

/// The form of a name in parentheses.
typedef enum {
    /// One identifier: `(x)`.
    NAME_PLAIN,
    /// Identifiers that `::` connects, with no template arguments at the end: `(a::b)`, `(A<T>::type)`.
    NAME_QUALIFIED,
    /// A name with template arguments at its end: `(A<T>)`, `(std::min<int>)`.
    NAME_TEMPLATE,
    /// A name and a group in parentheses that is not a declarator: `(f(x))`, `(std::max<T>(a, b))`. The
    /// type reading is a function type, and a cast to a function type is ill-formed ([expr.cast], GCC
    /// `cp_build_c_cast` in typeck.cc, Clang `CastOperation::CheckCXXCStyleCast` in SemaCast.cpp).
    NAME_CALL,
} NameForm;

/// The shape of the last identifier of a name in parentheses.
typedef enum {
    /// An uppercase first letter and a lowercase letter, one uppercase letter and digits, `_` and an
    /// uppercase letter at the start, `_t` or `_type` at the end, or a predefined type macro of GCC:
    /// `Value`, `T1`, `_Tp`, `off_t`, `__SIZE_TYPE__`.
    SHAPE_TYPE,
    /// Two or more characters that are uppercase letters, digits, or `_`: `DWORD`, `IP_USE_EVEX`.
    SHAPE_CAPITALS,
    /// A different identifier: `buf`, `kSize`, `__FILE__`, `_M_data`.
    SHAPE_VALUE,
} NameShape;

/// The shape of an identifier of `length` characters. O(n) in the length.
static NameShape name_shape(const char *text, size_t length) {
    bool reserved = length >= 4 && text[0] == '_' && text[1] == '_' && text[length - 1] == '_' && text[length - 2] == '_';
    // GCC predefines the macros of the standard types with this suffix: `__SIZE_TYPE__`
    // (c_stddef_cpp_builtins in c-common.cc).
    if (reserved && length > 7 && memcmp(&text[length - 7], "_TYPE__", 7) == 0) {
        return SHAPE_TYPE;
    }
    bool member = length >= 3 && text[0] == '_' && (text[1] == 'M' || text[1] == 'S') && text[2] == '_';
    if (reserved || member) {
        return SHAPE_VALUE;
    }
    size_t start = 0;
    while (start < length && text[start] == '_') {
        start++;
    }
    if (start < length && text[start] >= 'A' && text[start] <= 'Z') {
        bool lower = false;
        bool digits = true;
        for (size_t i = start + 1; i < length; i++) {
            lower |= text[i] >= 'a' && text[i] <= 'z';
            digits &= is_digit(text[i]);
        }
        return start > 0 || lower || digits ? SHAPE_TYPE : SHAPE_CAPITALS;
    }
    bool type_suffix = (length > 2 && memcmp(&text[length - 2], "_t", 2) == 0) ||
                       (length > 5 && memcmp(&text[length - 5], "_type", 5) == 0);
    return type_suffix ? SHAPE_TYPE : SHAPE_VALUE;
}

/// Go past a template argument list from the token after its `<` to the character after its `>`. Return
/// false at a `;`, `{`, or `}`, at a `)` or `]` that closes no bracket of the list, and at `>=`. O(n) in
/// the characters that it reads.
static bool skip_template_argument_tokens(Reader *reader) {
    unsigned angles = 1;
    unsigned brackets = 0;
    TextToken token;
    while (read_text_token(reader, &token)) {
        LOOP_STEP();
        const char *t = token.text;
        if (token.kind == TOKEN_SEMICOLON || t[0] == '{' || t[0] == '}') {
            return false;
        }
        if (token.kind == TOKEN_OPEN) {
            brackets++;
        } else if (token.kind == TOKEN_CLOSE) {
            if (brackets == 0) {
                return false;
            }
            brackets--;
        } else if (brackets == 0 && strcmp(t, "<") == 0) {
            angles++;
        } else if (brackets == 0 && strcmp(t, ">") == 0) {
            if (reader->lexer->lookahead == '=') {
                return false;
            }
            if (--angles == 0) {
                return true;
            }
        }
    }
    return false;
}

/// True if the token is `*`, `&`, `&&`, or `^`: an operator of a pointer or a reference declarator.
static bool is_pointer_operator(const TextToken *token) {
    const char *t = token->text;
    return strcmp(t, "*") == 0 || strcmp(t, "&") == 0 || strcmp(t, "&&") == 0 || strcmp(t, "^") == 0;
}

/// The facts about a group in parentheses.
typedef struct {
    /// 0 for `()`, 2 for a `,` at the top level of the group, and 1 otherwise. A `,` between a name and `<`
    /// and the `>` that closes the `<` is not at the top level.
    unsigned operands;
    /// The first token at the top level is a pointer operator: `(*)`, `(*p)`.
    bool first_pointer;
    /// The group is a type or a declarator, and not an expression: its last token at the top level is a
    /// pointer operator, `const`, or `volatile`, or its first token is a word of `TYPE_WORDS` with no `(`
    /// or `{` after it. `(C::*)`, `(unsigned long *)`, `(int)`.
    bool type;
} GroupFacts;

/// Read a group in parentheses from the character after its `(` to the character after its `)`, and give
/// its facts. Return false at a `;` and at the end of the budget. O(n) in the characters that it reads.
static bool read_paren_group(Reader *reader, GroupFacts *facts) {
    unsigned depth = 0;
    unsigned angles = 0;
    unsigned tokens = 0;
    bool comma = false;
    bool comma_outside_angles = false;
    bool after_word = false;
    bool last_declarator = false;
    bool first_type_word = false;
    *facts = (GroupFacts){0};
    TextToken token;
    while (read_text_token(reader, &token)) {
        LOOP_STEP();
        const char *t = token.text;
        if (token.kind == TOKEN_SEMICOLON) {
            return false;
        }
        if (token.kind == TOKEN_CLOSE && depth == 0) {
            bool top_level_comma = angles == 0 ? comma_outside_angles : comma;
            facts->operands = tokens == 0 ? 0 : top_level_comma ? 2 : 1;
            facts->type |= last_declarator || (first_type_word && tokens == 1);
            return true;
        }
        if (depth == 0) {
            if (tokens == 0) {
                facts->first_pointer = is_pointer_operator(&token);
                first_type_word = token.kind == TOKEN_WORD && word_in(t, TYPE_WORDS);
            } else if (tokens == 1) {
                facts->type |= first_type_word && strcmp(t, "(") != 0 && strcmp(t, "{") != 0;
            }
            last_declarator =
                is_pointer_operator(&token) || strcmp(t, "const") == 0 || strcmp(t, "volatile") == 0;
            tokens++;
        }
        if (token.kind == TOKEN_OPEN || token.kind == TOKEN_CLOSE) {
            depth += token.kind == TOKEN_OPEN ? 1 : -1;
            after_word = false;
            continue;
        }
        if (depth > 0) {
            continue;
        }
        if (token.kind == TOKEN_COMMA) {
            comma = true;
            comma_outside_angles |= angles == 0;
        } else if (after_word && strcmp(t, "<") == 0) {
            angles++;
        } else if (angles > 0 && strcmp(t, ">") == 0) {
            angles--;
        }
        after_word = token.kind == TOKEN_WORD;
    }
    return false;
}

/// Read a name and the `)` after it, from the character after the `(`. The name is an identifier, or
/// identifiers and template argument lists that `::` connects. A group that is not a declarator can come
/// after the name, as in `(f(x))`. Give the form of the name, and the text and the shape of its last
/// identifier. Return false for a different text, and for a keyword. O(n) in the characters that it reads.
static bool read_parenthesized_name(Reader *reader, NameForm *form, NameShape *shape, char text[NAME_WORD_SIZE]) {
    TSLexer *lexer = reader->lexer;
    TextToken token;
    Gap gap = {0};
    skip_gap(reader, &gap);
    bool qualified = false;
    if (lexer->lookahead == ':') {
        step(reader);
        if (lexer->lookahead != ':') {
            return false;
        }
        step(reader);
        gap = (Gap){0};
        skip_gap(reader, &gap);
        qualified = true;
    }
    for (bool first = !qualified;; first = false) {
        LOOP_STEP();
        if (gap.blocked || !is_word_start(lexer->lookahead)) {
            return false;
        }
        size_t length = 0;
        while (readable(reader) && is_word_char(lexer->lookahead)) {
            LOOP_STEP();
            if (length < NAME_WORD_SIZE - 1) {
                text[length] = lexer->lookahead < 0x80 ? (char)lexer->lookahead : '?';
            }
            length++;
            step(reader);
        }
        if (length >= NAME_WORD_SIZE) {
            return false;
        }
        text[length] = '\0';
        if (word_in(text, RESERVED_WORDS) || (first && is_grammar_keyword(text))) {
            return false;
        }
        *shape = name_shape(text, length);
        gap = (Gap){0};
        skip_gap(reader, &gap);
        *form = qualified ? NAME_QUALIFIED : NAME_PLAIN;
        if (!gap.blocked && lexer->lookahead == '<') {
            if (!read_text_token(reader, &token) || strcmp(token.text, "<") != 0 ||
                !skip_template_argument_tokens(reader)) {
                return false;
            }
            *form = NAME_TEMPLATE;
            gap = (Gap){0};
            skip_gap(reader, &gap);
        }
        if (gap.blocked || lexer->lookahead != ':') {
            break;
        }
        step(reader);
        if (lexer->lookahead != ':') {
            return false;
        }
        step(reader);
        gap = (Gap){0};
        skip_gap(reader, &gap);
        qualified = true;
    }
    if (!gap.blocked && lexer->lookahead == '(') {
        step(reader);
        GroupFacts group;
        if (!read_paren_group(reader, &group) || group.first_pointer || group.type) {
            return false;
        }
        *form = NAME_CALL;
        gap = (Gap){0};
        skip_gap(reader, &gap);
    }
    if (gap.blocked || lexer->lookahead != ')') {
        return false;
    }
    step(reader);
    return true;
}

/// True if the text after `sizeof(name)` divides it by the size of an element of `name`, as in
/// `sizeof(Values) / sizeof(Values[0])` and `sizeof(Values) / sizeof(*Values)`. The name is then an array.
/// O(1).
static bool is_array_size_quotient(Reader *reader, const char *name) {
    TextToken token;
    const char *const expected[] = {"/", "sizeof", "("};
    for (unsigned i = 0; i < 3; i++) {
        if (!read_text_token(reader, &token) || strcmp(token.text, expected[i]) != 0) {
            return false;
        }
    }
    if (!read_text_token(reader, &token)) {
        return false;
    }
    if (strcmp(token.text, "*") == 0 && !read_text_token(reader, &token)) {
        return false;
    }
    if (token.kind != TOKEN_WORD || strcmp(token.text, name) != 0 || !read_text_token(reader, &token)) {
        return false;
    }
    return strcmp(token.text, "[") == 0 || strcmp(token.text, ")") == 0;
}

/// The alternative tokens of the binary operators, which can come after an operand.
static const char *const ALTERNATIVE_OPERATOR_WORDS[] = {
    "and", "or", "bitand", "bitor", "xor", "not_eq", "and_eq", "or_eq", "xor_eq", NULL,
};

/// True if the token can start an operand and cannot come after a full operand: a word that is not an
/// alternative operator, a literal, `{`, `~`, or `!`. After `(T)(x)`, such a token shows that `(x)` is a
/// cast and not an argument list: `(u8)(s32)f(x)`, `(__m128i)(v16i8){2, 1}`.
static bool starts_operand_only(const TextToken *token) {
    const char *t = token->text;
    return (token->kind == TOKEN_WORD && !word_in(t, ALTERNATIVE_OPERATOR_WORDS)) || token->kind == TOKEN_LITERAL ||
           strcmp(t, "{") == 0 || strcmp(t, "~") == 0 || strcmp(t, "!") == 0;
}

/// Scan the `(` of a name in parentheses where an expression starts, and select the reading of the name
/// with the token of the `(`. The token of the `(` of a cast, `CAST_PAREN`, keeps only the cast. The token
/// of a parenthesized expression, `NAME_EXPRESSION_PAREN`, keeps only the expression. After `sizeof` and
/// `typeid`, `OPERAND_TYPE_PAREN` keeps only the type.
///
/// A name and a group that is not a declarator keep the expression: `(std::max<T>(a, b)) * 8`,
/// `sizeof(f(x))`.
///
/// After `sizeof` and `typeid`, the name is a type if it has template arguments, a scope, or the shape
/// `SHAPE_TYPE`: `sizeof(std::pair<A, B>)`, `sizeof(ns::T)`, `sizeof(Value)`. `sizeof(buf)` and
/// `sizeof(DATA)` keep the expression.
///
/// After the GNU `alignof`, each name is a type: `__alignof__(locale)`, `__alignof__(x)`. The operand of
/// this operator is a type in most code, and the shape of the name tells no type from a variable. The
/// measurement found the lowercase names `type`, `va_list`, `locale`, `istream`, and `runtime_error` as
/// the operand, and each one is a type.
///
/// In a different place, the token after the `)` also selects the reading:
/// - `(a, b)` and `&&` keep the call and the logical operator: `(std::min<T>)(a, b)`, `(Value) && x`.
/// - `(x)`, `*`, `&`, `+`, and `-` keep a cast for the shape `SHAPE_TYPE`, and for `SHAPE_CAPITALS`
///   without a scope: `(Value)(x)`, `(DWORD)&x`, `(std::size_t)(x)`, `(QFlags<F>)(x)`. The other names keep
///   the expression: `(isnan)(x)`, `(x)*(x)`, `~(X86::IP_USE_EVEX) & x`.
/// - A different name before a `&` with no space around it, or before a `-` and a number with no space
///   between them, also keeps a cast: `(uptr)&x`, `(u64)-1`. The measurement found 380 casts and 35
///   expressions in these forms.
/// - A `*` or a `&` before a number keeps the expression: `(Imm)&0xFFF`.
/// - For `()`, for a group before a token of `starts_operand_only`, for `...` after the operator of a fold,
///   and for each other token, only one reading continues, and the scan gives no token.
///
/// The scan reads a maximum of `MAX_PARENTHESIZED_NAME_LOOKAHEAD` characters. O(n) in the characters that
/// it reads.
static bool scan_parenthesized_name(TSLexer *lexer, const Scanner *scanner, const bool *valid_symbols) {
    Reader reader = start_reader(lexer, MAX_PARENTHESIZED_NAME_LOOKAHEAD, scanner);
    step(&reader);
    mark_end(lexer);
    NameForm form = NAME_PLAIN;
    NameShape shape = SHAPE_VALUE;
    char name[NAME_WORD_SIZE];
    if (!read_parenthesized_name(&reader, &form, &shape, name)) {
        return false;
    }
    bool alignof_operand = valid_symbols[ALIGNOF_TYPE_PAREN];
    bool operand = valid_symbols[OPERAND_TYPE_PAREN];
    bool type = false;
    if (alignof_operand) {
        // Each name is a type. Only a name with a call group is an expression: `__alignof__(f(x))`.
        type = form != NAME_CALL;
    } else if (operand) {
        type = form != NAME_CALL && (form != NAME_PLAIN || shape == SHAPE_TYPE);
        if (type && form == NAME_PLAIN) {
            type = !is_array_size_quotient(&reader, name);
        }
    } else {
        TextToken token;
        bool space_before = iswspace(lexer->lookahead) || lexer->lookahead == '/';
        if (!read_text_token(&reader, &token)) {
            return false;
        }
        const char *t = token.text;
        bool type_shape = form != NAME_CALL &&
                          (shape == SHAPE_TYPE || (shape == SHAPE_CAPITALS && form != NAME_QUALIFIED));
        if (token.kind == TOKEN_OPEN && t[0] == '(') {
            GroupFacts group;
            if (!read_paren_group(&reader, &group) || group.operands == 0 || group.type) {
                return false;
            }
            // The groups after the first group can also be casts: `(u16)(s16)(s8)val`,
            // `(hkey)(unsigned long *)(long)(1)`.
            GroupFacts next = group;
            do {
                LOOP_STEP();
                if (next.type || !read_text_token(&reader, &token) || starts_operand_only(&token)) {
                    return false;
                }
            } while (token.kind == TOKEN_OPEN && token.text[0] == '(' && read_paren_group(&reader, &next));
            type = group.operands == 1 && type_shape;
        } else if (strcmp(t, "*") == 0 || strcmp(t, "&") == 0 || strcmp(t, "+") == 0 || strcmp(t, "-") == 0) {
            char op = t[0];
            bool space_after = iswspace(lexer->lookahead) || lexer->lookahead == '/';
            // In a fold, only the expression continues: `((Values) + ... + 0)`.
            if (!read_text_token(&reader, &token) || strcmp(token.text, "...") == 0) {
                return false;
            }
            // The operand of a unary `*` or `&` is not a number: `(Imm)&0xFFF` and `(W)*2` are binary.
            bool binary_only = (op == '*' || op == '&') && token.number;
            // A cast has no space around a `&` and no space after a `-` before a number: `(uptr)&x`,
            // `(u64)-1`. A binary `&` or `-` has spaces: `(size) & mask`.
            bool tight_cast = form == NAME_PLAIN &&
                              ((op == '&' && !space_before && !space_after) || (op == '-' && !space_after && token.number));
            type = !binary_only && (type_shape || tight_cast);
        } else if (strcmp(t, "&&") != 0) {
            return false;
        }
    }
    // At the end of the budget, the scan compared the lookahead after its last read, a character that the budget does
    // not include, as the `)` of `(Value /*a...)*/)`. With a budget that is not 0, each compared character is in the
    // budget. As the other scans do, a scan that used its full budget gives no token.
    if (reader.budget == 0) {
        return false;
    }
    lexer->result_symbol = !type              ? NAME_EXPRESSION_PAREN
                           : alignof_operand  ? ALIGNOF_TYPE_PAREN
                           : operand          ? OPERAND_TYPE_PAREN
                                              : CAST_PAREN;
    return valid_symbols[lexer->result_symbol];
}

// The sign before a number that has a ud-suffix.
//
// The token `number_literal` of the grammar reads a sign at the start of a number. The lexer takes the
// longest token, and it reads `-1_k` as the number `-1` and the ud-suffix `_k`. GCC and Clang read the
// `-` as a unary operator: in a number, a sign comes only after `e`, `E`, `p`, or `P` (libcpp
// `lex_number` with `VALID_SIGN` in `internal.h`, Clang `LexNumericConstant`). The operand of the
// operator is the user-defined literal `1_k`.
//
// The scan reads the number after the sign, as the lexer reads it, and it gives the sign as its own
// token where a ud-suffix follows that number. A number with no ud-suffix keeps its sign, and `-1` stays
// one `number_literal`.

/// The maximum number of characters that the scan of a number after a sign reads.
#define MAX_NUMBER_LOOKAHEAD 256

/// The digits of a number.
typedef enum {
    /// `0` and `1`, after `0b`.
    DIGITS_BINARY,
    /// `0` thru `7`, after the `0` of an octal number.
    DIGITS_OCTAL,
    /// `0` thru `9`.
    DIGITS_DECIMAL,
    /// `0` thru `9`, `a` thru `f`, and `A` thru `F`, after `0x`.
    DIGITS_HEX,
} DigitClass;

/// True if the character is a digit of the class. O(1).
static bool is_class_digit(int32_t c, DigitClass digits) {
    switch (digits) {
        case DIGITS_BINARY:
            return c == '0' || c == '1';
        case DIGITS_OCTAL:
            return c >= '0' && c <= '7';
        case DIGITS_DECIMAL:
            return is_digit(c);
        default:
            return is_digit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
    }
}

/// True if the character can start a ud-suffix, which is an identifier (`literal_suffix`). O(1).
static bool starts_ud_suffix(int32_t c) {
    return c == '_' || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z');
}

/// Read the digits of a number, with the digit separators between them: `1'000`. `accept` is the
/// character after the digits that the caller read, or -1 when the caller read no digit. `after_digit`
/// tells that a digit of the same run comes before. Give the character after the last digit, or
/// `accept` when this run has no digit. A `'` that no digit follows is not part of the number.
/// O(n) in the characters that the scan reads.
static int32_t read_digits(Reader *reader, DigitClass digits, int32_t accept, bool after_digit) {
    TSLexer *lexer = reader->lexer;
    for (;;) {
        while (readable(reader) && is_class_digit(lexer->lookahead, digits)) {
            step(reader);
            after_digit = true;
            accept = lexer->lookahead;
        }
        if (!after_digit || accept != '\'') {
            return accept;
        }
        step(reader);
        if (!readable(reader) || !is_class_digit(lexer->lookahead, digits)) {
            return '\'';
        }
    }
}

/// Read the text if it comes next, and give true then. A text that starts in the same way moves the scan
/// to the character after the part that agrees. O(n) in the length of the text.
static bool read_text(Reader *reader, const char *text) {
    for (; *text != '\0'; text++) {
        if (!readable(reader) || reader->lexer->lookahead != (int32_t)*text) {
            return false;
        }
        step(reader);
    }
    return true;
}

/// Read the vendor suffix of Microsoft after the digits of an integer or after a `u`: `1i8`, `1ui64`.
/// `accept` is the character after the text that the caller read. Give the character after the suffix.
/// O(1).
static int32_t read_size_suffix(Reader *reader, int32_t accept) {
    TSLexer *lexer = reader->lexer;
    if (lexer->lookahead != 'i' && lexer->lookahead != 'I') {
        return accept;
    }
    step(reader);
    if (read_text(reader, "8") || read_text(reader, "16") || read_text(reader, "32") || read_text(reader, "64")) {
        return lexer->lookahead;
    }
    return accept;
}

/// Read the suffix of an integer, and give the character after it. `accept` is the character after the
/// digits. The suffixes are the standard suffixes and the vendor suffixes of `number_literal`. O(1).
static int32_t read_integer_suffix(Reader *reader, int32_t accept) {
    TSLexer *lexer = reader->lexer;
    if (accept == '\'') {
        // The scan read a digit separator that no digit follows. The number ends before that separator.
        return accept;
    }
    int32_t first = lexer->lookahead;
    if (first == 'u' || first == 'U') {
        step(reader);
        accept = lexer->lookahead;
        int32_t next = lexer->lookahead;
        if (next == 'l' || next == 'L') {
            // The two letters of `ull` have the same case, and `ulL` ends after `ul`.
            step(reader);
            accept = lexer->lookahead;
            if (lexer->lookahead == next) {
                step(reader);
                accept = lexer->lookahead;
            }
            return accept;
        }
        if (next == 'z' || next == 'Z') {
            step(reader);
            return lexer->lookahead;
        }
        return read_size_suffix(reader, accept);
    }
    if (first == 'l' || first == 'L') {
        step(reader);
        accept = lexer->lookahead;
        if (lexer->lookahead == first) {
            step(reader);
            accept = lexer->lookahead;
        }
        if (accept == 'u' || accept == 'U') {
            step(reader);
            accept = lexer->lookahead;
        }
        return accept;
    }
    if (first == 'z' || first == 'Z') {
        step(reader);
        accept = lexer->lookahead;
        if (accept == 'u' || accept == 'U') {
            step(reader);
            accept = lexer->lookahead;
        }
        return accept;
    }
    return read_size_suffix(reader, accept);
}

/// Read the suffix of a floating-point number, and give the character after it. `accept` is the
/// character after the digits. O(1).
static int32_t read_float_suffix(Reader *reader, int32_t accept) {
    TSLexer *lexer = reader->lexer;
    if (accept == '\'') {
        // The scan read a digit separator that no digit follows. The number ends before that separator.
        return accept;
    }
    int32_t first = lexer->lookahead;
    if (first == 'f' || first == 'F') {
        step(reader);
        accept = lexer->lookahead;
        if (lexer->lookahead == '1') {
            step(reader);
            if (read_text(reader, "6") || read_text(reader, "28")) {
                accept = lexer->lookahead;
            }
        } else if (read_text(reader, "32") || read_text(reader, "64")) {
            accept = lexer->lookahead;
        }
        return accept;
    }
    if (first == 'l' || first == 'L' || first == 'q' || first == 'Q' || first == 'w' || first == 'W') {
        step(reader);
        return lexer->lookahead;
    }
    if ((first == 'b' && read_text(reader, "bf16")) || (first == 'B' && read_text(reader, "BF16"))) {
        return lexer->lookahead;
    }
    return accept;
}

/// Read an exponent: its letter, an optional sign, and the digits. Give the character after the digits,
/// or -1 when no digit follows the letter. O(n) in the characters that the scan reads.
static int32_t read_exponent(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    step(reader);
    if (lexer->lookahead == '+' || lexer->lookahead == '-') {
        step(reader);
    }
    return read_digits(reader, DIGITS_DECIMAL, -1, false);
}

/// Read the end of a floating-point number after its first digits: the fraction, the exponent, and the
/// suffix. `accept` is the character after those digits. Give the character after the number.
/// O(n) in the characters that the scan reads.
static int32_t read_float_end(Reader *reader, int32_t accept) {
    TSLexer *lexer = reader->lexer;
    if (accept == '.') {
        // `1.` is a number, and the fraction after the `.` can be empty.
        step(reader);
        accept = read_digits(reader, DIGITS_DECIMAL, lexer->lookahead, false);
    }
    if (accept == 'e' || accept == 'E') {
        int32_t exponent = read_exponent(reader);
        if (exponent < 0) {
            // The number ends before the letter, and the letter starts a ud-suffix.
            return accept;
        }
        accept = exponent;
    }
    return read_float_suffix(reader, accept);
}

/// Read a hexadecimal number from its `x`, and give the character after it. `accept` is that `x`, which
/// is the character after the `0` that the caller read. A hexadecimal number with a `.` needs an
/// exponent. O(n) in the characters that the scan reads.
static int32_t read_hex_number(Reader *reader, int32_t accept) {
    TSLexer *lexer = reader->lexer;
    step(reader);
    int32_t digits = read_digits(reader, DIGITS_HEX, -1, false);
    if (digits == 'p' || digits == 'P') {
        int32_t exponent = read_exponent(reader);
        return exponent < 0 ? digits : read_float_suffix(reader, exponent);
    }
    if (digits >= 0 && digits != '.') {
        return read_integer_suffix(reader, digits);
    }
    if (digits < 0 && lexer->lookahead != '.') {
        // The number is the `0`, and the `x` starts a ud-suffix.
        return accept;
    }
    step(reader);
    int32_t fraction = read_digits(reader, DIGITS_HEX, digits < 0 ? -1 : lexer->lookahead, false);
    if (fraction == 'p' || fraction == 'P') {
        int32_t exponent = read_exponent(reader);
        if (exponent >= 0) {
            return read_float_suffix(reader, exponent);
        }
    }
    // The number has no exponent. It is the digits before the `.`, or the `0` when it has no digits.
    return digits < 0 ? accept : digits;
}

/// Read the number after a sign, and give the character after it. The number is the longest text that
/// `number_literal` matches without its sign. Give -1 when no number follows the sign, and also when the
/// number is an octal number with the digit `8` or `9`, which no ud-suffix follows.
/// O(n) in the characters that the scan reads.
static int32_t read_number(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    if (lexer->lookahead == '.') {
        step(reader);
        int32_t accept = read_digits(reader, DIGITS_DECIMAL, -1, false);
        return accept < 0 ? -1 : read_float_end(reader, accept);
    }
    if (!is_digit(lexer->lookahead)) {
        return -1;
    }
    bool zero = lexer->lookahead == '0';
    step(reader);
    int32_t accept = lexer->lookahead;
    if (zero && (accept == 'x' || accept == 'X')) {
        return read_hex_number(reader, accept);
    }
    if (zero && (accept == 'b' || accept == 'B')) {
        step(reader);
        int32_t digits = read_digits(reader, DIGITS_BINARY, -1, false);
        // The number is the `0` when no binary digit follows, and the `b` starts a ud-suffix.
        return digits < 0 ? accept : read_integer_suffix(reader, digits);
    }
    if (zero) {
        // The `0` is an octal number. A `8` or a `9` after it is part of a floating-point number only.
        accept = read_digits(reader, DIGITS_OCTAL, accept, true);
        if (accept == '8' || accept == '9') {
            int32_t decimal = read_digits(reader, DIGITS_DECIMAL, accept, true);
            if (decimal != '.' && decimal != 'e' && decimal != 'E') {
                return -1;
            }
            return read_float_end(reader, decimal);
        }
    } else {
        accept = read_digits(reader, DIGITS_DECIMAL, accept, true);
    }
    if (accept == '.' || accept == 'e' || accept == 'E') {
        return read_float_end(reader, accept);
    }
    return read_integer_suffix(reader, accept);
}

/// Scan the sign of a number that has a ud-suffix, and give the sign as its own token: `-1_k`, `+5ms`.
/// The scan reads a maximum of `MAX_NUMBER_LOOKAHEAD` characters.
static bool scan_sign_before_suffixed_number(TSLexer *lexer) {
    lexer->result_symbol = lexer->lookahead == '-' ? MINUS_BEFORE_SUFFIXED_NUMBER : PLUS_BEFORE_SUFFIXED_NUMBER;
    Reader reader = start_reader(lexer, MAX_NUMBER_LOOKAHEAD, NULL);
    step(&reader);
    mark_end(lexer);
    int32_t after = read_number(&reader);
    // A scan that used its full budget gives no token, as the other scans do.
    return reader.budget > 0 && after >= 0 && starts_ud_suffix(after);
}

/// Read the word at the start of a declarator, an item, a statement, or an expression, and select
/// the token for it.
///
/// The lexer cannot move back after it reads the word. For this reason, this function reads the
/// word one time for all tokens that start with a word, in this order:
/// - A `::`, a `<`, or the `(` of `decltype` immediately after the word starts the scope of a
///   member pointer (`scan_member_pointer_scope`).
/// - `auto` can start a decay-copy (`scan_decay_copy_auto`) or a structured binding
///   (`scan_structured_binding_auto`).
/// - A macro name can come between a type and a declarator (`scan_call_macro_name`).
/// - A Qt word, `va_arg`, or the name of a compiler trait selects its marker.
/// - A class key can start a class head, and the scanner records its name (`scan_class_head`).
/// - A name before `<` can be an operand of a comparison (`scan_comparison_name`).
/// - A different word can start a macro invocation, or the macros before a constructor
///   (`scan_macro_start`).
///
/// The tokens of a member pointer, a class head, and a macro start are empty. The token of a decay-copy,
/// a structured binding, a call macro, and a comparison name holds the word.
/// True where an element of a braced list starts and where no expression continues. The lookahead is
/// the first token after the arguments of a name. A word that continues the expression before it, as
/// the alternative operator `and` of [lex.digraph], starts no element. O(n) in the length of a word.
static bool starts_initializer_element(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    int32_t c = lexer->lookahead;
    if (c == '"' || c == '\'' || c == '{' || is_digit(c)) {
        return true;
    }
    if (!is_word_start(c)) {
        return false;
    }
    char word[MACRO_WORD_SIZE];
    bool has_lower = false;
    read_word(reader, word, &has_lower);
    return !word_in(word, CONTINUATION_KEYWORDS);
}

static bool scan_word_start(Scanner *scanner, TSLexer *lexer, const bool *valid_symbols) {
    mark_end(lexer);
    Reader reader = start_reader(lexer, MACRO_SCAN_LIMIT, scanner);
    // ONE READ FILLS TWO BUFFERS. `full` holds the name that a
    // seed lookup compares, and `word` holds the first `MACRO_WORD_SIZE - 1` characters, which is
    // what every comparison with a list takes. The copy is byte for byte what `read_word` wrote
    // before, because `read_word_sized` cuts at the same character and writes the NUL at the end.
    char full[TS_CPP_SEED_WORD_SIZE];
    bool has_lower = false;
    read_word_sized(&reader, full, TS_CPP_SEED_WORD_SIZE, &has_lower);
    char word[MACRO_WORD_SIZE];
    strncpy(word, full, MACRO_WORD_SIZE - 1);
    word[MACRO_WORD_SIZE - 1] = '\0';
    int length = (int)strlen(word);
    int32_t next = lexer->lookahead;
    bool is_decltype = strcmp(word, "decltype") == 0;
    if (valid_symbols[MEMBER_POINTER_START] && (next == ':' || next == '<' || (is_decltype && next == '('))) {
        lexer->result_symbol = MEMBER_POINTER_START;
        reader.budget = MAX_MEMBER_POINTER_LOOKAHEAD - length;
        return scan_member_pointer_scope(&reader, is_decltype);
    }
    if (strcmp(word, "auto") == 0 && (valid_symbols[DECAY_COPY_AUTO] || valid_symbols[STRUCTURED_BINDING_AUTO])) {
        // A decay-copy has `(` or `{` immediately after `auto`, and a structured binding has not.
        if (next == '(' || next == '{') {
            return valid_symbols[DECAY_COPY_AUTO] && scan_decay_copy_auto(lexer, scanner);
        }
        return valid_symbols[STRUCTURED_BINDING_AUTO] && scan_structured_binding_auto(lexer, scanner);
    }
    // A call macro has a declarator after it, and never an argument list. After a macro, as in
    // `[[noreturn]] A B(X) void f();`, the parser can also read a macro call before the type.
    bool call_attribute =
        (valid_symbols[MACRO_CALL_ATTRIBUTE_START] || valid_symbols[MACRO_CALL_ATTRIBUTE_TOKENS_START] ||
         valid_symbols[MACRO_TYPE_START] || valid_symbols[PARAMETER_MACRO_TYPE_START]) &&
        next == '(';
    bool pointer_macro = valid_symbols[POINTER_CALL_MACRO_NAME];
    bool macro_shaped = length >= 2 && is_macro_name(word, has_lower);
    // A macro between the type of a declaration and the declarator of an OBJECT. The macro cannot be
    // the type itself here. After one plain name the parser holds a version in which THAT name is the
    // macro and the type is still open, and a type specifier can start at this word. The marker of a
    // compiler trait that gives a type is valid exactly where a type specifier can start, and it tells
    // the two positions apart: `T MACRO x;` and `T MACRO(mu) x;` keep their readings, and
    // `unsigned int ALIGN16 t[2];` has a complete type before the macro.
    bool type_attribute = valid_symbols[TYPE_ATTRIBUTE_MACRO_NAME] && !valid_symbols[TYPE_TRAIT_TYPE_MARKER] &&
                          !valid_symbols[MACRO_TYPE_START] && !valid_symbols[PARAMETER_MACRO_TYPE_START];
    if ((valid_symbols[CALL_MACRO_NAME] || pointer_macro || type_attribute) && macro_shaped && !call_attribute) {
        return scan_call_macro_name(lexer, scanner, pointer_macro, true, type_attribute);
    }
    // A macro between the name of a function and its argument list, after a qualified name. The
    // grammar makes the token valid there only. Where a declarator can start after a type, the call
    // macro token is also valid, and an uppercase name is the declarator: `std::string NAME(x);`.
    //
    // A type-id keeps its reading. In the template argument
    // `::boost::recursive_variant_ BOOST_MPL_AUX_LAMBDA_ARITY_PARAM(Arity)` of
    // boost/variant/recursive_variant.hpp:59, the name is a type and the macro is the attribute macro
    // of an abstract function declarator. The token of that declarator is valid there, and the token
    // of a call takes the name away from it.
    if (valid_symbols[CALL_NAME_MACRO_NAME] && !valid_symbols[CALL_MACRO_NAME] &&
        !valid_symbols[DECLARATOR_NAME_MACRO_NAME] && length >= 2 && is_macro_name(word, has_lower)) {
        return scan_call_name_macro(&reader);
    }
    // A calling convention has no arguments, and no `<` comes after it. A name before `(` keeps the scans
    // of a macro call before a parameter, and a name before `<` keeps the scan of a comparison name. Where
    // an expression can also start, as in `(CONST * p)(q);` at the start of a statement, the grouping is the
    // declarator of a constructor, and a calling convention there has no type before it.
    if (valid_symbols[GROUPING_CALL_MACRO_NAME] && !valid_symbols[COMPARISON_NAME] && length >= 2 &&
        is_macro_name(word, has_lower) && !is_grammar_keyword(word)) {
        skip_blanks(&reader);
        if (lexer->lookahead != '(' && lexer->lookahead != '<') {
            return scan_grouping_call_macro_name(&reader);
        }
    }
    if (valid_symbols[QT_EMIT_MARKER] && (strcmp(word, "emit") == 0 || strcmp(word, "Q_EMIT") == 0)) {
        lexer->result_symbol = QT_EMIT_MARKER;
        return scan_qt_emit_marker(lexer, scanner);
    }
    if (valid_symbols[QT_FOREACH_MARKER] && (strcmp(word, "foreach") == 0 || strcmp(word, "Q_FOREACH") == 0)) {
        lexer->result_symbol = QT_FOREACH_MARKER;
        return scan_qt_foreach_marker(lexer, scanner);
    }
    if (valid_symbols[VA_ARG_MARKER] && strcmp(word, "va_arg") == 0) {
        lexer->result_symbol = VA_ARG_MARKER;
        return scan_parenthesis_after_word(lexer, scanner);
    }
    if (valid_symbols[TYPE_TRAIT_MARKER] && word[0] == '_' && is_type_trait(word)) {
        lexer->result_symbol = TYPE_TRAIT_MARKER;
        return scan_parenthesis_after_word(lexer, scanner);
    }
    // The two tables of traits are disjoint, and a word is in one of them only.
    if (valid_symbols[TYPE_TRAIT_TYPE_MARKER] && word[0] == '_' && is_type_trait_type(word)) {
        lexer->result_symbol = TYPE_TRAIT_TYPE_MARKER;
        return scan_parenthesis_after_word(lexer, scanner);
    }
    if (is_class_key(word)) {
        return valid_symbols[CLASS_HEAD_MARK] && scan_class_head(scanner, &reader);
    }
    if (valid_symbols[TEMPLATE_HEAD_MARK] && strcmp(word, "template") == 0) {
        return scan_template_head(scanner, &reader);
    }
    // A macro in the head of a class that has no member. The parser takes the mark after a class key
    // only, and the validity of the token is the evidence of that position.
    if (valid_symbols[CLASS_MACRO_MARK] && scan_class_macro_mark(&reader, word, has_lower)) {
        lexer->result_symbol = CLASS_MACRO_MARK;
        return true;
    }
    // The name of a recorded class head before a `(` is a functional cast: `A(x)`. Only name lookup
    // tells a type name from a function name there (GCC `cp_parser_postfix_expression`, Clang
    // `ParsePostfixExpressionSuffix`). The scan reads no more text: the name and the character after it
    // decide, and each other scan of this function keeps its own reading.
    //
    // The token is not valid where a type specifier can start. `TYPE_TRAIT_TYPE_MARKER` is in
    // `type_specifier`, and its validity tells that a type specifier can start. This condition is
    // necessary for two forms:
    // - A declaration. [stmt.ambig] makes `A(x);` a declaration of `x` in a block, and
    //   `void f(A (x));` a parameter `x`. The token keeps the tree of the declaration.
    // - A type-id. In a template argument, in a cast, and in the operand of `sizeof`, `A(int)` is the
    //   type of a function: `std::function<A(int)>`. The token there gives an ERROR node.
    //
    // The condition also stops the token where a type specifier can start and a cast is correct.
    // `A();` at the start of a statement is an expression statement, because it has no declarator. The
    // token keeps the reading of a call there. The condition above causes this loss.
    //
    // THE RULE IS INCORRECT FOR ONE FORM. A function or an object with the name of a class hides that
    // class ([basic.scope.scope]), and `A(1)` is then a call:
    //
    //     struct A { int m; };
    //     int A(int);
    //     int f() { return A(1); }
    //
    // The two front ends read a call there, and the GCC dump `-fdump-tree-original-raw` gives one
    // `call_expr`. The record holds names and no scopes. It cannot read this form, and only scoped
    // name lookup can repair it. A second record of the names that a declaration declares also has no
    // scopes. The measurement gave 4 incorrect rows of 224 in the 7646 compiler test files, against
    // approximately 1500 correct casts that a record like that removes in the corpus. Because of this
    // measurement, the fork keeps the incorrect form.
    // The operand of `alignas` is a type or a constant expression, and the grammar reads a bare name
    // as an expression. The token gives the type reading for a name that a source declares as a
    // type. The parser makes the token valid in that one position, so the validity is the evidence
    // of the position.
    if (valid_symbols[ALIGNAS_TYPE_NAME] && is_alignas_type_name(scanner, &reader)) {
        // The end of the token is the end of the name. The scan reads past it to look at the next
        // character, and the mark holds.
        mark_end(lexer);
        // THE NAME IS THE WHOLE OPERAND OR THERE IS NO TOKEN. The token stands for the operand, and
        // a name that something follows is only its first part:
        //   `alignas(T...)`                  a pack expansion of the alignment-specifier
        //   `alignas(Config::inline_align)`  a qualified name
        //   `alignas(std::atomic<uint64_t>)` a qualified template-id
        // The first build of this rule gave the token for the first name of each of those and left
        // the rest of the operand with nothing to read, which gave 4 NEW ERRORS in the corpus and an
        // ERROR for the syntax snippet `s11_alignas_pack`. A `)` after the name is the evidence that
        // the name is the whole operand. Every other operand keeps the reading of c4e81d4.
        Gap after = {0};
        skip_gap(&reader, &after);
        if (after.blocked || !readable(&reader) || lexer->lookahead != ')') {
            return false;
        }
        lexer->result_symbol = ALIGNAS_TYPE_NAME;
        return true;
    }
    bool cast_name = valid_symbols[FUNCTIONAL_CAST_NAME] && !valid_symbols[TYPE_TRAIT_TYPE_MARKER] &&
                     !valid_symbols[MACRO_TYPE_START] && !valid_symbols[PARAMETER_MACRO_TYPE_START] &&
                     !is_grammar_keyword(word) &&
                     (is_class_name(scanner, reader.word_hash) || is_loose_name(scanner, reader.word_hash) ||
                      is_seed_type_name(scanner, full, &reader));
    if (cast_name && next == '(') {
        mark_end(lexer);
        lexer->result_symbol = FUNCTIONAL_CAST_NAME;
        return true;
    }
    bool invocation = valid_symbols[MACRO_LINE_START] || valid_symbols[MACRO_BLOCK_START] ||
                      valid_symbols[MACRO_CALL_START] || valid_symbols[MACRO_ENUMERATOR_START];
    bool macro = invocation || valid_symbols[CONSTRUCTOR_MACRO_START] || valid_symbols[MACRO_CALL_ATTRIBUTE_START] ||
                 valid_symbols[MACRO_CALL_ATTRIBUTE_TOKENS_START] || valid_symbols[MACRO_TYPE_START] ||
                 valid_symbols[PARAMETER_MACRO_TYPE_START] || valid_symbols[STATEMENT_ATTRIBUTE_MACRO_START];
    // A comparison name has a `<` or a blank after it. A macro name keeps the scan of a macro invocation.
    // A macro before an attribute has a group after it, and the condition `if (N < 0 || N > M)` has a
    // comparison. A name with a blank after it keeps the scan of a macro start where a constructor can
    // start, because the scan of a comparison cannot go back: `simdjson_inline parser::parser()`. The
    // check of the next character comes before the long lists of keywords.
    bool blank = (next == ' ' || next == '\t') && !valid_symbols[CONSTRUCTOR_MACRO_START];
    // After the first macro of an attributed statement, a second macro name keeps the scan of a macro
    // start: `SUPPRESS_A SUPPRESS_B return f();`.
    bool macro_word = (invocation || valid_symbols[STATEMENT_ATTRIBUTE_MACRO_START]) &&
                      is_macro_name(word, has_lower);
    bool comparison = valid_symbols[COMPARISON_NAME] && (next == '<' || blank) && !macro_word &&
                      !word_in(word, LINE_KEYWORDS) && !word_in(word, PREFIX_WORDS) &&
                      !word_in(word, TYPE_WORDS) && !word_in(word, NOT_PARAMETER_WORDS) &&
                      !word_in(word, NOT_COMPARISON_NAMES);
    // A template-id of a recorded class before a `(` is a functional cast: `B<int>(x)`. One scan gives
    // the two answers, because a scan that declines cannot go back to the start of the brackets.
    if (comparison || (cast_name && (next == '<' || blank))) {
        return scan_comparison_name(&reader, word, comparison, cast_name);
    }
    // After the `*` or the `&` of a declarator, a name before the name of the declarator is a macro,
    // and the shape of the name has no effect: `char* absl_nonnull error`. A qualifier there is a
    // keyword of the grammar, and a recorded class name there is the type of a functional cast:
    // `*proxy(jv)`. The two front ends reject `char* a b` without a macro (GCC
    // `cp_parser_direct_declarator`, Clang `ParseDirectDeclarator`). Between a type and the name of a
    // function, `Foo Bar f()` has two readings, and the scan keeps the shape of the name there. This
    // scan reads the name after the word, and the lexer cannot go back, so it comes after each scan
    // that reads the next character only.
    bool pointer_name = pointer_macro && !macro_shaped && !call_attribute && !cast_name && !is_sal_name(word) &&
                        !is_grammar_keyword(word) && !word_in(word, POINTER_QUALIFIER_WORDS);
    if (pointer_name) {
        return scan_call_macro_name(lexer, scanner, true, false, false);
    }
    // A macro at the head of an element of a braced list gives several initializers, and an element
    // comes after its arguments with no comma between the two:
    // `{ PyVarObject_HEAD_INIT(nullptr, 0) "n", 0 }` builds a type object of CPython.
    //
    // THE POSITION GIVES THE EVIDENCE AND THE NAME PROVES NOTHING. A name with an argument list, and
    // an element after it with no comma, is no element list of C++. GCC `cp_parser_initializer_list`
    // (gcc/cp/parser.cc:29234) ends the list at a token that is not a `,` (parser.cc:29396), and
    // Clang `Parser::ParseBraceInitializer` (clang/lib/Parse/ParseInit.cpp:411) does the same
    // (ParseInit.cpp:501). Both reject the text without the macro. 510 of the 678 sites in the
    // 329,387 corpus files of 2026-09-16 hold a lowercase letter in the name, so a test of the name
    // reaches few of them.
    //
    // THE TOKEN IS A HARD GATE. Where the scan gives it, the parser reads the macro and no other
    // reading of the same text stays alive. Where the scan declines, the macro reading is not
    // reachable and the text keeps the ERROR node that it has today. A token that this scan gives by
    // mistake makes a correct call into a macro invocation with no ERROR node, so the condition
    // takes only text that is invalid without a macro.
    //
    // The scan reads the arguments of the name, and the lexer cannot go back, so it comes after each
    // scan that reads the next character only.
    // TWO SCANS READ THE ARGUMENTS OF THE NAME, AND ONE SCAN ANSWERS THE TWO. The head of an element
    // of a braced list has an element after its arguments, and a macro call that gives a scope has a
    // `::` after them. The lexer cannot go back, so a second scan of the same group is not possible.
    // The scan of the scope runs only where the scans of a macro invocation do not, because those
    // read the same name for their own forms and they keep it. `scan_macro_start` gives the scope
    // token for the names that reach it.
    // A macro call that is a part of a concatenation has a STRING after its arguments:
    // `"arena." STRINGIFY(ALL) ".purge"`. The check comes after the check of the braced list,
    // because a string also starts an element there and `_initializer_macro_start` keeps that
    // position. Refer to `_concatenated_macro_call` in the grammar.
    bool scope_start = valid_symbols[MACRO_SCOPE_START] && !macro;
    // The token is valid wherever a concatenation can start, which is most expression positions, so
    // the scan runs only where the scans of a macro invocation do not. Those read the same name for
    // their own forms and they keep it, and a scan that declines here gives no token at all.
    bool concatenated = valid_symbols[CONCATENATED_MACRO_START] && !macro;
    if ((valid_symbols[INITIALIZER_MACRO_START] || scope_start || concatenated) && next == '(' &&
        !is_grammar_keyword(word)) {
        Arguments arguments = {0};
        if (!skip_group(&reader, &arguments) || reader.budget == 0) {
            return false;
        }
        Gap gap = {0};
        skip_gap(&reader, &gap);
        if (gap.blocked || reader.budget == 0) {
            return false;
        }
        if (valid_symbols[INITIALIZER_MACRO_START] && starts_initializer_element(&reader)) {
            lexer->result_symbol = INITIALIZER_MACRO_START;
            return true;
        }
        // THE CHECK OF THE STRING COMES BEFORE THE CHECK OF THE `::`, BECAUSE `scope_follows` READS
        // THE FIRST `:` AND THE LEXER CANNOT GO BACK. With the other order, `cond ? f(x) : "s"`
        // lost its `:` to that scan and this check then saw the string and gave the token. The two
        // are disjoint at one position: where the lookahead is a `:`, `starts_string_literal` reads
        // no character and gives false, and where it is a quote or an encoding prefix,
        // `scope_follows` gives false at its first character.
        if (concatenated && starts_string_literal(&reader)) {
            lexer->result_symbol = CONCATENATED_MACRO_START;
            return true;
        }
        if (scope_start && scope_follows(&reader)) {
            lexer->result_symbol = MACRO_SCOPE_START;
            return true;
        }
        return false;
    }
    return macro && scan_macro_start(reader, word, has_lower, valid_symbols, scanner);
}

/// Read the assembly code of an MS `__asm { ... }` block, up to the brace that closes the block.
///
/// MSVC reads the block as assembly text, not as C++ tokens. A `{` and a `}` change the depth. A
/// `;` or `//` starts a comment to the end of the line, and a brace in a comment does not count
/// (Clang ParseStmtAsm.cpp, ParseMicrosoftAsmStatement). The token has no white space at its start
/// or at its end. O(n) in the length of the block.
static bool scan_ms_asm_code(TSLexer *lexer) {
    while (iswspace(lexer->lookahead)) {
        LOOP_STEP();
        skip(lexer);
    }
    unsigned depth = 0;
    bool has_code = false;
    while (!lexer->eof(lexer)) {
        LOOP_STEP();
        int32_t character = lexer->lookahead;
        if (character == '}' && depth == 0) {
            break;
        }
        advance(lexer);
        if (character == '{') {
            ++depth;
        } else if (character == '}') {
            --depth;
        } else if (character == ';' || (character == '/' && lexer->lookahead == '/')) {
            while (!lexer->eof(lexer) && !is_line_break(lexer->lookahead)) {
                LOOP_STEP();
                advance(lexer);
            }
        } else if (character == '/' && lexer->lookahead == '*') {
            advance(lexer);
            while (!lexer->eof(lexer)) {
                LOOP_STEP();
                int32_t inner = lexer->lookahead;
                advance(lexer);
                if (inner == '*' && lexer->lookahead == '/') {
                    advance(lexer);
                    break;
                }
            }
        }
        if (!iswspace(character)) {
            has_code = true;
            mark_end(lexer);
        }
    }
    return has_code;
}

void *tree_sitter_cpp_external_scanner_create() {
    // THE PARSER AND THE SCANNER MUST AGREE ON THE COUNT OF THE EXTERNAL TOKENS. `valid_symbols` has
    // one slot for each external token of the parser, in the order of `g.externals`, and the scanner
    // reads the slots with the enumerators of `TokenType`. A parser.c of one generate step and a
    // scanner.c of a different one give two counts, and a slot of the scanner then reads the token of
    // a different slot of the parser. A build of 2026-09-16 held a parser with 73 tokens, where slot
    // 66 was the `{` of an experiment, and a scanner with 72, where slot 66 is the attribute mark. The
    // scanner gave its empty mark nearly everywhere, the parser looped on a token with no width, and
    // one parse of a 20 KB file took 152 GB. The runtime calls this function at the start of the
    // first parse of a parser, before it reads a byte, so the check refuses the pair before a tree.
    const TSLanguage *language = tree_sitter_cpp();
    if (language->external_token_count != (uint32_t)TOKEN_TYPE_COUNT) {
        fprintf(
            stderr,
            "tree-sitter-cpp: the parser of src/parser.c has %u external tokens, and the scanner of "
            "src/scanner.c has %u. The two files come from different generate steps, and a parse with "
            "them does not end. Run `cargo xtask generate`, and build again.\n",
            language->external_token_count,
            (unsigned)TOKEN_TYPE_COUNT
        );
        abort();
    }
    // `bsearch` needs the traits in the order of `strcmp`.
    for (size_t i = 1; i < TYPE_TRAIT_COUNT; ++i) {
        assert(strcmp(TYPE_TRAITS[i - 1], TYPE_TRAITS[i]) < 0 && "The traits are not in order!");
    }
    for (size_t i = 1; i < TYPE_TRAIT_TYPE_COUNT; ++i) {
        assert(strcmp(TYPE_TRAIT_TYPES[i - 1], TYPE_TRAIT_TYPES[i]) < 0 && "The traits are not in order!");
    }
    // A word is the name of a trait that gives a value, or of a trait that gives a type, and never
    // of both.
    for (size_t i = 0; i < TYPE_TRAIT_TYPE_COUNT; ++i) {
        assert(!is_type_trait(TYPE_TRAIT_TYPES[i]) && "A trait is in the two tables!");
    }
    Scanner *scanner = (Scanner *)ts_calloc(1, sizeof(Scanner));
    memset(scanner, 0, sizeof(Scanner));
    return scanner;
}

#ifndef NDEBUG
/// True for a token that can be empty by design. Each other token holds at least one character.
///
/// - The macro invocation tokens, the macro call attribute tokens, and the constructor macro token come before a
///   name. The class head mark comes before a class key.
/// - The Qt markers, the `va_arg` marker, and the trait marker come before their word. The start of a member
///   pointer comes before its scope.
/// - The end of a directive line is empty at the end of the input.
/// - The content of a raw string literal is empty in `R"x()x"`.
static bool can_be_empty(TSSymbol symbol) {
    switch (symbol) {
        case MACRO_LINE_START:
        case MACRO_LINE_AFTER_SPECIFIERS:
        case MACRO_BLOCK_START:
        case MACRO_CALL_START:
        case MACRO_ENUMERATOR_START:
        case MACRO_STATEMENT_START:
        case TEMPLATE_HEAD_MARK:
        case MACRO_CALL_ATTRIBUTE_START:
        case MACRO_CALL_ATTRIBUTE_TOKENS_START:
        case ATTRIBUTE_TOKENS_MARKER:
        case MACRO_TYPE_START:
        case PARAMETER_MACRO_TYPE_START:
        case STATEMENT_ATTRIBUTE_MACRO_START:
        case STATEMENT_ATTRIBUTE_MACRO_TOKENS_START:
        case CONSTRUCTOR_MACRO_START:
        case CLASS_HEAD_MARK:
        case CLASS_MACRO_MARK:
        case QT_EMIT_MARKER:
        case QT_FOREACH_MARKER:
        case VA_ARG_MARKER:
        case TYPE_TRAIT_MARKER:
        case TYPE_TRAIT_TYPE_MARKER:
        case MEMBER_POINTER_START:
        case PREPROC_LINE_END:
        case PREPROC_EXTRA_MARK:
        case MACRO_SCOPE_START:
        case CONCATENATED_MACRO_START:
        case INITIALIZER_MACRO_START:
        case RAW_STRING_CONTENT:
            return true;
        default:
            return false;
    }
}
#endif

static bool scan_token(Scanner *scanner, TSLexer *lexer, const bool *valid_symbols);

bool tree_sitter_cpp_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
    start_loop_guard();
    start_token_guard();
    bool found = scan_token((Scanner *)payload, lexer, valid_symbols);
    assert((!found || marked_length != TOKEN_NOT_MARKED) && "A scanner branch returns a token with no mark_end.");
    assert((!found || marked_length > 0 || can_be_empty(lexer->result_symbol)) &&
           "A scanner branch returns an empty token.");
    return found;
}

/// Scan the next token. `tree_sitter_cpp_external_scanner_scan` gives the guards of a debug build.
static bool scan_token(Scanner *scanner, TSLexer *lexer, const bool *valid_symbols) {

    // In error recovery, each external token is valid. The raw string tokens are valid together only then.
    bool error_recovery = valid_symbols[RAW_STRING_DELIMITER] && valid_symbols[RAW_STRING_CONTENT];

    if (!error_recovery) {
        // No skipping leading whitespace: raw-string grammar is space-sensitive.
        if (valid_symbols[RAW_STRING_DELIMITER]) {
            lexer->result_symbol = RAW_STRING_DELIMITER;
            return scan_raw_string_delimiter(scanner, lexer);
        }

        if (valid_symbols[RAW_STRING_CONTENT]) {
            lexer->result_symbol = RAW_STRING_CONTENT;
            return scan_raw_string_content(scanner, lexer);
        }

        // In a string literal or a character literal, a `#` is text.
        if (valid_symbols[PREPROC_LITERAL_MARKER]) {
            return false;
        }

        // The extra tokens of an `#endif` or an `#else` line come after the directive name, on the
        // same line. A declined scan reads the horizontal space of that line only, and the scans
        // that follow read the same tokens as before.
        if (valid_symbols[PREPROC_EXTRA_MARK] && scan_preproc_extra_mark(scanner, lexer)) {
            return true;
        }

        // Inside a directive line, a line break ends the line, and the next line is not examined. A `(`
        // that comes immediately after the name of a macro opens a parameter list, and the lexer of the
        // parser reads it.
        bool parameters = valid_symbols[PREPROC_PARAMS_MARK] && lexer->lookahead == '(';
        if (valid_symbols[PREPROC_ARG] && !parameters) {
            switch (scan_preproc_arg(lexer)) {
                case ARG_TEXT:
                    return true;
                case ARG_STOP:
                    return false;
                case ARG_LINE_END:
                    return valid_symbols[PREPROC_LINE_END] && scan_line_end(lexer);
            }
        }
        if (valid_symbols[PREPROC_LINE_END]) {
            return scan_line_end(lexer);
        }

        // The assembly code of an MS `__asm` block is valid only after the `{` of the block.
        if (valid_symbols[MS_ASM_CODE]) {
            lexer->result_symbol = MS_ASM_CODE;
            return scan_ms_asm_code(lexer);
        }
    }

    // The scans of directives and of trailing macros need to know if a line break comes before the
    // token. A failed scan cannot move the lexer back. For this reason, the white space is read here
    // one time for all scans.
    Space space = skip_space_before_token(lexer);
    if (space.blocked) {
        return false;
    }

    // A directive, or the skipped text of a line group, comes before each other token. The directive
    // rules are extras, and each parse state accepts their tokens.
    bool pending = is_pending_group(innermost_group(scanner)) && valid_symbols[PREPROC_SKIPPED];
    if (pending || lexer->lookahead == '#' || lexer->lookahead == '%') {
        bool directive = valid_symbols[PREPROC_DIRECTIVE] || valid_symbols[PREPROC_SKIPPED] ||
                         valid_symbols[PREPROC_IF] || valid_symbols[PREPROC_ENDIF];
        return directive && scan_directive(scanner, lexer, valid_symbols, space);
    }
    // In error recovery, the mark of a class head is the only token of this scanner. The versions in
    // recovery then read the mark in the same place as the other versions, as they read a comment.
    if (error_recovery) {
        return is_word_start(lexer->lookahead) && scan_class_key(scanner, lexer);
    }

    // The sign before a number that has a ud-suffix. The token is the `-` or the `+` of the grammar, and
    // the number after it keeps no sign: `-1_k` is a unary operator with the literal `1_k`.
    if ((lexer->lookahead == '-' && valid_symbols[MINUS_BEFORE_SUFFIXED_NUMBER]) ||
        (lexer->lookahead == '+' && valid_symbols[PLUS_BEFORE_SUFFIXED_NUMBER])) {
        return scan_sign_before_suffixed_number(lexer);
    }

    // The `[:` of a splice, and the brackets of an attribute with a gap. No other token of this
    // scanner starts with a bracket.
    if (lexer->lookahead == '[' && (valid_symbols[SPLICE_OPEN] || valid_symbols[ATTRIBUTE_OPEN_BRACKET])) {
        return scan_open_bracket(lexer, scanner, valid_symbols);
    }
    if (lexer->lookahead == ']' && valid_symbols[ATTRIBUTE_CLOSE_BRACKET]) {
        advance(lexer);
        return scan_attribute_bracket(lexer, scanner, ']', ATTRIBUTE_CLOSE_BRACKET);
    }

    // The digraph `<:`. No other token of this scanner starts with a `<`.
    if (lexer->lookahead == '<' && valid_symbols[OPEN_BRACKET]) {
        return scan_less_than_digraph(lexer);
    }

    // A macro name after a declarator. No other token of this scanner is valid after a declarator. After
    // the parameter list of a declarator that declares an object, as `*(*p)(int)`, the two macro tokens
    // are valid. The macro is then part of the function declarator, as a GNU attribute is.
    // A macro name after the name of an enumerator is on the line of that name. On a different line, also
    // after a directive line, a name is an enumerator after a macro that expands to enumerators.
    if (valid_symbols[ENUMERATOR_MACRO_NAME]) {
        if (space.line_break || lexer->get_column(lexer) == space.spaces || !read_macro_name(lexer)) {
            return false;
        }
        mark_end(lexer);
        lexer->result_symbol = ENUMERATOR_MACRO_NAME;
        return true;
    }
    if (valid_symbols[DECLARATOR_MACRO_NAME] || valid_symbols[TRAILING_MACRO_NAME]) {
        bool declarator = valid_symbols[DECLARATOR_MACRO_NAME] && !valid_symbols[TRAILING_MACRO_NAME];
        // A directive line before the name ends with its line break. After a declarator, the name then
        // starts a line.
        bool line_start = space.line_break || (declarator && lexer->get_column(lexer) == space.spaces);
        return scan_trailing_macro_name(lexer, scanner, line_start, declarator,
                                        valid_symbols[DECLARATOR_NAME_MACRO_NAME]);
    }

    // The `...` of a pack index that a comment, a line splice, or a directive line divides from its `[`.
    if (lexer->lookahead == '.') {
        return valid_symbols[PACK_INDEX_ELLIPSIS] && scan_pack_index_ellipsis(lexer, scanner);
    }

    // A name in parentheses where an expression starts, or after `sizeof` and `typeid`.
    bool name_paren = valid_symbols[NAME_EXPRESSION_PAREN] &&
                      (valid_symbols[CAST_PAREN] || valid_symbols[OPERAND_TYPE_PAREN] || valid_symbols[ALIGNOF_TYPE_PAREN]);
    if (lexer->lookahead == '(') {
        if (valid_symbols[ATTRIBUTE_TOKENS_MARKER]) {
            return scan_attribute_tokens_marker(lexer, scanner);
        }
        return name_paren && scan_parenthesized_name(lexer, scanner, valid_symbols);
    }

    // Each other token starts with a word, and the scope of a member pointer can also start with
    // `::`. One function reads the word for all of these tokens.
    if (lexer->lookahead == ':') {
        return valid_symbols[MEMBER_POINTER_START] && scan_member_pointer_start(lexer, scanner);
    }
    return is_word_start(lexer->lookahead) && scan_word_start(scanner, lexer, valid_symbols);
}

/// The serialized state has these parts, in this order: the delimiter length and the delimiter, the group
/// count and the groups, the count of the deeper groups in two bytes, and the recorded class names. The state of
/// a scanner with no delimiter, no group, and no class name is empty. A deeper group needs a full array of groups,
/// so an empty state has no such group.
unsigned tree_sitter_cpp_external_scanner_serialize(void *payload, char *buffer) {
    static_assert(1 + MAX_DELIMITER_LENGTH * sizeof(wchar_t) + 1 + MAX_GROUPS + 2 + 1 + 1 +
                          1 + 3 * MAX_CLASSES * sizeof(uint32_t) <
                      TREE_SITTER_SERIALIZATION_BUFFER_SIZE,
                  "Serialized state is too long!");

    Scanner *scanner = (Scanner *)payload;
    if (scanner->delimiter_length == 0 && scanner->group_count == 0 && scanner->class_count == 0 &&
        scanner->loose_count == 0 && scanner->template_count == 0 && !scanner->preproc_extra_tokens) {
        return 0;
    }
    unsigned size = 0;
    buffer[size++] = (char)scanner->delimiter_length;
    memcpy(&buffer[size], scanner->delimiter, scanner->delimiter_length * sizeof(wchar_t));
    size += scanner->delimiter_length * sizeof(wchar_t);
    buffer[size++] = (char)scanner->group_count;
    memcpy(&buffer[size], scanner->groups, scanner->group_count);
    size += scanner->group_count;
    buffer[size++] = (char)(scanner->deep_groups & 0xff);
    buffer[size++] = (char)(scanner->deep_groups >> 8);
    buffer[size++] = (char)scanner->preproc_extra_tokens;
    // MEASUREMENT OF TASK 240. The second record carries its own count, so the names of the first
    // record stay the last part of the state and the reader takes them from the remaining length.
    buffer[size++] = (char)scanner->loose_count;
    memcpy(&buffer[size], scanner->loose, scanner->loose_count * sizeof(uint32_t));
    size += scanner->loose_count * sizeof(uint32_t);
    // TASK 291. The third record carries its own count for the same reason the second one does: the
    // names of the first record stay the last part of the state, and its reader takes them from the
    // remaining length.
    buffer[size++] = (char)scanner->template_count;
    memcpy(&buffer[size], scanner->templates, scanner->template_count * sizeof(uint32_t));
    size += scanner->template_count * sizeof(uint32_t);
    memcpy(&buffer[size], scanner->classes, scanner->class_count * sizeof(uint32_t));
    size += scanner->class_count * sizeof(uint32_t);
    return size;
}

void tree_sitter_cpp_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
    Scanner *scanner = (Scanner *)payload;
    reset(scanner);
    scanner->group_count = 0;
    scanner->deep_groups = 0;
    scanner->class_count = 0;
    scanner->loose_count = 0;
    scanner->template_count = 0;
    scanner->preproc_extra_tokens = false;
    if (length == 0) {
        return;
    }
    unsigned size = 0;
    scanner->delimiter_length = (uint8_t)buffer[size++];
    assert(scanner->delimiter_length <= MAX_DELIMITER_LENGTH && "Can't decode serialized delimiter!");
    memcpy(scanner->delimiter, &buffer[size], scanner->delimiter_length * sizeof(wchar_t));
    size += scanner->delimiter_length * sizeof(wchar_t);
    if (size < length) {
        scanner->group_count = (uint8_t)buffer[size++];
        memcpy(scanner->groups, &buffer[size], scanner->group_count);
        size += scanner->group_count;
        assert(size + 2 <= length && "Can't decode the count of the deeper groups!");
        scanner->deep_groups = (uint16_t)((uint8_t)buffer[size] | ((uint8_t)buffer[size + 1] << 8));
        size += 2;
        assert(size < length && "Can't decode the mark of the extra tokens!");
        scanner->preproc_extra_tokens = buffer[size++] != 0;
        assert(size < length && "Can't decode the count of the second record!");
        scanner->loose_count = (uint8_t)buffer[size++];
        assert(scanner->loose_count <= MAX_CLASSES && "Can't decode the names of the second record!");
        memcpy(scanner->loose, &buffer[size], scanner->loose_count * sizeof(uint32_t));
        size += scanner->loose_count * sizeof(uint32_t);
        assert(size < length && "Can't decode the count of the third record!");
        scanner->template_count = (uint8_t)buffer[size++];
        assert(scanner->template_count <= MAX_CLASSES && "Can't decode the names of the third record!");
        memcpy(scanner->templates, &buffer[size], scanner->template_count * sizeof(uint32_t));
        size += scanner->template_count * sizeof(uint32_t);
    }
    unsigned names = size < length ? (length - size) / sizeof(uint32_t) : 0;
    assert(names <= MAX_CLASSES && "Can't decode serialized class names!");
    memcpy(scanner->classes, &buffer[size], names * sizeof(uint32_t));
    scanner->class_count = (uint8_t)names;
}

/// Take the context of the parser, the seed of #275. The runtime calls this function right after
/// `create`, and again when the context changes while the scanner exists, so the scanner holds
/// the current context before each scan. The reader of the seed comes with #275.
void tree_sitter_cpp_external_scanner_set_context(void *payload, const void *context) {
    Scanner *scanner = (Scanner *)payload;
    scanner->context = context;
}

void tree_sitter_cpp_external_scanner_destroy(void *payload) {
    Scanner *scanner = (Scanner *)payload;
    ts_free(scanner);
}
