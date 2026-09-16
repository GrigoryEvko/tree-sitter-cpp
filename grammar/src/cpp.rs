//! The C++ grammar.
//!
//! The grammar started as a port of `grammar.js` of tree-sitter-cpp at commit 8b5b49e, and it
//! extends the C grammar. A rule of the C grammar keeps its position, and a new rule goes to
//! the end.

use crate::c::{self, close_bracket, close_brace, open_brace, preproc_if};
use crate::dsl::*;
use crate::{choice, s, seq};

/// The precedences of the C++ grammar.
pub mod precedence {
    pub use crate::c::precedence::*;
    pub const LAMBDA: i32 = SUBSCRIPT + 1;
    pub const NEW: i32 = CALL + 1;
    pub const THREE_WAY: i32 = RELATIONAL + 1;
    /// The precedence of `.*` and `->*`. Each operand of these operators is a cast expression, and each
    /// operand of a multiplicative operator is a pm-expression ([expr.mptr.oper], [expr.mul]). GCC
    /// `PREC_PM_EXPRESSION` and Clang `prec::PointerToMember` are the same level.
    pub const POINTER_TO_MEMBER: i32 = MULTIPLY + 1;
    const _: () = assert!(MULTIPLY < POINTER_TO_MEMBER && POINTER_TO_MEMBER < CAST);
    /// The precedence of the `...` of a pack expansion. The pattern of an expansion in an expression
    /// list is a full initializer-clause, and `f(a = b...)` expands `a = b` (GCC
    /// `cp_parser_parenthesized_expression_list_elt`, Clang `ParseExpressionList`).
    pub const PACK_EXPANSION: i32 = ASSIGNMENT - 1;
}

use precedence::*;

/// The operators of a fold expression.
const FOLD_OPERATORS: [&str; 38] = [
    "+", "-", "*", "/", "%", "^", "&", "|", "=", "<", ">", "<<", ">>", "+=", "-=", "*=", "/=", "%=", "^=", "&=", "|=",
    ">>=", "<<=", "==", "!=", "<=", ">=", "&&", "||", ",", ".*", "->*", "or", "and", "bitor", "xor", "bitand",
    "not_eq",
];

/// The alternative tokens of the binary operators, with the precedences of `&&`, `||`, `|`, `^`,
/// `&`, and `!=`. libcpp gives them the token types of their operators in `_cpp_lex_direct`, also
/// in a directive.
const ALTERNATIVE_BINARY_OPERATORS: [(&str, i32); 6] = [
    ("or", LOGICAL_OR),
    ("and", LOGICAL_AND),
    ("bitor", INCLUSIVE_OR),
    ("xor", EXCLUSIVE_OR),
    ("bitand", BITWISE_AND),
    ("not_eq", EQUAL),
];

/// The binary expressions of the alternative tokens, with the operands `operand`.
fn alternative_binary_operations(operand: &str) -> impl Iterator<Item = Rule> + '_ {
    ALTERNATIVE_BINARY_OPERATORS
        .into_iter()
        .map(move |(operator, precedence)| {
            prec_left(
                precedence,
                seq![
                    field("left", sym(operand)),
                    field("operator", operator),
                    field("right", sym(operand)),
                ],
            )
        })
}

/// The assignment operators.
const ASSIGNMENT_OPERATORS: [&str; 14] = [
    "=", "*=", "/=", "%=", "+=", "-=", "<<=", ">>=", "&=", "^=", "|=", "and_eq", "or_eq", "xor_eq",
];

/// The C++ grammar.
pub fn grammar() -> Grammar {
    let mut g = Grammar::extend_from(&c::grammar(), "cpp");
    // The order of the external tokens is the order of `enum TokenType` in `src/scanner.c`.
    g.externals = vec![
        s!(raw_string_delimiter),
        s!(raw_string_content),
        s!(_decay_copy_auto),
        s!(_macro_line_start),
        s!(_macro_block_start),
        s!(_macro_call_start),
        s!(_macro_enumerator_start),
        s!(_trailing_macro_name),
        s!(_call_macro_name),
        s!(_qt_emit_marker),
        s!(_qt_foreach_marker),
        s!(_type_trait_marker),
        s!(ms_asm_code),
        s!(_va_arg_marker),
        s!(_member_pointer_start),
        s!(_preproc_line_end),
        s!(_preproc_directive),
        s!(_preproc_define),
        s!(_preproc_include),
        s!(_preproc_if),
        s!(_preproc_ifdef),
        s!(_preproc_ifndef),
        s!(_preproc_elif),
        s!(_preproc_elifdef),
        s!(_preproc_elifndef),
        s!(_preproc_else),
        s!(_preproc_endif),
        s!(_preproc_conditional),
        s!(preproc_skipped),
        s!(_preproc_literal_marker),
        s!(_preproc_embed),
        s!(_structured_binding_auto),
        s!(_preproc_final_directive),
        s!(_preproc_final_define),
        s!(_preproc_final_include),
        s!(_preproc_if_in_case),
        s!(_preproc_ifdef_in_case),
        s!(_preproc_ifndef_in_case),
        s!(_class_head_mark),
        s!(_constructor_macro_start),
        s!(_unreachable_token),
        s!(_macro_call_attribute_start),
        s!(_macro_call_attribute_tokens_start),
        s!(_comparison_name),
        s!(_initializer_list_marker),
        s!(_grouping_call_macro_name),
        s!(_cast_paren),
        s!(_name_expression_paren),
        s!(_operand_type_paren),
        s!(_declarator_macro_name),
        s!(_enumerator_macro_name),
        s!(_macro_statement_start),
        s!(_pack_index_ellipsis),
        s!(_macro_type_start),
        s!(_parameter_macro_type_start),
        // The `-` and the `+` of the grammar. The scanner gives one of them before a number that has a
        // ud-suffix, and the sign of that number is then an operator: `-1_k`. Refer to
        // `scan_sign_before_suffixed_number` in `src/scanner.c`. An external token that has the same text
        // as a token of the grammar adds no parse action: the scanner reads it in each parse state that
        // takes that token, and the lexer reads it where the scanner gives none.
        Rule::from("-"),
        Rule::from("+"),
        s!(_alignof_type_paren),
        s!(_splice_open),
        s!(_attribute_open_bracket),
        s!(_attribute_close_bracket),
        s!(_statement_attribute_macro_start),
        s!(_pointer_call_macro_name),
        s!(_declarator_name_macro_name),
        s!(_type_trait_type_marker),
        // The `[` of the grammar. The scanner gives it for the digraph `<:` ([lex.digraph]). Only the
        // scanner can read the character after `<::`, which keeps the two tokens `<` and `::`. Refer to
        // `scan_less_than_digraph` in `src/scanner.c`. The lexer reads the other digraphs, which have no
        // such exception. Refer to `open_brace` in `grammar/src/c.rs`.
        Rule::from("["),
        s!(_attribute_tokens_marker),
        // The text of a directive line. The scanner reads a literal and a comment as the preprocessor
        // reads them, and a pattern of the lexer cannot. Refer to `scan_preproc_arg` in src/scanner.c.
        s!(preproc_arg),
        // A mark before the `(` of the parameter list of a function-like macro. The scanner gives no such
        // token, and it reads the mark in `valid_symbols` only: a `(` that comes immediately after the name
        // of a macro opens a parameter list, and it is no text of the line.
        s!(_preproc_params_mark),
        s!(_functional_cast_name),
        // An empty token before the name of a macro invocation line that a storage class specifier comes
        // before: `static Q_LOGGING_CATEGORY(log, "qtc", QtWarningMsg)`. The scanner gives it only for a
        // name with arguments, because a bare name there continues the declaration on the next line.
        // `_macro_line_start` keeps its meaning of a line with no specifier before it.
        s!(_macro_line_after_specifiers),
        // An empty mark before the extra tokens of an `#endif` or an `#else` line. The scanner gives
        // the mark only when the rest of that line holds a token, so a line with no such token takes
        // no line end, and the node of the group keeps its range.
        s!(_preproc_extra_mark),
    ];
    // The conflict sets of the grammar. Each set names the rules of one ambiguity, and it tells the
    // generator to keep each reading in a GLR split.
    //
    // DECLARE ONLY THE SETS THAT THE PARSE TABLE BUILDER USES. A set with no use stops the report of
    // a later conflict of the same rules. The generator then makes a GLR split with no word, the
    // parser selects one reading by the symbol order, and a silent misparse follows. With the set
    // removed, the generator stops with an error and names the rules. `cargo xtask generate` fails
    // when the generator reports a set with no use, and the count of such sets cannot grow again.
    g.conflicts = conflict_sets(&[
        // C
        &["type_specifier", "_declarator_of_name"],
        &["type_specifier", "expression"],
        &["sized_type_specifier"],
        &["attributed_statement"],
        &["_declaration_modifiers", "using_declaration"],
        &["_declaration_modifiers", "attributed_statement", "using_declaration"],
        // The GNU attributes have the same forks as the standard attributes: a declaration or a
        // statement after them, and a nested attributed statement.
        &["_gnu_attributed_statement"],
        &["_declaration_modifiers", "_gnu_attributed_statement"],
        &["_top_level_item", "_top_level_statement"],
        &["_block_item", "statement"],
        // The body of a namespace, of a linkage specification, and of an export declaration holds the
        // items of a block. Refer to `_declaration_list_item`.
        &["_declaration_list_item", "statement"],
        // A GNU asm label after a declarator belongs to a declaration with no initializer, or to an
        // init declarator. Only the token after the label decides. Refer to `c::asm_label`.
        &["_block_declaration", "_declarator_of_function"],
        // A GNU attribute before `extern` belongs to the specifiers of a declaration, or to a
        // linkage specification. The token after `extern` decides. In a block, the attributes of a
        // statement have the same start.
        &["_declaration_modifiers", "_linkage_attribute"],
        &["_declaration_modifiers", "_linkage_attribute", "_gnu_attributed_statement"],
        // After a label, the attributes belong to the label, to a declaration, or to a linkage
        // specification.
        &[
            "_declaration_modifiers",
            "labeled_statement",
            "_linkage_attribute",
            "_gnu_attributed_statement",
        ],
        // `__extension__` starts a declaration or an expression. Each declaration takes the same
        // prefix rule, and the token after the prefix decides. Refer to `c::extension_prefix`.
        &["_extension_specifier", "extension_expression"],
        // C++
        &["template_function", "template_type"],
        &["template_function", "template_type", "expression"],
        &["template_function", "template_type", "qualified_identifier"],
        &[
            "template_function",
            "template_type",
            "qualified_identifier",
            "qualified_type_identifier",
        ],
        &["template_type", "qualified_type_identifier"],
        &["qualified_type_identifier", "qualified_identifier"],
        // After `requires N::`, a `<` starts the template arguments of the name, or a comparison
        // with the name as its left operand. The name of a constraint has the same ambiguity as
        // each other qualified name. Refer to `_constraint_qualified_identifier`.
        &["template_function", "template_type", "_constraint_qualified_identifier"],
        // After `requires Cs...[0]<T>`, a `::` makes the pack index the scope of a qualified name,
        // and each other token ends the constraint. Refer to `_constraint_pack_index_template`.
        &["template_type", "_constraint_pack_index_template"],
        // At namespace scope, `asm("nop");` is an asm-declaration. The grammar keeps the expression
        // statement of the same text for macro code, and that statement has the dynamic precedence
        // `NAMESPACE_EXPRESSION`. Refer to `asm_declaration`.
        &["expression", "asm_declaration"],
        &["_top_level_expression_statement", "asm_declaration"],
        &["comma_expression", "initializer_list"],
        // In the arguments `f({ {} x; })`, the inner `{}` is a block of a GNU statement expression. In
        // `f({ {} })`, it can also be a braced list in a braced list. Only the tokens after the inner `}`
        // tell them apart.
        &["compound_statement", "_nested_initializer_list"],
        &["expression", "_declarator_of_name"],
        // `T* p(x);` declares a function with a parameter of type `x`, or it declares `p` with
        // the initializer `x`. Only the tokens in the parentheses tell them apart.
        &["_declarator_of_name", "_pointer_name_declarator"],
        &["_declarator_of_name", "_reference_name_declarator"],
        // In a block, `T &operator>>(S &in, T &t);` has the declarator of a reference to an operator
        // function, and the declarator of the other classes. Refer to `REFERENCE_FUNCTION_IN_BLOCK`.
        &["_declarator_of_name", "_block_operator_function_declarator"],
        &["_declarator_of_name", "_rvalue_reference_name_declarator"],
        &["_declarator_of_name", "_member_pointer_name_declarator"],
        &["_declarator_of_name", "_nested_pointer_name_declarator"],
        &["_declarator_of_name", "_nested_reference_name_declarator"],
        &["_declarator_of_name", "_nested_rvalue_reference_name_declarator"],
        &["_declarator_of_name", "_nested_member_pointer_name_declarator"],
        &["expression", "_declarator_of_name", "type_specifier"],
        &["expression", "identifier_parameter_pack_expansion"],
        &["expression", "_lambda_capture_identifier"],
        // `{ [x(1)] = 2 }` has a GNU array designator, and `{ [x(1)] {} }` has a lambda with an
        // init-capture. Only the token after `]` tells them apart.
        &["expression", "lambda_capture_initializer"],
        &["expression", "_lambda_capture"],
        // In a requirement body, `requires (` starts the constraint of a nested requirement,
        // `requires (int)sizeof(T) > 1;`, or the parameters of a requires expression,
        // `requires (T t) { t++; };`. Only the token after `)` tells them apart (Clang
        // `ParseRequiresExpression`).
        &["_declaration_modifiers", "type_descriptor"],
        &["_parameter_declaration_specifiers", "type_descriptor"],
        &["parameter_list", "argument_list"],
        &["type_specifier", "call_expression"],
        // A type that is a keyword has a second reading in declaration specifiers. Refer to
        // `KEYWORD_TYPE`. After the type, `(` starts a declarator or a functional cast.
        &["_declaration_specifiers", "type_specifier"],
        &["_declaration_specifiers", "type_specifier", "call_expression"],
        // After `operator`, the type belongs to the declarator of a conversion function, or to the
        // name of one in an expression. The two sets above have the same ambiguity for a declaration.
        &["_conversion_declaration_specifiers", "_nondefining_type_specifier"],
        // In `operator A B()`, `A` is the type or a macro before the type. The sets of
        // `type_specifier` and `attribute_macro` below have the same ambiguity for a declaration.
        &["_nondefining_type_specifier", "attribute_macro"],
        &["_binary_fold_operator", "_fold_operator"],
        &["_function_declarator_seq"],
        // The function declarator of a deduction guide has the parts of `_function_declarator_seq`, and the
        // same ambiguities after its parameter list.
        &["_deduction_guide_declarator", "_function_declarator_seq"],
        // After the `->` of a guide and a template-id, the return type of the guide is complete. The
        // trailing return type of a function declarator takes more qualifiers and a declarator. The
        // token after the template-id tells the two apart.
        &["_nondefining_type_specifier", "_deduction_guide_class"],
        &["type_specifier", "sized_type_specifier"],
        &["initializer_pair", "comma_expression"],
        &["expression_statement", "_for_statement_body"],
        &["init_statement", "_for_statement_body"],
        &["field_expression", "template_method", "template_type"],
        &["field_expression", "template_method"],
        &["qualified_field_identifier", "template_method", "template_type"],
        &["type_specifier", "template_type", "template_function", "expression"],
        &["splice_type_specifier", "splice_expression"],
        // Where a type can end before a less-than operator, as after `^^` or `new`, the `<` after
        // a splice starts template arguments or a comparison: `^^typename [:r:]<int>::type`,
        // `^^[:r:] < 1`. After `^^`, the splice can also be an expression. Only splice rules have
        // these sets.
        &["_splice_specialization_specifier", "splice_type_specifier"],
        &[
            "_splice_specialization_specifier",
            "splice_type_specifier",
            "splice_expression",
        ],
        // `A b` is a type and a declarator, or a macro and a type. Only the token after them
        // tells them apart, as in `A b;` and `A b c();`.
        &["type_specifier", "attribute_macro"],
        &["_declarator_of_name", "type_specifier", "attribute_macro"],
        &["_declarator_of_name", "attribute_macro"],
        // After a type, `x alignas(16);` declares `x` with an alignment. Where a type can also
        // start, `x` can be the type or an attribute macro. The tokens after `alignas(...)` tell
        // the readings apart.
        &["type_specifier", "attribute_macro", "_aligned_name_declarator"],
        &["attribute_macro", "_aligned_name_declarator"],
        &["sized_type_specifier", "attribute_macro"],
        // A `[:` after a name opens the index of a subscript, `a[:i:]`, or the type of a declaration
        // after a macro, `MACRO [:r:] x;`. The tokens after the splice tell the readings apart.
        &["type_specifier", "expression", "attribute_macro"],
        &["expression", "attribute_macro"],
        // In a typedef, `typedef A signed` has `A` as a macro before a sized type, as a name in a
        // sized type, or as the type. The tokens after the size keywords tell the readings apart.
        &["type_specifier", "sized_type_specifier", "attribute_macro"],
        // `struct A b` is the class `A` and a declarator, or a macro and the class `b`. Only the
        // token after `b` tells them apart, as in `struct stat st;` and `class LLVM_ABI A {`.
        &["_class_name", "attribute_macro"],
        // A name before a brace is the type of a functional cast, or an expression that the brace
        // does not continue: in `int i : N {1};` the width is `N` and `{1}` is the initializer of
        // the data member. Only name lookup tells a type name from a value name. Refer to
        // `_bitfield_width_before_initializer`.
        &["expression", "_class_name"],
        &["type_specifier", "compound_literal_expression"],
        &["type_specifier", "expression", "_class_name"],
        &["type_specifier", "_class_name"],
        &["type_specifier", "compound_literal_expression", "_class_name"],
        // A macro before a declaration, a constructor, or a conversion function. The token
        // after the specifiers decides, as for the conflicts of `_constructor_specifiers`.
        &[
            "operator_cast_definition",
            "operator_cast_declaration",
            "constructor_or_destructor_definition",
        ],
        // At namespace scope, a deduction guide starts with the specifiers of a constructor. This set
        // is the set above with `_deduction_guide_declaration`, for the same ambiguity.
        &[
            "operator_cast_definition",
            "operator_cast_declaration",
            "constructor_or_destructor_definition",
            "_deduction_guide_declaration",
        ],
        // The generator names a shared repeat after the first rule that uses it. This set is the
        // first set above with `constructor_or_destructor_declaration`, for the same ambiguity.
        &[
            "operator_cast_definition",
            "operator_cast_declaration",
            "constructor_or_destructor_definition",
            "constructor_or_destructor_declaration",
        ],
        // A friend declaration also starts with specifiers and attribute macros:
        // `ALWAYS_INLINE friend bool operator==(A, A);`. The keyword `friend` decides.
        &[
            "_declaration_specifiers",
            "operator_cast_definition",
            "operator_cast_declaration",
            "constructor_or_destructor_definition",
            "constructor_or_destructor_declaration",
            "friend_declaration",
        ],
        // Specifiers and attributes before `friend`, before a constructor, and before a member
        // declaration. The keyword or the type after them decides.
        &[
            "_declaration_specifiers",
            "_constructor_specifiers",
            "friend_declaration",
        ],
        // The specifiers of a declaration with the type `void` are a second reading, and the
        // specifiers of a block declaration with `extern` before the type are a third reading. Refer
        // to `_void_declaration_specifiers` and `_extern_declaration_specifiers`. A declaration with
        // no declarator shares the repeat of the specifiers, and the generator names that repeat
        // after `declaration`. The specifiers of a structured binding share the repeat too, and each
        // set with `declaration` names them.
        &["_extern_storage_class", "storage_class_specifier"],
        &["_declaration_specifiers", "_void_declaration_specifiers"],
        &["_declaration_specifiers", "_void_declaration_specifiers", "type_specifier"],
        &[
            "_declaration_specifiers",
            "_void_declaration_specifiers",
            "type_specifier",
            "call_expression",
        ],
        &["_extern_declaration_specifiers", "type_specifier"],
        // A calling convention before a constructor or a declaration: `__thiscall I::I() {}`,
        // `__regcall int f();`. The token after the specifiers decides, as for the other specifiers.
        &[
            "_declaration_specifiers",
            "_void_declaration_specifiers",
            "constructor_or_destructor_definition",
        ],
        &[
            "_declaration_specifiers",
            "_void_declaration_specifiers",
            "_constructor_specifiers",
        ],
        &[
            "declaration",
            "_declaration_specifiers",
            "_void_declaration_specifiers",
            "_constructor_specifiers",
            "_structured_binding_specifiers",
        ],
        // A storage class specifier and a cv-qualifier also come before a macro invocation line:
        // `static Q_LOGGING_CATEGORY(log, "qtc", QtWarningMsg)`. The empty token of the external
        // scanner after them decides between the two readings.
        &["_declaration_modifiers", "macro_invocation"],
        &[
            "declaration",
            "_declaration_specifiers",
            "_void_declaration_specifiers",
            "_constructor_specifiers",
            "friend_declaration",
            "_structured_binding_specifiers",
        ],
        &[
            "declaration",
            "_declaration_specifiers",
            "_void_declaration_specifiers",
            "operator_cast_definition",
            "operator_cast_declaration",
            "constructor_or_destructor_definition",
            "_structured_binding_specifiers",
        ],
        &[
            "declaration",
            "_declaration_specifiers",
            "_void_declaration_specifiers",
            "operator_cast_definition",
            "operator_cast_declaration",
            "constructor_or_destructor_definition",
            "constructor_or_destructor_declaration",
            "friend_declaration",
            "_structured_binding_specifiers",
        ],
        // A block holds no declaration and no definition of a conversion function. Refer to `items`.
        &[
            "_block_declaration",
            "_declaration_specifiers",
            "_void_declaration_specifiers",
            "_extern_declaration_specifiers",
            "constructor_or_destructor_definition",
            "_structured_binding_specifiers",
        ],
        // `P...[0](x)` calls an element of a value pack or casts to an element of a type pack.
        // Only name lookup tells them apart, as for `T(x)` and `f(x)`.
        &["pack_index_specifier", "pack_index_expression"],
        // `(ns::S::*)` is a member pointer and `(ns::S x)` is a parameter. Only the `*` after the
        // last `::` tells them apart.
        &["abstract_member_pointer_declarator", "_scope_resolution"],
        // The arguments of a macro in a condition are an expression list or tokens. Only a token that
        // is not part of an expression, as `>=` in `W(X, >= 1400)`, tells them apart. Until that token,
        // each expression in the arguments is also a sequence of tokens.
        &["preproc_argument_list", "_preproc_token_tree"],
        &["preproc_argument_list", "_preproc_token"],
        &["_preproc_expression", "_preproc_token"],
        &["preproc_defined", "_preproc_token"],
        // `int (__attribute__((x))` starts a grouping or a parameter list, and only the tokens after
        // the attributes tell them apart (Clang ParseParenDeclarator). Where a statement and a
        // constructor can start, `(__attribute__((x))` also starts a cast, or the grouping of the
        // declarator of a constructor.
        &["_declaration_modifiers", "_grouping_attributes"],
        &["_grouping_attributes", "type_descriptor"],
        // After `T (^`, a GNU attribute starts the qualifiers of a block pointer declarator, or the
        // qualifiers of the type of a block literal in a functional cast. The qualifiers of a type
        // also take `__declspec`, and only the tokens after the attributes tell the two apart.
        &["type_descriptor", "_block_pointer_declarator_of_function"],
    ]);
    g.inline.push("_namespace_identifier".to_owned());
    g.precedences = vec![
        vec![s!(argument_list), s!(type_qualifier)],
        vec![s!(_expression_not_binary), s!(_class_name)],
    ];
    items(&mut g);
    types(&mut g);
    declarations(&mut g);
    statements(&mut g);
    expressions(&mut g);
    macros(&mut g);
    // The directive rules change the item lists of the other rules. For this reason, this call comes
    // after the other rule functions.
    preprocessor(&mut g);
    // The expression list of this rule is the final list of `_expression_not_binary`. For this
    // reason, this call comes after the directive rules.
    top_level_expression_statement(&mut g);
    initializer_arguments(&mut g);
    conditional_expression_rule(&mut g);
    block_declaration(&mut g);
    // The declaration of a block comes from the declaration outside a block. For this reason, this
    // call comes after the copy.
    name_grouping_declarators(&mut g);
    extension_before_declarations(&mut g);
    void_type_in_each_rule(&mut g);
    // The literal keywords replace a symbol in each rule. For this reason, this call comes last.
    reserved_words(&mut g);
    g
}

/// The type `void`: a keyword with the node name `primitive_type`.
fn void_type() -> Rule {
    alias("void", s!(primitive_type))
}

/// Give each rule that takes `primitive_type` also the type `void`.
///
/// The token `primitive_type` does not match `void`. A rule that must tell `void` from the other
/// fundamental types takes `void_type`. The tree does not change. O(n) in the size of the rules.
fn void_type_in_each_rule(g: &mut Grammar) {
    let types = choice![s!(primitive_type), void_type()];
    // In an alias to a name, `void` is a plain keyword. An alias in the alias would give the node name
    // `primitive_type` to the node types of the outer rule. Refer to `keyword_stop`.
    let stop = alias(types.clone(), s!(identifier));
    let plain_stop = alias(choice![s!(primitive_type), Rule::from("void")], s!(identifier));
    for rule in g.rules.values_mut() {
        let with_void = replace_rule(rule.clone(), &s!(primitive_type), &types);
        *rule = replace_rule(with_void, &stop, &plain_stop);
    }
}

/// Give each declaration that starts with its own keyword the GNU prefix `__extension__`.
///
/// The keyword stops the pedantic diagnostics for the declaration that follows it, and the front ends
/// then read that declaration again: GCC `cp_parser_declaration` (parser.cc:17496),
/// `cp_parser_block_declaration` (parser.cc:17829), and `cp_parser_member_declaration`
/// (parser.cc:30932). Clang `ParseExternalDeclaration` (Parser.cpp:826) and
/// `ParseCXXClassMemberDeclaration` (ParseDeclCXX.cpp:2814) do the same. The C front ends read the
/// keyword in `c_parser_external_declaration` (c-parser.cc:2183) and `c_parser_struct_declaration`
/// (c-parser.cc:4607). GCC and Clang read a series of the keyword (c-parser.cc:7864,
/// ParseStmt.cpp:1209), and `c::extension_prefix` accepts each length.
///
/// The keyword is a token of the declaration, as in `type_definition` and `alias_declaration`. A
/// declaration with decl-specifiers takes the same prefix at the start of `_declaration_specifiers`.
/// Each declaration takes the same hidden rule, and the parser then reads the keyword one time for
/// each declaration. O(n) in the rules of the list.
fn extension_before_declarations(g: &mut Grammar) {
    for name in [
        "template_declaration",
        "template_instantiation",
        "linkage_specification",
        "namespace_definition",
        "namespace_alias_definition",
        "using_declaration",
        "static_assert_declaration",
        "consteval_block_declaration",
        "export_declaration",
    ] {
        // The keyword comes before each precedence of the rule. A precedence on the first step of a
        // declaration would take the token from the other readings, because the parse table builder
        // compares the precedences before it reads the conflict sets.
        g.redefine(name, |original| seq![c::extension_prefix(), original]);
    }
}

/// The expression statement outside a function, for macro code and code examples.
///
/// The C grammar reads no binary expression there, because `a * b;` is a declaration. For the same
/// reason, the left operand of an assignment there is not a binary expression:
/// `T *(*C::f)() = nullptr;` defines a member, and it is not the assignment
/// `(T * (*C::f)()) = nullptr`. The precedence 1 on the left operand gives `=` to this rule before
/// the parser reduces the operand to an expression.
fn top_level_expression_statement(g: &mut Grammar) {
    let operator = field("operator", choice_of(ASSIGNMENT_OPERATORS.map(Rule::from)));
    g.define(
        "_top_level_assignment_expression",
        seq![
            prec(1, seq![field("left", s!(_expression_not_binary)), operator]),
            prec_right(
                ASSIGNMENT,
                field("right", choice![s!(expression), s!(initializer_list)])
            ),
        ],
    );
    let expressions = replace_rule(
        g.rules["_expression_not_binary"].clone(),
        &s!(assignment_expression),
        &alias(s!(_top_level_assignment_expression), s!(assignment_expression)),
    );
    g.redefine("_top_level_expression_statement", |_| {
        prec_dynamic(NAMESPACE_EXPRESSION, seq![optional(expressions), ";"])
    });
}

/// The dynamic precedence of an expression statement at namespace scope.
///
/// [stmt.pre] gives a statement only to a block. At namespace scope, GCC
/// `cp_parser_toplevel_declaration` (parser.cc) and Clang `ParseExternalDeclaration` (Parser.cpp) read
/// a declaration for each form that is also an expression. The grammar keeps the expression statement
/// there for macro code, and this precedence makes it the last reading: a declaration of the same text
/// wins, also with the negative precedences of a grouping and of a pointer or a reference declarator.
/// `T (*p)[3];`, `T (*cb)(H) = nullptr;`, and `T (C::*pa)[3] = &C::a;` are then declarations with a
/// named type, as they are with a type that is a keyword.
///
/// The precedence is less than the precedences of the declarators of real code, and more than the
/// precedence of a grouping around a function declarator. `void (g(z));` stays an expression, as
/// `FUNCTION_GROUPING` says. A macro statement keeps its reading, because the declaration of the same
/// text has the precedence `NAME_GROUPING_OUTSIDE_BLOCK`.
const NAMESPACE_EXPRESSION: i32 = -100;

/// The keywords that cannot start a type name, and that the lexer gives as keywords where a type
/// name can start.
///
/// These keywords start only a literal or an expression. The lexer gives a keyword as an identifier
/// where the parse state has no action for the keyword. In the parameter list of `T *p(nullptr);`,
/// `nullptr` then became the type of a parameter, and the parse of a function declaration continued.
/// GCC `cp_parser_parameter_declaration` and Clang `TryParseParameterDeclarationClause` find no
/// declaration specifier at such a keyword, and they read an initializer. `this` is not in the set,
/// because C++23 permits `this` before a parameter.
///
/// The grammar also reads the macros `NULL`, `TRUE`, and `FALSE` as literals. `NULL` is in the set,
/// because no code declares a name with that spelling. In `T x(NULL);`, the reading with a parameter of
/// the type `NULL` then stops, and the reading with the initializer `NULL` stays. `TRUE` and `FALSE`
/// are not in the set, because code declares names with those spellings:
/// `SDValue TRUE = N->getOperand(1);`.
///
/// `__extension__` starts a declaration or an expression, and it is not a decl-specifier. In
/// `const __extension__ int x = 1;` the keyword became a macro before the type, and the parse
/// continued. Each front end gives a diagnostic there. Refer to `c::extension_prefix`.
///
/// `__label__` starts a label declaration, and only at the start of a block. In `__label__ a;`
/// outside that position the keyword became the type of a declaration. Refer to `label_declaration`.
const TYPE_NAME_RESERVED_WORDS: [&str; 22] = [
    "__extension__",
    "__label__",
    "nullptr",
    "NULL",
    "true",
    "false",
    "new",
    "delete",
    "sizeof",
    "typeid",
    "alignof",
    "_Alignof",
    "__alignof",
    "__alignof__",
    "__real__",
    "__real",
    "__imag__",
    "__imag",
    "static_cast",
    "dynamic_cast",
    "reinterpret_cast",
    "const_cast",
];

/// The named casts. Each is a keyword ([lex.key]), and the tree reads it as the name of a template
/// function: `static_cast<T>(x)`.
const NAMED_CASTS: [&str; 4] = ["static_cast", "dynamic_cast", "reinterpret_cast", "const_cast"];

/// Define the reserved word sets, and apply the set of type names to the type name of a type
/// specifier.
///
/// The first set applies to each word token outside a `reserved` rule, and it is empty. A parse state
/// in which a type specifier can start gets the set `type_name`: a parameter list, a block, a template
/// argument list, and each other place where a type can start. In such a state, a keyword of the set
/// with no action stops the parse version, and a different version continues. The two sets change
/// nothing where the state has an action for the keyword.
///
/// The set applies only to the start of a type specifier. A name after `*`, `->`, or `struct` keeps
/// the empty set, because C code uses `new` and `delete` as names there: `struct list_head *new;`.
///
/// A reserved word is a keyword, and the lexer reads a keyword with the word token. The C grammar
/// reads `true` and `false` as one token each, which the lexer does not read as a keyword. Each
/// spelling of these literals is then a keyword with the node name of the literal.
fn reserved_words(g: &mut Grammar) {
    for (literal, spellings) in [("true", ["TRUE", "true"]), ("false", ["FALSE", "false"])] {
        g.rules.shift_remove(literal);
        let keywords = alias(choice_of(spellings.map(Rule::from)), sym(literal));
        for rule in g.rules.values_mut() {
            *rule = replace_rule(rule.clone(), &sym(literal), &keywords);
        }
    }
    g.reserved.insert("global".to_owned(), Vec::new());
    g.reserved.insert(
        "type_name".to_owned(),
        TYPE_NAME_RESERVED_WORDS.iter().map(|&word| Rule::from(word)).collect(),
    );
    for name in ["type_specifier", "_nondefining_type_specifier"] {
        g.redefine(name, |original| {
            replace_rule(
                original,
                &s!(_type_identifier),
                &alias(reserved("type_name", s!(identifier)), s!(type_identifier)),
            )
        });
    }
}

/// The macro invocations that are a full statement, declaration, or member with no `;`, and the
/// token trees of macro arguments.
///
/// The external scanner decides where a macro invocation starts, with the rule of clang-format
/// for a macro with no semicolon (`UnwrappedLineParser::parseStructuralElement`). It emits
/// `_macro_line_start` before an uppercase name that ends its line, alone or with one argument
/// list. It emits `_macro_block_start` before an uppercase name and a body on the same line: a
/// call whose arguments cannot be a parameter list, for example `TEST_CASE("name") {`, or a name
/// with no arguments before a block of statements, for example `SCOPE_EXIT { f(); };`.
fn macros(g: &mut Grammar) {
    // The keywords that a token tree keeps as keywords. A different keyword in a token tree is an
    // identifier.
    const KEYWORDS: &[&str] = &[
        "alignas",
        "alignof",
        "and",
        "and_eq",
        "bitand",
        "bitor",
        "break",
        "case",
        "catch",
        "class",
        "co_await",
        "co_return",
        "co_yield",
        "compl",
        "concept",
        "const",
        "consteval",
        "constexpr",
        "constinit",
        "continue",
        "decltype",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "explicit",
        "export",
        "extern",
        "for",
        "friend",
        "goto",
        "if",
        "inline",
        "long",
        "mutable",
        "namespace",
        "new",
        "noexcept",
        "not",
        "not_eq",
        "operator",
        "or",
        "or_eq",
        "private",
        "protected",
        "public",
        "register",
        "requires",
        "return",
        "short",
        "signed",
        "sizeof",
        "static",
        "static_assert",
        "struct",
        "switch",
        "template",
        "thread_local",
        "throw",
        "try",
        "typedef",
        "typeid",
        "typename",
        "union",
        "unsigned",
        "using",
        "virtual",
        "volatile",
        "while",
        "xor",
        "xor_eq",
    ];
    // The punctuators of C++, except the brackets and the tokens that contain a bracket.
    const PUNCTUATORS: &[&str] = &[
        "!", "!=", "%", "%=", "&", "&&", "&=", "*", "*=", "+", "++", "+=", ",", "-", "--", "-=", "->", "->*", ".",
        ".*", "...", "/", "/=", ":", "::", ";", "<", "<<", "<<=", "<=", "<=>", "=", "==", ">", ">=", ">>", ">>=", "?",
        "^", "^=", "^^", "|", "|=", "||", "~",
    ];
    let mut items = vec![
        s!(token_tree),
        s!(identifier),
        s!(number_literal),
        s!(string_literal),
        s!(raw_string_literal),
        s!(char_literal),
        s!(true),
        s!(false),
        s!(null),
        s!(this),
        s!(auto),
        s!(primitive_type),
    ];
    items.extend(KEYWORDS.iter().chain(PUNCTUATORS).map(|&token| Rule::from(token)));
    g.define("_token_tree_item", Rule::Choice(items));
    // GCC `collect_args` (libcpp/macro.cc) balances only the parentheses of macro arguments. A
    // token tree also balances `[]` and `{}`, for the structure of the tree.
    let content = || repeat(s!(_token_tree_item));
    g.define(
        "token_tree",
        choice![
            seq!["(", content(), ")"],
            seq!["[", content(), close_bracket()],
            seq![open_brace(), content(), close_brace()],
        ],
    );
    // The arguments of a macro invocation are a token tree in parentheses.
    g.define("_macro_arguments", seq!["(", content(), ")"]);
    // The arguments of an attribute that the grammar does not read as expressions.
    // [dcl.attr.grammar]: an attribute-argument-clause is a balanced token sequence. For an attribute
    // that it does not know, GCC reads balanced tokens (cp_parser_std_attribute, gcc/cp/parser.cc) and
    // Clang skips to the closing parenthesis (ParseCXX11AttributeArgs,
    // clang/lib/Parse/ParseDeclCXX.cpp). The external scanner gives the empty token before the `(`
    // only when the tokens of the clause are not an expression list.
    g.define(
        "_attribute_token_arguments",
        seq![s!(_attribute_tokens_marker), "(", content(), ")"],
    );
    let arguments = || field("arguments", alias(s!(_macro_arguments), s!(token_tree)));
    // Right associativity gives a `(` after the name to the arguments.
    //
    // The storage class specifiers of a declaration can come before a macro invocation line, because
    // the macro gives the rest of the declaration: `static Q_LOGGING_CATEGORY(log, "qtc", QtWarningMsg)`
    // in the Qt sources expands to `static const QLoggingCategory &log() { ... }`
    // (qtbase/src/corelib/io/qloggingcategory.h:147). Clang gives one static FunctionDecl there, and
    // the next line is a second item. Without the prefix the line took the next declaration as its
    // declarator. The external scanner decides where such a line starts, and it gives no token where
    // the next line continues the declarator.
    //
    // After a specifier the macro takes its arguments, because a bare name there continues the
    // declaration on the next line: `inline CATCH_DESTRUCTOR_CONSTEXPR` before
    // `AllTrueMatcher AllTrue() { ... }`, and `extern BOOST_ASIO_DECL` before
    // `const error_category& get_netdb_category();`. Such a macro gives a keyword or a linkage
    // attribute, and the line is one declaration with the line after it.
    //
    // The prefix holds no cv-qualifier and no attribute. `alignas` is a `type_qualifier`, and a
    // qualifier in the prefix gives an ERROR node for
    // `alignas(N)` before `PA_COMPONENT_EXPORT(X) T::U v = {};` in the chromium allocator. An
    // attribute before the name has its own rules (`macro_call_attribute`, `attribute_macro`).
    g.define(
        "macro_invocation",
        prec_right(
            0,
            choice![
                seq![
                    s!(_macro_line_start),
                    field("name", s!(identifier)),
                    optional(arguments())
                ],
                seq![
                    repeat1(s!(storage_class_specifier)),
                    s!(_macro_line_after_specifiers),
                    field("name", s!(identifier)),
                    arguments()
                ],
                seq![
                    s!(_macro_block_start),
                    field("name", s!(identifier)),
                    optional(seq![
                        arguments(),
                        optional(field("template_parameters", s!(template_parameter_list))),
                        optional(field("parameters", s!(parameter_list))),
                    ]),
                    field("body", s!(compound_statement)),
                ],
            ],
        ),
    );
    // A macro call before `;` with arguments that are not expressions, as a statement:
    // `QTC_ASSERT(p, return nullptr);`. The external scanner emits `_macro_call_start` before it.
    // The statement is an expression statement with a call expression.
    g.define(
        "_macro_call_expression",
        seq![field("function", s!(identifier)), arguments()],
    );
    g.define(
        "_macro_call_statement",
        seq![
            s!(_macro_call_start),
            alias(s!(_macro_call_expression), s!(call_expression)),
            ";"
        ],
    );
    for name in ["_top_level_statement", "_non_case_statement"] {
        g.redefine(name, |original| {
            choice![
                original,
                s!(macro_invocation),
                alias(s!(_macro_call_statement), s!(expression_statement)),
            ]
        });
    }
    g.redefine("_field_declaration_list_item", |original| {
        choice![original, s!(macro_invocation)]
    });
    // A statement macro with arguments and a body: `FOREACH(item, items) { g(item); }`. A block, a case body, a
    // label, and a substatement hold a statement, and no function definition (GCC `cp_parser_init_declarator`
    // reports "a function-definition is not allowed here", Clang reports `err_function_definition_not_allowed`).
    // At namespace scope, `TEST(Suite, Name) {` is a function definition, and the token is not valid there.
    //
    // Boost defines `BOOST_IF_CONSTEXPR` as `if constexpr` or as `if`, and an `else` after the body continues the
    // statement of the macro (GCC `cp_parser_selection_statement`, Clang `Parser::ParseIfStatement`). Right
    // associativity gives an `else` to the nearest statement, as for `if_statement`.
    g.define(
        "_macro_statement",
        prec_right(
            0,
            seq![
                s!(_macro_statement_start),
                field("name", s!(identifier)),
                arguments(),
                field("body", s!(compound_statement)),
                optional(field("alternative", s!(else_clause))),
            ],
        ),
    );
    for name in ["_block_item", "_case_body_item", "_labeled_item", "_substatement"] {
        g.redefine(name, |original| {
            choice![original, alias(s!(_macro_statement), s!(macro_invocation))]
        });
    }
    // A macro call in an enumerator list: `enum E { A, LLVM_MARK_AS_BITMASK_ENUM(A) };`. The external
    // scanner emits `_macro_enumerator_start` before an uppercase call that `,`, `}`, or a line
    // break comes after. A name with no arguments stays an enumerator.
    g.define(
        "_macro_enumerator",
        seq![s!(_macro_enumerator_start), field("name", s!(identifier)), arguments()],
    );
    g.redefine("enumerator_list", |original| {
        let mut members = original.into_members();
        let Rule::Repeat(items) = members.remove(1) else {
            panic!("the second member of `enumerator_list` is the repeat of its items");
        };
        let macro_item = seq![alias(s!(_macro_enumerator), s!(macro_invocation)), optional(",")];
        members.insert(1, repeat(choice![*items, macro_item]));
        Rule::Seq(members)
    });
}

