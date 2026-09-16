//! Make a flat copy of our syntax tree, and give its nodes the shared categories.
//!
//! The ranges of Clang and of tree-sitter follow different rules. The facts of our tree use the rules
//! of Clang where the two differ:
//!
//! - A declaration or a statement ends before its `;`. A `DeclStmt` and an expression statement keep it.
//! - A declaration with more than one declarator gives one range for each declarator, from the start of the
//!   declaration to the end of the declarator. The attributes after a declarator are not in its range.
//! - A declaration outside a class starts after its C++ attributes, `alignas`, and `__extension__`. A member
//!   of a class keeps its attributes. A GNU attribute stays in the range.
//! - `= default` and `= delete` are in the range of a member or a friend, but not of other functions.
//! - A `case` label ends at the end of its first statement. An empty `case` label contains the next label.
//! - A friend function starts at `friend`. An explicit specialization and a member of a class template that
//!   is defined out of its class start at `template`.
//! - A function declaration with a trailing return type also gets a range that ends at its name.
//! - A signed number is one token of our grammar, but a unary operator and a literal for Clang.
//! - An unnamed variadic parameter gets two ranges: with its `...` for a pack, and without it for a C variadic
//!   parameter. Name lookup selects the correct range.
//!
//! A variable with parenthesized arguments that all start with a type keyword, and an expression statement
//! that starts with a type keyword, get the detail [`TYPE_FIRST`]. The standard reads these forms as
//! declarations with no name lookup, so they are not ambiguous.

use std::ops::ControlFlow;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tree_sitter::{Language, Node, ParseOptions, ParseState, Parser, Tree};

use super::compare::Fact;
use super::label::{Category, Label, detail};

/// The index of no node.
pub const NONE: u32 = u32::MAX;

/// The C++ language of the fork, for the life of the process.
///
/// A `Node` gives the name of its kind and of its field with the life of its tree. The names are in the
/// tables of the parser, and this language gives the same names with the life of the process. A flat tree
/// then stays after its `Tree`.
fn language() -> &'static Language {
    static LANGUAGE: OnceLock<Language> = OnceLock::new();
    LANGUAGE.get_or_init(|| Language::new(tree_sitter_cpp::LANGUAGE))
}

/// The detail of a form that the standard reads as a declaration with no name lookup: [dcl.ambig.res] and [stmt.ambig].
pub const TYPE_FIRST: &str = "type-first";

/// A node of the flat tree.
#[derive(Clone, Debug)]
pub struct OurNode {
    pub kind: &'static str,
    pub named: bool,
    pub extra: bool,
    pub error: bool,
    pub missing: bool,
    pub field: Option<&'static str>,
    pub start: u32,
    pub end: u32,
    pub parent: u32,
    pub first_child: u32,
    pub next_sibling: u32,
}

/// A flat copy of a syntax tree in pre-order. A parent has a smaller index than its children.
pub struct OurTree {
    pub nodes: Vec<OurNode>,
    /// The end of each node without a trailing `;`, as Clang gives the end of a declaration or a statement.
    pub norm_end: Vec<u32>,
    /// The number of ERROR and MISSING nodes.
    pub errors: u32,
    /// The index of the first ERROR or MISSING node.
    pub first_error: u32,
}

/// An iterator over the children of a node.
pub struct Children<'a> {
    tree: &'a OurTree,
    next: u32,
}

impl Iterator for Children<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.next == NONE {
            return None;
        }
        let index = self.next;
        self.next = self.tree.nodes[index as usize].next_sibling;
        Some(index)
    }
}

/// True for the declarations that keep their `;` in the range of a Clang statement.
fn is_declaration_kind(kind: &str) -> bool {
    matches!(
        kind,
        "declaration"
            | "alias_declaration"
            | "type_definition"
            | "using_declaration"
            | "static_assert_declaration"
            | "namespace_alias_definition"
    )
}

/// True for the statements whose last child is a statement.
fn is_statement_parent(kind: &str) -> bool {
    matches!(
        kind,
        "labeled_statement"
            | "case_statement"
            | "attributed_statement"
            | "else_clause"
            | "if_statement"
            | "while_statement"
            | "for_statement"
            | "for_range_loop"
    )
}

