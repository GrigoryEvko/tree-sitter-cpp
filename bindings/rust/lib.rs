//! This crate provides C++ language support for the [tree-sitter][] parsing library.
//!
//! Typically, you will use the [LANGUAGE][] constant to add this language to a
//! tree-sitter [Parser][], and then use the parser to parse some code:
//!
//! ```
//! use tree_sitter::Parser;
//!
//! let code = r#"
//! int double(int x) {
//!     return x * 2;
//! }
//! "#;
//! let mut parser = tree_sitter::Parser::new();
//! let language = tree_sitter_cpp::LANGUAGE;
//! parser
//!     .set_language(&language.into())
//!     .expect("Error loading C++ parser");
//! let tree = parser.parse(code, None).unwrap();
//! assert!(!tree.root_node().has_error());
//! ```
//!
//! [Parser]: https://docs.rs/tree-sitter/*/tree_sitter/struct.Parser.html
//! [tree-sitter]: https://tree-sitter.github.io/

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_cpp() -> *const ();
    fn tree_sitter_cpp_seed_version() -> u32;
}

/// The tree-sitter [`LanguageFn`][LanguageFn] for this grammar.
///
/// [LanguageFn]: https://docs.rs/tree-sitter-language/*/tree_sitter_language/struct.LanguageFn.html
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_cpp) };

/// The version of the seed struct that the scanner of this grammar reads, `TS_CPP_SEED_VERSION` of
/// src/seed.h.
///
/// The scanner reads a seed of a different version as no name, with no error. Compare this version
/// with the version of a seed before you give the seed to `Parser::set_scanner_context`.
pub fn seed_version() -> u32 {
    // SAFETY: the function takes no argument and reads no state.
    unsafe { tree_sitter_cpp_seed_version() }
}

/// The content of the [`node-types.json`][] file for this grammar.
///
/// [`node-types.json`]: https://tree-sitter.github.io/tree-sitter/using-parsers#static-node-types
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

/// The syntax highlighting query for this language.
pub const HIGHLIGHT_QUERY: &str = include_str!("../../queries/highlights.scm");

/// The symbol tagging query for this language.
pub const TAGS_QUERY: &str = include_str!("../../queries/tags.scm");

#[cfg(test)]
mod tests {
    use std::ops::ControlFlow;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use tree_sitter::{InputEdit, Language, ParseOptions, ParseState, Parser, Point, Query, Tree};

