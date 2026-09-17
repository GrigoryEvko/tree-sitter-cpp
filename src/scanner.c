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
    /// The operand of `alignas`, where a source of the parse declares the name as a type:
    /// `template <typename T> struct S { alignas(T) char storage[sizeof(T)]; };`. The token covers
    /// the name and the grammar reads it as a `type_descriptor`. Refer to `is_recorded_type_name`.
    ALIGNAS_TYPE_NAME,
    /// An empty token before the name of a macro call that is a part of a concatenation:
    /// `"arena." STRINGIFY(MALLCTL_ARENAS_ALL) ".purge"`. The scan gives it when a balanced group
    /// and a string literal come after the name.
    CONCATENATED_MACRO_START,
    /// An empty token before the name of a macro call that gives a whole template parameter:
    /// `template <typename T, FMT_ENABLE_IF(!x)>`. The scan gives it only when the balanced group
    /// after the name CANNOT be a parameter list, because a group that can be one keeps the reading
    /// of a parameter declaration that it has today.
    TEMPLATE_PARAMETER_MACRO_START,
    /// The type of a declaration, where the template head of the same declaration declares the name
    /// as a TYPE PARAMETER and a macro-shaped name follows it: `T HPX_RESTRICT dest`. The token
    /// makes the first name the type, and the bare name after it is then the attribute macro.
    TEMPLATE_PARAMETER_TYPE_NAME,
    /// The type of a declaration, a field, or a parameter, where a source of the parse declares the
    /// name as a type, a plain name follows it, and a macro-shaped name follows THAT name, on the same
    /// line or after a line break: `T value ABSL_ATTRIBUTE_LIFETIME_BOUND`, `Mutex mu MOZ_UNANNOTATED;`.
    /// The token makes the first name the type. The second name is then the declarator, and the third
    /// name is the attribute macro of that declarator. The sources are the template head of the same
    /// declaration, the class heads of the file, and the type rows of the seed of the project. Refer to
    /// `is_declared_type_name`. When no source declares the first name, a macro row of the third name
    /// in the seed is the source, and `scan_template_parameter_declarator` reads it. The token of `TEMPLATE_PARAMETER_TYPE_NAME` covers the shape where
    /// the SECOND name is the macro, and the template head is its only source.
    TEMPLATE_PARAMETER_DECLARATOR_TYPE,
    /// An empty token before a macro name with an argument list and a `;` where an item of a translation
    /// unit or of a namespace body starts: `DECLARE_HANDLE(foo);`.
    MACRO_ITEM_START,
    /// The name of a macro after a declarator, where the seed gives the macro an object row and no
    /// function row, and a group of expressions follows the name: `Mutex l1
    /// MOZ_UNANNOTATED("autolock");`. An object-like macro takes no arguments ([cpp.replace] p10), so
    /// the grammar reads the macro in the place of an attribute and gives the group to the declarator.
    /// Refer to `_declarator_object_macro` in grammar/src/cpp.rs.
    DECLARATOR_OBJECT_MACRO_NAME,
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

/// The maximum number of entries of the record of locals.
///
/// THE BOUND IS THE SERIALIZATION BUFFER OF THE RUNTIME BEFORE ABI 1018. That buffer holds 1,024
/// bytes. The other records take up to 648 of them, and this record takes 3 bytes and 6 bytes for
/// each entry, so 60 entries fit. Over the 329,387 corpus files, the count of the locals in scope at
/// one point is 34 at p99, 67 at p999, and 912 at the maximum. An entry that does not fit makes the
/// record SATURATED, and each lookup then gives `LOCAL_UNKNOWN` until the frame of that entry closes.
/// The record gives up no entry to make room, so a lookup never gives a wrong answer for a lost entry.
#define MAX_LOCALS 60

/// One entry of the record of locals.
typedef struct {
    /// The hash of the name, as `read_word` computes it. The value 0 is the entry of a body with no
    /// parameter: it opens the frame of the body, and no name has the hash 0.
    uint32_t name;
    /// The frame that holds the entry. The outermost frame is 1.
    uint8_t frame;
    /// The number of boundaries before the entry is in scope, and 0 for an entry in scope. A declaration
    /// waits for 1 boundary. A parameter waits for the `{` of its body, after the braces of a constructor
    /// initializer list: `A(int x) : m{x} {` waits for 3.
    uint8_t pending;
} LocalEntry;

/// The parameters and the block-scope variables in scope at the position of the scanner. Refer to
/// `scan_local_boundary`.
typedef struct {
    uint8_t count;
    /// The open frames. A frame is a `{` inside a body, or the body itself.
    uint8_t frames;
    /// 0, or the lowest frame of an entry that did not fit.
    uint8_t saturated;
    /// The frames that are the body of a class, a union or an enum, as bits: bit n - 1 for frame n. A
    /// frame deeper than 32 has no bit. The declarators after the `}` of such a body have its type:
    /// `struct { int a; } s = {1};`.
    uint32_t class_bodies;
    /// 1 from the scan of a class head to the `{` of its body, and 0 otherwise.
    uint8_t class_head;
    LocalEntry entries[MAX_LOCALS];
} LocalRecord;

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
    /// recent last. Two readers: `is_recorded_type_name` for `alignas(T)` and for a whole template
    /// parameter, and `scan_word_start` for
    /// the token `TEMPLATE_PARAMETER_TYPE_NAME`, which reads a type parameter before a macro-shaped
    /// name (`T HPX_RESTRICT dest`). An earlier form of this comment named `alignas` alone, and the
    /// second reader has read the record since the token was added.
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
    /// The number of recorded `using` alias names.
    uint8_t alias_count;
    /// The hashes of the names that `using` alias declarations of the file declare, the most recent
    /// last. `alignas(T)` reads them, AND NOTHING ELSE DOES.
    ///
    /// THE RECORD IS ITS OWN, AND THAT IS THE POINT. A shared record is an interface: putting these
    /// names into `classes` or `loose` would give them to the functional cast, which reads those two
    /// at its own position, and would change trees far beyond the task that added them. A record of
    /// its own cannot reach a consumer that does not name it.
    uint32_t aliases[MAX_CLASSES];
    /// The parameters and the block-scope variables in scope. No consumer reads the record yet. Refer
    /// to `scan_local_boundary` and `local_name_answer`.
    LocalRecord locals;
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
    /// A `;` or a statement keyword shows that the group holds statements. A body after a parameter
    /// list in a group in braces also shows it.
    bool statements;
    /// A `;` or a statement keyword is at the top level of the group. A lambda with a parameter list
    /// sets `statements` and not this field: `{ [](){ return 1; } }` is a braced initializer.
    ///
    /// ONLY THE SCAN OF A BODY AFTER A GROUP THAT CAN BE A PARAMETER LIST READS THIS. Refer to
    /// `scan_macro_invocation`.
    bool statement_token;
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
    /// An argument is one word of `SPECIFIER_WORDS`, so that argument is not a parameter
    /// declaration: `TEST(format_test, enum)`, `ARROW_PACKED_START(struct, Int96)`.
    ///
    /// ONLY THE SCAN OF A BODY READS THIS. `not_parameters` has eight readers, and a group with
    /// this shape and NO body keeps each of those readings: `MACRO(const, noexcept);` of
    /// v8/src/base/functional/bind-internal.h:228 gave a NEW ERROR when this flag set that one.
    bool specifier_only;
    /// The group has one argument with the shape of a parenthesized declarator: a `*`, a `&`, or a
    /// `&&` first, and then the tokens of a type-id: `(*p)`, `(&r)`, `(*const p)`.
    bool pointer_declarator;
    /// An argument of a nested group that must hold parameters starts with a literal or an operator.
    /// The nested group is then a call, and no argument of the group is a parameter declaration:
    /// `MATCHER_P(IsNode, height, absl::StrCat("height ", height))`. A default argument holds a call
    /// of its own, and the scan does not read the groups after a `=`.
    bool call_arguments;
    /// An argument starts with a `*`, a `&`, or a `&&`. Such an argument can be a declarator of a
    /// pointer or of a reference, whatever comes after the operator: `(*p)`, `(&)`, `(*p[3])`.
    /// `pointer_declarator` takes the tokens after the operator, and this field takes the operator
    /// only, for a reader that must know that a declarator is possible.
    bool pointer_first;
    /// An argument starts with a bracketed group: `((V))`, `((a, b))`, `([1])`. Such an argument can
    /// be a parenthesized declarator, and the tokens in the group decide which one. This record
    /// holds the top level only, so a reader that must know the shape of the group reads this field
    /// and takes no decision from the tokens of the group.
    bool group_first;
} Arguments;

/// The kind of the line that comes after a macro invocation.
typedef enum {
    /// The first token cannot start a line.
    NEXT_BLOCKED,
    /// The line starts with a function or variable declarator, and a type can come before it.
    NEXT_DECLARATOR,
    /// The line starts with the declarator of a constructor or a destructor, which has no type.
    NEXT_CONSTRUCTOR,
    /// The line starts with `try` and a `{` after it, after arguments that can be a parameter list.
    /// The block is a function try block, and the reader stands at its `{`.
    NEXT_TRY_BODY,
    /// The line starts with a different token that can start a line.
    NEXT_OTHER,
} NextLine;

/// The words that cannot be the first word of a parameter declaration.
static const char *const NOT_PARAMETER_WORDS[] = {
    "true",        "false",        "nullptr",    "sizeof",           "alignof", "typeid", "new", "delete",
    "co_await",    "static_cast", "dynamic_cast", "const_cast", "reinterpret_cast", "throw",   NULL,
};