/// True for the member of the C item choices that reads an old-style function definition.
fn is_old_style_definition(rule: &Rule) -> bool {
    matches!(rule, Rule::Alias { content, .. } if **content == sym("_old_style_function_definition"))
}

/// The dynamic precedence of a deduction guide at namespace scope: `S(const char *) -> S<std::string>;`.
///
/// A deduction guide has the form of a constructor declarator with a trailing return type
/// ([temp.deduct.guide]). GCC `cp_parser_constructor_declarator_p` and Clang `isConstructorDeclarator` read
/// a guide at namespace scope and in a class body, and not in a block (Clang ParseDecl.cpp,
/// `AllowDeductionGuide` in ParseDirectDeclarator). Clang `Sema::isDeductionGuideName` says that the
/// syntactic form is sufficient to identify a guide.
///
/// At namespace scope, `S(A) -> S<int>;` is also an expression statement with a member access to a
/// template-id, and the two readings have the same other dynamic precedences. GCC and Clang read no
/// expression statement at namespace scope. The precedence gives the text to the guide.
const DEDUCTION_GUIDE: i32 = 1;

/// A deduction guide at namespace scope, with the node name of a declaration. A namespace body, the
/// translation unit, an export declaration, and their conditional groups hold it. A block holds no guide.
fn deduction_guide() -> Rule {
    alias(s!(_deduction_guide_declaration), s!(declaration))
}

/// The `>` that closes a template list. The token precedence prefers `>` to `>>`.
fn closing_angle() -> Rule {
    alias(token(prec(1, ">")), ">")
}

/// The `>=` operator, as one token with the token precedence of `closing_angle`.
///
/// Without the same precedence, the lexer splits `>=` into `>` and `=` in a template argument:
/// `conditional_t<N >= 8, A, B>`. Where `>=` is not valid, the lexer still gives `>`.
fn greater_equal() -> Rule {
    alias(token(prec(1, ">=")), ">=")
}

/// The rule for an operator of a comparison or a fold. The operator `>=` is `greater_equal`.
fn operator_token(operator: &str) -> Rule {
    if operator == ">=" {
        greater_equal()
    } else {
        Rule::from(operator)
    }
}

/// The rule with each string `from` replaced by `to`. O(n) in the size of the rule.
fn replace_string(rule: Rule, from: &str, to: &Rule) -> Rule {
    let recurse = |content: Box<Rule>| Box::new(replace_string(*content, from, to));
    match rule {
        Rule::String(text) if text == from => to.clone(),
        Rule::Seq(members) => Rule::Seq(members.into_iter().map(|m| replace_string(m, from, to)).collect()),
        Rule::Choice(members) => Rule::Choice(members.into_iter().map(|m| replace_string(m, from, to)).collect()),
        Rule::Repeat(content) => Rule::Repeat(recurse(content)),
        Rule::Repeat1(content) => Rule::Repeat1(recurse(content)),
        Rule::Field { name, content } => Rule::Field {
            name,
            content: recurse(content),
        },
        Rule::Alias { content, named, value } => Rule::Alias {
            content: recurse(content),
            named,
            value,
        },
        Rule::Prec { kind, value, content } => Rule::Prec {
            kind,
            value,
            content: recurse(content),
        },
        other => other,
    }
}

/// The operator of a binary operation rule: the string in its field `operator`. O(n) in the members of the
/// rule.
fn binary_operator(rule: &Rule) -> Option<&str> {
    match rule {
        Rule::Prec { content, .. } => binary_operator(content),
        Rule::Seq(members) => members.iter().find_map(|member| match member {
            Rule::Field { name, content } if name == "operator" => match &**content {
                Rule::String(text) => Some(text.as_str()),
                _ => None,
            },
            _ => None,
        }),
        _ => None,
    }
}

/// The `...[` that opens a C++26 pack index, as one token. White space can separate `...` and `[`.
///
/// With one token, the token after a name tells a pack index from a pack expansion, and the
/// parser needs no second token of lookahead.
fn pack_index_open() -> Rule {
    choice![
        alias(token(seq!["...", re(r"\s*"), "["]), "...["),
        seq![alias(s!(_pack_index_ellipsis), "..."), "["],
    ]
}

/// A macro between the type and the declarator, in the place of a calling convention. The external
/// scanner reads its name: `DWORD WINAPI f(LPVOID p);`.
fn call_macro() -> Rule {
    alias(s!(_call_attribute_macro), s!(attribute_macro))
}

/// A macro after the `*` or the `&` of a declarator, in the place of a qualifier. The external
/// scanner reads its name: `char * BROTLI_RESTRICT buffer;`. Refer to `_pointer_call_attribute_macro`.
fn pointer_call_macro() -> Rule {
    alias(s!(_pointer_call_attribute_macro), s!(attribute_macro))
}

/// A macro after the `(` of a grouping, in the place of a calling convention. The external scanner
/// reads its name: `typedef void (WINAPI *F)(void *);`. Refer to `_grouping_call_attribute_macro`.
fn grouping_call_macro() -> Rule {
    alias(s!(_grouping_call_attribute_macro), s!(attribute_macro))
}

/// The attribute macros after the declarator of a variable or a data member: `int n GUARDED_BY(mu);`.
/// `first` is the rule of the first macro, which has a dynamic precedence. Refer to
/// `_declarator_attribute_macro`.
fn declarator_attribute_macros(first: &str) -> Rule {
    seq![
        alias(sym(first), s!(attribute_macro)),
        repeat(alias(s!(_next_declarator_attribute_macro), s!(attribute_macro))),
    ]
}

/// A function-like macro in the place of an attribute before specifiers: `DEPRECATED("x") void f();`.
/// Refer to `_macro_call_attribute`.
fn macro_call_attribute() -> Rule {
    choice![
        alias(s!(_macro_call_attribute), s!(attribute_macro)),
        alias(s!(_macro_call_attribute_tokens), s!(attribute_macro)),
    ]
}

/// The modifiers and the macros before the type of a declaration or a parameter: the storage
/// specifiers, the cv-qualifiers, the attributes, and the attribute macros.
///
/// A parameter with `this` takes the same prefix, and the two forms then have one repeat symbol. The
/// parser reads the prefix of a parameter with no fork, and `this` after the prefix selects the
/// explicit object parameter.
fn specifier_prefix() -> Rule {
    repeat(choice![
        s!(_declaration_modifiers),
        s!(attribute_macro),
        macro_call_attribute()
    ])
}

/// The attributes and the qualifiers after the `&` or `&&` of a reference declarator.
///
/// An attribute-specifier-seq can follow the operator ([dcl.decl.general]): `int &[[a]] r = x;`. GCC
/// and Clang also read qualifiers and GNU attributes there: `size_t & __restrict pos`,
/// `S &__attribute__((used)) f()` (GCC parser.cc, cp_parser_ptr_operator and cp_parser_declarator.
/// Clang ParseDecl.cpp, ParseDeclaratorInternal). Clang also reads a calling convention there:
/// `int & __cdecl f();`. GCC reads the qualifiers before the attributes, and Clang reads the
/// attributes first. The repeat accepts the parts in each order.
///
/// Each declarator with `&` or `&&` uses this rule. Where the parser keeps two such declarators in
/// one parse state, they then have the same repeat symbol, and the parser does not select one before
/// the name.
fn reference_qualifiers() -> Rule {
    repeat(choice![
        s!(type_qualifier),
        s!(attribute_specifier),
        s!(ms_call_modifier),
        s!(attribute_declaration)
    ])
}

/// The attributes and the cv-qualifiers after the `::*` of a pointer to a member: `int C::*[[a]] p`.
///
/// An attribute-specifier-seq and then cv-qualifiers can follow the `*` ([dcl.decl.general]). GCC
/// reads them in `cp_parser_ptr_operator`, and GNU attributes in `cp_parser_declarator`. Clang reads
/// all of them in `ParseTypeQualifierListOpt`. Each pointer to a member uses this rule, for the
/// reason that `reference_qualifiers` gives.
fn member_pointer_qualifiers() -> Rule {
    repeat(choice![
        s!(type_qualifier),
        s!(attribute_specifier),
        s!(attribute_declaration)
    ])
}

/// The rule with its sequence replaced by the result of `edit`. The sequence can be inside
/// precedences.
///
/// # Panics
///
/// If the rule is not a sequence inside zero or more precedences. A grammar that edits a rule
/// with a different shape has an error in its source code.
fn edit_sequence(rule: Rule, edit: impl FnOnce(Vec<Rule>) -> Vec<Rule>) -> Rule {
    match rule {
        Rule::Prec { kind, value, content } => Rule::Prec {
            kind,
            value,
            content: Box::new(edit_sequence(*content, edit)),
        },
        Rule::Seq(members) => Rule::Seq(edit(members)),
        other => panic!("the rule is not a sequence in precedence wrappers: {other:?}"),
    }
}

/// The rule with each member of its sequence replaced by the result of `edit`. O(n) in the
/// members of the sequence.
///
/// # Panics
///
/// If the rule is not a sequence inside zero or more precedences.
fn edit_members(rule: Rule, edit: &impl Fn(Rule) -> Rule) -> Rule {
    edit_sequence(rule, |members| members.into_iter().map(edit).collect())
}

/// The rule with `extra` after the first member of its sequence for which `is_marker` is true.
/// O(n) in the members of the sequence.
///
/// # Panics
///
/// If the rule is not a sequence inside zero or more precedences, or if the sequence has no such
/// member.
fn insert_after(rule: Rule, is_marker: impl Fn(&Rule) -> bool, extra: Rule) -> Rule {
    edit_sequence(rule, |mut members| {
        let index = members
            .iter()
            .position(is_marker)
            .unwrap_or_else(|| panic!("the sequence has no marker member: {members:?}"));
        members.insert(index + 1, extra);
        members
    })
}

/// The rule with each copy of `from` in it replaced by `to`. O(n) in the size of the rule.
fn replace_rule(rule: Rule, from: &Rule, to: &Rule) -> Rule {
    if rule == *from {
        return to.clone();
    }
    let replace = |content: Box<Rule>| Box::new(replace_rule(*content, from, to));
    let replace_all = |members: Vec<Rule>| members.into_iter().map(|m| replace_rule(m, from, to)).collect();
    match rule {
        Rule::Seq(members) => Rule::Seq(replace_all(members)),
        Rule::Choice(members) => Rule::Choice(replace_all(members)),
        Rule::Repeat(content) => Rule::Repeat(replace(content)),
        Rule::Repeat1(content) => Rule::Repeat1(replace(content)),
        Rule::Field { name, content } => Rule::Field {
            name,
            content: replace(content),
        },
        Rule::Alias { content, named, value } => Rule::Alias {
            content: replace(content),
            named,
            value,
        },
        Rule::Prec { kind, value, content } => Rule::Prec {
            kind,
            value,
            content: replace(content),
        },
        Rule::Token { immediate, content } => Rule::Token {
            immediate,
            content: replace(content),
        },
        Rule::Reserved { context, content } => Rule::Reserved {
            context,
            content: replace(content),
        },
        other @ (Rule::Blank | Rule::String(_) | Rule::Pattern { .. } | Rule::Symbol(_)) => other,
    }
}

fn items(g: &mut Grammar) {
    // An empty declaration after the GNU keyword `__extension__`: `__extension__ ;`. GCC
    // `cp_parser_declaration` (parser.cc:17519) reads the `;` after the keyword, and Clang
    // `ParseExternalDeclaration` (Parser.cpp:813) builds an `EmptyDecl` for it. A block takes no such
    // declaration, because `cp_parser_block_declaration` has no step for the `;`. The rule is hidden,
    // as `_empty_declaration` is, and the two tokens are children of the list.
    define_after(
        g,
        "_empty_declaration",
        "_extension_empty_declaration",
        seq![repeat1(s!(_extension_specifier)), ";"],
    );
    let declarations = || {
        [
            "namespace_definition",
            "concept_definition",
            "namespace_alias_definition",
            "using_declaration",
            "alias_declaration",
            "static_assert_declaration",
            "consteval_block_declaration",
            "template_declaration",
            "template_instantiation",
        ]
        .map(sym)
    };
    // [dcl.asm] gives the asm-declaration its own production, `asm ( balanced-token-seq ) ;`, among
    // the declarations. At namespace scope, in a namespace body, and in a linkage specification body,
    // `asm("nop");` is a declaration and no statement. GCC `cp_parser_block_declaration`
    // (parser.cc:17849) calls `cp_parser_asm_definition` for it, GCC C
    // `c_parser_external_declaration` (c-parser.cc:2189) calls `c_parser_asm_definition`, and Clang
    // `ParseExternalDeclaration` builds a `FileScopeAsmDecl`. A class body takes no asm-declaration,
    // and the two front ends give an error for `struct S { asm("nop"); };`. In a block, the same text
    // is a statement, and Clang builds a `GCCAsmStmt` there.
    //
    // The node holds `gnu_asm_expression`, as a declaration with a GNU asm label holds it. The rule
    // takes each form of that node, because the grammar read each form before. At namespace scope
    // GCC C++ gives a warning for the qualifier `volatile` (parser.cc:25219), reads the output
    // operands and the input operands, and rejects the clobbers and the labels (parser.cc:25296).
    // Clang rejects each qualifier and each operand list there, and GCC C reads only the plain form.
    g.define("asm_declaration", seq![s!(gnu_asm_expression), ";"]);
    let definitions = || [alias(s!(constructor_or_destructor_definition), s!(function_definition))];
    // A block holds no declaration and no definition of a conversion function. A declaration in a block
    // starts with a decl-specifier (GCC `cp_parser_simple_declaration`, Clang `isCXXSimpleDeclaration`). In a
    // block, `B::operator bool();` calls a conversion function. Refer to `_conversion_function_id`.
    let conversion_functions = || {
        [
            alias(s!(operator_cast_definition), s!(function_definition)),
            alias(s!(operator_cast_declaration), s!(declaration)),
        ]
    };
    g.redefine("_top_level_item", |original| {
        let mut members: Vec<Rule> = original
            .into_members()
            .into_iter()
            .filter(|m| !is_old_style_definition(m))
            .collect();
        members.extend(declarations());
        members.extend(
            [
                "module_declaration",
                "export_declaration",
                "import_declaration",
                "global_module_fragment_declaration",
                "private_module_fragment_declaration",
            ]
            .map(sym),
        );
        members.extend(definitions());
        members.extend(conversion_functions());
        members.push(deduction_guide());
        members.push(s!(asm_declaration));
        members.push(s!(ms_if_exists));
        members.push(s!(_extension_empty_declaration));
        Rule::Choice(members)
    });
    g.redefine("_block_item", |original| {
        let mut members: Vec<Rule> = original
            .into_members()
            .into_iter()
            .filter(|m| {
                !is_old_style_definition(m) && !matches!(m, Rule::Symbol(name) if name.starts_with("preproc_if"))
            })
            .map(|m| if m == s!(_empty_declaration) { s!(_block_empty_declaration) } else { m })
            .collect();
        members.extend(declarations());
        // An import declaration is valid only at global scope, as in GCC
        // `cp_parser_import_declaration`. In a block, `import` is an identifier: `import->run();`.
        members.push(s!(export_declaration));
        members.push(alias(s!(preproc_if_in_block), s!(preproc_if)));
        members.push(alias(s!(preproc_ifdef_in_block), s!(preproc_ifdef)));
        members.extend(definitions());
        members.push(alias(s!(_ms_if_exists_in_block), s!(ms_if_exists)));
        Rule::Choice(members)
    });
    // The body of a namespace, of a linkage specification, and of an export declaration holds the
    // items of a block with the declarations and the conditional groups of a namespace. A block item
    // is a declaration of a block. Refer to `block_declaration`.
    let declaration_list_items: Vec<Rule> = g.rules["_block_item"]
        .clone()
        .into_members()
        .into_iter()
        .map(|member| {
            if member == alias(s!(preproc_if_in_block), s!(preproc_if)) {
                alias(s!(preproc_if_in_declaration_list), s!(preproc_if))
            } else if member == alias(s!(preproc_ifdef_in_block), s!(preproc_ifdef)) {
                alias(s!(preproc_ifdef_in_declaration_list), s!(preproc_ifdef))
            } else if member == alias(s!(_ms_if_exists_in_block), s!(ms_if_exists)) {
                s!(ms_if_exists)
            } else {
                member
            }
        })
        .chain(conversion_functions())
        .chain([deduction_guide(), s!(_extension_empty_declaration), s!(asm_declaration)])
        .collect();
    // `export` takes an item of a declaration list. Its braces start only a declaration list, and the
    // only statement after it is an expression statement: `export MACRO(x);`.
    let export_items: Vec<Rule> = declaration_list_items
        .iter()
        .map(|member| {
            if *member == s!(statement) {
                prec_dynamic(NAMESPACE_EXPRESSION, s!(expression_statement))
            } else {
                member.clone()
            }
        })
        .collect();
    // A namespace body and a linkage specification hold no statement of C++. The grammar keeps the
    // statements for macro code, and a statement there is the last reading. Refer to
    // `NAMESPACE_EXPRESSION`.
    let declaration_list_items: Vec<Rule> = declaration_list_items
        .into_iter()
        .map(|member| {
            if member == s!(statement) {
                prec_dynamic(NAMESPACE_EXPRESSION, member)
            } else {
                member
            }
        })
        .collect();
    define_after(g, "_block_item", "_declaration_list_item", Rule::Choice(declaration_list_items));
    define_after(g, "_declaration_list_item", "_export_item", Rule::Choice(export_items));
    g.redefine("_block_item", |original| {
        replace_rule(
            original,
            &s!(declaration),
            &alias(s!(_block_declaration), s!(declaration)),
        )
    });
    // A case body holds the items of a block, except a case label: a case label starts the next case
    // statement. A conditional group in a case body has the tokens of the scanner for a group with no case
    // label. A group with a case label ends the case statement, and it is an item of the block.
    let case_body_items = g.rules["_block_item"]
        .clone()
        .into_members()
        .into_iter()
        .map(|member| {
            if member == s!(statement) {
                s!(_non_case_statement)
            } else if member == alias(s!(preproc_if_in_block), s!(preproc_if)) {
                alias(s!(preproc_if_in_case), s!(preproc_if))
            } else if member == alias(s!(preproc_ifdef_in_block), s!(preproc_ifdef)) {
                alias(s!(preproc_ifdef_in_case), s!(preproc_ifdef))
            } else {
                member
            }
        });
    g.define("_case_body_item", choice_of(case_body_items));
    // A label holds one item of a block after its conditional groups: `lab: using T = int;`. C++23 and C23
    // permit a declaration after a label ([stmt.label], GCC `cp_parser_statement`, Clang
    // `ParseLabeledStatement`). GCC C also reads a nested function definition there
    // (`c_parser_compound_statement_nostart`). The conditional groups before the item are children of the
    // label. For this reason, the item is not a group. `statement` includes `attributed_statement`.
    let not_labeled_items = [
        alias(s!(preproc_if_in_block), s!(preproc_if)),
        alias(s!(preproc_ifdef_in_block), s!(preproc_ifdef)),
        s!(attributed_statement),
    ];
    let labeled_items = g.rules["_block_item"]
        .clone()
        .into_members()
        .into_iter()
        .filter(|member| !not_labeled_items.contains(member));
    g.define("_labeled_item", choice_of(labeled_items));
    // [dcl.pre]: a declaration with no declarator declares a class or an enumeration. In a block,
    // `A<N>::X::Y;` and `C::x;` are then expression statements, as GCC and Clang read them. A
    // built-in type has no expression reading. `int;` stays a declaration, and the compilers give a
    // diagnostic for it (GCC decl.cc, check_tag_decl. Clang SemaDecl.cpp, ext_no_declarators).
    g.define(
        "_block_empty_declaration",
        seq![
            choice![
                s!(class_specifier),
                s!(struct_specifier),
                s!(union_specifier),
                s!(interface_specifier),
                s!(enum_specifier),
                s!(primitive_type),
                s!(sized_type_specifier),
            ],
            ";"
        ],
    );
    ms_if_exists(g);
    preproc_if(g, "", || repeat(s!(_top_level_item)), 0);
    preproc_if(g, "_in_block", || repeat(s!(_block_item)), 0);
    preproc_if(g, "_in_declaration_list", || repeat(s!(_declaration_list_item)), 0);
}

/// The MSVC `__if_exists` and `__if_not_exists` blocks: `__if_exists(T::type) { typedef T::type U; }`.
///
/// The condition is a name: an unqualified-id with an optional nested-name-specifier (Clang Parser.cpp,
/// ParseMicrosoftIfExistsCondition). The block holds the items of its context, and the compiler reads them
/// when the name exists, or when it does not exist. Clang reads the block at namespace scope
/// (ParseMicrosoftIfExistsExternalDeclaration), in a class body (ParseDeclCXX.cpp,
/// ParseMicrosoftIfExistsClassDeclaration), and in a block (ParseStmt.cpp, ParseMicrosoftIfExistsStatement).
/// The block makes no scope, as a conditional group does not. For this reason, the items are children of
/// the node, and not of a compound statement. GCC reads no such block.
///
/// Each context gets one copy of the rule, with the items of that context, and each copy has the node name
/// `ms_if_exists`. The items of a block are block items, and a case body and a label take the copy of a block.
/// At file scope, the block holds the items of a namespace body, as Clang reads the declarations of the two
/// with ParseExternalDeclaration. A module declaration and an import declaration are items of the translation
/// unit only. With them in a list that a `}` closes, the generator merges the parse states after a statement
/// in a case body with the states in such a list. In a block, a `module` or an `import` at the start of the
/// next statement then ends the case statement.
fn ms_if_exists(g: &mut Grammar) {
    let name = || {
        field(
            "name",
            choice![
                s!(identifier),
                s!(qualified_identifier),
                alias(s!(_qualified_conversion_function_id), s!(qualified_identifier)),
                s!(template_function),
                s!(operator_name),
                alias(s!(_conversion_function_id), s!(operator_cast)),
                s!(destructor_name),
            ],
        )
    };
    for (rule, item) in [
        ("ms_if_exists", "_declaration_list_item"),
        ("_ms_if_exists_in_block", "_block_item"),
        ("_ms_if_exists_in_field_declaration_list", "_field_declaration_list_item"),
    ] {
        g.define(
            rule,
            seq![
                choice!["__if_exists", "__if_not_exists"],
                "(",
                name(),
                ")",
                open_brace(),
                repeat(sym(item)),
                close_brace()
            ],
        );
    }
}

/// The dynamic precedence of declaration specifiers with a type that is a keyword: a fundamental
/// type, `auto`, `decltype(e)`, or `typename T::U`.
///
/// Such a type is not a name, and no name lookup is necessary. A statement that starts with it is a
/// declaration when a declaration is possible ([stmt.ambig]): `int (*p)[3];` and
/// `void (*f)(int) = g;` declare variables, and they are not functional casts in an expression
/// statement. GCC `cp_parser_statement` and Clang `isCXXDeclarationStatement` read a declaration
/// there.
///
/// The second reading of the type gives the precedence to the specifiers of a declaration. Where
/// the type is also a functional cast, the declaration then has the higher precedence. A
/// parenthesized declarator has the precedence -10, and a reference declarator with no initializer
/// in a block has the precedence -100. The precedence is more than their sum in real code. The
/// parser keeps the two readings of the type until the specifiers are complete. Then the reading
/// with the precedence replaces the other.
///
/// The specifiers of a parameter have no such reading. The parameters are inside the declarator, and
/// the precedence would also go to a declaration reading of a call: in `f(g(int(x)));`, the name `f`
/// is not a type, and a parameter `int(x)` does not make the statement a declaration.
const KEYWORD_TYPE: i32 = 200;

/// The dynamic precedence of a declaration with the type `void` and one function declarator of a name:
/// `void f(A);`, `static void g(A);`, `void h<T>(MACRO(T));`.
///
/// An object of the type `void` is not possible ([basic.fundamental]). GCC `check_var_type` gives the
/// error "variable or field declared void", and Clang gives the error `err_typecheck_decl_incomplete_type`.
/// The parenthesized list after the name is then a parameter list. Without name lookup, the grammar also
/// reads `void f(A);` in a block as a variable with the initializer `A` (refer to
/// `NAME_INITIALIZER_IN_BLOCK`), and `void h<T>(MACRO(T));` as a variable with a call.
///
/// The token `primitive_type` does not match `void`, and `void` is a separate keyword. Refer to
/// `void_type_in_each_rule`. A declaration takes a second reading with the specifiers
/// `_void_declaration_specifiers` and one `function_declarator`. The second reading has this precedence and
/// the tree of the function reading. The precedence is more than the sum of the precedences of the calls in
/// the initializer of a variable. A pointer, a reference, or an array declarator has no second reading:
/// `void *p(nullptr);` stays a variable.
const VOID_FUNCTION: i32 = 100;

/// The dynamic precedence of a block declaration with `extern` and one declarator of the class of a
/// function: `extern T f(U);`, `extern T &g(U);`.
///
/// A block declaration with `extern` has no initializer (C++ [dcl.init.general], C11 6.7.9p5). GCC
/// `grokdeclarator` gives the error "has both 'extern' and initializer", and Clang gives the error
/// `err_block_extern_cant_init`. The parenthesized list after the declarator is then a parameter list.
/// Without name lookup, the grammar also reads `extern T f(U);` in a block as a variable with the
/// initializer `U`, and `extern T &g(U);` as a reference with an initializer.
///
/// The block declaration takes a second reading with the specifiers `_extern_declaration_specifiers`, which
/// have `extern` before the type, and one declarator of the class of a function. The second reading has
/// this precedence and the tree of the function reading.
const EXTERN_FUNCTION_IN_BLOCK: i32 = 100;