    #[test]
    fn test_can_load_grammar() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading C++ parser");
    }

    /// The runtime in vendor/tree-sitter reads the 16-bit parse tables of the ABI versions 14 and 15 and the
    /// 32-bit parse tables of this grammar in the same process and in the same parser. The upstream grammars
    /// come from crates.io: tree-sitter-c 0.24.2 has ABI 15, and tree-sitter-typescript 0.23.2 has ABI 14.
    #[test]
    fn test_the_runtime_reads_upstream_grammars_next_to_this_grammar() {
        let cpp: Language = super::LANGUAGE.into();
        let c: Language = tree_sitter_c::LANGUAGE.into();
        let typescript: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        assert_eq!(cpp.abi_version(), tree_sitter::LANGUAGE_VERSION);
        assert_eq!(c.abi_version(), 15);
        assert_eq!(typescript.abi_version(), 14);
        let inputs = [
            (
                &cpp,
                "template <class T> struct S { T f(int x) const { return x < 2 ? g<T>(x) : x; } };\n",
                "translation_unit",
                "template_declaration",
            ),
            (&c, "int f(int x) { return x < 2 ? g(x) : x; }\n", "translation_unit", "function_definition"),
            (
                &typescript,
                "function f(x: number): number { return x < 2 ? g<number>(x) : x; }\n",
                "program",
                "function_declaration",
            ),
        ];
        let mut parser = Parser::new();
        // Two rounds change the language of the same parser before each parse.
        for _ in 0..2 {
            for &(language, text, root, first) in &inputs {
                parser.set_language(language).expect("the runtime reads the ABI of the language");
                let tree = parser.parse(text, None).expect("the parse ends");
                let node = tree.root_node();
                assert!(!node.has_error(), "{}", node.to_sexp());
                assert_eq!(node.kind(), root);
                assert_eq!(node.child(0).expect("the root has a child").kind(), first);
            }
        }
        // The analysis of a query reads the parse table of each state. A lookahead iterator reads the table of
        // one large state and of one small state, and each symbol in the table has a name.
        for (language, source) in [
            (&cpp, "(function_definition declarator: (function_declarator declarator: (_) @name))"),
            (&c, "(function_definition declarator: (function_declarator declarator: (identifier) @name))"),
            (&typescript, "(function_declaration name: (identifier) @name)"),
        ] {
            Query::new(language, source).expect("the query analysis reads the parse tables");
            let last_state = u32::try_from(language.parse_state_count() - 1).expect("a state id has 32 bits");
            let mut symbols = 0;
            for state in [1, last_state] {
                let iterator = language.lookahead_iterator(state).expect("the state exists");
                for symbol in iterator {
                    assert!(language.node_kind_for_id(symbol).is_some(), "the symbol {symbol} has a name");
                    symbols += 1;
                }
            }
            assert!(symbols > 0);
        }
    }

    /// The context of the parser reaches only a language of ABI 1017. An upstream grammar has no
    /// such field in its struct, and the runtime must not read past the end of the struct. The
    /// TypeScript grammar has an external scanner, so the runtime creates a scanner for it and then
    /// must not look for the entry point.
    #[test]
    fn test_a_scanner_context_does_not_reach_an_upstream_grammar() {
        let marker = 42u32;
        let context = std::ptr::addr_of!(marker).cast::<std::ffi::c_void>();
        for (language, source, root) in [
            (Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT), "let x = `a${1}b`;\n", "program"),
            (Language::from(tree_sitter_c::LANGUAGE), "int x = 1;\n", "translation_unit"),
            (Language::from(super::LANGUAGE), "int x = f(1);\n", "translation_unit"),
        ] {
            let mut parser = Parser::new();
            parser.set_language(&language).expect("the grammar loads");
            // The context comes before the first parse, so the runtime gives it right after `create`.
            unsafe { parser.set_scanner_context(context) };
            assert_eq!(parser.scanner_context(), context);
            let tree = parser.parse(source, None).expect("the parse ends");
            assert_eq!(tree.root_node().kind(), root);
            assert!(!tree.root_node().has_error(), "{source}");
            // The context changes after a parse, and a second parse takes the new one.
            unsafe { parser.set_scanner_context(std::ptr::null()) };
            assert!(parser.scanner_context().is_null());
            let again = parser.parse(source, None).expect("the parse ends");
            assert_eq!(again.root_node().to_sexp(), tree.root_node().to_sexp());
        }
    }

    /// The parse tables of this grammar are in the shape layout of ABI 1016, and the shape of a state
    /// keeps its symbols in the order in which the runtime reads them. A large state keeps ascending
    /// symbol order, and a small state keeps group order. The order decides which reduction survives
    /// in error recovery, so a change of the order changes a tree with no ERROR node.
    #[test]
    fn test_the_lookahead_order_of_the_shape_parse_tables() {
        let cpp: Language = super::LANGUAGE.into();
        assert_eq!(cpp.abi_version(), 1017);
        let state_count = u32::try_from(cpp.parse_state_count()).expect("a state id has 32 bits");
        let mut ascending = 0;
        let mut group_order = 0;
        for state in (0..state_count).step_by(101) {
            let symbols: Vec<u16> = cpp.lookahead_iterator(state).expect("the state exists").collect();
            assert!(!symbols.is_empty(), "the state {state} has no valid symbol");
            for &symbol in &symbols {
                assert!(cpp.node_kind_for_id(symbol).is_some(), "the symbol {symbol} has a name");
            }
            if symbols.windows(2).all(|pair| pair[0] < pair[1]) {
                ascending += 1;
            } else {
                group_order += 1;
            }
        }
        assert!(ascending > 0, "no sampled large state reads its symbols in ascending order");
        assert!(group_order > 0, "no sampled small state reads its symbols in group order");
    }

    /// The kind, the field name, and the byte range of each node of a tree, one line for each node.
    ///
    /// O(n) in the nodes of the tree.
    fn dump(tree: &Tree) -> String {
        let mut out = String::new();
        let mut cursor = tree.walk();
        let mut depth = 0;
        loop {
            let node = cursor.node();
            out.push_str(&format!(
                "{depth} {} {} {} {}{}\n",
                cursor.field_name().unwrap_or("-"),
                node.kind(),
                node.start_byte(),
                node.end_byte(),
                if node.is_missing() { " MISSING" } else { "" }
            ));
            if cursor.goto_first_child() {
                depth += 1;
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return out;
                }
                depth -= 1;
            }
        }
    }

    /// The edit removes the start of a comment. The fresh parse reads `else` as an identifier. The
    /// incremental parse gets the old `else_clause` node as the lookahead, and it must give the same
    /// tree as the fresh parse.
    #[test]
    fn test_incremental_parse_of_a_reused_node_with_no_action() {
        let before = "    {\n\n    // Verify that the drawing is actually a plane drawing\n    \
                      if (is_straight_line_drawing(g, straight_line_drawing))\n        \
                      std::cout << \"Is a plane drawing.\" << std::endl;\n    else\n        \
                      std::cout << \"Is not a plane drawing.\" << std::endl;\n    return 0;\n";
        let (start, removed) = (6, 12);
        let after = format!("{}{}", &before[..start], &before[start + removed..]);
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        let mut old = parser.parse(before, None).expect("the parse of the text before the edit ends");
        old.edit(&InputEdit {
            start_byte: start,
            old_end_byte: start + removed,
            new_end_byte: start,
            start_position: Point::new(0, 6),
            old_end_position: Point::new(2, 11),
            new_end_position: Point::new(0, 6),
        });
        let incremental = parser.parse(&after, Some(&old)).expect("the incremental parse ends");
        let fresh = parser.parse(&after, None).expect("the fresh parse ends");
        assert_eq!(dump(&incremental), dump(&fresh));
    }

    /// The row and the byte column of a byte offset. O(n) in the offset.
    fn point_at(text: &str, byte: usize) -> Point {
        let prefix = &text.as_bytes()[..byte];
        let row = prefix.iter().filter(|&&b| b == b'\n').count();
        let column = byte - prefix.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
        Point::new(row, column)
    }

    /// Parse `before`. Then replace `removed` bytes at `start` with `inserted`, and parse the new text
    /// with the edited old tree and with no old tree. Return the dumps of the incremental tree and of
    /// the fresh tree.
    ///
    /// O(n) in the bytes of the text, for each of the three parses.
    fn incremental_and_fresh(before: &str, start: usize, removed: usize, inserted: &str) -> (String, String) {
        let after = format!("{}{inserted}{}", &before[..start], &before[start + removed..]);
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        let mut old = parser.parse(before, None).expect("the parse of the text before the edit ends");
        old.edit(&InputEdit {
            start_byte: start,
            old_end_byte: start + removed,
            new_end_byte: start + inserted.len(),
            start_position: point_at(before, start),
            old_end_position: point_at(before, start + removed),
            new_end_position: point_at(&after, start + inserted.len()),
        });
        let incremental = parser.parse(&after, Some(&old)).expect("the incremental parse ends");
        let fresh = parser.parse(&after, None).expect("the fresh parse ends");
        (dump(&incremental), dump(&fresh))
    }

    /// The incremental parse reuses the old `template_argument_list` of `ration<double>`. A fresh parse
    /// lexes the second `>` in the state after the first `>`, and gets the `>` that closes a template
    /// argument list. The incremental parse must lex it in the same state.
    #[test]
    fn test_incremental_parse_lexes_after_a_reused_node_as_a_fresh_parse() {
        let before = "#include <Kokk  using Clock = std::chrono::high_resolution_clock;\n    return \
                      duration_cast<duMACRO_NAME ration<double>>(now - start_).count();\n";
        let (incremental, fresh) = incremental_and_fresh(before, 12, 2, ">");
        assert_eq!(incremental, fresh);
    }

    /// The incremental parse reuses the old `template_parameter_list` of `template<typename It>`. A
    /// fresh parse lexes `inline` in the state after `>`, where `inline` is not a keyword.
    #[test]
    fn test_incremental_parse_reads_a_keyword_after_a_reused_node_as_a_fresh_parse() {
        let before = "  typedef std::map<key_type, mapped_type, Cmp_Fn,\n{ PB_DS_ASSERT_VALID((*this)) }\n}\n\
                      PB_DS_CLASS_T_DEC\ntemplate<typename It>\ninline void\n";
        let (incremental, fresh) = incremental_and_fresh(before, 81, 1, "");
        assert_eq!(incremental, fresh);
    }

    /// The reductions for a reused node make two versions: `Iterator` as an attribute macro and as a
    /// type. A fresh parse shifts the first token of the node in each version, and the versions merge.
    #[test]
    fn test_incremental_parse_shifts_a_reused_node_in_one_version_only() {
        let before = "    retuIterator connect(basic_socket<Protocol, Executor>& s, Iterator begin,\n    >)\n    \
                      {\n    }\n  }\n}\n{\n        index_(other.index_),\n        start_(other.start_),\n        \
                      handler_(static_cast<RangeConnectHandler&&>(other.handler_))\n    {\n    }\n          }\n";
        let (incremental, fresh) = incremental_and_fresh(before, 2, 6, "");
        assert_eq!(incremental, fresh);
    }

    /// The old `template_argument_list` of `abi_t<T, N>` has a child that the parser made with two
    /// versions. After the edit, a fresh parse reads `N` in a different context and selects a different
    /// tree. The incremental parse must not reuse the list.
    #[test]
    fn test_incremental_parse_does_not_reuse_a_node_with_a_fragile_child() {
        let before = "        else if constexpr( cat == category::float16x32 )       return \
                      _mm512_range_ph(v0, v1, ctrl);\n/*\n*/\n//==============================================\
                      ====================================================\n -------------------------------\
                      ----------------------------------------------------------------\n  \
                      negabsmax_(EVE_REQUIRES(avx512_),\n             wide<T, N> const & w) noexcept requires \
                      x86_abi<abi_t<T, N>>\n";
        let (incremental, fresh) = incremental_and_fresh(before, 108, 0, "::");
        assert_eq!(incremental, fresh);
    }

    /// The text of the name of each `attribute_macro` node of a tree, in the order of the tree. O(n) in the
    /// nodes of the tree.
    fn attribute_macro_names<'a>(tree: &Tree, text: &'a str) -> Vec<&'a str> {
        let mut names = Vec::new();
        let mut cursor = tree.walk();
        loop {
            let node = cursor.node();
            if node.kind() == "attribute_macro" {
                let name = node.child_by_field_name("name").expect("an attribute macro has a name");
                names.push(&text[name.start_byte()..name.end_byte()]);
            }
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return names;
                }
            }
        }
    }

    /// The external scanner reads the white space after the name of a call macro and of a trailing macro. The
    /// token of the name holds the characters of the name, and no white space.
    #[test]
    fn test_macro_name_tokens_hold_the_name() {
        let text = "DWORD WINAPI f(LPVOID p);\nstruct S {\n  int n GUARDED_BY (mu);\n  void g()\n    REQUIRES (mu) ;\n};\n";
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        let tree = parser.parse(text, None).expect("the parse ends");
        assert!(!tree.root_node().has_error());
        assert_eq!(attribute_macro_names(&tree, text), ["WINAPI", "GUARDED_BY", "REQUIRES"]);
    }

    /// The kind and the text of the first child of each qualifier and ptr-operator node of a tree, in the order
    /// of the tree. A node with no child gives an empty text. O(n) in the nodes of the tree.
    fn operator_tokens<'t, 'a>(tree: &'t Tree, text: &'a str) -> Vec<(&'t str, &'a str)> {
        let mut tokens = Vec::new();
        let mut cursor = tree.walk();
        loop {
            let node = cursor.node();
            if matches!(
                node.kind(),
                "type_qualifier" | "abstract_pointer_declarator" | "abstract_reference_declarator"
            ) {
                let mut children = node.walk();
                let first = if children.goto_first_child() {
                    let child = children.node();
                    &text[child.start_byte()..child.end_byte()]
                } else {
                    ""
                };
                tokens.push((node.kind(), first));
            }
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return tokens;
                }
            }
        }
    }

    /// A conversion function name in a using-declaration and after a member access has the nodes of the
    /// declarator of a conversion function. Each qualifier holds its keyword, and each ptr-operator holds its
    /// token.
    #[test]
    fn test_conversion_function_names_hold_the_operator_tokens() {
        let text = "struct D : S {\n  using S::operator const char *;\n  void f() { p->operator char const * const &&(); }\n};\n";
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        let tree = parser.parse(text, None).expect("the parse ends");
        assert!(!tree.root_node().has_error());
        assert_eq!(
            operator_tokens(&tree, text),
            [
                ("type_qualifier", "const"),
                ("abstract_pointer_declarator", "*"),
                ("type_qualifier", "const"),
                ("abstract_pointer_declarator", "*"),
                ("type_qualifier", "const"),
                ("abstract_reference_declarator", "&&"),
            ]
        );
    }

    /// Each branch of the external scanner that returns a token ends the token with `mark_end`. In a build
    /// without NDEBUG, an assertion in `tree_sitter_cpp_external_scanner_scan` stops the process at a token with
    /// no `mark_end`, and at an empty token that `can_be_empty` does not permit. These texts give the names of
    /// enumerator macros, an empty raw string content, the end of a directive line at the end of the input, and a
    /// false branch at the end of the input, where the scanner gives no empty skipped text.
    #[test]
    fn test_scanner_tokens_end_at_their_marks() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        let text = "enum E { A MACRO, B DEPR = 1 };";
        let tree = parser.parse(text, None).expect("the parse ends");
        assert!(!tree.root_node().has_error());
        assert_eq!(attribute_macro_names(&tree, text), ["MACRO", "DEPR"]);
        for text in ["const char *s = R\"x()x\";\n#define A", "int x;\n#if 0"] {
            let tree = parser.parse(text, None).expect("the parse ends");
            assert!(!tree.root_node().has_error(), "{text}");
            assert_eq!(tree.root_node().end_byte(), text.len(), "{text}");
        }
    }

    /// The old `string_literal` has an ERROR node in a child in the middle. Error recovery depends on
    /// the text outside the literal, and the incremental parse must not reuse the literal.
    #[test]
    fn test_incremental_parse_does_not_reuse_a_node_with_an_error() {
        let before = "static_assert(st\r\n_sammplicitCo0x1p3nve\\0_sammplicitCo0x1p3nve\\0x1p300e9::)x\"isBAR_BAZ\n\
                      _sammplicitCo0x1p3nve\\0x1p300e9::)xWINAPI \"isBAR_BAZ\n";
        let (incremental, fresh) = incremental_and_fresh(before, 39, 2, ";");
        assert_eq!(incremental, fresh);
    }

    /// The S-expression of the tree of a text.
    fn sexp(parser: &mut Parser, text: &str) -> String {
        parser.parse(text, None).expect("the parse ends").root_node().to_sexp()
    }

    /// The scan of a name in parentheses (`scan_parenthesized_name` in src/scanner.c) reads a maximum of 4,096
    /// characters from its `(`. In these texts, a block comment after the name ends after that budget. Before,
    /// the scan read or compared characters after its budget: a `)` in the comment then gave a type reading to
    /// `sizeof(Value /*...)*/)` and a cast to `(Value /*...)*/) * x`. The tree must not depend on the text after
    /// the budget, and it must be the tree of the same text with no `)` in the comment.
    #[test]
    fn test_parenthesized_name_scan_reads_no_text_after_its_budget() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        let forms = [
            ("int n = sizeof(Value /*", "*/);\n"),
            ("void f() { g((Value /*", "*/) * x); }\n"),
        ];
        let sizeof_expression = "(translation_unit (declaration type: (primitive_type) declarator: (init_declarator \
                                 declarator: (identifier) value: (sizeof_expression value: (parenthesized_expression \
                                 (identifier) (comment))))))";
        let binary_expression = "(translation_unit (function_definition type: (primitive_type) declarator: \
                                 (function_declarator declarator: (identifier) parameters: (parameter_list)) body: \
                                 (compound_statement (expression_statement (call_expression function: (identifier) \
                                 arguments: (argument_list (binary_expression left: (parenthesized_expression \
                                 (identifier) (comment)) right: (identifier))))))))";
        let (head, tail) = forms[0];
        let edge = format!("{head}{}){tail}", "a".repeat(4082));
        assert_eq!(sexp(&mut parser, &edge), sizeof_expression);
        let (head, tail) = forms[1];
        let edge = format!("{head}{}){tail}", "a".repeat(4079));
        assert_eq!(sexp(&mut parser, &edge), binary_expression);

        // With a short comment, the scan reads the full name and selects the reading from the shape of the name.
        let sizeof_type = sexp(&mut parser, "int n = sizeof(Value /* c */);\n");
        assert!(
            sizeof_type.contains("(sizeof_expression type: (type_descriptor"),
            "{sizeof_type}"
        );
        let cast = sexp(&mut parser, "void f() { g((Value /* c */) * x); }\n");
        assert!(cast.contains("(cast_expression type: (type_descriptor"), "{cast}");

        for (head, tail) in forms {
            for count in 4072..4092 {
                for inner in [")", "<", ")::", "<T>)", ":: y)"] {
                    let text = format!("{head}{}{inner}{tail}", "a".repeat(count));
                    let plain = format!("{head}{}{tail}", "a".repeat(count + inner.len()));
                    assert_eq!(
                        sexp(&mut parser, &text),
                        sexp(&mut parser, &plain),
                        "a comment of {count} characters and {inner:?} in `{head}`"
                    );
                }
            }
        }
    }

    /// In this text, the budget of the scan of `(x` ends in the comment at a `<`. The read of the token then
    /// gave no token, and the scan compared the text of an uninitialized token with `<`. A run of valgrind on
    /// `xtask parse` of this text does the check of the read. The tree is a parenthesized expression.
    #[test]
    fn test_parenthesized_name_scan_checks_the_result_of_the_token_read() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        let text = format!("int y = (x /*{}<*/);\n", "a".repeat(4092));
        assert_eq!(
            sexp(&mut parser, &text),
            "(translation_unit (declaration type: (primitive_type) declarator: (init_declarator declarator: \
             (identifier) value: (parenthesized_expression (identifier) (comment)))))"
        );
    }

    /// Texts with line splices: each backslash before a line feed starts a line splice. The texts put a splice in a
    /// string literal and in a character literal with each prefix, after the spaces at the start of a string literal,
    /// and in a quote or a comment that each scanner loop reads with `skip_backslash` or `step_backslash`
    /// (src/scanner.c): a conditional group and its directive lines, the arguments of a macro, the parameters after a
    /// call macro, a decay-copy, a Qt `foreach`, and the lookaheads after a macro invocation line. The last texts put
    /// a splice between tokens and in a directive line.
    const LINE_SPLICE_TEXTS: [&str; 16] = [
        "auto a = \"abc \\\ndef\"; auto b = L\"x\\\ny\"; auto c = u\"x\\\ny\"; auto d = U\"x\\\ny\"; auto e = u8\"x\\\ny\";\n",
        "const char *s = \"  \\\nx\";\n",
        "char a = '\\\nx'; auto b = L'\\\nx'; auto c = u'\\\nx'; auto d = U'\\\nx'; auto e = u8'\\\nx';\n",
        "#ifdef A\n#error \"do not use \\\nthis\"\n#endif\nint x;\n",
        "#ifdef A\n#error do not use this, it's old \\\nand broken\n#endif\nint x;\n",
        "#ifdef A\nconst char *s = \"abc \\\ndef\";\n#endif\nint x;\n",
        "struct S {\n  FOO(\"abc \\\ndef\")\n  int x;\n};\n",
        "int WINAPI f(const char *s = \"\\\n):\");\n",
        "auto s = auto(\"\\\n(\");\n",
        "void f() {\n  foreach (QString s, QStringList{\"\\\n,\"}) g(s);\n}\n",
        "void g() {\n  FOO(x) // c \\\n  + 1;\n  int z;\n}\n",
        "void g() {\n  FOO(x)\n#error \"a \\\n+ b\"\n  int z;\n}\n",
        "#ifdef A\n// a comment \\\n#error \"x\nint a;\n#endif\nint x;\n",
        "#ifdef A\n// C:\\\\\n#error \"x\nint a;\n#endif\nint x;\n",
        "int a;\n\\\n#define X 1\nint b;\n",
        "#define M(a) \\\n  a + \\\n  1\nint x = M(2);\n",
    ];

    /// Phase 2 of [lex.phases] deletes each line splice before tokenization (GCC `_cpp_clean_line`, Clang
    /// `Lexer::getEscapedNewLineSize`). A text with CR LF line ends gives the tree of the same text with LF line ends.
    /// A text with white space between each backslash and its line break, with LF or CR LF line ends, also gives that
    /// tree. The trees have no ERROR node and no MISSING node.
    ///
    /// THE TEXTS OF THIS TEST REACH AN EXTERNAL TOKEN OF THE SCANNER, and that is an accident of the
    /// texts and not the subject of this test. A failure here WITH NO CHANGE OF THE LINE SPLICES
    /// means that the external lists moved: the externals of grammar/src/cpp.rs and the enumerators
    /// of `TokenType` in src/scanner.c give one slot to each token, and a slot that holds two
    /// different tokens makes the scanner answer for a token that the parser did not ask for. Read
    /// the failure of `the_external_tokens_of_the_scanner_agree_with_the_grammar` in
    /// xtask/src/generate.rs first, because it names the slot and both names. DO NOT REPAIR THIS
    /// TEST BY A CHANGE OF ITS TEXTS: that hides the defect and removes the reach of these texts in
    /// one edit.
    #[test]
    fn test_line_splices_with_crlf_and_white_space_give_the_trees_of_lf_line_splices() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        for text in LINE_SPLICE_TEXTS {
            let lf = sexp(&mut parser, text);
            assert!(!lf.contains("ERROR") && !lf.contains("MISSING"), "{text:?} gives {lf}");
            let blank = text.replace("\\\n", "\\ \t\n");
            for variant in [text.replace('\n', "\r\n"), blank.replace('\n', "\r\n"), blank] {
                assert_eq!(sexp(&mut parser, &variant), lf, "the text {variant:?}");
            }
        }
    }

    /// The tree of line splices with white space after the backslash and CR LF line ends. A corpus file cannot hold
    /// white space at the end of a line. The value of `s` is `abc def`, the value of `c` is `x`, the comment holds
    /// `+ 42`, and the value of `X` is `1 + 2` (GCC and Clang, P2223R2).
    #[test]
    fn test_line_splices_with_white_space_and_crlf_give_the_tree_of_the_front_ends() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        let text = "const char *s = \"abc \\ \t\r\ndef\";\r\nchar c = u8'\\  \r\nx';\r\nint i = 1 // \\ \r\n+ 42\r\n;\r\n\
                    #define X 1 \\  \r\n + 2\r\n#ifdef A\r\n#error \"do not use \\ \r\nthis\"\r\n#endif\r\nint y;\r\n";
        assert_eq!(
            sexp(&mut parser, text),
            "(translation_unit (declaration (type_qualifier) type: (primitive_type) declarator: (init_declarator \
             declarator: (pointer_declarator declarator: (identifier)) value: (string_literal (string_content) \
             (string_content)))) (declaration type: (primitive_type) declarator: (init_declarator declarator: \
             (identifier) value: (char_literal (character)))) (declaration type: (primitive_type) declarator: \
             (init_declarator declarator: (identifier) value: (number_literal)) (comment)) (preproc_def name: \
             (identifier) value: (preproc_arg)) (preproc_ifdef name: (identifier) (preproc_call directive: \
             (preproc_directive) argument: (preproc_arg))) (declaration type: (primitive_type) declarator: \
             (identifier)))"
        );
    }

    /// The kind and the byte range of each node of a tree, in the order of the tree. O(n) in the nodes of the tree.
    fn kinds_and_ranges(tree: &Tree) -> Vec<(String, usize, usize)> {
        let mut nodes = Vec::new();
        let mut cursor = tree.walk();
        loop {
            let node = cursor.node();
            nodes.push((node.kind().to_owned(), node.start_byte(), node.end_byte()));
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return nodes;
                }
            }
        }
    }

    /// A backslash with spaces and a comment after it, and no line break after the spaces, is not a line splice (GCC
    /// `_cpp_clean_line`, Clang `Lexer::getEscapedNewLineSize`). The ERROR node of error recovery starts at the
    /// backslash, and the comment after the spaces stays a comment node.
    #[test]
    fn test_a_stray_backslash_with_spaces_keeps_the_comment_after_it() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        let text = "int a = b \\   // c\nint d;\n";
        let tree = parser.parse(text, None).expect("the parse ends");
        let nodes = kinds_and_ranges(&tree);
        let backslash = text.find('\\').expect("the text has a backslash");
        let comment = text.find("//").expect("the text has a comment");
        let comment_end = text.find('\n').expect("the text has a line break");
        assert!(
            nodes.iter().any(|(kind, start, _)| kind == "ERROR" && *start == backslash),
            "{nodes:?}"
        );
        assert!(
            nodes.contains(&("comment".to_owned(), comment, comment_end)),
            "{nodes:?}"
        );
    }

    /// After `e"x`, a MISSING `"` closes the string literal, but a version with that token cannot shift the `}` of
    /// the next line. Error recovery must skip the `}`. When such a version stayed, each recovery added one more
    /// MISSING token at the same position, and the parse did not end. The parse runs in a thread, and its progress
    /// callback stops it at the time limit.
    #[test]
    fn test_error_recovery_after_an_unterminated_string_ends() {
        const LIMIT: Duration = Duration::from_secs(10);
        let text = "b<C>(b<D>(e\"x\n}\n";
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut parser = Parser::new();
            parser
                .set_language(&super::LANGUAGE.into())
                .expect("the C++ language loads");
            let started = Instant::now();
            let mut stop = |_: &ParseState| {
                if started.elapsed() > LIMIT {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            };
            let tree = parser.parse_with_options(
                &mut |at, _| text.as_bytes().get(at..).unwrap_or_default(),
                None,
                Some(ParseOptions::new().progress_callback(&mut stop)),
            );
            // The test can stop before the send, and then the send gives an error that has no effect.
            let _ = sender.send(tree.map(|tree| tree.root_node().to_sexp()));
        });
        let sexp = receiver
            .recv_timeout(LIMIT + Duration::from_secs(5))
            .expect("the parse thread sends a result before the time limit")
            .expect("the parse ends before the time limit");
        assert!(sexp.contains("(ERROR"), "{sexp}");
    }

    /// The scanner records the kind of MAX_GROUPS open conditional groups, and it counts each group that is
    /// deeper. The directives of a deeper group are lines, and its `#endif` closes it. Before the count, the
    /// scanner dropped a deeper group: its `#else` closed a branch of the group of the array around it, each
    /// later directive belonged to the group before it, and the last `#endif` of the text had no group.
    #[test]
    fn test_conditional_groups_deeper_than_the_array_are_lines() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        // The counts of the nodes are the limit of the scanner: 64 groups with a kind. A group with more than
        // 64 groups in it is also a group of lines, because the scan of its text records each group in it.
        for (depth, groups) in [(63usize, 63usize), (64, 64), (65, 64), (70, 59)] {
            let mut text = String::new();
            for level in 1..=depth {
                text.push_str(&format!("#ifdef A{level}\n"));
            }
            text.push_str("int x;\n#else\nint y;\n");
            text.push_str(&"#endif\n".repeat(depth));
            text.push_str("int after;\n");
            let tree = parser.parse(&text, None).expect("the parse ends");
            let root = tree.root_node();
            let sexp = root.to_sexp();
            assert!(!root.has_error(), "{depth} groups: {sexp}");
            assert_eq!(sexp.matches("(preproc_ifdef").count(), groups, "{depth} groups: {sexp}");
            // The declaration after the groups is the last child of the tree. A directive of a group that the
            // scanner dropped came after it.
            let last = root.child(root.child_count() - 1).expect("the tree has a child");
            assert_eq!(last.kind(), "declaration", "{depth} groups: {sexp}");
        }
    }

    /// A backslash before a carriage return is a line splice, with or without a line feed after the carriage
    /// return. The corpus gives no coverage for a lone carriage return, and this test holds the exact bytes.
    ///
    /// A line feed and a carriage return after it are two line breaks, as libcpp `_cpp_clean_line` reads them.
    /// Clang `Lexer::getEscapedNewLineSize` reads that order as one line break, and the two front ends disagree.
    #[test]
    fn test_a_line_splice_ends_at_a_carriage_return() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        // Each input holds one declaration with a line splice in it. The declaration starts at byte 0 and ends
        // at the semicolon, so its range shows that the splice is white space between the two tokens.
        for (text, end) in [
            ("int \\\ry = 2;\n", 12),
            ("int \\\r\nz = 3;\n", 13),
            ("int \\  \rq = 4;\n", 14),
            ("int \\\nw = 5;\n", 12),
            // A line feed, a carriage return, and then the rest of the declaration. The splice ends at the line
            // feed, and the carriage return is white space before the next token.
            ("int \\\n\rv = 6;\n", 13),
        ] {
            let tree = parser.parse(text, None).expect("the parse ends");
            let root = tree.root_node();
            let sexp = root.to_sexp();
            assert!(!root.has_error(), "{text:?}: {sexp}");
            let first = root.child(0).expect("the tree has a child");
            assert_eq!(first.kind(), "declaration", "{text:?}: {sexp}");
            assert_eq!(first.start_byte(), 0, "{text:?}: {sexp}");
            assert_eq!(first.end_byte(), end, "{text:?}: {sexp}");
        }
    }

    /// A line comment ends at a carriage return, with or without a line feed after it. The comment node of a file
    /// with the line ending of DOS then holds no carriage return.
    #[test]
    fn test_a_line_comment_ends_at_a_carriage_return() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        // Each input holds one comment and one declaration. The end byte of the comment node is the byte of the
        // first character of its line break.
        for (text, end) in [
            ("// c\rint x;\r", 4),
            ("// c\r\nint x;\r\n", 4),
            ("// c\nint x;\n", 4),
            // A line splice continues the comment over the carriage return, and the comment holds the next line.
            ("// x \\\rint x;\rint y;\r", 13),
            // A line feed and a carriage return after the backslash are two line breaks. The comment ends at the
            // carriage return, and the declaration after it is code.
            ("// x \\\n\rint x;\n", 7),
        ] {
            let tree = parser.parse(text, None).expect("the parse ends");
            let root = tree.root_node();
            let sexp = root.to_sexp();
            assert!(!root.has_error(), "{text:?}: {sexp}");
            let first = root.child(0).expect("the tree has a child");
            assert_eq!(first.kind(), "comment", "{text:?}: {sexp}");
            assert_eq!(first.start_byte(), 0, "{text:?}: {sexp}");
            assert_eq!(first.end_byte(), end, "{text:?}: {sexp}");
            let last = root.child(root.child_count() - 1).expect("the tree has a child");
            assert_eq!(last.kind(), "declaration", "{text:?}: {sexp}");
        }
    }

    /// The text of a directive line ends at its last character that is not white space, and it holds each
    /// literal as one token. The external scanner reads it (`scan_preproc_arg` in src/scanner.c).
    #[test]
    fn test_the_text_of_a_directive_line_ends_at_its_last_token() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("the C++ language loads");
        // Each input holds one directive. The range is the range of the first `preproc_arg` node.
        for (text, start, end) in [
            ("#define A 1   \n", 10, 11),
            ("#define A 1 /* c */ 2\n", 10, 11),
            ("#define A 1 // c\n", 10, 11),
            ("#define A 1/\n", 10, 12),
            ("#define A \"a/*b*/c\"\n", 10, 19),
            ("#define A R\"(a\nb)\"\n", 10, 18),
            // A line splice before the text is white space, and the text starts on the next line.
            ("#define A \\\n  b c\n", 14, 17),
            // The `(` of a parameter list comes immediately after the name. With white space before it,
            // the `(` is the first character of the text.
            ("#define A (x) y\n", 10, 15),
        ] {
            let tree = parser.parse(text, None).expect("the parse ends");
            let root = tree.root_node();
            let sexp = root.to_sexp();
            assert!(!root.has_error(), "{text:?}: {sexp}");
            let mut cursor = root.walk();
            let node = root
                .child(0)
                .expect("the tree has a child")
                .children(&mut cursor)
                .find(|child| child.kind() == "preproc_arg")
                .expect("the directive has a text node");
            assert_eq!(node.start_byte(), start, "{text:?}: {sexp}");
            assert_eq!(node.end_byte(), end, "{text:?}: {sexp}");
        }
    }
}