/// Parse `source` with a time limit and with the memory ceiling of `corpus`. `None` when one of
/// the two stops the parse. `label` names the source in the message of the cap of the runtime.
pub fn parse(parser: &mut Parser, source: &[u8], label: &str, limit: Duration) -> Option<Tree> {
    let _label = crate::allocation::Label::new(label);
    crate::allocation::reset();
    let ceiling = crate::corpus::ceiling(source.len());
    let started = Instant::now();
    let mut stop = |_: &ParseState| {
        if started.elapsed() > limit || crate::allocation::peak() > ceiling {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let tree = parser.parse_with_options(
        &mut |at, _| source.get(at..).unwrap_or_default(),
        None,
        Some(ParseOptions::new().progress_callback(&mut stop)),
    );
    if tree.is_none() {
        parser.reset();
    }
    tree
}

impl OurTree {
    /// Copy a syntax tree. O(n) in the number of nodes.
    pub fn new(tree: &Tree) -> OurTree {
        let mut nodes: Vec<OurNode> = Vec::new();
        let mut errors = 0;
        let mut first_error = NONE;
        let mut cursor = tree.walk();
        let mut ancestors: Vec<u32> = Vec::new();
        let mut last_child: Vec<u32> = vec![NONE];
        loop {
            let node: Node = cursor.node();
            let index = nodes.len() as u32;
            let parent = ancestors.last().copied().unwrap_or(NONE);
            if node.is_error() || node.is_missing() {
                errors += 1;
                if first_error == NONE {
                    first_error = index;
                }
            }
            nodes.push(OurNode {
                kind: language().node_kind_for_id(node.kind_id()).unwrap_or("?"),
                named: node.is_named(),
                extra: node.is_extra(),
                error: node.is_error(),
                missing: node.is_missing(),
                field: cursor.field_id().and_then(|id| language().field_name_for_id(id.get())),
                start: node.start_byte() as u32,
                end: node.end_byte() as u32,
                parent,
                first_child: NONE,
                next_sibling: NONE,
            });
            if parent != NONE {
                let previous = last_child.last_mut().expect("each level has a slot");
                if *previous == NONE {
                    nodes[parent as usize].first_child = index;
                } else {
                    nodes[*previous as usize].next_sibling = index;
                }
                *previous = index;
            }
            if cursor.goto_first_child() {
                ancestors.push(index);
                last_child.push(NONE);
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    let mut out = OurTree {
                        nodes,
                        norm_end: Vec::new(),
                        errors,
                        first_error,
                    };
                    out.norm_end = out.normalized_ends();
                    return out;
                }
                ancestors.pop();
                last_child.pop();
            }
        }
    }

    /// The end of each node without a trailing `;`. O(n) in the number of nodes.
    ///
    /// A statement that ends with a declaration keeps the `;` of the declaration, as a Clang `DeclStmt` does.
    fn normalized_ends(&self) -> Vec<u32> {
        let mut ends = vec![0; self.nodes.len()];
        for index in (0..self.nodes.len()).rev() {
            let (mut last, mut before) = (NONE, NONE);
            for child in self.children(index as u32).filter(|&c| !self.nodes[c as usize].extra) {
                before = last;
                last = child;
            }
            let node = &self.nodes[index];
            ends[index] = if last == NONE {
                node.end
            } else {
                let child = &self.nodes[last as usize];
                if !child.named && child.kind == ";" && before != NONE {
                    ends[before as usize]
                } else if child.named && is_statement_parent(node.kind) && is_declaration_kind(child.kind) {
                    child.end
                } else if child.named {
                    ends[last as usize]
                } else {
                    node.end
                }
            };
        }
        ends
    }

    /// The node at an index.
    pub fn node(&self, index: u32) -> &OurNode {
        &self.nodes[index as usize]
    }

    /// The kind of the node at an index.
    pub fn kind(&self, index: u32) -> &'static str {
        self.nodes[index as usize].kind
    }

    /// The children of a node, with the extra nodes.
    pub fn children(&self, index: u32) -> Children<'_> {
        Children {
            tree: self,
            next: self.nodes[index as usize].first_child,
        }
    }

    /// The children of a node that are not extra nodes.
    fn solid(&self, index: u32) -> impl Iterator<Item = u32> + '_ {
        self.children(index).filter(|&c| !self.nodes[c as usize].extra)
    }

    /// The parent of a node.
    pub fn parent(&self, index: u32) -> Option<u32> {
        let parent = self.nodes[index as usize].parent;
        (parent != NONE).then_some(parent)
    }

    /// The first child with a field name.
    fn field(&self, index: u32, name: &str) -> Option<u32> {
        self.children(index)
            .find(|&c| self.nodes[c as usize].field == Some(name))
    }

    /// The next sibling that is not an extra node.
    fn next_solid(&self, index: u32) -> Option<u32> {
        let mut next = self.nodes[index as usize].next_sibling;
        while next != NONE && self.nodes[next as usize].extra {
            next = self.nodes[next as usize].next_sibling;
        }
        (next != NONE).then_some(next)
    }

    /// The previous sibling that is not an extra node.
    fn previous_solid(&self, index: u32) -> Option<u32> {
        let parent = self.parent(index)?;
        let mut previous = None;
        for child in self.solid(parent) {
            if child == index {
                return previous;
            }
            previous = Some(child);
        }
        None
    }

    /// The text of a node.
    fn text<'s>(&self, index: u32, source: &'s [u8]) -> &'s [u8] {
        let node = &self.nodes[index as usize];
        source.get(node.start as usize..node.end as usize).unwrap_or_default()
    }

    /// The smallest node that contains a byte range. O(d w) for the depth d and the width w of the tree.
    pub fn cover(&self, begin: u32, end: u32) -> u32 {
        if self.nodes.is_empty() {
            return NONE;
        }
        let mut current = 0;
        loop {
            let inner = self.children(current).find(|&c| {
                let node = &self.nodes[c as usize];
                node.start <= begin && end <= node.end
            });
            match inner {
                Some(child) => current = child,
                None => return current,
            }
        }
    }

    /// True when a declaration is a member of a class, directly, in a template declaration, or as a friend.
    fn is_member(&self, index: u32) -> bool {
        let mut parent = self.parent(index);
        while let Some(current) = parent {
            match self.kind(current) {
                "field_declaration_list" => return true,
                "template_declaration" | "friend_declaration" => parent = self.parent(current),
                _ => return false,
            }
        }
        false
    }

    /// True when a leading child of a declaration is not in the Clang range of the declaration.
    fn outside_range(&self, child: u32, member: bool, source: &[u8]) -> bool {
        let text = self.text(child, source);
        match self.kind(child) {
            "attribute_macro" => true,
            "attribute_declaration" => !member,
            "type_qualifier" => {
                text == b"__extension__" || (!member && self.solid(child).any(|c| self.kind(c) == "alignas_qualifier"))
            }
            _ => text == b"__extension__",
        }
    }

    /// The start of a declaration as Clang gives it. Refer to the rules of the module.
    fn declaration_start(&self, index: u32, source: &[u8]) -> u32 {
        if let Some(parent) = self.parent(index)
            && self.kind(parent) == "friend_declaration"
        {
            return self.nodes[parent as usize].start;
        }
        let member = self.is_member(index);
        self.solid(index)
            .find(|&c| !self.outside_range(c, member, source))
            .map_or(self.nodes[index as usize].start, |c| self.nodes[c as usize].start)
    }

    /// The start of a declaration statement: after empty attribute macros and `__extension__`.
    fn statement_start(&self, index: u32, source: &[u8]) -> u32 {
        self.solid(index)
            .find(|&c| self.kind(c) != "attribute_macro" && self.text(c, source) != b"__extension__")
            .map_or(self.nodes[index as usize].start, |c| self.nodes[c as usize].start)
    }

    /// The end of a statement as Clang gives it: a declaration keeps its `;`, and a `case` label has its own rule.
    fn statement_end(&self, index: u32) -> u32 {
        let kind = self.kind(index);
        if is_declaration_kind(kind) {
            self.nodes[index as usize].end
        } else if kind == "case_statement" {
            self.case_end(index)
        } else {
            self.norm_end[index as usize]
        }
    }

    /// The end of a variadic parameter with no name, before its `...`, or `None` for other parameters.
    ///
    /// The `...` of a pack of `auto` stays in the range, because it makes a pack and not a C variadic parameter.
    fn unnamed_variadic_end(&self, index: u32) -> Option<u32> {
        let children: Vec<u32> = self.solid(index).collect();
        let (&last, rest) = children.split_last()?;
        let node = &self.nodes[last as usize];
        if !node.named && node.kind == "..." {
            return rest.last().map(|&previous| self.norm_end[previous as usize]);
        }
        if self
            .field(index, "type")
            .is_some_and(|t| self.kind(t) == "placeholder_type_specifier")
        {
            return None;
        }
        let mut current = self.field(index, "declarator")?;
        while self.kind(current) != "variadic_declarator" {
            current = self.inner_declarator(current)?;
        }
        if self.solid(current).any(|c| self.nodes[c as usize].named) {
            return None;
        }
        self.previous_solid(current)
            .map(|previous| self.nodes[previous as usize].end)
            .or_else(|| {
                self.parent(current)
                    .and_then(|p| self.previous_solid(p))
                    .map(|p| self.norm_end[p as usize])
            })
    }

    /// The end of a node without the GNU and C++ attributes at its end.
    fn end_before_attributes(&self, index: u32) -> u32 {
        let children: Vec<u32> = self.solid(index).collect();
        let mut kept = children.len();
        while kept > 0
            && matches!(
                self.kind(children[kept - 1]),
                "attribute_specifier" | "attribute_declaration"
            )
        {
            kept -= 1;
        }
        if kept == children.len() || kept == 0 {
            self.norm_end[index as usize]
        } else {
            self.norm_end[children[kept - 1] as usize]
        }
    }

    /// The declarator inside a declarator, or `None` at the name.
    fn inner_declarator(&self, index: u32) -> Option<u32> {
        self.field(index, "declarator").or_else(|| {
            self.solid(index)
                .filter(|&c| {
                    let node = &self.nodes[c as usize];
                    node.named
                        && !matches!(
                            node.kind,
                            "type_qualifier"
                                | "attribute_declaration"
                                | "ms_pointer_modifier"
                                | "ms_based_modifier"
                                | "parameter_list"
                                | "argument_list"
                                | "initializer_list"
                                | "virtual_specifier"
                                | "noexcept"
                                | "throw_specifier"
                                | "trailing_return_type"
                                | "requires_clause"
                        )
                })
                .last()
        })
    }

    /// The name of a declarator and the declarator operator that binds to the name first.
    fn declarator_name(&self, declarator: u32) -> (u32, Option<&'static str>) {
        let mut current = declarator;
        let mut operator = None;
        for _ in 0..512 {
            let kind = self.kind(current);
            match kind {
                "init_declarator" | "attributed_declarator" | "parenthesized_declarator" => {}
                "function_declarator"
                | "pointer_declarator"
                | "reference_declarator"
                | "array_declarator"
                | "member_pointer_declarator"
                | "block_pointer_declarator"
                | "pointer_type_declarator" => operator = Some(kind),
                "operator_cast" => return (current, Some("function_declarator")),
                _ => return (current, operator),
            }
            match self.inner_declarator(current) {
                Some(inner) => current = inner,
                None => return (current, operator),
            }
        }
        (current, operator)
    }

    /// True when a declarator declares a function, and not a pointer or a reference to a function.
    fn is_function(&self, declarator: u32) -> bool {
        self.declarator_name(declarator).1 == Some("function_declarator")
    }

    /// The end of the name of a function declarator with a trailing return type, or `None`.
    ///
    /// Clang ends some declarations with a dependent trailing return type at the name of the function.
    fn trailing_return_name_end(&self, declarator: u32) -> Option<u32> {
        let mut current = declarator;
        while self.kind(current) != "function_declarator" {
            current = self.inner_declarator(current)?;
        }
        if !self.solid(current).any(|c| self.kind(c) == "trailing_return_type") {
            return None;
        }
        let name = self.declarator_name(current).0;
        Some(self.nodes[name as usize].end)
    }

    /// The end of a declarator without the attributes that follow its name or its parameters.
    ///
    /// Clang keeps a GNU attribute after the parameters of a function in the range, but not after a variable.
    fn declarator_end(&self, index: u32) -> u32 {
        let mut current = index;
        while self.kind(current) == "attributed_declarator" {
            match self.solid(current).find(|&c| self.kind(c) != "attribute_declaration") {
                Some(inner) => current = inner,
                None => break,
            }
        }
        if !self.is_function(current) {
            return self.end_before_attributes(current);
        }
        let children: Vec<u32> = self.solid(current).collect();
        let kept = children
            .iter()
            .rposition(|&c| self.kind(c) != "attribute_declaration")
            .map_or(0, |position| position + 1);
        if kept == children.len() || kept == 0 {
            self.norm_end[current as usize]
        } else {
            self.norm_end[children[kept - 1] as usize]
        }
    }

    /// True when the first token of a node is a type keyword: a primitive type, a sized type, or `decltype`.
    fn starts_with_type_keyword(&self, mut index: u32) -> bool {
        loop {
            match self.kind(index) {
                "primitive_type" | "sized_type_specifier" | "decltype" => return true,
                _ => match self.solid(index).next() {
                    Some(first) => index = first,
                    None => return false,
                },
            }
        }
    }

    /// True when a declarator has parenthesized arguments, and each argument starts with a type keyword.
    fn arguments_start_with_types(&self, declarator: u32) -> bool {
        if self.kind(declarator) != "init_declarator" {
            return false;
        }
        let Some(arguments) = self
            .field(declarator, "value")
            .filter(|&v| self.kind(v) == "argument_list")
        else {
            return false;
        };
        let mut named = self
            .solid(arguments)
            .filter(|&c| self.nodes[c as usize].named)
            .peekable();
        named.peek().is_some() && named.all(|argument| self.starts_with_type_keyword(argument))
    }

    /// The last name of a qualified name. A dependent name gives the template-id inside it.
    fn terminal(&self, mut index: u32) -> u32 {
        while self.kind(index) == "qualified_identifier" {
            match self.field(index, "name") {
                Some(name) => index = name,
                None => break,
            }
        }
        if self.kind(index) == "dependent_name"
            && let Some(inner) = self.solid(index).find(|&c| self.nodes[c as usize].named)
        {
            return inner;
        }
        index
    }

    /// True when a name is a template-id, or a qualified name with a template-id in its scope.
    fn has_template_id(&self, mut index: u32) -> bool {
        loop {
            match self.kind(index) {
                "template_type" | "template_function" => return true,
                "qualified_identifier" => {
                    if self
                        .field(index, "scope")
                        .is_some_and(|s| self.kind(s) == "template_type")
                    {
                        return true;
                    }
                    match self.field(index, "name") {
                        Some(name) => index = name,
                        None => return false,
                    }
                }
                _ => return false,
            }
        }
    }

    /// True when a node is in the position of a statement.
    fn in_statement(&self, index: u32) -> bool {
        let Some(parent) = self.parent(index) else {
            return false;
        };
        match self.kind(parent) {
            "compound_statement"
            | "case_statement"
            | "labeled_statement"
            | "attributed_statement"
            | "init_statement"
            | "else_clause" => true,
            "if_statement" | "while_statement" | "do_statement" | "for_statement" | "for_range_loop"
            | "switch_statement" => matches!(
                self.nodes[index as usize].field,
                Some("consequence" | "body" | "initializer")
            ),
            "condition_clause" => self.nodes[index as usize].field == Some("value"),
            // The items of an `__if_exists` block are items of the context of the block.
            "ms_if_exists" => self.in_statement(parent),
            _ => false,
        }
    }

    /// The end of a `case` label as Clang gives it. An empty label contains the next label.
    fn case_end(&self, index: u32) -> u32 {
        let mut current = index;
        loop {
            let mut colon_end = self.nodes[current as usize].end;
            let mut after_colon = false;
            let mut first = None;
            for child in self.solid(current) {
                let node = &self.nodes[child as usize];
                if after_colon && node.named {
                    first = Some(child);
                    break;
                }
                if !node.named && node.kind == ":" {
                    after_colon = true;
                    colon_end = node.end;
                }
            }
            if let Some(first) = first {
                return if is_declaration_kind(self.kind(first)) {
                    self.nodes[first as usize].end
                } else {
                    self.norm_end[first as usize]
                };
            }
            match self.next_solid(current) {
                Some(next) if self.kind(next) == "case_statement" => current = next,
                _ => return colon_end,
            }
        }
    }

    /// The declaration inside nested template declarations: the node after the template parameter lists.
    fn templated(&self, index: u32) -> Option<u32> {
        let mut current = index;
        loop {
            let inner = self
                .solid(current)
                .filter(|&c| {
                    self.nodes[c as usize].named
                        && !matches!(self.kind(c), "template_parameter_list" | "requires_clause")
                })
                .last()?;
            if self.kind(inner) != "template_declaration" {
                return Some(inner);
            }
            current = inner;
        }
    }
}