fn types(g: &mut Grammar) {
    g.define(
        "placeholder_type_specifier",
        prec(
            1,
            seq![
                field(
                    "constraint",
                    optional(choice![
                        alias(s!(qualified_type_identifier), s!(qualified_identifier)),
                        s!(template_type),
                        s!(_type_identifier),
                    ])
                ),
                choice![s!(auto), alias(s!(decltype_auto), s!(decltype))],
            ],
        ),
    );
    g.define("auto", "auto");
    g.define("decltype_auto", seq!["decltype", "(", s!(auto), ")"]);
    g.define(
        "decltype",
        seq!["decltype", "(", choice![s!(expression), s!(comma_expression)], ")"],
    );
    // The GNU and C23 `typeof` of an expression or of a type.
    g.define(
        "typeof_specifier",
        seq![
            choice!["typeof", "__typeof__", "__typeof", "typeof_unqual", "__typeof_unqual__"],
            "(",
            choice![field("value", s!(expression)), field("type", s!(type_descriptor))],
            ")",
        ],
    );
    // C++26 pack indexing of a type: `T...[0]`.
    g.define(
        "pack_index_specifier",
        seq![
            field("pack", s!(_type_identifier)),
            pack_index_open(),
            field("index", s!(expression)),
            close_bracket()
        ],
    );
    // A function-like macro that gives the type of a declaration: `void f(BOOST_FWD_REF(T) x);`,
    // where the macro expands to `T&&`. The front ends read the expansion, and the type is the
    // result of the expansion (GCC cp_parser_decl_specifier_seq, Clang ParseDeclarationSpecifiers).
    // The external scanner gives the empty token before the name only when one argument that is a
    // type-id and a declarator with no type of its own come after it. `MAX(a, b) * c;` keeps its
    // expression, because the arguments are not one type-id. The C grammar has the same node with
    // the same fields, and it needs no token, because `T(x)` is no functional cast in C.
    //
    // The rule is in the specifiers of a declaration, of a parameter, and of a typedef, and not in
    // `type_specifier`. A cast, a template argument, and the operand of `sizeof` take a type with no
    // declarator after it, and the scanner gives no token there. The second token is for a parameter,
    // where a `)` also ends the declarator. Refer to `ends_macro_type_declarator` in src/scanner.c.
    let macro_type_specifier = |start: Rule| {
        seq![
            start,
            field("name", s!(identifier)),
            "(",
            field("type", s!(type_descriptor)),
            ")",
        ]
    };
    g.define("macro_type_specifier", macro_type_specifier(s!(_macro_type_start)));
    g.define(
        "_parameter_macro_type_specifier",
        macro_type_specifier(s!(_parameter_macro_type_start)),
    );
    let type_specifier = choice![
        s!(struct_specifier),
        s!(union_specifier),
        s!(enum_specifier),
        s!(class_specifier),
        s!(interface_specifier),
        s!(sized_type_specifier),
        s!(primitive_type),
        s!(template_type),
        s!(dependent_type),
        s!(splice_type_specifier),
        s!(placeholder_type_specifier),
        s!(decltype),
        s!(typeof_specifier),
        s!(pack_index_specifier),
        s!(type_trait_specifier),
        prec_right(
            0,
            choice![
                alias(s!(qualified_type_identifier), s!(qualified_identifier)),
                s!(_type_identifier)
            ]
        ),
    ];
    g.define("type_specifier", type_specifier.clone());
    // [dcl.type.general]: a type-specifier-seq does not define a class or an enumeration. A trailing
    // return type, a new-type-id, and a conversion-type-id are such contexts, and each specifier with
    // a class key there is an elaborated-type-specifier. Clang gives DSC_trailing, DSC_new, and
    // DSC_conv_operator the value AllowDefiningTypeSpec::No (Parser.h:1665 to 1669). The five
    // specifiers keep their node kinds and lose their bodies.
    let mut nondefining_type_specifier = type_specifier;
    for (body, elaborated) in [
        ("struct_specifier", "_elaborated_struct_specifier"),
        ("union_specifier", "_elaborated_union_specifier"),
        ("enum_specifier", "_elaborated_enum_specifier"),
        ("class_specifier", "_elaborated_class_specifier"),
        ("interface_specifier", "_elaborated_interface_specifier"),
    ] {
        nondefining_type_specifier = replace_rule(
            nondefining_type_specifier,
            &sym(body),
            &alias(sym(elaborated), sym(body)),
        );
    }
    g.define("_nondefining_type_specifier", nondefining_type_specifier);
    // `_Nullable` and its relatives are Clang nullability qualifiers. `_Complex` is a GNU type
    // keyword in the position of a qualifier.
    g.redefine("type_qualifier", |original| {
        choice![
            original,
            "mutable",
            "constinit",
            "consteval",
            "_Nullable",
            "_Null_unspecified",
            "__nullable",
            "__nonnull",
            "_Complex",
            "__complex__",
            // GCC and Clang spell `restrict` as `__restrict` in C++ (c-common.cc, c_common_reswords).
            "__restrict",
            // GCC also spells `const` and `volatile` with underscores.
            "__const",
            "__const__",
            "__volatile",
            "__volatile__",
            // Clang nullability of a result.
            "_Nullable_result",
        ]
    });
    // GCC spells `signed` as `__signed` and `__signed__` (c-common.cc, c_common_reswords).
    //
    // A size keyword combines with `char`, `int`, `double`, and the other size keywords, and not with a
    // type name ([dcl.type.general]). Clang rejects `T unsigned x;` (DeclSpec.cpp, DeclSpec::Finish), and
    // GCC accepts it only as an extension with a pedantic warning (decl.cc, grokdeclarator). A name before
    // a size keyword is then an attribute macro where a macro can come: `_In_ unsigned n`,
    // `API unsigned f();`. The name has a lower dynamic precedence than the macro. A type-id has no
    // attribute macros, and there the name stays the type: `(__private long *)p`.
    g.redefine("sized_type_specifier", |original| {
        let keywords = replace_rule(
            original,
            &choice!["signed", "unsigned", "long", "short"],
            &choice!["signed", "unsigned", "long", "short", "__signed", "__signed__"],
        );
        let mut forms = keywords.into_members();
        forms[0] = replace_rule(
            forms[0].clone(),
            &prec_dynamic(-1, s!(_type_identifier)),
            &prec_dynamic(-2, s!(_type_identifier)),
        );
        Rule::Choice(forms)
    });
    // After `*`, the precedence gives `__restrict` to the MS pointer modifier, as the C grammar does.
    g.redefine("ms_restrict_modifier", |original| prec(1, original));
    // GCC and Clang read GNU attributes among the qualifiers of a type-id and of a typedef:
    // `using V = int __attribute__((vector_size(8)));` (GCC parser.cc, cp_parser_type_specifier_seq).
    // Clang also reads `__declspec` there, as among the specifiers of a declaration:
    // `typedef __declspec(align(16)) struct { int x; } S;`, `using T = __declspec(align(16)) const S;`
    // (ParseDecl.cpp, ParseDeclarationSpecifiers).
    let qualifiers_with_attributes = |member: Rule| {
        if member == repeat(s!(type_qualifier)) {
            repeat(choice![
                s!(type_qualifier),
                s!(attribute_specifier),
                s!(ms_declspec_modifier)
            ])
        } else {
            member
        }
    };
    // Clang reads a calling convention among the specifiers of a type-id, before its abstract
    // declarator: `is_function<R __stdcall(A...)>` (ParseDecl.cpp, ParseDeclarationSpecifiers).
    // The calling convention is a child of the type-id, as it is a child of a member declaration.
    let calling_convention_before_declarator = |members: Vec<Rule>| {
        members
            .into_iter()
            .flat_map(|member| match member {
                Rule::Field { ref name, .. } if name == "declarator" => {
                    vec![optional(s!(ms_call_modifier)), member]
                }
                other => vec![qualifiers_with_attributes(other)],
            })
            .collect()
    };
    // The type-id of a trailing return type has the shape of each other type-id, and its specifier
    // defines no class and no enumeration. Refer to `_nondefining_type_specifier`.
    let mut trailing_type_descriptor = None;
    g.redefine("type_descriptor", |original| {
        let trailing =
            replace_rule(original.clone(), &s!(type_specifier), &s!(_nondefining_type_specifier));
        trailing_type_descriptor = Some(prec_right(
            0,
            edit_sequence(trailing, calling_convention_before_declarator),
        ));
        prec_right(0, edit_sequence(original, calling_convention_before_declarator))
    });
    g.define(
        "_trailing_type_descriptor",
        trailing_type_descriptor.expect("the redefinition of type_descriptor writes the copy"),
    );
    // A macro can come before the type of a typedef, as among the specifiers of a declaration:
    // `typedef BOOST_DEDUCED_TYPENAME T::type x;`, where the macro expands to `typename`. GCC and
    // Clang read `typedef`, the other specifiers, and GNU attributes in one decl-specifier-seq
    // (cp_parser_decl_specifier_seq, ParseDeclarationSpecifiers). After the type, an identifier is
    // the declarator, and the macro can come only before the type.
    g.redefine("_type_definition_type", |original| {
        edit_sequence(edit_members(original, &qualifiers_with_attributes), |mut members| {
            members[0] = repeat(choice![
                s!(type_qualifier),
                s!(attribute_specifier),
                s!(ms_declspec_modifier),
                s!(attribute_macro)
            ]);
            // A macro can also give the type: `typedef BOOST_CONTAINER_IMPDEF(impl) value_compare;`.
            members[1] = field("type", choice![s!(type_specifier), s!(macro_type_specifier)]);
            members
        })
    });
    // Clang reads a calling convention before each declarator of a typedef, as it does in a
    // declaration: `typedef long __stdcall F(long);`. After a comma, MSVC and Clang read and ignore
    // it: `typedef void __cdecl G(void), __cdecl H(int);` (ParseDecl.cpp, ParseDeclGroup).
    //
    // A macro can take the place of that keyword, as it does in a declaration:
    // `typedef int32_t U_CALLCONV UCharIteratorGetIndex(UCharIterator *iter, int origin);` in the ICU
    // headers, where `U_CALLCONV` is `__cdecl` or nothing. The external scanner reads the name.
    g.redefine("_type_definition_declarators", |_| {
        comma_sep1(seq![
            optional(s!(ms_call_modifier)),
            optional(call_macro()),
            field("declarator", s!(_type_declarator))
        ])
    });
    // GCC `cp_parser_std_attribute`: an attribute token can be a keyword, for example `using`.
    //
    // `__extension__` is also such a keyword. In `[[__extension__]]`, GCC C++ and Clang read an
    // attribute with the name `extension` and ignore it with a warning. After `[[`, the keyword has
    // a second reading as the prefix of the attribute list, and the token after it decides. Refer to
    // `attribute_declaration`.
    let attribute_token = || {
        choice![
            s!(identifier),
            alias("using", s!(identifier)),
            alias("__extension__", s!(identifier)),
        ]
    };
    g.define(
        "attribute",
        seq![
            optional(seq!["using", field("namespace", s!(identifier)), ":"]),
            optional(seq![field("prefix", attribute_token()), "::"]),
            field("name", attribute_token()),
            optional(choice![
                s!(argument_list),
                alias(s!(_attribute_token_arguments), s!(token_tree)),
            ]),
        ],
    );
    g.define("annotation", seq!["=", s!(expression)]);
    // One list holds attributes and C++26 annotations, for example `[[nodiscard, =1]]`. GCC
    // `cp_parser_std_attribute_list` permits an empty entry, `[[ , ]]`, and a pack expansion of an
    // attribute, `[[a(T)...]]`.
    let entry = || choice![seq![s!(attribute), optional("...")], s!(annotation)];
    // [dcl.attr.grammar]: an attribute-specifier is the tokens `[ [ ... ] ]`. White space between the two brackets
    // does not change the tokens, and `[ [nodiscard] ] int f();` is a declaration with an attribute. GCC
    // `cp_nth_tokens_can_be_std_attribute_p` and Clang `isCXX11AttributeSpecifier` read the two tokens.
    // A comment or a directive line between the two brackets does not change the tokens, and one token
    // holds no comment node and no directive node. The external scanner then gives the first bracket,
    // and the extras come between the two brackets. Refer to `scan_attribute_bracket` in
    // `src/scanner.c`.
    let open = || {
        choice![
            alias(token(seq!["[", re(r"\s*"), "["]), "[["),
            seq![alias(s!(_attribute_open_bracket), "["), "["],
        ]
    };
    let close = || {
        choice![
            alias(token(seq!["]", re(r"\s*"), "]"]), "]]"),
            seq![alias(s!(_attribute_close_bracket), "]"), close_bracket()],
        ]
    };
    // GCC C reads the keyword `__extension__` immediately after `[[`, and then the attribute list:
    // `[[__extension__ gnu::unused]] static int q;` (`c_parser_std_attribute_specifier`,
    // c-parser.cc:6333). The keyword stops the pedantic diagnostics of the list, and the tree holds
    // it as a token of the specifier, as GCC C gives it to no attribute. GCC C++ and Clang reject
    // the form, and the acceptance of a form is the union of the two front ends.
    //
    // The keyword takes a branch of its own with one entry after it. `[[__extension__]]` and
    // `[[__extension__, a]]` then keep the attribute with the name `extension`, which GCC C++ and
    // Clang read there. GCC C reads only one such keyword, and only after `[[`.
    let entries = || seq![optional(entry()), repeat(seq![",", optional(entry())])];
    g.define(
        "attribute_declaration",
        seq![
            open(),
            choice![
                seq![s!(_extension_specifier), entry(), repeat(seq![",", optional(entry())])],
                entries(),
            ],
            close()
        ],
    );
    // A GNU attribute can come after the body and before a declarator. The right associativity of
    // `_class_declaration_item` gives that attribute to the class.
    g.define(
        "_class_declaration",
        seq![
            repeat(choice![
                s!(attribute_specifier),
                s!(alignas_qualifier),
                s!(ms_declspec_modifier),
                s!(attribute_declaration),
            ]),
            s!(_class_declaration_item),
        ],
    );
    // A class head can have macros before the name, as clang-format reads them in `parseRecord`:
    // `class LLVM_ABI A final : B {`. After the macros come a virt-specifier, a base clause, or a
    // body with members. `struct stat st;` and `struct stat s{};` declare variables. The positive
    // dynamic precedence prefers the class to a variable `A` with a braced initializer in
    // `class EXPORT A { Q_OBJECT };`.
    let virt_specifiers = || choice![s!(virtual_specifier), s!(class_property_specifier)];
    // The external scanner records the names of the last class heads with a body. A constructor has the
    // name of its class (GCC `cp_parser_constructor_declarator_p`, Clang `isCurrentClassName`), and the
    // scanner uses the names to find a constructor after a macro: `LLVM_ABI A();`. Refer to
    // `_constructor_macro_start`.
    //
    // The scanner gives `_class_head_mark` before a class key when a class head with a body follows it,
    // and a name that it did not record as the last name. The token is an empty extra. The parser
    // shifts it in each state, and the parse states and the trees do not change.
    g.extras.push(s!(_class_head_mark));
    g.define(
        "_class_declaration_item",
        prec_right(
            0,
            seq![
                choice![
                    field("name", s!(_class_name)),
                    seq![
                        optional(field("name", s!(_class_name))),
                        repeat(virt_specifiers()),
                        optional(s!(base_class_clause)),
                        field("body", s!(field_declaration_list)),
                    ],
                    prec_dynamic(
                        3,
                        seq![
                            repeat1(choice![
                                s!(attribute_macro),
                                alias(s!(_attribute_macro_call), s!(attribute_macro)),
                            ]),
                            // Standard attributes and an alignment can follow the macros:
                            // `class BASE_EXPORT [[nodiscard]] A {`, `struct BASE_EXPORT alignas(8) B {`.
                            repeat(choice![s!(attribute_declaration), s!(alignas_qualifier)]),
                            field("name", s!(_class_name)),
                            choice![
                                seq![
                                    repeat1(virt_specifiers()),
                                    optional(s!(base_class_clause)),
                                    field("body", s!(field_declaration_list)),
                                ],
                                seq![s!(base_class_clause), field("body", s!(field_declaration_list))],
                                field(
                                    "body",
                                    alias(s!(_nonempty_field_declaration_list), s!(field_declaration_list))
                                ),
                            ],
                        ]
                    ),
                ],
                // GCC reads more than one attribute after the body (parser.cc, cp_parser_class_specifier).
                repeat(s!(attribute_specifier)),
            ],
        ),
    );
    g.define(
        "_nonempty_field_declaration_list",
        seq![open_brace(), repeat1(s!(_field_declaration_list_item)), close_brace()],
    );
    // A function-like macro in the position of an attribute: `class TSA_CAPABILITY("mutex") M {`.
    // The static precedence gives the `(` after the name to the macro. In `void f() M (int)` the
    // other reading is a second parameter list, of a function that returns a function.
    g.define(
        "_attribute_macro_call",
        prec(
            1,
            prec_dynamic(
                -1,
                seq![field("name", s!(identifier)), field("arguments", s!(argument_list))],
            ),
        ),
    );
    g.define("class_specifier", seq!["class", s!(_class_declaration)]);
    g.define("union_specifier", seq!["union", s!(_class_declaration)]);
    g.define("struct_specifier", seq!["struct", s!(_class_declaration)]);
    // The MSVC class key `__interface` has the head of a class: `__interface __declspec(uuid("...")) I
    // : IUnknown {`. Clang reads it as it reads `class`, `struct`, and `union` (ParseDecl.cpp,
    // ParseDeclarationSpecifiers. ParseDeclCXX.cpp, ParseClassSpecifier). The members of an interface
    // have public access, as in a struct (ParseCXXMemberSpecification). Each class key has its own node
    // kind. `interface` is not a keyword: the Windows headers define it as a macro for `struct`.
    g.define("interface_specifier", seq!["__interface", s!(_class_declaration)]);
    // An elaborated-type-specifier has a class key and a name, and no base clause and no body
    // ([dcl.type.elab]). A trailing return type takes these specifiers, and the brace after
    // `-> struct S` then starts the function body. Refer to `_nondefining_type_specifier`.
    g.define(
        "_elaborated_class_declaration",
        seq![
            repeat(choice![
                s!(attribute_specifier),
                s!(alignas_qualifier),
                s!(ms_declspec_modifier),
                s!(attribute_declaration),
            ]),
            field("name", s!(_class_name)),
        ],
    );
    g.define("_elaborated_class_specifier", seq!["class", s!(_elaborated_class_declaration)]);
    g.define("_elaborated_union_specifier", seq!["union", s!(_elaborated_class_declaration)]);
    g.define("_elaborated_struct_specifier", seq!["struct", s!(_elaborated_class_declaration)]);
    g.define(
        "_elaborated_interface_specifier",
        seq!["__interface", s!(_elaborated_class_declaration)],
    );
    // C++26 class properties: `struct S trivially_relocatable_if_eligible replaceable_if_eligible`.
    g.define(
        "class_property_specifier",
        choice!["trivially_relocatable_if_eligible", "replaceable_if_eligible"],
    );
    g.define(
        "_class_name",
        prec_right(
            0,
            choice![
                s!(_type_identifier),
                s!(template_type),
                s!(splice_type_specifier),
                alias(s!(qualified_type_identifier), s!(qualified_identifier)),
            ],
        ),
    );
    // A function body can be a function try block. A free function can be defaulted or deleted:
    // `void f(int) = delete;`. The declarator declares a function ([dcl.fct.def.general]): in
    // `class EXPORT A { ... };`, `A` is not the name of a function.
    g.redefine("function_definition", |original| {
        Rule::Seq(
            original
                .into_members()
                .into_iter()
                .map(|member| match member {
                    Rule::Field { name, content } if name == "body" => choice![
                        field("body", choice![*content, s!(try_statement)]),
                        s!(default_method_clause),
                        s!(delete_method_clause),
                    ],
                    Rule::Field { name, .. } if name == "declarator" => {
                        seq![optional(call_macro()), field(&name, s!(_declarator_of_function))]
                    }
                    other => other,
                })
                .collect(),
        )
    });
    // A calling convention can come before the specifiers, as before the specifiers of a C function
    // definition: `__regcall int f();` (Clang ParseDecl.cpp, ParseDeclarationSpecifiers).
    g.define(
        "declaration",
        choice![
            seq![
                optional(s!(ms_call_modifier)),
                s!(_declaration_specifiers),
                comma_sep1(seq![
                    optional(call_macro()),
                    field(
                        "declarator",
                        choice![
                            // C reads this declarator with `_declaration_declarator`, for macros in
                            // function declarators. That rule causes many conflicts in C++, and C++
                            // uses `_declarator`.
                            seq![
                                optional(s!(ms_call_modifier)),
                                s!(_declarator),
                                optional(s!(gnu_asm_expression)),
                            ],
                            alias(s!(_macro_attributed_declarator), s!(attributed_declarator)),
                            s!(init_declarator),
                            alias(s!(_pointer_init_declarator), s!(init_declarator)),
                        ]
                    ),
                ]),
                ";",
            ],
            // A declaration with the type `void` and one function declarator of a name has a second
            // reading. Refer to `VOID_FUNCTION`.
            prec_dynamic(
                VOID_FUNCTION,
                seq![
                    optional(s!(ms_call_modifier)),
                    s!(_void_declaration_specifiers),
                    optional(call_macro()),
                    field(
                        "declarator",
                        seq![
                            optional(s!(ms_call_modifier)),
                            s!(function_declarator),
                            optional(s!(gnu_asm_expression)),
                        ]
                    ),
                    ";",
                ]
            ),
            // GCC `cp_parser_simple_declaration` makes the init-declarator-list optional. After
            // specifiers or macros, a class or an enumeration needs no declarator:
            // `[[deprecated]] enum E { A };`, `EXPORT class A;`. The GNU keyword `__extension__`
            // also comes before such a declaration: `__extension__ enum BaseType { A, };`. With no
            // specifier and no keyword, `struct A {};` stays an empty declaration. The repeat has the
            // content of the repeat in `_declaration_specifiers`, and the `;` after the class decides
            // with no fork.
            //
            // A built-in type also needs no declarator: `_VARIANT_BOOL bool;` comes after
            // `#define _VARIANT_BOOL /##/` in the tests of boost wave. GCC `check_tag_decl`
            // (cp/decl.cc) gives the permerror "declaration does not declare anything" and reads the
            // declaration. The macro can expand to nothing, and `bool;` is then the full declaration.
            // A keyword of a built-in type is no declarator, so this rule has no fork with the
            // declaration that has one.
            seq![
                choice![
                    seq![repeat1(s!(_extension_specifier)), specifier_prefix()],
                    repeat1(choice![
                        s!(_declaration_modifiers),
                        s!(attribute_macro),
                        macro_call_attribute()
                    ]),
                ],
                field(
                    "type",
                    choice![
                        s!(class_specifier),
                        s!(struct_specifier),
                        s!(union_specifier),
                        s!(interface_specifier),
                        s!(enum_specifier),
                        s!(primitive_type),
                    ]
                ),
                ";",
            ],
            // A structured binding declaration has an initializer: `auto& [a, b] = p;`.
            seq![
                s!(_structured_binding_specifiers),
                field(
                    "declarator",
                    alias(s!(_structured_binding_init_declarator), s!(init_declarator))
                ),
                ";",
            ],
        ],
    );
    g.define(
        "virtual_specifier",
        choice![
            "final",    // The only permitted value for a class.
            "override", // A function also permits `override`, and the two in each order.
            // MSVC accepts `sealed` in the place of `final`, and `abstract` for a class or for a
            // function (Clang ParseDeclCXX.cpp, isCXX11VirtSpecifier).
            "sealed", "abstract",
        ],
    );
    // CUDA marks a function for the host, the device, or a kernel: `__host__ __device__ int f();`.
    // The CUDA headers define these reserved words as GNU attributes.
    g.redefine("_declaration_modifiers", |original| {
        choice![original, "virtual", "__host__", "__device__", "__global__"]
    });
    // The external scanner decides that the name is a macro, and the parser does not fork. The node
    // then has no negative dynamic precedence, and `DWORD WINAPI f();` keeps the type `DWORD`.
    g.define(
        "_call_attribute_macro",
        field("name", alias(s!(_call_macro_name), s!(identifier))),
    );
    // The same macro after the `*` or the `&` of a declarator. A name after that operator is the
    // declarator, and a name and a second name are the macro and the declarator:
    // `char * BROTLI_RESTRICT buffer;`. The scanner reads the second name before it gives the token.
    g.define(
        "_pointer_call_attribute_macro",
        field("name", alias(s!(_pointer_call_macro_name), s!(identifier))),
    );
    // An identifier in the position of an attribute: `LLVM_ABI void f();`. The negative dynamic
    // precedence gives an ambiguous input to the parse with no macro, for example `f(A b)`.
    g.define("attribute_macro", prec_dynamic(-1, field("name", s!(identifier))));
    // A function-like macro in the place of an attribute, before the specifiers of a declaration:
    // `DEPRECATED("x") void f();`. GCC and Clang read GNU attributes between the decl-specifiers
    // (`cp_parser_decl_specifier_seq`, `ParseDeclarationSpecifiers`). The external scanner gives the
    // empty token before the name only when a specifier, a type and a declarator, or a constructor
    // follows the arguments on the same line. `FOO(x);` stays a call, and `STACK_OF(X509) sk;` gets
    // no token.
    g.define(
        "_macro_call_attribute",
        seq![
            s!(_macro_call_attribute_start),
            field("name", s!(identifier)),
            field("arguments", s!(argument_list)),
        ],
    );
    // The scanner gives a different token when the arguments are not expressions, and the arguments
    // are then a token tree, as in a macro invocation: `LLVM_PREFERRED_TYPE(bool) unsigned f : 1;`.
    g.define(
        "_macro_call_attribute_tokens",
        seq![
            s!(_macro_call_attribute_tokens_start),
            field("name", s!(identifier)),
            field("arguments", alias(s!(_macro_arguments), s!(token_tree))),
        ],
    );
    // A macro can come before the type, between the other specifiers: `static INLINE bool f();`.
    // After the type an identifier is the declarator, as in `A b;`. A parameter takes the same
    // specifiers. In a declaration, a type that is a keyword has a second reading with a precedence.
    // Refer to `KEYWORD_TYPE`.
    let specifiers = |types: Rule| {
        prec_right(
            0,
            seq![
                specifier_prefix(),
                field("type", types),
                repeat(s!(_declaration_modifiers)),
            ],
        )
    };
    // The specifiers of a declaration take the GNU keyword `__extension__` before each other
    // specifier. The specifiers of a parameter and of a conversion-type-id take no such prefix,
    // because the front ends read the keyword only before a declaration. Refer to
    // `c::extension_prefix`.
    let declaration_specifiers = |types: Rule| {
        prec_right(
            0,
            seq![
                c::extension_prefix(),
                specifier_prefix(),
                field("type", types),
                repeat(s!(_declaration_modifiers)),
            ],
        )
    };
    let declaration_type = || {
        choice![
            s!(type_specifier),
            s!(macro_type_specifier),
            prec_dynamic(
                KEYWORD_TYPE,
                choice![
                    s!(primitive_type),
                    s!(sized_type_specifier),
                    s!(placeholder_type_specifier),
                    s!(decltype),
                    s!(dependent_type),
                ]
            ),
        ]
    };
    g.redefine("_declaration_specifiers", |_| declaration_specifiers(declaration_type()));
    // The specifiers of a conversion-type-id: the specifiers of a declaration, with a type that
    // defines no class and no enumeration. `struct C { operator struct S(); };` then reads `struct S`
    // as an elaborated-type-specifier, and the `(` starts the parameter list. Refer to
    // `_nondefining_type_specifier` and to `operator_cast`.
    define_after(
        g,
        "_declaration_specifiers",
        "_conversion_declaration_specifiers",
        specifiers(replace_rule(
            declaration_type(),
            &s!(type_specifier),
            &s!(_nondefining_type_specifier),
        )),
    );
    define_after(
        g,
        "_conversion_declaration_specifiers",
        "_parameter_declaration_specifiers",
        specifiers(choice![
            s!(type_specifier),
            alias(s!(_parameter_macro_type_specifier), s!(macro_type_specifier))
        ]),
    );
    // The second readings of the specifiers of a declaration with the type `void`, and of a block
    // declaration with `extern` before the type. Refer to `VOID_FUNCTION` and `EXTERN_FUNCTION_IN_BLOCK`.
    define_after(
        g,
        "_parameter_declaration_specifiers",
        "_void_declaration_specifiers",
        declaration_specifiers(prec_dynamic(KEYWORD_TYPE, void_type())),
    );
    let modifiers = || {
        repeat(choice![
            s!(_declaration_modifiers),
            s!(attribute_macro),
            macro_call_attribute()
        ])
    };
    define_after(
        g,
        "_void_declaration_specifiers",
        "_extern_declaration_specifiers",
        prec_right(
            0,
            seq![
                c::extension_prefix(),
                modifiers(),
                alias(s!(_extern_storage_class), s!(storage_class_specifier)),
                modifiers(),
                field("type", declaration_type()),
                repeat(s!(_declaration_modifiers)),
            ],
        ),
    );
    define_after(g, "_extern_declaration_specifiers", "_extern_storage_class", Rule::from("extern"));
    g.define(
        "explicit_function_specifier",
        choice!["explicit", prec(CALL, seq!["explicit", "(", s!(expression), ")"]),],
    );
    // C++26 permits a pack index and a splice with `typename` as a base: `struct S : T...[1]`,
    // `struct C : typename [:r:]` (cp_parser_base_specifier in GCC).
    g.define(
        "base_class_clause",
        seq![
            ":",
            comma_sep1(seq![
                repeat(s!(attribute_declaration)),
                optional(choice![
                    s!(access_specifier),
                    seq![s!(access_specifier), optional("virtual")],
                    seq!["virtual", optional(s!(access_specifier))],
                ]),
                // A class-or-decltype, as in GCC `cp_parser_base_specifier` and Clang
                // `ParseBaseTypeSpecifier`: `struct A : decltype(f()) {};`.
                choice![
                    s!(_class_name),
                    s!(decltype),
                    s!(pack_index_specifier),
                    alias(s!(_typename_splice_type_specifier), s!(dependent_type)),
                ],
                optional("..."),
            ]),
        ],
    );
    // A splice type specifier with `typename` in a base specifier and in a member initializer,
    // where the grammar permits no other dependent type.
    g.define(
        "_typename_splice_type_specifier",
        seq!["typename", s!(splice_type_specifier)],
    );
    // Clang reads GNU attributes, standard attributes, and `__declspec` after `enum`, `enum class`,
    // and `enum struct`, as in a class head: `enum class __declspec(deprecated) E { A };`
    // (ParseDecl.cpp, ParseEnumSpecifier).
    g.define(
        "enum_specifier",
        prec_right(
            0,
            seq![
                "enum",
                optional(choice!["class", "struct"]),
                repeat(choice![
                    s!(attribute_specifier),
                    s!(attribute_declaration),
                    s!(ms_declspec_modifier)
                ]),
                choice![
                    seq![
                        field("name", s!(_class_name)),
                        optional(s!(_enum_base_clause)),
                        optional(field("body", s!(enumerator_list))),
                    ],
                    // An unnamed enumeration can have a base type: `enum : unsigned { A };`.
                    seq![optional(s!(_enum_base_clause)), field("body", s!(enumerator_list))],
                ],
                repeat(s!(attribute_specifier)),
            ],
        ),
    );
    // An elaborated enum-specifier has the keyword and a name, and no base clause and no body:
    // `auto f() -> enum E { return A; }` reads the brace as the function body.
    g.define(
        "_elaborated_enum_specifier",
        seq![
            "enum",
            optional(choice!["class", "struct"]),
            repeat(choice![
                s!(attribute_specifier),
                s!(attribute_declaration),
                s!(ms_declspec_modifier)
            ]),
            field("name", s!(_class_name)),
        ],
    );
    g.define(
        "_enum_base_clause",
        prec_left(
            0,
            seq![
                ":",
                // The base is a type-specifier-seq, as in GCC `cp_parser_enum_specifier`:
                // `enum E : typename T::type`, `enum E : decltype(x)`, `enum E : int_t<8>`.
                field(
                    "base",
                    choice![
                        alias(s!(qualified_type_identifier), s!(qualified_identifier)),
                        s!(_type_identifier),
                        s!(primitive_type),
                        s!(sized_type_specifier),
                        s!(template_type),
                        s!(dependent_type),
                        s!(decltype),
                    ]
                ),
            ],
        ),
    );
    // C++17 permits attributes after the name of an enumerator. GCC and Clang also read GNU attributes
    // there (GCC parser.cc, cp_parser_enumerator_definition. Clang ParseDecl.cpp, ParseEnumBody). A
    // macro in the place of a GNU attribute is an attribute macro:
    // `A Q_DECL_ENUMERATOR_DEPRECATED_X("x") = 1,`. A macro call as a full item stays a macro
    // invocation (`_macro_enumerator`).
    g.define(
        "enumerator",
        seq![
            field("name", s!(identifier)),
            repeat(choice![
                s!(attribute_declaration),
                s!(attribute_specifier),
                alias(s!(_enumerator_attribute_macro), s!(attribute_macro)),
            ]),
            optional(seq!["=", field("value", s!(expression))]),
        ],
    );
    // The external scanner gives the name of the macro only on the line of the enumerator name. A name on
    // a different line is an enumerator after a macro that expands to enumerators:
    // `LIST_TOKEN_TYPES` before `NUM_TOKEN_TYPES`. Right associativity gives the `(` after the name to the
    // arguments.
    g.define(
        "_enumerator_attribute_macro",
        prec_right(
            0,
            seq![
                field("name", alias(s!(_enumerator_macro_name), s!(identifier))),
                optional(field("arguments", s!(argument_list))),
            ],
        ),
    );
    // C++11 removes the `auto` storage class, and `auto` is a type.
    g.redefine("storage_class_specifier", |original| {
        let mut members: Vec<Rule> = original
            .into_members()
            .into_iter()
            .filter(|m| *m != Rule::from("auto"))
            .collect();
        members.push("thread_local".into());
        Rule::Choice(members)
    });
    g.define(
        "dependent_type",
        prec_dynamic(-1, prec_right(0, seq!["typename", s!(type_specifier)])),
    );
}

