//! The C grammar: a port of `grammar.js` of tree-sitter-c 0.24.1.
//!
//! The rules come in the order of `grammar.js`. This order is the order of the rules in
//! `grammar.json`, and the C++ grammar keeps it.

use crate::dsl::*;
use crate::{choice, s, seq};

/// The precedences of the C grammar.
pub mod precedence {
    pub const PAREN_DECLARATOR: i32 = -10;
    pub const ASSIGNMENT: i32 = -2;
    pub const CONDITIONAL: i32 = -1;
    pub const DEFAULT: i32 = 0;
    pub const LOGICAL_OR: i32 = 1;
    pub const LOGICAL_AND: i32 = 2;
    pub const INCLUSIVE_OR: i32 = 3;
    pub const EXCLUSIVE_OR: i32 = 4;
    pub const BITWISE_AND: i32 = 5;
    pub const EQUAL: i32 = 6;
    pub const RELATIONAL: i32 = 7;
    pub const OFFSETOF: i32 = 8;
    pub const SHIFT: i32 = 9;
    pub const ADD: i32 = 10;
    pub const MULTIPLY: i32 = 11;
    // The C++ grammar puts the pointer-to-member operators at 12, between MULTIPLY and CAST.
    pub const CAST: i32 = 13;
    pub const SIZEOF: i32 = 14;
    pub const UNARY: i32 = 15;
    pub const CALL: i32 = 16;
    pub const FIELD: i32 = 17;
    pub const SUBSCRIPT: i32 = 18;
}

use precedence::*;

/// The binary operators of C and of the preprocessor, with their precedences.
const BINARY_OPERATORS: [(&str, i32); 18] = [
    ("+", ADD),
    ("-", ADD),
    ("*", MULTIPLY),
    ("/", MULTIPLY),
    ("%", MULTIPLY),
    ("||", LOGICAL_OR),
    ("&&", LOGICAL_AND),
    ("|", INCLUSIVE_OR),
    ("^", EXCLUSIVE_OR),
    ("&", BITWISE_AND),
    ("==", EQUAL),
    ("!=", EQUAL),
    (">", RELATIONAL),
    (">=", RELATIONAL),
    ("<=", RELATIONAL),
    ("<", RELATIONAL),
    ("<<", SHIFT),
    (">>", SHIFT),
];

/// The pattern of a line splice: a backslash, white space that does not end a line, and a line break.
///
/// Phase 2 of [lex.phases] deletes each line splice before tokenization. The line break is a line feed, a carriage
/// return, or a carriage return and a line feed. In C++23, white space can come between the backslash and the line
/// break (P2223R2). GCC and Clang also accept this white space in C, with a warning (libcpp `_cpp_clean_line`, Clang
/// `Lexer::getEscapedNewLineSize`). The external scanner reads the same text (`skip_backslash` in src/scanner.c).
///
/// A line feed and a carriage return after it are two line breaks, as libcpp `_cpp_clean_line` reads them. Clang
/// `Lexer::getEscapedNewLineSize` reads that order as one line break, and the two front ends disagree here.
pub const LINE_SPLICE: &str = r"\\[ \t\f\v]*(\r\n?|\n)";

/// The `{` of the grammar, with its alternative spelling `<%`.
///
/// [lex.digraph]: `<%`, `%>`, `<:`, and `:>` are the tokens `{`, `}`, `[`, and `]`. The lexer reads the
/// two spellings of one token, and the node keeps the name of the token. GCC reads the digraphs in
/// `_cpp_lex_direct` (libcpp/lex.cc), and Clang in `LexTokenInternal` (clang/lib/Lex/Lexer.cpp).
///
/// The digraph `<:` has an exception that needs the character after `<::`, and the external scanner
/// reads that digraph. Refer to `scan_less_than_digraph` in `src/scanner.c`.
pub fn open_brace() -> Rule {
    alias(token(choice!["{", "<%"]), "{")
}

/// The `}` of the grammar, with its alternative spelling `%>`. Refer to `open_brace`.
pub fn close_brace() -> Rule {
    alias(token(choice!["}", "%>"]), "}")
}

/// The `]` of the grammar, with its alternative spelling `:>`. Refer to `open_brace`.
pub fn close_bracket() -> Rule {
    alias(token(choice!["]", ":>"]), "]")
}

/// The C grammar.
pub fn grammar() -> Grammar {
    let mut g = Grammar::new("c");
    g.conflicts = conflict_sets(&[
        &["type_specifier", "_declarator"],
        &["type_specifier", "_declarator", "macro_type_specifier"],
        &["type_specifier", "expression"],
        &["type_specifier", "expression", "macro_type_specifier"],
        &["type_specifier", "macro_type_specifier"],
        &["type_specifier", "sized_type_specifier"],
        &["sized_type_specifier"],
        &["attributed_statement"],
        &["_declaration_modifiers", "attributed_statement"],
        &["enum_specifier"],
        &["type_specifier", "_old_style_parameter_list"],
        &["parameter_list", "_old_style_parameter_list"],
        &["function_declarator", "_function_declaration_declarator"],
        &["_block_item", "statement"],
        &["_top_level_item", "_top_level_statement"],
        &["type_specifier", "_top_level_expression_statement"],
        &["_extension_specifier", "extension_expression"],
    ]);
    // A null character in the text is white space between two tokens, and the front ends give a warning for it
    // (libcpp `skip_whitespace`, Clang `Lexer::LexTokenInternal` at the case 0). The end of the file is not such a
    // character: the lexer reads the end of the file before the character sets of the extras.
    g.extras = vec![re(r"\s|\x00|\\(\r\n?|\n)"), s!(comment), s!(_spaced_line_splice)];
    g.inline = names(&[
        "_type_identifier",
        "_field_identifier",
        "_statement_identifier",
        "_non_case_statement",
        "_assignment_left_expression",
        "_expression_not_binary",
    ]);
    g.supertypes = names(&[
        "expression",
        "statement",
        "type_specifier",
        "_declarator",
        "_field_declarator",
        "_type_declarator",
        "_abstract_declarator",
    ]);
    g.word = Some("identifier".to_owned());
    top_level(&mut g);
    preprocessor_rules(&mut g);
    declarations(&mut g);
    statements(&mut g);
    expressions(&mut g);
    literals(&mut g);
    names_and_comments(&mut g);
    g
}

/// The GNU keyword `__extension__` before a declaration, as a series of any length.
///
/// The keyword stops the pedantic diagnostics for the declaration that follows it. The front ends
/// read the keyword and then read the declaration again, and the recursion accepts a series: GCC
/// `cp_parser_declaration` (parser.cc:17496), `cp_parser_block_declaration` (parser.cc:17829),
/// `cp_parser_member_declaration` (parser.cc:30932), `c_parser_external_declaration`
/// (c-parser.cc:2183), and `c_parser_struct_declaration` (c-parser.cc:4607). Clang does the same in
/// `ParseExternalDeclaration` (Parser.cpp:826), `ParseCXXClassMemberDeclaration`
/// (ParseDeclCXX.cpp:2814), and `ParseStructDeclaration` (ParseDecl.cpp:4766). The C loops at
/// c-parser.cc:7864 and c-parser.cc:9275 and the Clang loops at ParseStmt.cpp:1209 and
/// ParseExprCXX.cpp:1895 read a series before a statement and before a condition.
///
/// The keyword is not a decl-specifier, and it comes before each other specifier. GCC and Clang
/// give a diagnostic for `int __extension__ x;`, `const __extension__ int x = 1;`, and
/// `void p(__extension__ int x);`. `_extension_specifier` is the symbol that the parser reduces at
/// the step where an expression with the same keyword is also possible. Refer to
/// `extension_expression`.
pub fn extension_prefix() -> Rule {
    repeat(s!(_extension_specifier))
}