/// The words that a declaration can hold and that are not a type.
///
/// A PARAMETER DECLARATION NEEDS A TYPE. One of these words alone is a specifier with no type after
/// it, so it is not a parameter declaration. GCC accepts `void f(int);` and it refuses
/// `void f(enum);`, `void f(const);`, `void f(static);` and `void f(explicit);`.
///
/// `TYPE_WORDS` HOLDS TWELVE OF THESE WORDS, AND THAT IS WHY THE SCAN READ THEM AS A PARAMETER.
/// `class`, `const`, `enum`, `struct`, `union`, `static`, `inline`, `extern`, `register`, `mutable`,
/// `volatile` and `constexpr` are in that list, because each of them begins a type in a declaration.
/// None of them IS a type. `int` alone is a parameter and `const` alone is not.
///
/// The words that `NOT_PARAMETER_WORDS` and `STATEMENT_KEYWORDS` already hold are not here. GCC
/// refuses `void f(new);` and `void f(return);` too, and the scan already marks those groups.
static const char *const SPECIFIER_WORDS[] = {
    "enum",     "class",    "struct",       "union",    "const",    "volatile", "static",
    "inline",   "virtual",  "extern",       "register", "mutable",  "constexpr", "consteval",
    "constinit", "thread_local", "alignas", "typename", "template", "decltype", "this",
    "explicit", "friend",   "export",       "concept",  "requires", "operator",
    NULL,
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
    /// The first token is a word of `SPECIFIER_WORDS`.
    bool first_is_specifier;
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
    // One specifier and no type. The argument is not a parameter declaration.
    args->specifier_only |= arg->tokens == 1 && arg->first_is_specifier;
    args->pointer_first |= arg->first_is_pointer;
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
            if (arg.tokens == 0) {
                args->group_first = true;
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
            args->statement_token = true;
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
            arg.first_is_specifier = is_word && word_in(word, SPECIFIER_WORDS);
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
static bool is_local_name(const Scanner *scanner, uint32_t name);

/// Classify the line after a macro invocation, from its first tokens.
///
/// The scan starts at the first token of the line. `skip_gap` went past the directive lines and the skipped branches
/// before it, and the first line of code after them is the line that the parser reads after the macro
/// (GCC `cp_parser_constructor_declarator_p` and Clang `isConstructorDeclarator` also read the tokens after the
/// directives). A declarator is a name before `(`, or one name before `;` or `,`. The name can be qualified.
/// One uppercase name before `(` is a macro call. A constructor or a destructor has no type: `A::A(`,
/// `A::~A(`, `~A(`, and the name of a recorded class head before `(` and parameters. After arguments
/// that can be a parameter list, `try` continues a function definition, and `try` before a `{` is
/// the function try block of that definition, which the caller reads as the body of a macro where a
/// call statement can start. Where no statement can start,
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
        if (word_in(word, CONTINUATION_KEYWORDS)) {
            return NEXT_BLOCKED;
        }
        if (after_parameters && strcmp(word, "try") == 0) {
            // A FUNCTION TRY BLOCK CONTINUES THE DEFINITION, AND ITS `{` TELLS THE CALLER SO. `try`
            // after a group that can be a parameter list is the function try block of a definition
            // with that group as its parameters: `TEST(A, B)`, a line break, `try`, `{`, of
            // ClickHouse, and `S(int x)`, a line break, `try : m(x) {` in a class body. The caller
            // reads the first as the body of a macro where a call statement can start, so the `{`
            // after `try` is the fact it needs, and `try :` is not that fact. The scan of the gap
            // after `try` reads no character that the caller reads again, because each caller of
            // NEXT_BLOCKED and of NEXT_TRY_BODY returns with no token or with the token before the
            // name.
            Gap body = {0};
            skip_gap(reader, &body);
            return !body.blocked && lexer->lookahead == '{' ? NEXT_TRY_BODY : NEXT_BLOCKED;
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
    "_Atomic", "_BitInt", "_Complex", "_Generic", "_Nonnull", "_Noreturn", "_Null_unspecified", "_Nullable",
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
    "requires", "return", "sealed", "short", "signals", "signed", "size_t", "sizeof", "slots", "ssize_t",
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

/// True when WORD, of LENGTH characters, can be the name of a macro invocation line that has a lowercase letter:
/// `nssv_RESTORE_WARNINGS()`, `__glibcxx_class_requires(T, C)`, `ClassDef(A, 1)`.
///
/// The word is not a keyword of the grammar, and it does not start with `_` and an uppercase letter. Such a
/// reserved name is a keyword or an annotation of the declaration after it: `_Success_(return != 0)` of MSVC.
/// A word of more than MAX_NAME_LENGTH characters is no keyword here, because the `Name` of the scan of a
/// conditional group holds only its first characters. `group_is_structured` and `scan_macro_start` read the word
/// with this one test, so that the two scans agree. O(log n) in the number of keywords.
static bool is_call_line_word(const char *word, size_t length) {
    bool reserved = word[0] == '_' && word[1] >= 'A' && word[1] <= 'Z';
    return length > 0 && !reserved && (length > MAX_NAME_LENGTH || !is_grammar_keyword(word));
}

/// True when a structured conditional group is open. O(n) in the number of open groups, a maximum of MAX_GROUPS.
static bool in_structured_group(const Scanner *scanner) {
    for (uint32_t i = 0; i < scanner->group_count; i++) {
        if (scanner->groups[i] == GROUP_STRUCTURED) {
            return true;
        }
    }
    return false;
}

/// The keywords of `_constructor_specifiers` in the grammar, which can come between the macros before
/// a constructor.
///
/// The list holds `__restrict` and `__restrict__` and no bare `restrict`, because C++ has no such
/// keyword and the grammar gives the word no rule. Refer to `type_qualifier` in `grammar/src/cpp.rs`.
static const char *const CONSTRUCTOR_SPECIFIER_WORDS[] = {
    "extern",           "static",     "register",   "inline",      "__inline",      "__inline__", "__forceinline",
    "thread_local",     "__thread",   "const",      "constexpr",   "volatile",      "__restrict__",
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

/// Record the name of a `using` alias as the most recent name. Return false if it already is the
/// most recent name.
static bool record_alias_name(Scanner *scanner, uint32_t name) {
    return record_name(scanner->aliases, &scanner->alias_count, name);
}

/// True when a `using` alias declaration of the file declared the name. O(n) in MAX_CLASSES.
static bool is_alias_name(const Scanner *scanner, uint32_t name) {
    if (scanner == NULL) {
        return false;
    }
    for (unsigned i = 0; i < scanner->alias_count; i++) {
        if (scanner->aliases[i] == name) {
            return true;
        }
    }
    return false;
}

/// Record the name of a template type parameter as the most recent name. Return false if it already
/// is the most recent name.
static bool record_template_name(Scanner *scanner, uint32_t name) {
    return record_name(scanner->templates, &scanner->template_count, name);
}

/// Take a name out of the record of template type parameters. O(n) in MAX_CLASSES.
static void forget_template_name(Scanner *scanner, uint32_t name) {
    unsigned kept = 0;
    for (unsigned i = 0; i < scanner->template_count; i++) {
        if (scanner->templates[i] != name) {
            scanner->templates[kept++] = scanner->templates[i];
        }
    }
    scanner->template_count = (uint8_t)kept;
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

static bool is_seed_type_name(const Scanner *scanner, const char *name, const Reader *reader);

/// True when a source of the parse declares the name as a type.
///
/// TWO READERS: the operand of an `alignas`, and a whole template parameter of a template head.
/// Each position holds a name that is a type or is no type, and the sources of that fact are the same.
///
/// ONE LOOKUP WITH SEVERAL SOURCES AND A STATED ORDER OF AUTHORITY, and not several call sites that
/// can disagree. The construct, the file and the project are three sources of ONE fact, and a
/// disagreement between them is where a silent wrong tree would live.
///
///   1. THE CONSTRUCT. A name that a template head of the same declaration declares as a type
///      parameter IS a type, by the grammar. No record and no artifact can go stale under it.
///   2. THE FILE. The class heads and the `using` aliases that the scanner recorded. NO TYPEDEF IS
///      RECORDED: the name of a typedef is its declarator and comes last, and `scan_using_alias`
///      says so below. An earlier form of this line named the typedefs as recorded.
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
static bool is_recorded_type_name(const Scanner *scanner, const char *full, const Reader *reader) {
    if (scanner == NULL) {
        return false;
    }
    // 1. THE CONSTRUCT, task 291 step 2a. 200 sites in 150 files.
    if (is_template_parameter(scanner, reader->word_hash)) {
        return true;
    }
    // 1b. THE BLOCK, task 389. A name that the record of locals holds is a variable of the block, and
    //     the operand is then the constant expression of that variable. `constexpr int N = 8;` and
    //     `alignas(N) char buf[16];` after `struct N { char c; };` give `alignof(buf) == 8` in
    //     Clang 22 and GCC 16, and the same input with no local gives 1. The block comes after the
    //     construct, because no declaration can take the name of a type parameter of its own head.
    //     The corpus holds 0 such sites with no seed, and a seed can give them.
    if (is_local_name(scanner, reader->word_hash)) {
        return false;
    }
    // 2. THE FILE, task 291 step 2b. The class heads that `scan_class_head` recorded, which is what
    //    the functional cast reads at its own position.
    //
    //    THIS READS CLASS HEADS AND `using` ALIASES, which is what the two records hold. Of the 217
    //    sites that the file decides and the construct does not, a class head declares 119 and the
    //    ring reaches 111 of those. The other 98 rest on a `using` alias or a typedef: 90 are an
    //    alias, which `scan_using_alias` records, and 8 are a typedef, which nothing records. An
    //    earlier form of this comment said the scanner records neither, from before the alias record.
    if (is_class_name(scanner, reader->word_hash) || is_loose_name(scanner, reader->word_hash)
        || is_alias_name(scanner, reader->word_hash)) {
        return true;
    }
    // 3. THE PROJECT, task 291 step 3. The seed of the parse, which holds the names that the whole
    //    project declares as a type or as a template. 102 sites in 40 files that no source above
    //    reaches, because the file that uses the name declares it nowhere.
    //
    //    THE GATE RUNS NO SEED, so this line measures zero in every gate and the only evidence that
    //    it does anything is a tree that differs under a seed. Refer to the tests of `seeded` in
    //    xtask/src/scanner.rs.
    return is_seed_type_name(scanner, full, reader);
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

/// True when the parse has a seed that this scanner reads: a context with the magic and the version
/// of src/seed.h. O(1).
///
/// A RULE THAT A MACRO ROW DECIDES READS THIS BEFORE IT READS A CHARACTER. With no seed, no name has
/// a macro row, so such a rule can give no token. The rule then must not enter at all, because a scan
/// that reads characters and declines stops each later scan of `scan_word_start`. The parse with no
/// seed then keeps each tree that it has.
static bool has_seed(const Scanner *scanner) {
    const TSCppSeed *seed = scanner == NULL ? NULL : (const TSCppSeed *)scanner->context;
    return seed != NULL && seed->magic == TS_CPP_SEED_MAGIC && seed->version == TS_CPP_SEED_VERSION;
}

/// True when the seed of the parse names `name` as a macro that a `#define` of the project defines,
/// with either shape.
///
/// A MACRO ROW SAYS THAT A FILE OF THE PROJECT DEFINES THE NAME, AND NOT THAT THE NAME IS A MACRO AT
/// THIS SITE. The collector reads every branch of a conditional and every file, test fixtures too, and
/// qtbase defines `QString()` in a test of moc. A reader asks this at a position where the text of the
/// construct already allows only a macro, and never as the only evidence of a reading.
///
/// A CUT WORD GIVES FALSE, for the reason that `is_seed_type_name` gives. A macro row and a type row
/// of one name are independent, so a name can give true here and in `is_seed_type_name`.
static bool is_seed_macro_name(const Scanner *scanner, const char *name, const Reader *reader) {
    if (scanner == NULL || reader->word_cut) {
        return false;
    }
    uint16_t kinds = seed_kinds((const TSCppSeed *)scanner->context, name, (uint32_t)strlen(name));
    return (kinds & TS_CPP_SEED_MACRO) != 0;
}

/// True when the seed of the project declares the name as a function-like macro. It reads no character.
static bool is_seed_function_macro_name(const Scanner *scanner, const char *name, const Reader *reader) {
    if (scanner == NULL || reader->word_cut) {
        return false;
    }
    uint16_t kinds = seed_kinds((const TSCppSeed *)scanner->context, name, (uint32_t)strlen(name));
    return (kinds & TS_CPP_SEED_FUNCTION_MACRO) != 0;
}

/// True when a class head of the file or the seed of the project declares the name as a type.
///
/// ONE LOOKUP WITH TWO SOURCES, AND EACH SOURCE READS NO CHARACTER. `is_class_name` compares the
/// hash of the name with the record of the class heads that have a member, and `is_seed_type_name`
/// compares the bytes of the name with the entries of the seed. The template head of the same
/// declaration is the third source of the same fact, and `scan_word_start` reads it on a path of
/// its own, because that path was there first and its declines are measured. Refer to
/// `is_recorded_type_name`, which reads the same sources for the operand of `alignas` and for a
/// whole template parameter.
///
/// THE RECORD OF THE CLASS HEADS HOLDS THE LAST MAX_CLASSES NAMES OF THE FILE, so a class that the
/// file declares far above the site is not in it. THE GATE RUNS NO SEED, so the seed source measures
/// zero in every gate, and the test `a_seed_reaches_the_type_before_a_declarator_and_a_macro` in
/// xtask/src/scanner.rs is the evidence that it does anything.
static bool is_declared_type_name(const Scanner *scanner, const char *full, const Reader *reader) {
    // A NAME THAT THE RECORD OF LOCALS HOLDS IS A VARIABLE, AND A VARIABLE IS NO TYPE. `is_bool &&
    // s.empty()` after `const bool is_bool = ...;` in the utility header of TBB read as the type
    // `is_bool` with the macro `&&`, and the two front ends read the two names as one expression.
    // The record holds a new name as pending until the `;` of its declaration, so it claims no local
    // before the point of declaration of [basic.scope.pdecl]. Refer to task 389.
    if (is_local_name(scanner, reader->word_hash)) {
        return false;
    }
    return is_class_name(scanner, reader->word_hash) || is_seed_type_name(scanner, full, reader);
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
        // `SWIFT_EXPORT_FROM(swift_Concurrency) SWIFT_CC(swift)` and
        // `size_t swift_task_registryCount(int a);` of swift.
        //
        // AN EARLIER COMMENT NAMED `bool g(int a, int b);` HERE, AND THE KEYWORD TEST BELOW DECIDES
        // THAT ONE WHATEVER THIS TERMINATOR DOES. The word after the break must be a NAME for this
        // rule to matter.
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
        // A chain of macros can cross a line break, and the word after the break tells the scan what
        // follows. A keyword of the grammar stops the chain, and the test below reads it:
        // `ATTR_WARN_UNUSED_RESULT ATTR_NONNULL(1, 2)` and `bool g(int a, int b);` on the next line.
        // A paragraph that stood above this one described a guard that f27dcd0 removed, "only the
        // name of a macro continues a chain", with the same example. The guard is gone and so is it.
        //
        // A NAME THAT IS NO KEYWORD MUST NOT STOP THE CHAIN, because the type of a declaration is
        // such a name: `SWIFT_EXPORT_FROM(swift_Concurrency) SWIFT_CC(swift)` and
        // `AsyncTaskAndContext swift_task_create(int a);` of swift. GCC and Clang accept that text
        // with no diagnostic.
        //
        // An earlier guard here stopped the chain for each word that was no macro name. It fired 524
        // times over the corpus. 474 of those held a keyword, which the test below stops anyway. The
        // guard alone decided 50, and 48 of those held the shape above, at 23 sites of one file.

        // A chain that crossed a line break and then meets the keyword of a statement is a line of
        // macro invocations, and not the attributes of that statement. The names are alone on their
        // line: `P_(LINE) P_(INPUT)` of bde, and `if (SUCCESS) {` on the line after.
        if (crossed_line && (word_in(word, STATEMENT_ATTRIBUTE_WORDS) || is_macro_name(word, has_lower))) {
            return AFTER_CALL_MACRO_LINE;
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
                // A `;` or an assignment after the parameter list also ends an expression statement,
                // and the macro can be the attribute of that statement: `TEST_CYCLE() f(src, dst);`.
                // The caller keeps the type of a macro whose one argument is a type-id.
                int32_t call_end = lexer->lookahead;
                if (!ends_macro_type_declarator(reader, false, parameter)) {
                    return AFTER_CALL_NONE;
                }
                return call_end == ';' || call_end == '=' ? AFTER_CALL_TYPE_OR_STATEMENT : AFTER_CALL_TYPE;
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
        // A member access after the name is no part of a declarator, so the text is an expression
        // statement and the macro is an attribute of that statement: `SUPPRESS_UNCOUNTED_ARG
        // baseValue.m_structure.forEach(f);` of WebKit. [dcl.decl] gives a declarator no `.`. One
        // character decides it, and the scan reads no more.
        if (c == '.') {
            *statement = true;
            return false;
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

/// The words of the attributes that can come after a class key, before the name of the class, and after a macro
/// name or a macro call in that position.
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
///
/// A `/` that starts no comment is the operator `/` or `/=`, and the scan goes past it as a token: `static
/// constexpr size_t kMaxLength = TypedArray::kMaxByteLength / sizeof(uint8_t);` of v8, `struct APValue::LV :
/// LVBase { static const unsigned InlinePathSpace = (DataSize - sizeof(LVBase)) / sizeof(LValuePathEntry);` of
/// clang, `: public B<N / 2>`, and `T operator/(double) const;`. `skip_gap` reads the `/` and stops there, as
/// `read_text_token` reads it. The scan stopped at that `/` before, and the record held no name for 282 heads of
/// the corpus.
static bool class_body_has_member(Reader *reader) {
    TSLexer *lexer = reader->lexer;
    char word[MACRO_WORD_SIZE];
    bool body = false;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(reader, &gap);
        if (gap.slash) {
            continue;
        }
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
/// after the head, and the body has a member, record the name of the class. Return false: the scan gives
/// no token. When the recorded names change, the result symbol is TREE_SITTER_EXTERNAL_STATE_ONLY, and the
/// runtime of ABI 1018 stores the state on the class key that its internal lexer reads next.
///
/// AN EMPTY EXTRA FOR THE CHANGE WAS A LOOKAHEAD OF ITS OWN. The mark `_class_head_mark` changed trees
/// with no rule that read it: in error recovery, in the order of the versions, and in ties. The record
/// of locals has the same history (5255191). At a class key each path of `scan_word_start` after this
/// scan gives no token, so no token of the parser changes.
///
/// The name of the class is the last name before these tokens. A name can be qualified, and template
/// arguments can come after it: `ns::A<int> {`. The names and the macro calls before it are macros:
/// `class LLVM_ABI A final {`, `struct TSA_CAPABILITY("mutex") M {`. Attributes can come before the
/// first name, and after a macro name or a macro call: `class GTEST_API_ [[nodiscard]] RE {`,
/// `class LLVM_MOVABLE_POLYMORPHIC_TYPE alignas(uint64_t) Cleanup {`. An attribute after a name ends that
/// name, as the group of a macro call does, so the name after the attribute is the name of the class. A
/// head with no name, `struct {`, records no name. Other text, as in `class T>` of a template parameter,
/// or in `struct stat *p;`, records no name. The key of `enum class E {` gives the
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
        // AN ATTRIBUTE-SPECIFIER CAN COME AFTER A MACRO NAME. [class.pre] p1 puts the attribute-specifier-seq
        // after the class key, and an export macro that expands to nothing or to `__attribute__((...))`
        // comes before it: `class GTEST_API_ [[nodiscard]] RE {` of googletest, `class
        // BASE_EXPORT [[nodiscard]] ScopedBlockingCall` of chromium. GCC and Clang accept each such
        // head with the macro defined as nothing. The scan read a `[` after a name as the end of the head, so
        // the record held no name for 73 heads of the corpus, and each rule that reads the record missed
        // those classes: a62d1cb read `RE(const RE& other);` as a macro. Only `[[` starts an attribute here.
        // A single `[` after a name is no class head, as in `struct S x[3];`, and the scan stops.
        if (c == '[') {
            step(reader);
            skip_gap(reader, &gap);
            Arguments attribute = {0};
            if (gap.blocked || lexer->lookahead != '[' || !skip_group(reader, &attribute)) {
                return false;
            }
            skip_gap(reader, &gap);
            if (gap.blocked || lexer->lookahead != ']') {
                return false;
            }
            step(reader);
            name = 0;
            after_name = false;
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
        // An attribute word after a name ends that name, as a `[[` does: `class LLVM_MOVABLE_POLYMORPHIC_TYPE
        // alignas(uint64_t) Cleanup {` of clang. The scan read `alignas` after a name as a reserved word and
        // stopped, and the record held no name for 4 heads of the corpus. `__attribute__` and `__declspec` after
        // a name gave the same result before, through the group of a macro call.
        if (word_in(word, CLASS_ATTRIBUTE_WORDS)) {
            skip_gap(reader, &gap);
            Arguments attribute = {0};
            if (gap.blocked || lexer->lookahead != '(' || !skip_group(reader, &attribute)) {
                return false;
            }
            name = 0;
            after_name = false;
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
    if (changed) {
        lexer->result_symbol = TREE_SITTER_EXTERNAL_STATE_ONLY;
    }
    return false;
}

/// Scan a template head after the word `template`, and record the names that it declares as TYPE
/// parameters. Return false: the scan gives no token. When the recorded names change, the result symbol
/// is TREE_SITTER_EXTERNAL_STATE_ONLY, and the runtime of ABI 1018 stores the state on the word
/// `template` that its internal lexer reads next. Refer to `scan_class_head` for the empty extra that
/// this replaces.
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
    if (changed) {
        lexer->result_symbol = TREE_SITTER_EXTERNAL_STATE_ONLY;
    }
    return false;
}

/// Scan an alias declaration after the word `using`, and record the name that it declares. Return
/// false: the scan gives no token. When the recorded names change, the result symbol is
/// TREE_SITTER_EXTERNAL_STATE_ONLY, and the runtime of ABI 1018 stores the state on the word `using`
/// that its internal lexer reads next. Refer to `scan_class_head` for the empty extra that this
/// replaces.
///
/// AN ALIAS DECLARATION HAS AN `=` AND THE OTHER TWO FORMS OF `using` DO NOT. `using A = B;`
/// declares the type name `A`. `using namespace ns;` and `using ns::f;` declare no type name, and
/// the first word after `using` is `namespace` or the first part of a qualified name in those. The
/// scan reads one word and then requires an `=`, which tells the three apart with no list of
/// keywords. `template <class T> using V = W<T>;` has the same shape after its head.
///
/// A TYPEDEF DECLARES A TYPE NAME TOO AND THIS SCAN DOES NOT READ IT. The name of a typedef is its
/// declarator and it comes last, `typedef int (*fn)(void);`, so reading it needs a declarator scan
/// rather than one word. Of the 98 sites that only an alias or a typedef declares, 90 are a `using`
/// alias and 8 are a typedef. The 8 keep the expression reading.
static bool scan_using_alias(Scanner *scanner, Reader *reader) {
    TSLexer *lexer = reader->lexer;
    Gap gap = {0};
    skip_gap(reader, &gap);
    if (gap.blocked || !readable(reader) || !is_word_start(lexer->lookahead)) {
        return false;
    }
    char word[MACRO_WORD_SIZE];
    bool has_lower = false;
    read_word(reader, word, &has_lower);
    if (word[0] == '\0') {
        return false;
    }
    uint32_t name = reader->word_hash;
    Gap after = {0};
    skip_gap(reader, &after);
    if (after.blocked || !readable(reader) || lexer->lookahead != '=') {
        return false;
    }
    // `==` is a comparison and not the `=` of an alias declaration.
    step(reader);
    if (readable(reader) && lexer->lookahead == '=') {
        return false;
    }
    if (record_alias_name(scanner, name)) {
        lexer->result_symbol = TREE_SITTER_EXTERNAL_STATE_ONLY;
    }
    return false;
}

/// True if the word is a class key: `class`, `struct`, `union`, or the MSVC `__interface`.
static bool is_class_key(const char *word) {
    return strcmp(word, "class") == 0 || strcmp(word, "struct") == 0 || strcmp(word, "union") == 0 ||
           strcmp(word, "__interface") == 0;
}

/// Read a word, and scan the head of a class after it when the word is a class key. The scan gives no
/// token. Refer to `scan_class_head`.
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
/// The block form is an uppercase name with a body. With no arguments, the name and the `{` are on
/// the same line, and the block holds statements: `SCOPE_EXIT { f(); };`. With an argument list,
/// the arguments cannot be a parameter list, or a call statement can start at the name, and an
/// optional template parameter list and parameter list can come before the body. The argument
/// list can start on the line after a name of MACRO_MIN_BARE_LENGTH or more characters:
/// `BOOST_AUTO_TEST_CASE`, a line break, `(name)`, and `{ g(); }` of boost icl. The `{` can come
/// after line breaks. Where a call statement can start and no statement macro can, a function try
/// block on the line after the argument list is the body: `TEST(A, B)`, a line break, `try`, `{`,
/// of ClickHouse. `classify_next_line` reads that `try`.
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
/// The item form is an uppercase name with an argument list of any shape, before `;`, where an item
/// of a translation unit or of a namespace body starts: `DECLARE_HANDLE(foo);`. The arguments need no
/// shape, because no call stands at that position in C++. A name that a class head of the file or the
/// seed declares as a type keeps its declaration, and so does a group with the shape of a
/// parenthesized declarator.
///
/// `name` is the name, and `length` is its length. `same_line` tells if no line break comes between the
/// name and its arguments. `call` tells if the name has arguments, and `args` holds their facts. `gap`
/// is the space after the name and its arguments. `classes` holds the recorded class names, or it is
/// NULL where a member cannot start. `member_macro_name` tells if a member starts at the name, the
/// name has the shape of a macro name with two characters or more, and no class head of the file
/// recorded the name. `declared_type` tells if a class head of the file or the seed of the project
/// declares the name as a type.
static Invocation scan_macro_invocation(Reader *reader, const char *name, size_t length, bool same_line, bool call,
                                        const Arguments *args, const Gap *gap, const bool *valid_symbols,
                                        const Scanner *classes, bool member_macro_name, bool declared_type,
                                        bool item_only) {
    TSLexer *lexer = reader->lexer;
    // A name that is no macro by its shape reads as a macro only where an item starts and a `;` ends it.
    // A decline gives INVOCATION_NONE, so that the paths after this scan keep their readings: a SAL
    // annotation with arguments, `_Pre_satisfies_(n >= sizeof(T)) static DWORD CALLBACK f(...)` of dolphin,
    // takes the line token there, and a stop gave that file a MISSING `;`.
    if (item_only && !(call && !gap->directive && lexer->lookahead == ';')) {
        return INVOCATION_NONE;
    }
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
        // In an enumerator list, a call before `,`, `}`, or a name is a macro invocation. An
        // enumerator never has arguments. A name with no arguments is an enumerator. A name after
        // the call is a second macro invocation, because no enumerator follows another one with no
        // comma: `enum E { AST_NODE_LIST(DECLARE_TYPE_ENUM) FAILURE_NODE_LIST(DECLARE_TYPE_ENUM) };`.
        bool end = c == ',' || c == '}' || lexer->eof(lexer) || gap->directive || is_word_start(c) ||
                   (gap->newlines > 0 && c == '#');
        if (!call || !end) {
            return INVOCATION_STOP;
        }
        lexer->result_symbol = MACRO_ENUMERATOR_START;
        return INVOCATION_TOKEN;
    }

    if (call && c == ';') {
        // A NAME WITH A GROUP AND A `;` WHERE AN ITEM STARTS IS A MACRO. A translation unit and a
        // namespace body hold declarations and no statement ([basic.link] p1, [namespace.def] p1;
        // GCC `cp_parser_toplevel_declaration`, Clang `ParseExternalDeclaration`), so `FOO(x);`
        // there is no call. Its one reading in C++ is a declaration of the object `x` with the type
        // `FOO` and a parenthesized declarator, and that reading needs `FOO` to name a type. With
        // `FOO` declared as nothing or as a function, GCC gives "expected constructor, destructor,
        // or type conversion before '(' token" and Clang "unknown type name 'FOO'". So
        // `DECLARE_HANDLE(foo);`, `BENCHMARK(BM_Run);`,
        // `INSTANTIATE_TEST_SUITE_P(a, b, c);`, `STATIC_ASSERT(x == 1);`, `Q_ENUM_NS(Mode);`,
        // `FOO();` and `TEST_SPECIALIZATION(unsigned int);` are macro invocations, and the group
        // needs no shape. Over the corpus at f97fe26, 81,664 such items stand in 16,175 files with
        // no error, a project of the corpus defines the name of 81,055 of them as a macro, and a
        // random sixty of them are sixty macros whose expansion is a declaration or a definition.
        //
        // `MACRO_ITEM_START` IS THE EVIDENCE OF THE POSITION. The grammar makes it valid where an
        // item of a translation unit or of a namespace body starts, and nowhere else. In a block the
        // call form keeps `MAX(a, b);` an expression, and in a class body the member form decides.
        //
        // THE TWO LOOKUPS OF A DECLARED TYPE GUARD THE ONE C++ READING. `T (x);` after
        // `struct T {};` declares `x`, and the base reads a call there, which is wrong in the other
        // direction. A class head of the file or a seed type keeps the reading of the base, so the
        // rule replaces no wrong reading with another. Each lookup reads no character. The one-
        // character names of the corpus at this position are X-macro lists, `X(KindOfBoolean, bool);`
        // of hhvm, so the shape of the name is no filter here. A group with the shape of a
        // parenthesized declarator, `FOO(*p);`, keeps its declaration, which is the reading of the
        // base and the one C++ reading of that text. No such site stands in a clean file of the
        // corpus.
        if (valid_symbols[MACRO_ITEM_START] && !declared_type && !args->pointer_declarator) {
            lexer->result_symbol = MACRO_ITEM_START;
            return INVOCATION_TOKEN;
        }
        if (item_only) {
            return INVOCATION_NONE;
        }
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
        // A MEMBER WITH NO TYPE WHOSE NAME IS NOT THE NAME OF ITS CLASS IS A MACRO. C++ gives a
        // member with no decl-specifier-seq three readings, a constructor, a destructor, and a
        // conversion function ([class.mem], GCC `cp_parser_member_declaration`, Clang
        // `ParseCXXClassMemberDeclaration`), and a constructor takes the name of its class. So
        // `WTF_MAKE_NONCOPYABLE(LowerDFGToB3);` in `class LowerDFGToB3`, `DISALLOW_NEW();`,
        // `GDCLASS(Area3D, CollisionObject3D);`, `MOCK_METHOD1(g, void(int));` with a function type
        // in the group, and `BOOST_STATIC_CONSTANT(bool, value = true);` with a default argument in
        // the group are macro invocations, and the group needs no shape. Over the corpus, 30,669
        // such members have a name in uppercase, the project of 26,482 of them defines the name as
        // a function-like macro, and no project defines one as an object-like macro that names the
        // class.
        //
        // THE CLASS RECORD IS THE EVIDENCE, AND THE SHAPE OF THE NAME GUARDS TWO DEFECTS OF THE
        // RECORD. `scan_class_head` records the last name of the head, so `class CaseMap U_FINAL :
        // public UMemory {` records `U_FINAL`, and `CaseMap();` in its body is a constructor whose
        // name the record does not hold: 488 sites, 486 with a lowercase letter. The record also
        // misses a head when a `/` that is no comment comes before the first member, when more than
        // MAX_CLASSES heads come after it, or when the head holds an error: 62 constructors in 36
        // files, each with a lowercase letter. A name with no lowercase letter and two characters
        // or more leaves both defects to the record, and it leaves the macro reading to the
        // mixed-case macros: `CGAL_Algebraic_Kernel_cons(x);` and `CPP_assert(x);` keep the
        // reading of a constructor declaration until the record is correct. `S(int x);` and
        // `FOO(int x);` in the class of that name keep their reading, because the record holds the
        // name.
        if (member_macro_name && valid_symbols[MACRO_LINE_START]) {
            lexer->result_symbol = MACRO_LINE_START;
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

    // THE ARGUMENT LIST CAN START ON THE LINE AFTER THE NAME. `scan_macro_start` reads a group on
    // the next line for a name of MACRO_MIN_BARE_LENGTH or more characters, so `call` with no
    // `same_line` is that shape: `BOOST_AUTO_TEST_CASE`, a line break, `(name)`, and `{ g(); }` of
    // boost icl. The line break between the name and the group gives no reading of its own: at an
    // item start, `NAME`, a line break, `(x)`, and `{` has the readings of `NAME(x) {`, and the
    // branches below decide them with the same tests. A name of fewer characters reads no group on
    // the next line, and `FOUR`, a line break, `(x)`, and `{ g(); }` is a function definition. A
    // name that is the declarator of a function whose return type stands on the line before,
    // `restriction<Ch>`, `BOOST_IOSTREAMS_RESTRICT`, `(Ch& is)`, and `{` of boost iostreams, is not
    // at an item start, and no token of this form is valid there. The bare form keeps `same_line`:
    // `SCOPE_EXIT` and `{` on one line.
    if ((same_line || call) && valid_symbols[MACRO_BLOCK_START] && !gap->directive) {
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
        } else if (call && (args->not_parameters || args->call_arguments || args->specifier_only)) {
            // A call in an argument gives a body to the macro, and no function definition starts:
            // `MATCHER_P(IsNode, height, absl::StrCat("height ", height)) {`.
            block = lexer->lookahead == '{';
        } else if (call && (valid_symbols[MACRO_CALL_START] || member_macro_name) && lexer->lookahead == '{') {
            // A NAME WITH A GROUP AND A BODY IS A MACRO WHERE A CALL STATEMENT CAN START. The other
            // reading of that text is a constructor definition, which has no decl-specifier-seq. At
            // namespace scope C++ takes that definition only with a QUALIFIED name, and GCC gives
            // "expected constructor, destructor, or type conversion" for `A() { }` there. So the
            // arguments need no shape, and `TEST(a, b) { ... }` of Google Test gets its body.
            //
            // `MACRO_CALL_START` IS THE EVIDENCE OF THE POSITION. It is valid where a macro call
            // statement can start, which is namespace scope and no class body. In a class body an
            // unqualified constructor IS correct, the token is not valid, and `S(int x) { }` keeps
            // its reading.
            //
            // IN A CLASS BODY THE NAME IS THE EVIDENCE, AS IT IS FOR THE MEMBER BEFORE A `;`. A
            // member with no decl-specifier-seq has three readings, a constructor, a destructor and
            // a conversion function ([class.mem]), and a constructor takes the name of its class. A
            // member with a body whose name is not the name of its class is a macro:
            // `TEST_METHOD(TestReflow) { ... }` of the TAEF header of terminal,
            // `SERIALIZE_METHODS(AddrInfo, obj) { ... }` of bitcoin, and
            // `DENC(bufferlist::const_iterator& p) { ... }` of ceph. `member_macro_name` holds the
            // same guard as the member form before a `;`: a macro-shaped name of two characters or
            // more that no class head of the file recorded. The record gives up the oldest name
            // after MAX_CLASSES names and misses a head that holds an error, and a name with a
            // lowercase letter keeps its constructor until the record is right.
            //
            // THE ARGUMENTS GIVE NO EVIDENCE HERE, SO THE BRACE MUST. An enumerator list and a
            // braced initializer hold a list of expressions: no `;`, no statement keyword, and no
            // two words in sequence at the top level. Such a brace is a declaration when a `;` comes
            // after it, or when the list has a comma, because no body has a comma at its top level:
            // `BOOST_SCOPED_ENUM_START(traffic_light) { red=0, yellow, green };` of boost,
            // `DECLARE(cpu_thread::g_threads_created){0};` of rpcs3, `{ [](){ return 1; } };`, and
            // `BOOST_SCOPED_ENUM_DECLARE_BEGIN(timezone) { utc, local }` before the END macro.
            // Each other brace is a body: `{ g(); }`, `{}` before the next item, `{ FOO }`,
            // `{ { g(); } { h(); } }`, and `{ template <class T> void operator()(T) {} };` of boost
            // msm, which is a class body with no `;` in it. A body with a statement in it keeps its
            // reading before a stray `;`.
            //
            // THE SCAN READS THE GROUP, AND A STOP UNDOES THE READ. The runtime resets the lexer
            // when the scan gives no token. At a `{` on the line of the arguments, and at a `{` on
            // the next line, the paths after this one give no token: the line form gives
            // `INVOCATION_NONE` on the same line, `NEXT_BLOCKED` on the next line, and the scan of
            // the caller stops at the `{`. After a directive line, the line form gives a token, and
            // the stop takes that token away for a declaration only. A group that the scan limit
            // ends also stops, and the text keeps its reading.
            Arguments body = {0};
            if (!skip_group(reader, &body)) {
                return INVOCATION_STOP;
            }
            Gap after = {0};
            skip_gap(reader, &after);
            bool expressions = !body.statement_token && !body.word_sequence;
            if (expressions && (lexer->lookahead == ';' || body.argument_count >= 2)) {
                return INVOCATION_STOP;
            }
            block = true;
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
        if (next == NEXT_TRY_BODY) {
            // A NAME WITH A GROUP AND A FUNCTION TRY BLOCK IS A MACRO WITH A BODY WHERE A CALL
            // STATEMENT CAN START. The other reading of `TEST(A, B)`, a line break, `try`, `{`, of
            // ClickHouse is a constructor definition with a function try block, and at namespace
            // scope C++ takes that definition only with a QUALIFIED name, as for `TEST(A, B) {`
            // above. GCC gives "expected constructor, destructor, or type conversion" for the text
            // with no macro, and both front ends accept it with the macro. `try` and `{` decide the
            // body where `{` alone decided it above, and the brace needs no test of its content,
            // because no enumerator list and no braced initializer comes after `try`.
            //
            // THE POSITION IS THE EVIDENCE, AND TWO TOKENS GIVE IT. `MACRO_CALL_START` is valid
            // where a macro call statement can start, and not in a class body, where `S(int x)`, a
            // line break, and `try { g(x); } catch (...) { }` is a constructor with a function try
            // block: without this token the branch reads a macro there, with a name of one
            // character too. `MACRO_STATEMENT_START` is valid where a statement macro can start
            // and no function definition can: in a block, `CAPTURE(byte)`, a line break, and
            // `try {` of nlohmann-json is a macro line and a try statement, and no body. This
            // branch gives no token there, and that text keeps the reading of the base, a nested
            // function definition, for a separate repair of the line form. `try :` never comes
            // here, so `S(int x)`, a line break, and `try : m(x) { }` keeps its reading in each
            // position.
            //
            // The group on the line of the name and the group on the next line take the same path,
            // as for `{`. `try` on the line of the group does not come here: the line form gives
            // no token on one line, and the corpus of 2026-09-16 holds no such site.
            if (valid_symbols[MACRO_BLOCK_START] && valid_symbols[MACRO_CALL_START] &&
                !valid_symbols[MACRO_STATEMENT_START] && reader->budget > 0) {
                lexer->result_symbol = MACRO_BLOCK_START;
                return INVOCATION_TOKEN;
            }
            return INVOCATION_STOP;
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
static bool scan_macro_start(Reader reader, const char *name, const char *full, bool has_lower,
                             const bool *valid_symbols, const Scanner *scanner) {
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
    // A name that a class head of the file or the seed declares as a type keeps a declaration with a
    // parenthesized declarator where an item starts: `T (x);`. Refer to `is_declared_type_name`.
    bool declared_type = class_name || is_seed_type_name(scanner, full, &reader);
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
    // A NAME WITH A FUNCTION-MACRO ROW OF THE SEED IS A MACRO WHERE AN ITEM STARTS, WHATEVER ITS SHAPE.
    // `PyDoc_STRVAR(doc, "text");` and `TF_CALL_half(REGISTER);` hold a lowercase letter, so the shape
    // test of a macro name declines them, and the row of the seed is the evidence in their place. The row
    // must be the function-like one, because an object-like macro takes no arguments ([cpp.replace] p10):
    // `printf("good");` of the Clang interpreter tests has an object row, and the file is a REPL input
    // where a statement at file scope is the reading of Clang itself. Refer to `_macro_item`.
    bool item_name = valid_symbols[MACRO_ITEM_START] && !macro_name && !is_grammar_keyword(name) && !class_name &&
                     !word_in(name, GRAMMAR_WORDS) && is_seed_function_macro_name(scanner, full, &reader);
    bool invocation = (macro_name || member_name || item_name) &&
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
    // A macro call that gives a whole template parameter needs no macro-shaped name, because the
    // group after the name decides it. Refer to the reading of `template_parameter_macro` below.
    bool template_parameter_macro = valid_symbols[TEMPLATE_PARAMETER_MACRO_START] && !is_grammar_keyword(name) &&
                                    !word_in(name, TYPE_WORDS);
    // EVERY RECORD OF THE PARSE DECIDES A WHOLE TEMPLATE PARAMETER, AND THE LOOKUP COMES BEFORE THE SCAN
    // OF A GROUP. `skip_group` reads each word of the group with `read_word`, and each read writes
    // `reader.word_hash`. The same lookup after the group holds the hash of the last word of the group,
    // and it gives a miss for the name of the macro: `template<typename FT, FT(T::*mem)>` of
    // g++.dg/cpp0x/pr52744.C became a macro invocation although the head declares `FT` as a type
    // parameter. The rule below reads this name after the group, so the result comes from here.
    bool recorded_type = template_parameter_macro && is_recorded_type_name(scanner, full, &reader);
    // A call of a name with a lowercase letter where a declaration or a statement starts, with its `(` immediately
    // after the name. Refer to the branch below.
    bool branch_end_call = lexer->lookahead == '(' && !is_macro_name(name, has_lower) && !member_start &&
                           valid_symbols[MACRO_LINE_START] && is_call_line_word(name, length) &&
                           in_structured_group(scanner);
    if (!invocation && !constructor && !attribute_call && !statement_attribute && !template_parameter_macro &&
        !branch_end_call) {
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
    // A CALL OF A NAME WITH A LOWERCASE LETTER IS A MACRO INVOCATION LINE WHEN THE DIRECTIVE THAT ENDS A BRANCH OF A
    // STRUCTURED GROUP COMES AFTER IT. `nssv_RESTORE_WARNINGS()` of simdjson, a line break, and `#endif`: a call
    // with no `;` there is no declaration and no statement of C++, and the name is a macro. `group_is_structured`
    // makes such a group a node only where the two scans agree: the call is the last line of the branch, and a
    // token that starts an item or a `}` comes after the group (`branch_ends_fit`).
    //
    // A class body reads such a call as a member macro (`member_name`), and this branch does not run there.
    //
    // THE BRANCH READS THE GROUP, AND A DECLINE GIVES THE RESULT OF THE OLD PATH. The other paths for such a name
    // are the macros before a constructor, and `scan_constructor_after_macro` gives false at the `(`. So a decline
    // after the read gives no token, as that path did.
    Arguments args = {0};
    bool call = false;
    if (branch_end_call && same_line) {
        if (!skip_group(&reader, &args)) {
            return false;
        }
        call = true;
        gap = (Gap){0};
        skip_gap(&reader, &gap);
        if (gap.directive && gap.newlines > 0) {
            lexer->result_symbol = MACRO_LINE_START;
            return true;
        }
        // THE READ OF THE GROUP STAYS, AND THE SCAN GOES ON. A `return false` here gave no token at all,
        // and the item form after it never ran: `TF_CALL_half(DECLARE_GPU_SPECS);` in a `#if` group of
        // tensorflow kept its call, while the same line outside the group became a macro invocation.
        if (!item_name) {
            return false;
        }
    } else if ((macro_name || sal || member_name || template_parameter_macro || item_name) && !gap.directive &&
               lexer->lookahead == '(' && (same_line || length >= MACRO_MIN_BARE_LENGTH)) {
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
    // A macro call that gives a whole template parameter: `template <typename T,
    // FMT_ENABLE_IF(!std::is_same<Char, char>::value)>` in fmt. The grammar reads `NAME(...)` there
    // as a parameter of type NAME with an abstract function declarator, so a group that is no
    // parameter list has no reading, and the file holds an ERROR node.
    //
    // THE GROUP GIVES THE EVIDENCE AND THE NAME PROVES NOTHING, so the name needs no macro shape.
    // A `!`, a `==`, a `?`, a `;`, and a statement keyword cannot come at the top level of a
    // parameter list, and a text with such a group is invalid without a macro. GCC
    // `cp_parser_template_parameter_list` (gcc/cp/parser.cc) and Clang `ParseTemplateParameterList`
    // (clang/lib/Parse/ParseTemplate.cpp) read a parameter declaration there, and both reject each
    // of those groups.
    //
    // A GROUP THAT CAN BE A PARAMETER LIST OR A DECLARATOR GETS NO TOKEN, and the reading of that
    // text does not move. `U(V)` is a legal non-type parameter of function type, `U()` is the same
    // form with no parameter, and `U(*p)`, `U(&r)`, `U(*)` and `U(*p[3])` are the parenthesized
    // declarator of a pointer or of a reference. `U((V))`, `U((v))` and `U((V, W))` are the same
    // declarator with the parentheses that a declarator can always take. THE TWO FRONT ENDS TAKE ALL
    // OF THEM: with every name declared, Clang 22.1.8 reports no error for any of the nine, and GCC
    // 16.2.1 reports only that `U(V)` declares a parameter with no type, which is its reading of the
    // same declarator. `pointer_first` holds the fact for the pointers and the references,
    // `group_first` for the group in a group, and `one_type_id` keeps the group of a macro type
    // specifier, which this scan answers first.
    //
    // A GROUP IN A GROUP DECLINES WHATEVER IT HOLDS, and `hb_requires ((hb_is_source_of<...>::value))`
    // of harfbuzz keeps the tree that it has today, which is wrong. This record reads the top level
    // of the group only, so the scan cannot tell that group from `U((V))`, and a rule that cannot
    // tell two texts apart must take the reading that holds for both. Refer to the macro table.
    //
    // The scan of the group is the one above, because the lexer cannot read the same group twice.
    // A MACRO-SHAPED NAME THAT IS A WHOLE TEMPLATE PARAMETER IS A MACRO, AND A DECLARED NAME DECLINES.
    // `template <UNDIRECTED_GRAPH_PARAMS> class undirected_graph` of boost graph and
    // `template <class S, FMT_ENABLE_IF(...)>` of fmt hold a macro where one parameter stands, and the tree
    // states a parameter whose type is the macro. The rule fires on the position and the shape, so a parse
    // with no seed reads them too, and the two lookups of a declared type decline a real parameter.
    //
    // THE CHARACTER AFTER THE NAME, OR AFTER ITS GROUP, ENDS THE PARAMETER. `FILE *BufType::FileMemberPtr`
    // of fmt ostream.h is a real type with a declarator, and a form with no such test gave 467 files a new
    // error.
    //
    // A NON-TYPE PARAMETER OF CLASS TYPE HAS THE SAME SHAPE, AND A RECORD OF THE TYPE SEPARATES IT.
    // `struct BR { const int &r; };` and then `template<BR> void f() {}` of clang mangle-class-nttp.cpp is
    // valid C++. 19 such sites of the corpus take no token, 15 in that file and 4 in a documented head of
    // cgal, and without this test each of those trees becomes a macro invocation with no ERROR node.
    // `is_recorded_type_name` reads every record of the parse, and each source decides a site of its own:
    // - The class heads with a member decide `BR` and 9 more names of the clang file.
    // - The loose record, which holds a head with an empty body, decides `union H1 {};` at line 270 of the
    //   same file. `H2`, `H3` and `H4` there hold a member.
    // - The record of type parameters decides `template<typename CT, CT> struct member_helper;` of
    //   g++.dg/cpp0x/pr52744.C and `template <typename T1, typename T2, template <T2> class Comp>` of
    //   g++.dg/template/pr30044.C, where the name is a type parameter of the same head.
    // - The `using` aliases of the file decide a name that an alias declares.
    // - The type rows of the seed decide the 4 sites of cgal, where `ET` is a type of the project.
    // The five sources cost one repair of the corpus, and that repair is `H1`.
    //
    // AN ENUM AND A TYPEDEF KEEP THE WRONG READING IN A PARSE WITH NO SEED, because no record holds
    // either name. `enum E2 : int { x2 };` and then `template<E2> struct A2 {};` of g++.dg/cpp0x/enum30.C
    // is such a site. A parse with the seed of the project declines it through the type row, and a record
    // of the enum heads and of the typedef names is a task of its own.
    if (template_parameter_macro && !gap.directive && is_macro_name(name, has_lower) && length >= 2 &&
        !recorded_type && (lexer->lookahead == ',' || lexer->lookahead == '>')) {
        lexer->result_symbol = TEMPLATE_PARAMETER_MACRO_START;
        return true;
    }
    if (call && template_parameter_macro && !gap.directive && args.not_parameters && !args.empty &&
        !args.pointer_first && !args.group_first && !args.one_type_id) {
        lexer->result_symbol = TEMPLATE_PARAMETER_MACRO_START;
        return true;
    }
    // A name that is a macro only by its place must have the argument list that makes it one. A group
    // with the shape of a parenthesized declarator declares a member: `Foo (*p);`, `Foo (&r);`.
    if (invocation && (macro_name || (call && !args.pointer_declarator))) {
        Invocation result =
            scan_macro_invocation(&reader, name, length, same_line, call, &args, &gap, valid_symbols, classes,
                                  member_start && macro_name && length >= 2 && !class_name, declared_type, item_name);
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

/// Read the name of a macro, as `read_macro_name` does, and copy it into `text` of `size` bytes with a
/// NUL at its end. `*cut` is true when the name has more characters than `text` holds. With a NULL
/// `text`, the scan copies nothing. O(n) in the length of the name.
static bool read_macro_name_into(TSLexer *lexer, char *text, unsigned size, bool *cut) {
    unsigned length = 0;
    bool has_uppercase = false;
    if (cut != NULL) {
        *cut = false;
    }
    if (text != NULL && size > 0) {
        text[0] = '\0';
    }
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
        if (text != NULL && size > 0) {
            if (length + 1 < size) {
                text[length] = (char)c;
                text[length + 1] = '\0';
            } else if (cut != NULL) {
                *cut = true;
            }
        }
        advance(lexer);
    }
    return length >= 2 && has_uppercase;
}

/// Read the name of a macro: two or more characters, an uppercase letter, and no lowercase letter,
/// as clang-format reads the name of a macro. No C++ keyword has this shape.
static bool read_macro_name(TSLexer *lexer) {
    return read_macro_name_into(lexer, NULL, 0, NULL);
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
///
/// The list holds `__restrict` and `__restrict__` and no bare `restrict`, because C++ has no such
/// keyword. A bare `restrict` in that position is a name, and the macro path reads it as one.
static const char *const POINTER_QUALIFIER_WORDS[] = {
    "const",    "volatile",  "__restrict", "__restrict__",      "_Nonnull",
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
        } else if (end != ';' && end != ',' && end != '(') {
            // A `(` after the name opens the parameter list of a function declarator:
            // `int PRINTF_FORMAT(1, 2) f(const char *s);`. The grammar takes the declarator, because
            // the attribute macro sits in `_declaration_specifiers` and each declarator follows it.
            //
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
///
/// WITH `object_valid`, AN OBJECT-LIKE MACRO OF THE SEED BEFORE A GROUP OF EXPRESSIONS AND A `;` OR A
/// `,` GIVES DECLARATOR_OBJECT_MACRO_NAME. An object-like macro takes no arguments ([cpp.replace] p10), so
/// the group is the initializer or the parameter list of the declarator, and the grammar reads it as it
/// reads the group with no macro: `Mutex l1 MOZ_UNANNOTATED("autolock");` is `Mutex l1("autolock");`.
/// The seed decides the kind, because the text cannot: `ABSL_GUARDED_BY(mu)` of abseil is
/// function-like, and envoy uses it with no row of its own. A macro with a function row, with the two
/// rows, or with no row keeps the arguments. The collector gives an alias the rows of its target, so
/// `P_` of bde, which expands to the function-like `BSLIM_TESTUTIL_P_`, has a function row. A name with
/// more bytes than a word of the seed takes no row. A group that is empty or holds no expressions keeps its
/// reading of a parameter list: `Rep max BOOST_PREVENT_MACRO_SUBSTITUTION ()`, whose macro is also
/// object-like.
static bool scan_trailing_macro_name(TSLexer *lexer, const Scanner *scanner, bool line_break, bool declarator,
                                     bool name_macro, bool object_valid) {
    char name[TS_CPP_SEED_WORD_SIZE];
    bool cut = false;
    if (!read_macro_name_into(lexer, name, TS_CPP_SEED_WORD_SIZE, &cut)) {
        return false;
    }
    uint16_t kinds = declarator && object_valid && has_seed(scanner) && !cut
                         ? seed_kinds((const TSCppSeed *)scanner->context, name, (uint32_t)strlen(name))
                         : 0;
    bool object_macro = (kinds & TS_CPP_SEED_OBJECT_MACRO) != 0 && (kinds & TS_CPP_SEED_FUNCTION_MACRO) == 0;
    lexer->result_symbol = declarator ? DECLARATOR_MACRO_NAME : TRAILING_MACRO_NAME;
    mark_end(lexer);
    if (!line_break && !declarator) {
        return true;
    }
    Reader reader = start_reader(lexer, MACRO_SCAN_LIMIT, scanner);
    // A macro name can have arguments. A virt-specifier and the arguments of an attribute cannot.
    bool after_macro = true;
    // The object reading takes only the group directly after the name. The group after a later macro
    // belongs to that macro: `Mutex l1 MOZ_UNANNOTATED GUARDED_BY(mu);`.
    bool object_group = object_macro;
    for (;;) {
        LOOP_STEP();
        Gap gap = {0};
        skip_gap(&reader, &gap);
        if (gap.blocked) {
            return false;
        }
        bool group_after_name = object_group;
        object_group = false;
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
                // A group of expressions after an object-like macro of the seed is no argument list of
                // the macro. The object readings are declarators of a declaration, and only a `;` or a
                // `,` comes after them. A body keeps the macro between the name and the parameter list:
                // `void swap BOOST_MATH_PREVENT_MACRO_SUBSTITUTION (T& a, T& b) {` in boost/math/tools/utility.hpp.
                if (group_after_name) {
                    Gap after_group = {0};
                    skip_gap(&reader, &after_group);
                    int32_t end = lexer->lookahead;
                    if (!after_group.blocked && (end == ';' || end == ',')) {
                        lexer->result_symbol = DECLARATOR_OBJECT_MACRO_NAME;
                        return true;
                    }
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

/// Read the name of a `#define` and take it out of the record of template type parameters.
///
/// A NAME THAT THE FILE DEFINES AS A MACRO IS NOT A TEMPLATE PARAMETER. The record is not scoped, so
/// a name stays in it after its template ends, and the exposure of that leak is a property of the
/// POSITION that reads the record and not of the record. At the operand of `alignas` it costs
/// nothing, 4 sites and 0 errors over the corpus. Where the record decides whether a name is a type
/// or a macro it costs a wrong tree: v8 src/compiler/heap-refs.h declares `template <class K, class
/// V>` at line 506 and writes `#define V(Name)` at 1411, and the record still held `V` at 1421.
///
/// THE DEFINE PRECEDES THE USE, so a record filled as the scan goes forward can act on it. This is a
/// fact of the file and not a heuristic, and it repairs the record rather than the rule that reads
/// it. The lookahead is the character after the directive name and the scan reads no character of
/// the token of the directive, because `scan_directive` ended that token before it.
static void forget_defined_name(Scanner *scanner, TSLexer *lexer) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
        advance(lexer);
    }
    if (!is_word_start(lexer->lookahead)) {
        return;
    }
    uint32_t hash = HASH_START;
    while (is_word_char(lexer->lookahead)) {
        hash = hash_character(hash, lexer->lookahead);
        advance(lexer);
    }
    forget_template_name(scanner, finish_hash(hash));
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
    /// True when the name of `macro` has a lowercase letter (`is_call_line_name`). Such a call is a macro invocation
    /// line only as the last line of a branch.
    bool macro_lower;
    /// True when the name of `macro` is `_Pragma` or `__pragma`, whose call is an item of the grammar.
    bool macro_pragma;
    /// The number of completed branches that end with END_VALUE.
    uint32_t value_ends;
    /// The number of those branches whose last line is a call of a name with a lowercase letter.
    uint32_t call_line_value_ends;
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

/// True when NAME can be the name of a macro invocation line that has a lowercase letter (`is_call_line_word`).
static bool is_call_line_name(const Name *name) {
    return !name->macro_shaped && is_call_line_word(name->text, name->length);
}

/// True when NAME is `_Pragma` or `__pragma`. Their call is a `pragma_operator` item of the grammar.
static bool is_pragma_operator_name(const Name *name) {
    return name_is(name, "_Pragma") || name_is(name, "__pragma");
}

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
///
/// A call of a name with a lowercase letter is no macro invocation line before a token. `scan_macro_start` reads
/// such a call as a macro only before the directive that ends a branch, and `end_branch` records that form.
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
    bool line_end = (scan->macro == MACRO_CALL && !scan->macro_lower) ||
                    (scan->macro == MACRO_LONG_NAME && !scan->enumerators);
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
                if (scan->macro_pragma) {
                    // `_Pragma ( ... )` and `__pragma ( ... )` are `pragma_operator` items with no `;` in each
                    // scope of the grammar ([cpp.pragma.op]).
                    scan->macro = MACRO_NONE;
                    scan->end = END_ITEM;
                    scan->at_item_start = outside_parentheses(scan);
                    scan->before_body = false;
                    return;
                }
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
        scan->macro_lower = false;
        scan->macro_pragma = false;
    } else if (item_start && !scan->enumerators && (is_pragma_operator_name(&name) || is_call_line_name(&name))) {
        // The arguments must start on the line of the name, with no blank before them.
        scan->macro = MACRO_SHORT_NAME;
        scan->macro_not_names = false;
        scan->macro_pragma = is_pragma_operator_name(&name);
        scan->macro_lower = !scan->macro_pragma;
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
///
/// A call of a name with a lowercase letter keeps the end END_VALUE, because it is also the head of a function
/// before its body. The scan counts such a last line apart, and `branch_ends_fit` reads the count.
static void end_branch(GroupScan *scan) {
    bool lower_call = scan->macro == MACRO_CALL && scan->macro_lower;
    if ((scan->macro == MACRO_CALL && !lower_call) || (scan->macro == MACRO_LONG_NAME && !scan->enumerators)) {
        scan->end = END_MACRO;
        scan->call_end |= scan->macro == MACRO_CALL && !scan->macro_not_names;
    }
    if (scan->end == END_VALUE) {
        scan->value_ends++;
        if (lower_call) {
            scan->call_line_value_ends++;
        }
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
///
/// A call of a name with a lowercase letter as the last line of a branch, as `nssv_RESTORE_WARNINGS()` of
/// simdjson, is a macro invocation line only before a token that starts an item or before a `}`. The text after
/// the preprocessor is the call and that token, and it is valid C++ only when the name is a macro. GCC gives
/// "expected constructor, destructor, or type conversion" for such a call at namespace scope. `scan_macro_start`
/// reads the same line as a macro before the directive of a structured group. Before `{`, `;`, `:`, `,`, or `=`,
/// the call is a function head, a declarator, or an expression, and the group stays a group of lines.
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
    bool calls_are_items = scan->value_ends > 0 && scan->value_ends == scan->call_line_value_ends &&
                           (after == AFTER_ITEM_START || after == AFTER_CLOSE_BRACE);
    if (calls_are_items) {
        ends = (ends & ~(1U << END_VALUE)) | (1U << END_MACRO);
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
            // `scan_macro_start` reads a call of a name with a lowercase letter only with its `(` immediately after
            // the name. With a blank, the scan of a comparison can take the name first.
            if (scan.macro == MACRO_SHORT_NAME && scan.macro_lower) {
                scan.macro = MACRO_NONE;
            }
            continue;
        }
        if (c == '\\') {
            if (scan.macro == MACRO_SHORT_NAME && scan.macro_lower) {
                scan.macro = MACRO_NONE;
            }
            skip_backslash(lexer, false);
            continue;
        }
        if (c == '/') {
            if (scan.macro == MACRO_SHORT_NAME && scan.macro_lower) {
                scan.macro = MACRO_NONE;
            }
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
        if (type == PREPROC_DEFINE || type == PREPROC_FINAL_DEFINE) {
            forget_defined_name(scanner, lexer);
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
///
/// WITH `after_blanks`, THE SCAN TELLS ITS CALLER WHEN IT STOPPED RIGHT AFTER THE BLANKS. The scan of
/// a declared type in `scan_word_start` runs at the positions of this scan, and it must read the
/// blanks after the name before it can know whether a name follows. The lexer cannot go back, so the
/// two scans read those blanks one time: this scan reads them, and when the character after them is
/// not a `<` and not the `(` of a cast, it gives no token, sets `*after_blanks`, and reads no more.
/// The reader is then after the blanks, the mark is at the end of the name, and the caller reads the
/// character there. With NULL, the scan is as before.
static bool scan_comparison_name(Reader *reader, const char *name, bool comparison_valid, bool cast_name,
                                 bool *after_blanks) {
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
        if (after_blanks != NULL) {
            *after_blanks = true;
        }
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

// THE RECORD OF LOCALS.
//
// The scanner records the parameters and the block-scope variables of the bodies of the file, so that
// a rule can ask whether a name is one of them where it reads the name. A front end reads
// `sizeof(buf)` as a value operand when `buf` is a local variable, and the shape of a name does not
// tell a local from a type.
//
// THE RECORD CHANGES AT A BOUNDARY: THE `{`, THE `}`, OR THE `;` BEFORE AN ITEM. At these three
// characters each other path of `scan_token` gives no token and reads no character. So a scan here
// can read the next item and then give no token, and no other token of the scanner changes. A scan
// at a word cannot do that, because the other scans of a word start at the same character.
//
// NO TOKEN CARRIES A CHANGE. The scan gives TREE_SITTER_EXTERNAL_STATE_ONLY and no token, and the runtime
// of ABI 1018 stores the state on the boundary token that its internal lexer reads next. An empty extra
// for each change was a lookahead of its own, and it changed trees with no rule that read the record:
// in error recovery, in the order of the versions, and in ties. The scan reads the text and the record
// only, never `valid_symbols`. So each GLR version at a boundary gets the same record, and two versions
// that merge without the record also merge with it.
//
// THE RECORD HAS NO BYTE POSITION, because the lexer gives none. A name that the scan of an item finds
// is PENDING, and the next boundary makes it live. A name is therefore never in scope before its
// declaration, and the type of `Module *Module = nullptr;` reads while the local `Module` is pending.
// A use in the statement that declares the name is a miss: `T *S0 = S, *S1 = (T *)((char *)(S0) + 1);`.
//
// THE LOOPS OVER THE TOKENS OF AN ITEM HAVE NO LOOP GUARD. An item holds at most LOCAL_ITEM_TOKENS
// tokens in memory, and the index of each such loop grows at each step, so the loop ends with no read.
// The guard counts the steps after the last read, and an item of 384 tokens takes more of them than
// MAX_IDLE_STEPS. The loops that read the input keep the guard.
//
// EACH RANGE OF THE RECORD FAILS CLOSED. A function that gets an index or an end outside the tokens of
// its item gives its answer for no match: LOCAL_NO_TOKEN, false, or no entry. A loop over the entries
// reads MAX_LOCALS of them at most, and a state whose record does not fit decodes as an empty record.
// The scanner runs in the process of the consumer. An assertion stops that whole process at one
// malformed file, and a build with NDEBUG reads past the item. In `[=]({) {` and in
// `void f(Callback cb = [](int a {{)) {`, `local_match` counts each `{` as a bracket, so the `(` of the
// lambda has no `)` in the item.
//
// FRAMES EXIST ONLY WHILE THE RECORD IS NOT EMPTY. The head of a definition or of a lambda opens the
// first frame at its body. In a frame, each `{` opens a frame and each `}` closes one with its entries.
// At file scope, in a namespace, and in a class body outside a body, the record stays empty, and a
// boundary changes it only before a head. A brace that a scanner token reads, as in the arguments of
// a macro, is no boundary, and it is no brace of the grammar either.

/// The maximum number of characters that the scan of one item reads.
#define LOCAL_ITEM_LIMIT 8192

/// The maximum number of tokens of one item. An item with more tokens gives no entry. The head of a
/// constructor with a long initializer list takes 150 tokens and more.
#define LOCAL_ITEM_TOKENS 384

/// The index of no token.
#define LOCAL_NO_TOKEN ((unsigned)-1)

/// The maximum number of steps of a loop over the tokens of an item. Each step moves past one token or
/// more, so an item takes LOCAL_ITEM_TOKENS steps at most. A loop that gets to this maximum gives its
/// answer for no match.
#define LOCAL_MAX_STEPS (LOCAL_ITEM_TOKENS + 1)

/// One token of an item.
typedef struct {
    TokenClass kind;
    /// The hash of a word, as `read_word` computes it, or 0.
    uint32_t name;
    /// The boundaries of the item before the token: the braces in brackets, `f(T{1}, x)`.
    unsigned boundaries;
    /// The text of an operator or a bracket, or the start of a word.
    char text[MACRO_WORD_SIZE];
} LocalToken;

/// The tokens from a boundary to the next boundary that ends the item.
typedef struct {
    LocalToken tokens[LOCAL_ITEM_TOKENS];
    unsigned count;
    /// The boundaries inside the item, before its end.
    unsigned boundaries;
    /// The boundary that ends the item, `{`, `}`, or `;`, or 0 when the scan stopped before one.
    int32_t end;
} LocalItem;

static unsigned local_lambda_head(const LocalItem *item, unsigned end, unsigned *open, unsigned *close);

/// The words that no declaration of a local holds before its declarator. A statement that holds one
/// of them there declares no local: `return a * b;`, `x = a and b;`.
///
/// `explicit` IS HERE because `explicit Foo(int x);` in a local class would give the declarator `Foo`,
/// and `Foo` is a type. The statement keywords are here for the tokens after the first one, and
/// `read_local_statement` reads them before this list at the start of a statement.
static const char *const LOCAL_STOP_WORDS[] = {
    "return", "goto",     "break",  "continue", "throw",    "delete",     "new",     "co_return", "co_yield",
    "co_await", "typedef", "using", "namespace", "friend",  "static_assert", "_Static_assert", "sizeof",
    "alignof", "_Alignof", "__alignof__", "typeid", "this", "true",       "false",   "nullptr",   "operator",
    "do",     "try",      "explicit", "template", "concept", "requires",  "export",  "import",    "module",
    "and",    "or",       "not",    "xor",      "bitand",   "bitor",      "compl",   "not_eq",    "and_eq",
    "or_eq",  "xor_eq",   "if",     "else",     "while",    "switch",     "for",     "catch",     "case",
    "default", "__asm",   "__asm__", NULL,
};

/// The words that take a group and that a declaration holds before its declarator: `decltype(x) y;`,
/// `__attribute__((unused)) int z;`.
static const char *const LOCAL_GROUP_WORDS[] = {
    "decltype", "__attribute__", "__attribute", "alignas", "_Alignas", "__declspec", "typeof", "__typeof__",
    "__typeof", NULL,
};

/// The words that qualify a type and that are not a type: `const Foo BAR;` has one type.
static const char *const LOCAL_QUALIFIER_WORDS[] = {
    "const", "volatile", "static", "extern", "inline", "constexpr", "constinit", "mutable", "register",
    "thread_local", "typename", "__restrict", "restrict", NULL,
};

/// The words after the parameter list of a definition or of a lambda and before its body.
static const char *const LOCAL_TRAILER_WORDS[] = {
    "const", "volatile", "noexcept", "throw", "override", "final", "mutable", "constexpr", "static", "try",
    "restrict", "__restrict", "__attribute__", "__attribute", "__declspec", "transaction_safe", NULL,
};

/// The words before a `(` that make it no parameter list of a definition: `if (x) {`, `sizeof(x)`.
static const char *const LOCAL_NOT_HEAD_WORDS[] = {
    "if",       "while",   "for",     "switch",   "catch",   "return",       "sizeof",      "alignof",
    "decltype", "typeid",  "noexcept", "throw",   "new",     "delete",       "static_cast", "const_cast",
    "dynamic_cast", "reinterpret_cast", "__attribute__", "__attribute", "alignas", "requires", "co_await",
    "co_yield", "co_return", "typeof", "__typeof__", "__typeof", "_Generic", "__builtin_offsetof", NULL,
};

/// True when a token is a word, and its text is `text`. O(n) in the text.
static bool local_word_is(const LocalToken *token, const char *text) {
    return token->kind == TOKEN_WORD && strcmp(token->text, text) == 0;
}

/// True when a token is not a word, and its text is `text`. O(n) in the text.
static bool local_mark_is(const LocalToken *token, const char *text) {
    return token->kind != TOKEN_WORD && strcmp(token->text, text) == 0;
}

/// True for a word that has the shape of a macro name: an uppercase letter and no lowercase letter.
/// A SHAPE IS A FILTER AND NEVER EVIDENCE. `read_local_declaration` reads it only where a plain word
/// comes before it that can be the declarator. The text is the text of a token, MACRO_WORD_SIZE bytes at
/// most. O(n) in the text.
static bool local_macro_shaped(const char *text) {
    bool has_lower = false;
    for (unsigned i = 0; i < MACRO_WORD_SIZE && text[i] != '\0'; i++) {
        has_lower |= text[i] >= 'a' && text[i] <= 'z';
    }
    return text[0] != '\0' && text[1] != '\0' && is_macro_name(text, has_lower);
}

/// Read the tokens from the character after a boundary to the boundary that ends the item: a `;`, a `{`
/// or a `}` outside brackets, or the `{` of the body of a lambda. A brace inside brackets is a token of
/// the item, and the item counts it as a boundary: `A() : m(T{1}) {`, `void f(Options o = {}) {`. Each
/// such boundary is a scan of its own too, and the counts make an entry wait for the right one. O(n) in
/// the characters that the scan reads.
static void read_local_item(Reader *reader, LocalItem *item) {
    item->count = 0;
    item->boundaries = 0;
    item->end = 0;
    unsigned brackets = 0;
    while (item->count < LOCAL_ITEM_TOKENS) {
        LOOP_STEP();
        TextToken token;
        if (!read_text_token(reader, &token) || reader->budget == 0) {
            return;
        }
        bool open_brace = token.kind == TOKEN_OPEN && token.text[0] == '{';
        bool close_brace = token.kind == TOKEN_CLOSE && token.text[0] == '}';
        unsigned open = LOCAL_NO_TOKEN;
        unsigned close = LOCAL_NO_TOKEN;
        bool lambda_body = open_brace && brackets > 0 && local_lambda_head(item, item->count, &open, &close) != LOCAL_NO_TOKEN;
        if (token.kind == TOKEN_SEMICOLON || ((open_brace || close_brace) && brackets == 0) || lambda_body) {
            item->end = token.text[0];
            return;
        }
        if (open_brace || close_brace) {
            item->boundaries++;
        } else if (token.kind == TOKEN_OPEN) {
            brackets++;
        } else if (token.kind == TOKEN_CLOSE && brackets > 0) {
            brackets--;
        } else if (token.kind == TOKEN_CLOSE) {
            // The item started inside brackets, at a brace of an argument: `f(Options o = {}, int k)`. It
            // is the rest of a construct and no item, so it gives no entry.
            return;
        }
        LocalToken *local = &item->tokens[item->count++];
        local->kind = token.kind;
        local->name = token.kind == TOKEN_WORD ? reader->word_hash : 0;
        local->boundaries = item->boundaries - (open_brace || close_brace);
        memcpy(local->text, token.text, MACRO_WORD_SIZE);
    }
}

/// True when `end` is no end of a range of the tokens of the item. Refer to "EACH RANGE OF THE RECORD
/// FAILS CLOSED" above. O(1).
static bool local_range_out(const LocalItem *item, unsigned end) {
    return item->count > LOCAL_ITEM_TOKENS || end > item->count;
}

/// The count of the entries of a record that a loop reads, with MAX_LOCALS as its maximum. O(1).
static unsigned local_entry_count(const LocalRecord *record) {
    return record->count < MAX_LOCALS ? record->count : MAX_LOCALS;
}

/// The index of the bracket that closes the bracket at `open`, or LOCAL_NO_TOKEN. O(n) in the tokens.
static unsigned local_match(const LocalItem *item, unsigned open, unsigned end) {
    if (local_range_out(item, end)) {
        return LOCAL_NO_TOKEN;
    }
    unsigned depth = 0;
    for (unsigned i = open; i < end; i++) {
        TokenClass kind = item->tokens[i].kind;
        if (kind == TOKEN_OPEN) {
            depth++;
        } else if (kind == TOKEN_CLOSE && --depth == 0) {
            return i;
        }
    }
    return LOCAL_NO_TOKEN;
}

/// The index after the `>` that closes the `<` at `open`, or LOCAL_NO_TOKEN. Groups in brackets are
/// read whole, so a `>` in `(a > b)` closes nothing. O(n) in the tokens.
static unsigned local_skip_angles(const LocalItem *item, unsigned open, unsigned end) {
    if (local_range_out(item, end)) {
        return LOCAL_NO_TOKEN;
    }
    unsigned depth = 0;
    for (unsigned i = open; i < end; i++) {
        const LocalToken *token = &item->tokens[i];
        if (token->kind == TOKEN_OPEN) {
            i = local_match(item, i, end);
            if (i == LOCAL_NO_TOKEN) {
                return LOCAL_NO_TOKEN;
            }
        } else if (token->kind == TOKEN_CLOSE) {
            return LOCAL_NO_TOKEN;
        } else if (local_mark_is(token, "<")) {
            depth++;
        } else if (local_mark_is(token, ">") && --depth == 0) {
            return i + 1;
        }
    }
    return LOCAL_NO_TOKEN;
}

/// The index after a name at `start`, with its `::` parts and its template argument lists, or
/// LOCAL_NO_TOKEN when a `<` does not close. O(n) in the tokens.
static unsigned local_skip_name(const LocalItem *item, unsigned start, unsigned end) {
    if (local_range_out(item, end) || start >= end) {
        return LOCAL_NO_TOKEN;
    }
    unsigned i = start + 1;
    for (unsigned steps = 0; steps < LOCAL_MAX_STEPS; steps++) {
        if (i < end && local_mark_is(&item->tokens[i], "<")) {
            i = local_skip_angles(item, i, end);
            if (i == LOCAL_NO_TOKEN) {
                return LOCAL_NO_TOKEN;
            }
        }
        if (i + 1 < end && local_mark_is(&item->tokens[i], "::") && item->tokens[i + 1].kind == TOKEN_WORD) {
            i += 2;
            continue;
        }
        return i;
    }
    return LOCAL_NO_TOKEN;
}

/// The kind of a declaration that `read_local_declaration` reads.
typedef enum {
    /// A statement: `T x = 1, *y;`, `T x(args);`, `T x[3];`.
    LOCAL_STATEMENT,
    /// One parameter, whose range ends at its `,` or `)`: `T x`, `T x = 1`, `T x[]`.
    LOCAL_PARAMETER,
    /// The declaration of a condition of `if`, `while` or `switch`, or of a range-for: `if (T x = f())`,
    /// `for (T x : v)`. Only a `=` or a `:` ends its declarator. A condition that declares a name has an
    /// initializer, so the end of the range ends no declarator: in `if (flags & MAP_PURGEABLE) {`,
    /// `MAP_PURGEABLE` is an operand and no local. `if (a && b(c))` is an expression too.
    LOCAL_CONDITION,
    /// The declaration of a `for` head that its first `;` ends: `for (T x = 0;`, `for (T x;`. A `=` or the
    /// end of the range ends its declarator, because a declaration of a `for` head needs no initializer.
    LOCAL_FOR_INIT,
} LocalDeclarationKind;

/// True for a token that can end a declarator of a declaration of `kind`. A `(` is the direct
/// initializer of `T x(args)`, and not the parenthesized declarator of `int WINAPI (*f)(int)`, whose name
/// is inside the group.
///
/// A GROUP THAT STARTS LIKE A DECLARATOR IS A DECLARATOR ONLY BEFORE A PARAMETER LIST OR AN ARRAY BOUND. A
/// group that starts with `&`, `&&`, `*`, `^`, or a word and `*`, and that a `(` or a `[` follows, is a
/// declarator: `int WINAPI (*f)(int)`, `T x(*p)[3]`. With any other token after it, the group is an
/// initializer: `T x(&y);`, `T x(*p), z;`. O(n) in the tokens of the group.
static bool local_declarator_end(const LocalItem *item, unsigned i, unsigned end, LocalDeclarationKind kind) {
    if (local_range_out(item, end)) {
        return false;
    }
    if (i >= end) {
        return kind != LOCAL_CONDITION;
    }
    const LocalToken *token = &item->tokens[i];
    if (local_mark_is(token, "=")) {
        return true;
    }
    if (kind == LOCAL_CONDITION || kind == LOCAL_FOR_INIT) {
        return local_mark_is(token, ":");
    }
    // A `[[` starts an attribute, and a `[` alone starts an array declarator.
    if (local_mark_is(token, "[")) {
        return !(i + 1 < end && local_mark_is(&item->tokens[i + 1], "["));
    }
    if (kind == LOCAL_PARAMETER) {
        return false;
    }
    if (local_mark_is(token, "(")) {
        const LocalToken *inner = i + 1 < end ? &item->tokens[i + 1] : NULL;
        const LocalToken *second = i + 2 < end ? &item->tokens[i + 2] : NULL;
        bool pointer = inner != NULL && (local_mark_is(inner, "*") || local_mark_is(inner, "&") ||
                                         local_mark_is(inner, "&&") || local_mark_is(inner, "^"));
        bool convention = inner != NULL && inner->kind == TOKEN_WORD && second != NULL && local_mark_is(second, "*");
        if (!pointer && !convention) {
            return true;
        }
        unsigned close = local_match(item, i, end);
        if (close == LOCAL_NO_TOKEN) {
            return false;
        }
        const LocalToken *next = close + 1 < end ? &item->tokens[close + 1] : NULL;
        return next == NULL || !(local_mark_is(next, "(") || local_mark_is(next, "["));
    }
    return token->kind == TOKEN_COMMA || local_mark_is(token, ":") || local_mark_is(token, ")");
}

/// Mark the record saturated from `frame`. O(1).
static void saturate_locals(LocalRecord *record, uint8_t frame) {
    if (record->saturated == 0 || frame < record->saturated) {
        record->saturated = frame;
    }
}

/// Append an entry that waits for the boundary after the `boundaries` boundaries of its item that come
/// before its declaration. When the record is full, or the wait does not fit its byte, mark the record
/// saturated from the frame of the entry and append nothing. O(1).
static void append_local(LocalRecord *record, uint32_t name, uint8_t frame, unsigned boundaries) {
    if (record->count >= MAX_LOCALS || boundaries >= UINT8_MAX) {
        saturate_locals(record, frame);
        return;
    }
    record->entries[record->count++] =
        (LocalEntry){.name = name, .frame = frame, .pending = (uint8_t)(boundaries + 1)};
}

/// The index after the initializer of a declarator at `start` and the `,` after it, or
/// LOCAL_NO_TOKEN when no `,` comes before `end`.
///
/// A `<` after a name that a `>` closes before `end` opens a template argument list, and its commas
/// divide no declarators: `auto d = f<Real, p, order>(j);` declares `d` alone. A comparison that a `>`
/// closes by chance, `int a = b < c, e = d > 1;`, then hides the declarator `e`, and that is a miss
/// and never a wrong entry.
///
/// A `,` between a `?` and its `:` is an operator in the second operand of a conditional expression,
/// and it divides no declarators: `U const n = +i < 0 ? *b++ = '-', U(0) - U(i) : U(i);` declares `n`
/// alone. The `,` after the third operand divides them: `int a = c ? 1 : 2, b;` declares `b` too. O(n)
/// in the tokens.
static unsigned local_next_declarator(const LocalItem *item, unsigned start, unsigned end) {
    if (local_range_out(item, end)) {
        return LOCAL_NO_TOKEN;
    }
    // The count of the `?` whose `:` did not come.
    unsigned conditionals = 0;
    for (unsigned i = start; i < end; i++) {
        const LocalToken *token = &item->tokens[i];
        if (local_mark_is(token, "<") && i > start &&
            (item->tokens[i - 1].kind == TOKEN_WORD || local_mark_is(&item->tokens[i - 1], "]"))) {
            unsigned after = local_skip_angles(item, i, end);
            if (after != LOCAL_NO_TOKEN) {
                i = after - 1;
                continue;
            }
        }
        if (token->kind == TOKEN_OPEN) {
            i = local_match(item, i, end);
            if (i == LOCAL_NO_TOKEN) {
                return LOCAL_NO_TOKEN;
            }
        } else if (token->kind == TOKEN_CLOSE) {
            return LOCAL_NO_TOKEN;
        } else if (local_mark_is(token, "?")) {
            conditionals++;
        } else if (local_mark_is(token, ":") && conditionals > 0) {
            conditionals--;
        } else if (token->kind == TOKEN_COMMA && conditionals == 0) {
            return i + 1;
        }
    }
    return LOCAL_NO_TOKEN;
}

/// Read a simple declaration from token `start` to `end`, and append each declarator name to `frame`.
/// Return the number of names that the declaration declares, with the names that did not fit.
///
/// A DECLARATOR NAME IS A PLAIN WORD WITH A TYPE BEFORE IT AND A DECLARATOR END AFTER IT: `T x =`,
/// `a * b;`, `std::vector<int> v(3)`, `unsigned n;`. A name that is a value in each reading of the
/// text is safe to record, and the declarator of `a * b;` is `b` whether the statement declares `b`
/// or multiplies. A word with no type before it is no declarator: `x = y;`, `f(x);`.
///
/// A parameter and a condition read one declarator, and a `=` starts its initializer. In a statement,
/// a `,` after an initializer starts the next declarator: `int a = 1, *b;`. The next declarator holds
/// only pointer operators and qualifiers before its name, so `auto f = [&]<typename D, typename P>` is
/// no list of declarators.
///
/// A macro-shaped word after a plain word that two plain types come before is an attribute macro of
/// that word, when the declaration ends after it: `Mutex mu MOZ_UNANNOTATED;`. The plain word is the
/// declarator. Each other macro-shaped word is a declarator: `const T Q_1_2[] = {...};`,
/// `BOOST_MATH_STATIC const T C3;`, `int N;`.
///
/// O(n) in the tokens.
static unsigned read_local_declaration(const LocalItem *item, unsigned start, unsigned end, LocalRecord *record,
                                       uint8_t frame, LocalDeclarationKind kind, bool typed) {
    if (local_range_out(item, end)) {
        return 0;
    }
    bool parameter = kind != LOCAL_STATEMENT;
    unsigned declared = 0;
    // The tokens of the type, the types among them, and the plain types that are not macro-shaped. A
    // declaration after the body of a class has the type of that body before its first token.
    unsigned units = typed ? 1 : 0;
    unsigned types = typed ? 1 : 0;
    unsigned plain_types = 0;
    // The index of a plain word that can be the declarator before a macro-shaped word, or LOCAL_NO_TOKEN.
    unsigned candidate = LOCAL_NO_TOKEN;
    // True after the `,` that starts a declarator after the first one.
    bool continuation = false;
    unsigned i = start;
    for (unsigned steps = 0; i < end; steps++) {
        if (steps == LOCAL_MAX_STEPS) {
            return declared;
        }
        const LocalToken *token = &item->tokens[i];
        if (local_mark_is(token, "[") && i + 1 < end && local_mark_is(&item->tokens[i + 1], "[")) {
            unsigned close = local_match(item, i, end);
            if (close == LOCAL_NO_TOKEN) {
                return declared;
            }
            i = close + 1;
            continue;
        }
        if (local_mark_is(token, "::") && units == 0) {
            i++;
            continue;
        }
        if (local_mark_is(token, "...") || local_mark_is(token, "*") || local_mark_is(token, "&") ||
            local_mark_is(token, "&&")) {
            if (units == 0) {
                return declared;
            }
            candidate = LOCAL_NO_TOKEN;
            i++;
            continue;
        }
        // A structured binding: `auto [a, b] = f();`, `const auto& [k, v] : map`. The `[` comes after
        // `auto` or after a reference operator after `auto`. After a name, it is a subscript: `a[i] = b;`.
        const LocalToken *before = i > start ? &item->tokens[i - 1] : NULL;
        bool binding = before != NULL &&
                       (local_word_is(before, "auto") ||
                        ((local_mark_is(before, "&") || local_mark_is(before, "&&")) && i - 1 > start &&
                         local_word_is(&item->tokens[i - 2], "auto")));
        if (local_mark_is(token, "[") && units > 0 && !binding) {
            return declared;
        }
        if (local_mark_is(token, "[") && binding) {
            unsigned close = local_match(item, i, end);
            if (close == LOCAL_NO_TOKEN || parameter) {
                return declared;
            }
            for (unsigned j = i + 1; j < close; j++) {
                if (item->tokens[j].kind == TOKEN_WORD) {
                    append_local(record, item->tokens[j].name, frame, item->tokens[j].boundaries);
                    declared++;
                } else if (item->tokens[j].kind != TOKEN_COMMA) {
                    return declared;
                }
            }
            return declared;
        }
        // `module` and `import` are keywords of a module unit only, and they are ordinary names
        // everywhere else: `int ViewMap_Init(PyObject *module)` of blender and
        // `void testImport(A *import)` of clang. The two words keep their stop at the first token of
        // an item, where `module foo;` declares a module and no local. Refer to task 394.
        bool module_word = strcmp(token->text, "module") == 0 || strcmp(token->text, "import") == 0;
        bool stop = word_in(token->text, LOCAL_STOP_WORDS) && (i == start || !module_word);
        if (token->kind != TOKEN_WORD || stop) {
            return declared;
        }
        if (continuation && (local_word_is(token, "const") || local_word_is(token, "volatile"))) {
            i++;
            continue;
        }
        // A later declarator has no type of its own: `[]<class... _Tail>` after `auto f =` is no list.
        if (continuation && (is_class_key(token->text) || strcmp(token->text, "enum") == 0 ||
                             word_in(token->text, LOCAL_GROUP_WORDS))) {
            return declared;
        }
        if (is_class_key(token->text) || strcmp(token->text, "enum") == 0) {
            // An elaborated type: `struct stat st;`, `enum class E e;`. The name belongs to the type.
            i++;
            if (i < end && (local_word_is(&item->tokens[i], "class") || local_word_is(&item->tokens[i], "struct"))) {
                i++;
            }
            if (i < end && item->tokens[i].kind == TOKEN_WORD) {
                i = local_skip_name(item, i, end);
                if (i == LOCAL_NO_TOKEN) {
                    return declared;
                }
            }
            units++;
            types++;
            candidate = LOCAL_NO_TOKEN;
            continue;
        }
        if (word_in(token->text, LOCAL_GROUP_WORDS)) {
            i++;
            if (i < end && local_mark_is(&item->tokens[i], "(")) {
                unsigned close = local_match(item, i, end);
                if (close == LOCAL_NO_TOKEN) {
                    return declared;
                }
                i = close + 1;
            }
            units++;
            types += strcmp(token->text, "decltype") == 0 || strncmp(token->text, "typeof", 6) == 0 ||
                     strncmp(token->text, "__typeof", 8) == 0;
            candidate = LOCAL_NO_TOKEN;
            continue;
        }
        unsigned after = local_skip_name(item, i, end);
        if (after == LOCAL_NO_TOKEN) {
            return declared;
        }
        bool plain = after == i + 1;
        bool keyword = word_in(token->text, RESERVED_WORDS);
        bool at_end = local_declarator_end(item, after, end, kind);
        if (types > 0 && plain && !keyword && at_end) {
            uint32_t name = token->name;
            bool ends = after == end || item->tokens[after].kind == TOKEN_COMMA || local_mark_is(&item->tokens[after], ")");
            if (plain_types >= 2 && candidate != LOCAL_NO_TOKEN && candidate + 1 == i && ends &&
                local_macro_shaped(token->text)) {
                name = item->tokens[candidate].name;
            }
            append_local(record, name, frame, token->boundaries);
            declared++;
            if (parameter || after == end) {
                return declared;
            }
            i = local_next_declarator(item, after, end);
            if (i == LOCAL_NO_TOKEN) {
                return declared;
            }
            // The declarators after a `,` share the type of the first one.
            candidate = LOCAL_NO_TOKEN;
            continuation = true;
            continue;
        }
        if (continuation) {
            return declared;
        }
        bool qualifier = word_in(token->text, LOCAL_QUALIFIER_WORDS);
        units++;
        types += !qualifier;
        plain_types += !qualifier && plain && !local_macro_shaped(token->text);
        candidate = plain && !keyword ? i : LOCAL_NO_TOKEN;
        i = after;
    }
    return declared;
}

/// Read the parameter declarations of the group from `open` to `close`, and append each parameter
/// name to `frame`. Return the number of names. A `,` inside brackets or inside a template argument
/// list does not divide two parameters: `std::map<int, int> m`.
///
/// NO PARAMETER DECLARATION STARTS WITH `(`. A group with such an entry is the argument list of a macro,
/// and it gives no entry. In `LLVM_LIBC_FUNCTION(short accum, exphk, (short accum x)) {`, the entry
/// `short accum` holds a type and no parameter, and `accum` is no local. O(n) in the tokens.
static unsigned read_local_parameters(const LocalItem *item, unsigned open, unsigned close, LocalRecord *record,
                                      uint8_t frame) {
    if (local_range_out(item, close) || open >= close || close >= item->count) {
        return 0;
    }
    unsigned declared = 0;
    // Pass 0 reads the first token of each entry, and pass 1 appends the names.
    for (unsigned pass = 0; pass < 2; pass++) {
        unsigned start = open + 1;
        unsigned angles = 0;
        for (unsigned i = open + 1; i <= close; i++) {
            const LocalToken *token = &item->tokens[i];
            if (i < close && token->kind == TOKEN_OPEN) {
                i = local_match(item, i, close);
                if (i == LOCAL_NO_TOKEN) {
                    return declared;
                }
                continue;
            }
            if (i < close && local_mark_is(token, "<") && i > start && item->tokens[i - 1].kind == TOKEN_WORD) {
                angles++;
            } else if (i < close && local_mark_is(token, ">") && angles > 0) {
                angles--;
            } else if (i == close || (token->kind == TOKEN_COMMA && angles == 0)) {
                if (pass == 0 && start < i && local_mark_is(&item->tokens[start], "(")) {
                    return 0;
                }
                if (pass == 1) {
                    declared += read_local_declaration(item, start, i, record, frame, LOCAL_PARAMETER, false);
                }
                start = i + 1;
                angles = 0;
            }
        }
    }
    return declared;
}

/// True when the tokens from `start` to `end` are the tokens between the parameter list of a
/// definition or of a lambda and its body: qualifiers, `noexcept(...)`, an attribute, a macro, a
/// trailing return type, a constraint, or a constructor initializer list with groups in parentheses.
///
/// An initializer in braces ends the item at its `{`: `A(int x) : m{x} {`. When `brace_init` is not
/// NULL, a list whose last entry is a name before that `{` also gives true, and `*brace_init` becomes
/// true. The caller then reads the rest of the list. O(n) in the tokens.
static bool local_trailers_reach(const LocalItem *item, unsigned start, unsigned end, bool *brace_init) {
    if (local_range_out(item, end)) {
        return false;
    }
    unsigned i = start;
    for (unsigned steps = 0; i < end; steps++) {
        if (steps == LOCAL_MAX_STEPS) {
            return false;
        }
        const LocalToken *token = &item->tokens[i];
        if (token->kind == TOKEN_WORD) {
            if (strcmp(token->text, "requires") == 0) {
                return true;
            }
            if (!word_in(token->text, LOCAL_TRAILER_WORDS) && !local_macro_shaped(token->text)) {
                return false;
            }
            i++;
            if (i < end && local_mark_is(&item->tokens[i], "(")) {
                i = local_match(item, i, end);
                if (i == LOCAL_NO_TOKEN) {
                    return false;
                }
                i++;
            }
            continue;
        }
        if (local_mark_is(token, "&") || local_mark_is(token, "&&")) {
            i++;
            continue;
        }
        if (local_mark_is(token, "[") && i + 1 < end && local_mark_is(&item->tokens[i + 1], "[")) {
            i = local_match(item, i, end);
            if (i == LOCAL_NO_TOKEN) {
                return false;
            }
            i++;
            continue;
        }
        if (local_mark_is(token, "->")) {
            // The trailing return type: names, `::`, template arguments, pointer operators, groups.
            i++;
            for (unsigned part_steps = 0; i < end; part_steps++) {
                if (part_steps == LOCAL_MAX_STEPS) {
                    return false;
                }
                const LocalToken *part = &item->tokens[i];
                if (local_mark_is(part, "<")) {
                    i = local_skip_angles(item, i, end);
                } else if (part->kind == TOKEN_OPEN) {
                    i = local_match(item, i, end);
                    if (i != LOCAL_NO_TOKEN) {
                        i++;
                    }
                } else if (part->kind == TOKEN_WORD || local_mark_is(part, "::") || local_mark_is(part, "*") ||
                           local_mark_is(part, "&") || local_mark_is(part, "&&") || local_mark_is(part, "...")) {
                    i++;
                } else {
                    return false;
                }
                if (i == LOCAL_NO_TOKEN) {
                    return false;
                }
            }
            return true;
        }
        if (local_mark_is(token, ":")) {
            // A constructor initializer list: `: a(x), b(y)`.
            i++;
            for (unsigned entry_steps = 0; i < end; entry_steps++) {
                if (entry_steps == LOCAL_MAX_STEPS) {
                    return false;
                }
                if (item->tokens[i].kind != TOKEN_WORD) {
                    return false;
                }
                i = local_skip_name(item, i, end);
                if (i == end && brace_init != NULL) {
                    *brace_init = true;
                    return true;
                }
                if (i == LOCAL_NO_TOKEN || i >= end || !local_mark_is(&item->tokens[i], "(")) {
                    return false;
                }
                i = local_match(item, i, end);
                if (i == LOCAL_NO_TOKEN) {
                    return false;
                }
                i++;
                if (i < end && local_mark_is(&item->tokens[i], "...")) {
                    i++;
                }
                if (i < end) {
                    if (item->tokens[i].kind != TOKEN_COMMA) {
                        return false;
                    }
                    i++;
                }
            }
            return true;
        }
        return false;
    }
    return true;
}

/// The index of the bracket that opens the bracket at `close`, or LOCAL_NO_TOKEN. O(n) in the tokens.
static unsigned local_match_back(const LocalItem *item, unsigned close) {
    if (local_range_out(item, close) || close >= item->count) {
        return LOCAL_NO_TOKEN;
    }
    unsigned depth = 0;
    for (unsigned i = close + 1; i-- > 0;) {
        TokenClass kind = item->tokens[i].kind;
        if (kind == TOKEN_CLOSE) {
            depth++;
        } else if (kind == TOKEN_OPEN && --depth == 0) {
            return i;
        }
    }
    return LOCAL_NO_TOKEN;
}

/// True when the tokens before the `(` at `open` name the declarator of a definition: a name that is
/// no keyword of an expression, a destructor, the `>` of a template argument list, or an `operator`
/// function. An attribute can come between the name and the list, `f [[gnu::used]] (int)`, and so can
/// the arguments of a macro that gives the name, `MONGO_INITIALIZER(Name)(InitializerContext *c)`.
/// O(n) in the tokens.
static bool local_head_name(const LocalItem *item, unsigned open) {
    if (open == 0 || local_range_out(item, open) || open >= item->count) {
        return false;
    }
    const LocalToken *before = &item->tokens[open - 1];
    if (before->kind == TOKEN_WORD) {
        return !word_in(before->text, LOCAL_NOT_HEAD_WORDS);
    }
    if (local_mark_is(before, ">")) {
        return true;
    }
    if (local_mark_is(before, "]") || local_mark_is(before, ")")) {
        unsigned group = local_match_back(item, open - 1);
        bool attribute = local_mark_is(before, "]") && group != LOCAL_NO_TOKEN && group + 1 < open &&
                         local_mark_is(&item->tokens[group + 1], "[");
        bool call = local_mark_is(before, ")") && group != LOCAL_NO_TOKEN;
        if ((attribute || call) && group > 0 && item->tokens[group - 1].kind == TOKEN_WORD &&
            !word_in(item->tokens[group - 1].text, LOCAL_NOT_HEAD_WORDS) &&
            !local_word_is(&item->tokens[group - 1], "operator")) {
            return true;
        }
    }
    // `operator()(int x)`, `operator==(const A& a)`, `operator new(size_t n)`.
    for (unsigned back = 2; back <= 4 && back <= open; back++) {
        if (local_word_is(&item->tokens[open - back], "operator")) {
            return true;
        }
    }
    return false;
}

/// Read the rest of a constructor initializer list from the character after the `{` of a braced
/// initializer, to the `{` of the body: `A(int x) : m{x}, n(x) {`. Return the count of the boundaries
/// from the `{` of the initializer to the `{` of the body, the two included, or 0 when the text is no
/// such list or the count does not fit a `uint8_t`. O(n) in the characters that the scan reads.
static unsigned count_initializer_boundaries(Reader *reader);

/// Read the rest of the head of a `for` from the character after its first `;`, to the `{` of its body.
/// Return the count of the boundaries from that `;` to the `{` of the body, the two included, or 0 when
/// the body is no block or the count does not fit a `uint8_t`. O(n) in the characters that the scan
/// reads.
static unsigned count_for_head_boundaries(Reader *reader) {
    unsigned boundaries = 1;
    // The `(` of the head is open.
    unsigned depth = 1;
    for (unsigned tokens = 0; tokens < 4 * LOCAL_ITEM_TOKENS; tokens++) {
        LOOP_STEP();
        TextToken token;
        if (!read_text_token(reader, &token) || reader->budget == 0) {
            return 0;
        }
        bool brace = token.kind == TOKEN_OPEN && token.text[0] == '{';
        if (brace || (token.kind == TOKEN_CLOSE && token.text[0] == '}') || token.kind == TOKEN_SEMICOLON) {
            boundaries++;
        }
        if (depth == 0) {
            return brace && boundaries <= UINT8_MAX ? boundaries : 0;
        }
        if (token.kind == TOKEN_OPEN) {
            depth++;
        } else if (token.kind == TOKEN_CLOSE) {
            depth--;
        }
    }
    return 0;
}

static unsigned count_initializer_boundaries(Reader *reader) {
    unsigned boundaries = 1;
    // The open brackets of the current initializer. The `{` of the first initializer is open.
    unsigned depth = 1;
    // True after an initializer closes, where a `,` or the `{` of the body comes next.
    bool after_entry = false;
    unsigned angles = 0;
    for (unsigned tokens = 0; tokens < 4 * LOCAL_ITEM_TOKENS; tokens++) {
        LOOP_STEP();
        TextToken token;
        if (!read_text_token(reader, &token) || reader->budget == 0) {
            return 0;
        }
        bool brace = token.kind == TOKEN_OPEN && token.text[0] == '{';
        if (brace || (token.kind == TOKEN_CLOSE && token.text[0] == '}') || token.kind == TOKEN_SEMICOLON) {
            boundaries++;
        }
        if (depth > 0) {
            if (token.kind == TOKEN_OPEN) {
                depth++;
            } else if (token.kind == TOKEN_CLOSE && --depth == 0) {
                after_entry = true;
            }
            continue;
        }
        if (after_entry) {
            if (brace) {
                return boundaries <= UINT8_MAX ? boundaries : 0;
            }
            if (token.kind == TOKEN_COMMA) {
                after_entry = false;
            } else if (strcmp(token.text, "...") != 0) {
                return 0;
            }
            continue;
        }
        // The name of the next initializer: words, `::`, and template arguments.
        if (angles == 0 && (brace || (token.kind == TOKEN_OPEN && token.text[0] == '('))) {
            depth = 1;
        } else if (strcmp(token.text, "<") == 0) {
            angles++;
        } else if (strcmp(token.text, ">") == 0 && angles > 0) {
            angles--;
        } else if (!(token.kind == TOKEN_WORD || strcmp(token.text, "::") == 0 ||
                     (angles > 0 && (token.kind == TOKEN_COMMA || token.kind == TOKEN_LITERAL)))) {
            return 0;
        }
    }
    return 0;
}

/// True when an item that ends at a `{` is the head of a class, a union or an enum, from token `start`:
/// `struct S {`, `static const struct {`, `class [[nodiscard]] awaitable {`, `enum class E : int {`. A
/// `=` outside brackets makes the item a declaration with an initializer in braces: `struct S s = {`.
/// O(n) in the tokens.
static bool local_class_head(const LocalItem *item, unsigned start) {
    unsigned n = item->count;
    if (item->end != '{' || local_range_out(item, n)) {
        return false;
    }
    unsigned i = start;
    for (unsigned steps = 0; i < n && steps < LOCAL_MAX_STEPS; steps++) {
        const LocalToken *token = &item->tokens[i];
        if (token->kind == TOKEN_WORD && word_in(token->text, LOCAL_QUALIFIER_WORDS)) {
            i++;
        } else if (local_mark_is(token, "[") && i + 1 < n && local_mark_is(&item->tokens[i + 1], "[")) {
            unsigned close = local_match(item, i, n);
            if (close == LOCAL_NO_TOKEN) {
                return false;
            }
            i = close + 1;
        } else {
            break;
        }
    }
    if (i >= n || item->tokens[i].kind != TOKEN_WORD ||
        !(is_class_key(item->tokens[i].text) || strcmp(item->tokens[i].text, "enum") == 0)) {
        return false;
    }
    for (unsigned j = i; j < n; j++) {
        if (item->tokens[j].kind == TOKEN_OPEN) {
            j = local_match(item, j, n);
            if (j == LOCAL_NO_TOKEN) {
                return false;
            }
        } else if (local_mark_is(&item->tokens[j], "=")) {
            return false;
        }
    }
    return true;
}

/// The index of the `[` of a lambda whose captures, parameter list and trailers reach token `end`, or
/// LOCAL_NO_TOKEN. `*open` and `*close` get its parameter list, or LOCAL_NO_TOKEN for a lambda with none
/// and for no lambda. The last such `[` wins. A `[` after a name, a `)`, a `]` or a literal is a
/// subscript: `a[i]`, `f()[0]`. O(n) for each `[` of the item.
///
/// A candidate that fails writes nothing to the caller. In `[=]({) {`, the `(` has no `)` in the item. An
/// `*open` of that candidate with no `*close` sends `read_local_parameters` past the last token.
static unsigned local_lambda_head(const LocalItem *item, unsigned end, unsigned *open, unsigned *close) {
    *open = LOCAL_NO_TOKEN;
    *close = LOCAL_NO_TOKEN;
    if (local_range_out(item, end)) {
        return LOCAL_NO_TOKEN;
    }
    for (unsigned p = end; p-- > 0;) {
        if (!local_mark_is(&item->tokens[p], "[") || (p + 1 < end && local_mark_is(&item->tokens[p + 1], "["))) {
            continue;
        }
        if (p > 0) {
            const LocalToken *before = &item->tokens[p - 1];
            bool operand = (before->kind == TOKEN_WORD && !word_in(before->text, LOCAL_STOP_WORDS)) ||
                           before->kind == TOKEN_CLOSE || before->kind == TOKEN_LITERAL;
            if (operand) {
                continue;
            }
        }
        unsigned captures = local_match(item, p, end);
        if (captures == LOCAL_NO_TOKEN) {
            continue;
        }
        unsigned after = captures + 1;
        // A generic lambda can have a template parameter list: `[&]<typename D, typename P>(D d, P p)`.
        if (after < end && local_mark_is(&item->tokens[after], "<")) {
            after = local_skip_angles(item, after, end);
            if (after == LOCAL_NO_TOKEN) {
                continue;
            }
        }
        unsigned params_open = LOCAL_NO_TOKEN;
        unsigned params_close = LOCAL_NO_TOKEN;
        if (after < end && local_mark_is(&item->tokens[after], "(")) {
            params_open = after;
            params_close = local_match(item, after, end);
            if (params_close == LOCAL_NO_TOKEN) {
                continue;
            }
            after = params_close + 1;
        }
        if (local_trailers_reach(item, after, end, NULL)) {
            *open = params_open;
            *close = params_close;
            return p;
        }
    }
    return LOCAL_NO_TOKEN;
}

/// True when an item starts a namespace or a class, after `export`, `inline`, `extern "C"`, a template
/// head, and attributes: `namespace std _GLIBCXX_VISIBILITY(default) {`, `class A DECLARE(x) {`. A group
/// before the body of such an item is no parameter list. O(n) in the tokens.
static bool local_item_starts_scope(const LocalItem *item) {
    if (local_range_out(item, item->count)) {
        return false;
    }
    unsigned i = 0;
    unsigned n = item->count;
    for (unsigned steps = 0; i < n && steps < LOCAL_MAX_STEPS; steps++) {
        const LocalToken *token = &item->tokens[i];
        if (local_word_is(token, "export") || local_word_is(token, "inline") || local_word_is(token, "extern") ||
            token->kind == TOKEN_LITERAL) {
            i++;
        } else if (local_word_is(token, "template") && i + 1 < n && local_mark_is(&item->tokens[i + 1], "<")) {
            i = local_skip_angles(item, i + 1, n);
            if (i == LOCAL_NO_TOKEN) {
                return false;
            }
        } else if (local_mark_is(token, "[") && i + 1 < n && local_mark_is(&item->tokens[i + 1], "[")) {
            i = local_match(item, i, n);
            if (i == LOCAL_NO_TOKEN) {
                return false;
            }
            i++;
        } else {
            return token->kind == TOKEN_WORD &&
                   (strcmp(token->text, "namespace") == 0 || strcmp(token->text, "enum") == 0 ||
                    strcmp(token->text, "concept") == 0 || is_class_key(token->text));
        }
    }
    return false;
}

/// Read a head that ends at the `{` of a body, and append its parameters to the frame of that body. A
/// head of a lambda can be inside brackets: `f(a, [](int x) {`. A head of a definition is at the top
/// level of the item: `void f(int x) {`, `A::A(int x) : a(x) {`, `TEST(Suite, Name) {`. At file scope, a
/// head with no parameter name appends an entry with the name 0, so that its body opens a frame.
///
/// The entries wait for the `{` of the body: the braces inside the item come first, `f(Options o = {}) {`.
/// A constructor whose initializer list has an initializer in braces ends the item at that `{`. With a
/// reader, the scan reads the rest of the list and counts its boundaries too. Without a reader, such a
/// head gives no entry.
///
/// Return true when the item is the head of a definition, and false for a lambda and for no head. The
/// name of a definition is no local of the frame, and the caller then reads no statement. O(n) in the
/// tokens.
static bool read_local_head(const LocalItem *item, LocalRecord *record, Reader *reader) {
    if (item->end != '{' || item->count == 0 || local_range_out(item, item->count) || record->frames == UINT8_MAX) {
        return false;
    }
    unsigned n = item->count;
    uint8_t body = record->frames + 1;
    unsigned open = LOCAL_NO_TOKEN;
    unsigned close = LOCAL_NO_TOKEN;
    unsigned wait = item->boundaries + 1;
    bool definition = false;
    if (local_lambda_head(item, n, &open, &close) == LOCAL_NO_TOKEN) {
        if (local_item_starts_scope(item)) {
            return false;
        }
        // A definition: the FIRST `(` at the top level whose group and trailers reach the body. Each entry
        // of a constructor initializer list after it reaches the body too: `A(int x) : m(x) {`.
        bool last_brace_init = false;
        for (unsigned i = 0; i < n && open == LOCAL_NO_TOKEN; i++) {
            if (item->tokens[i].kind != TOKEN_OPEN) {
                continue;
            }
            unsigned group_close = local_match(item, i, n);
            if (group_close == LOCAL_NO_TOKEN) {
                return false;
            }
            bool brace_init = false;
            if (local_mark_is(&item->tokens[i], "(") && local_head_name(item, i) &&
                local_trailers_reach(item, group_close + 1, n, &brace_init)) {
                open = i;
                close = group_close;
                last_brace_init = brace_init;
            }
            i = group_close;
        }
        if (open == LOCAL_NO_TOKEN) {
            return false;
        }
        definition = true;
        if (last_brace_init) {
            unsigned rest = reader == NULL ? 0 : count_initializer_boundaries(reader);
            if (rest == 0) {
                return definition;
            }
            wait = item->boundaries + rest;
        }
    }
    if (wait > UINT8_MAX) {
        return definition;
    }
    unsigned first = local_entry_count(record);
    unsigned declared = open == LOCAL_NO_TOKEN ? 0 : read_local_parameters(item, open, close, record, body);
    if (declared == 0 && record->frames == 0) {
        append_local(record, 0, body, 0);
    }
    for (unsigned i = first; i < local_entry_count(record); i++) {
        record->entries[i].pending = (uint8_t)wait;
    }
    return definition;
}

/// Read the declarations of a statement in a frame. A condition of `if`, `while` and `switch`, the head
/// of a `for`, and the parameter of `catch` declare their names in the frame of the body in braces. A
/// head of a `for` ends the item at its first `;`, and with a reader the scan reads the rest of the head
/// to that body. A statement with no body in braces gives no entry for its condition. O(n) in the
/// tokens.
static void read_local_statement(const LocalItem *item, LocalRecord *record, Reader *reader, bool after_class_body) {
    if (local_range_out(item, item->count)) {
        return;
    }
    if (after_class_body) {
        // The declarators after the `}` of a class body have the type of that body: `} s = {1};`, `} *p, a[2];`.
        read_local_declaration(item, 0, item->count, record, record->frames, LOCAL_STATEMENT, true);
        return;
    }
    unsigned n = item->count;
    unsigned i = 0;
    // The prefixes: `else`, a label, `case X:`, `default:`, and an access specifier.
    for (unsigned steps = 0;; steps++) {
        if (steps == LOCAL_MAX_STEPS) {
            return;
        }
        if (i < n && local_word_is(&item->tokens[i], "else")) {
            i++;
            continue;
        }
        if (i + 1 < n && item->tokens[i].kind == TOKEN_WORD && local_mark_is(&item->tokens[i + 1], ":") &&
            (!word_in(item->tokens[i].text, RESERVED_WORDS) || local_word_is(&item->tokens[i], "default") ||
             local_word_is(&item->tokens[i], "public") || local_word_is(&item->tokens[i], "private") ||
             local_word_is(&item->tokens[i], "protected"))) {
            i += 2;
            continue;
        }
        if (i < n && local_word_is(&item->tokens[i], "case")) {
            unsigned j = i + 1;
            while (j < n && !local_mark_is(&item->tokens[j], ":")) {
                j++;
            }
            if (j == n) {
                return;
            }
            i = j + 1;
            continue;
        }
        break;
    }
    if (i >= n || record->frames == UINT8_MAX) {
        return;
    }
    const LocalToken *first = &item->tokens[i];
    uint8_t body = record->frames + 1;
    // The head of a class, whose body ends the item, declares a type and no local:
    // `struct ABSL_ATTRIBUTE_TRIVIAL_ABI Probe {`, `class [[nodiscard]] awaitable {`. Its body is a frame
    // whose `}` the declarators of that type follow.
    if (local_class_head(item, i)) {
        record->class_head = 1;
        return;
    }
    // A declaration of a condition, of a `for` head, or of a `catch` is in scope in the body of its
    // statement, and only a body in braces is a frame. A statement with no such body gives no entry,
    // because its declaration leaves scope at a `;` that no frame closes: after
    // `for (auto *Use : I->users()) f(Use);`, `for (Use &Op : ...)` reads `Use` as a type.
    bool control = local_word_is(first, "if") || local_word_is(first, "while") ||
                   local_word_is(first, "switch") || local_word_is(first, "for");
    if (control || local_word_is(first, "catch")) {
        if (i + 1 >= n || !local_mark_is(&item->tokens[i + 1], "(")) {
            return;
        }
        unsigned close = local_match(item, i + 1, n);
        unsigned end = close == LOCAL_NO_TOKEN ? n : close;
        unsigned wait = item->boundaries + 1;
        if (local_word_is(first, "for") && item->end == ';' && close == LOCAL_NO_TOKEN) {
            // `for (int i = 0; i < n; ++i) {`: the item ends at the first `;` of the head, and the scan
            // reads the rest of the head to find the body.
            unsigned rest = reader == NULL ? 0 : count_for_head_boundaries(reader);
            if (rest == 0) {
                return;
            }
            wait = item->boundaries + rest;
        } else if (item->end != '{') {
            return;
        }
        if (wait > UINT8_MAX) {
            return;
        }
        unsigned first_entry = local_entry_count(record);
        LocalDeclarationKind kind = LOCAL_CONDITION;
        if (local_word_is(first, "catch")) {
            kind = LOCAL_PARAMETER;
        } else if (local_word_is(first, "for") && close == LOCAL_NO_TOKEN) {
            kind = LOCAL_FOR_INIT;
        }
        read_local_declaration(item, i + 2, end, record, body, kind, false);
        for (unsigned entry = first_entry; entry < local_entry_count(record); entry++) {
            record->entries[entry].pending = (uint8_t)wait;
        }
        return;
    }
    read_local_declaration(item, i, n, record, record->frames, LOCAL_STATEMENT, false);
}

/// Apply a boundary to a record: each pending entry comes one boundary nearer to scope, a `{` opens a
/// frame, and a `}` closes the innermost frame with its entries in scope. O(n) in the entries.
///
/// A `{` opens a frame in a frame, and at file scope only when an entry of a head comes into scope at it.
/// An entry in scope whose frame is not open after the boundary goes: the entries of a closed frame,
/// and the entries of a head whose body did not come at the boundary that the head scan counted. A
/// pending entry stays across a `}`, because the braces of a constructor initializer list come before
/// the body of its head: `A() : m{1} {`.
///
/// A `{` after the scan of a class head opens a frame that is a class body. Return true when the
/// boundary is the `}` of such a frame.
static bool apply_local_boundary(LocalRecord *record, int32_t boundary) {
    bool body = false;
    bool class_body = false;
    unsigned count = local_entry_count(record);
    for (unsigned i = 0; i < count; i++) {
        LocalEntry *entry = &record->entries[i];
        if (entry->pending > 0 && --entry->pending == 0) {
            body |= entry->frame > record->frames;
        }
    }
    bool class_head = record->class_head != 0;
    record->class_head = 0;
    if (boundary == '{' && (record->frames > 0 || body)) {
        if (record->frames == UINT8_MAX) {
            saturate_locals(record, 1);
        } else {
            record->frames++;
            uint32_t bit = record->frames <= 32 ? (uint32_t)1 << (record->frames - 1) : 0;
            record->class_bodies = class_head ? (record->class_bodies | bit) : (record->class_bodies & ~bit);
        }
    } else if (boundary == '}' && record->frames > 0) {
        if (record->saturated >= record->frames) {
            record->saturated = 0;
        }
        uint32_t bit = record->frames <= 32 ? (uint32_t)1 << (record->frames - 1) : 0;
        class_body = (record->class_bodies & bit) != 0;
        record->class_bodies &= ~bit;
        record->frames--;
    } else if (boundary == '}') {
        record->saturated = 0;
    }
    unsigned kept = 0;
    for (unsigned i = 0; i < count; i++) {
        const LocalEntry *entry = &record->entries[i];
        if (entry->pending > 0 || entry->frame <= record->frames) {
            record->entries[kept++] = *entry;
        }
    }
    record->count = (uint8_t)kept;
    return class_body;
}

/// True when two records hold the same entries, frames, saturation and class bodies. O(n) in the entries.
static bool local_record_same(const LocalRecord *a, const LocalRecord *b) {
    if (a->count != b->count || a->frames != b->frames || a->saturated != b->saturated ||
        a->class_bodies != b->class_bodies || a->class_head != b->class_head) {
        return false;
    }
    for (unsigned i = 0; i < local_entry_count(a); i++) {
        const LocalEntry *x = &a->entries[i];
        const LocalEntry *y = &b->entries[i];
        if (x->name != y->name || x->frame != y->frame || x->pending != y->pending) {
            return false;
        }
    }
    return true;
}

#ifdef TS_CPP_LOCAL_TRACE
// MEASUREMENT BUILD ONLY. A build with TS_CPP_LOCAL_TRACE records each change of the record of locals with
// its byte offset, so that a tool can compare the frames and the entries with a tree. No shipped build
// defines it.

/// The first members of the `Lexer` of vendor/tree-sitter/src/lexer.h: `TSLexer data` and the byte of
/// `Length current_position`. The tool that reads the trace checks the offsets against a known input.
typedef struct {
    TSLexer data;
    uint32_t bytes;
} TraceLexer;

static _Thread_local char *trace_text;
static _Thread_local size_t trace_length;
static _Thread_local size_t trace_capacity;

/// Append one line of the trace: the byte of the boundary, the boundary, the frames, the saturation,
/// and each entry as its hash, its frame, and its pending mark.
static void trace_local_mark(uint32_t bytes, int32_t boundary, const LocalRecord *record) {
    char line[64 + MAX_LOCALS * 24];
    int length = snprintf(line, sizeof line, "%u\t%c\t%u\t%u", bytes, (char)boundary, record->frames, record->saturated);
    for (unsigned i = 0; i < local_entry_count(record); i++) {
        const LocalEntry *entry = &record->entries[i];
        length += snprintf(line + length, sizeof line - (size_t)length, "\t%u:%u:%u", entry->name, entry->frame,
                           entry->pending);
    }
    length += snprintf(line + length, sizeof line - (size_t)length, "\n");
    if (trace_length + (size_t)length > trace_capacity) {
        size_t capacity = trace_capacity == 0 ? 1 << 16 : trace_capacity * 2;
        while (capacity < trace_length + (size_t)length) {
            capacity *= 2;
        }
        trace_text = realloc(trace_text, capacity);
        trace_capacity = capacity;
    }
    memcpy(trace_text + trace_length, line, (size_t)length);
    trace_length += (size_t)length;
}

/// Copy the trace of this thread into `out` and clear it, when `capacity` holds it. Return its length.
size_t tree_sitter_cpp_local_trace_take(char *out, size_t capacity) {
    size_t length = trace_length;
    if (length <= capacity) {
        memcpy(out, trace_text, length);
        trace_length = 0;
    }
    return length;
}
#endif

/// Scan a boundary, `{`, `}`, or `;`, and change the record of locals with no token. The scan applies
/// the boundary, reads the next item, and appends the names that the item declares. Refer to "THE RECORD
/// OF LOCALS" above.
///
/// The scan gives no token in each path. When the record changes, the result symbol is
/// TREE_SITTER_EXTERNAL_STATE_ONLY, and the runtime of ABI 1018 stores the state on the boundary
/// token that the internal lexer reads next. A version reads a boundary one time, because the state
/// after it lives on the boundary token itself, and a version that holds that state stands after it.
/// O(n) in the characters of the item and in the entries.
static bool scan_local_boundary(Scanner *scanner, TSLexer *lexer) {
    LocalRecord *record = &scanner->locals;
    int32_t boundary = lexer->lookahead;
#ifdef TS_CPP_LOCAL_TRACE
    uint32_t bytes = ((const TraceLexer *)lexer)->bytes;
#endif
    mark_end(lexer);
    Reader reader = start_reader(lexer, LOCAL_ITEM_LIMIT, scanner);
    step(&reader);
    LocalItem item;
    read_local_item(&reader, &item);
    LocalRecord next = *record;
    bool after_class_body = apply_local_boundary(&next, boundary);
    // The head of a definition declares no local of the frame: `S(int val) : val_(val) {` is a
    // constructor, and `S` is a type. So a head that matches skips the statement.
    if (item.end != 0 && !read_local_head(&item, &next, &reader) && next.frames > 0) {
        read_local_statement(&item, &next, &reader, after_class_body);
    }
    if (local_record_same(record, &next)) {
        return false;
    }
    *record = next;
#ifdef TS_CPP_LOCAL_TRACE
    trace_local_mark(bytes, boundary, record);
#endif
    lexer->result_symbol = TREE_SITTER_EXTERNAL_STATE_ONLY;
    return false;
}

/// Scan the item after a directive line, and append the names that it declares. The token of the
/// directive carries the change, and this scan gives no token of its own. It applies no boundary.
///
/// A DIRECTIVE LINE IS THE BOUNDARY BEFORE AN ITEM WHERE NO `;`, `{` OR `}` IS: `#include <x>` and then
/// `int main(int argc, char **argv) {`. Over the corpus, 127,112 definitions follow a directive line
/// directly. The caller gives the token that ends the line: the line end, or an `#endif` or `#else` of a
/// structured group, which has no line end.
///
/// THE SAME ITEM CAN BE READ TWO TIMES, and the second read appends nothing. A version that reads
/// `#endif` as a structured group and a version that reads it as a line with a line end read the same
/// item. A boundary scan before a directive line that it reads past, `;` and `#pragma once`, reads the
/// same item as the line end of that directive. The entries that such a read appends equal the entries
/// at the end of the record, and the scan keeps one copy. O(n) in the characters of the item and in the
/// entries.
static void scan_local_after_directive(Scanner *scanner, TSLexer *lexer) {
    LocalRecord next = scanner->locals;
    unsigned before = local_entry_count(&next);
    uint8_t saturated = next.saturated;
#ifdef TS_CPP_LOCAL_TRACE
    uint32_t bytes = ((const TraceLexer *)lexer)->bytes;
#endif
    Reader reader = start_reader(lexer, LOCAL_ITEM_LIMIT, scanner);
    LocalItem item;
    read_local_item(&reader, &item);
    if (item.end == 0) {
        return;
    }
    if (!read_local_head(&item, &next, &reader) && next.frames > 0) {
        read_local_statement(&item, &next, &reader, false);
    }
    unsigned after = local_entry_count(&next);
    unsigned added = after > before ? after - before : 0;
    if (added == 0 && next.saturated == saturated && next.class_head == scanner->locals.class_head) {
        return;
    }
    if (added > 0 && before >= added) {
        bool same = true;
        for (unsigned i = 0; i < added && same; i++) {
            const LocalEntry *old = &next.entries[before - added + i];
            const LocalEntry *new = &next.entries[before + i];
            same = old->name == new->name && old->frame == new->frame && old->pending == new->pending;
        }
        if (same) {
            return;
        }
    }
    scanner->locals = next;
#ifdef TS_CPP_LOCAL_TRACE
    trace_local_mark(bytes, 'n', &scanner->locals);
#endif
}

/// The answer of the record of locals about a name.
typedef enum {
    /// No live entry has the name, and the record lost no entry.
    LOCAL_NO,
    /// A live entry has the name.
    LOCAL_YES,
    /// No live entry has the name, and the record lost an entry that is in scope.
    LOCAL_UNKNOWN,
} LocalAnswer;

/// The answer of the record of locals about the name of the hash, at the position of the scanner.
///
/// A HASH IS NOT A NAME, so two names with one hash give one answer. Over the 73,288,021 distinct words
/// of the corpus files, summed for each file, two pairs of distinct words in one file share a hash:
/// `limiter` and `Denominator` in the two copies of llvm/lib/Analysis/ValueTracking.cpp, and
/// `kScalarCount` and `get` in arrow/cpp/src/arrow/compute/function_benchmark.cc. With no scanner,
/// the answer is `LOCAL_UNKNOWN`. O(n) in the entries.
static LocalAnswer local_name_answer(const Scanner *scanner, uint32_t name) {
    if (scanner == NULL) {
        return LOCAL_UNKNOWN;
    }
    const LocalRecord *record = &scanner->locals;
    for (unsigned i = local_entry_count(record); i-- > 0;) {
        const LocalEntry *entry = &record->entries[i];
        if (entry->pending == 0 && entry->name == name && name != 0) {
            return LOCAL_YES;
        }
    }
    return record->saturated != 0 ? LOCAL_UNKNOWN : LOCAL_NO;
}

/// True when a live entry of the record of locals has the name of the hash.
static bool is_local_name(const Scanner *scanner, uint32_t name) {
    return local_name_answer(scanner, name) == LOCAL_YES;
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
/// after the name, as in `(f(x))`. Give the form of the name, and the text, the shape and the hash of its
/// last identifier. The hash is the hash of `read_word`. Return false for a different text, and for a
/// keyword. O(n) in the characters that it reads.
static bool read_parenthesized_name(Reader *reader, NameForm *form, NameShape *shape, char text[NAME_WORD_SIZE],
                                    uint32_t *hash) {
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
        *hash = HASH_START;
        while (readable(reader) && is_word_char(lexer->lookahead)) {
            LOOP_STEP();
            if (length < NAME_WORD_SIZE - 1) {
                text[length] = lexer->lookahead < 0x80 ? (char)lexer->lookahead : '?';
            }
            *hash = hash_character(*hash, lexer->lookahead);
            length++;
            step(reader);
        }
        *hash = finish_hash(*hash);
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
/// In the three operators, a plain name that the record of locals holds keeps the expression. A local
/// is a variable, and a variable is no type: `sizeof(FloatBuffer)` after `char FloatBuffer[32];` in
/// compiler-rt, and `__alignof__ (aa)` after `A aa;` in gcc g++.dg/cpp0x/gen-attrs-52.C.
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
/// - A plain name that the record of locals holds keeps the expression before `(x)`, `*`, `&`, `+`, and
///   `-`, whatever its shape. A local is a variable, and a variable is no type: in opencv
///   `(const uchar*)(S) + step`, `S` is a parameter.
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
    uint32_t hash = 0;
    if (!read_parenthesized_name(&reader, &form, &shape, name, &hash)) {
        return false;
    }
    bool local = form == NAME_PLAIN && is_local_name(scanner, hash);
    bool alignof_operand = valid_symbols[ALIGNOF_TYPE_PAREN];
    bool operand = valid_symbols[OPERAND_TYPE_PAREN];
    bool type = false;
    if (alignof_operand) {
        // Each name is a type. Only a name with a call group or a local is an expression: `__alignof__(f(x))`.
        type = form != NAME_CALL && !local;
    } else if (operand) {
        type = form != NAME_CALL && !local && (form != NAME_PLAIN || shape == SHAPE_TYPE);
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
            type = group.operands == 1 && type_shape && !local;
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
            type = !binary_only && !local && (type_shape || tight_cast);
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

/// The words after the parameter list of a member function that end its declarator: the cv-qualifiers
/// and the noexcept-specifier of parameters-and-qualifiers ([dcl.decl]), the dynamic exception
/// specification `throw()`, and the virt-specifiers ([class.mem]). The ref-qualifiers `&` and `&&` are
/// characters, and `scan_template_parameter_declarator` reads them apart.
static const char *const FUNCTION_QUALIFIER_WORDS[] = {
    "const", "volatile", "noexcept", "throw", "override", "final", NULL,
};

/// Select `TEMPLATE_PARAMETER_DECLARATOR_TYPE` for a type that a template head declares, where the
/// declarator follows the type and a macro follows the declarator: `T value LIFETIME_BOUND`.
///
/// The caller read the first name and the second name. `declarator` is the second name. The reader
/// is at the end of it. The scan reads the gap, each macro with an optional argument list, and the
/// text after the last macro. It gives no token when one of these tests fails.
///
/// EACH TEST BELOW NAMES AN INPUT THAT IT DECIDES, and a reader can parse that input with the test
/// and without it. A comment that names an input which a later test stops anyway is a record of an
/// occasion and not of a mechanism. Refer to the two guards of `scan_after_macro_call`.
///
/// A KEYWORD IS A SPECIFIER AND NEVER A DECLARATOR. `void d(T typename NAME)` has the dependent type
/// `typename NAME`, `void e(T struct NAME)` has the elaborated type `struct NAME`, and
/// `void g(T class NAME)` has the same. Without `is_grammar_keyword` the scan gives the token for
/// each of the three, and each parameter then loses its type. `void a(T const NAME)` gives the same
/// tree with the test and without it, because a qualifier is not a type, so that input is NOT
/// evidence for this test.
///
/// THE MACRO CAN BE ON THE LINE OF THE DECLARATOR OR ON A LATER LINE, AND THE SCAN NEEDS NO TEST
/// FOR THE LINE BREAK. The 81 sites of the corpus split into 45 on one line and 36 across a line
/// break, with a comment or a blank line between the two in 12 of the 36. The first version of this
/// scan took the 45 only. A draft with no test for the line break, measured with `xtask trees` over
/// the corpus at f27dcd0 and again at 150087f, changed exactly the 16 files that hold the 36 sites,
/// 7 of them clean, and no other file. The population that a test for the line break would have to
/// decline is zero, so there is no such test. This is NOT the terminator of a chain of macro
/// invocations in `scan_after_macro_call`, which is a different mechanism with a measured price of
/// two clean files. `skip_gap` passes the comments and the blank lines.
///
/// THE TESTS ON THE MACRO ARE THE TESTS OF `read_macro_name`, WHICH READS THE MACRO LATER. That
/// function gives the token of the attribute macro after the declarator. It rejects a name with a
/// lowercase letter, a name of one character, and a name with a character that is not ASCII. A
/// difference between the two tests makes a declaration that has the type token and no macro token,
/// and such a declaration gets an ERROR node. For this reason the tests here are the stricter ones.
///
/// THE TEXT AFTER THE MACROS MUST END THE DECLARATOR. A `,` or a `)` ends a parameter, and each of the
/// 81 sites of the template head is a parameter. A `;` ends a declaration or a field: `Mutex mu
/// MOZ_UNANNOTATED;` in firefox and `hb_locale_t oldlocale HB_UNUSED;` in harfbuzz. A `{` starts the
/// braced initializer of a declaration or of a field: `atomic_uintptr_t XRayArgLogger
/// SANITIZER_INTERFACE_ATTRIBUTE{0};` in compiler-rt. A `=` starts the initializer of a declaration
/// or the default argument of a parameter: `Agg good1 ABSL_ATTRIBUTE_UNUSED = {1, 2};` in abseil and
/// `const_pointer hint MY_ATTRIBUTE((unused)) = nullptr` in hhvm, where `optional_parameter_declaration`
/// reads the macro in the declarator. `void b(T value MACRO x)` has a different word there, and
/// without this test it gets an ERROR node.
///
/// A SECOND MACRO CAN COME AFTER THE FIRST MACRO AND ITS GROUP, AND THE TEST READS THE TEXT AFTER THE
/// LAST MACRO: `static __thread ThreadLocalData threadlocal_data_ CACHELINE_ALIGNED ATTR_INITIAL_EXEC;`
/// in gperftools. Each macro gets the tests of the first macro, and only the first macro needs the
/// macro row of `seed_macro`.
///
/// A QUALIFIER OF A MEMBER FUNCTION ENDS THE DECLARATOR ONLY AFTER A PARAMETER LIST, WHICH IS A GROUP
/// THAT HOLDS NO EXPRESSION: `static MyInt min BOOST_PREVENT_MACRO_SUBSTITUTION () throw() {` in
/// boost. Of the 175 clean sites of this shape that a census of 2026-09-17 found, the scan reaches 144
/// before `const`, 8 before `throw` and 6 before `noexcept`, with the seeds of the corpus.
/// A group of expressions before `const` is the arguments of the macro and not a parameter list:
/// `T f PREVENT (x) const { return 1; }` keeps the tree of today with this test, and it gets an ERROR
/// node without it. The ref-qualifier has the same test: `T f PREVENT (x) & { return 1; }`.
///
/// WITH `seed_macro`, THE MACRO MUST HAVE A MACRO ROW IN THE SEED OF THE PROJECT. The caller sets it
/// when no source declares the first name as a type, so the macro row is then the evidence of the
/// construct. The name shape stays a filter and is never the evidence, because the scan of
/// `DECLARATOR_MACRO_NAME` that reads the macro later takes the shape only.
static bool scan_template_parameter_declarator(Reader *reader, const char *declarator, bool seed_macro,
                                               const bool *valid_symbols) {
    TSLexer *lexer = reader->lexer;
    if (!valid_symbols[TEMPLATE_PARAMETER_DECLARATOR_TYPE] || declarator[0] == '\0' ||
        is_grammar_keyword(declarator) || strchr(declarator, '?') != NULL) {
        return false;
    }
    Gap after_declarator = {0};
    skip_gap(reader, &after_declarator);
    if (after_declarator.blocked || !readable(reader) || !is_word_start(lexer->lookahead)) {
        return false;
    }
    // The buffer holds the length of a seed name. A name of `MACRO_WORD_SIZE` characters or more is
    // the name that a buffer of that size cuts, and the shape test declines it as before.
    char macro[TS_CPP_SEED_WORD_SIZE];
    bool macro_has_lower = false;
    read_word_sized(reader, macro, TS_CPP_SEED_WORD_SIZE, &macro_has_lower);
    for (bool first = true;; first = false) {
        LOOP_STEP();
        if (strlen(macro) < 2 || strlen(macro) >= MACRO_WORD_SIZE || reader->word_cut ||
            !is_macro_name(macro, macro_has_lower) || is_grammar_keyword(macro) || strchr(macro, '\\') != NULL) {
            return false;
        }
        if (first && seed_macro && !is_seed_macro_name(reader->scanner, macro, reader)) {
            return false;
        }
        Gap after_macro = {0};
        skip_gap(reader, &after_macro);
        if (after_macro.blocked || !readable(reader)) {
            return false;
        }
        // A group that holds no expression comes directly after this macro, so the group is the parameter
        // list of a function.
        bool parameters = false;
        // The argument list of the macro comes between the macro and the end of the parameter:
        // `CentralityMap centrality BOOST_GRAPH_ENABLE_IF_MODELS_PARM(Graph, vertex_list_graph_tag)`.
        if (lexer->lookahead == '(') {
            Arguments arguments = {0};
            if (!skip_group(reader, &arguments) || reader->budget == 0) {
                return false;
            }
            bool no_expression = arguments.empty || arguments.not_expressions || arguments.statements;
            // THE GROUP IS THE ARGUMENTS OF THE MACRO OR THE PARAMETER LIST OF THE DECLARATOR, AND THIS
            // TEST IS THE TEST OF `scan_trailing_macro_name`, WHICH READS THE GROUP LATER. A group of
            // expressions is the arguments of the macro. An empty group, or a group with a token that no
            // expression holds, is the parameter list of a function, and the macro sits between the name
            // of the function and that list: `Rep max BOOST_PREVENT_MACRO_SUBSTITUTION () {` in
            // boost/chrono. A group that is neither gets no macro token from that scan, and the type
            // token alone then gives an ERROR node in the place of the wrong tree of today:
            // `AnyGlobalsTypeInternal Any_globals_ PROTOBUF_MESSAGE_GLOBALS_SECTION(.data.rel.ro)` in
            // 37 sites of protobuf, 14 files that hold an error with this test and without it. A draft
            // without this test gave those 14 files a different wrong tree, and this test keeps the
            // tree that they have today.
            if (no_expression && arguments.not_parameters) {
                return false;
            }
            parameters = no_expression;
            Gap after_arguments = {0};
            skip_gap(reader, &after_arguments);
            if (after_arguments.blocked || !readable(reader)) {
                return false;
            }
        }
        int32_t end = lexer->lookahead;
        if (end == ',' || end == ')' || end == ';' || end == '{' || end == '=') {
            break;
        }
        if (end == '&') {
            if (!parameters) {
                return false;
            }
            break;
        }
        if (!is_word_start(end)) {
            return false;
        }
        read_word_sized(reader, macro, TS_CPP_SEED_WORD_SIZE, &macro_has_lower);
        if (word_in(macro, FUNCTION_QUALIFIER_WORDS)) {
            if (!parameters) {
                return false;
            }
            break;
        }
    }
    lexer->result_symbol = TEMPLATE_PARAMETER_DECLARATOR_TYPE;
    return true;
}

/// Select `TEMPLATE_PARAMETER_DECLARATOR_TYPE` for a type that a class head of the file or the seed
/// of the project declares, where a declarator follows the type and a macro follows the declarator:
/// `Mutex mu MOZ_UNANNOTATED;` in firefox, `hb_codepoint_t glyph HB_UNUSED` in harfbuzz, `StringRef
/// ref CATCH_ATTR_LIFETIMEBOUND` in Catch2.
///
/// The caller read the first name, and the character after the name is a blank or a line break. The
/// reader is at the end of the name. The scan reads the blanks, an optional gap, and the second
/// name, and it gives the rest of the text to `scan_template_parameter_declarator`.
///
/// THE BLANKS AFTER THE NAME BELONG TO THE SCAN OF A COMPARISON, AND THIS SCAN READS THEM THROUGH
/// THAT SCAN. The positions of this scan are the positions where a type can start. The measurement
/// of 2026-09-16 over the 43,269,941 such positions of the corpus with a blank after the name says
/// which other scans of `scan_word_start` run there: `COMPARISON_NAME` is valid at 23,001,326 of
/// them, `TYPE_TRAIT_TYPE_MARKER` at all of them, so the functional cast never is, and
/// `ALIGNAS_TYPE_NAME` and `POINTER_CALL_MACRO_NAME` at none. So the one scan after this one that
/// can give a token for a name with a blank after it is the scan of a comparison, for `x < 0 || x >
/// (n - 1)`. The lexer cannot go back, so this scan cannot read the blanks and then let that scan
/// read them again. It calls that scan first. A `<` or a `(` after the blanks keeps the token that
/// it has today. A name after the blanks is the declarator, and the scan continues from there with
/// the mark at the end of the type. A draft with no hand-off, and with every plain name admitted,
/// changed 542 files of the corpus, and 163 of them lost a comparison to a template-id:
/// `zfac < 1.e-6f && zfac > -1.e-6f` in blender.
///
/// THE SCANS OF A MACRO INVOCATION GIVE NO TOKEN FOR A NAME OF THESE TWO SOURCES, so no hand-off to
/// `scan_macro_start` is necessary. A name that enters on the macro rows of the seed is a name of
/// neither source, and the caller keeps a name with a macro row out for this reason. The caller admits no macro-shaped name, and `scan_macro_start`
/// gives its tokens to a macro-shaped name, to a SAL name, to a name where a member starts that is
/// not a recorded class name, or to a name before a constructor that is not a recorded class name.
/// A recorded class name reaches none of those. A seed name that is not a recorded class name can
/// reach the last two, and the text that takes them is a type before a macro invocation or before a
/// constructor, which is no C++ declaration.
///
/// A KEYWORD IS A SPECIFIER AND NEVER A DECLARATOR, AND A MACRO-SHAPED SECOND NAME IS THE OTHER
/// CONSTRUCT. `Mutex MOZ_UNANNOTATED mu;` reads today as the attribute macro `Mutex`, the type
/// `MOZ_UNANNOTATED` and the declarator `mu`, which is the swapped reading that the token of
/// `TEMPLATE_PARAMETER_TYPE_NAME` repairs when a template head declares the first name. This scan
/// keeps that reading for a class head and for a seed name, because the population of that construct
/// is a different table: 6,518 sites at 3977e59 whose first name is plain and whose second name is
/// macro-shaped, most of them SAL annotations where the first name IS the macro. A commit that takes
/// the ones with a declared first name measures that table.
static bool scan_declared_type_declarator(Reader *reader, const char *word, bool comparison, bool cast_name,
                                          bool seed_macro, const bool *valid_symbols) {
    TSLexer *lexer = reader->lexer;
    uint32_t before = reader->budget;
    bool after_blanks = false;
    if (scan_comparison_name(reader, word, comparison, cast_name, &after_blanks)) {
        return true;
    }
    // A `<` after the blanks took the scan of a comparison past them, and that scan gave no token.
    // The reader is then somewhere in the comparison, and this scan reads nothing there.
    if (!after_blanks || !readable(reader)) {
        return false;
    }
    // The scan of a comparison keeps a budget of MAX_COMPARISON_LOOKAHEAD, and this scan reads a gap
    // that can hold a long comment before the macro. The budget of this scan is the budget before
    // that scan, less the blanks that it read. `clamped` is the budget that it started with.
    uint32_t clamped = before < MAX_COMPARISON_LOOKAHEAD ? before : MAX_COMPARISON_LOOKAHEAD;
    reader->budget = before - (clamped - reader->budget);
    if (!is_word_start(lexer->lookahead)) {
        // A LINE BREAK OR A COMMENT AFTER THE BLANKS IS A GAP BEFORE THE DECLARATOR, AND NOT THE END
        // OF THE CONSTRUCT. The scan of a comparison gives no token there today, so the path through
        // the gap changes no tree of a comparison.
        int32_t c = lexer->lookahead;
        if (!is_line_break(c) && c != '/' && c != '\\' && c != '\f' && c != '\v') {
            return false;
        }
        Gap after_type = {0};
        skip_gap(reader, &after_type);
        if (after_type.blocked || !readable(reader) || !is_word_start(lexer->lookahead)) {
            return false;
        }
    }
    char second[MACRO_WORD_SIZE];
    bool second_has_lower = false;
    read_word(reader, second, &second_has_lower);
    if (strlen(second) >= 2 && is_macro_name(second, second_has_lower) && !is_grammar_keyword(second)) {
        return false;
    }
    return scan_template_parameter_declarator(reader, second, seed_macro, valid_symbols);
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
    // THE TEMPLATE HEAD COMES BEFORE EVERY BRANCH THAT READS `next`. `template<` with no blank puts
    // a `<` there, and the member pointer branch below takes a word before a `<`, so a check after
    // it never saw `template<` and saw only `template <`. The word is a keyword and can be the name
    // of nothing, so no other branch loses a reading to this one. The scan writes the record of template
    // parameters with no token, in each state where the parser calls this scanner.
    if (strcmp(word, "template") == 0) {
        return scan_template_head(scanner, &reader);
    }
    // The word `using` writes the record of aliases with no token, in each state where the parser calls
    // this scanner, as the class key writes the record of class heads.
    if (strcmp(word, "using") == 0) {
        return scan_using_alias(scanner, &reader);
    }
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
    // A class key writes the record of class heads with no token. The scan runs in each state where the
    // parser calls this scanner, as the mark of the record ran: that extra was valid in each of them.
    if (is_class_key(word)) {
        return scan_class_head(scanner, &reader);
    }
    // A macro in the head of a class that has no member. The parser takes the mark after a class key
    // only, and the validity of the token is the evidence of that position.
    //
    // THIS SCAN READS THE CLASS HEAD, AND THE LEXER CANNOT GO BACK. The scan gives no token for a
    // class that has a member, which is the common form. The reader is then in the body of the
    // class. Each scan after this one reads the text at the name. For this reason the word scan
    // stops where this scan moved the reader. Without the stop, the scan of `MACRO_SCOPE_START`
    // reads the first group of the body as the arguments of the name. The input
    // `class A("x") B { decltype(c)::type d; };` then gave `A` a `macro_scope_specifier` and a
    // MISSING `::`. The budget of the reader counts one for each character, so an equal budget is
    // the evidence that the scan read no character.
    if (valid_symbols[CLASS_MACRO_MARK]) {
        uint32_t start_budget = reader.budget;
        if (scan_class_macro_mark(&reader, word, has_lower)) {
            lexer->result_symbol = CLASS_MACRO_MARK;
            return true;
        }
        if (reader.budget != start_budget) {
            return false;
        }
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
    // THE FLAGS OF THE SCANS AFTER THE SCAN OF A DECLARED TYPE, COMPUTED BEFORE IT. Each one reads
    // the word, the character after it, and the records, and none reads a character. The scan of a
    // declared type hands the reader to the scan of a comparison, so it must know the flags first.
    bool cast_name = valid_symbols[FUNCTIONAL_CAST_NAME] && !valid_symbols[TYPE_TRAIT_TYPE_MARKER] &&
                     !valid_symbols[MACRO_TYPE_START] && !valid_symbols[PARAMETER_MACRO_TYPE_START] &&
                     !is_grammar_keyword(word) &&
                     (is_class_name(scanner, reader.word_hash) || is_loose_name(scanner, reader.word_hash) ||
                      is_seed_type_name(scanner, full, &reader)) &&
                     // A LOCAL HIDES THE CLASS AT A FUNCTIONAL CAST. `int A = 0;` and then `A(1)` is a
                     // call of the variable, which Clang 22 and GCC 16 accept for a callable local and
                     // reject for the class. `eval(program)` after `ast_eval eval;` in the calculator
                     // examples of boost read as a cast. The test comes last, because the record scan
                     // then runs only where a source of the parse already called the name a type.
                     // Refer to task 389.
                     !is_local_name(scanner, reader.word_hash);
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
    // THE TYPE OF A DECLARATION THAT A SOURCE OF THE PARSE DECLARES.
    //
    // `T HPX_RESTRICT dest` reads today as the attribute macro `T` with the type `HPX_RESTRICT`. The
    // two names are swapped: a name after `typename` or `class` in the head of the same declaration
    // IS a type, by the grammar, so it is the type and the bare name after it is the macro. The
    // token makes the first name the type and the grammar then reads the second as the attribute
    // macro. Refer to task 276.
    //
    // A MACRO-SHAPED SECOND NAME IS REQUIRED. Without it the token would fire at every use of a type
    // parameter as a type, which is most of the text of a template, and the rule it licenses needs a
    // second name to exist at all. The shape of the second name is the same test that every other
    // macro rule of this file uses, so a name that no rule would call a macro does not become one
    // here either.
    // A BLANK MUST FOLLOW THE NAME, AND THAT TEST COMES BEFORE THE SCAN READS A CHARACTER.
    //
    // A BRANCH THAT ADVANCES THE LEXER AND THEN DECLINES STOPS EVERY LATER BRANCH OF THIS FUNCTION.
    // `P(CONFIG)` with `P` in the record entered this branch, found a `(` where it wanted a name,
    // and returned false, and `scan_macro_start` below never ran. The macro invocation then had no
    // token and the statement became an ERROR. Two names with a macro between them always have a
    // blank there, so the test costs nothing and it keeps the branch out of every shape that another
    // branch answers.
    //
    // THREE SOURCES SAY THAT THE FIRST NAME IS A TYPE, AND TWO OF THEM ENTER ON A DIFFERENT PATH.
    // The template head of the same declaration is the first source, and the branch after this one
    // is its path. The class heads of the file and the seed of the project are the two other
    // sources, and `is_declared_type_name` reads them. A name of those two sources takes the path of
    // `scan_declared_type_declarator`, which reads the blanks after the name with the scan of a
    // comparison, so that a `<` after the blanks keeps the token that it has today.
    //
    // A FOURTH SOURCE SAYS THAT THE THIRD NAME IS A MACRO: THE MACRO ROWS OF THE SEED. A name that no
    // source declares as a type enters the same path when the parse has a seed, and the macro must
    // then have a macro row. `Mutex mu MOZ_UNANNOTATED;` of firefox needs it, because a template
    // parameter named `Mutex` removes that name from the type rows. Over the 652 sites of this
    // construct at 6e1adae, the macro rows repair 316 sites in 117 files that no type source reaches.
    //
    // A FIRST NAME WITH A MACRO ROW DOES NOT ENTER ON THAT SOURCE, BECAUSE A DECLINE AFTER THE BLANK
    // STOPS THE SCAN OF AN INVOCATION. The scan of a comparison marks the end of the token after the
    // first name, and each token of `scan_macro_start` is empty and ends before the name, so no
    // branch can give that token after the scan read the blank. With no such test, 46 files of the
    // corpus lost a macro and 10 of them got an ERROR node: `ClassDef (TFileInfo, 1)` in a class
    // body of ROOT, `P (TYPE_ALLOC)` in a block of bde, `simdjson_inline simdjson_result(...)` before
    // a constructor of simdjson. Each of those names has a macro row. A build that calls
    // `scan_macro_start` after that decline gave `ClassDef (TFileInfo, 1)` 2 ERROR nodes, and the
    // bde and ROOT files twice the ERROR nodes of the decline. The test costs 13 sites of `hb_locale_t oldlocale HB_UNUSED;`, because harfbuzz
    // defines `hb_locale_t` as a macro of a type and declares no typedef.
    //
    // THE TEST CANNOT SEE A MACRO WITH NO ROW, AND THE DECLINE STILL STOPS ITS INVOCATION. A macro of
    // another project or of a system header gives no row. Such a name before `(` with a blank, where
    // a member, a constructor or an item starts, loses its macro when the parse has a seed. The corpus
    // holds no such site under the seeds of 2026-09-17. A rewind of the lexer, or a type token that is
    // an empty mark before the name, removes the loss for every name.
    //
    // EACH TEST OF THIS CONDITION NAMES AN INPUT THAT IT DECIDES.
    // A MACRO-SHAPED NAME DOES NOT ENTER. `class FOO { int x; };` and then `FOO (y)` on a line of
    // its own is a macro invocation with the name of the class, and the scan would read the blank
    // and the `(` and give the line an ERROR node. The price is `FOO bar BAZ;`, which keeps the
    // reading with the macro first for a class name in capitals.
    // A KEYWORD OF THE GRAMMAR DOES NOT ENTER. The seed of ClickHouse declares `size_t` as a type,
    // and `size_t max_speed TSA_GUARDED_BY(mutex){0};` already reads correctly with the type
    // `primitive_type`. The token would make that node a `type_identifier`, in 18 files of the
    // corpus under the seeds of 2026-09-16.
    // A TEMPLATE PARAMETER LIST DOES NOT ENTER. `hb_enable_if (P == 1)` in a template head of
    // harfbuzz is a macro call that gives a whole template parameter, and the scan of that call
    // runs after this branch. No site of the population is in a template parameter list.
    bool type_parameter = is_template_parameter(scanner, reader.word_hash);
    if (valid_symbols[TEMPLATE_PARAMETER_TYPE_NAME] && (next == ' ' || next == '\t' || next == '\n')
        && !type_parameter && !macro_shaped && !is_grammar_keyword(word) &&
        !valid_symbols[TEMPLATE_PARAMETER_MACRO_START]) {
        bool declared = is_declared_type_name(scanner, full, &reader);
        bool macro_evidence = !declared && has_seed(scanner) && !is_seed_macro_name(scanner, full, &reader);
        if (declared || macro_evidence) {
            return scan_declared_type_declarator(&reader, word, comparison, cast_name, !declared, valid_symbols);
        }
    }
    if (valid_symbols[TEMPLATE_PARAMETER_TYPE_NAME] && (next == ' ' || next == '\t' || next == '\n')
        && type_parameter) {
        // The end of the token is the end of the first name. The scan reads past it and the mark
        // holds.
        mark_end(lexer);
        Gap after_type = {0};
        skip_gap(&reader, &after_type);
        if (after_type.blocked || !readable(&reader) || !is_word_start(lexer->lookahead)) {
            return false;
        }
        char second[MACRO_WORD_SIZE];
        bool second_has_lower = false;
        read_word(&reader, second, &second_has_lower);
        if (strlen(second) < 2 || !is_macro_name(second, second_has_lower) || is_grammar_keyword(second)) {
            // THE SECOND NAME IS NOT A MACRO, SO IT IS THE DECLARATOR, AND A MACRO CAN FOLLOW IT.
            // `void f(T value ABSL_ATTRIBUTE_LIFETIME_BOUND)` reads today as the attribute macro
            // `T`, the type `value`, and the declarator `ABSL_ATTRIBUTE_LIFETIME_BOUND`. All three
            // fields are wrong. A different token makes `T` the type here. Refer to
            // `TEMPLATE_PARAMETER_DECLARATOR_TYPE` and to task 276.
            //
            // THIS PATH ADDS NO DECLINE TO `scan_word_start`. The branch above already decides
            // before it reads a character, and the code here runs only where the branch gave `false`
            // after it read the two names. A scan that reads more and then gives `false` costs
            // nothing, because `scan_token` gives `false` and the runtime resets the lexer. The
            // measurement of 2026-09-16 proves it: a build that reads exactly this text on exactly
            // this path gives 0 changed tree hashes over the 329,387 files of the corpus.
            return scan_template_parameter_declarator(&reader, second, false, valid_symbols);
        }
        // A DECLARATOR MUST FOLLOW THE MACRO, AND THAT IS WHAT SEPARATES A MACRO FROM A NAME. An
        // ALL-CAPS name is macro-shaped whether it is a macro or an object, and `const TYPE TYPE_MAX
        // = Limits::max();` names an object. Without this test the scan took the object name as the
        // macro and the declaration lost its declarator: 330 NEW ERRORS in the corpus, all of the
        // shape `T NAME = ...` or `T NAME[n];`. A macro before a declarator has a third name after
        // it, or a `*` or a `&` that starts one.
        Gap after_macro = {0};
        skip_gap(&reader, &after_macro);
        if (after_macro.blocked || !readable(&reader)) {
            return false;
        }
        // An argument list of the macro comes between it and the declarator, so the scan goes past
        // a balanced group before it looks for the declarator. `T TSA_GUARDED_BY(m) dest` has one
        // and `T MIN(a);` does not, because a `;` follows the group there and no declarator does.
        if (lexer->lookahead == '(') {
            Arguments arguments = {0};
            if (!skip_group(&reader, &arguments) || reader.budget == 0) {
                return false;
            }
            Gap after_arguments = {0};
            skip_gap(&reader, &after_arguments);
            if (after_arguments.blocked || !readable(&reader)) {
                return false;
            }
        }
        int32_t after = lexer->lookahead;
        if (!is_word_start(after) && after != '*' && after != '&') {
            return false;
        }
        // THE NAME AFTER THE MACRO MUST BE A DECLARATOR AND NOT A SPECIFIER. `template <typename
        // INT> constexpr INT EXPONENT() const {` is a member FUNCTION whose name is all capitals,
        // and a `const` follows its parameter list. Taking `EXPONENT` as a macro there loses the
        // function: one of the 4 NEW ERRORS of the first gate of this rule.
        if (is_word_start(after)) {
            char third[MACRO_WORD_SIZE];
            bool third_has_lower = false;
            read_word(&reader, third, &third_has_lower);
            if (third[0] == '\0' || is_grammar_keyword(third)) {
                return false;
            }
            // A `(` AFTER THE DECLARATOR IS [dcl.ambig.res], AND THIS TOKEN DECIDES IT WRONGLY.
            // `T Q_DECL_UNINITIALIZED handler(data, op);` of qtbase declares a VARIABLE. GCC gives
            // `var_decl` and Clang gives `VarDecl ... callinit` with a `CXXConstructExpr`. The
            // declaration that this token opens reads the group as a parameter list, and the
            // reading without the token, an `init_declarator` with an `argument_list`, is the
            // correct one. The token repairs 203 sites and NONE of them has a declarator of this
            // shape, so the test costs no repair.
            //
            // THE TEST DECLINES ONLY WHERE IT SEES THE `(`. A gap that a directive blocks hides the
            // character, and a reader with no budget cannot look. The reading of those is the
            // reading of the token, which is what this scan gave before the test.
            Gap after_declarator = {0};
            skip_gap(&reader, &after_declarator);
            if (!after_declarator.blocked && readable(&reader) && lexer->lookahead == '(') {
                return false;
            }
        }
        lexer->result_symbol = TEMPLATE_PARAMETER_TYPE_NAME;
        return true;
    }
    // The operand of `alignas` is a type or a constant expression, and the grammar reads a bare name
    // as an expression. The token gives the type reading for a name that a source declares as a
    // type. The parser makes the token valid in that one position, so the validity is the evidence
    // of the position.
    if (valid_symbols[ALIGNAS_TYPE_NAME] && is_recorded_type_name(scanner, full, &reader)) {
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
    if (cast_name && next == '(') {
        mark_end(lexer);
        lexer->result_symbol = FUNCTIONAL_CAST_NAME;
        return true;
    }
    // A template-id of a recorded class before a `(` is a functional cast: `B<int>(x)`. One scan gives
    // the two answers, because a scan that declines cannot go back to the start of the brackets.
    if (comparison || (cast_name && (next == '<' || blank))) {
        return scan_comparison_name(&reader, word, comparison, cast_name, NULL);
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
    return macro && scan_macro_start(reader, word, full, has_lower, valid_symbols, scanner);
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
///   name.
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
        case MACRO_CALL_ATTRIBUTE_START:
        case MACRO_CALL_ATTRIBUTE_TOKENS_START:
        case ATTRIBUTE_TOKENS_MARKER:
        case MACRO_TYPE_START:
        case PARAMETER_MACRO_TYPE_START:
        case STATEMENT_ATTRIBUTE_MACRO_START:
        case STATEMENT_ATTRIBUTE_MACRO_TOKENS_START:
        case CONSTRUCTOR_MACRO_START:
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
        case TEMPLATE_PARAMETER_MACRO_START:
        case MACRO_ITEM_START:
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
                    if (!valid_symbols[PREPROC_LINE_END] || !scan_line_end(lexer)) {
                        return false;
                    }
                    scan_local_after_directive(scanner, lexer);
                    return true;
            }
        }
        if (valid_symbols[PREPROC_LINE_END]) {
            if (!scan_line_end(lexer)) {
                return false;
            }
            // The item after the line. Refer to `scan_local_after_directive`.
            scan_local_after_directive(scanner, lexer);
            return true;
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
        if (!directive || !scan_directive(scanner, lexer, valid_symbols, space)) {
            return false;
        }
        // An `#endif` or an `#else` of a structured group has no line end. The item after its line comes
        // after the directive token. Refer to `scan_local_after_directive`.
        if (lexer->result_symbol == PREPROC_ENDIF || lexer->result_symbol == PREPROC_ELSE) {
            scan_local_after_directive(scanner, lexer);
        }
        return true;
    }
    // A boundary of the record of locals. At `{`, `}` and `;`, each path below gives no token and reads
    // no character, and this scan gives no token either, so no token of the parser changes. It runs in
    // error recovery too, so that a version in recovery keeps the frames of the other versions. It reads
    // no `valid_symbols`, so each version at a boundary writes the same record.
    if (lexer->lookahead == '{' || lexer->lookahead == '}' || lexer->lookahead == ';') {
        return scan_local_boundary(scanner, lexer);
    }
    // In error recovery, the scan of a class head is the only scan of this scanner, and it gives no token.
    // A version in recovery then writes the record of class heads at the same class key as the other
    // versions.
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
    //
    // THE SCAN READS THE NAME AND CAN THEN GIVE NO TOKEN, AND THAT COSTS NOTHING HERE, BY TWO FACTS.
    // The four parse states that hold the token hold no macro token, and the tokens that they hold
    // want a `[`, a `#`, or a keyword, which the refused name is not (`ts_external_scanner_states`
    // of src/parser.c). A build where this branch yields to the scans after it changed no tree over
    // 329,387 files on 2026-09-16. Refer to `stop_sites` of xtask/src/scanner.rs.
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
                                        valid_symbols[DECLARATOR_NAME_MACRO_NAME],
                                        valid_symbols[DECLARATOR_OBJECT_MACRO_NAME]);
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
    // The record of locals takes 8 bytes and 6 bytes for each entry.
    static_assert(1 + MAX_DELIMITER_LENGTH * sizeof(wchar_t) + 1 + MAX_GROUPS + 2 + 1 + 1 +
                          2 + 4 * MAX_CLASSES * sizeof(uint32_t) + 8 + 6 * MAX_LOCALS <
                      TREE_SITTER_SERIALIZATION_BUFFER_SIZE,
                  "Serialized state is too long!");

    Scanner *scanner = (Scanner *)payload;
    const LocalRecord *locals = &scanner->locals;
    bool no_locals = locals->count == 0 && locals->frames == 0 && locals->saturated == 0 && locals->class_bodies == 0 &&
                     locals->class_head == 0;
    if (scanner->delimiter_length == 0 && scanner->group_count == 0 && scanner->class_count == 0 &&
        scanner->loose_count == 0 && scanner->template_count == 0 && scanner->alias_count == 0 && !scanner->preproc_extra_tokens &&
        no_locals) {
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
    buffer[size++] = (char)scanner->alias_count;
    memcpy(&buffer[size], scanner->aliases, scanner->alias_count * sizeof(uint32_t));
    size += scanner->alias_count * sizeof(uint32_t);
    // The record of locals carries its own count too. Each entry is its hash, its frame and its count of
    // pending boundaries.
    unsigned entry_count = local_entry_count(locals);
    buffer[size++] = (char)entry_count;
    buffer[size++] = (char)locals->frames;
    buffer[size++] = (char)locals->saturated;
    memcpy(&buffer[size], &locals->class_bodies, sizeof(uint32_t));
    size += sizeof(uint32_t);
    buffer[size++] = (char)locals->class_head;
    for (unsigned i = 0; i < entry_count; i++) {
        memcpy(&buffer[size], &locals->entries[i].name, sizeof(uint32_t));
        size += sizeof(uint32_t);
        buffer[size++] = (char)locals->entries[i].frame;
        buffer[size++] = (char)locals->entries[i].pending;
    }
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
    scanner->alias_count = 0;
    scanner->preproc_extra_tokens = false;
    scanner->locals.count = 0;
    scanner->locals.frames = 0;
    scanner->locals.saturated = 0;
    scanner->locals.class_bodies = 0;
    scanner->locals.class_head = 0;
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
        assert(size < length && "Can't decode the count of the alias record!");
        scanner->alias_count = (uint8_t)buffer[size++];
        assert(scanner->alias_count <= MAX_CLASSES && "Can't decode the names of the alias record!");
        memcpy(scanner->aliases, &buffer[size], scanner->alias_count * sizeof(uint32_t));
        size += scanner->alias_count * sizeof(uint32_t);
        // The record of locals fails closed. A header or entries that do not fit in the state, or more
        // entries than MAX_LOCALS, give an empty record and no class names, because the class names come
        // after the entries. Refer to "EACH RANGE OF THE RECORD FAILS CLOSED".
        if (size + 8 > length) {
            return;
        }
        unsigned entry_count = (uint8_t)buffer[size];
        if (entry_count > MAX_LOCALS || size + 8 + 6 * entry_count > length) {
            return;
        }
        LocalRecord *locals = &scanner->locals;
        locals->count = (uint8_t)buffer[size++];
        locals->frames = (uint8_t)buffer[size++];
        locals->saturated = (uint8_t)buffer[size++];
        memcpy(&locals->class_bodies, &buffer[size], sizeof(uint32_t));
        size += sizeof(uint32_t);
        locals->class_head = (uint8_t)buffer[size++];
        for (unsigned i = 0; i < entry_count; i++) {
            memcpy(&locals->entries[i].name, &buffer[size], sizeof(uint32_t));
            size += sizeof(uint32_t);
            locals->entries[i].frame = (uint8_t)buffer[size++];
            locals->entries[i].pending = (uint8_t)buffer[size++];
        }
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

/// Give the version of the seed struct that this scanner reads, `TS_CPP_SEED_VERSION` of src/seed.h.
///
/// THE SCANNER CANNOT REFUSE A SEED BY ITSELF, SO A CALLER ASKS BEFORE IT GIVES ONE. The runtime gives
/// the context through `tree_sitter_cpp_external_scanner_set_context`, which returns nothing, and
/// `seed_kinds` reads a seed of a different magic or version as no name. The parse then gives the tree
/// of a parse with no seed and no message. A caller that builds a seed of a different version must
/// stop with an error, and `Seed::read` of xtask/src/seed.rs does. A tool that links two parsers of
/// two trees asks each parser, because each one reads the version of its own header.
uint32_t tree_sitter_cpp_seed_version(void) {
    return TS_CPP_SEED_VERSION;
}

void tree_sitter_cpp_external_scanner_destroy(void *payload) {
    Scanner *scanner = (Scanner *)payload;
    ts_free(scanner);
}