fn declarations(g: &mut Grammar) {
    g.define("module_name", seq![s!(identifier), repeat(seq![".", s!(identifier)])]);
    g.define("module_partition", seq![":", s!(module_name)]);
    g.define(
        "module_declaration",
        seq![
            optional("export"),
            "module",
            field("name", s!(module_name)),
            field("partition", optional(s!(module_partition))),
            optional(s!(attribute_declaration)),
            ";",
        ],
    );
    g.define(
        "export_declaration",
        prec(
            1,
            seq![
                "export",
                choice![s!(_export_item), s!(declaration_list), s!(import_declaration)]
            ],
        ),
    );
    g.define(
        "import_declaration",
        seq![
            "import",
            choice![
                field("name", s!(module_name)),
                field("partition", s!(module_partition)),
                field("header", choice![s!(string_literal), s!(system_lib_string)]),
            ],
            optional(s!(attribute_declaration)),
            ";",
        ],
    );
    g.define("global_module_fragment_declaration", seq!["module", ";"]);
    g.define(
        "private_module_fragment_declaration",
        seq!["module", ":", "private", ";"],
    );
    // One macro can come before `template`, for example an export macro of a module:
    // `HPX_CXX_EXPORT template <typename T> struct S;`. With more than one macro, the parser
    // cannot tell at the first macro if `template` or a type comes after them. The precedence of
    // `_macro_before_template` gives the identifier before `template` to the template declaration,
    // and not to a macro before a dependent type name `template A<T>::B`.
    g.define(
        "_macro_before_template",
        prec(1, prec_dynamic(-1, field("name", s!(identifier)))),
    );
    g.define(
        "template_declaration",
        seq![
            optional(alias(s!(_macro_before_template), s!(attribute_macro))),
            "template",
            field("parameters", s!(template_parameter_list)),
            optional(s!(requires_clause)),
            choice![
                s!(_empty_declaration),
                s!(alias_declaration),
                s!(declaration),
                s!(template_declaration),
                s!(function_definition),
                s!(concept_definition),
                s!(friend_declaration),
                alias(s!(constructor_or_destructor_declaration), s!(declaration)),
                alias(s!(constructor_or_destructor_definition), s!(function_definition)),
                alias(s!(operator_cast_declaration), s!(declaration)),
                alias(s!(operator_cast_definition), s!(function_definition)),
            ],
        ],
    );
    g.define(
        "template_instantiation",
        prec(
            1,
            seq![
                optional("extern"),
                "template",
                optional(s!(_declaration_specifiers)),
                // The explicit instantiation of a class has no declarator: `template class S<int>;`.
                optional(field("declarator", s!(_declarator))),
                ";",
            ],
        ),
    );
    g.define(
        "template_parameter_list",
        seq![
            "<",
            comma_sep(choice![
                s!(parameter_declaration),
                alias(
                    s!(_optional_template_parameter_declaration),
                    s!(optional_parameter_declaration)
                ),
                s!(type_parameter_declaration),
                s!(variadic_parameter_declaration),
                s!(variadic_type_parameter_declaration),
                s!(optional_type_parameter_declaration),
                s!(template_template_parameter_declaration),
            ]),
            closing_angle(),
        ],
    );
    // In a template parameter list the default can be a type, for the constrained parameter of
    // `template <C T = int>`. The dynamic precedences are those of a template argument list.
    g.define(
        "_optional_template_parameter_declaration",
        seq![
            s!(_parameter_declaration_specifiers),
            field(
                "declarator",
                optional(choice![s!(_declarator), s!(_abstract_declarator)])
            ),
            "=",
            field(
                "default_value",
                choice![
                    prec_dynamic(3, s!(type_descriptor)),
                    prec_dynamic(1, s!(expression)),
                    s!(initializer_list),
                ]
            ),
        ],
    );
    let type_keyword = || choice!["typename", "class"];
    g.define(
        "type_parameter_declaration",
        prec(1, seq![type_keyword(), optional(s!(_type_identifier))]),
    );
    g.define(
        "variadic_type_parameter_declaration",
        prec(1, seq![type_keyword(), "...", optional(s!(_type_identifier))]),
    );
    // The default of a type parameter is a type-id: `typename T = void *`, `class R = C const &`.
    // A bare type keeps the tree `default_type: type_specifier`. Before `,` and `>`, the
    // precedence gives a bare type to the parameter and not to a type descriptor.
    g.define(
        "optional_type_parameter_declaration",
        seq![
            type_keyword(),
            optional(field("name", s!(_type_identifier))),
            "=",
            field(
                "default_type",
                choice![prec(1, s!(type_specifier)), s!(type_descriptor)]
            ),
        ],
    );
    // A template head has a requires clause after its parameter list ([temp.param]):
    // `template<template<class U> requires C<U> class TT> struct X {};`. GCC reads it in
    // cp_parser_type_parameter (parser.cc:21048), and Clang in ParseTemplateTemplateParameter
    // (ParseTemplate.cpp:704).
    g.define(
        "template_template_parameter_declaration",
        seq![
            "template",
            field("parameters", s!(template_parameter_list)),
            optional(s!(requires_clause)),
            choice![
                s!(type_parameter_declaration),
                s!(variadic_type_parameter_declaration),
                s!(optional_type_parameter_declaration),
                // C++26: concept and variable template template parameters.
                s!(concept_parameter_declaration),
                s!(variable_template_parameter_declaration),
            ],
        ],
    );
    g.define(
        "concept_parameter_declaration",
        seq!["concept", optional("..."), optional(field("name", s!(identifier)))],
    );
    g.define(
        "variable_template_parameter_declaration",
        seq!["auto", optional("..."), optional(field("name", s!(identifier)))],
    );
    // The `...` of C varargs can follow the last parameter with no comma: `void f(int x...)`,
    // `void g(auto......)`. C++26 deprecates this form (P3176R1), and GCC parses it in
    // cp_parser_parameter_declaration_clause.
    g.define(
        "parameter_list",
        seq![
            "(",
            optional(seq![
                comma_sep1(choice![
                    s!(parameter_declaration),
                    s!(explicit_object_parameter_declaration),
                    s!(optional_parameter_declaration),
                    s!(variadic_parameter_declaration),
                    "...",
                ]),
                optional("..."),
            ]),
            ")",
        ],
    );
    // The precedence gives `(this [:r:] self)` to the explicit object parameter. The other
    // reading, a declaration with the constructor argument `this[:i:]`, is not in real code.
    //
    // Attributes can come before `this`: `void f([[maybe_unused]] this S &self)` (GCC
    // cp_parser_decl_specifier_seq, Clang ParseParameterDeclarationClause). The parameter takes the
    // specifier prefix of each other parameter, and the two forms then have one repeat symbol. After
    // the prefix, the parse state has an action for `this`, and the lexer reads the keyword. `this`
    // is not the name of an attribute macro and not the constraint of `auto`. GCC also reads a
    // specifier that is not an attribute there, and its analysis rejects it: "'this' must be the
    // first specifier in a parameter declaration".
    g.define(
        "explicit_object_parameter_declaration",
        prec(1, seq![specifier_prefix(), s!(this), s!(parameter_declaration)]),
    );
    g.define(
        "optional_parameter_declaration",
        seq![
            s!(_parameter_declaration_specifiers),
            // An unnamed parameter can have a default: `const T* = nullptr`, `Options = {}`.
            field(
                "declarator",
                optional(choice![s!(_declarator), s!(_abstract_declarator)])
            ),
            "=",
            field("default_value", choice![s!(expression), s!(initializer_list)]),
        ],
    );
    // In `(T...)` the `...` is a pack declarator, not the `...` of C varargs after an unnamed
    // parameter. The precedence selects the pack, as the standard does for a type with a pack.
    g.define(
        "variadic_parameter_declaration",
        prec(
            1,
            seq![
                s!(_parameter_declaration_specifiers),
                field("declarator", s!(_variadic_declarator)),
            ],
        ),
    );
    // The declarator-id of a pack, `...name`, or the `...` of an abstract pack declarator. In `T (...)`,
    // the parentheses hold C varargs, and not a pack in a grouping (Clang ParseParenDeclarator). The
    // precedence gives the `)` after `(...` to the parameter list.
    g.define(
        "variadic_declarator",
        choice![prec(-1, "..."), seq!["...", s!(identifier)]],
    );
    // The function `pack_declarators` defines this rule when the declarator rules are complete.
    g.define("_variadic_declarator", blank());
    // A function definition in the same list takes only the class of a function. For this reason,
    // each form names the classes. Refer to `declarator_classes`. A function declarator takes `=`,
    // as in GCC `cp_parser_init_declarator`: `int f() = 0;` has an initializer, and only the
    // compiler rejects it. A function declarator takes no braced or parenthesized initializer.
    // After the attribute macros of a declarator, a `(` belongs to the arguments of the last macro, and
    // the initializer is `= value` or a braced list: `std::atomic<int> x GUARDED_BY(mu){0};`. A data
    // member takes the same forms. The rule of a macro in the place of `constexpr` reads
    // `else if MACRO (c) { return x; }` with no error (398f1e2), and the error recovery of that form
    // no longer reads the block as a braced list.
    g.redefine("init_declarator", |original| {
        choice![
            replace_rule(original, &s!(_declarator), &declarator_of_each_class("_declarator")),
            seq![
                field("declarator", declarator_of_variable("_declarator")),
                c::asm_label(),
                field("value", choice![s!(argument_list), s!(initializer_list)]),
            ],
            seq![
                field("declarator", alias(s!(_macro_attributed_declarator), s!(attributed_declarator))),
                choice![
                    seq!["=", field("value", choice![s!(initializer_list), s!(expression)])],
                    field("value", s!(initializer_list)),
                ],
            ],
        ]
    });
    pointer_init_declarators(g);
    // Without name lookup, `Foo bar BAZ(x);` has more readings than the macro `BAZ` after the name `bar`
    // of the type `Foo`. The macro `Foo` can come before the type `bar`, and `BAZ(x)` then declares a
    // function, or in a namespace or a block a variable with an initializer. These readings have the
    // dynamic precedence of an attribute macro, -1. The first macro after the declarator decides the
    // dynamic precedence of the macros.
    //
    // In a namespace or a block, the first macro has the precedence -3, and the other readings stay:
    // `BOOST_CONSTEXPR_OR_CONST DWORD_ DELETE_ = DELETE;`, `LIBC_INLINE_VAR Frac128 PI_OVER_2_M1(1);`. In
    // a class, a first macro with arguments has the precedence 1: `Mutex mu; Foo bar GUARDED_BY(mu);`.
    // In the corpus, nearly all such members are data members with thread safety macros. A first macro
    // with no arguments keeps the precedence -3: `static TORCH_API FnPtr AVX512;`. Right associativity
    // gives the `(` after the name to the arguments.
    //
    // The external scanner reads the names of these macros (`_declarator_macro_name`). It reads the text
    // after them up to `;`, `,`, `=`, `{`, or `:`, and the arguments of each macro are expressions that
    // are not empty. In `T min BOOST_PREVENT_MACRO_SUBSTITUTION () const;`, the `()` is a parameter list.
    let name = || field("name", alias(s!(_declarator_macro_name), s!(identifier)));
    let arguments = || field("arguments", alias(s!(_attribute_macro_arguments), s!(argument_list)));
    let first_macro = |with_arguments: i32| {
        prec_right(
            0,
            choice![
                prec_dynamic(-3, name()),
                prec_dynamic(with_arguments, seq![name(), arguments()]),
            ],
        )
    };
    g.define("_declarator_attribute_macro", first_macro(-3));
    g.define("_field_declarator_attribute_macro", first_macro(1));
    g.define(
        "_next_declarator_attribute_macro",
        prec_right(0, seq![name(), optional(arguments())]),
    );
    g.define(
        "_attribute_macro_arguments",
        seq![
            "(",
            comma_sep1(choice![s!(expression), s!(initializer_list), s!(compound_statement)]),
            ")",
        ],
    );
    // The name and the declarator of a conversion function: `operator int() const`, `operator T *() &&`.
    // The declarator is a function declarator ([class.conv.fct]), and the parameter list is necessary. GCC
    // cp_parser_direct_declarator finds the conversion function (`sfk_conversion`) only before a parameter
    // list, and `operator int;` is an error. Refer to `conversion_function_declarators`. A name with no
    // parameter list, as in `using Base::operator bool;`, is `_conversion_function_id`.
    //
    // A conversion-type-id is a type-specifier-seq with a conversion-declarator ([class.conv.fct]), and
    // it defines no class and no enumeration. `operator struct S();` names the class `S`. GCC
    // `cp_parser_conversion_type_id` (parser.cc:19670) stops a definition with the message "types may
    // not be defined in a conversion-type-id" (parser.cc:19683). Clang gives DSC_conv_operator the
    // value AllowDefiningTypeSpec::No (Parser.h:1666).
    g.define(
        "operator_cast",
        prec_right(
            1,
            seq![
                "operator",
                s!(_conversion_declaration_specifiers),
                field("declarator", s!(_conversion_function_declarator)),
            ],
        ),
    );
    // `export { int x; }` can start a declaration list or a compound statement, and the items of the
    // two lists are different rules. The precedence gives each item to the declaration list.
    g.define(
        "declaration_list",
        seq![open_brace(), repeat(prec(1, s!(_declaration_list_item))), close_brace()],
    );
    // The precedence gives `{}` to the other list where a compound statement can also come: to an
    // initializer list in the arguments `f({})`, and to a declaration list in `export {}`.
    //
    // The GNU label declarations come before the items of the block: `{ __label__ a, b; goto a; a:; }`.
    // GCC and Clang read them only there, in C and in C++ (c-parser.cc,
    // c_parser_compound_statement_nostart. parser.cc, cp_parser_compound_statement. ParseStmt.cpp,
    // ParseCompoundStatementBody).
    g.define(
        "compound_statement",
        prec(
            -1,
            seq![
                open_brace(),
                repeat(s!(label_declaration)),
                repeat(s!(_block_item)),
                close_brace()
            ]
        ),
    );
    g.define("field_initializer_list", seq![":", comma_sep1(s!(field_initializer))]);
    // C++26 permits a pack index and a splice as the base that a member initializer names:
    // `S() : T...[1]() {}`, `B() : typename [:r:]() {}` (cp_parser_mem_initializer_id in GCC).
    // A mem-initializer-id is a class-or-decltype, and a decltype-specifier names the base:
    // `CI() : decltype(Bx())() {}` (GCC `cp_parser_mem_initializer_id`, Clang
    // `ParseMemInitializer` with the token `annot_decltype`).
    g.define(
        "field_initializer",
        prec(
            1,
            seq![
                choice![
                    s!(_field_identifier),
                    s!(template_method),
                    alias(s!(qualified_field_identifier), s!(qualified_identifier)),
                    s!(pack_index_specifier),
                    s!(splice_type_specifier),
                    alias(s!(_typename_splice_type_specifier), s!(dependent_type)),
                    s!(decltype),
                ],
                choice![s!(initializer_list), s!(argument_list)],
                optional("..."),
            ],
        ),
    );
    g.redefine("_field_declaration_list_item", |original| {
        choice![
            original,
            s!(template_declaration),
            alias(s!(inline_method_definition), s!(function_definition)),
            alias(s!(constructor_or_destructor_definition), s!(function_definition)),
            alias(s!(constructor_or_destructor_declaration), s!(declaration)),
            alias(s!(operator_cast_definition), s!(function_definition)),
            alias(s!(operator_cast_declaration), s!(declaration)),
            s!(friend_declaration),
            // Clang reads GNU attributes before the colon: `public __attribute__((annotate("x"))):`
            // (ParseDeclCXX.cpp, ParseCXXClassMemberDeclarationWithPragmas).
            seq![
                choice![
                    s!(access_specifier),
                    alias(s!(_qt_access_specifier), s!(access_specifier))
                ],
                repeat(s!(attribute_specifier)),
                ":",
            ],
            s!(alias_declaration),
            s!(using_declaration),
            s!(type_definition),
            s!(static_assert_declaration),
            s!(consteval_block_declaration),
            alias(s!(_ms_if_exists_in_field_declaration_list), s!(ms_if_exists)),
            ";",
            // `struct S { __extension__ ; };` is a member declaration with only the `;`. GCC
            // `cp_parser_member_declaration` (parser.cc:30932) reads the keyword and then the member
            // declaration again, and the `;` is such a declaration.
            s!(_extension_empty_declaration),
        ]
    });
    // A calling convention can come before the declarator of a member function, as it can in a
    // declaration: `ULONG __stdcall AddRef();` (Clang ParseDecl.cpp, ParseDeclarationSpecifiers).
    // A member function definition in the same list takes only the class of a function. For this
    // reason, each form names the classes. Refer to `declarator_classes`.
    g.define(
        "field_declaration",
        seq![
            s!(_declaration_specifiers),
            comma_sep(choice![
                // An unnamed bit-field has no declarator: `int : 0;`. GCC
                // `cp_parser_member_declaration` and Clang
                // `ParseCXXMemberDeclaratorBeforeInitializer` read a `:` in place of a declarator.
                // GCC also reads attributes before the `:`: `int b UNUSED : 3;`. Clang rejects them.
                // GCC and Clang read attributes after the width: `bool f : 1 GUARDED_BY(mu);`. The
                // width before a macro is a number. After a width expression, the token of a macro
                // name would change the parse states of each expression.
                seq![
                    optional(field(
                        "declarator",
                        choice![
                            declarator_of_each_class("_field_declarator"),
                            alias(s!(_macro_attributed_field_declarator), s!(attributed_declarator)),
                        ]
                    )),
                    choice![
                        s!(bitfield_clause),
                        seq![
                            alias(s!(_bitfield_width_before_macro), s!(bitfield_clause)),
                            declarator_attribute_macros("_field_declarator_attribute_macro"),
                        ],
                    ],
                ],
                seq![
                    optional(call_macro()),
                    optional(s!(ms_call_modifier)),
                    field(
                        "declarator",
                        choice![
                            declarator_of_variable("_field_declarator"),
                            alias(s!(_macro_attributed_field_declarator), s!(attributed_declarator)),
                        ]
                    ),
                    // A member takes a GNU asm label: `struct S { static int x asm("s"); };`. GCC
                    // reads it in `cp_parser_member_declaration` (parser.cc:31390) and Clang in
                    // `ParseCXXMemberDeclaratorBeforeInitializer` (ParseDeclCXX.cpp:2620). The
                    // attributes after the label are the attributes before the `;` of this rule.
                    optional(s!(gnu_asm_expression)),
                    optional(choice![
                        // C++20 permits a default member initializer after the width of a
                        // bit-field. The width stops before the brace: in `int x : N {2};` the
                        // width is `N` and the initializer is `{2}`.
                        seq![
                            alias(s!(_bitfield_width_before_initializer), s!(bitfield_clause)),
                            field("default_value", s!(initializer_list)),
                        ],
                        // The width is a constant-expression, and `=` ends it: `int x : N = 2;`.
                        seq![
                            s!(bitfield_clause),
                            "=",
                            field("default_value", choice![s!(expression), s!(initializer_list)]),
                        ],
                        s!(_default_member_initializer),
                    ]),
                ],
                // A member function takes a pure-specifier and no braced initializer:
                // `virtual void f() = 0;`. In `void f() {};` the brace starts the body.
                seq![
                    optional(call_macro()),
                    optional(s!(ms_call_modifier)),
                    field("declarator", s!(_field_declarator_of_function)),
                    optional(s!(gnu_asm_expression)),
                    optional(seq![
                        "=",
                        field("default_value", choice![s!(expression), s!(initializer_list)])
                    ]),
                ],
            ]),
            optional(s!(attribute_specifier)),
            ";",
        ],
    );
    // The width before an attribute macro is a number. After a width expression, the token of a
    // macro name would change the parse states of each expression.
    g.define("_bitfield_width_before_macro", seq![":", s!(number_literal)]);
    // The width of a bit-field stops before the brace of a default member initializer.
    // `cp_parser_member_declaration` reads the width with `cp_parser_constant_expression`
    // (parser.cc:31276) and reads the initializer after it at a `{` or an `=` (parser.cc:31282 to
    // :31296). `ParseCXXMemberDeclaratorBeforeInitializer` reads the width with
    // `ParseConstantExpression` (ParseDeclCXX.cpp:2599), and the caller reads the initializer.
    //
    // A brace after a name is also a functional cast, and only name lookup separates `N{1}` from
    // the width `N` with the initializer `{1}`. The dynamic precedence gives the width and the
    // initializer, which is the reading of the two front ends for a value N. For a type N the two
    // front ends read the width `N{1}`, and no rule of the grammar can tell the two apart. The
    // reading also holds for `int b : N {};`, where the braces are empty.
    g.define(
        "_bitfield_width_before_initializer",
        prec_dynamic(1, seq![":", prec_right(CONDITIONAL, s!(expression))]),
    );
    // The width of a bit-field is a constant-expression, which is a conditional-expression
    // ([class.mem], GCC `cp_parser_member_declaration`, Clang
    // `ParseCXXMemberDeclaratorBeforeInitializer`). The precedence of a conditional ends the width
    // before `=`. A `?` continues it, and in `int y : c ? 8 : a = 42;` the width is `c ? 8 : a = 42`.
    g.define(
        "bitfield_clause",
        seq![":", prec_right(CONDITIONAL, s!(expression))],
    );
    g.define(
        "_default_member_initializer",
        choice![
            field("default_value", s!(initializer_list)),
            seq![
                "=",
                field("default_value", choice![s!(expression), s!(initializer_list)])
            ],
        ],
    );
    // The declarator declares a function, as in `function_definition`.
    g.define(
        "inline_method_definition",
        seq![
            s!(_declaration_specifiers),
            optional(call_macro()),
            optional(s!(ms_call_modifier)),
            field("declarator", s!(_field_declarator_of_function)),
            choice![
                field("body", choice![s!(compound_statement), s!(try_statement)]),
                s!(default_method_clause),
                s!(delete_method_clause),
                s!(pure_virtual_clause),
            ],
        ],
    );
    g.define(
        "_constructor_specifiers",
        choice![s!(_declaration_modifiers), s!(explicit_function_specifier)],
    );
    // The specifiers of a conversion function can include macros: `LIBC_INLINE operator T()`.
    let cast_specifiers = || {
        repeat(choice![
            s!(_constructor_specifiers),
            s!(attribute_macro),
            macro_call_attribute()
        ])
    };
    let cast_declarator = || {
        field(
            "declarator",
            choice![
                s!(operator_cast),
                alias(s!(qualified_operator_cast_identifier), s!(qualified_identifier)),
            ],
        )
    };
    g.define(
        "operator_cast_definition",
        seq![
            c::extension_prefix(),
            cast_specifiers(),
            cast_declarator(),
            field("body", choice![s!(compound_statement), s!(try_statement)]),
        ],
    );
    // The GNU prefix comes before the precedence. A precedence on the first step of a declaration
    // would take the token from the other readings, because the parse table builder compares the
    // precedences before it reads the conflict sets.
    g.define(
        "operator_cast_declaration",
        seq![
            c::extension_prefix(),
            prec(
                1,
                seq![
                    cast_specifiers(),
                    cast_declarator(),
                    choice![
                        seq![optional(seq!["=", field("default_value", s!(expression))]), ";"],
                        // `explicit operator bool() const = delete;`
                        s!(default_method_clause),
                        s!(delete_method_clause),
                    ],
                ],
            ),
        ],
    );
    g.define(
        "constructor_try_statement",
        seq![
            "try",
            optional(s!(field_initializer_list)),
            field("body", s!(compound_statement)),
            repeat1(s!(catch_clause)),
        ],
    );
    // The specifiers and the declarator of a constructor or a destructor. A declarator after a
    // macro starts with a name: `LLVM_ABI explicit A(int);`. A parenthesized declarator after a
    // macro would fork the parse at each call `f(x)`.
    //
    // The external scanner emits `_constructor_macro_start` before the first macro when the name
    // after the macros is the name of a class head that it recorded (refer to `_class_head_mark`), or
    // when the name is `A::A` or `A::~A`. The parser then does not fork, and a macro before a
    // constructor is not a return type: `LLVM_ABI A();`, `simdjson_inline A::A() {}`.
    let named_declarator = || {
        field(
            "declarator",
            alias(s!(_named_function_declarator), s!(function_declarator)),
        )
    };
    // A calling convention can come before a constructor outside its class, as before the specifiers
    // of a declaration: `__thiscall I::I() {}`.
    let constructor_head = || {
        seq![
            c::extension_prefix(),
            optional(s!(ms_call_modifier)),
            repeat(s!(_constructor_specifiers)),
            choice![
                field("declarator", s!(function_declarator)),
                seq![
                    repeat1(seq![
                        choice![s!(attribute_macro), macro_call_attribute()],
                        repeat(s!(_constructor_specifiers)),
                    ]),
                    named_declarator(),
                ],
                seq![
                    alias(s!(_constructor_first_macro), s!(attribute_macro)),
                    repeat(s!(_constructor_specifiers)),
                    repeat(seq![
                        choice![
                            alias(s!(_constructor_attribute_macro), s!(attribute_macro)),
                            macro_call_attribute(),
                        ],
                        repeat(s!(_constructor_specifiers)),
                    ]),
                    named_declarator(),
                ],
            ],
        ]
    };
    // The empty token is a child of the first macro. The error recovery then pops the same number of
    // stack entries as for a macro without the token. The first macro can have arguments:
    // `DEPRECATED("x") S(double);`.
    g.define(
        "_constructor_first_macro",
        seq![
            s!(_constructor_macro_start),
            field("name", s!(identifier)),
            optional(field("arguments", s!(argument_list))),
        ],
    );
    g.define("_constructor_attribute_macro", field("name", s!(identifier)));
    g.define(
        "_named_function_declarator",
        prec_dynamic(
            1,
            seq![
                field(
                    "declarator",
                    choice![
                        s!(identifier),
                        s!(qualified_identifier),
                        s!(template_function),
                        s!(destructor_name),
                        s!(operator_name),
                    ]
                ),
                s!(_function_declarator_seq),
            ],
        ),
    );
    g.define(
        "constructor_or_destructor_definition",
        seq![
            constructor_head(),
            choice![
                seq![
                    optional(s!(field_initializer_list)),
                    field("body", s!(compound_statement))
                ],
                alias(s!(constructor_try_statement), s!(try_statement)),
                s!(default_method_clause),
                s!(delete_method_clause),
                s!(pure_virtual_clause),
            ],
        ],
    );
    g.define("constructor_or_destructor_declaration", seq![constructor_head(), ";"]);
    // A deduction guide at namespace scope: `S(const char *) -> S<std::string>;`. Refer to
    // `DEDUCTION_GUIDE`. The node is a declaration with a function declarator, as the guide in a class
    // body and after a template head. The specifiers are the specifiers of a constructor.
    g.define(
        "_deduction_guide_declaration",
        seq![
            c::extension_prefix(),
            prec_dynamic(
                DEDUCTION_GUIDE,
                seq![
                    repeat(s!(_constructor_specifiers)),
                    field(
                        "declarator",
                        alias(s!(_deduction_guide_declarator), s!(function_declarator))
                    ),
                    ";",
                ],
            ),
        ],
    );
    // The function declarator of a deduction guide has the parts of `_function_declarator_seq`, and the
    // trailing return type is necessary. The front ends read each part, and their semantic analysis
    // rejects the parts that a guide cannot have (GCC decl.cc, grokdeclarator. Clang SemaDeclCXX.cpp,
    // CheckDeductionGuideDeclarator).
    g.define(
        "_deduction_guide_declarator",
        prec_dynamic(
            1,
            seq![
                field("declarator", s!(_declarator_of_name)),
                field("parameters", s!(parameter_list)),
                optional(s!(_function_attributes_start)),
                optional(s!(ref_qualifier)),
                optional(s!(_function_exception_specification)),
                optional(s!(_function_attributes_end)),
                alias(s!(_deduction_guide_return_type), s!(trailing_return_type)),
                optional(s!(_function_postfix)),
            ],
        ),
    );
    // [temp.deduct.guide] gives a deduction guide the form `template-name (parameters) ->
    // simple-template-id ;`. The return type of a guide is a specialization of the template that the
    // name of the guide gives, and the name of that template takes no scope. Clang
    // `CheckDeductionGuideDeclarator` (SemaDeclCXX.cpp) reads the same form. At namespace scope,
    // `BENCHMARK(b)->Apply(f);` and `T(x)->run();` then stay macro statements.
    g.define(
        "_deduction_guide_return_type",
        seq!["->", alias(s!(_deduction_guide_class), s!(type_descriptor))],
    );
    g.define("_deduction_guide_class", field("type", s!(template_type)));
    // A defaulted or a deleted definition has a positive dynamic precedence. At namespace scope,
    // `S::S() = default;` is also an expression statement that assigns `default` to a call. The
    // parse version of the expression has no action for the keyword, and the lexer gives it an
    // identifier. The two readings have equal other precedences. GCC `cp_parser_toplevel_declaration`
    // and Clang `ParseExternalDeclaration` read no expression at namespace scope.
    g.define("default_method_clause", prec_dynamic(1, seq!["=", "default", ";"]));
    // C++26 permits a reason: `= delete("use g")`. The precedence prefers the reason to a
    // default value `delete ("use g")`, which deletes a string literal and is ill-formed. The
    // dynamic precedence prefers the definition to the expression `S::S() = delete ("use g")`.
    g.define(
        "delete_method_clause",
        prec(
            1,
            prec_dynamic(1, seq!["=", "delete", optional(seq!["(", s!(_string), ")"]), ";"]),
        ),
    );
    g.define("pure_virtual_clause", seq!["=", re("0"), ";"]);
    g.define(
        "friend_declaration",
        choice![
            seq![
                c::extension_prefix(),
                // Specifiers, attributes, and attribute macros can come before `friend`:
                // `inline friend void f();`, `ALWAYS_INLINE friend bool operator==(A, A) {}`.
                repeat(choice![
                    s!(_declaration_modifiers),
                    s!(attribute_macro),
                    macro_call_attribute()
                ]),
                "friend",
                choice![
                    s!(declaration),
                    s!(function_definition),
                    // C++26 permits a list with packs: `friend A, Ts...;`. A friend type specifier
                    // can also be a fundamental type: `friend int, long;` (the variadic friend
                    // declarations of ParseCXXClassMemberDeclaration in Clang).
                    seq![
                        comma_sep1(seq![
                            choice![
                                seq![
                                    optional(choice![
                                        "class",
                                        "struct",
                                        "union",
                                        "__interface",
                                        "typename"
                                    ]),
                                    s!(_class_name),
                                ],
                                s!(primitive_type),
                                s!(sized_type_specifier),
                            ],
                            optional("..."),
                        ]),
                        ";",
                    ],
                ],
            ],
            // `friend` can come after the type: `bool friend operator==(A, A) {}`. GCC reads
            // `friend` as one decl-specifier in `cp_parser_decl_specifier_seq`, in each position.
            alias(s!(_friend_after_type_definition), s!(function_definition)),
            alias(s!(_friend_after_type_declaration), s!(declaration)),
        ],
    );
    g.define(
        "_friend_after_type_definition",
        seq![
            s!(_declaration_specifiers),
            "friend",
            field("declarator", s!(_declarator_of_function)),
            choice![
                field("body", choice![s!(compound_statement), s!(try_statement)]),
                s!(default_method_clause),
                s!(delete_method_clause),
            ],
        ],
    );
    g.define(
        "_friend_after_type_declaration",
        seq![
            s!(_declaration_specifiers),
            "friend",
            comma_sep1(field("declarator", s!(_declarator))),
            ";",
        ],
    );
    g.define("access_specifier", choice!["public", "private", "protected"]);
    // Qt adds `slots` after an access specifier and `signals` in its place: `public slots:`,
    // `signals:`. The Qt words are keywords only in a member list (clang-format:
    // UnwrappedLineParser.cpp, parseAccessSpecifier).
    g.define(
        "_qt_access_specifier",
        choice![
            seq![choice!["public", "private", "protected"], choice!["slots", "Q_SLOTS"]],
            "signals",
            "Q_SIGNALS",
        ],
    );
    g.redefine("_declarator", |original| {
        choice![
            original,
            s!(reference_declarator),
            s!(qualified_identifier),
            s!(template_function),
            s!(operator_name),
            alias(s!(_plain_destructor_name), s!(destructor_name)),
            s!(block_pointer_declarator),
            s!(member_pointer_declarator),
            alias(s!(_aligned_name_declarator), s!(attributed_declarator)),
        ]
    });
    g.redefine("_field_declarator", |original| {
        choice![
            original,
            alias(s!(reference_field_declarator), s!(reference_declarator)),
            s!(template_method),
            s!(operator_name),
            alias(s!(block_pointer_field_declarator), s!(block_pointer_declarator)),
            alias(s!(member_pointer_field_declarator), s!(member_pointer_declarator)),
            alias(s!(_aligned_field_name_declarator), s!(attributed_declarator)),
        ]
    });
    g.redefine("_type_declarator", |original| {
        choice![
            original,
            alias(s!(reference_type_declarator), s!(reference_declarator)),
            alias(s!(block_pointer_type_declarator), s!(block_pointer_declarator)),
            alias(s!(member_pointer_type_declarator), s!(member_pointer_declarator)),
        ]
    });
    g.redefine("_abstract_declarator", |original| {
        choice![
            original,
            s!(abstract_reference_declarator),
            s!(abstract_member_pointer_declarator),
            s!(abstract_block_pointer_declarator),
        ]
    });
    // A parameter with an abstract function declarator has a lower dynamic precedence. In a block,
    // `T x(g<U>(y));` is then a variable with an initializer, and not a function with a parameter of
    // the function type `g<U>(y)`. Only name lookup tells the two apart, and real code has the call.
    // The named declarator names each class. The parser then reads a GNU attribute after the
    // declarator before it selects the parameter or an attributed declarator, as
    // `attributed_declarator` says. Refer to `declarator_classes`. The specifiers of a parameter have
    // no precedence for a type that is a keyword. Refer to `KEYWORD_TYPE`.
    g.redefine("parameter_declaration", |original| {
        let members = choice![
            s!(abstract_pointer_declarator),
            prec_dynamic(-2, s!(abstract_function_declarator)),
            s!(abstract_array_declarator),
            s!(abstract_parenthesized_declarator),
            s!(abstract_reference_declarator),
            s!(abstract_member_pointer_declarator),
            s!(abstract_block_pointer_declarator),
        ];
        let named = replace_rule(original, &s!(_abstract_declarator), &members);
        let specifiers = replace_rule(
            named,
            &s!(_declaration_specifiers),
            &s!(_parameter_declaration_specifiers),
        );
        replace_rule(specifiers, &s!(_declarator), &declarator_of_each_class("_declarator"))
    });
    // A Clang block pointer: `void (^callback)(int)`, `void (^)(int)`. Clang reads `^` as it reads
    // `*`, with the same qualifiers (ParseDecl.cpp, ParseDeclaratorInternal).
    let block_pointer = |declarator: Rule| {
        prec_dynamic(
            1,
            prec_right(
                0,
                seq![
                    "^",
                    repeat(choice![s!(type_qualifier), s!(attribute_specifier)]),
                    field("declarator", declarator),
                ],
            ),
        )
    };
    g.define("block_pointer_declarator", block_pointer(s!(_declarator)));
    g.define("block_pointer_field_declarator", block_pointer(s!(_field_declarator)));
    g.define("block_pointer_type_declarator", block_pointer(s!(_type_declarator)));
    g.define(
        "abstract_block_pointer_declarator",
        block_pointer(optional(s!(_abstract_declarator))),
    );
    // A GNU attribute can follow the declarator of a variable: `int x __attribute__((unused));`.
    // A parameter and a function declarator have their own trailing GNU attributes, and the
    // lower precedence gives the attribute to them.
    g.define(
        "attributed_declarator",
        prec_right(
            0,
            seq![
                s!(_declarator),
                repeat1(choice![s!(attribute_declaration), prec(-1, s!(attribute_specifier))])
            ],
        ),
    );
    // An alignment-specifier is an attribute-specifier, and it can follow the name of a variable
    // or a field: `int y alignas(16);`, `char x alignas(4)[8];`. GCC reads it in
    // `cp_parser_std_attribute_spec`. These rules take only a name before `alignas`. After the
    // attributes of a function declarator, `alignas` is a qualifier of the function.
    g.define(
        "_aligned_name_declarator",
        seq![s!(identifier), repeat1(s!(alignas_qualifier))],
    );
    g.define(
        "_aligned_field_name_declarator",
        seq![s!(_field_identifier), repeat1(s!(alignas_qualifier))],
    );
    // The `ptr-operator` of a pointer to a member: `C::*`, `::ns::C<T>::*`. GCC reads it in
    // `cp_parser_ptr_operator`, and Clang in `ParseDeclaratorInternal`. The external scanner
    // gives `_member_pointer_start`, a token with no width, only before a scope that ends with
    // `::*` and a name or the `...` of a pack after the `*`. A qualified name such as `a::b::C<int>` does not enter this
    // rule. Its parse and its error recovery stay the same. An abstract declarator such as
    // `(C::*)` gets no token, and `abstract_member_pointer_declarator` reads it.
    g.define(
        "_member_pointer_scope",
        seq![s!(_member_pointer_start), repeat1(s!(_scope_resolution)), "*"],
    );
    let member_pointer = |declarator: Rule| {
        prec_dynamic(
            1,
            prec_right(
                0,
                seq![
                    s!(_member_pointer_scope),
                    member_pointer_qualifiers(),
                    field("declarator", declarator),
                ],
            ),
        )
    };
    // A pointer to a member: `int C::*p`, `bool (item::*filter)() const`, `typedef int C::*P`.
    g.define("member_pointer_declarator", member_pointer(s!(_declarator)));
    g.define("member_pointer_field_declarator", member_pointer(s!(_field_declarator)));
    g.define("member_pointer_type_declarator", member_pointer(s!(_type_declarator)));
    // MSVC reads a list of specifiers in `__declspec`. Each is a name or a string with optional
    // arguments, and a comma or a space divides them: `__declspec(align(16))`,
    // `__declspec(dllexport noinline)` (Clang ParseDecl.cpp, ParseMicrosoftDeclSpecs).
    g.define(
        "ms_declspec_modifier",
        seq![
            "__declspec",
            "(",
            repeat(choice![
                seq![s!(identifier), optional(s!(argument_list))],
                s!(string_literal),
                ",",
            ]),
            ")",
        ],
    );
    // MSVC and Clang read a calling convention among the qualifiers after `*`, and GCC and Clang
    // read GNU attributes there: `void* __cdecl operator new(size_t)`, `void *__attribute__((weak))
    // f()` (Clang ParseDecl.cpp, ParseTypeQualifierListOpt. GCC parser.cc, cp_parser_declarator).
    g.redefine("ms_call_modifier", |original| choice![original, "__regcall"]);
    let pointer_qualifiers = || {
        repeat(choice![
            s!(type_qualifier),
            s!(ms_call_modifier),
            s!(attribute_specifier)
        ])
    };
    let with_pointer_qualifiers = |member: Rule| {
        if member == repeat(s!(type_qualifier)) {
            pointer_qualifiers()
        } else {
            member
        }
    };
    // GCC and Clang read GNU attributes after the `(` of a grouping, and Clang also reads a calling
    // convention there: `void (__attribute__((unused)) *p)(int)`, `void (__stdcall *f)(int)` (GCC
    // parser.cc, cp_parser_declarator. Clang ParseDecl.cpp, ParseParenDeclarator). A macro can take
    // the place of the calling convention: `typedef void (WINAPI *F)(void *);`,
    // `typedef BOOL (WINAPI F_t)(int);`.
    let calling_convention = || choice![s!(ms_call_modifier), grouping_call_macro()];
    let grouping_start = || {
        choice![
            seq![s!(_grouping_attributes), optional(calling_convention())],
            calling_convention(),
        ]
    };
    // One rule holds the attributes of each grouping, so that each conflict set of these attributes
    // names one rule. A rule that is only a repeat becomes an auxiliary rule of each grouping.
    g.define(
        "_grouping_attributes",
        seq![s!(attribute_specifier), repeat(s!(attribute_specifier))],
    );
    // The dynamic precedence of the C grammar gives `f(*p);` in a block to a call. A grouping with
    // attributes or a calling convention is not an expression, and [dcl.ambig.res] makes it a
    // declarator: `void cb(DWORD (WINAPI *fn)(LPVOID));` declares a function with a parameter. The
    // grouping of a name with attributes only gets a different precedence. Refer to
    // `name_grouping_with_attributes`.
    let grouping = |declarator: &str| {
        choice![
            prec_dynamic(PAREN_DECLARATOR, seq!["(", sym(declarator), ")"]),
            prec_dynamic(
                GROUPING_WITH_ATTRIBUTES,
                seq![
                    "(",
                    optional(s!(_grouping_attributes)),
                    calling_convention(),
                    sym(declarator),
                    ")"
                ]
            ),
            prec_dynamic(
                GROUPING_WITH_ATTRIBUTES,
                seq!["(", s!(_grouping_attributes), sym(declarator), ")"]
            ),
        ]
    };
    g.define("parenthesized_declarator", grouping("_declarator"));
    g.define("parenthesized_field_declarator", grouping("_field_declarator"));
    g.define("parenthesized_type_declarator", grouping("_type_declarator"));
    g.define(
        "abstract_parenthesized_declarator",
        prec(
            1,
            seq!["(", optional(grouping_start()), s!(_abstract_declarator), ")"],
        ),
    );
    // The external scanner reads the name of the macro only before a declarator, the `)` of the
    // grouping, and a parameter list. The calling convention applies to a function type. In a block,
    // `f(SIZE * n);` then stays a call. Refer to `scan_grouping_call_macro_name` in `src/scanner.c`.
    g.define(
        "_grouping_call_attribute_macro",
        field("name", alias(s!(_grouping_call_macro_name), s!(identifier))),
    );
    for name in [
        "pointer_declarator",
        "pointer_field_declarator",
        "pointer_type_declarator",
        "abstract_pointer_declarator",
        // The pointers before a parenthesized initializer take the same qualifiers. With a different
        // list, `* const` before a name has two repeat symbols, and the parser cannot select one. The
        // pointer of a pack is a copy of `pointer_declarator`. Refer to `pack_declarators`.
        "_pointer_name_declarator",
        "_nested_pointer_name_declarator",
    ] {
        g.redefine(name, |original| edit_members(original, &with_pointer_qualifiers));
    }
    // A pointer to a member in an abstract declarator: `void (S::*)()`, `int (::ns::S::*)`. The
    // scope has a name before each `::`, as the nested-name-specifier of the standard has. An
    // abstract declarator can start after each type argument of a template.
    //
    // The steps are inline, and they are not `_scope_resolution`, the steps of a qualified name.
    // DO NOT WRITE `repeat1(s!(_scope_resolution))` HERE. The generator then stops with three
    // unresolved conflicts, and the last one needs a conflict set of `_scope_resolution` alone.
    // `_scope_resolution` takes an empty scope, and a `repeat1` of it reads `A:: ::` in two ways.
    // Such a set stops the report of a conflict in each qualified name, which is the defect that
    // the check of `cargo xtask generate` removes.
    //
    // A wider scope choice gains no measured file. `::template V<int>::*` is in 4 corpus files, and
    // each use is a member pointer with a name, which `_member_pointer_scope` reads.
    // `P...[0]::*` is in 0 corpus files, and only GCC accepts it.
    //
    // The older reason for the inline steps was the version limit of the parser. With
    // `_scope_resolution`, the parse states of `a<b::c<d::e<f>>>` change. The parser then removed
    // the correct parse of approximately 200 corpus files. The runtime repairs that limit:
    // `ts_parser__merge_finished_version` in vendor/tree-sitter/src/parser.c merges those versions.
    // Refer to upstream-patches/11-merge-finished-version.patch.
    //
    // The scope steps have the precedence of `_scope_resolution`, and the steps after the `*` keep
    // the precedence 0. With the precedence 0 after a leading `::`, the parse table builder removes
    // the step of this rule with no report, because the reduction of a `_scope_resolution` with an
    // empty scope has the precedence 1. `void f(int ::ns::T::*);`, `void f(int ::S::*);` and
    // `void f(void (::ns::T::*)());` then got a MISSING node. GCC and Clang accept each of the
    // three. The precedence of the reduction stays 0: `int S::* (S::*)(int) const` is a pointer to
    // a member of a function type, and the function declarator is inside the outer member pointer.
    g.define(
        "abstract_member_pointer_declarator",
        prec_dynamic(
            1,
            prec_right(
                0,
                seq![
                    // The precedence covers the leading `::` and the steps after it. The last step
                    // of the region keeps the precedence 0, because the `*` follows it.
                    prec(
                        1,
                        seq![
                            optional("::"),
                            // Each step has the precedence of `_scope_resolution`, so that a second
                            // `::` does not prefer a qualified name.
                            repeat1(prec(
                                1,
                                seq![
                                    field(
                                        "scope",
                                        choice![
                                            s!(_namespace_identifier),
                                            s!(template_type),
                                            s!(decltype)
                                        ]
                                    ),
                                    "::",
                                ]
                            )),
                        ]
                    ),
                    "*",
                    member_pointer_qualifiers(),
                    field("declarator", optional(s!(_abstract_declarator))),
                ],
            ),
        ),
    );
    // The attributes and the qualifiers after `&` come from `reference_qualifiers`.
    let reference = |declarator: Rule| {
        prec_dynamic(
            1,
            prec_right(0, seq![choice!["&", "&&"], reference_qualifiers(), declarator]),
        )
    };
    // A macro can come after the `&` and the qualifiers of a named declarator, in the place of a
    // calling convention: `static Cache& NODELETE singleton();`.
    let reference_with_macro =
        |declarator: Rule| reference(seq![optional(choice![call_macro(), pointer_call_macro()]), declarator]);
    g.define("reference_declarator", reference_with_macro(s!(_declarator)));
    g.define(
        "reference_field_declarator",
        reference_with_macro(s!(_field_declarator)),
    );
    g.define("reference_type_declarator", reference(s!(_type_declarator)));
    // A member pointer cannot follow `&` or `&&`, because a pointer to a member of reference type
    // is ill-formed. Without it, the type reading of `A<T>::value && B<U>::value` in a template
    // argument stops at `B`, and the parser keeps fewer versions. The lower precedence of the
    // declarator after `&&` gives `&&(x)(int)` the tree of `_abstract_declarator`: the function
    // declarator is in the reference declarator.
    g.define(
        "abstract_reference_declarator",
        prec_right(
            0,
            seq![
                choice!["&", "&&"],
                reference_qualifiers(),
                optional(prec_right(
                    -1,
                    choice![
                        s!(abstract_pointer_declarator),
                        s!(abstract_function_declarator),
                        s!(abstract_array_declarator),
                        s!(abstract_parenthesized_declarator),
                        s!(abstract_reference_declarator),
                    ]
                )),
            ],
        ),
    );
    // Attributes can follow the `*` of a pointer declarator: `int * [[a]] p;`. GCC reads them in
    // `cp_parser_ptr_operator`. The pointers before a parenthesized initializer take the same
    // attributes, for the reason that `with_pointer_qualifiers` gives: `int *[[a]] p(&x);`.
    for name in [
        "pointer_declarator",
        "pointer_field_declarator",
        "pointer_type_declarator",
        "abstract_pointer_declarator",
        "_pointer_name_declarator",
        "_nested_pointer_name_declarator",
    ] {
        g.redefine(name, |original| {
            insert_after(original, |m| *m == Rule::from("*"), repeat(s!(attribute_declaration)))
        });
    }
    // A macro can come after the `*` and the qualifiers of a named declarator, in the place of a
    // calling convention: `const char * U_EXPORT2 f(int);`.
    for name in ["pointer_declarator", "pointer_field_declarator"] {
        g.redefine(name, |original| {
            insert_after(
                original,
                |m| *m == pointer_qualifiers(),
                optional(seq![
                    choice![call_macro(), pointer_call_macro()],
                    pointer_qualifiers()
                ]),
            )
        });
    }
    // C++26 permits attributes after each name, and one pack: `auto [a [[maybe_unused]], ...rest]`.
    g.define(
        "structured_binding_declarator",
        seq![
            "[",
            comma_sep1(seq![optional("..."), s!(identifier), repeat(s!(attribute_declaration))]),
            close_bracket(),
        ],
    );
    // [dcl.pre]: a structured binding has only `auto`, cv-qualifiers, storage specifiers, and
    // attributes before its `[`, and one optional ref-qualifier. GCC and Clang parse `T [a, b]` after
    // each type, and their semantic analysis rejects a type that is not `auto` (GCC decl.cc,
    // grokdeclarator. Clang SemaDeclCXX.cpp, ActOnDecompositionDeclarator). This grammar has no name
    // lookup. The `auto` then tells a binding apart from `arr[i] = v;` and from a lambda after a
    // declaration, `int d; [d]() {};`.
    //
    // The external scanner gives the `auto` of a binding as its own token. After `auto`, it reads
    // the specifiers, one `&` or `&&`, and a `[` that does not open an attribute, as GCC does in
    // `cp_parser_simple_declaration`. The parser then selects the binding at `auto` with no fork.
    // The specifiers before `auto` share the repeat of `_declaration_specifiers`.
    g.define(
        "_structured_binding_specifiers",
        seq![
            c::extension_prefix(),
            repeat(choice![
                s!(_declaration_modifiers),
                s!(attribute_macro),
                macro_call_attribute()
            ]),
            field(
                "type",
                alias(s!(_structured_binding_type), s!(placeholder_type_specifier))
            ),
            repeat(s!(_declaration_modifiers)),
        ],
    );
    // The `auto` node of a binding has the keyword as its child, as the `auto` rule has.
    g.define(
        "_structured_binding_type",
        seq![alias(s!(_structured_binding_auto_keyword), s!(auto))],
    );
    g.define(
        "_structured_binding_auto_keyword",
        seq![alias(s!(_structured_binding_auto), "auto")],
    );
    g.define(
        "_structured_binding_declarator",
        choice![
            s!(structured_binding_declarator),
            alias(s!(_structured_binding_reference), s!(reference_declarator)),
        ],
    );
    // Clang also reads attributes after the ref-qualifier: `auto &[[maybe_unused]] [a, b]`,
    // `auto &__attribute__((used)) [a, b]` (ParseDeclaratorInternal). GCC and the grammar of
    // [dcl.pre] let only `&` or `&&` come there.
    g.define(
        "_structured_binding_reference",
        seq![
            choice!["&", "&&"],
            repeat(choice![s!(attribute_declaration), s!(attribute_specifier)]),
            s!(structured_binding_declarator)
        ],
    );
    // The initializer of a binding is `= e`, `{e}`, or `(e)` (GCC cp_parser_initializer).
    g.define(
        "_structured_binding_init_declarator",
        seq![
            field("declarator", s!(_structured_binding_declarator)),
            choice![
                seq!["=", field("value", choice![s!(initializer_list), s!(expression)])],
                field("value", choice![s!(argument_list), s!(initializer_list)]),
            ],
        ],
    );
    g.define("ref_qualifier", choice!["&", "&&"]);
    // A macro can come between the name of a declarator and its parameter list, and the macro expands
    // to nothing: `const P& min BOOST_PREVENT_MACRO_SUBSTITUTION () const;` in boost, which stops the
    // expansion of the `min` macro of the Windows headers. The external scanner reads the name of the
    // macro, and only a `(` comes after it. The negative dynamic precedence keeps the other reading of
    // `A B C(x);`, where `A` is a macro and `C(x)` the declarator.
    g.define(
        "_declarator_name_macro",
        prec_dynamic(-2, field("name", alias(s!(_declarator_name_macro_name), s!(identifier)))),
    );
    g.define(
        "_function_declarator_seq",
        seq![
            optional(alias(s!(_declarator_name_macro), s!(attribute_macro))),
            field("parameters", s!(parameter_list)),
            optional(s!(_function_attributes_start)),
            optional(s!(ref_qualifier)),
            optional(s!(_function_exception_specification)),
            optional(s!(_function_attributes_end)),
            optional(choice![
                seq![s!(trailing_return_type), optional(s!(_function_postfix))],
                s!(_function_postfix_with_macros),
            ]),
        ],
    );
    // An attribute after the parameters belongs to the function declarator, and not to an
    // attributed declarator around it. The dynamic precedences here and in
    // `_function_attributes_end` decide the fork of `void f() __attribute__((x));`.
    g.define(
        "_function_attributes_start",
        prec(
            1,
            choice![
                prec_dynamic(1, seq![repeat1(s!(attribute_specifier)), repeat(s!(type_qualifier))]),
                repeat1(s!(type_qualifier)),
            ],
        ),
    );
    g.define(
        "_function_exception_specification",
        choice![s!(noexcept), s!(throw_specifier)],
    );
    g.define(
        "_function_attributes_end",
        prec_right(
            0,
            prec_dynamic(
                1,
                seq![
                    optional(s!(gnu_asm_expression)),
                    choice![
                        seq![repeat1(s!(attribute_specifier)), repeat(s!(attribute_declaration))],
                        seq![repeat(s!(attribute_specifier)), repeat1(s!(attribute_declaration))],
                    ],
                ],
            ),
        ),
    );
    // C++26 contracts come after the virt-specifiers and the requires clause. The rule accepts
    // the three in each order. GCC also reads GNU attributes after the first of them:
    // `void f() override __attribute__((cold)) {}` (parser.cc, cp_parser_init_declarator).
    let postfix = || {
        choice![
            s!(virtual_specifier),
            s!(requires_clause),
            s!(function_contract_specifier)
        ]
    };
    g.define(
        "_function_postfix",
        prec_right(0, seq![postfix(), repeat(choice![postfix(), s!(attribute_specifier)])]),
    );
    // With no trailing return type, a macro in the place of a GNU attribute can come among the
    // virt-specifiers and the contracts: `T& get() const LIFETIME_BOUND;`,
    // `A(A&&) V8_NOEXCEPT = default;`, `void f() TSA_REQUIRES(mu);`, `void f() override LLVM_READONLY {`.
    // No macro comes after a trailing return type or a requires clause. The external token of the
    // macro name then does not split the parse states of the types and the constraints.
    // A GNU attribute can come after the first of these items, as in `_function_postfix`:
    // `void f() override __attribute__((format(printf, 2, 0))) {}`.
    //
    // The cost of that rule is 1 file. `auto f() -> const int * LIFETIME_BOUND;` gets an ERROR node.
    // A measurement of the corpus found the form `-> TYPE MACRO;` in 4 lines of 2 files, and each of
    // the 2 is a compiler test file: `use-trailing-return-type.cpp` and
    // `cxx0x-keyword-attributes.cpp` of llvm-project. Only the first is one of the 329,387 measured
    // files. The form is in 0 of the 7,646 compiler test files, and a requires clause with a macro
    // is in 0 C++ files.
    //
    // `operator->() ABSL_ATTRIBUTE_LIFETIME_BOUND {` of abseil is a macro after a PARAMETER LIST,
    // and not after a trailing return type. The rule reads it.
    let postfix_with_macro = || {
        choice![
            s!(virtual_specifier),
            s!(function_contract_specifier),
            alias(s!(_trailing_attribute_macro), s!(attribute_macro)),
        ]
    };
    g.define(
        "_function_postfix_with_macros",
        prec_right(
            0,
            choice![
                seq![
                    postfix_with_macro(),
                    repeat(choice![postfix_with_macro(), s!(attribute_specifier)]),
                    optional(seq![s!(requires_clause), optional(s!(_function_postfix))]),
                ],
                seq![s!(requires_clause), optional(s!(_function_postfix))],
            ],
        ),
    );
    // A macro after a declarator. The external scanner reads the name by its shape, and the parser
    // does not fork. Right associativity gives the `(` after the name to the arguments.
    g.define(
        "_trailing_attribute_macro",
        prec_right(
            0,
            seq![
                field("name", alias(s!(_trailing_macro_name), s!(identifier))),
                optional(field("arguments", s!(argument_list))),
            ],
        ),
    );
    g.define(
        "function_contract_specifier",
        seq![
            field("kind", choice!["pre", "post"]),
            repeat(s!(attribute_declaration)),
            "(",
            // The result name is an attributed identifier: `post(r [[maybe_unused]]: r > 0)`
            // (cp_parser_function_contract_specifier in GCC).
            optional(seq![
                field("result", s!(identifier)),
                repeat(s!(attribute_declaration)),
                ":"
            ]),
            field("condition", s!(expression)),
            ")",
        ],
    );
    g.define(
        "function_declarator",
        prec_dynamic(
            1,
            seq![field("declarator", s!(_declarator)), s!(_function_declarator_seq),],
        ),
    );
    g.define(
        "function_field_declarator",
        prec_dynamic(
            1,
            seq![field("declarator", s!(_field_declarator)), s!(_function_declarator_seq),],
        ),
    );
    // The function type of a typedef has the suffix of a function declarator:
    // `typedef void F() noexcept;`, `typedef T* (C::*P)() const;`. GCC reads the suffix in
    // `cp_parser_direct_declarator`.
    g.define(
        "function_type_declarator",
        prec_dynamic(
            1,
            seq![field("declarator", s!(_type_declarator)), s!(_function_declarator_seq)],
        ),
    );
    g.define(
        "abstract_function_declarator",
        seq![
            field("declarator", optional(s!(_abstract_declarator))),
            s!(_function_declarator_seq),
        ],
    );
    // A trailing return type is a type-id, and [dcl.type.general] lets a type-specifier-seq define no
    // class and no enumeration. In `auto f() -> struct S { return {}; }` the specifier is
    // `struct S`, and the brace starts the function body. GCC `cp_parser_type_specifier_seq`
    // (parser.cc:28004) sets CP_PARSER_FLAGS_NO_TYPE_DEFINITIONS for a trailing return type, and
    // `cp_parser_type_specifier` then reads an elaborated-type-specifier (parser.cc:22371 and
    // parser.cc:22396). Clang `ParseTrailingReturnType` (ParseDeclCXX.cpp:4168) reads the type in
    // DeclaratorContext::TrailingReturn. `isDefiningTypeSpecifierContext` (Parser.h:1665) gives
    // DSC_trailing no definition, and `ParseClassSpecifier` (ParseDeclCXX.cpp:1920) then makes the
    // tag a reference.
    g.define(
        "trailing_return_type",
        seq!["->", alias(s!(_trailing_type_descriptor), s!(type_descriptor))],
    );
    g.define(
        "noexcept",
        prec_right(0, seq!["noexcept", optional(seq!["(", optional(s!(expression)), ")"])]),
    );
    // A type in a dynamic exception specification can be a pack expansion: `throw(T...)`,
    // `throw(int, const T &...)` (GCC cp_parser_type_id_list, Clang ParseDynamicExceptionSpecification).
    // The node is the pack expansion of a template argument.
    g.define(
        "throw_specifier",
        seq![
            "throw",
            seq![
                "(",
                comma_sep(choice![
                    s!(type_descriptor),
                    alias(s!(type_parameter_pack_expansion), s!(parameter_pack_expansion)),
                ]),
                ")",
            ],
        ],
    );
    // A template name can be a pack index: `TT...[0]<int>`. The precedence gives the `<` after a
    // pack index to the template arguments, and [temp.names] p7 asks for that reading: a `<` is the
    // delimiter of a template-argument-list when it follows a pack-index-template-name. Clang
    // `AnnotatePackIndexingTemplateName` (ParseDeclCXX.cpp:1237) cites the same paragraph. The
    // installed Clang 22.1.8 and GCC 16.2 read no such name, and the LLVM 24 source has it.
    //
    // Only name lookup tells a template pack from a value pack, and a conflict in the place of the
    // precedence gives `N...[0] < 3` the two readings. The value pack is the more frequent one, and
    // `pack_index_expression` gives it a reading of its own: `N...[0] < 3` is a comparison in an
    // expression, in an argument list, and in a template argument. An older comment named the
    // version limit of the parser as the reason for the precedence. The runtime repairs that limit,
    // and the paragraph of the standard is the reason. Refer to
    // upstream-patches/11-merge-finished-version.patch.
    g.define(
        "template_type",
        choice![
            seq![
                field("name", s!(_type_identifier)),
                field("arguments", s!(template_argument_list)),
            ],
            prec(
                1,
                seq![
                    field("name", s!(pack_index_specifier)),
                    field("arguments", s!(template_argument_list)),
                ]
            ),
        ],
    );
    g.define(
        "template_method",
        seq![
            field("name", choice![s!(_field_identifier), s!(operator_name)]),
            field("arguments", s!(template_argument_list)),
        ],
    );
    // An operator function or a literal operator can have template arguments: `operator||<int>`,
    // `operator""_b<'1'>()`. GCC reads them in `cp_parser_template_id`, and Clang in
    // `ParseUnqualifiedId`. The precedence gives the `<` after an operator name to the template
    // arguments.
    g.define(
        "template_function",
        choice![
            seq![
                field(
                    "name",
                    choice![
                        s!(identifier),
                        alias(choice_of(NAMED_CASTS.map(Rule::from)), s!(identifier))
                    ]
                ),
                field("arguments", s!(template_argument_list)),
            ],
            prec(
                1,
                seq![
                    field("name", s!(operator_name)),
                    field("arguments", s!(template_argument_list)),
                ]
            ),
        ],
    );
    // Each argument is one hidden symbol. The type reading and the expression reading of an
    // argument then reduce to the same symbol, and the parser merges their versions after the
    // argument. Without the merge, each argument of a nested list doubles the parse versions.
    // The dynamic precedences select the reading of the merged argument.
    //
    // `>=` is one token, also where the list can end ([lex.pptoken], and [temp.names] splits only
    // `>>`). GCC and Clang reject `v<int>=3`. The second alternative makes `>=` valid where the list
    // can end, and the lexer does not split `>=` into `>` and `=` there. The reading of the list then
    // stops, because the scanner never gives the token after `>=`: `a < b && (c) >= d` is a comparison.
    g.define(
        "template_argument_list",
        seq![
            "<",
            comma_sep(s!(_template_argument)),
            choice![closing_angle(), seq![greater_equal(), s!(_unreachable_token)]],
        ],
    );
    g.define(
        "_template_argument",
        choice![
            prec_dynamic(3, s!(type_descriptor)),
            prec_dynamic(3, alias(s!(_macro_type_descriptor), s!(type_descriptor))),
            alias(s!(type_parameter_pack_expansion), s!(parameter_pack_expansion)),
            prec_dynamic(1, s!(expression)),
            s!(initializer_list),
        ],
    );
    // A macro can come before a template-id or a qualified name in a template argument. In
    // `is_convertible<BOOST_DEDUCED_TYPENAME T::type, int>`, the macro expands to `typename`. In
    // `basic_istream<char, STD char_traits<char>>`, it expands to `std::`. An identifier before such a
    // name is not an expression and not a type. Only the macro reading is then possible. A plain name
    // after the identifier can also be a macro, as the comma macro in
    // `is_convertible<Pair BOOST_MOVE_I value_type>`, and the rule does not read it. A type-id in other
    // places takes no macro: `(__private long *)p` keeps the type `__private`.
    //
    // A keyword type after the macro is no plain name, and the rule reads it: `S<STD size_t>` in the
    // tests of the MSVC STL, where `STD` expands to `std::`.
    g.define(
        "_macro_type_descriptor",
        seq![
            s!(attribute_macro),
            field(
                "type",
                choice![
                    s!(template_type),
                    alias(s!(qualified_type_identifier), s!(qualified_identifier)),
                    s!(primitive_type),
                    s!(sized_type_specifier),
                ]
            ),
            field("declarator", optional(s!(_abstract_declarator))),
        ],
    );
    g.define(
        "namespace_definition",
        seq![
            // A macro can take the place of `inline` or `export`: `BOOST_PROCESS_V1_INLINE namespace v1`,
            // `VULKAN_HPP_EXPORT namespace std`. The keyword `namespace` after the name tells the
            // macro apart from a type.
            optional(choice![s!(attribute_macro), "inline"]),
            "namespace",
            // An attribute specifier sequence, for example `namespace [[=1]] [[=2]] N`
            // (cp_parser_namespace_definition in GCC).
            repeat(s!(attribute_declaration)),
            choice![
                // GCC reads attributes after the name in `cp_parser_namespace_definition`. A macro
                // can take their place: `namespace std _GLIBCXX_VISIBILITY(default) {`. An unnamed
                // namespace takes no macro, because its first identifier is the name.
                seq![
                    field(
                        "name",
                        choice![s!(_namespace_identifier), s!(nested_namespace_specifier)]
                    ),
                    repeat(choice![
                        s!(attribute_specifier),
                        s!(attribute_macro),
                        alias(s!(_attribute_macro_call), s!(attribute_macro)),
                    ]),
                ],
                repeat(s!(attribute_specifier)),
            ],
            field("body", s!(declaration_list)),
        ],
    );
    // A linkage specification applies to a braced list or to one declaration of each kind, as in
    // GCC `cp_parser_linkage_specification`: `extern "C" struct A {};`, `extern "C" typedef int T;`,
    // `extern "C++" template <class T> struct B {};`, and `extern R"(C++)" {}`.
    //
    // GNU attributes can come before `extern`: `__attribute__((weak)) extern "C" void f() {}`. GCC
    // `cp_parser_declaration` (parser.cc:17567) reads them when `extern` and a string literal follow,
    // gives the warning "attributes are not permitted in this position" (parser.cc:17584), drops
    // them, and calls `cp_parser_linkage_specification` (parser.cc:17613). Clang
    // `ParseDeclOrFunctionDefInternal` (Parser.cpp:1143) calls `ParseLinkage`, which starts the
    // `LinkageSpecDecl` at the start of the decl-specifiers (ParseDeclCXX.cpp:320). The attributes
    // are children of this rule for that reason. A standard attribute before `extern` gets an error
    // from each front end, and the rule takes none.
    g.define(
        "linkage_specification",
        seq![
            repeat(s!(_linkage_attribute)),
            "extern",
            field("value", choice![s!(string_literal), s!(raw_string_literal)]),
            choice![
                field(
                    "body",
                    choice![
                        s!(function_definition),
                        s!(declaration),
                        s!(declaration_list),
                        s!(template_declaration),
                        s!(type_definition),
                        s!(alias_declaration),
                        s!(linkage_specification),
                        // The declarations that start with their own keyword:
                        // `export extern "C" using ::foo;`.
                        s!(using_declaration),
                        s!(namespace_definition),
                        s!(namespace_alias_definition),
                        s!(static_assert_declaration),
                        s!(template_instantiation),
                        s!(asm_declaration),
                    ]
                ),
                seq![field("body", s!(type_specifier)), ";"],
            ],
        ],
    );
    // A GNU attribute before `extern` takes its own rule. The repeat of this rule is then not the
    // repeat of the attributes of a typedef and of a statement, and the conflict sets name it.
    g.define("_linkage_attribute", s!(attribute_specifier));
    g.define(
        "namespace_alias_definition",
        seq![
            "namespace",
            field("name", s!(_namespace_identifier)),
            "=",
            choice![
                s!(_namespace_identifier),
                s!(nested_namespace_specifier),
                s!(splice_specifier)
            ],
            ";",
        ],
    );
    g.define(
        "_namespace_specifier",
        seq![optional("inline"), s!(_namespace_identifier)],
    );
    g.define(
        "nested_namespace_specifier",
        prec(
            1,
            seq![
                optional(s!(_namespace_specifier)),
                "::",
                choice![s!(nested_namespace_specifier), s!(_namespace_specifier)],
            ],
        ),
    );
    g.define(
        "using_declaration",
        seq![
            repeat(s!(attribute_declaration)),
            "using",
            optional(choice!["namespace", "enum", "typename"]),
            // C++17 permits a list with packs: `using A::f, Ts::operator()...;`.
            // The name is an unqualified-id after a scope, as in GCC `cp_parser_using_declaration`.
            // A conversion function is one: `using Base::operator bool;`.
            comma_sep1(seq![
                choice![
                    s!(identifier),
                    s!(qualified_identifier),
                    alias(s!(_qualified_conversion_function_id), s!(qualified_identifier)),
                    s!(splice_type_specifier)
                ],
                optional("..."),
            ]),
            ";",
        ],
    );
    g.define(
        "alias_declaration",
        seq![
            c::extension_prefix(),
            "using",
            field("name", s!(_type_identifier)),
            // GCC reads GNU attributes after the name (parser.cc, cp_parser_alias_declaration).
            repeat(choice![s!(attribute_declaration), s!(attribute_specifier)]),
            "=",
            field("type", s!(type_descriptor)),
            ";",
        ],
    );
    // The condition and the message of a static assertion are each one conditional-expression.
    // [dcl.pre] gives the condition a constant-expression, and [expr.const] gives
    // `constant-expression: conditional-expression`. In C++26, a message is a constant expression
    // too (P2741R3): `static_assert(true, Msg{});`, `static_assert(false, string_view("test"));`.
    // GCC cp_parser_static_assert (parser.cc:19232) and Clang ParseStaticAssertDeclaration
    // (ParseDeclCXX.cpp:940) look ahead to the `)`. A message that is not a sequence of string
    // literals then goes to cp_parser_conditional_expression (parser.cc:19271) and to
    // ParseConstantExpressionInExprEvalContext (ParseDeclCXX.cpp:973), which reads a
    // conditional-expression (ParseExpr.cpp:121).
    //
    // GCC reads the condition with cp_parser_constant_expression (parser.cc:19232). That function
    // reads an assignment-expression on purpose, to give a better diagnostic (parser.cc:12297), and
    // its own comment says that the grammar gives a conditional-expression. GCC then rejects
    // `static_assert(a = b);` and `static_assert(throw 1);` in finish_static_assert, and Clang
    // rejects the two forms at the `=` and at the `throw`. The two front ends reject the message
    // `a = b` and the message `throw 1` at the same tokens.
    //
    // In `static_assert(is_same_v<A, B<C>>)`, the parser also reads the condition `is_same_v < A`
    // and the message `B<C>`. At each nested `<`, that reading adds one version for each version of
    // the other reading. The condition before a message has a negative dynamic precedence. At its
    // limit of versions, the parser then removes that reading first.
    //
    // `_Static_assert` is the C11 keyword of the same declaration. Clang reads it in C++ too
    // (ParseDecl.cpp, ParseDeclaration and isDeclarationSpecifier. ParseTentative.cpp,
    // isCXXDeclarationStatement). GCC reads it only in C (c-common.cc, `D_CONLY`), and GCC C++ reads a
    // call of an undeclared function, which is an error.
    g.define(
        "static_assert_declaration",
        seq![
            choice!["static_assert", "_Static_assert"],
            "(",
            choice![
                field("condition", s!(_conditional_expression)),
                seq![
                    field("condition", s!(_static_assert_condition)),
                    ",",
                    field("message", s!(_conditional_expression)),
                ],
            ],
            ")",
            ";",
        ],
    );
    g.define("_static_assert_condition", prec_dynamic(-10, s!(_conditional_expression)));
    // The members come from the final list of `_expression_not_binary`. Refer to
    // `conditional_expression_rule`.
    g.define("_conditional_expression", blank());
    g.define(
        "consteval_block_declaration",
        seq!["consteval", field("body", s!(compound_statement))],
    );
    // CWG2428 puts attributes after the name of a concept: `concept C [[deprecated]] = true;`.
    // GCC reads GNU attributes and C++11 attributes there (parser.cc, cp_parser_concept_definition
    // and cp_parser_attributes_opt), and Clang reads the two kinds (ParseTemplate.cpp,
    // ParseConceptDefinition).
    g.define(
        "concept_definition",
        seq![
            "concept",
            field("name", s!(identifier)),
            repeat(choice![s!(attribute_declaration), s!(attribute_specifier)]),
            "=",
            s!(expression),
            ";"
        ],
    );
    // The declarator rules are complete here.
    pack_declarators(g);
    declarator_classes(g);
    name_grouping_with_attributes(g);
    function_grouping(g);
    parameter_grouping(g);
    keyword_parameter_grouping(g);
    conversion_function_declarators(g);
    // Attribute macros after the declarator of a variable or a data member, in the place of GNU
    // attributes: `int count GUARDED_BY(mu);`, `char *name ATTRIBUTE_UNUSED = "x";`. GCC reads the
    // attributes after the declarator (cp_parser_init_declarator, cp_parser_member_declaration), and
    // Clang reads them before the initializer (ParseAsmAttributesAfterDeclarator,
    // ParseCXXMemberDeclaratorBeforeInitializer). The declarator does not declare a function, and it
    // does not end with a parameter list. The macros after a parameter list are part of its function
    // declarator (`_function_postfix_with_macros`), as the GNU attributes are: `void (*p)(int) ATTR;`.
    // The name of the declarator has no scope: in `API ns::T\nVERSION();`, `ns::T` is a type.
    g.define(
        "_macro_attributed_declarator",
        seq![
            declarator_of_data(g, "_declarator", s!(identifier)),
            declarator_attribute_macros("_declarator_attribute_macro"),
        ],
    );
    g.define(
        "_macro_attributed_field_declarator",
        seq![
            declarator_of_data(g, "_field_declarator", s!(_field_identifier)),
            declarator_attribute_macros("_field_declarator_attribute_macro"),
        ],
    );
    // A macro can also come after the declarator of a parameter:
    // `void f(int a VULKAN_HPP_DEFAULT_ASSIGNMENT(nullptr), int b);`, where the macro expands to
    // `= nullptr` (Vulkan-Hpp vulkan_hpp_macros.hpp line 320). Clang reads GNU attributes and the
    // default argument after the declarator of a parameter (ParseDecl.cpp,
    // ParseParameterDeclarationClause line 7555), and the parameter gets an initializer. The tree
    // gives the macro to the declarator, as it does for a data member. A `)` then also ends the
    // macros of a declarator. Refer to `scan_trailing_macro_name` in src/scanner.c.
    g.redefine("parameter_declaration", |original| {
        replace_rule(
            original,
            &declarator_of_each_class("_declarator"),
            &choice![
                declarator_of_each_class("_declarator"),
                alias(s!(_macro_attributed_declarator), s!(attributed_declarator)),
            ],
        )
    });
}