/// The builder of the facts of our tree.
struct Facts<'a> {
    tree: &'a OurTree,
    source: &'a [u8],
    out: Vec<Fact>,
}

/// The facts of our tree. O(n) in the number of nodes, plus the depth of the declarators.
pub fn facts(tree: &OurTree, source: &[u8]) -> Vec<Fact> {
    let mut builder = Facts {
        tree,
        source,
        out: Vec::with_capacity(tree.nodes.len() / 2),
    };
    for index in 0..tree.nodes.len() as u32 {
        let node = tree.node(index);
        if node.named && !node.extra && !node.error {
            builder.node(index);
        }
    }
    builder.out
}

impl Facts<'_> {
    /// Add a fact with no detail.
    fn push(&mut self, begin: u32, end: u32, category: Category, node: u32) {
        self.out.push(Fact {
            begin,
            end,
            label: Label::new(category),
            node,
        });
    }

    /// Add a fact with a detail.
    fn push_detail(&mut self, begin: u32, end: u32, category: Category, text: &str, node: u32) {
        self.out.push(Fact {
            begin,
            end,
            label: Label::with(category, text),
            node,
        });
    }

    /// Add a fact with the detail [`TYPE_FIRST`] when `type_first` is true, and with no detail otherwise.
    fn push_form(&mut self, begin: u32, end: u32, category: Category, type_first: bool, node: u32) {
        self.out.push(Fact {
            begin,
            end,
            label: Label {
                category,
                detail: if type_first { TYPE_FIRST } else { "" },
            },
            node,
        });
    }

    /// The static detail of the `operator` field of a node.
    fn operator(&self, index: u32) -> &'static str {
        self.tree.field(index, "operator").map_or("?", |o| {
            detail(std::str::from_utf8(self.tree.text(o, self.source)).unwrap_or("?"))
        })
    }

    /// Add the facts of one node.
    fn node(&mut self, index: u32) {
        use Category as C;
        let tree = self.tree;
        let source = self.source;
        let node = tree.node(index);
        let (start, end, norm) = (node.start, node.end, tree.norm_end[index as usize]);
        let parent_kind = tree.parent(index).map(|p| tree.kind(p));
        match node.kind {
            "function_definition" => {
                let end = self.function_end(index);
                self.push(tree.declaration_start(index, source), end, C::FunctionDefinition, index);
            }
            "declaration" | "field_declaration" | "type_definition" => {
                self.statement_declaration(index);
                self.declarators(index);
            }
            // Clang reads a GNU label declaration as a `DeclStmt` of `LabelDecl` nodes.
            "label_declaration" => self.statement_declaration(index),
            "alias_declaration" | "static_assert_declaration" | "namespace_alias_definition" => {
                self.statement_declaration(index);
                let category = match node.kind {
                    "alias_declaration" => C::TypedefAlias,
                    "static_assert_declaration" => C::StaticAssert,
                    _ => C::NamespaceAlias,
                };
                self.push(tree.declaration_start(index, source), norm, category, index);
            }
            "using_declaration" => {
                self.statement_declaration(index);
                let children: Vec<u32> = tree.solid(index).filter(|&c| tree.kind(c) != ";").collect();
                let end = match children.as_slice() {
                    [.., before, last] if tree.kind(*last) == "..." => tree.norm_end[*before as usize],
                    _ => norm,
                };
                self.push(tree.declaration_start(index, source), end, C::Using, index);
            }
            "class_specifier" | "struct_specifier" | "union_specifier" | "interface_specifier" | "enum_specifier" => {
                let category = if node.kind == "enum_specifier" {
                    C::Enum
                } else {
                    C::Class
                };
                let body_end = tree.field(index, "body").map_or(norm, |b| tree.node(b).end);
                self.push(start, body_end, category, index);
                if tree.in_statement(index)
                    && let Some(next) = tree.next_solid(index)
                    && tree.kind(next) == ";"
                {
                    self.push(start, tree.node(next).end, C::DeclarationStatement, index);
                }
            }
            "enumerator" => self.push(start, tree.end_before_attributes(index), C::Enumerator, index),
            "namespace_definition" => self.push(start, norm, C::Namespace, index),
            "friend_declaration" => self.friend(index),
            "linkage_specification" => self.push(start, norm, C::Linkage, index),
            "access_specifier" => {
                let mut next = tree.next_solid(index);
                while let Some(sibling) = next
                    && tree.kind(sibling) == "attribute_specifier"
                {
                    next = tree.next_solid(sibling);
                }
                let colon = next.filter(|&n| tree.kind(n) == ":");
                self.push(
                    start,
                    colon.map_or(end, |c| tree.node(c).end),
                    C::AccessSpecifier,
                    index,
                );
            }
            "export_declaration" => self.push(start, norm, C::Export, index),
            "import_declaration" => self.push(start, norm, C::Import, index),
            "template_declaration" => self.template(index),
            "template_instantiation" => self.push(start, norm, C::ExplicitInstantiation, index),
            "parameter_declaration"
            | "optional_parameter_declaration"
            | "variadic_parameter_declaration"
            | "explicit_object_parameter_declaration" => {
                let grandparent = tree.parent(index).and_then(|p| tree.parent(p)).map(|g| tree.kind(g));
                let category = match (parent_kind, grandparent) {
                    (Some("template_parameter_list"), _) => C::TemplateParameter,
                    (Some("parameter_list"), Some("catch_clause")) => C::Variable,
                    _ => C::Parameter,
                };
                let declarator = tree.field(index, "declarator");
                let full = match declarator {
                    Some(d) if tree.solid(index).last() == Some(d) => tree.declarator_end(d),
                    _ => tree.end_before_attributes(index),
                };
                let begin = tree.declaration_start(index, source);
                if let Some(short) = tree.unnamed_variadic_end(index) {
                    self.push(begin, short, category, index);
                }
                self.push(begin, full, category, index);
            }
            "type_parameter_declaration"
            | "optional_type_parameter_declaration"
            | "variadic_type_parameter_declaration"
            | "template_template_parameter_declaration"
            | "variable_template_parameter_declaration"
            | "concept_parameter_declaration" => {
                let end = tree.unnamed_variadic_end(index).unwrap_or(norm);
                self.push(start, end, C::TemplateParameter, index);
            }
            "compound_statement" => self.push(start, end, C::Compound, index),
            "if_statement" => self.push(start, norm, C::If, index),
            "for_statement" => self.push(start, norm, C::For, index),
            "for_range_loop" => {
                self.push(start, norm, C::RangeFor, index);
                self.loop_variable(index);
            }
            "while_statement" => self.push(start, norm, C::While, index),
            "do_statement" => self.push(start, norm, C::Do, index),
            "switch_statement" => self.push(start, norm, C::Switch, index),
            "case_statement" => self.push(start, tree.case_end(index), C::Case, index),
            "return_statement" => self.push(start, norm, C::Return, index),
            "break_statement" => self.push(start, norm, C::Break, index),
            "continue_statement" => self.push(start, norm, C::Continue, index),
            "goto_statement" => self.push(start, norm, C::Goto, index),
            "labeled_statement" => self.push(start, norm, C::Label, index),
            "try_statement" => self.push(start, norm, C::Try, index),
            "catch_clause" => self.push(start, norm, C::Catch, index),
            "co_return_statement" => self.push(start, norm, C::CoReturn, index),
            "attributed_statement" => {
                let statement = tree
                    .solid(index)
                    .filter(|&c| tree.node(c).named && tree.kind(c) != "attribute_declaration")
                    .last();
                let end = statement.map_or(norm, |s| tree.statement_end(s));
                self.push(start, end, C::AttributedStatement, index);
            }
            "expression_statement" => match tree.solid(index).find(|&c| tree.node(c).named) {
                Some(expression) => {
                    let type_first = tree.starts_with_type_keyword(expression);
                    self.push_form(start, end, C::ExpressionStatement, type_first, index);
                }
                None => self.push(start, end, C::NullStatement, index),
            },
            "throw_statement" => {
                self.push(start, norm, C::Throw, index);
                self.push(start, end, C::ExpressionStatement, index);
            }
            "co_yield_statement" => {
                self.push(start, norm, C::CoYield, index);
                self.push(start, end, C::ExpressionStatement, index);
            }
            "requires_clause" => {
                let open = tree.solid(index).find(|&c| tree.kind(c) == "(");
                let close = tree.solid(index).filter(|&c| tree.kind(c) == ")").last();
                if let (Some(open), Some(close)) = (open, close) {
                    self.push(tree.node(open).start, tree.node(close).end, C::Paren, index);
                }
            }
            "call_expression" => self.call(index),
            "field_expression" => match self.operator(index) {
                operator @ (".*" | "->*") => self.push_detail(start, end, C::Binary, operator, index),
                _ => self.push(start, end, C::MemberAccess, index),
            },
            "binary_expression" => {
                let operator = self.operator(index);
                self.push_detail(start, end, C::Binary, operator, index);
            }
            "assignment_expression" => {
                let operator = self.operator(index);
                self.push_detail(start, end, C::Assignment, operator, index);
            }
            "comma_expression" => self.push_detail(start, end, C::Binary, ",", index),
            "constraint_conjunction" => self.push_detail(start, end, C::Binary, "&&", index),
            "constraint_disjunction" => self.push_detail(start, end, C::Binary, "||", index),
            "unary_expression" | "pointer_expression" => {
                let operator = self.operator(index);
                self.push_detail(start, end, C::Unary, operator, index);
            }
            "update_expression" => {
                let operator = tree.field(index, "operator");
                let prefix = match (operator, tree.field(index, "argument")) {
                    (Some(o), Some(a)) => tree.node(o).start < tree.node(a).start,
                    _ => false,
                };
                let text = match (operator.map(|o| tree.text(o, source)), prefix) {
                    (Some(b"++"), true) => "++x",
                    (Some(b"++"), false) => "x++",
                    (Some(b"--"), true) => "--x",
                    (Some(b"--"), false) => "x--",
                    _ => "?",
                };
                self.push_detail(start, end, C::Unary, text, index);
            }
            "extension_expression" => self.push_detail(start, end, C::Unary, "__extension__", index),
            "co_await_expression" => self.push(start, end, C::CoAwait, index),
            "conditional_expression" => self.push(start, end, C::Conditional, index),
            "cast_expression" => self.push_detail(start, end, C::Cast, "c-style", index),
            "compound_literal_expression" => {
                let parenthesized = tree.solid(index).next().is_some_and(|c| tree.kind(c) == "(");
                let category = if parenthesized {
                    C::CompoundLiteral
                } else {
                    C::FunctionalCast
                };
                self.push(start, end, category, index);
            }
            "new_expression" => self.push(start, end, C::New, index),
            "delete_expression" => self.push(start, end, C::Delete, index),
            "lambda_expression" => self.push(start, end, C::Lambda, index),
            "subscript_expression" => self.push(start, end, C::Subscript, index),
            "sizeof_expression" => {
                self.push(start, end, C::Sizeof, index);
                self.type_operand(index, true);
            }
            "alignof_expression" => {
                self.push(start, end, C::Alignof, index);
                self.type_operand(index, true);
            }
            "typeid_expression" => {
                self.push(start, end, C::Typeid, index);
                self.type_operand(index, false);
            }
            "type_trait_expression" => self.push(start, end, C::TypeTrait, index),
            // A template argument that is only a name is a type or a value. Name lookup selects the category.
            "type_descriptor" if parent_kind == Some("template_argument_list") => {
                self.push(start, end, C::TypeOperand, index);
            }
            "noexcept_expression" => self.push(start, end, C::Noexcept, index),
            "throw_expression" => self.push(start, end, C::Throw, index),
            "number_literal" | "char_literal" | "string_literal" | "raw_string_literal" => {
                if parent_kind == Some("gnu_asm_expression") {
                    self.push(start, end, C::Attribute, index);
                }
                if !matches!(parent_kind, Some("user_defined_literal" | "concatenated_string")) {
                    let kind = match node.kind {
                        "number_literal" => "number",
                        "char_literal" => "char",
                        _ => "string",
                    };
                    let sign = source.get(start as usize).filter(|b| matches!(b, b'-' | b'+'));
                    match sign {
                        Some(&sign) if node.kind == "number_literal" => {
                            let operator = if sign == b'-' { "-" } else { "+" };
                            self.push_detail(start, end, C::Unary, operator, index);
                            self.push_detail(start + 1, end, C::Literal, kind, index);
                        }
                        _ => self.push_detail(start, end, C::Literal, kind, index),
                    }
                }
            }
            "concatenated_string" => self.push_detail(start, end, C::Literal, "string", index),
            "true" | "false" => self.push_detail(start, end, C::Literal, "bool", index),
            "null" => self.push_detail(start, end, C::Literal, "nullptr", index),
            "user_defined_literal" => self.push_detail(start, end, C::Literal, "user-defined", index),
            "initializer_list" => self.push(start, end, C::InitList, index),
            "initializer_pair" => self.push(start, end, C::DesignatedInit, index),
            "template_function" | "template_method" | "template_type" => self.push(start, end, C::TemplateId, index),
            "identifier" | "operator_name" | "destructor_name" => {
                if self.in_attribute(index) {
                    self.push(start, end, C::Attribute, index);
                }
                self.push(start, end, C::Name, index);
            }
            "qualified_identifier" => {
                let category = match tree.kind(tree.terminal(index)) {
                    "template_function" | "template_type" | "template_method" => C::TemplateId,
                    "type_identifier" => C::Type,
                    _ => C::Name,
                };
                self.push(start, end, category, index);
            }
            "type_identifier" => self.push(start, end, C::Type, index),
            "type_qualifier" if tree.text(index, source) == b"constinit" => {
                self.push(start, end, C::Attribute, index);
            }
            "this" => self.push(start, end, C::This, index),
            "parenthesized_expression" => {
                let statement = tree.solid(index).any(|c| tree.kind(c) == "compound_statement");
                let category = if statement { C::StatementExpression } else { C::Paren };
                self.push(start, end, category, index);
            }
            "fold_expression" => self.push(start, end, C::Fold, index),
            "parameter_pack_expansion" => self.push(start, end, C::PackExpansion, index),
            "pack_index_expression" => self.push(start, end, C::PackIndex, index),
            "requires_expression" => self.push(start, end, C::Requires, index),
            "co_yield_expression" => self.push(start, end, C::CoYield, index),
            "offsetof_expression" => self.push(start, end, C::Offsetof, index),
            "attribute" | "virtual_specifier" | "alignas_qualifier" => self.push(start, end, C::Attribute, index),
            _ => {}
        }
    }

    /// True when a node is a GNU or Microsoft attribute: `__attribute__((name))` or `__declspec(name)`.
    fn in_attribute(&self, index: u32) -> bool {
        let tree = self.tree;
        let Some(parent) = tree.parent(index) else {
            return false;
        };
        match tree.kind(parent) {
            "ms_declspec_modifier" => true,
            "argument_list" => tree
                .parent(parent)
                .is_some_and(|g| tree.kind(g) == "attribute_specifier"),
            _ => false,
        }
    }

    /// The end of a function definition. Refer to the rules of the module for `= default` and `= delete`.
    fn function_end(&self, index: u32) -> u32 {
        let tree = self.tree;
        let norm = tree.norm_end[index as usize];
        let Some(clause) = tree
            .solid(index)
            .find(|&c| matches!(tree.kind(c), "default_method_clause" | "delete_method_clause"))
        else {
            return norm;
        };
        if tree.is_member(index) {
            tree.children(clause)
                .find(|&c| matches!(tree.kind(c), "default" | "delete"))
                .map_or(norm, |keyword| tree.node(keyword).end)
        } else {
            tree.field(index, "declarator")
                .map_or(norm, |d| tree.norm_end[d as usize])
        }
    }

    /// Add a declaration statement when a declaration is in the position of a statement.
    fn statement_declaration(&mut self, index: u32) {
        let tree = self.tree;
        if tree.in_statement(index) {
            let start = tree.statement_start(index, self.source);
            self.push(start, tree.node(index).end, Category::DeclarationStatement, index);
        }
    }

    /// Add one fact for each declarator of a declaration, from the start of the declaration to the declarator end.
    ///
    /// An unnamed bit-field is a declarator. The attributes after a declarator are not in its range, and an
    /// assembler label is in the range of a function but not of a variable.
    fn declarators(&mut self, index: u32) {
        let tree = self.tree;
        let kind = tree.kind(index);
        let start = tree.declaration_start(index, self.source);
        let static_member = kind == "field_declaration"
            && tree
                .solid(index)
                .any(|c| tree.kind(c) == "storage_class_specifier" && tree.text(c, self.source) == b"static");
        let mut group: Option<(u32, u32)> = None;
        let mut groups = Vec::new();
        for child in tree.solid(index) {
            let node = tree.node(child);
            let assembler = node.kind == "gnu_asm_expression" && group.is_some();
            if node.field == Some("declarator") && !assembler {
                groups.extend(group.take());
                group = Some((child, child));
            } else if node.kind == "bitfield_clause" && group.is_none() {
                group = Some((child, child));
            } else if !node.named && (node.kind == "," || node.kind == ";") {
                groups.extend(group.take());
            } else if let Some((_, last)) = group.as_mut()
                && !matches!(node.kind, "attribute_declaration" | "attribute_specifier")
            {
                *last = child;
            }
        }
        groups.extend(group);
        for (declarator, last) in groups {
            let function = tree.is_function(declarator);
            let category = match kind {
                "type_definition" => Category::TypedefAlias,
                "field_declaration" if function => Category::FunctionDeclaration,
                "field_declaration" if static_member => Category::Variable,
                "field_declaration" => Category::Field,
                _ if function => Category::FunctionDeclaration,
                _ => Category::Variable,
            };
            let end = if tree.kind(last) == "gnu_asm_expression" && !function {
                tree.declarator_end(declarator)
            } else {
                tree.declarator_end(last)
            };
            let type_first = category == Category::Variable && tree.arguments_start_with_types(declarator);
            self.push_form(start, end, category, type_first, declarator);
            if function && let Some(name_end) = tree.trailing_return_name_end(declarator) {
                self.push(start, name_end, category, declarator);
            }
        }
    }

    /// Add the facts of a friend declaration: one for each type of a variadic friend, and the classes that it declares.
    fn friend(&mut self, index: u32) {
        let tree = self.tree;
        let start = tree.node(index).start;
        let children: Vec<u32> = tree.solid(index).collect();
        let mut previous = None;
        for (position, &child) in children.iter().enumerate() {
            let node = tree.node(child);
            if !node.named && node.kind == "," {
                if let Some(previous) = previous {
                    self.push(start, tree.norm_end[previous as usize], Category::Friend, index);
                }
                continue;
            }
            if !node.named
                && matches!(node.kind, "class" | "struct" | "union")
                && let Some(&name) = children.get(position + 1)
                && tree.node(name).named
            {
                self.push(node.start, tree.node(name).end, Category::Class, index);
            }
            if node.kind != ";" {
                previous = Some(child);
            }
        }
        self.push(start, self.friend_end(index), Category::Friend, index);
    }

    /// The end of a friend declaration: a friend function with `= default` or `= delete` has its own rule.
    fn friend_end(&self, index: u32) -> u32 {
        let tree = self.tree;
        tree.solid(index)
            .find(|&c| tree.kind(c) == "function_definition")
            .map_or(tree.norm_end[index as usize], |function| self.function_end(function))
    }

    /// Add the facts of a template declaration.
    ///
    /// Clang starts an explicit specialization, a friend template, and a member of a class template that is
    /// defined out of its class, at the first `template`. For these, the declaration also gets its own category
    /// at the range of the template.
    fn template(&mut self, index: u32) {
        let tree = self.tree;
        let (start, norm) = (tree.node(index).start, tree.norm_end[index as usize]);
        let Some(inner) = tree.templated(index) else {
            self.push(start, norm, Category::Template, index);
            return;
        };
        // `template<>` with no parameters starts an explicit specialization, also when its name has no arguments.
        let specialization = tree
            .field(index, "parameters")
            .is_some_and(|p| !tree.solid(p).any(|c| tree.node(c).named));
        let concept = tree.kind(inner) == "concept_definition";
        let end = match tree.kind(inner) {
            "function_definition" => self.function_end(inner),
            "friend_declaration" => self.friend_end(inner),
            _ => norm,
        };
        self.push(
            start,
            end,
            if concept { Category::Concept } else { Category::Template },
            index,
        );
        let entity = match tree.kind(inner) {
            "function_definition" => tree
                .field(inner, "declarator")
                .filter(|&d| specialization || tree.has_template_id(tree.declarator_name(d).0))
                .map(|_| (Category::FunctionDefinition, end)),
            "declaration" | "field_declaration" => tree.field(inner, "declarator").and_then(|d| {
                if let Some(name_end) = tree.trailing_return_name_end(d) {
                    self.out.push(Fact {
                        begin: start,
                        end: name_end,
                        label: Label::new(Category::Template),
                        node: index,
                    });
                }
                (specialization || tree.has_template_id(tree.declarator_name(d).0)).then(|| {
                    let category = if tree.is_function(d) {
                        Category::FunctionDeclaration
                    } else {
                        Category::Variable
                    };
                    (category, norm)
                })
            }),
            "class_specifier" | "struct_specifier" | "union_specifier" | "interface_specifier" => {
                tree.field(inner, "name").filter(|&n| tree.has_template_id(n)).map(|_| {
                    (
                        Category::Class,
                        tree.field(inner, "body").map_or(norm, |b| tree.node(b).end),
                    )
                })
            }
            "friend_declaration" => Some((Category::Friend, end)),
            _ => None,
        };
        if let Some((category, end)) = entity {
            self.push(start, end, category, inner);
        }
    }

    /// Add the variable of a range `for`, from the first token after `(` to the end of the declarator.
    fn loop_variable(&mut self, index: u32) {
        let tree = self.tree;
        let Some(declarator) = tree.field(index, "declarator") else {
            return;
        };
        let mut after_paren = false;
        for child in tree.solid(index) {
            let node = tree.node(child);
            if !node.named && node.kind == "(" {
                after_paren = true;
            } else if after_paren && node.field != Some("initializer") {
                self.push(
                    node.start,
                    tree.norm_end[declarator as usize],
                    Category::Variable,
                    declarator,
                );
                return;
            }
        }
    }

    /// Add the facts of a call: a cast when the callee is a type or a named cast.
    fn call(&mut self, index: u32) {
        let tree = self.tree;
        let (start, end) = (tree.node(index).start, tree.node(index).end);
        let function = tree.field(index, "function");
        let callee = function.map(|f| tree.kind(f));
        match callee {
            Some(
                "primitive_type"
                | "sized_type_specifier"
                | "decltype"
                | "placeholder_type_specifier"
                | "type_identifier"
                | "template_type"
                // A functional cast to a dependent type: `typename A::X()`, `typename T::type()`.
                // The grammar gives the callee this kind. Refer to `call_expression` in
                // grammar/src/cpp.rs.
                | "dependent_type",
            ) => self.push(start, end, Category::FunctionalCast, index),
            Some("template_function") => {
                let name = function
                    .and_then(|f| tree.field(f, "name"))
                    .map(|n| tree.text(n, self.source));
                match name {
                    Some(
                        name @ (b"static_cast" | b"dynamic_cast" | b"reinterpret_cast" | b"const_cast"
                        | b"addrspace_cast"),
                    ) => self.push_detail(
                        start,
                        end,
                        Category::Cast,
                        std::str::from_utf8(name).unwrap_or("?"),
                        index,
                    ),
                    _ => self.push(start, end, Category::Call, index),
                }
            }
            Some("qualified_identifier") => {
                let terminal = function.map(|f| tree.kind(tree.terminal(f)));
                let category = if matches!(terminal, Some("template_type" | "type_identifier")) {
                    Category::FunctionalCast
                } else {
                    Category::Call
                };
                self.push(start, end, category, index);
            }
            _ => self.push(start, end, Category::Call, index),
        }
        if self.in_attribute(index) {
            self.push(start, end, Category::Attribute, index);
        }
    }

    /// Add the type operand of `sizeof`, `alignof`, or `typeid`, and the parentheses around it.
    fn type_operand(&mut self, index: u32, parentheses: bool) {
        let tree = self.tree;
        let Some(operand) = tree.field(index, "type") else {
            return;
        };
        let node = tree.node(operand);
        self.push(node.start, node.end, Category::TypeOperand, operand);
        if !parentheses {
            return;
        }
        let mut open = None;
        let mut close = None;
        for child in tree.solid(index) {
            let child_node = tree.node(child);
            if child_node.end <= node.start && child_node.kind == "(" {
                open = Some(child_node.start);
            }
            if child_node.start >= node.end && child_node.kind == ")" && close.is_none() {
                close = Some(child_node.end);
            }
        }
        if let (Some(open), Some(close)) = (open, close) {
            self.push(open, close, Category::ParenType, operand);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Language;

    /// The facts of a snippet as `text label` strings, in the order of the tree.
    fn labels(source: &str) -> Vec<String> {
        let mut parser = Parser::new();
        parser
            .set_language(&Language::new(tree_sitter_cpp::LANGUAGE))
            .expect("the grammar has a compatible ABI");
        let tree = parser.parse(source, None).expect("a parser with no limit gives a tree");
        let flat = OurTree::new(&tree);
        facts(&flat, source.as_bytes())
            .iter()
            .filter(|f| !matches!(f.label.category, Category::Type | Category::Literal))
            .map(|f| format!("{} {}", &source[f.begin as usize..f.end as usize], f.label))
            .collect()
    }

    /// Assert that each expected `text label` string is in the facts of a snippet.
    fn assert_has(source: &str, expected: &[&str]) {
        let found = labels(source);
        for item in expected {
            assert!(found.contains(&(*item).to_owned()), "`{item}` is not in {found:?}");
        }
    }

    #[test]
    fn each_declarator_gets_a_range_from_the_start_of_the_declaration() {
        assert_has(
            "void f() { int a = 1, *b, c(2); }",
            &[
                "int a = 1, *b, c(2); declaration statement",
                "int a = 1 variable",
                "int a = 1, *b variable",
                "int a = 1, *b, c(2) variable",
            ],
        );
    }

    #[test]
    fn a_field_declaration_includes_its_bit_field_and_a_static_member_is_a_variable() {
        assert_has(
            "struct S { int x : 3; static int y; virtual void f() = 0; int : 0; };",
            &[
                "int x : 3 field",
                "static int y variable",
                "virtual void f() = 0 function declaration",
                "int : 0 field",
            ],
        );
    }

    #[test]
    fn an_empty_case_contains_the_next_case_and_a_statement_ends_before_its_semicolon() {
        assert_has(
            "void f(int v) { switch (v) { case 1: case 2: v++; break; } if (v) return; }",
            &[
                "case 1: case 2: v++ case",
                "case 2: v++ case",
                "if (v) return if",
                "v++; expression statement",
            ],
        );
    }

    #[test]
    fn a_callee_that_is_a_type_or_a_named_cast_gives_a_cast() {
        assert_has(
            "int x = int(1) + static_cast<int>(2) + f(3);",
            &[
                "int(1) functional cast",
                "static_cast<int>(2) cast static_cast",
                "f(3) call",
            ],
        );
    }

    #[test]
    fn a_specialization_and_an_out_of_class_member_also_start_at_template() {
        assert_has(
            "template<> struct S<int> {};\ntemplate<class T> void S<T>::f() {}\n\
             template<class T> template<class U> void S<T>::g(U) {}\nstruct F { template<int> friend void h(); };\n\
             template<> void k(Y) {}",
            &[
                "template<> struct S<int> {} class",
                "template<class T> void S<T>::f() {} function definition",
                "template<class T> template<class U> void S<T>::g(U) {} function definition",
                "template<int> friend void h() friend",
                "template<> void k(Y) {} function definition",
            ],
        );
    }

    #[test]
    fn a_function_declarator_in_parentheses_with_a_pointer_is_a_variable() {
        assert_has(
            "int (*fp)(int); int *g(int);",
            &["int (*fp)(int) variable", "int *g(int) function declaration"],
        );
    }

    #[test]
    fn attributes_start_a_member_but_not_a_declaration_outside_a_class() {
        assert_has(
            "[[nodiscard]] int f(); __attribute__((unused)) int v; int w [[maybe_unused]];\n\
             struct S { [[nodiscard]] int m(); S(int) = delete; friend bool operator==(S, S) = default; };\n\
             void g(int) = delete; void h() [[ , ]]; void k(int x __attribute__((unused)), int y [[maybe_unused]]);\n\
             enum E { A [[deprecated]], B };",
            &[
                "int f() function declaration",
                "__attribute__((unused)) int v variable",
                "int w variable",
                "[[nodiscard]] int m() function declaration",
                "S(int) = delete function definition",
                "friend bool operator==(S, S) = default function definition",
                "void g(int) function definition",
                "void h() function declaration",
                "int x parameter",
                "int y parameter",
                "A enumerator",
            ],
        );
    }

    #[test]
    fn an_unnamed_variadic_parameter_has_a_range_with_and_without_its_ellipsis() {
        assert_has(
            "template<class..., char...> struct U; void h(int...); void p(char const * ...); void q(auto...);",
            &[
                "class template parameter",
                "char template parameter",
                "int parameter",
                "int... parameter",
                "char const * parameter",
                "char const * ... parameter",
                "auto... parameter",
            ],
        );
    }

    #[test]
    fn a_form_that_starts_with_a_type_keyword_is_type_first() {
        assert_has(
            "void f() { int(x) + 1; a(b); int(y); }\nS s(int(a) + 1, char() + 2); T t(a + 1);",
            &[
                "int(x) + 1; expression statement type-first",
                "a(b); expression statement",
                "int(y); declaration statement",
                "S s(int(a) + 1, char() + 2) variable type-first",
                "T t(a + 1) variable",
            ],
        );
    }

    #[test]
    fn a_pointer_to_a_member_function_is_a_variable() {
        assert_has(
            "int (Z::*m)() = &Z::f; void (^b)(int); int (Z::*g())();",
            &[
                "int (Z::*m)() = &Z::f variable",
                "void (^b)(int) variable",
                "int (Z::*g())() function declaration",
            ],
        );
    }

    #[test]
    fn a_gnu_attribute_after_a_function_is_in_its_range_but_not_after_a_variable() {
        assert_has(
            "void c() __attribute__((cold)); int x __attribute__((unused)), y [[maybe_unused]];",
            &[
                "void c() __attribute__((cold)) function declaration",
                "int x variable",
                "int x __attribute__((unused)), y variable",
            ],
        );
    }

    #[test]
    fn operands_and_template_arguments_follow_the_clang_nodes() {
        assert_has(
            "int d = -1; void h() { (this->*func)(); } B<_N, int> x; auto g() -> int;\nint k = __is_trivial(int);",
            &[
                "-1 unary -",
                "func name",
                "_N type operand",
                "auto g function declaration",
                "__is_trivial(int) type trait",
            ],
        );
    }
}