/// The GNU asm label of a declarator, with the attributes that can follow it.
///
/// `int a asm("sym") = 1;` gives the symbol `sym` to `a`, and
/// `register unsigned long r asm("rdi") = p;` puts a local variable in a register. The label comes
/// after the declarator and before the initializer: GCC `cp_parser_init_declarator`
/// (parser.cc:25996) reads the asm-specification, then the attributes, then the initializer, and the
/// C front end does the same in `c_parser_declaration_or_fndef` (c-parser.cc:2894). Clang
/// `ParseAsmAttributesAfterDeclarator` (ParseDecl.cpp:2470) reads the label and then the GNU
/// attributes, and `ParseDeclarationAfterDeclarator` (ParseDecl.cpp:2488) then reads the initializer.
///
/// A declarator with no initializer takes the label in `declaration`. Refer to `gnu_asm_expression`.
pub fn asm_label() -> Rule {
    optional(seq![s!(gnu_asm_expression), repeat(s!(attribute_specifier))])
}

/// A directive token: `#` and the command, with optional spaces and tabs between them.
pub fn preprocessor(command: &str) -> Rule {
    alias(re(&format!("#[ \t]*{command}")), format!("#{command}"))
}

/// The conditional directive rules for one kind of content.
///
/// The rules are `preproc_if`, `preproc_ifdef`, `preproc_else`, `preproc_elif`, and
/// `preproc_elifdef`, each with the name suffix. A rule with a suffix names its
/// alternatives with an alias to the rule without the suffix. `branch` gives the content of
/// one branch, for example `repeat(_block_item)`.
pub fn preproc_if(g: &mut Grammar, suffix: &str, branch: impl Fn() -> Rule, precedence: i32) {
    // The tokens after the name of an `#ifdef`, an `#ifndef`, an `#elifdef`, or an `#elifndef` are
    // extra tokens of the directive line. The preprocessor reads the name, gives a warning, and
    // removes the rest of the line: libcpp `check_eol` (gcc/libcpp/directives.cc:259) from
    // `do_ifdef`, `do_ifndef` and `do_elif`, and Clang `Preprocessor::CheckEndOfDirective`
    // (clang/lib/Lex/PPDirectives.cpp:465). The two front ends compile such a line, so the tokens
    // are no error. They have no meaning, and the node of the text of a directive line holds them.
    //
    // The token of the line end comes after them, as it comes after the value of a `#define`. Only
    // that token makes the tokens of the branch invalid in this position. The text of `preproc_arg`
    // has a lower precedence than each other token, and the lexer gives a name or a keyword where
    // the two are valid. The end of the file also ends the line.
    //
    // An `#else` and an `#endif` keep this defect. The line end after them extends the node of the
    // branch or of the group over the line break, and 235 corpus files then get a different range.
    let extra_tokens = || seq![optional(repeat1(s!(preproc_arg))), token_immediate(re(r"\r?\n"))];
    let alternative = || {
        let rule = |name: &str| {
            if suffix.is_empty() {
                sym(name)
            } else {
                alias(sym(&format!("{name}{suffix}")), sym(name))
            }
        };
        optional(choice![
            rule("preproc_else"),
            rule("preproc_elif"),
            rule("preproc_elifdef")
        ])
    };
    g.define(
        &format!("preproc_if{suffix}"),
        prec(
            precedence,
            seq![
                preprocessor("if"),
                field("condition", s!(_preproc_expression)),
                "\n",
                branch(),
                field("alternative", alternative()),
                preprocessor("endif"),
            ],
        ),
    );
    g.define(
        &format!("preproc_ifdef{suffix}"),
        prec(
            precedence,
            seq![
                choice![preprocessor("ifdef"), preprocessor("ifndef")],
                field("name", s!(identifier)),
                extra_tokens(),
                branch(),
                field("alternative", alternative()),
                preprocessor("endif"),
            ],
        ),
    );
    g.define(
        &format!("preproc_else{suffix}"),
        prec(precedence, seq![preprocessor("else"), branch()]),
    );
    g.define(
        &format!("preproc_elif{suffix}"),
        prec(
            precedence,
            seq![
                preprocessor("elif"),
                field("condition", s!(_preproc_expression)),
                "\n",
                branch(),
                field("alternative", alternative()),
            ],
        ),
    );
    g.define(
        &format!("preproc_elifdef{suffix}"),
        prec(
            precedence,
            seq![
                choice![preprocessor("elifdef"), preprocessor("elifndef")],
                field("name", s!(identifier)),
                extra_tokens(),
                branch(),
                field("alternative", alternative()),
            ],
        ),
    );
}

/// The size keywords of `sized_type_specifier`.
fn size_keyword() -> Rule {
    choice!["signed", "unsigned", "long", "short"]
}

/// A choice of one binary operation for each operator, with the operand rule on each side.
fn binary_operations(operand: &str) -> Rule {
    choice_of(BINARY_OPERATORS.map(|(operator, precedence)| {
        prec_left(
            precedence,
            seq![
                field("left", sym(operand)),
                field("operator", operator),
                field("right", sym(operand))
            ],
        )
    }))
}

fn top_level(g: &mut Grammar) {
    g.define("translation_unit", repeat(s!(_top_level_item)));
    // The top level items are the block items, except the expression statement.
    g.define(
        "_top_level_item",
        choice![
            s!(function_definition),
            alias(s!(_old_style_function_definition), s!(function_definition)),
            s!(linkage_specification),
            s!(declaration),
            s!(_top_level_statement),
            s!(attributed_statement),
            s!(type_definition),
            s!(_empty_declaration),
            s!(preproc_if),
            s!(preproc_ifdef),
            s!(preproc_include),
            s!(preproc_def),
            s!(preproc_function_def),
            s!(preproc_call),
        ],
    );
    g.define(
        "_block_item",
        choice![
            s!(function_definition),
            alias(s!(_old_style_function_definition), s!(function_definition)),
            s!(linkage_specification),
            s!(declaration),
            s!(statement),
            s!(attributed_statement),
            s!(type_definition),
            s!(_empty_declaration),
            s!(preproc_if),
            s!(preproc_ifdef),
            s!(preproc_include),
            s!(preproc_def),
            s!(preproc_function_def),
            s!(preproc_call),
        ],
    );
}