/// A declarator of the family `family` that does not declare a function and does not end with the
/// parameter list of a function declarator: the name `name`, `*p`, `a[2]`, `&r`, but not `(*p)(int)`.
/// O(n) in the members of the classes of objects and references.
///
/// # Panics
///
/// If `declarator_classes` did not define the classes of objects and references of the family.
fn declarator_of_data(g: &Grammar, family: &str, name: Rule) -> Rule {
    let function = "function_declarator";
    let mut data = vec![name];
    for class in ["object", "reference"] {
        let rule = format!("{family}_of_{class}");
        let Rule::Choice(members) = &g.rules[&rule] else {
            panic!("`{rule}` is a choice");
        };
        data.extend(
            members
                .iter()
                .filter(|member| match member {
                    Rule::Alias { value, .. } => value != function,
                    other => **other != sym(function),
                })
                .cloned(),
        );
    }
    choice_of(data)
}

/// The rules of `_declarator` with one inner declarator. A pack declarator has a copy of each. Refer to
/// `pack_declarators`.
const PACK_DECLARATOR_RULES: [&str; 8] = [
    "attributed_declarator",
    "parenthesized_declarator",
    "function_declarator",
    "pointer_declarator",
    "reference_declarator",
    "array_declarator",
    "block_pointer_declarator",
    "member_pointer_declarator",
];

/// Define `_variadic_declarator`, the declarator of a parameter pack.
///
/// The `...` of a pack comes immediately before the declarator-id, in each shape of a declarator
/// ([dcl.decl.general], noptr-declarator): `Ts &&...xs`, `T... a[1]`, `R (*...fs)()`,
/// `const char (&...s)[N]`, `int C::*...ps`, `Ts... xs [[maybe_unused]]`. GCC `cp_parser_direct_declarator`
/// and Clang `ParseDirectDeclarator` read the `...`, and then the name and the other parts of the
/// declarator. An abstract pack declarator has no name: `Ts &&...`, `Ts...()` ([dcl.name]).
///
/// Each rule in `PACK_DECLARATOR_RULES` gets a copy with `_variadic_declarator` in the place of `_declarator`,
/// and the copy has the node name of the rule. `variadic_declarator` is in the place of each name. A pack
/// declarator then has the qualifiers, the attributes, and the macros of the other declarators, and the same
/// node names. A pack declarator is only in a parameter, and it has no classes. For this reason, the grammar
/// calls this function before `declarator_classes`. O(n) in the size of the rules.
///
/// The copy of a grouping also reads a grouping around an abstract pack: `T (&...)[N]`. Clang reads it with
/// the warning `ext_abstract_pack_declarator_parens`. GCC rejects it, and a FIXME comment in its test
/// `g++.dg/abi/lambda-tpl1.h` identifies the rejection as a defect. Core issue CWG1488 identifies the missing
/// grouping in the grammar of [dcl.name] as a problem.
fn pack_declarators(g: &mut Grammar) {
    let mut members = vec![s!(variadic_declarator)];
    let mut previous = "_variadic_declarator".to_owned();
    for rule in PACK_DECLARATOR_RULES {
        let copy = format!("_variadic_{rule}");
        let content = replace_rule(g.rules[rule].clone(), &s!(_declarator), &s!(_variadic_declarator));
        define_after(g, &previous, &copy, content);
        members.push(alias(sym(&copy), sym(rule)));
        previous = copy;
    }
    g.redefine("_variadic_declarator", |_| choice_of(members));
}

/// The dynamic precedence of a parameter with a grouping that holds a pointer, a reference, or a
/// pointer to a member before an array bound or a parameter list: `T (*a)[3]`, `T (&a)[3]`,
/// `R (C::*f)()`, `R (*f)(A)`.
///
/// Such a parameter needs the grouping, and real code writes it: `void f(T (&a)[N]);`,
/// `XRayBuffer f(XRayBuffer (*g)(XRayBuffer));`. The grouping has the precedence
/// `PAREN_DECLARATOR`. Without this precedence, the other reading of the parentheses, an initializer
/// with a call, has the higher precedence. [dcl.ambig.res] makes the text a parameter when `T` is a
/// type: GCC `cp_parser_direct_declarator` and Clang `isCXXFunctionDeclarator` parse a parameter
/// declaration clause first. An initializer with a call of this shape, `T x(f(*p)[3]);`, is rare in
/// real code.
///
/// A grouping of a name before an array bound keeps its precedence, because the grouping is not
/// necessary: `T x(f(a)[3]);` stays a variable with an initializer.
const PARAMETER_GROUPING: i32 = -PAREN_DECLARATOR + 1;

/// Give a parameter with a necessary grouping the dynamic precedence `PARAMETER_GROUPING`.
///
/// The declarator of such a parameter is an array declarator or a function declarator around a
/// declarator of the class of an object or a reference. The first operator after the name is then in
/// the grouping. Refer to `declarator_classes`. Each of these declarators gets a copy for the
/// parameter, and the copy takes only the inner declarator of the object or the reference class. An
/// array declarator of the class of an object also holds a name, as in `(a)[3]`, and the copy does not.
///
/// The parameter takes the copies with the precedence, and also the classes. The two readings of the
/// same declarator have the same tree, and the parser selects the copy with the precedence. The
/// classes stay the declarator of the parameter, so that a GNU attribute after the declarator goes to
/// the parameter, as `parameter_declaration` says. O(n) in the size of the four rules.
fn parameter_grouping(g: &mut Grammar) {
    let copies = [
        ("array_declarator", "_parameter_array_declarator", "array_declarator"),
        (
            "_array_declarator_of_reference",
            "_parameter_array_declarator_of_reference",
            "array_declarator",
        ),
        (
            "_function_declarator_of_object",
            "_parameter_function_declarator_of_object",
            "function_declarator",
        ),
        (
            "_function_declarator_of_reference",
            "_parameter_function_declarator_of_reference",
            "function_declarator",
        ),
    ];
    let mut members = Vec::new();
    for (original, copy, node) in copies {
        let rule = replace_rule(
            g.rules[original].clone(),
            &choice![s!(_declarator_of_name), s!(_declarator_of_object)],
            &s!(_declarator_of_object),
        );
        define_after(g, original, copy, rule);
        members.push(alias(sym(copy), sym(node)));
        g.conflicts.push(vec![original.to_owned(), copy.to_owned()]);
    }
    let declarators = choice![
        declarator_of_each_class("_declarator"),
        prec_dynamic(PARAMETER_GROUPING, choice_of(members)),
    ];
    g.redefine("parameter_declaration", |original| {
        replace_rule(original, &declarator_of_each_class("_declarator"), &declarators)
    });
}

/// The dynamic precedence of a parameter with a type that is a keyword and a grouping of a name:
/// `int(x)`, `unsigned long (n)`, `decltype(i)(j)`.
///
/// A parameter list of such parameters is a parameter declaration clause, and no name lookup is
/// necessary ([dcl.ambig.res]). `Bar f(int(x), int(y));` declares a function, also in a block, and it
/// does not initialize a variable with two functional casts. Clang `isCXXFunctionDeclarator` reads
/// the parameters in `TryParseParameterDeclarationClause`, and GCC gives the warning
/// `-Wvexing-parse`. The name in the grouping is the name of the parameter: Clang declares `x` of
/// the type `int`. It is not a parameter of the type `x` in the type of a function.
///
/// The grouping of this form has no negative precedence, and the parameter has the precedence of a
/// functional cast, `CALL_DYNAMIC`. A function with such parameters then has one more precedence than a
/// variable with the functional casts, from the function declarator. The parameter also has more
/// precedence than the abstract function declarator in `int (x)` with a parameter of the type `x`. A
/// type-id with a function type in a template argument, as in `A<f(int(x))>`, keeps less precedence
/// than the call that Clang reads. A grouping around a function declarator has the precedence
/// `FUNCTION_GROUPING`, and the statement `f(g(int(x)));` stays a call.
const KEYWORD_PARAMETER_GROUPING: i32 = CALL_DYNAMIC;

/// The dynamic precedence of a call and of a functional cast: `f(x)`, `int(x)`.
const CALL_DYNAMIC: i32 = 1;

/// The type keywords of a parameter with a grouping of a name, and of a functional cast of a name:
/// a fundamental type, a sized type, and `decltype(e)`.
///
/// `auto` is not in the set: in a block, `T x(auto(y));` initializes `x` with a decay copy, because a
/// function with a placeholder parameter is not permitted there. `typename T::U` is not in the set:
/// `typename BOOST_MPL_AUX_NA_PARAM(T)` in a template parameter list is a macro.
fn parameter_keyword_types() -> Rule {
    choice![s!(primitive_type), s!(sized_type_specifier), s!(decltype)]
}

/// A keyword in the place of a name, and a token that the scanner never gives after it.
///
/// Where a rule takes only a name, the lexer reads a keyword with no action as a name: in the
/// parentheses of `T f(int);` and `T x(this);`, a rule of names reads `int` and `this` as names. In such a
/// place, this rule makes the type keywords, `this`, and the literal keywords valid. The lexer then
/// reads the keyword as a keyword, and the reading stops at the token after it. The keyword has the
/// node name of a name, and the node types of the rule do not change.
fn keyword_stop() -> Rule {
    let words = [
        "long", "short", "signed", "unsigned", "auto", "true", "TRUE", "false", "FALSE", "nullptr",
        "NULL",
    ]
    .map(|word| alias(word, s!(identifier)))
    .into_iter()
    .chain([alias(s!(primitive_type), s!(identifier)), alias(s!(this), s!(identifier))]);
    seq![choice_of(words), s!(_unreachable_token)]
}

/// Give a parameter with a type that is a keyword and a grouping of a name the dynamic precedence
/// `KEYWORD_PARAMETER_GROUPING`.
///
/// The parameter takes a second form with the type and the grouping only. The two forms have the
/// same tree, and the parser selects the form with the precedence. A parameter with other specifiers,
/// `const int (x)`, has no other reading as an expression. After the type, `(` starts the grouping of
/// the second form, or a declarator of the first form, and a conflict keeps the two. O(1).
fn keyword_parameter_grouping(g: &mut Grammar) {
    for set in [
        &["type_specifier", "parameter_declaration"][..],
        &["type_specifier", "parameter_declaration", "call_expression"],
        &["expression", "_keyword_parameter_grouping"],
    ] {
        g.conflicts.push(set.iter().map(|&name| name.to_owned()).collect());
    }
    // The grouping holds a name. After `(`, a keyword is also valid, and that reading of `int (int)`
    // stops at the keyword: the parameter has a function type. Refer to `keyword_stop`.
    g.define(
        "_keyword_parameter_grouping",
        seq!["(", choice![s!(identifier), keyword_stop()], ")"],
    );
    g.redefine("parameter_declaration", |original| {
        choice![
            original,
            prec_dynamic(
                KEYWORD_PARAMETER_GROUPING,
                seq![
                    field("type", parameter_keyword_types()),
                    field(
                        "declarator",
                        alias(s!(_keyword_parameter_grouping), s!(parenthesized_declarator))
                    ),
                    repeat(s!(attribute_specifier)),
                ],
            ),
        ]
    });
}

/// Define `_conversion_function_declarator`, the declarator after the type of a conversion function.
///
/// A conversion-type-id ends with ptr-operators, and the declarator of the conversion function is a
/// function declarator with a parameter list ([class.conv.fct]): `operator int()`, `operator T *() &&`,
/// `operator int &() &`, `operator int A::*() const`. As in each abstract declarator, a pointer declarator
/// holds the function declarator that comes after it. The copies keep the node names and the fields of the
/// abstract declarators. A ptr-operator is not a grouping and not an array bound, and the copies have
/// neither. O(n) in the size of the copied rules.
///
/// # Panics
///
/// If an abstract declarator rule does not have the inner declarator that its copy replaces.
fn conversion_function_declarators(g: &mut Grammar) {
    let copy = |g: &Grammar, rule: &str, from: &Rule, inner: &str| {
        let original = g.rules[rule].clone();
        let replaced = replace_rule(original.clone(), from, &sym(inner));
        assert!(replaced != original, "`{rule}` has no inner declarator {from:?}");
        replaced
    };
    let optional_inner = optional(s!(_abstract_declarator));
    let reference_inner = optional(prec_right(
        -1,
        choice![
            s!(abstract_pointer_declarator),
            s!(abstract_function_declarator),
            s!(abstract_array_declarator),
            s!(abstract_parenthesized_declarator),
            s!(abstract_reference_declarator),
        ],
    ));
    let all = "_conversion_function_declarator";
    // As in `abstract_reference_declarator`, no member pointer comes after `&` or `&&`.
    let after_reference = "_conversion_function_declarator_after_reference";
    let pointer = copy(g, "abstract_pointer_declarator", &optional_inner, all);
    let member_pointer = copy(g, "abstract_member_pointer_declarator", &optional_inner, all);
    let reference = copy(g, "abstract_reference_declarator", &reference_inner, after_reference);
    let parameters = alias(s!(_conversion_parameters), s!(abstract_function_declarator));
    let pointer_alias = alias(s!(_conversion_pointer_declarator), s!(abstract_pointer_declarator));
    let reference_alias = alias(s!(_conversion_reference_declarator), s!(abstract_reference_declarator));
    let member_pointer_alias = alias(
        s!(_conversion_member_pointer_declarator),
        s!(abstract_member_pointer_declarator),
    );
    g.define(
        all,
        choice![
            parameters.clone(),
            pointer_alias.clone(),
            reference_alias.clone(),
            member_pointer_alias
        ],
    );
    g.define(after_reference, choice![parameters, pointer_alias, reference_alias]);
    // The function declarator has no inner declarator, as `abstract_function_declarator` with no
    // declarator.
    g.define("_conversion_parameters", seq![s!(_function_declarator_seq)]);
    g.define("_conversion_pointer_declarator", pointer);
    g.define("_conversion_reference_declarator", reference);
    g.define("_conversion_member_pointer_declarator", member_pointer);
}

/// The dynamic precedence of a grouping with no attributes around a function declarator: `(f(x))`,
/// `(f() -> T)`.
///
/// The grouping is not necessary, and real code does not write it. GCC `grokdeclarator` gives the
/// warning "unnecessary parentheses in declaration" for it. A statement with this form is an
/// expression, as `void(f(x));` and `int(f()->x);` (GCC `g++.dg/parse/ambig9.C`). A type that is a
/// keyword gives the declaration reading the precedence `KEYWORD_TYPE`, and the sum with this
/// precedence is less than zero. The precedence replaces `PAREN_DECLARATOR` in the production of the
/// grouping.
const FUNCTION_GROUPING: i32 = -300;

/// Give the grouping of a function declarator the dynamic precedence `FUNCTION_GROUPING`.
///
/// The grouping of the class of a function holds each declarator of that class: `(*f())`, `(&f())`,
/// `(f())`. Only a function declarator with no other operator gets the precedence. `(*f(x))(y)`
/// declares a function that returns a pointer to a function, and keeps the precedence of a grouping.
/// Refer to `declarator_classes`. O(n) in the members of the two rules.
fn function_grouping(g: &mut Grammar) {
    let others: Vec<Rule> = g.rules["_declarator_of_function"]
        .clone()
        .into_members()
        .into_iter()
        .filter(|member| *member != s!(function_declarator))
        .collect();
    let inner = choice_of(
        [prec_dynamic(FUNCTION_GROUPING, s!(function_declarator))]
            .into_iter()
            .chain(others),
    );
    g.redefine("_parenthesized_declarator_of_function", |original| {
        Rule::Choice(
            original
                .into_members()
                .into_iter()
                .map(|member| match member {
                    Rule::Prec {
                        kind: PrecKind::Dynamic,
                        value: PAREN_DECLARATOR,
                        content,
                    } => prec_dynamic(
                        PAREN_DECLARATOR,
                        replace_rule(*content, &s!(_declarator_of_function), &inner),
                    ),
                    other => other,
                })
                .collect(),
        )
    });
}

/// Give the grouping of a name with attributes only the dynamic precedence of the C grammar.
///
/// In a class body, `S(__attribute__((unused)) int);` declares a constructor with a parameter (GCC
/// `cp_parser_constructor_declarator_p`). Where only a declarator can come, the lexer gives `int` as a
/// name, and `S (__attribute__((unused)) int)` is also a member with a grouping of a name. A grouping
/// of a pointer, a reference, an array, or a function has no such second reading. The grouping of a
/// name is the first copy of each grouping rule, and its declarator is of the class of a name. Refer
/// to `declarator_classes`. O(n) in the members of the two rules.
fn name_grouping_with_attributes(g: &mut Grammar) {
    let attributes_only = |content: &Rule| {
        matches!(content, Rule::Seq(members) if members.len() == 4 && members[1] == s!(_grouping_attributes))
    };
    for name in ["parenthesized_declarator", "parenthesized_field_declarator"] {
        g.redefine(name, |original| {
            Rule::Choice(
                original
                    .into_members()
                    .into_iter()
                    .map(|member| match member {
                        Rule::Prec {
                            kind: PrecKind::Dynamic,
                            value: GROUPING_WITH_ATTRIBUTES,
                            content,
                        } if attributes_only(&content) => prec_dynamic(PAREN_DECLARATOR, *content),
                        other => other,
                    })
                    .collect(),
            )
        });
    }
}

/// The dynamic precedence of a grouping with GNU attributes or a calling convention after its `(`.
///
/// The C grammar gives a grouping the precedence `PAREN_DECLARATOR`, and the parser selects a call
/// reading of the same text: `f(*p)`. The attributes and the calling convention are not in an
/// expression. The declarator of `T (__attribute__((x)) *p)(U);` has the precedence 2 without the
/// grouping. The call reading `T(__attribute__((x)) * p)(U)` has three calls and the precedence 3.
/// With this precedence, the parser selects the declarator.
const GROUPING_WITH_ATTRIBUTES: i32 = 2;

/// The dynamic precedence of a pointer, a pointer to a member, or an lvalue reference with a
/// parenthesized initializer that is not a name, in a block.
///
/// In a block, `T *p(make(q));` is also an expression statement that multiplies `T` by a call, and
/// `T &r(*q);` is also a bitwise AND. Only name lookup tells them apart (GCC `cp_parser_statement`,
/// Clang `isCXXDeclarationStatement`). A statement that discards such a product is rare in real code.
/// The precedence prefers the declaration to the expression, and to a function declarator with a
/// parameter of the type `NULL` in `T *p(NULL);`.
const POINTER_INITIALIZER: i32 = 4;

/// The dynamic precedence of a pointer or a reference with a parenthesized initializer outside a block.
///
/// Outside a block, a function declaration is frequent, also with a parameter that is an expression
/// with other precedences: `T *begin(T (&array)[N]);`, `R *add(Args...);`. The precedence is lower than
/// the precedence of each function declarator. The initializer then applies only where the
/// parentheses cannot hold parameters: `static T *instance(nullptr);`.
const POINTER_INITIALIZER_OUTSIDE_BLOCK: i32 = -100;

/// The expressions that can also be the type of a parameter: `x`, `a::b`, `A<B>`, `[:r:]`, `T...[0]`.
const NAME_EXPRESSIONS: [&str; 5] = [
    "identifier",
    "qualified_identifier",
    "template_function",
    "splice_expression",
    "pack_index_expression",
];

/// The declarators of a pointer, a pointer to a member, or a reference with a parenthesized
/// initializer: `int *p(&x);`, `const T &r(a.b);`, `int **pp(&p);`, `int S::*pm(&S::m);`.
///
/// In `pointer_declarator`, the `(` after the name starts a function declarator. These flat
/// declarators end with the name, and the parser keeps the two readings until the tokens in the
/// parentheses tell them apart. The declarator of the outer operator reduces before the `(`, and its
/// lower dynamic precedence gives the parameters of `T *f(a);` to the function declarator, as
/// [dcl.ambig.res] says. When the parser has too many versions, it drops the versions of the arguments
/// first.
///
/// The initializer of a pointer or a reference is one expression or one braced list ([dcl.init.general]):
/// in `ar & make_nvp("x", d);`, `&` is an operator.
///
/// A declaration in a block takes `_block_pointer_init_declarator`. There, an initializer that is not
/// a name gets the precedence `POINTER_INITIALIZER`. A name can also be the type of a parameter, and it
/// gets no precedence. An rvalue reference gets no precedence, because `trace && log("x");` is frequent
/// in real code. Each other declaration takes `_pointer_init_declarator`, with the precedence
/// `POINTER_INITIALIZER_OUTSIDE_BLOCK`.
fn pointer_init_declarators(g: &mut Grammar) {
    let other = || field("value", alias(s!(_initializer_arguments), s!(argument_list)));
    let name = || field("value", alias(s!(_name_initializer_arguments), s!(argument_list)));
    let lvalue = || {
        field(
            "declarator",
            choice![
                alias(s!(_pointer_name_declarator), s!(pointer_declarator)),
                alias(s!(_member_pointer_name_declarator), s!(member_pointer_declarator)),
                alias(s!(_reference_name_declarator), s!(reference_declarator)),
            ],
        )
    };
    let rvalue = || {
        field(
            "declarator",
            alias(s!(_rvalue_reference_name_declarator), s!(reference_declarator)),
        )
    };
    g.define(
        "_pointer_init_declarator",
        prec_dynamic(
            POINTER_INITIALIZER_OUTSIDE_BLOCK,
            seq![choice![lvalue(), rvalue()], choice![other(), name()]],
        ),
    );
    g.define(
        "_block_pointer_init_declarator",
        choice![
            prec_dynamic(POINTER_INITIALIZER, seq![lvalue(), other()]),
            seq![lvalue(), name()],
            seq![rvalue(), choice![other(), name()]],
        ],
    );
    // The function `initializer_arguments` defines this rule when the expression rules are complete.
    g.define("_initializer_arguments", blank());
    g.define(
        "_name_initializer_arguments",
        seq!["(", choice_of(NAME_EXPRESSIONS.map(sym)), ")"],
    );
    // The name after the outer operator: `*p`, `**pp`, `*const *p`, `*&rp`, `C::**p`. A definition
    // outside a class has a qualified name: `T *C::p(nullptr);`. A reference takes no qualified name,
    // because in a block `ar & ns::base_object<T>(*this);` calls an overloaded `operator&`.
    let inner = || {
        choice![
            s!(identifier),
            s!(qualified_identifier),
            alias(s!(_nested_pointer_name_declarator), s!(pointer_declarator)),
            alias(s!(_nested_member_pointer_name_declarator), s!(member_pointer_declarator)),
            alias(s!(_nested_reference_name_declarator), s!(reference_declarator)),
            alias(s!(_nested_rvalue_reference_name_declarator), s!(reference_declarator)),
        ]
    };
    // The qualifiers of `pointer_declarator` and `reference_declarator`. `declarations` gives the pointer
    // the calling conventions and GNU attributes of `pointer_declarator`.
    let pointer = || seq!["*", repeat(s!(type_qualifier)), field("declarator", inner())];
    let member_pointer = || {
        seq![
            s!(_member_pointer_scope),
            member_pointer_qualifiers(),
            field("declarator", inner())
        ]
    };
    let reference = |operator: &str| seq![operator, reference_qualifiers(), s!(identifier)];
    g.define("_pointer_name_declarator", prec_dynamic(-1, pointer()));
    g.define("_member_pointer_name_declarator", prec_dynamic(-1, member_pointer()));
    g.define("_reference_name_declarator", prec_dynamic(-1, reference("&")));
    g.define("_rvalue_reference_name_declarator", prec_dynamic(-1, reference("&&")));
    g.define("_nested_pointer_name_declarator", pointer());
    g.define("_nested_member_pointer_name_declarator", member_pointer());
    g.define("_nested_reference_name_declarator", reference("&"));
    g.define("_nested_rvalue_reference_name_declarator", reference("&&"));
}

/// The initializer in parentheses of a pointer or a reference that is not a name: an expression that
/// is not in `NAME_EXPRESSIONS`, or a braced list.
///
/// The members come from the final list of `_expression_not_binary`. For this reason, the grammar calls
/// this function after the directive rules. O(n) in the members of the list.
fn initializer_arguments(g: &mut Grammar) {
    let names = NAME_EXPRESSIONS.map(sym);
    let expressions = choice_members(g.rules["_expression_not_binary"].clone())
        .into_iter()
        .filter(|member| !names.contains(member))
        .chain([s!(binary_expression), s!(initializer_list)]);
    g.redefine("_initializer_arguments", |_| {
        seq!["(", choice_of(expressions), ")"]
    });
}

/// The members of `expression` that are no conditional-expression.
///
/// [expr.ass] gives `assignment-expression: conditional-expression | ... | throw-expression |
/// yield-expression`. A pack expansion is no expression of [expr]: it belongs to an expression
/// list, to a template argument list, and to the other lists of [temp.variadic].
const NOT_CONDITIONAL_EXPRESSIONS: [&str; 4] = [
    "assignment_expression",
    "throw_expression",
    "co_yield_expression",
    "parameter_pack_expansion",
];

/// The conditional-expression of [expr.cond]: each member of `expression` that is not in
/// `NOT_CONDITIONAL_EXPRESSIONS`.
///
/// The members come from the final list of `_expression_not_binary`. For this reason, the grammar
/// calls this function after the directive rules. O(n) in the members of the list.
fn conditional_expression_rule(g: &mut Grammar) {
    let assignments = NOT_CONDITIONAL_EXPRESSIONS.map(sym);
    let expressions = choice_members(g.rules["_expression_not_binary"].clone())
        .into_iter()
        .filter(|member| !assignments.contains(member))
        .chain([s!(binary_expression)]);
    g.redefine("_conditional_expression", |_| choice_of(expressions));
}

/// The dynamic precedence of a reference declarator with no initializer in a block.
///
/// A reference declaration has an initializer, except with `extern`, as a member, as a parameter,
/// or as a return type ([dcl.ref]). GCC `grok_reference_init` and Clang
/// `Sema::ActOnUninitializedDecl` reject `T &x;` in a block. In a block, `ar & x;` and `a && b;` are
/// then expressions: a bitwise AND, an overloaded `operator&`, or a logical AND. The precedence is
/// less than the precedence of each other reading. Where no other reading is possible, the
/// declaration stays: `extern T &x;`, `static T &x;`, `const T &x;`.
const REFERENCE_WITHOUT_INITIALIZER: i32 = -100;

/// The dynamic precedence of a function declarator that returns a reference, with no initializer, in a
/// block: `T &f(U);`, `T &&f(U);`.
///
/// Only name lookup tells such a declaration from the other readings of the same text. GCC
/// `cp_parser_statement` and Clang `isCXXDeclarationStatement` look up `T` and `U`. In a block,
/// `ar & get<0>(x);` and `ar & BOOST_SERIALIZATION_NVP(x);` call an overloaded `operator&`,
/// `trace && log(x);` is a logical AND, and `const T &r(x);` declares a variable. In 329,387 corpus
/// files, blocks had approximately 500 such statements with no parse error, and each was an
/// expression or a variable. Where no other reading is possible, the declaration stays:
/// `T &f(int);`, `T &g(U u);`.
///
/// An operator function has no such precedence: `S &operator>>(S &in, T &t);`. A call of an operator
/// function by its name after a binary operator is not in real code. The declaration of a block takes a
/// second declarator for this form, and that declarator has the precedences of a reference and a
/// function declarator.
const REFERENCE_FUNCTION_IN_BLOCK: i32 = -100;

/// The dynamic precedence of a variable in a block with an initializer in parentheses that holds only
/// names: `T x(y);`, `T x(a, ns::b);`, `std::tuple<Ts...> t(ts...);`.
///
/// [dcl.ambig.res] makes `T x(y);` a function declaration when `y` names a type, and only name lookup
/// tells the two readings apart (GCC `cp_parser_direct_declarator`, Clang `isCXXFunctionDeclarator`).
/// In a block, a function declaration with parameters of such types and no parameter names is rare.
/// GCC gives the warning `-Wvexing-parse` for it. In 329,387 corpus files, blocks with no parse error
/// had 203,833 such statements. After 145,561 of them, the block used the name as a variable, and after
/// 4,992 as a call, as `rng(x)` for a random generator. The block did not use the name after 48,748 of
/// them, as for a lock guard.
///
/// The precedence is more than the precedence of the function declarator. A function declaration with
/// a parameter that is not a name keeps its reading: `T f(int);`, `T f(U u);`, `T f(U *);`, and
/// `T x();`, which [dcl.ambig.res] makes a function.
const NAME_INITIALIZER_IN_BLOCK: i32 = 3;

/// The declaration in a block: a declaration with the pointer declarators of a block.
///
/// A block item, a label, a case body, a substatement, and the initializer of a `for`, an `if`, or a
/// `switch` hold this declaration. Refer to `pointer_init_declarators`. The copy comes after `declaration` in the
/// order of the rules, and it has the place of `declaration` among the other symbols. Each conflict set
/// with `declaration` gets a copy with the block declaration, and each conflict set with a block item gets
/// a copy with the item of a declaration list. The grammar calls this function when the declaration rule
/// is complete. O(n) in the size of the rule and the conflict sets.
///
/// A reference declarator with no initializer gets the precedence `REFERENCE_WITHOUT_INITIALIZER`, and a
/// reference declarator around a function declarator gets the precedence `REFERENCE_FUNCTION_IN_BLOCK`.
/// The declarator without an initializer names each class and the members of the class of a function for
/// this reason. The precedence applies to the production of the declaration, and the declaration has the
/// precedence one time for each such declarator.
fn block_declaration(g: &mut Grammar) {
    let pointers = replace_rule(
        g.rules["declaration"].clone(),
        &alias(s!(_pointer_init_declarator), s!(init_declarator)),
        &alias(s!(_block_pointer_init_declarator), s!(init_declarator)),
    );
    // A variable with an initializer of names takes a second reading with a precedence. Refer to
    // `NAME_INITIALIZER_IN_BLOCK`.
    let pointers = replace_rule(
        pointers,
        &s!(init_declarator),
        &choice![
            s!(init_declarator),
            prec_dynamic(
                NAME_INITIALIZER_IN_BLOCK,
                alias(s!(_block_name_init_declarator), s!(init_declarator))
            ),
        ],
    );
    let reference_function = alias(s!(_reference_declarator_of_function), s!(reference_declarator));
    let functions = g.rules["_declarator_of_function"]
        .clone()
        .into_members()
        .into_iter()
        .map(|member| {
            if member == reference_function {
                prec_dynamic(REFERENCE_FUNCTION_IN_BLOCK, member)
            } else {
                member
            }
        });
    let copy = replace_rule(
        pointers,
        &s!(_declarator),
        &choice![
            s!(_declarator_of_name),
            choice_of(functions),
            alias(s!(_block_reference_operator_declarator), s!(reference_declarator)),
            s!(_declarator_of_object),
            prec_dynamic(REFERENCE_WITHOUT_INITIALIZER, s!(_declarator_of_reference)),
        ],
    );
    // A block declaration with `extern` before the type and one declarator of the class of a function
    // has a second reading. Refer to `EXTERN_FUNCTION_IN_BLOCK`.
    let copy = choice_of(copy.into_members().into_iter().chain([prec_dynamic(
        EXTERN_FUNCTION_IN_BLOCK,
        seq![
            optional(s!(ms_call_modifier)),
            s!(_extern_declaration_specifiers),
            optional(call_macro()),
            field(
                "declarator",
                seq![
                    optional(s!(ms_call_modifier)),
                    s!(_declarator_of_function),
                    optional(s!(gnu_asm_expression)),
                ]
            ),
            ";",
        ],
    )]));
    define_after(g, "declaration", "_block_declaration", copy);
    // The second reading has the same tree as the variable with an argument list. Each argument is a
    // name, the pack expansion of a name, or a functional cast of a name to a keyword type, and one
    // argument or more is not a cast. With only such casts, the parentheses hold parameters, as
    // `KEYWORD_PARAMETER_GROUPING` says. After the name of the declarator, `(` starts the arguments of the
    // second reading or a parameter list, and a conflict keeps the two readings.
    g.conflicts.push(vec!["_declarator_of_name".to_owned(), "_block_name_init_declarator".to_owned()]);
    define_after(
        g,
        "_block_declaration",
        "_block_name_init_declarator",
        seq![
            field("declarator", s!(identifier)),
            field("value", alias(s!(_block_name_arguments), s!(argument_list))),
        ],
    );
    // After `(` or `,`, a keyword is also valid, and the lexer reads it as a keyword and not as a name.
    // That reading of `T f(int);` stops at the keyword. Refer to `keyword_stop`.
    let name = || {
        choice_of(
            NAME_EXPRESSIONS
                .map(sym)
                .into_iter()
                .chain([
                    alias(s!(identifier_parameter_pack_expansion), s!(parameter_pack_expansion)),
                    keyword_stop(),
                ]),
        )
    };
    let cast = || alias(s!(_block_keyword_cast), s!(call_expression));
    define_after(
        g,
        "_block_name_init_declarator",
        "_block_name_arguments",
        seq!["(", s!(_block_name_argument_list), ")"],
    );
    // The lists are right recursive, and a list ends only after a name: `a`, `int(a), b`, `a, int(b)`.
    define_after(
        g,
        "_block_name_arguments",
        "_block_name_argument_list",
        choice![
            name(),
            seq![name(), ",", s!(_block_argument_list_tail)],
            seq![cast(), ",", s!(_block_name_argument_list)],
        ],
    );
    define_after(
        g,
        "_block_name_argument_list",
        "_block_argument_list_tail",
        choice![
            name(),
            cast(),
            seq![choice![name(), cast()], ",", s!(_block_argument_list_tail)],
        ],
    );
    // The cast has the precedence of a call, as the same cast in an argument list has.
    define_after(
        g,
        "_block_argument_list_tail",
        "_block_keyword_cast",
        prec_dynamic(
            CALL_DYNAMIC,
            seq![
                field("function", parameter_keyword_types()),
                field(
                    "arguments",
                    alias(s!(_keyword_parameter_grouping), s!(argument_list))
                ),
            ],
        ),
    );
    // The reference to an operator function. Refer to `REFERENCE_FUNCTION_IN_BLOCK`.
    define_after(
        g,
        "_block_declaration",
        "_block_reference_operator_declarator",
        prec_dynamic(
            1,
            seq![
                choice!["&", "&&"],
                alias(s!(_block_operator_function_declarator), s!(function_declarator)),
            ],
        ),
    );
    define_after(
        g,
        "_block_reference_operator_declarator",
        "_block_operator_function_declarator",
        prec_dynamic(
            1,
            seq![field("declarator", s!(operator_name)), s!(_function_declarator_seq)],
        ),
    );
}

/// The dynamic precedence of a declaration outside a block in which a grouping of a name is the full
/// declarator: `T (x);`, `T (x) = f;`, `T (x)(a, b);`.
///
/// The grouping is not necessary, and real code writes `T x;`. GCC `grokdeclarator` (decl.cc) gives the
/// warning "unnecessary parentheses in declaration" for it. Outside a block, real code writes this text
/// as a macro statement: `EXPORT_SYMBOL(f);`, `BENCHMARK(b);`, `DECLARE(ns::m)("name", f);`,
/// `BOOST_HOF_STATIC_FUNCTION(print) = f;`. The precedence is less than `NAMESPACE_EXPRESSION`, and the
/// call keeps the text. In a block, the precedence of a call is already more than the precedence of the
/// declaration, and the declaration of a block takes no such precedence.
///
/// A type that is a keyword lifts the declaration above the call again, with `KEYWORD_TYPE`: `int (x);`
/// and `int (x) = 1;` declare a variable, and no name lookup is necessary ([dcl.ambig.res]. GCC
/// `cp_parser_simple_declaration`. Clang `isCXXSimpleDeclaration`). A declarator with one operator more
/// keeps the declaration: `T (*p)[3];`, `T (x)[3];`.
const NAME_GROUPING_OUTSIDE_BLOCK: i32 = -150;

/// Give a declaration outside a block in which a grouping of a name is the full declarator the dynamic
/// precedence `NAME_GROUPING_OUTSIDE_BLOCK`.
///
/// The precedence applies to the production of the declaration and to the production of its init
/// declarator. A declaration has the precedence one time for each such declarator. A declaration
/// outside a block takes a copy of the init declarator, so that the parse states of a block do not
/// change. The declaration of a block comes before this function. O(n) in the size of the two rules.
fn name_grouping_declarators(g: &mut Grammar) {
    let names = choice_of(
        g.rules["_declarator_of_name"]
            .clone()
            .into_members()
            .into_iter()
            .map(|member| {
                if member == s!(parenthesized_declarator) {
                    prec_dynamic(NAME_GROUPING_OUTSIDE_BLOCK, member)
                } else {
                    member
                }
            }),
    );
    let declarators = choice![
        names.clone(),
        s!(_declarator_of_function),
        s!(_declarator_of_object),
        s!(_declarator_of_reference),
    ];
    let init = replace_rule(g.rules["init_declarator"].clone(), &s!(_declarator_of_name), &names);
    define_after(g, "init_declarator", "_namespace_init_declarator", init);
    g.redefine("declaration", |original| {
        let classes = replace_rule(original, &s!(_declarator), &declarators);
        replace_rule(
            classes,
            &s!(init_declarator),
            &alias(s!(_namespace_init_declarator), s!(init_declarator)),
        )
    });
    // This init declarator names the members of the class of a name. After the specifiers of a
    // declaration outside a block, a name is then the declarator of the init declarator, or a name that
    // a declarator operator follows.
    g.conflicts.push(vec![
        "_declarator_of_name".to_owned(),
        "_namespace_init_declarator".to_owned(),
    ]);
}

/// The names of the classes of declarators, in the order of the class vectors.
const DECLARATOR_CLASSES: [&str; 4] = ["name", "function", "object", "reference"];

/// How the class of a declarator rule with one inner declarator follows from the class of the
/// inner declarator.
#[derive(Clone, Copy)]
enum Wrapper {
    /// Parentheses and attributes keep the class of the inner declarator.
    SameClass,
    /// A parameter list after a name or after a function declarator declares a function: `f()`,
    /// `(f)()`, `(*f())()`. After an object declarator, it declares an object: `(*p)()`. After a
    /// reference declarator, it declares a reference: `(&r)()`.
    Function,
    /// A pointer or an array bound after a name or after an object declarator declares an object:
    /// `*p`, `a[2]`. After a function declarator, it declares a function: `*f()`. After a reference
    /// declarator, it declares a reference: `*&r`, `(&r)[2]`.
    Object,
    /// A reference after a name or after a reference declarator declares a reference: `&r`. After a
    /// function declarator, it declares a function: `&f()`. After an object declarator, it declares
    /// an object: `&*p`.
    Reference,
}

/// The classes of declarators: a name, a function, an object, and a reference.
///
/// GCC `function_declarator_p` and Clang `Declarator::isFunctionDeclarator` (DeclSpec.h) find a
/// function declarator in the same way. The first declarator operator after the name, with no
/// parentheses, is a parameter list: `f()`, `*f()`, `(f)()`, `(*f())[2]`. In `(*p)()` and `*a[2]`,
/// the first operator is `*` or `[2]`, and the declarator declares an object. When the first
/// operator is `&` or `&&`, the declarator declares a reference: `&r`, `*&r`, `(&r)[2]`. GCC
/// `grok_reference_init` and Clang `Sema::ActOnUninitializedDecl` find a reference type in the
/// same way.
///
/// Each family of named declarators, `_declarator` and `_field_declarator`, gets four hidden
/// rules: `_of_name` (a name, also in parentheses or with attributes), `_of_function`,
/// `_of_object`, and `_of_reference`. Each declarator rule gets one copy for each class that it can
/// have, and the supertype is the choice of the four classes. Each declarator has exactly one class.
/// The parser then reduces a declarator to its class with no conflict, also where two items of a
/// parse state take different classes.
///
/// A function definition takes only the class of a function ([dcl.fct.def.general]). A context
/// that also accepts other classes, and that can come in the same parse state, names each class in
/// place of the supertype. After a function declarator, the parser then continues each item with
/// the next token. It does not reduce to the supertype for one item before the token that selects
/// the item.
fn declarator_classes(g: &mut Grammar) {
    split_declarator_family(
        g,
        "_declarator",
        &[
            ("attributed_declarator", Wrapper::SameClass),
            ("parenthesized_declarator", Wrapper::SameClass),
            ("function_declarator", Wrapper::Function),
            ("pointer_declarator", Wrapper::Object),
            ("reference_declarator", Wrapper::Reference),
            ("array_declarator", Wrapper::Object),
            ("block_pointer_declarator", Wrapper::Object),
            ("member_pointer_declarator", Wrapper::Object),
        ],
    );
    split_declarator_family(
        g,
        "_field_declarator",
        &[
            ("attributed_field_declarator", Wrapper::SameClass),
            ("parenthesized_field_declarator", Wrapper::SameClass),
            ("function_field_declarator", Wrapper::Function),
            ("pointer_field_declarator", Wrapper::Object),
            ("reference_field_declarator", Wrapper::Reference),
            ("array_field_declarator", Wrapper::Object),
            ("block_pointer_field_declarator", Wrapper::Object),
            ("member_pointer_field_declarator", Wrapper::Object),
        ],
    );
}

/// A declarator of each class of the family `family`, `_declarator` or `_field_declarator`.
fn declarator_of_each_class(family: &str) -> Rule {
    choice_of(DECLARATOR_CLASSES.map(|class| sym(&format!("{family}_of_{class}"))))
}

/// A declarator of the family `family` that does not declare a function: a name, an object, or a
/// reference.
///
/// A braced initializer and a parenthesized initializer take this declarator. After a function
/// declarator, `{` starts the body: GCC `cp_parser_init_declarator` and
/// `cp_parser_member_declaration`, Clang `Parser::isStartOfFunctionDefinition` and
/// `ParseCXXClassMemberDeclaration`. `void (*p)() {};` declares a variable with an initializer.
fn declarator_of_variable(family: &str) -> Rule {
    choice_of(["name", "object", "reference"].map(|class| sym(&format!("{family}_of_{class}"))))
}

/// The members of a choice, with the members of each choice in it. A rule that is not a choice is
/// its only member. O(n) in the members.
fn choice_members(rule: Rule) -> Vec<Rule> {
    match rule {
        Rule::Choice(members) => members.into_iter().flat_map(choice_members).collect(),
        other => vec![other],
    }
}

/// Define the class rules of one declarator family and the copies of its declarator rules.
///
/// Each member of the supertype `family` is a symbol or an alias of a symbol. A member rule in
/// `wrappers` gets one copy for each class that it can have. The first copy keeps the name of the
/// rule, and each other copy is an alias to the node name of the member. Each other member rule is a
/// name. O(n) in the size of the family rules.
///
/// # Panics
///
/// If a member of the supertype is not a symbol or an alias of a symbol.
fn split_declarator_family(g: &mut Grammar, family: &str, wrappers: &[(&str, Wrapper)]) {
    let class = |index: usize| sym(&format!("{family}_of_{}", DECLARATOR_CLASSES[index]));
    let (name, function, object, reference) = (0, 1, 2, 3);
    let mut classes: [Vec<Rule>; 4] = [vec![], vec![], vec![], vec![]];
    for member in choice_members(g.rules[family].clone()) {
        let not_a_symbol = |other: &Rule| -> ! {
            panic!("a member of `{family}` is not a symbol or an alias of a symbol: {other:?}")
        };
        let (rule, node) = match &member {
            Rule::Symbol(rule) => (rule.clone(), rule.clone()),
            Rule::Alias { content, value, .. } => match &**content {
                Rule::Symbol(rule) => (rule.clone(), value.clone()),
                other => not_a_symbol(other),
            },
            other => not_a_symbol(other),
        };
        let Some(&(_, wrapper)) = wrappers.iter().find(|(wrapper, _)| *wrapper == rule) else {
            classes[name].push(member);
            continue;
        };
        // The class of each copy, and the classes of its inner declarator.
        let copies: &[(usize, &[usize])] = match wrapper {
            Wrapper::SameClass => &[
                (name, &[name]),
                (function, &[function]),
                (object, &[object]),
                (reference, &[reference]),
            ],
            Wrapper::Function => &[
                (function, &[name, function]),
                (object, &[object]),
                (reference, &[reference]),
            ],
            Wrapper::Object => &[
                (object, &[name, object]),
                (function, &[function]),
                (reference, &[reference]),
            ],
            Wrapper::Reference => &[
                (reference, &[name, reference]),
                (function, &[function]),
                (object, &[object]),
            ],
        };
        let original = g.rules[&rule].clone();
        let mut previous = rule.clone();
        for (position, &(copy_class, inner_classes)) in copies.iter().enumerate() {
            let inner = match inner_classes {
                [single] => class(*single),
                several => choice_of(several.iter().map(|&index| class(index))),
            };
            let copy = replace_rule(original.clone(), &sym(family), &inner);
            if position == 0 {
                g.define(&rule, copy);
                classes[copy_class].push(member.clone());
            } else {
                let name = format!("_{rule}_of_{}", DECLARATOR_CLASSES[copy_class]);
                define_after(g, &previous, &name, copy);
                classes[copy_class].push(alias(sym(&name), sym(&node)));
                previous = name;
            }
        }
    }
    let mut previous = family.to_owned();
    for (index, members) in classes.into_iter().enumerate() {
        let name = format!("{family}_of_{}", DECLARATOR_CLASSES[index]);
        define_after(g, &previous, &name, Rule::Choice(members));
        previous = name;
    }
    g.define(family, declarator_of_each_class(family));
}

/// Define a new rule immediately after the rule `after` in the order of the rules.
///
/// The order of the rules gives the order of the symbols. When two parses of an ambiguity have the
/// same precedences, the parser selects the parse with the earlier symbols, and the order of the
/// parse versions also follows the order of the actions of the symbols. A copy next to its original
/// has the same place as the original among the other symbols, and these selections do not change.
/// O(n) in the number of rules.
///
/// # Panics
///
/// If the grammar has no rule `after`.
fn define_after(g: &mut Grammar, after: &str, name: &str, rule: Rule) {
    let index = g
        .rules
        .get_index_of(after)
        .unwrap_or_else(|| panic!("the grammar has no rule `{after}`"));
    g.rules.shift_insert(index + 1, name.to_owned(), rule);
}

fn statements(g: &mut Grammar) {
    let added = || {
        [
            "co_return_statement",
            "co_yield_statement",
            "for_range_loop",
            "expansion_statement",
            "try_statement",
            "throw_statement",
            "contract_assert_statement",
            "qt_emit_statement",
            "qt_foreach_statement",
            "qt_forever_statement",
            "ms_asm_statement",
        ]
        .map(sym)
    };
    // An MS inline assembly block: `__asm { mov eax, 1 }`. The external scanner reads the block as
    // text (Clang ParseStmtAsm.cpp, ParseMicrosoftAsmStatement). A GNU `__asm ("...")` has `(`
    // after the keyword, and the brace tells the two apart.
    g.define(
        "ms_asm_statement",
        seq![
            "__asm",
            open_brace(),
            optional(field("assembly_code", s!(ms_asm_code))),
            close_brace()
        ],
    );
    g.redefine("_top_level_statement", |original| {
        choice_of([original].into_iter().chain(added()))
    });
    g.redefine("_non_case_statement", |original| {
        choice_of([original].into_iter().chain(added()))
    });
    // A declaration statement is a statement in C++ ([stmt.pre]). The substatement of a selection statement
    // or an iteration statement then also takes a block declaration: `if (c) int x = 1;`, `else
    // static_assert(false);` (GCC `cp_parser_implicitly_scoped_statement` and `cp_parser_block_declaration`,
    // Clang `ParseStatementOrDeclarationAfterAttributes`). A declaration statement takes no function
    // definition (GCC `cp_parser_simple_declaration`). C does not permit a declaration there, and the grammar
    // accepts it in C code too.
    g.define(
        "_substatement",
        choice![
            s!(statement),
            alias(s!(_block_declaration), s!(declaration)),
            s!(type_definition),
            s!(_block_empty_declaration),
            s!(alias_declaration),
            s!(using_declaration),
            s!(static_assert_declaration),
            s!(namespace_alias_definition),
            s!(consteval_block_declaration),
        ],
    );
    for name in ["else_clause", "do_statement", "for_statement"] {
        g.redefine(name, |original| {
            replace_rule(original, &s!(statement), &s!(_substatement))
        });
    }
    // Qt defines `emit` and `Q_EMIT` as empty words before a signal call: `emit changed(x);`. The
    // external scanner gives the marker only when an identifier follows the word.
    g.define(
        "qt_emit_statement",
        seq![
            s!(_qt_emit_marker),
            choice!["emit", "Q_EMIT"],
            choice![s!(expression), s!(comma_expression)],
            ";",
        ],
    );
    // Qt defines `foreach` and `Q_FOREACH` as a loop on a container: `foreach (const T &x, list)`.
    // The first argument is a declaration or a variable. The external scanner gives the marker
    // only when the argument list has a comma at its top level.
    g.define(
        "qt_foreach_statement",
        seq![
            s!(_qt_foreach_marker),
            choice!["foreach", "Q_FOREACH"],
            "(",
            choice![
                seq![s!(_declaration_specifiers), field("declarator", s!(_declarator))],
                field("left", s!(expression)),
            ],
            ",",
            field("right", s!(expression)),
            ")",
            field("body", s!(_substatement)),
        ],
    );
    // Qt defines `forever` and `Q_FOREVER` as `for (;;)`.
    g.define(
        "qt_forever_statement",
        seq![choice!["forever", "Q_FOREVER"], field("body", s!(_substatement))],
    );
    g.define(
        "switch_statement",
        seq![
            "switch",
            field("condition", s!(condition_clause)),
            field("body", s!(compound_statement)),
        ],
    );
    g.define(
        "while_statement",
        seq![
            "while",
            field("condition", s!(condition_clause)),
            field("body", s!(_substatement)),
        ],
    );
    g.define(
        "if_statement",
        prec_right(
            0,
            seq![
                "if",
                choice![
                    // A macro can come in the place of `constexpr`: `if PLF_CONSTEXPR (x)` in plf
                    // (Cataclysm-DDA src/third-party/plf/list.h). plf defines `PLF_CONSTEXPR` as `constexpr`
                    // or as nothing. GCC `cp_parser_selection_statement` and Clang `Parser::ParseIfStatement`
                    // read `constexpr` after `if`.
                    seq![
                        optional(choice!["constexpr", s!(attribute_macro)]),
                        field("condition", s!(condition_clause))
                    ],
                    // C++23: `if consteval`, `if !consteval`, and `if not consteval`. The
                    // alternative token `not` is the same token as `!` (cp_parser_selection_statement
                    // in GCC).
                    seq![optional(choice!["!", "not"]), "consteval"],
                ],
                field("consequence", s!(_substatement)),
                optional(field("alternative", s!(else_clause))),
            ],
        ),
    );
    // With `prec(1)` in place of `prec.dynamic(1)`, a range loop with `int` in its
    // declaration specifiers always selects the other for loop, and the parse fails.
    // C++23 permits an alias declaration as the initializer, and C++26 permits a
    // structured binding declaration as the condition.
    let optional_expression = || optional(choice![s!(expression), s!(comma_expression)]);
    g.define(
        "_for_statement_body",
        prec_dynamic(
            1,
            seq![
                choice![
                    field("initializer", alias(s!(_block_declaration), s!(declaration))),
                    field("initializer", s!(alias_declaration)),
                    seq![field("initializer", optional_expression()), ";"],
                ],
                field(
                    "condition",
                    optional(choice![
                        s!(expression),
                        s!(comma_expression),
                        alias(s!(condition_declaration), s!(declaration)),
                        alias(s!(_structured_binding_condition), s!(declaration)),
                    ])
                ),
                ";",
                field("update", optional_expression()),
            ],
        ),
    );
    g.define(
        "for_range_loop",
        seq!["for", "(", s!(_for_range_loop_body), ")", field("body", s!(_substatement))],
    );
    g.define(
        "_for_range_loop_body",
        seq![
            field("initializer", optional(s!(init_statement))),
            choice![
                seq![s!(_declaration_specifiers), field("declarator", s!(_declarator))],
                seq![
                    s!(_structured_binding_specifiers),
                    field("declarator", s!(_structured_binding_declarator)),
                ],
            ],
            ":",
            field("right", choice![s!(expression), s!(initializer_list)]),
        ],
    );
    g.define(
        "init_statement",
        choice![
            s!(alias_declaration),
            s!(type_definition),
            alias(s!(_block_declaration), s!(declaration)),
            s!(expression_statement),
        ],
    );
    g.define(
        "condition_clause",
        seq![
            "(",
            field("initializer", optional(s!(init_statement))),
            field(
                "value",
                choice![
                    s!(expression),
                    s!(comma_expression),
                    alias(s!(condition_declaration), s!(declaration)),
                    alias(s!(_structured_binding_condition), s!(declaration)),
                ]
            ),
            ")",
        ],
    );
    // An init statement in the same condition has an init declarator. For this reason, each form
    // names the classes. Refer to `declarator_classes`.
    g.define(
        "condition_declaration",
        seq![
            s!(_declaration_specifiers),
            choice![
                seq![
                    field("declarator", declarator_of_each_class("_declarator")),
                    "=",
                    field("value", s!(expression))
                ],
                seq![
                    field("declarator", declarator_of_variable("_declarator")),
                    field("value", s!(initializer_list))
                ],
            ],
        ],
    );
    // C++26 permits a structured binding declaration as a condition: `if (auto [ok, v] = g())`. Its
    // initializer can also be `(e)`, as [stmt.pre] and GCC `cp_parser_condition` permit. Clang
    // rejects that form in `ParseCXXCondition`.
    g.define(
        "_structured_binding_condition",
        seq![
            s!(_structured_binding_specifiers),
            field("declarator", s!(_structured_binding_declarator)),
            choice![
                seq!["=", field("value", s!(expression))],
                field("value", choice![s!(initializer_list), s!(argument_list)]),
            ],
        ],
    );
    g.redefine("return_statement", |original| {
        seq![choice![original, seq!["return", s!(initializer_list), ";"]]]
    });
    // GCC `cp_parser_jump_statement` and `cp_parser_yield_expression`: `co_return` and `co_yield`
    // take an expression or a braced-init-list, `co_return {};`.
    g.define(
        "co_return_statement",
        seq![
            "co_return",
            optional(choice![s!(expression), s!(comma_expression), s!(initializer_list)]),
            ";"
        ],
    );
    g.define(
        "co_yield_statement",
        seq!["co_yield", choice![s!(expression), s!(initializer_list)], ";"],
    );
    g.define("throw_statement", seq!["throw", optional(s!(expression)), ";"]);
    // C++26 `contract_assert [[attr]] (x > 0);` (cp_parser_contract_assert in GCC).
    g.define(
        "contract_assert_statement",
        seq![
            "contract_assert",
            repeat(s!(attribute_declaration)),
            "(",
            field("condition", s!(expression)),
            ")",
            ";",
        ],
    );
    g.define(
        "try_statement",
        seq!["try", field("body", s!(compound_statement)), repeat1(s!(catch_clause))],
    );
    g.define(
        "catch_clause",
        seq![
            "catch",
            field("parameters", s!(parameter_list)),
            field("body", s!(compound_statement)),
        ],
    );
    // GNU C permits a case range: `case 1 ... 5:`.
    g.define(
        "case_statement",
        prec_right(
            0,
            seq![
                choice![
                    seq![
                        "case",
                        field("value", s!(expression)),
                        optional(seq!["...", field("end", s!(expression))]),
                    ],
                    "default",
                ],
                ":",
                repeat(s!(_case_body_item)),
            ],
        ),
    );
    // GCC reads GNU attributes and standard attributes together at the start of a statement (parser.cc,
    // cp_parser_statement and cp_parser_attributes_opt). Clang reads them in ParseStmt.cpp,
    // ParseStatementOrDeclarationAfterAttributes. `__attribute__((fallthrough));` is then an attributed null
    // statement. Before a declaration, the attributes stay specifiers of the declaration, as the two front
    // ends read them: `__attribute__((unused)) int x;`.
    //
    // A statement and a declaration can start with the same GNU attributes: `__attribute__((unused)) T(x);`.
    // Clang reads a declaration when an attribute is not a statement attribute (ParseStmt.cpp, lines 211 to
    // 215), and GCC tries the declaration first (parser.cc, cp_parser_statement). At namespace scope, no
    // statement is valid. Each GNU attribute specifier of an attributed statement then has this negative
    // dynamic precedence, and a declaration of the same tokens gets the tokens. The precedence is less than
    // the sum of the negative precedences of a declaration in real code. A statement with no declaration of
    // the same tokens keeps its attributes: `__attribute__((fallthrough));`, `__attribute__((nomerge)) f();`.
    const GNU_ATTRIBUTED_STATEMENT: i32 = -1000;
    //
    // The GNU attributes have their own rule with the node name `attributed_statement`, and the rule of the
    // standard attributes does not change. A list of the two kinds gives two nested attributed statements. A
    // block item, a case body, a label, and a substatement hold the rule, and the items of the top level do
    // not. At file scope, GCC and Clang read GNU attributes only at the start of a declaration.
    //
    // After GNU attributes, a line with only a macro name usually gives specifiers of the declaration on the
    // next lines: `__attribute__((__nonnull__))`, `_GLIBCXX20_CONSTEXPR`, `void f(int *p);`. For this reason,
    // the statement after GNU attributes is not a macro invocation. The scanner then gives no macro token
    // after the attributes, and the name is an attribute macro of the declaration. The rule of the statements
    // after GNU attributes is inline, and the tree has no node for it.
    g.define(
        "_gnu_attributed_statement",
        seq![
            repeat1(prec_dynamic(GNU_ATTRIBUTED_STATEMENT, s!(attribute_specifier))),
            s!(_gnu_attributed_statement_body),
        ],
    );
    g.redefine("_non_case_statement", |original| {
        choice![original, alias(s!(_gnu_attributed_statement), s!(attributed_statement))]
    });
    /// The members of a choice, with the members of each nested choice in its place. O(n) in the size of the
    /// choice.
    fn choice_members(rule: Rule) -> Vec<Rule> {
        match rule {
            Rule::Choice(members) => members.into_iter().flat_map(choice_members).collect(),
            other => vec![other],
        }
    }
    let statements_after_attributes = std::iter::once(s!(case_statement))
        .chain(choice_members(g.rules["_non_case_statement"].clone()))
        .filter(|member| *member != s!(macro_invocation));
    g.define("_gnu_attributed_statement_body", choice_of(statements_after_attributes));
    g.inline.push("_gnu_attributed_statement_body".to_owned());
    // After `lab: __attribute__((x))`, the attributes start an attributed statement or they are the
    // attributes of the label. Only the token after the attributes tells them apart, and the dynamic
    // precedence of `labeled_statement` selects the label before `;`.
    g.conflicts.push(vec!["_gnu_attributed_statement".to_owned(), "labeled_statement".to_owned()]);
    // A macro before a statement keyword is an attribute of that statement:
    // `MUST_TAIL_CALL return g(a);`. The macro expands to `[[clang::musttail]]` in ladybird and
    // protobuf, and Clang builds an `AttributedStmt` with a `MustTailAttr` and the return statement.
    // The external scanner gives the empty token before the name only when uppercase macro names and
    // a keyword of `STATEMENT_ATTRIBUTE_WORDS` come after it on the same line. `return` starts no
    // declaration, and the name was the type of a declaration before this rule.
    // Right associativity gives a `(` after the name to the arguments of the macro, and not to a
    // parenthesized expression of the statement after it.
    g.define(
        "_statement_attribute_macro",
        prec_right(
            0,
            seq![
                s!(_statement_attribute_macro_start),
                field("name", s!(identifier)),
                optional(field("arguments", s!(argument_list))),
            ],
        ),
    );
    g.define(
        "_macro_attributed_statement",
        seq![
            repeat1(alias(s!(_statement_attribute_macro), s!(attribute_macro))),
            s!(_gnu_attributed_statement_body),
        ],
    );
    g.redefine("_non_case_statement", |original| {
        choice![original, alias(s!(_macro_attributed_statement), s!(attributed_statement))]
    });
    // C++23 permits a label at the end of a compound statement. Right associativity gives the
    // statement after a label to the label, and a label is empty only before `}`. A label applies to
    // the statement after the directive lines (GCC `cp_parser_statement`, Clang `ParseLabeledStatement`).
    // A conditional group in that position is a child of the label, before the statement.
    //
    // GNU attributes after the colon are attributes of the label when `;` comes after them:
    // `out: __attribute__((unused));` (GCC parser.cc, cp_parser_label_for_labeled_statement. Clang ParseStmt.cpp,
    // ParseLabeledStatement). The label then holds the attributes and the null statement. The dynamic
    // precedence selects this tree over a label with an attributed null statement. Before a different
    // statement or a declaration, C++ gives the attributes to that item: `lab: __attribute__((cold)) f();`.
    // GCC C and Clang C give them to the label (c-parser.cc, c_parser_label), and the tree of such C code
    // is different from the front ends. GCC C++ gives `fallthrough` before `;` to the null statement, and
    // the tree gives it to the label, as Clang does.
    let label_groups = [
        alias(s!(preproc_if_in_block), s!(preproc_if)),
        alias(s!(preproc_ifdef_in_block), s!(preproc_ifdef)),
        alias(s!(preproc_if_in_case), s!(preproc_if)),
        alias(s!(preproc_ifdef_in_case), s!(preproc_ifdef)),
    ];
    g.define(
        "labeled_statement",
        prec_right(
            0,
            seq![
                field("label", s!(_statement_identifier)),
                ":",
                repeat(choice_of(label_groups)),
                optional(choice![
                    s!(_labeled_item),
                    prec_dynamic(
                        1,
                        seq![
                            repeat1(s!(attribute_specifier)),
                            alias(s!(_label_null_statement), s!(expression_statement)),
                        ]
                    ),
                ]),
            ],
        ),
    );
    // The null statement after the attributes of a label. The `;` is a child of the statement, as in an
    // expression statement with no expression.
    g.define("_label_null_statement", Rule::from(";"));
    // A GNU label declaration: `__label__ a, b;`. Each name declares a label of the block that holds the
    // declaration, and a goto statement in a nested function can use such a label (GCC
    // `c_parser_compound_statement_nostart` and `cp_parser_label_declaration`, Clang
    // `ParseCompoundStatementBody`). The names are labels, as in a goto statement. Only a compound statement
    // holds this declaration, before its items.
    g.define(
        "label_declaration",
        seq!["__label__", comma_sep1(field("label", s!(_statement_identifier))), ";"],
    );
    // GNU C permits a computed goto: `goto *p;`.
    g.define(
        "goto_statement",
        seq![
            "goto",
            choice![
                field("label", s!(_statement_identifier)),
                seq!["*", field("label", s!(expression))],
            ],
            ";",
        ],
    );
}