fn preprocessor_rules(g: &mut Grammar) {
    let line_end = || token_immediate(re(r"\r?\n"));
    g.define(
        "preproc_include",
        seq![
            preprocessor("include"),
            field(
                "path",
                choice![
                    s!(string_literal),
                    s!(system_lib_string),
                    s!(identifier),
                    alias(s!(preproc_call_expression), s!(call_expression)),
                ]
            ),
            line_end(),
        ],
    );
    g.define(
        "preproc_def",
        seq![
            preprocessor("define"),
            field("name", s!(identifier)),
            field("value", optional(s!(preproc_arg))),
            line_end(),
        ],
    );
    g.define(
        "preproc_function_def",
        seq![
            preprocessor("define"),
            field("name", s!(identifier)),
            field("parameters", s!(preproc_params)),
            field("value", optional(s!(preproc_arg))),
            line_end(),
        ],
    );
    g.define(
        "preproc_params",
        seq![token_immediate("("), comma_sep(choice![s!(identifier), "..."]), ")"],
    );
    g.define(
        "preproc_call",
        seq![
            field("directive", s!(preproc_directive)),
            field("argument", optional(s!(preproc_arg))),
            line_end(),
        ],
    );
    preproc_if(g, "", || repeat(s!(_block_item)), 0);
    preproc_if(g, "_in_field_declaration_list", || repeat(s!(_field_declaration_list_item)), 0);
    // The last enumerator of a branch can have no comma, as the last enumerator of the list can
    // (GCC `cp_parser_enumerator_list`, Clang `ParseEnumBody`).
    preproc_if(
        g,
        "_in_enumerator_list",
        || seq![repeat(seq![s!(enumerator), ","]), optional(s!(enumerator))],
        0,
    );
    // A carriage return ends a directive line, with or without a line feed after it. Phase 1 of [lex.phases] gives
    // each line break the same form (libcpp `_cpp_clean_line`, Clang `Lexer::LexTokenInternal` at the case of the
    // carriage return).
    //
    // A slash and the character after it are one member of the repetition, so that the token ends before the `/*` of
    // a block comment. A line feed is still such a character, and the token of `#define A 1/` then holds the next
    // line. A repair of that defect needs a rule for a macro before a declaration with no declarator, because
    // `_VARIANT_BOOL bool;` comes after `#define _VARIANT_BOOL /##/` in the tests of boost wave.
    g.define(
        "preproc_arg",
        token(prec(-1, re(&format!(r"\S([^/\r\n]|\/[^*\r]|{LINE_SPLICE})*")))),
    );
    g.define("preproc_directive", re(r"#[ \t]*[a-zA-Z0-9]\w*"));
    g.define(
        "_preproc_expression",
        choice![
            s!(identifier),
            alias(s!(preproc_call_expression), s!(call_expression)),
            s!(number_literal),
            s!(char_literal),
            s!(preproc_defined),
            alias(s!(preproc_unary_expression), s!(unary_expression)),
            alias(s!(preproc_binary_expression), s!(binary_expression)),
            alias(s!(preproc_parenthesized_expression), s!(parenthesized_expression)),
        ],
    );
    g.define(
        "preproc_parenthesized_expression",
        seq!["(", s!(_preproc_expression), ")"],
    );
    g.define(
        "preproc_defined",
        choice![
            prec(CALL, seq!["defined", "(", s!(identifier), ")"]),
            seq!["defined", s!(identifier)],
        ],
    );
    g.define(
        "preproc_unary_expression",
        prec_left(
            UNARY,
            seq![
                field("operator", choice!["!", "~", "-", "+"]),
                field("argument", s!(_preproc_expression)),
            ],
        ),
    );
    g.define(
        "preproc_call_expression",
        prec(
            CALL,
            seq![
                field("function", s!(identifier)),
                field("arguments", alias(s!(preproc_argument_list), s!(argument_list))),
            ],
        ),
    );
    g.define(
        "preproc_argument_list",
        seq!["(", comma_sep(s!(_preproc_expression)), ")"],
    );
    g.define("preproc_binary_expression", binary_operations("_preproc_expression"));
}

fn declarations(g: &mut Grammar) {
    g.define(
        "function_definition",
        seq![
            optional(s!(ms_call_modifier)),
            s!(_declaration_specifiers),
            optional(s!(ms_call_modifier)),
            field("declarator", s!(_declarator)),
            field("body", s!(compound_statement)),
        ],
    );
    g.define(
        "_old_style_function_definition",
        seq![
            optional(s!(ms_call_modifier)),
            s!(_declaration_specifiers),
            field(
                "declarator",
                alias(s!(_old_style_function_declarator), s!(function_declarator))
            ),
            repeat1(s!(declaration)),
            field("body", s!(compound_statement)),
        ],
    );
    g.define(
        "declaration",
        seq![
            s!(_declaration_specifiers),
            comma_sep1(field(
                "declarator",
                choice![
                    seq![
                        optional(s!(ms_call_modifier)),
                        s!(_declaration_declarator),
                        optional(s!(gnu_asm_expression))
                    ],
                    s!(init_declarator),
                ]
            )),
            ";",
        ],
    );
    // `typedef` is a specifier of the declaration, and the specifiers have no fixed order. The C front
    // end reads it in `c_parser_declspecs` (gcc/c/c-parser.cc), which takes `RID_TYPEDEF` among the
    // storage class specifiers, and Clang reads `tok::kw_typedef` in `ParseDeclarationSpecifiers`
    // (clang/lib/Parse/ParseDecl.cpp) in the same group. An attribute and a qualifier can come before
    // the keyword: `MBEDTLS_DEPRECATED typedef int t;` (dolphin/Externals/mbedtls, after the macro
    // expands to `__attribute__((deprecated))`) and `const typedef int T;`.
    //
    // The prefix is the prefix of a declaration, so that the two rules share it. The parser reads the
    // specifiers with no fork, and the `typedef` token then selects this rule.
    //
    // WITHOUT THE PREFIX THE TREE HELD NO ERROR NODE AND WAS WRONG. The state after an attribute had
    // no action for `typedef`, the lexer gives a keyword as an identifier in such a state, and the
    // rule of an attribute macro then read the keyword. `[[deprecated]] typedef int T;` gave a
    // `declaration` of a VARIABLE named `T`, with `typedef` as an `attribute_macro` and with the
    // declarator as an `identifier` and not a `type_identifier`.
    g.define(
        "type_definition",
        seq![
            extension_prefix(),
            repeat(s!(_declaration_modifiers)),
            "typedef",
            s!(_type_definition_type),
            s!(_type_definition_declarators),
            repeat(s!(attribute_specifier)),
            ";",
        ],
    );
    // `typedef int __w64 my_int;` still gives an ERROR node. The C++ layer rebuilds this rule with
    // `qualifiers_with_attributes`, which matches on the shape of the trailing repeat, so a choice
    // here loses the attributes and the declspec of a typedef. No corpus file writes `__w64` outside
    // a string or a comment, so the form waits for a repair in `cpp.rs`.
    g.define(
        "_type_definition_type",
        seq![
            repeat(s!(type_qualifier)),
            field("type", s!(type_specifier)),
            repeat(s!(type_qualifier)),
        ],
    );
    g.define(
        "_type_definition_declarators",
        comma_sep1(field("declarator", s!(_type_declarator))),
    );
    g.define(
        "_declaration_modifiers",
        choice![
            s!(storage_class_specifier),
            s!(type_qualifier),
            s!(attribute_specifier),
            s!(attribute_declaration),
            s!(ms_declspec_modifier),
            s!(ms_w64_modifier),
        ],
    );
    g.define(
        "_declaration_specifiers",
        prec_right(
            0,
            seq![
                extension_prefix(),
                repeat(s!(_declaration_modifiers)),
                field("type", s!(type_specifier)),
                repeat(s!(_declaration_modifiers)),
            ],
        ),
    );
    g.define(
        "linkage_specification",
        seq![
            "extern",
            field("value", s!(string_literal)),
            field(
                "body",
                choice![s!(function_definition), s!(declaration), s!(declaration_list)]
            ),
        ],
    );
    g.define(
        "attribute_specifier",
        seq![choice!["__attribute__", "__attribute"], "(", s!(argument_list), ")"],
    );
    g.define(
        "attribute",
        seq![
            optional(seq![field("prefix", s!(identifier)), "::"]),
            field("name", s!(identifier)),
            optional(s!(argument_list)),
        ],
    );
    g.define("attribute_declaration", seq!["[[", comma_sep1(s!(attribute)), "]]"]);
    g.define("ms_declspec_modifier", seq!["__declspec", "(", s!(identifier), ")"]);
    g.define("ms_based_modifier", seq!["__based", s!(argument_list)]);
    g.define(
        "ms_call_modifier",
        choice![
            "__cdecl",
            "__clrcall",
            "__stdcall",
            "__fastcall",
            "__thiscall",
            "__vectorcall"
        ],
    );
    g.define("ms_restrict_modifier", "__restrict");
    g.define("ms_unsigned_ptr_modifier", "__uptr");
    g.define("ms_signed_ptr_modifier", "__sptr");
    g.define("ms_unaligned_ptr_modifier", choice!["_unaligned", "__unaligned"]);
    // MSVC `__ptr32` and `__ptr64` give the size of a pointer, and they stand where `__uptr` and
    // `__sptr` stand: after the `*` of a declarator. Clang reads them with `-fms-extensions`
    // (`Parser::ParseTypeQualifierListOpt`, clang/lib/Parse/ParseDecl.cpp).
    g.define("ms_sized_ptr_modifier", choice!["__ptr32", "__ptr64"]);
    g.define(
        "ms_pointer_modifier",
        choice![
            s!(ms_unaligned_ptr_modifier),
            s!(ms_restrict_modifier),
            s!(ms_unsigned_ptr_modifier),
            s!(ms_signed_ptr_modifier),
            s!(ms_sized_ptr_modifier),
        ],
    );
    // MSVC `__w64` marks a declaration for the portability warnings of a 64-bit build. It is not a
    // pointer modifier: it stands in the specifiers, as in `typedef int __w64 my_int;` and
    // `long __w64 v;`. Without the rule the second form reads `__w64` as the name of a type and
    // gives no ERROR node, so the tree is wrong and no error count shows it.
    g.define("ms_w64_modifier", "__w64");
    g.define(
        "declaration_list",
        seq![open_brace(), repeat(s!(_block_item)), close_brace()],
    );

    g.define(
        "_declarator",
        choice![
            s!(attributed_declarator),
            s!(pointer_declarator),
            s!(function_declarator),
            s!(array_declarator),
            s!(parenthesized_declarator),
            s!(identifier),
        ],
    );
    g.define(
        "_declaration_declarator",
        choice![
            s!(attributed_declarator),
            s!(pointer_declarator),
            alias(s!(_function_declaration_declarator), s!(function_declarator)),
            s!(array_declarator),
            s!(parenthesized_declarator),
            s!(identifier),
        ],
    );
    g.define(
        "_field_declarator",
        choice![
            alias(s!(attributed_field_declarator), s!(attributed_declarator)),
            alias(s!(pointer_field_declarator), s!(pointer_declarator)),
            alias(s!(function_field_declarator), s!(function_declarator)),
            alias(s!(array_field_declarator), s!(array_declarator)),
            alias(s!(parenthesized_field_declarator), s!(parenthesized_declarator)),
            s!(_field_identifier),
        ],
    );
    g.define(
        "_type_declarator",
        choice![
            alias(s!(attributed_type_declarator), s!(attributed_declarator)),
            alias(s!(pointer_type_declarator), s!(pointer_declarator)),
            alias(s!(function_type_declarator), s!(function_declarator)),
            alias(s!(array_type_declarator), s!(array_declarator)),
            alias(s!(parenthesized_type_declarator), s!(parenthesized_declarator)),
            s!(_type_identifier),
            alias(size_keyword(), s!(primitive_type)),
            s!(primitive_type),
        ],
    );
    g.define(
        "_abstract_declarator",
        choice![
            s!(abstract_pointer_declarator),
            s!(abstract_function_declarator),
            s!(abstract_array_declarator),
            s!(abstract_parenthesized_declarator),
        ],
    );

    let parenthesized = |declarator: Rule| {
        prec_dynamic(
            PAREN_DECLARATOR,
            seq!["(", optional(s!(ms_call_modifier)), declarator, ")"],
        )
    };
    g.define("parenthesized_declarator", parenthesized(s!(_declarator)));
    g.define("parenthesized_field_declarator", parenthesized(s!(_field_declarator)));
    g.define("parenthesized_type_declarator", parenthesized(s!(_type_declarator)));
    g.define(
        "abstract_parenthesized_declarator",
        prec(
            1,
            seq!["(", optional(s!(ms_call_modifier)), s!(_abstract_declarator), ")",],
        ),
    );

    let attributed = |declarator: Rule| prec_right(0, seq![declarator, repeat1(s!(attribute_declaration))]);
    g.define("attributed_declarator", attributed(s!(_declarator)));
    g.define("attributed_field_declarator", attributed(s!(_field_declarator)));
    g.define("attributed_type_declarator", attributed(s!(_type_declarator)));

    let pointer = |declarator: Rule| {
        prec_dynamic(
            1,
            prec_right(
                0,
                seq![
                    optional(s!(ms_based_modifier)),
                    "*",
                    repeat(s!(ms_pointer_modifier)),
                    repeat(s!(type_qualifier)),
                    field("declarator", declarator),
                ],
            ),
        )
    };
    g.define("pointer_declarator", pointer(s!(_declarator)));
    g.define("pointer_field_declarator", pointer(s!(_field_declarator)));
    g.define("pointer_type_declarator", pointer(s!(_type_declarator)));
    g.define(
        "abstract_pointer_declarator",
        prec_dynamic(
            1,
            prec_right(
                0,
                seq![
                    "*",
                    repeat(s!(ms_pointer_modifier)),
                    repeat(s!(type_qualifier)),
                    field("declarator", optional(s!(_abstract_declarator))),
                ],
            ),
        ),
    );

    g.define(
        "function_declarator",
        prec_right(
            1,
            seq![
                field("declarator", s!(_declarator)),
                field("parameters", s!(parameter_list)),
                optional(s!(gnu_asm_expression)),
                repeat(choice![
                    s!(attribute_specifier),
                    s!(identifier),
                    alias(s!(preproc_call_expression), s!(call_expression)),
                ]),
            ],
        ),
    );
    g.define(
        "_function_declaration_declarator",
        prec_right(
            1,
            seq![
                field("declarator", s!(_declarator)),
                field("parameters", s!(parameter_list)),
                optional(s!(gnu_asm_expression)),
                repeat(s!(attribute_specifier)),
            ],
        ),
    );
    let function = |declarator: Rule| {
        prec(
            1,
            seq![field("declarator", declarator), field("parameters", s!(parameter_list))],
        )
    };
    g.define("function_field_declarator", function(s!(_field_declarator)));
    g.define("function_type_declarator", function(s!(_type_declarator)));
    g.define(
        "abstract_function_declarator",
        function(optional(s!(_abstract_declarator))),
    );
    g.define(
        "_old_style_function_declarator",
        seq![
            field("declarator", s!(_declarator)),
            field("parameters", alias(s!(_old_style_parameter_list), s!(parameter_list))),
        ],
    );

    let array = |declarator: Rule| {
        prec(
            1,
            seq![
                field("declarator", declarator),
                "[",
                repeat(choice![s!(type_qualifier), "static"]),
                field("size", optional(choice![s!(expression), "*"])),
                close_bracket(),
            ],
        )
    };
    g.define("array_declarator", array(s!(_declarator)));
    g.define("array_field_declarator", array(s!(_field_declarator)));
    g.define("array_type_declarator", array(s!(_type_declarator)));
    g.define("abstract_array_declarator", array(optional(s!(_abstract_declarator))));

    g.define(
        "init_declarator",
        seq![
            field("declarator", s!(_declarator)),
            asm_label(),
            "=",
            field("value", choice![s!(initializer_list), s!(expression)]),
        ],
    );
    g.define(
        "compound_statement",
        seq![open_brace(), repeat(s!(_block_item)), close_brace()],
    );
    g.define(
        "storage_class_specifier",
        choice![
            "extern",
            "static",
            "auto",
            "register",
            "inline",
            "__inline",
            "__inline__",
            "__forceinline",
            "thread_local",
            "__thread",
        ],
    );
    g.define(
        "type_qualifier",
        choice![
            "const",
            "constexpr",
            "volatile",
            "restrict",
            "__restrict__",
            "_Atomic",
            "_Noreturn",
            "noreturn",
            "_Nonnull",
            s!(alignas_qualifier),
        ],
    );
    g.define("_extension_specifier", Rule::from("__extension__"));
    g.define(
        "alignas_qualifier",
        seq![
            choice!["alignas", "_Alignas"],
            "(",
            choice![s!(expression), s!(type_descriptor)],
            ")",
        ],
    );
    g.define(
        "type_specifier",
        choice![
            s!(struct_specifier),
            s!(union_specifier),
            s!(enum_specifier),
            s!(macro_type_specifier),
            s!(sized_type_specifier),
            s!(primitive_type),
            s!(_type_identifier),
        ],
    );
    let base_type = || {
        field(
            "type",
            choice![prec_dynamic(-1, s!(_type_identifier)), s!(primitive_type)],
        )
    };
    // A type qualifier in a sized type comes before a base type or a size keyword: `unsigned const
    // int`, `long const long`. A qualifier after the last keyword is a declaration specifier or a
    // qualifier of the type-id: `unsigned long const x;`, as for `int const x;`.
    //
    // The first form holds a base type. Without one it reads a sequence of size keywords, which is
    // also the second form with no base type and no qualifier. `unsigned long long` then had two
    // readings of one text with the same precedence, and the version order of the GLR parser picked
    // one of them. The two readings give the same children, because the repeat rule of the second
    // form is auxiliary. A base type in the first form leaves one reading. The first form still
    // reads a size keyword after the base type, and it reads a base type with no size keyword
    // before it: `int long`, `long int long`.
    g.define(
        "sized_type_specifier",
        choice![
            seq![repeat(size_keyword()), base_type(), repeat1(size_keyword())],
            seq![
                repeat1(size_keyword()),
                optional(choice![
                    seq![repeat(s!(type_qualifier)), base_type(), repeat(size_keyword())],
                    seq![
                        repeat1(s!(type_qualifier)),
                        repeat1(size_keyword()),
                        optional(base_type())
                    ],
                ]),
            ],
        ],
    );
    let mut primitive: Vec<Rule> = [
        "bool",
        "char",
        "int",
        "float",
        "double",
        "size_t",
        "ssize_t",
        "ptrdiff_t",
        "intptr_t",
        "uintptr_t",
        "charptr_t",
        "nullptr_t",
        "max_align_t",
    ]
    .map(Rule::from)
    .into();
    for prefix in ["int", "uint", "char"] {
        primitive.extend([8, 16, 32, 64].map(|bits| Rule::from(format!("{prefix}{bits}_t"))));
    }
    g.define("primitive_type", token(Rule::Choice(primitive)));
    g.define(
        "enum_specifier",
        seq![
            "enum",
            choice![
                seq![
                    field("name", s!(_type_identifier)),
                    optional(seq![":", field("underlying_type", s!(primitive_type))]),
                    field("body", optional(s!(enumerator_list))),
                ],
                field("body", s!(enumerator_list)),
            ],
            optional(s!(attribute_specifier)),
        ],
    );
    g.define(
        "enumerator_list",
        seq![
            open_brace(),
            repeat(choice![
                seq![s!(enumerator), ","],
                alias(s!(preproc_if_in_enumerator_list), s!(preproc_if)),
                alias(s!(preproc_ifdef_in_enumerator_list), s!(preproc_ifdef)),
                seq![s!(preproc_call), ","],
            ]),
            optional(choice![s!(enumerator), s!(preproc_call)]),
            close_brace(),
        ],
    );
    let record_body = || {
        choice![
            seq![
                field("name", s!(_type_identifier)),
                field("body", optional(s!(field_declaration_list)))
            ],
            field("body", s!(field_declaration_list)),
        ]
    };
    g.define(
        "struct_specifier",
        prec_right(
            0,
            seq![
                "struct",
                optional(s!(attribute_specifier)),
                optional(s!(ms_declspec_modifier)),
                record_body(),
                optional(s!(attribute_specifier)),
            ],
        ),
    );
    g.define(
        "union_specifier",
        prec_right(
            0,
            seq![
                "union",
                optional(s!(ms_declspec_modifier)),
                record_body(),
                optional(s!(attribute_specifier)),
            ],
        ),
    );
    g.define(
        "field_declaration_list",
        seq![open_brace(), repeat(s!(_field_declaration_list_item)), close_brace()],
    );
    g.define(
        "_field_declaration_list_item",
        choice![
            s!(field_declaration),
            s!(preproc_def),
            s!(preproc_function_def),
            s!(preproc_call),
            alias(s!(preproc_if_in_field_declaration_list), s!(preproc_if)),
            alias(s!(preproc_ifdef_in_field_declaration_list), s!(preproc_ifdef)),
        ],
    );
    g.define(
        "field_declaration",
        seq![
            s!(_declaration_specifiers),
            optional(s!(_field_declaration_declarator)),
            optional(s!(attribute_specifier)),
            ";",
        ],
    );
    g.define(
        "_field_declaration_declarator",
        comma_sep1(seq![
            field("declarator", s!(_field_declarator)),
            optional(s!(bitfield_clause)),
        ]),
    );
    g.define("bitfield_clause", seq![":", s!(expression)]);
    g.define(
        "enumerator",
        seq![
            field("name", s!(identifier)),
            optional(seq!["=", field("value", s!(expression))])
        ],
    );
    g.define("variadic_parameter", "...");
    g.define(
        "parameter_list",
        seq![
            "(",
            choice![
                comma_sep(choice![s!(parameter_declaration), s!(variadic_parameter)]),
                s!(compound_statement)
            ],
            ")",
        ],
    );
    g.define(
        "_old_style_parameter_list",
        seq!["(", comma_sep(choice![s!(identifier), s!(variadic_parameter)]), ")"],
    );
    g.define(
        "parameter_declaration",
        seq![
            s!(_declaration_specifiers),
            optional(field("declarator", choice![s!(_declarator), s!(_abstract_declarator)])),
            repeat(s!(attribute_specifier)),
        ],
    );
}