fn expressions(g: &mut Grammar) {
    g.redefine("_expression_not_binary", |original| {
        choice_of(
            [original].into_iter().chain(
                [
                    "co_await_expression",
                    "co_yield_expression",
                    "throw_expression",
                    "requires_expression",
                    "template_function",
                    "qualified_identifier",
                    "new_expression",
                    "delete_expression",
                    "lambda_expression",
                    "parameter_pack_expansion",
                    "this",
                    "user_defined_literal",
                    "fold_expression",
                    "reflect_expression",
                    "splice_expression",
                    "typeid_expression",
                    "noexcept_expression",
                    "pack_index_expression",
                    // An operator function by name: `&operator*()`.
                    "operator_name",
                    "block_literal",
                    "va_arg_expression",
                    "availability_check_expression",
                    "type_trait_expression",
                ]
                .map(sym),
            ),
        )
    });
    // A name before a `<` that the external scanner reads as an operand of a comparison, and not as
    // the name of a template-id: `x < 0 || x > (n - 1)`. Refer to `scan_comparison_name` in
    // `src/scanner.c`.
    g.redefine("_expression_not_binary", |original| {
        choice![original, alias(s!(_comparison_name), s!(identifier))]
    });
    // A conversion function by name is an id-expression: `&X::operator bool`, `return operator bool();`.
    // GCC `cp_parser_unqualified_id` and Clang `ParseUnqualifiedIdOperator` read a conversion-function-id
    // after each scope. The name has the shape of the name after a member access. Refer to
    // `_conversion_function_id`.
    g.redefine("_expression_not_binary", |original| {
        choice![
            original,
            alias(s!(_conversion_function_id), s!(operator_cast)),
            alias(s!(_qualified_conversion_function_id), s!(qualified_identifier)),
        ]
    });
    // A GCC or Clang trait with type arguments: `__is_same(T, U)`, `__is_constructible(T, Args...)`
    // (GCC parser.cc, cp_parser_trait. Clang ParseExprCXX.cpp, ParseTypeTrait). The external scanner
    // gives a marker only for the name of a trait before `(`. A library template with the name of a
    // trait, for example `__is_arithmetic<T>`, stays a name. The dynamic precedences are those of a
    // template argument list. A trait that gives a type, for example `__underlying_type(E)`, has no
    // rule. A marker at each start of a type makes nested template argument lists fail.
    let trait_arguments = || {
        seq![
            "(",
            comma_sep1(choice![
                field("type", prec_dynamic(3, s!(type_descriptor))),
                field(
                    "type",
                    prec_dynamic(
                        2,
                        alias(s!(type_parameter_pack_expansion), s!(parameter_pack_expansion))
                    )
                ),
                field("value", prec_dynamic(1, s!(expression))),
            ]),
            ")",
        ]
    };
    g.define(
        "type_trait_expression",
        seq![s!(_type_trait_marker), field("name", s!(identifier)), trait_arguments()],
    );
    // A GCC or Clang trait that gives a type: `__underlying_type(E)`, `__remove_cv(const int)`.
    // GCC reads it as a simple-type-specifier in `cp_parser_simple_type_specifier`
    // (parser.cc:22756 to :22767) with `cp_parser_trait` (parser.cc:12586), and the argument is one
    // type-id (parser.cc:12626). Clang reads it in `MaybeParseTypeTransformTypeSpecifier`
    // (ParseDeclCXX.cpp:1314) with one `ParseTypeName` (ParseDeclCXX.cpp:1327), and it builds a
    // `UnaryTransformType`. The rule is a type specifier, so that a declaration, an alias, a
    // parameter, a member, a trailing return type, and a template argument each read the trait
    // through `type_descriptor`.
    //
    // The external scanner gives a marker only for the name of such a trait before `(`. The two
    // front ends do the same check: `cp_lexer_peek_trait` in GCC and the `l_paren` check at
    // ParseDeclCXX.cpp:1315. A library template with the name of a trait, for example
    // `__make_signed<T>`, stays a name.
    g.define(
        "type_trait_specifier",
        seq![
            s!(_type_trait_type_marker),
            field("name", s!(identifier)),
            "(",
            field("type", s!(type_descriptor)),
            ")",
        ],
    );
    // The `va_arg` macro and GCC `__builtin_va_arg` take a type: `va_arg(ap, const char *)`
    // (GCC c-common.cc, RID_VA_ARG. Clang ParseExpr.cpp, ParseBuiltinPrimaryExpression). The
    // external scanner gives the marker only when `(` follows `va_arg`, so that a macro argument
    // `INSTKEYWORD(va_arg, VAArg)` stays an identifier.
    g.define(
        "va_arg_expression",
        seq![
            choice!["__builtin_va_arg", seq![s!(_va_arg_marker), "va_arg"]],
            "(",
            field("value", s!(expression)),
            ",",
            field("type", s!(type_descriptor)),
            ")",
        ],
    );
    // GCC and Clang read a member designator in `offsetof`: `offsetof(S, a.b[1])` (GCC parser.cc,
    // cp_parser_builtin_offsetof). A member with one name keeps the tree of the C rule.
    g.define(
        "offsetof_expression",
        prec(
            OFFSETOF,
            seq![
                choice!["offsetof", "__builtin_offsetof"],
                "(",
                field("type", s!(type_descriptor)),
                ",",
                field("member", s!(_field_identifier)),
                repeat(choice![s!(field_designator), s!(subscript_designator)]),
                ")",
            ],
        ),
    );
    // Clang checks the platform version: `__builtin_available(macOS 10.12, iOS 10, *)`. A version
    // can be a macro (ParseExpr.cpp, ParseAvailabilityCheckExpr).
    g.define(
        "availability_check_expression",
        seq![
            "__builtin_available",
            "(",
            comma_sep1(choice![
                "*",
                seq![s!(identifier), repeat1(choice![s!(number_literal), s!(identifier)])],
            ]),
            ")",
        ],
    );
    // A Clang block literal: `^{ ... }`, `^(int x) { ... }`, `^int (int x) { ... }`. A parameter
    // list or a type can come before the body, and GNU attributes can follow a parameter list
    // (ParseExpr.cpp, ParseBlockLiteralExpression). A type reads its own attributes.
    g.define(
        "block_literal",
        seq![
            "^",
            optional(choice![
                seq![field("parameters", s!(parameter_list)), repeat(s!(attribute_specifier))],
                field("type", s!(type_descriptor)),
            ]),
            field("body", s!(compound_statement)),
        ],
    );
    g.define(
        "_string",
        choice![s!(string_literal), s!(raw_string_literal), s!(concatenated_string)],
    );
    g.define(
        "raw_string_literal",
        seq![
            choice!["R\"", "LR\"", "uR\"", "UR\"", "u8R\""],
            choice![
                seq![
                    field("delimiter", s!(raw_string_delimiter)),
                    "(",
                    s!(raw_string_content),
                    ")",
                    s!(raw_string_delimiter),
                ],
                seq!["(", s!(raw_string_content), ")"],
            ],
            "\"",
        ],
    );
    g.define(
        "subscript_expression",
        prec(
            SUBSCRIPT,
            seq![
                field("argument", s!(expression)),
                // A splice as the index: `members[:i:]`.
                field(
                    "indices",
                    choice![s!(subscript_argument_list), s!(splice_specifier)]
                ),
            ],
        ),
    );
    g.define(
        "subscript_argument_list",
        seq![
            "[",
            comma_sep(choice![s!(expression), s!(initializer_list)]),
            close_bracket()
        ],
    );
    g.redefine("call_expression", |original| {
        prec_dynamic(
            CALL_DYNAMIC,
            choice![
                original,
                seq![
                    field(
                        "function",
                        choice![
                            s!(primitive_type),
                            s!(splice_type_specifier),
                            // A functional cast to a dependent type: `typename T::type(x)`. The
                            // dependent type also covers `typename [:r:](x)`.
                            s!(dependent_type),
                            // GCC `cp_parser_functional_cast`: `unsigned(x)`.
                            s!(sized_type_specifier),
                            // The C++23 decay-copy `auto(x)` (P0849R8, cp_parser_functional_cast
                            // in GCC).
                            alias(s!(_decay_copy_type), s!(placeholder_type_specifier)),
                        ]
                    ),
                    field("arguments", s!(argument_list)),
                ],
                // A functional cast to a class that the file declares: `A(x)`. Only name lookup tells
                // a type name from a function name in `T(x)` (GCC `cp_parser_postfix_expression`,
                // Clang `ParsePostfixExpressionSuffix`). The external scanner records the names of the
                // class heads of the file, and it gives `_functional_cast_name` for such a name before
                // a `(`, where a declaration cannot start. Refer to `scan_word_start` in
                // src/scanner.c. A name that the record does not hold keeps the reading of a call.
                seq![
                    field("function", alias(s!(_functional_cast_name), s!(type_identifier))),
                    field("arguments", s!(argument_list)),
                ],
                // The same cast to a class template: `B<int>(x)`. The scanner gives the token for a
                // recorded name whose angle brackets close before a `(`. Refer to
                // `scan_comparison_name` in src/scanner.c, which gives this token and the token of a
                // comparison in one scan.
                seq![
                    field(
                        "function",
                        alias(s!(_functional_cast_template), s!(template_type))
                    ),
                    field("arguments", s!(argument_list)),
                ],
                // A CUDA kernel call: `kernel<<<blocks, threads>>>(args)` (Clang ParseExpr.cpp,
                // ParsePostfixExpressionSuffix). The precedence is the precedence of `<<`, so that
                // the call does not change how the parser reads a shift expression.
                prec_left(
                    SHIFT,
                    seq![
                        field("function", s!(expression)),
                        field("configuration", s!(cuda_execution_configuration)),
                        field("arguments", s!(argument_list)),
                    ]
                ),
                // A functional cast to a decltype: `it = decltype(it)(x)`. GCC reads it in
                // `cp_parser_postfix_expression`, because a decltype-specifier is a
                // simple-type-specifier. The dynamic precedence is lower than the precedence of a
                // parenthesized declarator. `decltype(x)(y);` then stays a declaration of `y`, as
                // [stmt.ambig] says.
                prec_dynamic(
                    -20,
                    seq![field("function", s!(decltype)), field("arguments", s!(argument_list))]
                ),
            ],
        )
    });
    // `<<<` and `>>>` are two tokens each. One token `<<<` changes the tokens of `operator<<<T>`.
    // Each comma ends the step of its expression. The last expression then meets `>>` with no
    // reduction, and the shift of a shift expression `b >> c` cannot take the `>>` first.
    // The callee of a functional cast to a class template. The name comes from the external scanner,
    // and the rule has the shape of `template_type`, which it takes as its node kind.
    g.define(
        "_functional_cast_template",
        seq![
            field("name", alias(s!(_functional_cast_name), s!(type_identifier))),
            field("arguments", s!(template_argument_list)),
        ],
    );
    g.define(
        "cuda_execution_configuration",
        seq!["<<", "<", repeat(seq![s!(expression), ","]), s!(expression), ">>", ">"],
    );
    g.define(
        "co_await_expression",
        prec_left(
            UNARY,
            seq![field("operator", "co_await"), field("argument", s!(expression)),],
        ),
    );
    // GCC `cp_parser_throw_expression` and `cp_parser_yield_expression`, Clang
    // `ParseThrowExpression`: the two are assignment expressions, `c ? v : throw e` and
    // `auto x = co_yield v`. The shift of `;` has more precedence than the reduction, and
    // `throw e;` stays a throw statement.
    g.define(
        "throw_expression",
        prec_right(ASSIGNMENT, seq!["throw", optional(s!(expression))]),
    );
    g.define(
        "co_yield_expression",
        prec_right(
            ASSIGNMENT,
            seq!["co_yield", choice![s!(expression), s!(initializer_list)]],
        ),
    );
    // The noexcept operator of GCC `cp_parser_unary_expression`: `noexcept(f())`.
    g.define(
        "noexcept_expression",
        seq![
            "noexcept",
            "(",
            field("value", choice![s!(expression), s!(comma_expression)]),
            ")"
        ],
    );
    // GCC `cp_parser_new_expression` and Clang `ParseCXXNewExpression`. The type is a new-type-id
    // with cv-qualifiers and pointers, `new const char *[n]`, or a type-id in parentheses,
    // `new (int*)`. Only the token after `)` tells `new (T)` from the placement of `new (p) T`.
    let new_declarator = || {
        choice![
            s!(new_declarator),
            alias(s!(_new_pointer_declarator), s!(abstract_pointer_declarator)),
        ]
    };
    g.define(
        "new_expression",
        prec_right(
            NEW,
            seq![
                optional("::"),
                "new",
                field("placement", optional(s!(argument_list))),
                s!(_new_type_id),
                field("arguments", optional(choice![s!(argument_list), s!(initializer_list)])),
            ],
        ),
    );
    // The type of a new expression is one hidden rule. Otherwise each form of the type gets its
    // own copy of the parse states of the initializer. Real code puts only `const` and
    // `volatile` before the type.
    //
    // A new-type-id defines no class and no enumeration ([expr.new]), and the brace after
    // `new struct S` starts the new-initializer. GCC `cp_parser_new_type_id` (parser.cc:10942) stops
    // a definition with the message "types may not be defined in a new-type-id" (parser.cc:10957).
    // Clang `ParseCXXNewExpression` (ParseExprCXX.cpp:2989) reads the type-specifier-seq in
    // DeclaratorContext::CXXNew, and `isDefiningTypeSpecifierContext` (Parser.h:1668) gives DSC_new
    // the value AllowDefiningTypeSpec::No. Refer to `_nondefining_type_specifier`.
    g.define(
        "_new_type_id",
        prec_right(
            0,
            choice![
                seq![
                    repeat(alias(choice!["const", "volatile"], s!(type_qualifier))),
                    field("type", s!(_nondefining_type_specifier)),
                    field("declarator", optional(new_declarator())),
                ],
                seq!["(", field("type", s!(type_descriptor)), ")"],
            ],
        ),
    );
    // GCC `cp_parser_new_declarator_opt`: a ptr-operator before the array bounds. Attributes can
    // follow the `*`: `new int *[[a]][3]` (GCC `cp_parser_ptr_operator`, Clang
    // `ParseDeclaratorInternal`).
    g.define(
        "_new_pointer_declarator",
        prec_right(
            0,
            seq![
                "*",
                repeat(s!(attribute_declaration)),
                repeat(s!(type_qualifier)),
                field("declarator", optional(new_declarator()))
            ],
        ),
    );
    // GCC `cp_parser_direct_new_declarator`: the first bound can be empty, `new int[]{1, 2}`. A bound
    // is a full expression, and `new int[n, 1]` has a comma expression. Clang
    // `ParseDirectNewDeclarator` also reads the first bound with `ParseExpression`.
    g.define(
        "new_declarator",
        prec_right(
            0,
            seq![
                "[",
                field("length", optional(choice![s!(expression), s!(comma_expression)])),
                close_bracket(),
                // `new int[n] [[a]]`: GCC reads attributes in `cp_parser_direct_new_declarator`.
                repeat(s!(attribute_declaration)),
                optional(s!(new_declarator)),
            ],
        ),
    );
    // GCC `cp_parser_delete_expression`: `[` after `delete` always opens the array form. The
    // reduction of `[]` has more precedence than the reduction of a lambda capture, and `[]` in
    // `delete[] (char*)p` does not start a lambda. The operand is a cast expression, as the operand
    // of a unary operator (GCC `cp_parser_simple_cast_expression`, Clang `ParseCastExpression`), and
    // `delete p + 1` is `(delete p) + 1`. The precedence is only on the operand, as for
    // `__extension__`.
    g.define(
        "delete_expression",
        seq![
            optional("::"),
            "delete",
            optional(s!(_delete_array_brackets)),
            prec_left(UNARY, s!(expression))
        ],
    );
    g.define("_delete_array_brackets", prec(LAMBDA + 1, seq!["[", close_bracket()]));
    let member_access = |operators: Rule| {
        prec(
            FIELD,
            seq![field("argument", s!(expression)), field("operator", operators)],
        )
    };
    g.define(
        "field_expression",
        choice![
            seq![
                member_access(choice![".", "->"]),
                field(
                    "field",
                    choice![
                        prec_dynamic(1, s!(_field_identifier)),
                        prec_dynamic(1, alias(s!(_comparison_name), s!(field_identifier))),
                        alias(s!(qualified_field_identifier), s!(qualified_identifier)),
                        s!(destructor_name),
                        s!(template_method),
                        alias(s!(dependent_field_identifier), s!(dependent_name)),
                        s!(operator_name),
                        alias(s!(_conversion_function_id), s!(operator_cast)),
                        s!(splice_expression),
                    ]
                ),
            ],
            // `.*` and `->*` are binary operators, and each operand is a cast expression (GCC
            // `cp_parser_binary_expression`, Clang `ParseRHSOfBinaryExpression`). `*p.*m` is
            // `(*p).*m`, and `o->*t[i]()` is `o->*(t[i]())`. The node keeps the kind and the fields
            // of a member access: `argument` is the object, and `field` is the member pointer.
            prec_left(
                POINTER_TO_MEMBER,
                seq![
                    field("argument", s!(expression)),
                    field("operator", choice![".*", "->*"]),
                    field("field", s!(expression)),
                ],
            ),
        ],
    );
    // GCC `cp_parser_conversion_function_id`: a conversion function by name after a member
    // access, `x.operator bool()` and `p->operator const char *()`. Only ptr-operators follow
    // the type, and the call keeps its parentheses.
    //
    // At namespace scope, `operator T *();` and `X::operator T();` also start the declaration of a
    // conversion function (`operator_cast`). GCC and Clang read no expression statement there. The
    // precedence -1 is less than the precedence of the declaration specifiers, and the parser keeps
    // only the declaration. A block holds no such declaration, and the name stays an expression there.
    // The right associativity gives each ptr-operator after the type to the name.
    //
    // The qualifiers and the ptr-operators are rules, and each node holds its token, as in the declarator
    // of a conversion function. An alias of a sequence gives the alias to each token of the sequence, and
    // the node then has no token: `(type_qualifier)` with no `const`.
    let cv_qualifier = || alias(s!(_conversion_cv_qualifier), s!(type_qualifier));
    g.define(
        "_conversion_function_id",
        prec_right(
            -1,
            seq![
                "operator",
                repeat(cv_qualifier()),
                field("type", s!(_nondefining_type_specifier)),
                // The cv-qualifiers can also follow the type: `p->operator char const *()`.
                repeat(cv_qualifier()),
                field("declarator", optional(s!(_conversion_declarator))),
            ],
        ),
    );
    // At namespace scope, `operator const` also starts the specifiers of a declaration. The precedence -1
    // gives `const` to the declaration there.
    g.define("_conversion_cv_qualifier", prec(-1, choice!["const", "volatile"]));
    g.define(
        "_conversion_declarator",
        choice![
            alias(s!(_conversion_name_pointer_declarator), s!(abstract_pointer_declarator)),
            alias(s!(_conversion_name_reference_declarator), s!(abstract_reference_declarator)),
        ],
    );
    // Attributes can follow each ptr-operator of the name: `p->operator int *[[a]]()` (GCC
    // `cp_parser_conversion_declarator_opt` and `cp_parser_ptr_operator`).
    g.define(
        "_conversion_name_pointer_declarator",
        prec_right(
            0,
            seq![
                "*",
                repeat(s!(attribute_declaration)),
                repeat(s!(type_qualifier)),
                field("declarator", optional(s!(_conversion_declarator))),
            ],
        ),
    );
    g.define(
        "_conversion_name_reference_declarator",
        prec_right(
            0,
            seq![
                choice!["&", "&&"],
                repeat(s!(attribute_declaration)),
                optional(s!(_conversion_declarator))
            ],
        ),
    );
    // The `;` belongs to the type requirement. Otherwise `typename T` followed by `(` can also
    // start the next requirement, and `typename T::type(x);` is ambiguous.
    g.define("type_requirement", seq!["typename", s!(_class_name), ";"]);
    // The braces of a compound requirement hold a full expression, and the comma operator is part
    // of it ([expr.prim.req.compound]): `requires (T t) { { t, t }; }`. GCC
    // cp_parser_compound_requirement (parser.cc:35113) calls cp_parser_expression, and Clang
    // (ParseExprCXX.cpp:3264) calls ParseExpression.
    g.define(
        "compound_requirement",
        seq![
            open_brace(),
            choice![s!(expression), s!(comma_expression)],
            close_brace(),
            optional(s!(noexcept)),
            optional(s!(trailing_return_type)),
            ";",
        ],
    );
    g.define(
        "_requirement",
        choice![
            alias(s!(expression_statement), s!(simple_requirement)),
            s!(type_requirement),
            s!(compound_requirement),
            s!(nested_requirement),
        ],
    );
    // [expr.prim.req.nested] gives the nested requirement its own production,
    // `requires constraint-expression ;`, beside the simple requirement, the type requirement, and
    // the compound requirement. GCC `cp_parser_requirement` (parser.cc:34984) calls the separate
    // `cp_parser_nested_requirement` (parser.cc:35199) for it, and Clang `ParseRequiresExpression`
    // gives the same production in a comment (ParseExprCXX.cpp:3363) and builds a
    // `NestedRequirement` (ParseExprCXX.cpp:3376). The node holds the constraint directly, as the
    // three other requirements hold their operands.
    //
    // The constraint is an expression: `requires sizeof(T) == 4;`,
    // `requires !std::same_as<T, int>;` (GCC `cp_parser_constraint_expression`, Clang
    // `ParseConstraintExpression`). A simple requirement cannot start with `requires`.
    g.define(
        "nested_requirement",
        seq!["requires", field("constraint", s!(expression)), ";"],
    );
    g.define(
        "requirement_seq",
        seq![open_brace(), repeat(s!(_requirement)), close_brace()],
    );
    g.define(
        "constraint_conjunction",
        prec_left(
            LOGICAL_AND,
            seq![
                field("left", s!(_requirement_clause_constraint)),
                field("operator", choice!["&&", "and"]),
                field("right", s!(_requirement_clause_constraint)),
            ],
        ),
    );
    g.define(
        "constraint_disjunction",
        prec_left(
            LOGICAL_OR,
            seq![
                field("left", s!(_requirement_clause_constraint)),
                field("operator", choice!["||", "or"]),
                field("right", s!(_requirement_clause_constraint)),
            ],
        ),
    );
    // The operand of a constraint is a primary expression ([temp.constr.decl]). GCC
    // cp_parser_constraint_primary_expression (parser.cc:34612) calls
    // cp_parser_primary_expression, and Clang ParseConstraintLogicalAndExpression
    // (ParseExpr.cpp:197) calls ParseCastExpression with PrimaryExprOnly. A trait is such a
    // primary expression (parser.cc:7000, ParseExpr.cpp:1531), and the parentheses hold a full
    // expression with the comma operator (parser.cc:6745, ParseExpr.cpp:790). A unary expression
    // is no primary expression, and `requires !C<T>` is an error in the two front ends.
    //
    // The parentheses give a `parenthesized_expression`, as they do in a concept definition and in
    // a nested requirement. GCC reads them in `cp_parser_primary_expression` (parser.cc:6745) and
    // Clang in `ParseParenExpression` (ParseExpr.cpp:2662). Clang builds a `ParenExpr` in each of
    // the three positions. The same text then gives the same tree in each position.
    //
    // A name in a constraint is an id-expression, and it names a value or a concept. The names are
    // the names of an expression, and not the names of a type: `identifier`,
    // `qualified_identifier`, `template_function`, and `splice_expression`. In
    // `requires S<T>::value`, the scope `S<T>` is a nested-name-specifier and stays a
    // `template_type`, and the name `value` is an `identifier`. GCC
    // `cp_parser_constraint_primary_expression` (parser.cc:34612) reads the name with
    // `cp_parser_primary_expression`, and Clang `ParseConstraintLogicalAndExpression`
    // (ParseExpr.cpp:203) reads it with `ParseCastExpression`. Clang builds a `DeclRefExpr` for a
    // plain name, a `DependentScopeDeclRefExpr` for a qualified name, and a
    // `ConceptSpecializationExpr` for a concept-id. A concept definition of the same text already
    // gives these nodes.
    g.define(
        "_requirement_clause_constraint",
        choice![
            // Primary expressions
            s!(true),
            s!(false),
            s!(_constraint_name),
            s!(fold_expression),
            s!(lambda_expression),
            s!(requires_expression),
            s!(type_trait_expression),
            s!(parenthesized_expression),
            // A conjunction or a disjunction of the constraints above
            s!(constraint_conjunction),
            s!(constraint_disjunction),
        ],
    );
    // The names of an expression, with the shape of `_class_name`. A name in a constraint takes
    // these nodes, and a name in a type requirement takes the nodes of `_class_name`.
    g.define(
        "_constraint_name",
        prec_right(
            0,
            choice![
                s!(identifier),
                s!(template_function),
                alias(s!(_constraint_pack_index_template), s!(template_function)),
                s!(splice_expression),
                alias(s!(_constraint_qualified_identifier), s!(qualified_identifier)),
            ],
        ),
    );
    // C++26 gives a template parameter list a concept pack, and the name of a concept-id can then be
    // a pack index: `requires Cs...[0]<T>`. The rule has the shape of the second alternative of
    // `template_type`, with the node of an expression template-id. The precedence gives the `<`
    // after a pack index to the template arguments.
    g.define(
        "_constraint_pack_index_template",
        prec(
            1,
            seq![
                field("name", s!(pack_index_specifier)),
                field("arguments", s!(template_argument_list)),
            ],
        ),
    );
    // The qualified name of a constraint, with the shape of `qualified_type_identifier` and the
    // names of an expression. The name of a constraint is no declarator, and the rule takes no
    // member pointer. It also takes no operator function and no destructor, because a constraint
    // names a value or a concept.
    g.define(
        "_constraint_qualified_identifier",
        seq![
            s!(_scope_resolution),
            field(
                "name",
                choice![
                    alias(s!(dependent_identifier), s!(dependent_name)),
                    prec_dynamic(2, alias(s!(_constraint_qualified_identifier), s!(qualified_identifier))),
                    // The two names have the dynamic precedence of the names of
                    // `qualified_type_identifier`. In `requires ns::C<T>::value auto f();` the
                    // constraint is then the full name, as `cp_parser_primary_expression` and
                    // `ParseCastExpression` read it, and the `::value` starts no type constraint of
                    // the placeholder type.
                    prec_dynamic(1, s!(template_function)),
                    prec_dynamic(1, seq![optional("template"), s!(identifier)]),
                ]
            ),
        ],
    );
    g.define(
        "requires_clause",
        seq!["requires", field("constraint", s!(_requirement_clause_constraint))],
    );
    g.define(
        "requires_parameter_list",
        seq![
            "(",
            comma_sep(choice![
                s!(parameter_declaration),
                s!(optional_parameter_declaration),
                s!(variadic_parameter_declaration),
            ]),
            ")",
        ],
    );
    g.define(
        "requires_expression",
        seq![
            "requires",
            field(
                "parameters",
                optional(alias(s!(requires_parameter_list), s!(parameter_list)))
            ),
            field("requirements", s!(requirement_seq)),
        ],
    );
    g.define(
        "lambda_specifier",
        choice!["static", "constexpr", "consteval", "mutable"],
    );
    let attributes = || repeat(s!(attribute_declaration));
    // The parts after the lambda specifiers: an exception specification, attributes, and a trailing
    // return type ([expr.prim.lambda.general]).
    let lambda_declarator_end = || {
        seq![
            optional(s!(_function_exception_specification)),
            repeat(s!(attribute_declaration)),
            optional(s!(trailing_return_type)),
        ]
    };
    g.define(
        "lambda_declarator",
        choice![
            // The primary form, with a parameter list. Clang reads GNU attributes before the lambda
            // specifiers (ParseExprCXX.cpp, ParseLambdaExpressionAfterIntroducer). A macro can come
            // after the specifiers: `[](auto& st) TEST_ALIGN_BENCHMARK { ... }`.
            seq![
                attributes(),
                field("parameters", s!(parameter_list)),
                repeat(s!(attribute_specifier)),
                repeat(s!(lambda_specifier)),
                optional(s!(_function_exception_specification)),
                repeat(choice![
                    s!(attribute_declaration),
                    alias(s!(_trailing_attribute_macro), s!(attribute_macro))
                ]),
                optional(s!(trailing_return_type)),
                optional(s!(requires_clause)),
                repeat(s!(function_contract_specifier)),
            ],
            // The forms with no parameter list. Each form starts with a different part, and the
            // first token selects one form. GNU attributes can come before the lambda specifiers,
            // as after a parameter list: `[] __attribute__((noinline)) mutable { ... }`. GCC reads
            // them before the optional parameter list (cp_parser_lambda_declarator_opt), and Clang
            // reads them before the specifiers (ParseLambdaExpressionAfterIntroducer).
            repeat1(s!(attribute_declaration)),
            seq![
                attributes(),
                repeat1(s!(attribute_specifier)),
                repeat(s!(lambda_specifier)),
                lambda_declarator_end(),
            ],
            seq![attributes(), repeat1(s!(lambda_specifier)), lambda_declarator_end()],
            seq![
                attributes(),
                s!(_function_exception_specification),
                attributes(),
                optional(s!(trailing_return_type)),
            ],
            seq![attributes(), s!(trailing_return_type)],
        ],
    );
    g.define(
        "lambda_expression",
        seq![
            field("captures", s!(lambda_capture_specifier)),
            optional(seq![
                field("template_parameters", s!(template_parameter_list)),
                optional(field("constraint", s!(requires_clause))),
            ]),
            optional(field("declarator", s!(lambda_declarator))),
            field("body", s!(compound_statement)),
        ],
    );
    g.define(
        "lambda_capture_specifier",
        prec(
            LAMBDA,
            seq![
                "[",
                choice![
                    s!(lambda_default_capture),
                    comma_sep(s!(_lambda_capture)),
                    seq![s!(lambda_default_capture), ",", comma_sep1(s!(_lambda_capture))],
                ],
                close_bracket(),
            ],
        ),
    );
    g.define("lambda_default_capture", choice!["=", "&"]);
    g.define(
        "_lambda_capture_identifier",
        seq![
            optional("&"),
            choice![
                s!(identifier),
                s!(qualified_identifier),
                alias(s!(identifier_parameter_pack_expansion), s!(parameter_pack_expansion)),
            ],
        ],
    );
    // GCC `cp_parser_lambda_introducer` and Clang `ParseLambdaIntroducer`: an init-capture is a
    // name and an initializer, `[x = 1]`, `[x(1)]`, or `[x{1}]`.
    //
    // In a braced list, `{ [&r = v] = 1 }` has a GNU array designator, and `{ [&r = v]() {} }` has
    // a lambda. Only the token after `]` tells them apart (Clang ParseInit.cpp,
    // MayBeDesignationStart). In the designator, the parser reduces `r` to an expression before `=`,
    // and the init-capture shifts `=`. The two actions have no precedence, and the conflict set of
    // `expression` and `lambda_capture_initializer` in `grammar` keeps the two readings.
    let head = || seq![optional("&"), optional("...")];
    g.define(
        "lambda_capture_initializer",
        choice![
            seq![head(), field("left", s!(identifier)), "=", field("right", s!(expression))],
            seq![
                head(),
                field("left", s!(identifier)),
                field("right", choice![s!(argument_list), s!(initializer_list)]),
            ],
        ],
    );
    g.define(
        "_lambda_capture",
        choice![
            seq![optional("*"), s!(this)],
            s!(_lambda_capture_identifier),
            s!(lambda_capture_initializer),
        ],
    );
    // A fold and a comparison use the same `>=` token. With two tokens for `>=`, the lexer takes
    // the token with the higher precedence, and a fold with `>=` fails.
    g.define("_fold_operator", choice_of(FOLD_OPERATORS.map(operator_token)));
    g.define(
        "_binary_fold_operator",
        choice_of(FOLD_OPERATORS.map(|operator| {
            seq![
                field("operator", operator_token(operator)),
                "...",
                operator_token(operator)
            ]
        })),
    );
    g.define(
        "_unary_left_fold",
        seq![
            field("left", "..."),
            field("operator", s!(_fold_operator)),
            field("right", s!(expression)),
        ],
    );
    g.define(
        "_unary_right_fold",
        seq![
            field("left", s!(expression)),
            field("operator", s!(_fold_operator)),
            field("right", "..."),
        ],
    );
    g.define(
        "_binary_fold",
        seq![
            field("left", s!(expression)),
            s!(_binary_fold_operator),
            field("right", s!(expression)),
        ],
    );
    g.define(
        "fold_expression",
        seq![
            "(",
            choice![s!(_unary_right_fold), s!(_unary_left_fold), s!(_binary_fold)],
            ")",
        ],
    );
    g.define(
        "parameter_pack_expansion",
        prec(PACK_EXPANSION, seq![field("pattern", s!(expression)), "..."]),
    );
    // The dynamic precedence is on this rule and not on the template argument. The parser table has
    // no reduction from this rule to `_template_argument`, because the rule always has the same
    // alias, and a precedence on that reduction has no effect.
    g.define(
        "type_parameter_pack_expansion",
        prec_dynamic(2, seq![field("pattern", s!(type_descriptor)), "..."]),
    );
    g.define(
        "identifier_parameter_pack_expansion",
        seq![field("pattern", s!(identifier)), "..."],
    );
    // Only name lookup tells a type from an expression in `(T) * x`, `(T)(x)`, and `sizeof(T)` (GCC
    // `cp_parser_cast_expression` and `cp_parser_sizeof_operand`, Clang `ParseParenExpression`). The
    // external scanner reads a name in parentheses and the token after them, and it selects one
    // reading with its own `(` token. Refer to `scan_parenthesized_name` in `src/scanner.c`.
    let open_paren = |token: Rule| choice!["(", alias(token, "(")];
    g.redefine("cast_expression", |original| {
        replace_string(original, "(", &open_paren(s!(_cast_paren)))
    });
    g.redefine("parenthesized_expression", |original| {
        replace_string(original, "(", &open_paren(s!(_name_expression_paren)))
    });
    g.redefine("sizeof_expression", |original| {
        prec_right(
            SIZEOF,
            choice![
                replace_string(original, "(", &open_paren(s!(_operand_type_paren))),
                seq!["sizeof", "...", "(", field("value", s!(identifier)), ")"]
            ],
        )
    });
    // The operand of the GNU spellings of `alignof` is a type or an expression, and the scanner gives its
    // own `(` for a name in parentheses. A name there is a type: `__alignof__(locale)`. Only a name with
    // a call group is an expression: `__alignof__(f(x))`. The scanner gives no token after the standard
    // spellings, because a parenthesized expression is not valid there.
    //
    // A measurement of the corpus found the lowercase names `type`, `va_list`, `locale`, `istream`, and
    // `runtime_error` as the operand, and each one is a type. The shape of a name tells a type from a
    // variable for `sizeof`, and for this operator it does not. Refer to `scan_parenthesized_name` in
    // `src/scanner.c`.
    g.redefine("alignof_expression", |original| {
        replace_string(original, "(", &open_paren(s!(_alignof_type_paren)))
    });
    g.redefine("unary_expression", |original| {
        choice![
            original,
            prec_left(
                UNARY,
                seq![
                    field("operator", choice!["not", "compl"]),
                    field("argument", s!(expression))
                ]
            ),
        ]
    });
    g.redefine("binary_expression", |original| {
        // `>=` is one token in a template argument, as GCC and Clang read it.
        let original = replace_string(original, ">=", &greater_equal());
        // A comparison with `<` or `>` has the dynamic precedence -1. Where a name, `<`, and `>` can also
        // be a template-id, the reading with fewer of these comparisons wins, and a chain of two
        // comparisons loses: `hana::size_c<3> * hana::ushort_c<5>` multiplies two template-ids, and it is
        // not `(hana::size_c < 3) > *hana::ushort_c<5>`. Clang rejects a chain `a < b > c`
        // (`warn_consecutive_comparison` is an error by default in SemaExpr.cpp), and GCC warns about it
        // (`warn_about_parentheses` in c-warn.cc).
        let original = choice_of(original.into_members().into_iter().map(|member| {
            if matches!(binary_operator(&member), Some("<" | ">")) {
                prec_dynamic(-1, member)
            } else {
                member
            }
        }));
        let three_way = prec_left(
            THREE_WAY,
            seq![
                field("left", s!(expression)),
                field("operator", "<=>"),
                field("right", s!(expression)),
            ],
        );
        choice_of(
            [original, three_way]
                .into_iter()
                .chain(alternative_binary_operations("expression")),
        )
    });
    // The compound statement is for macros that take statements as arguments, for
    // example `MYFORLOOP(1, 10, i, { foo(i); bar(i); })`.
    g.define(
        "argument_list",
        seq![
            "(",
            comma_sep(choice![s!(expression), s!(initializer_list), s!(compound_statement)]),
            ")",
        ],
    );
    // The alternative token `compl` is `~` also in a destructor name: `compl S()`. GCC reads the
    // name after `~` as a type name, and a type name can be a template-id: `d.~GG<int>()`,
    // `A<T>::~A<T>()`, `p->~Base<T>()`. The copy of `template_type` has a precedence on its name,
    // and the right associativity of the destructor name gives the `<` after `~A` to the template
    // arguments. The precedence also applies to the reduction of the copy. For this reason a
    // declarator without a scope uses `_plain_destructor_name`: in a block, `~f<T>(x)` is also
    // the complement of a call, and C++20 does not permit a template-id there (CWG 2237).
    g.define(
        "_plain_destructor_name",
        prec(1, seq![choice!["~", "compl"], s!(identifier)]),
    );
    g.define(
        "destructor_name",
        prec_right(
            1,
            seq![
                choice!["~", "compl"],
                choice![s!(identifier), alias(s!(_destructor_template_type), s!(template_type))]
            ],
        ),
    );
    g.define(
        "_destructor_template_type",
        prec(
            1,
            seq![
                field("name", s!(_type_identifier)),
                field("arguments", s!(template_argument_list)),
            ],
        ),
    );
    g.redefine("compound_literal_expression", |original| {
        choice![
            original,
            seq![
                field(
                    "type",
                    choice![
                        s!(_class_name),
                        s!(primitive_type),
                        s!(splice_type_specifier),
                        s!(dependent_type),
                        s!(decltype),
                        s!(sized_type_specifier),
                        // C++26: `T...[0]{}` (cp_parser_pack_index in GCC).
                        s!(pack_index_specifier),
                        // The C++23 decay-copy `auto{x}` (P0849R8).
                        alias(s!(_decay_copy_type), s!(placeholder_type_specifier)),
                    ]
                ),
                field("value", s!(initializer_list)),
            ],
        ]
    });
    // The `auto` of a decay-copy comes from the external scanner, and only immediately before `(`
    // or `{`. Where an expression starts, `auto` is otherwise not a keyword, and a macro argument
    // such as `MACRO(auto* x, y)` keeps the tree that it had before C++23.
    g.define("_decay_copy_type", seq![alias(s!(_decay_copy_auto), s!(auto))]);
    g.define("dependent_identifier", seq!["template", s!(template_function)]);
    g.define("dependent_field_identifier", seq!["template", s!(template_method)]);
    g.define("dependent_type_identifier", seq!["template", s!(template_type)]);
    g.define(
        "_scope_resolution",
        prec(
            1,
            seq![
                field(
                    "scope",
                    optional(choice![
                        s!(_namespace_identifier),
                        s!(template_type),
                        s!(decltype),
                        s!(pack_index_specifier),
                        s!(splice_expression),
                        s!(splice_type_specifier),
                        alias(s!(dependent_type_identifier), s!(dependent_name)),
                    ])
                ),
                "::",
            ],
        ),
    );
    g.define(
        "qualified_field_identifier",
        seq![
            s!(_scope_resolution),
            field(
                "name",
                choice![
                    alias(s!(dependent_field_identifier), s!(dependent_name)),
                    alias(s!(qualified_field_identifier), s!(qualified_identifier)),
                    s!(template_method),
                    prec_dynamic(2, s!(_field_identifier)),
                    // `x.Base::foo < 0 || y > (5)`. Refer to `_comparison_name`.
                    prec_dynamic(2, alias(s!(_comparison_name), s!(field_identifier))),
                    s!(destructor_name),
                    // `this->B::operator=(x)`. The lower precedence gives the `<` after
                    // `B::operator<` to the template arguments of a `template_method`.
                    prec(-1, s!(operator_name)),
                    alias(s!(_conversion_function_id), s!(operator_cast)),
                ]
            ),
        ],
    );
    g.define(
        "qualified_identifier",
        seq![
            s!(_scope_resolution),
            field(
                "name",
                choice![
                    alias(s!(dependent_identifier), s!(dependent_name)),
                    s!(qualified_identifier),
                    s!(template_function),
                    prec_dynamic(1, seq![optional("template"), s!(identifier)]),
                    prec_dynamic(1, alias(s!(_comparison_name), s!(identifier))),
                    s!(operator_name),
                    // `T::template operator|` (cp_parser_id_expression in GCC).
                    seq!["template", s!(operator_name)],
                    s!(destructor_name),
                    // The parser reads the token `_member_pointer_start` first. This alternative
                    // keeps the node type of the base. It also reads a member pointer that the
                    // scanner does not accept, as in `A<(p->q)>::*m`.
                    s!(pointer_type_declarator),
                ]
            ),
        ],
    );
    g.define(
        "qualified_type_identifier",
        seq![
            s!(_scope_resolution),
            field(
                "name",
                choice![
                    alias(s!(dependent_type_identifier), s!(dependent_name)),
                    alias(s!(qualified_type_identifier), s!(qualified_identifier)),
                    // The two names of a type have the same dynamic precedence. In a block,
                    // `a::B<1> c;` is then a declaration, as [stmt.ambig] says, and not the
                    // comparison `a::B < 1 > c`.
                    prec_dynamic(1, s!(template_type)),
                    prec_dynamic(1, s!(_type_identifier)),
                ]
            ),
        ],
    );
    g.define(
        "qualified_operator_cast_identifier",
        seq![
            s!(_scope_resolution),
            field(
                "name",
                choice![
                    alias(s!(qualified_operator_cast_identifier), s!(qualified_identifier)),
                    s!(operator_cast),
                ]
            ),
        ],
    );
    // A conversion function name after a scope, where the name is not the declarator of a conversion
    // function: `&X::operator bool`, `n::X::operator int()`. A declarator takes
    // `qualified_operator_cast_identifier`, and `qualified_identifier` does not take this name. A
    // constructor declarator `X::operator int()` then does not compete with the conversion function.
    g.define(
        "_qualified_conversion_function_id",
        seq![
            s!(_scope_resolution),
            field(
                "name",
                choice![
                    alias(s!(_qualified_conversion_function_id), s!(qualified_identifier)),
                    alias(s!(_conversion_function_id), s!(operator_cast)),
                ]
            ),
        ],
    );
    // The left operand of an assignment is a logical-or-expression, and the right operand is an
    // initializer-clause ([expr.assign], GCC `cp_parser_assignment_expression`, Clang
    // `ParseAssignmentExpression`). Each operator with a precedence more than ASSIGNMENT gives its
    // full expression to the left operand, as in `++x = 1` and `(T &)x = 1`. `a + b = c` is
    // `(a + b) = c`. An assignment, a throw, a yield, and the third operand of a conditional have the
    // precedence ASSIGNMENT and right associativity. The parser then starts a new assignment in
    // their last operand. The C grammar lists the left operands, and C++ does not use that list.
    g.rules.shift_remove("_assignment_left_expression");
    g.inline.retain(|name| name != "_assignment_left_expression");
    g.define(
        "assignment_expression",
        prec_right(
            ASSIGNMENT,
            seq![
                field("left", s!(expression)),
                field("operator", choice_of(ASSIGNMENT_OPERATORS.map(Rule::from))),
                field("right", choice![s!(expression), s!(initializer_list)]),
            ],
        ),
    );
    // In C++ the third operand of a conditional is an assignment-expression ([expr.cond], GCC
    // `cp_parser_question_colon_clause`, Clang `ParseRHSOfBinaryExpression`). The precedence
    // ASSIGNMENT on that operand gives `a ? b : c = d` the tree `a ? b : (c = d)`. The `?` has the
    // precedence CONDITIONAL, and `a = b ? c : d` is `a = (b ? c : d)`. A C compiler reads a
    // conditional-expression as the third operand, but no valid C program has an assignment there.
    g.define(
        "conditional_expression",
        prec_right(
            CONDITIONAL,
            seq![
                field("condition", s!(expression)),
                "?",
                optional(field("consequence", choice![s!(expression), s!(comma_expression)])),
                ":",
                prec_right(ASSIGNMENT, field("alternative", s!(expression))),
            ],
        ),
    );
    // The operand of `^^` is `::`, a type, or an id-expression (GCC `cp_parser_reflect_expression`,
    // parser.cc:10098). An id-expression includes a conversion function name: `^^S::operator int`.
    //
    // Where the operand reads as a type and as an expression, it is a type. GCC reads a type-id
    // before an id-expression, and it simulates an error on a reflection-name that names a type, so
    // that `^^A &` gives the type `A &` (parser.cc:10116 to :10121). The dynamic precedences are
    // those of a template argument. They give `^^int()` the function type and not a
    // value-initialization, and `^^A[3]` the array type and not a subscript.
    //
    // A plain name after `^^` is a type or a value, and only name lookup separates the two. GCC
    // reads `^^F` as a type-id for a class F, and as a reflection-name for a variable F. Without the
    // dynamic precedences the two readings tie, and the symbol order of the generator picks one.
    g.define(
        "reflect_expression",
        prec_right(
            0,
            seq![
                "^^",
                choice![
                    "::",
                    prec_dynamic(3, s!(type_descriptor)),
                    prec_dynamic(1, s!(expression)),
                ]
            ],
        ),
    );
    // The external scanner gives the `[:` of a splice. [lex.pptoken]p4.3 keeps `[` and `::` apart in
    // `a[::b]`, and only the scanner can read the character after `[::`. Refer to
    // `scan_bracket_start` in `src/scanner.c`.
    g.define(
        "splice_specifier",
        seq![alias(s!(_splice_open), "[:"), s!(expression), ":]"],
    );
    g.define(
        "_splice_specialization_specifier",
        seq![s!(splice_specifier), s!(template_argument_list)],
    );
    // After `^^`, a type can end before a less-than operator. There, `^^[:r:] < 1` is a comparison
    // and `^^typename [:r:]<int>` has template arguments, and the parser keeps the two readings
    // (the splice conflicts in `grammar`). Where only a type can come, the parser reads the
    // template arguments.
    g.define(
        "splice_type_specifier",
        choice![
            s!(splice_specifier),
            prec_right(0, s!(_splice_specialization_specifier)),
        ],
    );
    // `template [:r:](1)` names a function template for overload resolution, and needs no
    // template arguments (cp_parser_splice_expression in GCC). A `<` after a splice starts
    // template arguments only after `template` or `typename` ([temp.names],
    // cp_parser_splice_specifier in GCC). The left associativity gives the `<` of `[:r:] < 42`
    // to a binary expression, and the right associativity gives the `<` of `template [:r:]<1>`
    // to the template arguments.
    g.define(
        "splice_expression",
        choice![
            prec_left(0, s!(splice_specifier)),
            prec_right(0, seq!["template", s!(splice_specifier)]),
            seq!["template", s!(_splice_specialization_specifier)],
        ],
    );
    g.define(
        "expansion_statement",
        seq![
            "template",
            "for",
            "(",
            s!(_for_range_loop_body),
            ")",
            field("body", s!(_substatement)),
        ],
    );
    let mut operators: Vec<Rule> = [
        "co_await", "+", "-", "*", "/", "%", "^", "&", "|", "~", "!", "=", "<", ">", "+=", "-=", "*=", "/=", "%=",
        "^=", "&=", "|=", "<<", ">>", ">>=", "<<=", "==", "!=", "<=", ">=", "<=>", "&&", "||", "++", "--", ",", "->*",
        "->", "()", "[]", "xor", "bitand", "bitor", "compl", "not", "xor_eq", "and_eq", "or_eq", "not_eq", "and", "or",
    ]
    .map(Rule::from)
    .into();
    operators.push(seq![choice!["new", "delete"], optional("[]")]);
    operators.push(seq!["\"\"", s!(identifier)]);
    g.define("operator_name", prec(1, seq!["operator", Rule::Choice(operators)]));
    g.define("this", "this");
    // A concatenation has two or more parts, and one of the first two parts is a string. An
    // identifier is a part for macros that are strings, for example `"%" PRId64`. The
    // precedence gives an identifier after a string to the concatenation. Only macro
    // concatenation puts an identifier immediately after a string.
    // A part can have a ud-suffix: `"a"_s "b"`. The precedence gives the suffix after the last
    // part to the full concatenation: `"a" "b"_s` is a user-defined literal.
    let string = || {
        choice![
            s!(string_literal),
            s!(raw_string_literal),
            alias(s!(_string_user_defined_literal), s!(user_defined_literal)),
        ]
    };
    g.define(
        "concatenated_string",
        prec_right(
            1,
            seq![
                choice![
                    seq![s!(identifier), string()],
                    seq![string(), choice![string(), s!(identifier)]]
                ],
                repeat(prec(1, choice![s!(identifier), string()])),
            ],
        ),
    );
    // C++23 adds delimited escapes: `\x{41}`, `\o{101}`, `\u{1F600}`, and `\N{NAME}`.
    g.define(
        "escape_sequence",
        token(prec(
            1,
            seq![
                "\\",
                choice![
                    re("[^xuU]"),
                    re(r"\d{2,3}"),
                    re("x[0-9a-fA-F]+"),
                    re("u[0-9a-fA-F]{4}"),
                    re("U[0-9a-fA-F]{8}"),
                    re(r"[xou]\{[0-9a-fA-F]+\}"),
                    re(r"N\{[^}\n]+\}"),
                ],
            ],
        )),
    );
    // A compound statement has a lower precedence than a braced list, for `f({})`. A braced list in a
    // braced list has the precedence of a compound statement. The parser then keeps the two readings of
    // the inner `{}` in `f({ {} x; })`, and the tokens after the inner `}` select one. The dynamic
    // precedence gives `f({ {} })` to the braced list, and not to a GNU statement expression.
    //
    // The external scanner never gives `_initializer_list_marker`. The token is valid only after the `{` of a
    // braced list, and the scanner reads its validity. In `f({ FLAG })`, `{` also starts a GNU statement
    // expression, and the macro scan then reads `FLAG` as an element of the list, not as a macro statement.
    g.redefine("initializer_list", |original| {
        let list = replace_rule(
            original,
            &s!(initializer_list),
            &alias(s!(_nested_initializer_list), s!(initializer_list)),
        );
        insert_after(
            list,
            |member| *member == open_brace(),
            optional(s!(_initializer_list_marker)),
        )
    });
    let nested_list = g.rules["initializer_list"].clone();
    g.define("_nested_initializer_list", prec(-1, prec_dynamic(1, nested_list)));
    // C++20 permits a designator with a braced initializer and no `=`: `{.p{1, 2}}`.
    g.define(
        "initializer_pair",
        choice![
            seq![
                field(
                    "designator",
                    repeat1(choice![
                        s!(subscript_designator),
                        s!(field_designator),
                        s!(subscript_range_designator),
                    ])
                ),
                choice![
                    seq!["=", field("value", choice![s!(expression), s!(initializer_list)])],
                    field("value", s!(initializer_list)),
                ],
            ],
            seq![
                field("designator", s!(_field_identifier)),
                ":",
                field("value", choice![s!(expression), s!(initializer_list)]),
            ],
        ],
    );
    // `__has_include(<x>)` and `__has_embed("x")` take a header name. The argument of a macro can
    // be a pp-number that is not a number, for a macro that pastes it: `ENABLE(2022_GLIB_API)`.
    g.define(
        "preproc_argument_list",
        seq![
            "(",
            comma_sep(choice![
                s!(_preproc_expression),
                s!(system_lib_string),
                s!(string_literal),
                alias(s!(_preproc_user_defined_literal), s!(user_defined_literal)),
            ]),
            ")",
        ],
    );
    g.define(
        "_preproc_user_defined_literal",
        seq![s!(number_literal), s!(literal_suffix)],
    );
    // The arguments of a macro in a condition are tokens that the preprocessor replaces before it
    // evaluates the condition (libcpp `collect_args`). An argument can be text that is not an
    // expression, as in `BOOST_WORKAROUND(BOOST_MSVC, >= 1400)` or `__has_cpp_attribute(clang::musttail)`.
    // The dynamic precedence prefers the argument list when the arguments are also expressions.
    g.define(
        "preproc_call_expression",
        prec(
            CALL,
            seq![
                field("function", s!(identifier)),
                field(
                    "arguments",
                    choice![
                        prec_dynamic(1, alias(s!(preproc_argument_list), s!(argument_list))),
                        alias(s!(_preproc_token_tree), s!(token_tree)),
                    ]
                ),
            ],
        ),
    );
    g.define(
        "_preproc_token_tree",
        seq!["(", repeat(choice![s!(_preproc_token_tree), s!(_preproc_token)]), ")"],
    );
    let punctuators = [
        ",", ";", "::", ".", "...", "->", "+", "-", "*", "/", "%", "^", "&", "|", "~", "!", "=", "<", ">", "<=", ">=",
        "==", "!=", "<<", ">>", "&&", "||", "?", ":", "[", "#", "##", "and", "or", "not", "bitand", "bitor", "xor",
        "compl", "not_eq", "defined",
    ];
    let named = [
        "identifier",
        "number_literal",
        "char_literal",
        "string_literal",
        "system_lib_string",
        "primitive_type",
        "true",
        "false",
    ];
    g.define(
        "_preproc_token",
        choice_of(
            named
                .map(sym)
                .into_iter()
                .chain(punctuators.map(Rule::from))
                .chain([close_bracket(), open_brace(), close_brace()]),
        ),
    );
    // A directive reads the alternative tokens as operators: `#if defined(A) and not defined(B)`
    // (libcpp `mark_named_operators`).
    g.redefine("preproc_binary_expression", |original| {
        choice_of(
            [original]
                .into_iter()
                .chain(alternative_binary_operations("_preproc_expression")),
        )
    });
    g.redefine("preproc_unary_expression", |original| {
        choice![
            original,
            prec_left(
                UNARY,
                seq![
                    field("operator", choice!["not", "compl"]),
                    field("argument", s!(_preproc_expression)),
                ]
            ),
        ]
    });
    // A condition can have a conditional operator (libcpp `reduce` for `CPP_QUERY` and `CPP_COLON`).
    g.define(
        "preproc_conditional_expression",
        prec_right(
            CONDITIONAL,
            seq![
                field("condition", s!(_preproc_expression)),
                "?",
                field("consequence", s!(_preproc_expression)),
                ":",
                field("alternative", s!(_preproc_expression)),
            ],
        ),
    );
    g.redefine("_preproc_expression", |original| {
        choice![
            original,
            alias(s!(preproc_conditional_expression), s!(conditional_expression))
        ]
    });
    // GNU C names the variadic parameter of a macro: `#define F(a, args...)`.
    g.define(
        "preproc_params",
        seq![
            token_immediate("("),
            comma_sep(choice![s!(identifier), "...", seq![s!(identifier), "..."]]),
            ")",
        ],
    );
    // GNU C takes the address of a label: `&&label`.
    g.redefine("pointer_expression", |original| {
        choice![
            original,
            prec_left(
                CAST,
                seq![field("operator", "&&"), field("argument", s!(_statement_identifier))]
            ),
        ]
    });
    // The external scanner selects the reading of a name in the parentheses, as for `sizeof`. Refer to
    // `scan_parenthesized_name` in `src/scanner.c`.
    g.define(
        "typeid_expression",
        prec(
            SIZEOF,
            seq![
                "typeid",
                choice![
                    // GCC `cp_parser_postfix_expression` reads a full expression: `typeid(a, *p)`.
                    seq![
                        choice!["(", alias(s!(_name_expression_paren), "(")],
                        field("value", choice![s!(expression), s!(comma_expression)]),
                        ")",
                    ],
                    seq![
                        choice!["(", alias(s!(_operand_type_paren), "(")],
                        field("type", s!(type_descriptor)),
                        ")",
                    ],
                ],
            ],
        ),
    );
    // C++26 pack indexing of an expression: `args...[0]`.
    g.define(
        "pack_index_expression",
        seq![
            field("pack", s!(identifier)),
            pack_index_open(),
            field("index", s!(expression)),
            close_bracket()
        ],
    );
    g.define("number_literal", number_literal());
    // C++23 permits a delimited or a named universal character name in an identifier:
    // `jalape\u{f1}o`, `jalape\N{LATIN SMALL LETTER N WITH TILDE}o`. libcpp reads them in
    // `forms_identifier_p` with `_cpp_valid_ucn`.
    let ucn = r"\\u[0-9A-Fa-f]{4}|\\U[0-9A-Fa-f]{8}|\\u\{[0-9A-Fa-f]+\}|\\N\{[A-Za-z0-9 _-]+\}";
    // Phase 2 of [lex.phases] deletes each line splice before tokenization, so a splice inside an
    // identifier makes one token: `Q\` and `_OBJECT` on two lines are the one word `Q_OBJECT`.
    // libcpp `_cpp_clean_line` (gcc/libcpp/lex.cc:877) deletes the splice before `lex_identifier`
    // reads the word. Clang `Lexer::LexIdentifierContinue` (clang/lib/Lex/Lexer.cpp:2039) reads each
    // character with `getCharAndSize`, which goes past a splice (`getEscapedNewLineSize`, :1336).
    //
    // A run of splices comes before each character after the first one, and no splice ends the
    // token. The lexer gives the longest match, and a splice at the end belongs to the white space
    // between two tokens.
    let splice = c::LINE_SPLICE;
    g.redefine("identifier", |_| {
        re(&format!(
            r"(\p{{XID_Start}}|\$|_|{ucn})(({splice})*(\p{{XID_Continue}}|\$|{ucn}))*"
        ))
    });
    // A ud-suffix is an identifier, as in `LexUDSuffix` of Clang: `1_km`, `1_π`.
    g.define(
        "literal_suffix",
        token_immediate(re(r"[a-zA-Z_](\w|\p{XID_Continue}|\$)*")),
    );
    // A character literal can hold `//` or `/*`: `'//'`. The precedence gives each character to the
    // literal, and a comment does not start in it. A backslash starts only an escape sequence or a
    // line splice, so that `'\''` holds one escape sequence.
    g.define(
        "char_literal",
        seq![
            choice!["L'", "u'", "U'", "u8'", "'"],
            repeat1(choice![
                s!(escape_sequence),
                s!(_line_splice),
                alias(token_immediate(prec(1, re(r"[^\r\n'\\]"))), s!(character)),
            ]),
            "'",
        ],
    );
    // A string with a ud-suffix is one rule, so that the parser needs no second token of lookahead
    // to see whether a string follows: `"a"_s` and `"a"_s "b"`.
    g.define(
        "user_defined_literal",
        choice![
            seq![
                choice![s!(number_literal), s!(char_literal), s!(concatenated_string)],
                s!(literal_suffix),
            ],
            s!(_string_user_defined_literal),
        ],
    );
    g.define(
        "_string_user_defined_literal",
        seq![choice![s!(string_literal), s!(raw_string_literal)], s!(literal_suffix)],
    );
    g.define("_namespace_identifier", alias(s!(identifier), s!(namespace_identifier)));
}