fn statements(g: &mut Grammar) {
    g.define(
        "attributed_statement",
        seq![repeat1(s!(attribute_declaration)), s!(statement)],
    );
    g.define("statement", choice![s!(case_statement), s!(_non_case_statement)]);
    g.define(
        "_non_case_statement",
        choice![
            s!(attributed_statement),
            s!(labeled_statement),
            s!(compound_statement),
            s!(expression_statement),
            s!(if_statement),
            s!(switch_statement),
            s!(do_statement),
            s!(while_statement),
            s!(for_statement),
            s!(return_statement),
            s!(break_statement),
            s!(continue_statement),
            s!(goto_statement),
            s!(seh_try_statement),
            s!(seh_leave_statement),
        ],
    );
    g.define(
        "_top_level_statement",
        choice![
            s!(case_statement),
            s!(attributed_statement),
            s!(labeled_statement),
            s!(compound_statement),
            alias(s!(_top_level_expression_statement), s!(expression_statement)),
            s!(if_statement),
            s!(switch_statement),
            s!(do_statement),
            s!(while_statement),
            s!(for_statement),
            s!(return_statement),
            s!(break_statement),
            s!(continue_statement),
            s!(goto_statement),
        ],
    );
    g.define(
        "labeled_statement",
        seq![
            field("label", s!(_statement_identifier)),
            ":",
            choice![s!(declaration), s!(statement)],
        ],
    );
    // This rule has no binary expression. It keeps the other expressions for macro code
    // and for code examples.
    g.define(
        "_top_level_expression_statement",
        seq![optional(s!(_expression_not_binary)), ";"],
    );
    g.define(
        "expression_statement",
        seq![optional(choice![s!(expression), s!(comma_expression)]), ";"],
    );
    g.define(
        "if_statement",
        prec_right(
            0,
            seq![
                "if",
                field("condition", s!(parenthesized_expression)),
                field("consequence", s!(statement)),
                optional(field("alternative", s!(else_clause))),
            ],
        ),
    );
    g.define("else_clause", seq!["else", s!(statement)]);
    g.define(
        "switch_statement",
        seq![
            "switch",
            field("condition", s!(parenthesized_expression)),
            field("body", s!(compound_statement)),
        ],
    );
    g.define(
        "case_statement",
        prec_right(
            0,
            seq![
                choice![seq!["case", field("value", s!(expression))], "default"],
                ":",
                repeat(choice![s!(_non_case_statement), s!(declaration), s!(type_definition)]),
            ],
        ),
    );
    g.define(
        "while_statement",
        seq![
            "while",
            field("condition", s!(parenthesized_expression)),
            field("body", s!(statement)),
        ],
    );
    g.define(
        "do_statement",
        seq![
            "do",
            field("body", s!(statement)),
            "while",
            field("condition", s!(parenthesized_expression)),
            ";",
        ],
    );
    g.define(
        "for_statement",
        seq!["for", "(", s!(_for_statement_body), ")", field("body", s!(statement))],
    );
    let optional_expression = || optional(choice![s!(expression), s!(comma_expression)]);
    g.define(
        "_for_statement_body",
        seq![
            choice![
                field("initializer", s!(declaration)),
                seq![field("initializer", optional_expression()), ";"]
            ],
            field("condition", optional_expression()),
            ";",
            field("update", optional_expression()),
        ],
    );
    g.define("return_statement", seq!["return", optional_expression(), ";"]);
    g.define("break_statement", seq!["break", ";"]);
    g.define("continue_statement", seq!["continue", ";"]);
    g.define(
        "goto_statement",
        seq!["goto", field("label", s!(_statement_identifier)), ";"],
    );
    g.define(
        "seh_try_statement",
        seq![
            "__try",
            field("body", s!(compound_statement)),
            choice![s!(seh_except_clause), s!(seh_finally_clause)],
        ],
    );
    g.define(
        "seh_except_clause",
        seq![
            "__except",
            field("filter", s!(parenthesized_expression)),
            field("body", s!(compound_statement)),
        ],
    );
    g.define(
        "seh_finally_clause",
        seq!["__finally", field("body", s!(compound_statement))],
    );
    g.define("seh_leave_statement", seq!["__leave", ";"]);
}