/// The directive rules that are extras. They are not items of the lists of declarations and statements.
const EXTRA_DIRECTIVES: [&str; 4] = ["preproc_call", "preproc_def", "preproc_function_def", "preproc_include"];

/// The rules of a conditional directive. `c::preproc_if` makes one family of them for each suffix.
const CONDITIONAL_RULES: [&str; 5] = [
    "preproc_if",
    "preproc_ifdef",
    "preproc_else",
    "preproc_elif",
    "preproc_elifdef",
];

/// Each extra directive rule, the external token that starts it, and the external token of the scanner for
/// the last directive line of a branch.
const FINAL_DIRECTIVES: [(&str, &str, &str); 4] = [
    ("preproc_call", "_preproc_directive", "_preproc_final_directive"),
    ("preproc_def", "_preproc_define", "_preproc_final_define"),
    ("preproc_function_def", "_preproc_define", "_preproc_final_define"),
    ("preproc_include", "_preproc_include", "_preproc_final_include"),
];

/// The name suffixes of the conditional directive families.
const CONDITIONAL_SUFFIXES: [&str; 5] = [
    "",
    "_in_block",
    "_in_declaration_list",
    "_in_field_declaration_list",
    "_in_enumerator_list",
];

/// The preprocessor directives.
///
/// The external scanner lexes each directive token, because a `#` starts a directive only as the first
/// token of a line (libcpp `_cpp_handle_directive`, Clang `Preprocessor::HandleDirective`). It also
/// lexes the line break at the end of a directive line, and it does not look for a directive on the
/// next line before that line break.
///
/// A directive line can come between two tokens of each construct, as it can for the preprocessor. The
/// directive rules are extras for this reason, and they are not items. The preprocessor removes a
/// directive line before the parser gets the tokens (libcpp `_cpp_lex_token`, Clang
/// `Lexer::LexTokenInternal`). The parse table has no extra action for a token that also has an item
/// action. An item then ends the construct before the line, for example the `if` statement in `}`,
/// `#pragma x`, `else {}`. The runtime puts an extra into the smallest node that has tokens before and
/// after the extra. A directive between two items is then a child of the list.
///
/// An `#elif` or `#else` branch has no token after its last line: the `#endif` is a token of the group.
/// An extra at the end of the branch then goes into the group. For this reason, the scanner gives a
/// different token for a directive when the next line holds the `#elif`, `#else`, or `#endif` of the
/// group. The branch reads that directive line as its last child. The extras before that line are
/// between two children of the branch.
///
/// A conditional group is a structured node, `preproc_if` or `preproc_ifdef`, where the scanner finds
/// that each branch is a sequence of complete declarations or statements. In other groups each
/// directive is a line. `#if`, `#ifdef`, `#ifndef`, `#endif`, and the `#elif` or `#else` of the branch
/// that the parser reads are a `preproc_call`. A `preproc_skipped` holds the text of the other branches.
/// The parser reads the first branch that is not `#if 0` as code, as the clangd `DirectiveTree` branch
/// chooser does. The token of such a directive line is only in the extra. A token of an item would end
/// the construct before the line, for example the `if` statement in `if (x) f();`, `#ifdef A`, `else g();`.
fn preprocessor(g: &mut Grammar) {
    let conditionals = CONDITIONAL_SUFFIXES
        .iter()
        .flat_map(|suffix| CONDITIONAL_RULES.map(|rule| format!("{rule}{suffix}")));
    for name in EXTRA_DIRECTIVES.map(str::to_owned).into_iter().chain(conditionals) {
        g.redefine(&name, scanner_directive_tokens);
    }
    g.rules.shift_remove("preproc_directive");
    // The tokens after an `#endif` and after an `#else` are extra tokens of the directive line. The
    // preprocessor gives a warning and removes them: libcpp `check_eol_endif_labels`
    // (gcc/libcpp/directives.cc:250) from `do_else` (:2648) and `do_endif` (:2791), and Clang
    // `Preprocessor::CheckEndOfDirective` (clang/lib/Lex/PPDirectives.cpp:465) from
    // `HandleElseDirective` (:3661) and `HandleEndifDirective` (:3635). The two front ends compile
    // such a line with a warning only. Without this rule `#endif USE_DML` gave a `macro_invocation`
    // for a token that no longer exists after the preprocessor, with no ERROR node.
    //
    // The mark of the scanner comes before the text. It is empty, and the scanner gives it only when
    // the rest of the line holds a token. A line with no such token then takes no mark and no line
    // end, and the node of the branch or of the group keeps the range that it had. A line end with
    // no mark would extend `preproc_else`, `preproc_elif` and the group over the line break, and 235
    // corpus files would get a different range to repair 13 files.
    let extra_tokens = || {
        optional(seq![
            s!(_preproc_extra_mark),
            repeat1(s!(preproc_arg)),
            s!(_preproc_line_end),
        ])
    };
    for suffix in CONDITIONAL_SUFFIXES {
        for (rule, token, name) in [
            ("preproc_if", "_preproc_endif", "#endif"),
            ("preproc_ifdef", "_preproc_endif", "#endif"),
            ("preproc_else", "_preproc_else", "#else"),
        ] {
            let directive = alias(sym(token), name);
            let replacement = seq![alias(sym(token), name), extra_tokens()];
            g.redefine(&format!("{rule}{suffix}"), |original| {
                replace_rule(original, &directive, &replacement)
            });
        }
    }
    // The external scanner gives the text of a directive line (`scan_preproc_arg` in src/scanner.c). A
    // pattern of the lexer reads no literal, so a `//` or a `/*` in the content of a string literal
    // started a comment, a raw string literal ended at the line break, and a `/` at the end of a line
    // took the text to the next line.
    //
    // The pattern stays as the token of error recovery, where the scanner gives no token. Its first
    // character is not a `/`, so that a comment after the name of a directive stays a comment: the
    // scanner gives no token there, and the lexer reads the comment.
    g.redefine("preproc_arg", |_| {
        let splice = c::LINE_SPLICE;
        token(prec(-1, re(&format!(r"[^\s/]([^/\r\n]|\/[^*\r\n]|{splice})*"))))
    });
    // The `(` of a parameter list comes immediately after the name of the macro, and the text of the
    // line starts after the list. The mark before that `(` tells the scanner that a parameter list can
    // start here. The scanner gives no mark, and the rule has the mark as an option for this reason.
    g.redefine("preproc_params", |original| {
        replace_rule(
            original,
            &token_immediate("("),
            &seq![optional(s!(_preproc_params_mark)), token_immediate("(")],
        )
    });
    // A comment can come between two parts of the text of a directive: `#define X a /* c */ b`. The
    // preprocessor reads such a comment as a space.
    for rule in ["preproc_def", "preproc_function_def", "preproc_call"] {
        g.redefine(rule, |original| {
            map_children(original, |member| match member {
                Rule::Field { name, content }
                    if (name == "value" || name == "argument") && *content == optional(s!(preproc_arg)) =>
                {
                    field(&name, optional(repeat1(s!(preproc_arg))))
                }
                other => other,
            })
        });
    }
    let mut final_lines = Vec::new();
    for (rule, token, final_token) in FINAL_DIRECTIVES {
        let name = format!("_final_{rule}");
        let copy = replace_rule(g.rules[rule].clone(), &sym(token), &sym(final_token));
        g.define(&name, copy);
        final_lines.push(alias(sym(&name), sym(rule)));
    }
    g.define("_preproc_final_line", Rule::Choice(final_lines));
    // The final line is the last child of the content of a branch: before the `alternative` field of an
    // `#elif` branch, and at the end of an `#else` branch.
    for suffix in CONDITIONAL_SUFFIXES {
        for rule in ["preproc_else", "preproc_elif", "preproc_elifdef"] {
            g.redefine(&format!("{rule}{suffix}"), |original| {
                edit_sequence(original, |mut members| {
                    let index = members
                        .iter()
                        .position(|member| matches!(member, Rule::Field { name, .. } if name == "alternative"))
                        .unwrap_or(members.len());
                    members.insert(index, optional(s!(_preproc_final_line)));
                    members
                })
            });
        }
    }
    // A group in a case body starts with the token of the scanner for a group with no case label at its
    // top level. The branches after the first branch are the branches of a group in a block.
    let case_group_tokens: [(&str, &[(&str, &str)]); 2] = [
        ("preproc_if", &[("_preproc_if", "_preproc_if_in_case")]),
        (
            "preproc_ifdef",
            &[
                ("_preproc_ifdef", "_preproc_ifdef_in_case"),
                ("_preproc_ifndef", "_preproc_ifndef_in_case"),
            ],
        ),
    ];
    for (name, tokens) in case_group_tokens {
        let block_group = g.rules[&format!("{name}_in_block")].clone();
        let case_group = tokens.iter().fold(block_group, |rule, (token, case_token)| {
            replace_rule(rule, &sym(token), &sym(case_token))
        });
        g.define(&format!("{name}_in_case"), case_group);
    }
    for name in [
        "_top_level_item",
        "_block_item",
        "_declaration_list_item",
        "_export_item",
        "_case_body_item",
        "_labeled_item",
        "_field_declaration_list_item",
        "enumerator_list",
    ] {
        g.redefine(name, without_directive_items);
    }
    // A branch of a group in an enumerator list holds the items of the list: enumerators with a comma, macro
    // invocations, and groups. The scanner reads a group in an enumerator list by the validity of the macro
    // enumerator token, and a group in a branch is then also a structured node. The last enumerator of a
    // branch can have no comma.
    let Rule::Seq(list_members) = &g.rules["enumerator_list"] else {
        panic!("`enumerator_list` is a sequence");
    };
    let list_items = list_members[1].clone();
    let branch_items = repeat(seq![s!(enumerator), ","]);
    for rule in CONDITIONAL_RULES {
        g.redefine(&format!("{rule}_in_enumerator_list"), |original| {
            replace_rule(original, &branch_items, &list_items)
        });
    }
    g.redefine("preproc_call", |original| {
        map_children(original, |member| match member {
            Rule::Field { name, content } if name == "directive" => field(
                "directive",
                choice![*content, alias(s!(_preproc_conditional), s!(preproc_directive))],
            ),
            other => other,
        })
    });
    g.extras.extend(EXTRA_DIRECTIVES.map(sym));
    g.extras.push(s!(preproc_skipped));
    // A directive cannot start in a string literal or a character literal, also on a line that a line
    // splice continues. The scanner finds these parse states by the validity of a token that it never gives.
    for name in ["string_literal", "char_literal"] {
        g.redefine(name, with_literal_marker);
    }
    // `#embed` gives a list of integer constants. Where an expression can start, and a declaration or a
    // statement cannot, the directive is an expression, as Clang `EmbedExpr` (`Parser::ParseCastExpression`).
    // Elsewhere it is a `preproc_call`, as in the C grammar.
    g.define(
        "preproc_embed",
        seq![
            alias(s!(_preproc_embed), "#embed"),
            field(
                "path",
                choice![s!(string_literal), s!(system_lib_string), s!(identifier)]
            ),
            field("parameters", optional(repeat1(s!(preproc_arg)))),
            s!(_preproc_line_end),
        ],
    );
    g.redefine("_expression_not_binary", |original| {
        choice![original, s!(preproc_embed)]
    });
}

/// The rule with the literal marker of the scanner as one more member of the choice in its repetition.
fn with_literal_marker(rule: Rule) -> Rule {
    match rule {
        Rule::Repeat(content) => repeat(with_literal_marker_member(*content)),
        Rule::Repeat1(content) => repeat1(with_literal_marker_member(*content)),
        other => map_children(other, with_literal_marker),
    }
}

/// The choice with the literal marker of the scanner as one more member.
fn with_literal_marker_member(rule: Rule) -> Rule {
    let mut members = rule.into_members();
    members.push(s!(_preproc_literal_marker));
    Rule::Choice(members)
}

/// The rule without the members of its choices that are an extra directive rule, alone or before `,`.
/// The C grammar has these members in the item lists: `preproc_call` in a block, `seq(preproc_call, ',')`
/// in an enumerator list. O(n) in the size of the rule.
fn without_directive_items(rule: Rule) -> Rule {
    fn is_directive(member: &Rule) -> bool {
        matches!(member, Rule::Symbol(name) if EXTRA_DIRECTIVES.contains(&name.as_str()))
    }
    fn is_directive_item(member: &Rule) -> bool {
        match member {
            Rule::Seq(parts) => {
                matches!(parts.as_slice(), [first, comma] if is_directive(first) && *comma == Rule::from(","))
            }
            other => is_directive(other),
        }
    }
    match rule {
        Rule::Choice(members) => Rule::Choice(
            members
                .into_iter()
                .filter(|member| !is_directive_item(member))
                .map(without_directive_items)
                .collect(),
        ),
        other => map_children(other, without_directive_items),
    }
}

/// The rule with `transform` applied to each direct member or content of a composite rule.
fn map_children(rule: Rule, mut transform: impl FnMut(Rule) -> Rule) -> Rule {
    let mut boxed = |content: Box<Rule>| Box::new(transform(*content));
    match rule {
        Rule::Seq(members) => Rule::Seq(members.into_iter().map(|m| *boxed(Box::new(m))).collect()),
        Rule::Choice(members) => Rule::Choice(members.into_iter().map(|m| *boxed(Box::new(m))).collect()),
        Rule::Repeat(content) => Rule::Repeat(boxed(content)),
        Rule::Repeat1(content) => Rule::Repeat1(boxed(content)),
        Rule::Field { name, content } => Rule::Field {
            name,
            content: boxed(content),
        },
        Rule::Alias { content, named, value } => Rule::Alias {
            content: boxed(content),
            named,
            value,
        },
        Rule::Prec { kind, value, content } => Rule::Prec {
            kind,
            value,
            content: boxed(content),
        },
        Rule::Token { immediate, content } => Rule::Token {
            immediate,
            content: boxed(content),
        },
        Rule::Reserved { context, content } => Rule::Reserved {
            context,
            content: boxed(content),
        },
        leaf => leaf,
    }
}

/// The rule with each directive token of the C grammar replaced by the external token of the scanner.
///
/// The end of a directive line is also an external token, which accepts the end of the file too. The
/// aliases keep the node names of the C grammar: `#if`, `#define`, `\n`, `preproc_directive`, and the
/// other directive names.
fn scanner_directive_tokens(rule: Rule) -> Rule {
    let replacement = match &rule {
        Rule::Alias {
            content,
            named: false,
            value,
        } if **content == re(&format!("#[ \t]*{}", value.trim_start_matches('#'))) => Some(alias(
            sym(&format!("_preproc_{}", value.trim_start_matches('#'))),
            value.as_str(),
        )),
        Rule::Token {
            immediate: true,
            content,
        } if **content == re(r"\r?\n") => Some(s!(_preproc_line_end)),
        Rule::String(text) if text == "\n" => Some(alias(s!(_preproc_line_end), "\n")),
        Rule::Symbol(name) if name == "preproc_directive" => Some(alias(s!(_preproc_directive), s!(preproc_directive))),
        _ => None,
    };
    replacement.unwrap_or_else(|| map_children(rule, scanner_directive_tokens))
}

/// The number literal of C++: digit separators, binary, octal, hexadecimal floats, and suffixes.
fn number_literal() -> Rule {
    let sign = || re(r"[-\+]");
    let separated = |digit: &str| repeat(seq!["'", repeat1(re(digit))]);
    let binary_digits = || seq![repeat1(re("[01]")), separated("[01]")];
    let int_decimal_digits = || seq![re("[1-9]"), repeat(re("[0-9]")), separated("[0-9]")];
    let float_decimal_digits = || seq![repeat1(re("[0-9]")), separated("[0-9]")];
    let hex_digits = || seq![repeat1(re("[0-9a-fA-F]")), separated("[0-9a-fA-F]")];
    let octal_digits = || seq!["0", repeat(re("[0-7]")), separated("[0-7]")];
    let hex_exponent = || seq![re("[pP]"), optional(sign()), float_decimal_digits()];
    let decimal_exponent = || seq![re("[eE]"), optional(sign()), float_decimal_digits()];
    let hex_prefix = || choice!["0x", "0X"];
    // The vendor suffixes are parts of a number, not ud-suffixes: `1i64` and `1ui64` of Microsoft,
    // `1.0q` and `1.0w` of GCC. `NumericLiteralParser` of Clang and `cpp_classify_number` of libcpp
    // read them.
    let int_suffix = re("(ll|LL)[uU]?|[uU](ll|LL)?|[uU][lL]?|[uU][zZ]?|[lL][uU]?|[zZ][uU]?|[uU]?[iI](8|16|32|64)");
    let float_suffix = re("([fF](16|32|64|128)?)|[lL]|(bf16|BF16)|[qQwW]");
    token(seq![
        optional(sign()),
        choice![
            seq![
                choice![
                    seq![choice!["0b", "0B"], binary_digits()],
                    int_decimal_digits(),
                    seq![hex_prefix(), hex_digits()],
                    octal_digits(),
                ],
                optional(int_suffix),
            ],
            seq![
                choice![
                    seq![float_decimal_digits(), decimal_exponent()],
                    seq![
                        float_decimal_digits(),
                        ".",
                        optional(float_decimal_digits()),
                        optional(decimal_exponent())
                    ],
                    seq![".", float_decimal_digits(), optional(decimal_exponent())],
                    seq![
                        hex_prefix(),
                        choice![
                            hex_digits(),
                            seq![hex_digits(), ".", optional(hex_digits())],
                            seq![".", hex_digits()]
                        ],
                        hex_exponent(),
                    ],
                ],
                optional(float_suffix),
            ],
        ],
    ])
}