fn expressions(g: &mut Grammar) {
    g.define("expression", choice![s!(_expression_not_binary), s!(binary_expression)]);
    g.define(
        "_expression_not_binary",
        choice![
            s!(conditional_expression),
            s!(assignment_expression),
            s!(unary_expression),
            s!(update_expression),
            s!(cast_expression),
            s!(pointer_expression),
            s!(sizeof_expression),
            s!(alignof_expression),
            s!(offsetof_expression),
            s!(generic_expression),
            s!(subscript_expression),
            s!(call_expression),
            s!(field_expression),
            s!(compound_literal_expression),
            s!(identifier),
            s!(number_literal),
            s!(_string),
            s!(true),
            s!(false),
            s!(null),
            s!(char_literal),
            s!(parenthesized_expression),
            s!(gnu_asm_expression),
            s!(extension_expression),
        ],
    );
    g.define(
        "_string",
        prec_left(0, choice![s!(string_literal), s!(concatenated_string)]),
    );
    // The comma operator is left-associative: `a, b, c` is `(a, b), c`. GCC
    // `cp_parser_expression` and Clang `ParseRHSOfBinaryExpression` make this tree. The left
    // recursion of the rule gives the same tree.
    g.define(
        "comma_expression",
        seq![
            field("left", choice![s!(expression), s!(comma_expression)]),
            ",",
            field("right", s!(expression)),
        ],
    );
    g.define(
        "conditional_expression",
        prec_right(
            CONDITIONAL,
            seq![
                field("condition", s!(expression)),
                "?",
                optional(field("consequence", choice![s!(expression), s!(comma_expression)])),
                ":",
                field("alternative", s!(expression)),
            ],
        ),
    );
    g.define(
        "_assignment_left_expression",
        choice![
            s!(identifier),
            s!(call_expression),
            s!(field_expression),
            s!(pointer_expression),
            s!(subscript_expression),
            s!(parenthesized_expression),
        ],
    );
    g.define(
        "assignment_expression",
        prec_right(
            ASSIGNMENT,
            seq![
                field("left", s!(_assignment_left_expression)),
                field(
                    "operator",
                    choice!["=", "*=", "/=", "%=", "+=", "-=", "<<=", ">>=", "&=", "^=", "|="]
                ),
                field("right", s!(expression)),
            ],
        ),
    );
    g.define(
        "pointer_expression",
        prec_left(
            CAST,
            seq![field("operator", choice!["*", "&"]), field("argument", s!(expression)),],
        ),
    );
    // The GNU operators `__real__` and `__imag__` give the parts of a complex number. Each one takes a
    // cast expression, as the other unary operators do, and `__real__ z + 1` is `(__real__ z) + 1` (GCC
    // `c_parser_unary_expression` and `cp_parser_unary_expression` at RID_REALPART, Clang
    // `ParseCastExpression` at `tok::kw___real`). The operand of the operator is an lvalue, and
    // `__real__ z = v` assigns to the real part of `z`. Each operator has two spellings (GCC
    // `c_common_reswords` in c-common.cc).
    g.define(
        "unary_expression",
        prec_left(
            UNARY,
            seq![
                field(
                    "operator",
                    choice!["!", "~", "-", "+", "__real__", "__real", "__imag__", "__imag"]
                ),
                field("argument", s!(expression)),
            ],
        ),
    );
    g.define("binary_expression", binary_operations("expression"));
    let argument = || field("argument", s!(expression));
    let operator = || field("operator", choice!["--", "++"]);
    // A postfix increment is a postfix expression. Its precedence is more than the precedence of
    // each prefix operator, and `!n--` is `!(n--)`. GCC reads it in `cp_parser_postfix_expression`,
    // and Clang reads it in `ParsePostfixExpressionSuffix`. It has the precedence of a call.
    g.define(
        "update_expression",
        choice![
            prec_right(UNARY, seq![operator(), argument()]),
            prec_left(CALL, seq![argument(), operator()]),
        ],
    );
    g.define(
        "cast_expression",
        prec(
            CAST,
            seq![
                "(",
                field("type", s!(type_descriptor)),
                ")",
                field("value", s!(expression)),
            ],
        ),
    );
    g.define(
        "type_descriptor",
        seq![
            repeat(s!(type_qualifier)),
            field("type", s!(type_specifier)),
            repeat(s!(type_qualifier)),
            field("declarator", optional(s!(_abstract_declarator))),
        ],
    );
    g.define(
        "sizeof_expression",
        prec(
            SIZEOF,
            seq![
                "sizeof",
                choice![
                    field("value", s!(expression)),
                    seq!["(", field("type", s!(type_descriptor)), ")"]
                ],
            ],
        ),
    );
    // The operand of the standard operator is a parenthesized type-id only ([expr.alignof],
    // C23 6.5.4.5). The operand of the GNU spellings is a type or an expression, as the operand of
    // `sizeof` is: `__alignof__ s` and `__alignof__(s.m)` (GCC `cp_parser_unary_expression` at
    // RID_ALIGNOF with `cp_parser_sizeof_operand`, and `c_parser_alignof_expression`. Clang
    // `ParseExprAfterUnaryExprOrTypeTrait`). `_alignof` is the spelling of Microsoft, and Clang reads
    // it as `__alignof`.
    //
    // GCC and Clang also take an expression for `alignof` and `_Alignof`, and they give a warning for a
    // program that is not standard. In a standard program, the name of `alignof(name)` is a type.
    let parenthesized_type = || seq!["(", field("type", s!(type_descriptor)), ")"];
    g.define(
        "alignof_expression",
        prec(
            SIZEOF,
            choice![
                seq![choice!["alignof", "_Alignof"], parenthesized_type()],
                seq![
                    choice!["__alignof__", "__alignof", "_alignof"],
                    choice![field("value", s!(expression)), parenthesized_type()],
                ],
            ],
        ),
    );
    g.define(
        "offsetof_expression",
        prec(
            OFFSETOF,
            seq![
                "offsetof",
                seq![
                    "(",
                    field("type", s!(type_descriptor)),
                    ",",
                    field("member", s!(_field_identifier)),
                    ")"
                ],
            ],
        ),
    );
    g.define(
        "generic_expression",
        prec(
            CALL,
            seq![
                "_Generic",
                "(",
                s!(expression),
                ",",
                comma_sep1(seq![s!(type_descriptor), ":", s!(expression)]),
                ")",
            ],
        ),
    );
    g.define(
        "subscript_expression",
        prec(
            SUBSCRIPT,
            seq![
                field("argument", s!(expression)),
                "[",
                field("index", s!(expression)),
                close_bracket(),
            ],
        ),
    );
    g.define(
        "call_expression",
        prec(
            CALL,
            seq![field("function", s!(expression)), field("arguments", s!(argument_list)),],
        ),
    );
    g.define(
        "gnu_asm_expression",
        prec(
            CALL,
            seq![
                choice!["asm", "__asm__", "__asm"],
                repeat(s!(gnu_asm_qualifier)),
                "(",
                field("assembly_code", s!(_string)),
                optional(seq![
                    field("output_operands", s!(gnu_asm_output_operand_list)),
                    optional(seq![
                        field("input_operands", s!(gnu_asm_input_operand_list)),
                        optional(seq![
                            field("clobbers", s!(gnu_asm_clobber_list)),
                            optional(field("goto_labels", s!(gnu_asm_goto_list))),
                        ]),
                    ]),
                ]),
                ")",
            ],
        ),
    );
    g.define(
        "gnu_asm_qualifier",
        choice!["volatile", "__volatile__", "inline", "goto"],
    );
    let operand = || {
        seq![
            optional(seq!["[", field("symbol", s!(identifier)), close_bracket()]),
            field("constraint", s!(string_literal)),
            "(",
            field("value", s!(expression)),
            ")",
        ]
    };
    g.define(
        "gnu_asm_output_operand_list",
        seq![":", comma_sep(field("operand", s!(gnu_asm_output_operand)))],
    );
    g.define("gnu_asm_output_operand", operand());
    g.define(
        "gnu_asm_input_operand_list",
        seq![":", comma_sep(field("operand", s!(gnu_asm_input_operand)))],
    );
    g.define("gnu_asm_input_operand", operand());
    g.define(
        "gnu_asm_clobber_list",
        seq![":", comma_sep(field("register", s!(_string)))],
    );
    g.define(
        "gnu_asm_goto_list",
        seq![":", comma_sep(field("label", s!(identifier)))],
    );
    // `__extension__` is a unary operator with a cast expression as the operand, and
    // `__extension__ a + b` is `(__extension__ a) + b` (GCC `c_parser_unary_expression` and
    // `cp_parser_unary_expression`, Clang `ParseCastExpression`). The precedence is only on the
    // operand. The keyword keeps its conflict with `_extension_specifier`, the same keyword before
    // a declaration.
    g.define(
        "extension_expression",
        seq!["__extension__", prec_left(UNARY, s!(expression))],
    );
    // The compound statement is for macros that take statements as arguments, for
    // example `MYFORLOOP(1, 10, i, { foo(i); bar(i); })`.
    g.define(
        "argument_list",
        seq!["(", comma_sep(choice![s!(expression), s!(compound_statement)]), ")"],
    );
    g.define(
        "field_expression",
        seq![
            prec(
                FIELD,
                seq![field("argument", s!(expression)), field("operator", choice![".", "->"])]
            ),
            field("field", s!(_field_identifier)),
        ],
    );
    g.define(
        "compound_literal_expression",
        seq![
            "(",
            field("type", s!(type_descriptor)),
            ")",
            field("value", s!(initializer_list)),
        ],
    );
    g.define(
        "parenthesized_expression",
        seq![
            "(",
            choice![s!(expression), s!(comma_expression), s!(compound_statement)],
            ")",
        ],
    );
    g.define(
        "initializer_list",
        seq![
            open_brace(),
            comma_sep(choice![s!(initializer_pair), s!(expression), s!(initializer_list)]),
            optional(","),
            close_brace(),
        ],
    );
    let value = || field("value", choice![s!(expression), s!(initializer_list)]);
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
                "=",
                value(),
            ],
            seq![field("designator", s!(_field_identifier)), ":", value()],
        ],
    );
    g.define("subscript_designator", seq!["[", s!(expression), close_bracket()]);
    g.define(
        "subscript_range_designator",
        seq![
            "[",
            field("start", s!(expression)),
            "...",
            field("end", s!(expression)),
            close_bracket(),
        ],
    );
    g.define("field_designator", seq![".", s!(_field_identifier)]);
}

fn literals(g: &mut Grammar) {
    let digits = |digit: &str| seq![repeat1(re(digit)), repeat(seq!["'", repeat1(re(digit))])];
    let hex_digits = || digits("[0-9a-fA-F]");
    let decimal_digits = || digits("[0-9]");
    g.define(
        "number_literal",
        token(seq![
            optional(re(r"[-\+]")),
            optional(choice![re("0[xX]"), re("0[bB]")]),
            choice![
                seq![
                    choice![
                        decimal_digits(),
                        seq![re("0[bB]"), decimal_digits()],
                        seq![re("0[xX]"), hex_digits()]
                    ],
                    optional(seq![".", optional(hex_digits())]),
                ],
                seq![".", decimal_digits()],
            ],
            optional(seq![re("[eEpP]"), optional(seq![optional(re(r"[-\+]")), hex_digits()])]),
            re("[uUlLwWfFbBdD]*"),
        ]),
    );
    // A character literal and a string literal end at a line break, and a carriage return is also a
    // line break. libcpp `lex_string` (gcc/libcpp/lex.cc:2867) gives "missing terminating %c
    // character", because `_cpp_clean_line` (lex.cc:877) wrote a line feed at each of the two.
    // Clang gives `ext_unterminated_char_or_string` in `LexCharConstant`
    // (clang/lib/Lex/Lexer.cpp:2578) and in `LexStringLiteral` (:2342).
    g.define(
        "char_literal",
        seq![
            choice!["L'", "u'", "U'", "u8'", "'"],
            repeat1(choice![
                s!(escape_sequence),
                s!(_line_splice),
                alias(token_immediate(re(r"[^\r\n']")), s!(character))
            ]),
            "'",
        ],
    );
    // A concatenation has two or more parts, and one part is a string literal. An
    // identifier is a part for macros that are strings, for example `PRIu64`.
    g.define(
        "concatenated_string",
        prec_right(
            0,
            seq![
                choice![
                    seq![s!(identifier), s!(string_literal)],
                    seq![s!(string_literal), s!(string_literal)],
                    seq![s!(string_literal), s!(identifier)],
                ],
                repeat(choice![s!(string_literal), s!(identifier)]),
            ],
        ),
    );
    g.define(
        "string_literal",
        seq![
            choice!["L\"", "u\"", "U\"", "u8\"", "\""],
            repeat(choice![
                alias(token_immediate(prec(1, re(r#"[^\\"\r\n]+"#))), s!(string_content)),
                s!(escape_sequence),
                s!(_line_splice),
            ]),
            "\"",
        ],
    );
    // A line splice in a string literal or a character literal is not part of the value of the literal, and the tree
    // has no node for it: `"abc \` and `def"` on the next line hold the value `abc def`. The precedence of the token is
    // higher than the precedence of an escape sequence. A backslash before a line break then starts a line splice,
    // and not an escape sequence. A raw string literal has no line splice ([lex.pptoken]), and the scanner reads its
    // content.
    g.define("_line_splice", token_immediate(prec(2, re(LINE_SPLICE))));
    // A line splice with white space before its line break is an extra token between two tokens, and not a part of
    // the separator pattern of the extras. The lexer skips each character of a separator, also when the separator
    // does not match to its end. For a stray backslash with spaces after it, as in `\  // c`, such a separator skips
    // the spaces, and error recovery then skips the first `/` of the comment. A token skips no character, and the
    // ERROR node of error recovery holds the backslash and the spaces. The precedence of the token is the precedence
    // of the content of a string literal, so that the token does not take the spaces at the start of that content.
    g.define("_spaced_line_splice", token(prec(1, re(r"\\[ \t\f\v]+(\r\n?|\n)"))));
    g.define(
        "escape_sequence",
        token(prec(
            1,
            seq![
                "\\",
                choice![
                    re("[^xuU]"),
                    re(r"\d{2,3}"),
                    re("x[0-9a-fA-F]{1,4}"),
                    re("u[0-9a-fA-F]{4}"),
                    re("U[0-9a-fA-F]{8}"),
                ],
            ],
        )),
    );
    // A carriage return ends the line of a header name, as a line feed does. libcpp `lex_string`
    // (gcc/libcpp/lex.cc:2867) gives `CPP_LESS` for a header name with no `>` on its line, and Clang
    // `Lexer::LexAngledStringLiteral` (clang/lib/Lex/Lexer.cpp:2485) stops at each vertical white space.
    g.define(
        "system_lib_string",
        token(seq!["<", repeat(choice![re(r"[^>\r\n]"), "\\>"]), ">"]),
    );
    g.define("true", token(choice!["TRUE", "true"]));
    g.define("false", token(choice!["FALSE", "false"]));
    g.define("null", choice!["NULL", "nullptr"]);
}

fn names_and_comments(g: &mut Grammar) {
    g.define(
        "identifier",
        re(r"(\p{XID_Start}|\$|_|\\u[0-9A-Fa-f]{4}|\\U[0-9A-Fa-f]{8})(\p{XID_Continue}|\$|\\u[0-9A-Fa-f]{4}|\\U[0-9A-Fa-f]{8})*"),
    );
    g.define("_type_identifier", alias(s!(identifier), s!(type_identifier)));
    g.define("_field_identifier", alias(s!(identifier), s!(field_identifier)));
    g.define("_statement_identifier", alias(s!(identifier), s!(statement_identifier)));
    g.define("_empty_declaration", seq![s!(type_specifier), ";"]);
    g.define(
        "macro_type_specifier",
        prec_dynamic(
            -1,
            seq![
                field("name", s!(identifier)),
                "(",
                field("type", s!(type_descriptor)),
                ")",
            ],
        ),
    );
    // The expression for a block comment comes from
    // http://stackoverflow.com/questions/13014947/regex-to-match-a-c-style-multiline-comment/36328890#36328890
    g.define(
        "comment",
        token(choice![
            // A line splice continues a line comment: `\\*` and the splice read the last backslash of a run.
            //
            // A carriage return ends a line comment, with or without a line feed after it. libcpp
            // `_cpp_clean_line` (lex.cc:876) ends each line at a line feed or at a carriage return, and Clang
            // `Lexer::SkipLineComment` (Lexer.cpp:2693) stops its scan at the two characters. In a file with the
            // line ending of DOS, the comment node then holds no carriage return.
            seq!["//", re(&format!(r"(\\*{LINE_SPLICE}|\\+.|[^\\\r\n])*"))],
            seq!["/*", re(r"[^*]*\*+([^/*][^*]*\*+)*"), "/"],
        ]),
    );
}
