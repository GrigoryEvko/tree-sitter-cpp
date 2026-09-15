//! Read the Clang JSON AST, keep the nodes of the main file, and give them the shared categories.
//!
//! The dumper is `clang/lib/AST/JSONNodeDumper.cpp`, and the order of the children comes from
//! `clang/include/clang/AST/ASTNodeTraverser.h`. These details of the format are important:
//!
//! - A location omits `file` when its file is the file of the location that the dumper wrote before
//!   it. The reader keeps the current file in document order, in all objects that have `offset`.
//! - `range.end` is the start of the last token, and `tokLen` is the length of that token.
//! - A location in a macro has `spellingLoc` and `expansionLoc` objects and no `offset`.
//! - A location in an included file has an `includedFrom` object. Its `file` is not a location.
//! - The children of a node are in an array after the attributes of the node. The key of the array is
//!   `inner` or the label of the first child. A null child is an empty object.
//!
//! Some nodes are implicit but have no `isImplicit` flag. The labels ignore a name reference whose range
//! does not spell its name, a member call that does not end with `)`, and an instantiation that does not
//! start with `template`.

use std::io::BufRead;

use super::compare::Fact;
use super::json::{JsonError, Lexer, Token};
use super::label::{Category, Label};

/// The index of no node.
pub const NONE: u32 = u32::MAX;

/// The flags of a [`ClangNode`].
pub mod flag {
    /// A member access with an implicit `this`: the source code has only the name.
    pub const IMPLICIT_BASE: u32 = 1;
    /// A postfix increment or decrement.
    pub const POSTFIX: u32 = 1 << 1;
    /// An `if` with an `else` branch.
    pub const HAS_ELSE: u32 = 1 << 2;
    /// An instantiation outside its template.
    pub const INSTANTIATION: u32 = 1 << 3;
    /// The variable of a range `for`.
    pub const RANGE_FOR_VARIABLE: u32 = 1 << 4;
    /// The inner namespace of a nested namespace definition `namespace a::b`.
    pub const NESTED: u32 = 1 << 5;
    /// A function with `= default`.
    pub const DEFAULTED: u32 = 1 << 6;
    /// A function with `= delete`.
    pub const DELETED: u32 = 1 << 7;
    /// A function with a body.
    pub const HAS_BODY: u32 = 1 << 8;
    /// A variable with an initializer in parentheses.
    pub const PAREN_INIT: u32 = 1 << 9;
    /// The begin or the end of the node is in a macro expansion.
    pub const MACRO: u32 = 1 << 10;
}

/// A node of the Clang AST whose begin is in the main file, or in a macro expansion in the main file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClangNode {
    /// The kind, for example `CXXRecordDecl`.
    pub kind: Box<str>,
    /// The operator, the name, or the referenced name of the node.
    pub detail: Box<str>,
    /// The first byte of the node.
    pub begin: u32,
    /// The byte after the last token of the node.
    pub end: u32,
    /// The first byte of the last token of the node.
    pub last: u32,
    /// True when the begin and the end are in the main file and not in a macro expansion.
    pub plain: bool,
    /// The index of the nearest ancestor in the node list, or [`NONE`].
    pub parent: u32,
    /// True when the parent in the node list is the direct parent in the AST.
    pub direct: bool,
    /// The position of the node in the child array of its direct parent.
    pub pos: u32,
    /// The number of elements in the child array of the direct parent.
    pub siblings: u32,
    /// The number of elements in the child arrays of the node.
    pub children: u32,
    /// A set of [`flag`] values.
    pub flags: u32,
}

/// A source location of the dump.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Loc {
    #[default]
    Invalid,
    /// A location in a file.
    File { offset: u32, tok_len: u32, main: bool },
    /// A location in a macro expansion. `main` is true when the expansion is in the main file.
    Macro { main: bool },
}

/// The keys that the reader uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    None,
    Id,
    Kind,
    Loc,
    Range,
    Begin,
    End,
    SpellingLoc,
    ExpansionLoc,
    Offset,
    File,
    TokLen,
    Implicit,
    Inherited,
    Detail,
    Name,
    Postfix,
    Pattern,
    ReferencedDecl,
    HasElse,
    Init,
    Nested,
    Defaulted,
    Deleted,
    /// An array of attribute values, not of children.
    AttributeArray,
    /// The child array of an initializer list whose first child is the implicit array filler.
    ArrayFiller,
    Other,
}

/// The reader key of a JSON key.
fn key(text: &str) -> Key {
    match text {
        "id" => Key::Id,
        "kind" => Key::Kind,
        "loc" => Key::Loc,
        "range" => Key::Range,
        "begin" => Key::Begin,
        "end" => Key::End,
        "spellingLoc" => Key::SpellingLoc,
        "expansionLoc" => Key::ExpansionLoc,
        "offset" => Key::Offset,
        "file" => Key::File,
        "tokLen" => Key::TokLen,
        "isImplicit" | "implicit" => Key::Implicit,
        "inherited" => Key::Inherited,
        "opcode" | "member" => Key::Detail,
        "name" => Key::Name,
        "isPostfix" => Key::Postfix,
        "TemplateInstantiationPattern" => Key::Pattern,
        "referencedDecl" => Key::ReferencedDecl,
        "hasElse" => Key::HasElse,
        "init" => Key::Init,
        "isNested" => Key::Nested,
        "explicitlyDefaulted" => Key::Defaulted,
        "explicitlyDeleted" => Key::Deleted,
        "lookups"
        | "explicitTemplateArgs"
        | "templateArgsAsWritten"
        | "bases"
        | "cleanups"
        | "path"
        | "exceptionTypes"
        | "protocols"
        | "positions"
        | "args"
        | "attrs" => Key::AttributeArray,
        "array_filler" => Key::ArrayFiller,
        _ => Key::Other,
    }
}

/// The kinds of parent nodes that have special rules for their children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Other,
    /// The first child is the operator function, with the range of the operator token.
    OperatorCall,
    /// The children at positions 1 to 6 are the implicit variables and statements of the loop.
    ForRange,
    /// The children after the first templated declaration are specializations.
    TemplateDecl,
    /// A child `ConceptSpecializationExpr` is the implicit constraint of a type constraint.
    TemplateTypeParm,
    /// All the children are implicit.
    Discard,
    /// The children after the first child are implicit calls of the coroutine machinery.
    CoroutinePart,
    /// The children of a defaulted function, other than its parameters, are implicit.
    Function,
}

/// The owner rules of a node kind.
fn owner_of(kind: &str) -> Owner {
    match kind {
        "CXXOperatorCallExpr" => Owner::OperatorCall,
        "CXXForRangeStmt" => Owner::ForRange,
        "ClassTemplateDecl" | "FunctionTemplateDecl" | "VarTemplateDecl" => Owner::TemplateDecl,
        "TemplateTypeParmDecl" => Owner::TemplateTypeParm,
        "BindingDecl" | "UserDefinedLiteral" | "ImplicitConceptSpecializationDecl" | "PredefinedExpr" => Owner::Discard,
        "CoroutineBodyStmt" | "CoreturnStmt" | "CoawaitExpr" | "CoyieldExpr" | "DependentCoawaitExpr" => {
            Owner::CoroutinePart
        }
        "FunctionDecl"
        | "CXXMethodDecl"
        | "CXXConstructorDecl"
        | "CXXDestructorDecl"
        | "CXXConversionDecl"
        | "CXXDeductionGuideDecl" => Owner::Function,
        _ => Owner::Other,
    }
}

/// The role of a node in a range `for`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Role {
    #[default]
    Normal,
    /// The implicit `DeclStmt` of the range variable. The reader keeps its grandchildren.
    RangeStatement,
    /// The implicit range variable. The reader keeps its initializer.
    RangeVariable,
    /// The `DeclStmt` of the loop variable.
    LoopStatement,
    /// The loop variable. The reader keeps the variable, but not its implicit initializer.
    LoopVariable,
}

/// The decision of the reader about a node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
    #[default]
    Undecided,
    /// The node is in the node list at this index.
    Recorded(u32),
    /// The node is not in the list, but its children can be.
    Transparent,
    /// The node is not related to the main file. Its children can be.
    Unrecorded,
    /// The node and all its descendants are not in the list.
    Skipped,
}

/// An open object that can be a node.
struct NodeFrame {
    id: String,
    kind: String,
    detail: String,
    pattern: String,
    begin: Loc,
    end: Loc,
    flags: u32,
    implicit: bool,
    inherited: bool,
    state: State,
    role: Role,
    /// The node and its subtree are not recorded.
    skim: bool,
    /// The children of the node are not recorded.
    children_skim: bool,
    /// For a template declaration: the first templated declaration was a child.
    templated_seen: bool,
    /// The stack index of the node that receives an operator name from this subtree.
    capture: Option<usize>,
    /// The stack index of the parent node frame.
    owner: Option<usize>,
    owner_kind: Owner,
    owner_role: Role,
    owner_flags: u32,
    parent: u32,
    direct: bool,
    pos: u32,
    children: u32,
}

impl NodeFrame {
    /// A frame with no attributes.
    fn new(owner: Option<usize>, pos: u32) -> NodeFrame {
        NodeFrame {
            id: String::new(),
            kind: String::new(),
            detail: String::new(),
            pattern: String::new(),
            begin: Loc::Invalid,
            end: Loc::Invalid,
            flags: 0,
            implicit: false,
            inherited: false,
            state: State::Undecided,
            role: Role::Normal,
            skim: false,
            children_skim: false,
            templated_seen: false,
            capture: None,
            owner,
            owner_kind: Owner::Other,
            owner_role: Role::Normal,
            owner_flags: 0,
            parent: NONE,
            direct: false,
            pos,
            children: 0,
        }
    }
}

/// The target of a location object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Discard,
    Begin,
    End,
    Spelling,
    Expansion,
}

/// An open JSON container.
enum Frame {
    Node(Box<NodeFrame>),
    /// An array. `owner` is the stack index of the node frame when the elements are children.
    Array {
        owner: Option<usize>,
        count: u32,
        recorded: Vec<u32>,
        /// The first element is the implicit array filler of an initializer list.
        filler: bool,
    },
    Range {
        begin: Loc,
        end: Loc,
    },
    Loc {
        target: Target,
        offset: Option<u32>,
        tok_len: u32,
        expansion: Option<Loc>,
    },
    /// Another object. `capture` is the stack index of the node frame that receives a name.
    Other {
        saw_offset: bool,
        capture: Option<usize>,
    },
}

/// The state of the reader.
struct Builder<'a> {
    main: &'a str,
    source: &'a [u8],
    file_is_main: bool,
    nodes: Vec<ClangNode>,
    stack: Vec<Frame>,
    key: Key,
}

/// Read a Clang JSON AST, and keep the nodes of the main file `main` with the bytes `source`.
///
/// The read stops with [`JsonError::TooLarge`] after `limit` bytes. It gives the nodes in document
/// order and the size of the dump. O(n) in the size of the dump.
pub fn read<R: BufRead>(input: R, main: &str, source: &[u8], limit: u64) -> Result<(Vec<ClangNode>, u64), JsonError> {
    let mut lexer = Lexer::new(input, limit);
    let mut builder = Builder {
        main,
        source,
        file_is_main: false,
        nodes: Vec::new(),
        stack: Vec::new(),
        key: Key::None,
    };
    loop {
        match lexer.next_token()? {
            Token::End => break,
            Token::Key => builder.key = key(lexer.text()),
            token => builder.token(token, lexer.text())?,
        }
    }
    Ok((builder.nodes, lexer.bytes()))
}

impl Builder<'_> {
    /// Apply a token that is not a key.
    fn token(&mut self, token: Token, text: &str) -> Result<(), JsonError> {
        let key = std::mem::replace(&mut self.key, Key::None);
        match token {
            Token::BeginObject => self.begin_object(key),
            Token::EndObject => self.end_object()?,
            Token::BeginArray => self.begin_array(key),
            Token::EndArray => self.end_array()?,
            Token::Str => self.string(key, text),
            Token::Number(value) => self.number(key, value),
            Token::Bool(value) => self.boolean(key, value),
            Token::Null | Token::Key | Token::End => self.element(),
        }
        Ok(())
    }

    /// Count a scalar element of an array.
    fn element(&mut self) {
        if let Some(Frame::Array { count, .. }) = self.stack.last_mut() {
            *count += 1;
        }
    }

    /// Open an object. Its frame depends on the container and on the key.
    fn begin_object(&mut self, key: Key) {
        let index = self.stack.len().wrapping_sub(1);
        let other = |capture| Frame::Other {
            saw_offset: false,
            capture,
        };
        let loc = |target| Frame::Loc {
            target,
            offset: None,
            tok_len: 0,
            expansion: None,
        };
        let frame = match self.stack.last_mut() {
            None => Frame::Node(Box::new(NodeFrame::new(None, 0))),
            Some(Frame::Array {
                owner, count, filler, ..
            }) => {
                let pos = *count;
                *count += 1;
                let implicit_filler = *filler && pos == 0;
                match *owner {
                    Some(owner) => {
                        self.push_child(owner, pos, implicit_filler);
                        return;
                    }
                    None => other(None),
                }
            }
            Some(Frame::Node(node)) => match key {
                Key::Range => Frame::Range {
                    begin: Loc::Invalid,
                    end: Loc::Invalid,
                },
                Key::Loc => loc(Target::Discard),
                Key::ReferencedDecl => other(Some(node.capture.unwrap_or(index))),
                _ => other(node.capture),
            },
            Some(Frame::Range { .. }) => match key {
                Key::Begin => loc(Target::Begin),
                Key::End => loc(Target::End),
                _ => other(None),
            },
            Some(Frame::Loc { .. }) => match key {
                Key::SpellingLoc => loc(Target::Spelling),
                Key::ExpansionLoc => loc(Target::Expansion),
                _ => other(None),
            },
            Some(Frame::Other { capture, .. }) => other(*capture),
        };
        self.stack.push(frame);
    }

    /// Open the frame of a child node at position `pos` of the node frame at stack index `owner`.
    fn push_child(&mut self, owner: usize, pos: u32, implicit_filler: bool) {
        let mut frame = NodeFrame::new(Some(owner), pos);
        if let Some(Frame::Node(parent)) = self.stack.get(owner) {
            frame.skim = parent.state == State::Skipped || parent.children_skim || implicit_filler;
            frame.capture = if frame.skim { parent.capture } else { None };
            frame.owner_kind = owner_of(&parent.kind);
            frame.owner_role = parent.role;
            frame.owner_flags = parent.flags;
            (frame.parent, frame.direct) = match parent.state {
                State::Recorded(index) => (index, true),
                _ => (parent.parent, false),
            };
        }
        self.stack.push(Frame::Node(Box::new(frame)));
    }

    /// Close an object, and give its value to the frame that contains it.
    fn end_object(&mut self) -> Result<(), JsonError> {
        let Some(index) = self.stack.len().checked_sub(1) else {
            return Err(JsonError::Syntax("an object closes, but no object is open".into()));
        };
        if matches!(self.stack[index], Frame::Node(_)) {
            self.decide(index);
        }
        match self.stack.pop() {
            Some(Frame::Node(node)) => {
                if let State::Recorded(id) = node.state {
                    let record = &mut self.nodes[id as usize];
                    record.children = node.children;
                    record.flags |= node.flags;
                    if record.detail.is_empty() && !node.detail.is_empty() {
                        record.detail = node.detail.into();
                    }
                }
            }
            Some(Frame::Range { begin, end }) => {
                if let Some(Frame::Node(node)) = self.stack.last_mut() {
                    node.begin = begin;
                    node.end = end;
                }
            }
            Some(Frame::Loc {
                target,
                offset,
                tok_len,
                expansion,
            }) => {
                let value = match (expansion, offset) {
                    (Some(expansion), _) => Loc::Macro {
                        main: matches!(expansion, Loc::File { main: true, .. }),
                    },
                    (None, Some(offset)) => Loc::File {
                        offset,
                        tok_len,
                        main: self.file_is_main,
                    },
                    (None, None) => Loc::Invalid,
                };
                match (self.stack.last_mut(), target) {
                    (Some(Frame::Range { begin, .. }), Target::Begin) => *begin = value,
                    (Some(Frame::Range { end, .. }), Target::End) => *end = value,
                    (Some(Frame::Loc { expansion, .. }), Target::Expansion) => *expansion = Some(value),
                    _ => {}
                }
            }
            Some(Frame::Other { .. }) => {}
            Some(Frame::Array { .. }) | None => {
                return Err(JsonError::Syntax("an object closes in an array".into()));
            }
        }
        Ok(())
    }

    /// Open an array. An array of a node frame holds children, unless its key is an attribute array.
    fn begin_array(&mut self, key: Key) {
        let index = self.stack.len().wrapping_sub(1);
        let owner = match self.stack.last_mut() {
            Some(Frame::Node(_)) if key != Key::AttributeArray => Some(index),
            Some(Frame::Array { count, .. }) => {
                *count += 1;
                None
            }
            _ => None,
        };
        if let Some(owner) = owner {
            self.decide(owner);
        }
        self.stack.push(Frame::Array {
            owner,
            count: 0,
            recorded: Vec::new(),
            filler: key == Key::ArrayFiller,
        });
    }

    /// Close an array, and give the number of its elements to its recorded children and to its owner.
    fn end_array(&mut self) -> Result<(), JsonError> {
        let Some(Frame::Array {
            owner, count, recorded, ..
        }) = self.stack.pop()
        else {
            return Err(JsonError::Syntax("an array closes in an object".into()));
        };
        for id in recorded {
            self.nodes[id as usize].siblings = count;
        }
        if let Some(Some(Frame::Node(node))) = owner.map(|o| self.stack.get_mut(o)) {
            node.children += count;
        }
        Ok(())
    }

    /// Apply a string value.
    fn string(&mut self, key: Key, text: &str) {
        let index = self.stack.len().wrapping_sub(1);
        let main = self.main;
        let mut capture = None;
        let mut resolve = false;
        match self.stack.last_mut() {
            Some(Frame::Node(node)) => match key {
                Key::Id => node.id = text.to_owned(),
                Key::Kind => {
                    node.kind = text.to_owned();
                    resolve = true;
                }
                Key::Detail => node.detail = text.to_owned(),
                Key::Name => match node.capture {
                    Some(target) => capture = Some(target),
                    None if node.detail.is_empty() => node.detail = text.to_owned(),
                    None => {}
                },
                Key::Pattern => node.pattern = text.to_owned(),
                Key::Init if matches!(text, "call" | "paren-list") => node.flags |= flag::PAREN_INIT,
                Key::Defaulted => node.flags |= flag::DEFAULTED,
                _ => {}
            },
            Some(Frame::Loc { offset, .. }) => {
                if key == Key::File && offset.is_some() {
                    self.file_is_main = text == main;
                }
            }
            Some(Frame::Other {
                saw_offset,
                capture: target,
            }) => match key {
                Key::File if *saw_offset => self.file_is_main = text == main,
                Key::Name => capture = *target,
                _ => {}
            },
            Some(Frame::Array { count, .. }) => *count += 1,
            Some(Frame::Range { .. }) | None => {}
        }
        if resolve {
            self.resolve(index);
        }
        if let Some(target) = capture {
            self.set_detail(target, text);
        }
    }

    /// Apply a number value.
    fn number(&mut self, key: Key, value: Option<u64>) {
        let small = value.and_then(|v| u32::try_from(v).ok());
        match self.stack.last_mut() {
            Some(Frame::Loc { offset, tok_len, .. }) => match key {
                Key::Offset => *offset = small,
                Key::TokLen => *tok_len = small.unwrap_or(0),
                _ => {}
            },
            Some(Frame::Other { saw_offset, .. }) if key == Key::Offset => *saw_offset = true,
            Some(Frame::Array { count, .. }) => *count += 1,
            _ => {}
        }
    }

    /// Apply a boolean value.
    fn boolean(&mut self, key: Key, value: bool) {
        match self.stack.last_mut() {
            Some(Frame::Node(node)) if value => match key {
                Key::Implicit => node.implicit = true,
                Key::Inherited => node.inherited = true,
                Key::Postfix => node.flags |= flag::POSTFIX,
                Key::HasElse => node.flags |= flag::HAS_ELSE,
                Key::Nested => node.flags |= flag::NESTED,
                Key::Deleted => node.flags |= flag::DELETED,
                _ => {}
            },
            Some(Frame::Array { count, .. }) => *count += 1,
            _ => {}
        }
    }

    /// Give a name to the detail of the node frame at stack index `target`, if it has no detail.
    ///
    /// An operator call accepts only the name of an operator function.
    fn set_detail(&mut self, target: usize, text: &str) {
        let Some(Frame::Node(node)) = self.stack.get_mut(target) else {
            return;
        };
        if !node.detail.is_empty() || (node.kind == "CXXOperatorCallExpr" && !text.starts_with("operator")) {
            return;
        }
        node.detail = text.to_owned();
        if let State::Recorded(id) = node.state {
            self.nodes[id as usize].detail = text.into();
        }
    }

    /// Apply the rules of the parent to a node frame when its kind is known.
    fn resolve(&mut self, index: usize) {
        let (before, rest) = self.stack.split_at_mut(index);
        let Some(Frame::Node(node)) = rest.first_mut() else {
            return;
        };
        if node.skim {
            return;
        }
        let kind = node.kind.as_str();
        match node.owner_kind {
            Owner::OperatorCall if node.pos == 0 => {
                node.skim = true;
                node.capture = node.owner;
            }
            Owner::ForRange => match node.pos {
                1 => node.role = Role::RangeStatement,
                2..=5 => node.skim = true,
                6 => node.role = Role::LoopStatement,
                _ => {}
            },
            Owner::TemplateDecl
                if kind.ends_with("Decl")
                    && !matches!(
                        kind,
                        "TemplateTypeParmDecl" | "NonTypeTemplateParmDecl" | "TemplateTemplateParmDecl"
                    ) =>
            {
                if let Some(Frame::Node(owner)) = node.owner.and_then(|o| before.get_mut(o)) {
                    if owner.templated_seen || kind.ends_with("SpecializationDecl") {
                        node.skim = true;
                    } else {
                        owner.templated_seen = true;
                    }
                }
            }
            Owner::TemplateTypeParm if kind == "ConceptSpecializationExpr" => node.skim = true,
            Owner::Discard => node.skim = true,
            Owner::CoroutinePart if node.pos != 0 => node.skim = true,
            Owner::Function if node.owner_flags & flag::DEFAULTED != 0 && kind != "ParmVarDecl" => node.skim = true,
            _ => {}
        }
        match node.owner_role {
            Role::RangeStatement => node.role = Role::RangeVariable,
            Role::LoopStatement => node.role = Role::LoopVariable,
            _ => {}
        }
        if matches!(
            kind,
            "ImplicitConceptSpecializationDecl" | "CXXDefaultArgExpr" | "CXXDefaultInitExpr"
        ) {
            node.skim = true;
        }
        let body = node.owner_kind == Owner::Function
            && node.direct
            && matches!(kind, "CompoundStmt" | "CXXTryStmt" | "CoroutineBodyStmt");
        let parent = node.parent;
        if body {
            self.nodes[parent as usize].flags |= flag::HAS_BODY;
        }
    }

    /// Decide if the node frame at stack index `index` goes into the node list, and add it.
    fn decide(&mut self, index: usize) {
        let (before, rest) = self.stack.split_at_mut(index);
        let Some(Frame::Node(node)) = rest.first_mut() else {
            return;
        };
        if node.state != State::Undecided {
            return;
        }
        if node.skim || node.inherited || (node.implicit && node.role != Role::RangeVariable) {
            node.state = State::Skipped;
            if node.implicit && node.kind == "CXXThisExpr" && node.direct {
                self.nodes[node.parent as usize].flags |= flag::IMPLICIT_BASE;
            }
            return;
        }
        if matches!(
            node.role,
            Role::RangeStatement | Role::RangeVariable | Role::LoopStatement
        ) {
            node.state = State::Transparent;
            return;
        }
        let instantiation = !node.pattern.is_empty() && node.pattern != node.id;
        if instantiation && node.owner_kind == Owner::TemplateDecl {
            node.state = State::Skipped;
            return;
        }
        if !matches!(node.begin, Loc::File { main: true, .. } | Loc::Macro { main: true }) {
            node.state = State::Unrecorded;
            return;
        }
        let mut flags = node.flags;
        if instantiation {
            flags |= flag::INSTANTIATION;
            node.children_skim = true;
        }
        if node.role == Role::LoopVariable {
            flags |= flag::RANGE_FOR_VARIABLE;
            node.children_skim = true;
        }
        let (begin, end, last, plain) = match (node.begin, node.end) {
            (
                Loc::File {
                    offset: begin,
                    main: true,
                    ..
                },
                Loc::File {
                    offset: last,
                    tok_len,
                    main: true,
                },
            ) => {
                // The dumper measures `>>` as one token where it closes two template argument lists.
                let short = tok_len > 1
                    && self.source.get(last as usize) == Some(&b'>')
                    && !node.detail.starts_with("operator");
                let end = last.saturating_add(if short { 1 } else { tok_len });
                if begin <= last && end as usize <= self.source.len() {
                    (begin, end, last, true)
                } else {
                    (begin, begin, begin, false)
                }
            }
            (begin, end) => {
                if matches!(begin, Loc::Macro { .. }) || matches!(end, Loc::Macro { .. }) {
                    flags |= flag::MACRO;
                }
                (0, 0, 0, false)
            }
        };
        let id = self.nodes.len() as u32;
        self.nodes.push(ClangNode {
            kind: node.kind.as_str().into(),
            detail: node.detail.as_str().into(),
            begin,
            end,
            last,
            plain,
            parent: node.parent,
            direct: node.direct,
            pos: node.pos,
            siblings: 0,
            children: 0,
            flags,
        });
        node.state = State::Recorded(id);
        if let Some(Frame::Array { recorded, .. }) = before.last_mut() {
            recorded.push(id);
        }
    }
}

/// True when a node kind is an expression.
fn is_expression(kind: &str) -> bool {
    kind.ends_with("Expr") || kind.ends_with("Operator") || kind.ends_with("Literal") || kind == "ExprWithCleanups"
}

/// The first byte at or after `at` that is not white space, a comment, or a line splice.
pub fn skip_trivia(source: &[u8], mut at: usize) -> usize {
    while at < source.len() {
        match source[at] {
            b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c' => at += 1,
            b'\\' if matches!(source.get(at + 1), Some(b'\n' | b'\r')) => at += 2,
            b'/' if source.get(at + 1) == Some(&b'/') => {
                at = source[at..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map_or(source.len(), |p| at + p);
            }
            b'/' if source.get(at + 1) == Some(&b'*') => {
                at = source[at + 2..]
                    .windows(2)
                    .position(|w| w == b"*/")
                    .map_or(source.len(), |p| at + 2 + p + 2);
            }
            _ => break,
        }
    }
    at
}

/// The end of a balanced parenthesized group that starts at or after `at`, after trivia.
fn close_paren(source: &[u8], at: usize) -> Option<usize> {
    let open = skip_trivia(source, at);
    if source.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0usize;
    for (offset, &byte) in source[open..].iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// True when a text contains a name. An empty name is in all texts.
fn spells(text: &[u8], name: &str) -> bool {
    let name = name.as_bytes();
    name.is_empty() || text.windows(name.len()).any(|window| window == name)
}

/// True when a text is an explicit instantiation: `template` or `extern template` with no `<` after `template`.
fn explicit_instantiation(text: &[u8]) -> bool {
    let text = text.strip_prefix(b"extern").unwrap_or(text).trim_ascii_start();
    text.strip_prefix(b"template")
        .is_some_and(|rest| !rest.trim_ascii_start().starts_with(b"<"))
}

/// The facts of the plain nodes of a Clang node list. O(n) in the number of nodes.
pub fn facts(nodes: &[ClangNode], source: &[u8]) -> Vec<Fact> {
    let mut out = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        if !node.plain {
            continue;
        }
        let index = index as u32;
        if let Some((label, begin, end)) = label(nodes, index, source) {
            out.push(Fact {
                begin,
                end,
                label,
                node: index,
            });
        }
        if let Some(end) = statement_end(nodes, index, source) {
            out.push(Fact {
                begin: node.begin,
                end,
                label: Label::new(Category::ExpressionStatement),
                node: index,
            });
        }
    }
    out
}

/// The end of the expression statement of an expression in the position of a statement.
///
/// Clang has no node for an expression statement. The statement ends after the `;` that follows the expression.
fn statement_end(nodes: &[ClangNode], index: u32, source: &[u8]) -> Option<u32> {
    let node = &nodes[index as usize];
    if !is_expression(&node.kind) || !node.direct || node.parent == NONE {
        return None;
    }
    let parent = &nodes[node.parent as usize];
    let last = node.pos + 1 == node.siblings;
    let slot = match &*parent.kind {
        "CompoundStmt" => true,
        "IfStmt" => last || (parent.flags & flag::HAS_ELSE != 0 && node.pos + 2 == node.siblings),
        "WhileStmt" | "ForStmt" | "CXXForRangeStmt" | "CaseStmt" | "DefaultStmt" | "LabelStmt" | "AttributedStmt"
        | "SwitchStmt" => last,
        "DoStmt" => node.pos == 0,
        _ => false,
    };
    if !slot {
        return None;
    }
    let at = skip_trivia(source, node.end as usize);
    (source.get(at) == Some(&b';')).then_some(at as u32 + 1)
}

/// The label of a name reference, or `None` when the range does not spell the name.
///
/// A name reference is a template-id when its range ends with `>` after the name.
fn name_label(node: &ClangNode, text: &[u8]) -> Option<Label> {
    if !spells(text, &node.detail) {
        return None;
    }
    let template = text.last() == Some(&b'>') && (node.detail.is_empty() || !text.ends_with(node.detail.as_bytes()));
    Some(Label::new(if template { Category::TemplateId } else { Category::Name }))
}

/// The label of an overloaded operator call, from the name of the operator function.
fn operator_label(node: &ClangNode) -> Option<Label> {
    let operator = node.detail.strip_prefix("operator")?.trim();
    let operands = node.children.saturating_sub(1);
    Some(match operator {
        "()" => Label::new(Category::Call),
        "[]" => Label::new(Category::Subscript),
        "->" | "" | "new" | "delete" | "new[]" | "delete[]" | "co_await" => return None,
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "^=" | "&=" | "|=" | "<<=" | ">>=" => {
            Label::with(Category::Assignment, operator)
        }
        "++" | "--" => {
            let postfix = operands >= 2;
            let text = match (operator, postfix) {
                ("++", false) => "++x",
                ("++", true) => "x++",
                ("--", false) => "--x",
                _ => "x--",
            };
            Label::with(Category::Unary, text)
        }
        _ if operands == 1 => Label::with(Category::Unary, operator),
        _ => Label::with(Category::Binary, operator),
    })
}

/// The label and the byte range of a plain Clang node, or `None` for a kind with no category.
fn label(nodes: &[ClangNode], index: u32, source: &[u8]) -> Option<(Label, u32, u32)> {
    use Category as C;
    let node = &nodes[index as usize];
    let mut begin = node.begin;
    let mut end = node.end;
    // A token after a line splice starts at the backslash of the splice.
    if source.get(begin as usize) == Some(&b'\\') {
        begin = (skip_trivia(source, begin as usize) as u32).min(end);
    }
    let text = &source[begin as usize..end as usize];
    let flags = node.flags;
    let instantiation = flags & flag::INSTANTIATION != 0;
    if instantiation && !explicit_instantiation(text) {
        // An implicit instantiation outside its template has the range of its pattern.
        return None;
    }
    let label = match &*node.kind {
        "FunctionDecl" | "CXXMethodDecl" | "CXXConstructorDecl" | "CXXDestructorDecl" | "CXXConversionDecl" => {
            if instantiation {
                Label::new(C::ExplicitInstantiation)
            } else if flags & (flag::HAS_BODY | flag::DEFAULTED | flag::DELETED) != 0 {
                Label::new(C::FunctionDefinition)
            } else {
                Label::new(C::FunctionDeclaration)
            }
        }
        "CXXDeductionGuideDecl" => Label::new(C::FunctionDeclaration),
        "VarDecl" | "DecompositionDecl" | "VarTemplateSpecializationDecl" | "VarTemplatePartialSpecializationDecl" => {
            if instantiation {
                Label::new(C::ExplicitInstantiation)
            } else {
                if flags & flag::RANGE_FOR_VARIABLE != 0 {
                    end = node.last;
                    while end > begin && source[end as usize - 1].is_ascii_whitespace() {
                        end -= 1;
                    }
                }
                if flags & flag::PAREN_INIT != 0 {
                    let at = skip_trivia(source, end as usize);
                    if source.get(at) == Some(&b')') {
                        end = at as u32 + 1;
                    }
                }
                Label::new(C::Variable)
            }
        }
        "ParmVarDecl" => Label::new(C::Parameter),
        "FieldDecl" => Label::new(C::Field),
        "CXXRecordDecl"
        | "RecordDecl"
        | "ClassTemplateSpecializationDecl"
        | "ClassTemplatePartialSpecializationDecl" => Label::new(if instantiation {
            C::ExplicitInstantiation
        } else {
            C::Class
        }),
        "EnumDecl" => Label::new(C::Enum),
        "EnumConstantDecl" => Label::new(C::Enumerator),
        "NamespaceDecl" if flags & flag::NESTED != 0 => return None,
        "NamespaceDecl" => Label::new(C::Namespace),
        "NamespaceAliasDecl" => Label::new(C::NamespaceAlias),
        "TypedefDecl" | "TypeAliasDecl" => Label::new(C::TypedefAlias),
        // An abbreviated function template has no `template` keyword and the range of its function.
        "ClassTemplateDecl" | "FunctionTemplateDecl" | "VarTemplateDecl" | "TypeAliasTemplateDecl" => {
            if !text.starts_with(b"template") {
                return None;
            }
            Label::new(C::Template)
        }
        "ConceptDecl" => Label::new(C::Concept),
        // An unnamed template template parameter ends at the token after `class`, where the name can be.
        "TemplateTemplateParmDecl" => {
            if matches!(text.last(), Some(b',' | b'>')) {
                let head = source[begin as usize..node.last as usize].trim_ascii_end();
                if head.ends_with(b"class") || head.ends_with(b"typename") || head.ends_with(b"...") {
                    end = begin + head.len() as u32;
                }
            }
            Label::new(C::TemplateParameter)
        }
        "TemplateTypeParmDecl" | "NonTypeTemplateParmDecl" => Label::new(C::TemplateParameter),
        "StaticAssertDecl" => Label::new(C::StaticAssert),
        "UsingDecl"
        | "UsingDirectiveDecl"
        | "UsingEnumDecl"
        | "UsingPackDecl"
        | "UnresolvedUsingValueDecl"
        | "UnresolvedUsingTypenameDecl" => Label::new(C::Using),
        "FriendDecl" | "FriendTemplateDecl" => Label::new(C::Friend),
        "LinkageSpecDecl" => Label::new(C::Linkage),
        "AccessSpecDecl" => Label::new(C::AccessSpecifier),
        "ExportDecl" => Label::new(C::Export),
        "ImportDecl" => Label::new(C::Import),
        "ExplicitInstantiationDecl" => Label::new(C::ExplicitInstantiation),
        "DeclStmt" => {
            // Clang ends the `DeclStmt` of an alias declaration in the first clause of `for` one token after its `;`.
            if !text.ends_with(b";") {
                let mut at = node.last as usize;
                while at > begin as usize && source[at - 1].is_ascii_whitespace() {
                    at -= 1;
                }
                if at > begin as usize && source[at - 1] == b';' {
                    end = at as u32;
                }
            }
            Label::new(C::DeclarationStatement)
        }
        // Clang reads the block of a dependent `__if_exists` as a compound statement, but the block makes no
        // scope (ParseStmt.cpp, ParseMicrosoftIfExistsStatement). The tree has no node for its braces.
        "CompoundStmt"
            if node.direct
                && nodes
                    .get(node.parent as usize)
                    .is_some_and(|p| &*p.kind == "MSDependentExistsStmt") =>
        {
            return None;
        }
        "CompoundStmt" => Label::new(C::Compound),
        "IfStmt" => Label::new(C::If),
        "ForStmt" => Label::new(C::For),
        "CXXForRangeStmt" => Label::new(C::RangeFor),
        "WhileStmt" => Label::new(C::While),
        "DoStmt" => Label::new(C::Do),
        "SwitchStmt" => Label::new(C::Switch),
        "CaseStmt" | "DefaultStmt" => Label::new(C::Case),
        "ReturnStmt" => Label::new(C::Return),
        "BreakStmt" => Label::new(C::Break),
        "ContinueStmt" => Label::new(C::Continue),
        "GotoStmt" | "IndirectGotoStmt" => Label::new(C::Goto),
        "LabelStmt" => Label::new(C::Label),
        "CXXTryStmt" => Label::new(C::Try),
        "CXXCatchStmt" => Label::new(C::Catch),
        "CoreturnStmt" => Label::new(C::CoReturn),
        // A label at the end of a block gets an implicit `NullStmt` at its colon.
        "NullStmt" if text != b";" => return None,
        "NullStmt" => {
            // A macro that expands to a full statement leaves the `;` after it as an empty statement.
            let in_block = node.direct && &*nodes[node.parent as usize].kind == "CompoundStmt";
            let previous = source[..begin as usize].trim_ascii_end().last();
            if in_block && !previous.is_none_or(|b| matches!(b, b';' | b'{' | b'}' | b':')) {
                return None;
            }
            Label::new(C::NullStatement)
        }
        "AttributedStmt" if text.starts_with(b"#") => return None,
        "AttributedStmt" => Label::new(C::AttributedStatement),
        // A call of a conversion function has the range of its operand and no parentheses.
        "CXXMemberCallExpr" if !text.ends_with(b")") => return None,
        "CallExpr" | "CXXMemberCallExpr" | "CUDAKernelCallExpr" => Label::new(C::Call),
        "CXXOperatorCallExpr" => operator_label(node)?,
        "MemberExpr" if flags & flag::IMPLICIT_BASE != 0 => name_label(node, text)?,
        "CXXDependentScopeMemberExpr" | "UnresolvedMemberExpr" if node.children == 0 => name_label(node, text)?,
        "MemberExpr" | "CXXDependentScopeMemberExpr" | "UnresolvedMemberExpr" if !spells(text, &node.detail) => {
            return None;
        }
        "MemberExpr" | "CXXDependentScopeMemberExpr" | "UnresolvedMemberExpr" | "CXXPseudoDestructorExpr" => {
            Label::new(C::MemberAccess)
        }
        "BinaryOperator" => match &*node.detail {
            "=" => Label::with(C::Assignment, "="),
            detail => Label::with(C::Binary, detail),
        },
        "CompoundAssignOperator" => Label::with(C::Assignment, &node.detail),
        // The capture `[*this]` has an implicit dereference with the range of the `*` token.
        "UnaryOperator" if end - begin <= 1 => return None,
        "UnaryOperator" => match (&*node.detail, flags & flag::POSTFIX != 0) {
            ("++", false) => Label::with(C::Unary, "++x"),
            ("++", true) => Label::with(C::Unary, "x++"),
            ("--", false) => Label::with(C::Unary, "--x"),
            ("--", true) => Label::with(C::Unary, "x--"),
            ("co_await", _) => Label::new(C::CoAwait),
            (detail, _) => Label::with(C::Unary, detail),
        },
        "ConditionalOperator" | "BinaryConditionalOperator" => Label::new(C::Conditional),
        "CStyleCastExpr" => Label::with(C::Cast, "c-style"),
        "CXXStaticCastExpr" => Label::with(C::Cast, "static_cast"),
        "CXXDynamicCastExpr" => Label::with(C::Cast, "dynamic_cast"),
        "CXXReinterpretCastExpr" => Label::with(C::Cast, "reinterpret_cast"),
        "CXXConstCastExpr" => Label::with(C::Cast, "const_cast"),
        "CXXAddrspaceCastExpr" => Label::with(C::Cast, "addrspace_cast"),
        "CXXFunctionalCastExpr"
        | "CXXTemporaryObjectExpr"
        | "CXXUnresolvedConstructExpr"
        | "CXXScalarValueInitExpr" => Label::new(C::FunctionalCast),
        "CompoundLiteralExpr" => Label::new(C::CompoundLiteral),
        "CXXNewExpr" => Label::new(C::New),
        "CXXDeleteExpr" => Label::new(C::Delete),
        "LambdaExpr" => Label::new(C::Lambda),
        "ArraySubscriptExpr" | "MatrixSubscriptExpr" => Label::new(C::Subscript),
        "UnaryExprOrTypeTraitExpr" => match &*node.detail {
            "sizeof" => Label::new(C::Sizeof),
            "alignof" | "__alignof" | "_Alignof" => Label::new(C::Alignof),
            _ => return None,
        },
        "SizeOfPackExpr" => Label::new(C::Sizeof),
        "CXXThrowExpr" => Label::new(C::Throw),
        "IntegerLiteral" | "FloatingLiteral" | "FixedPointLiteral" | "ImaginaryLiteral" => {
            Label::with(C::Literal, "number")
        }
        "CharacterLiteral" => Label::with(C::Literal, "char"),
        "StringLiteral" => Label::with(C::Literal, "string"),
        "CXXBoolLiteralExpr" => Label::with(C::Literal, "bool"),
        "CXXNullPtrLiteralExpr" => Label::with(C::Literal, "nullptr"),
        "UserDefinedLiteral" => Label::with(C::Literal, "user-defined"),
        // An implicit initializer list, for example an array filler, has the range of the `}` token.
        "InitListExpr" if !text.starts_with(b"{") => return None,
        "InitListExpr" => Label::new(C::InitList),
        "DesignatedInitExpr" => Label::new(C::DesignatedInit),
        "DeclRefExpr" | "UnresolvedLookupExpr" | "DependentScopeDeclRefExpr" => name_label(node, text)?,
        "PredefinedExpr" => Label::new(C::Name),
        // The constraint of `C auto x` or of `template<C T>` has the range of the concept name only.
        "ConceptSpecializationExpr" if !text.ends_with(b">") => return None,
        "ConceptSpecializationExpr" => Label::new(C::TemplateId),
        "CXXThisExpr" if text != b"this" => return None,
        "CXXThisExpr" => Label::new(C::This),
        "ParenExpr" => Label::new(C::Paren),
        "CXXTypeidExpr" => Label::new(C::Typeid),
        "CXXNoexceptExpr" => Label::new(C::Noexcept),
        "CXXFoldExpr" => Label::new(C::Fold),
        "PackExpansionExpr" => Label::new(C::PackExpansion),
        "PackIndexingExpr" => Label::new(C::PackIndex),
        "RequiresExpr" => Label::new(C::Requires),
        "CoawaitExpr" | "DependentCoawaitExpr" => Label::new(C::CoAwait),
        "CoyieldExpr" => Label::new(C::CoYield),
        "StmtExpr" => Label::new(C::StatementExpression),
        "TypeTraitExpr" | "ArrayTypeTraitExpr" | "ExpressionTraitExpr" => Label::new(C::TypeTrait),
        "OffsetOfExpr" => Label::new(C::Offsetof),
        "AlignedAttr" => {
            if text.starts_with(b"alignas") || text.starts_with(b"_Alignas") {
                end = close_paren(source, end as usize).map_or(end, |close| close as u32);
            }
            Label::new(C::Attribute)
        }
        kind if kind.ends_with("Attr") => Label::new(C::Attribute),
        _ => return None,
    };
    Some((label, begin, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The nodes of a JSON fixture for the main file `/m.cc` with the bytes `source`.
    fn nodes(json: &str, source: &str) -> Vec<ClangNode> {
        read(json.as_bytes(), "/m.cc", source.as_bytes(), u64::MAX)
            .expect("the fixture is valid")
            .0
    }

    /// The kind, begin, end, and plain flag of each node.
    fn summary(nodes: &[ClangNode]) -> Vec<(String, u32, u32, bool)> {
        nodes
            .iter()
            .map(|n| (n.kind.to_string(), n.begin, n.end, n.plain))
            .collect()
    }

    /// The facts of a node list as `label begin..end` strings.
    fn fact_labels(nodes: &[ClangNode], source: &str) -> Vec<String> {
        facts(nodes, source.as_bytes())
            .iter()
            .map(|f| format!("{} {}..{}", f.label, f.begin, f.end))
            .collect()
    }

    #[test]
    fn a_location_with_no_file_is_in_the_file_of_the_previous_location() {
        let source = "int a; int b;";
        let json = r#"{"id":"0x1","kind":"TranslationUnitDecl","loc":{},"range":{"begin":{},"end":{}},"inner":[
            {"id":"0x2","kind":"VarDecl","loc":{"offset":4,"file":"/h.h","line":1,"col":5,"tokLen":1,
               "includedFrom":{"file":"/m.cc"}},
             "range":{"begin":{"offset":0,"col":1,"tokLen":3},"end":{"offset":4,"col":5,"tokLen":1}}},
            {"id":"0x3","kind":"VarDecl","loc":{"offset":4,"file":"/m.cc","line":1,"col":5,"tokLen":1},
             "range":{"begin":{"offset":0,"col":1,"tokLen":3},"end":{"offset":4,"col":5,"tokLen":1}}},
            {"id":"0x4","kind":"VarDecl","loc":{"offset":11,"col":12,"tokLen":1},
             "range":{"begin":{"offset":7,"col":8,"tokLen":3},"end":{"offset":11,"col":12,"tokLen":1}}}
        ]}"#;
        let found = nodes(json, source);
        assert_eq!(
            summary(&found),
            [("VarDecl".to_owned(), 0, 5, true), ("VarDecl".to_owned(), 7, 12, true)]
        );
    }

    #[test]
    fn the_byte_range_ends_after_the_last_token_and_a_macro_location_is_not_plain() {
        let source = "x = M(a) + bb;";
        let json = r#"{"id":"0x1","kind":"CompoundStmt","range":{"begin":{"offset":0,"file":"/m.cc","tokLen":1},"end":{"offset":11,"tokLen":2}},"inner":[
            {"id":"0x2","kind":"BinaryOperator","opcode":"+","range":{
                "begin":{"spellingLoc":{"offset":6,"tokLen":1},"expansionLoc":{"offset":4,"tokLen":1,"isMacroArgExpansion":true}},
                "end":{"offset":11,"tokLen":2}}}
        ]}"#;
        let found = nodes(json, source);
        assert_eq!(
            summary(&found),
            [
                ("CompoundStmt".to_owned(), 0, 13, true),
                ("BinaryOperator".to_owned(), 0, 0, false)
            ]
        );
        assert_eq!(found[1].flags & flag::MACRO, flag::MACRO);
        assert_eq!(&*found[1].detail, "+");
    }

    #[test]
    fn implicit_subtrees_and_specializations_in_a_template_are_not_kept() {
        let source = "template<class T> struct S { T t; };";
        let json = r#"{"id":"0x1","kind":"ClassTemplateDecl","range":{"begin":{"offset":0,"file":"/m.cc","tokLen":8},"end":{"offset":34,"tokLen":1}},"inner":[
            {"id":"0x2","kind":"CXXRecordDecl","range":{"begin":{"offset":18,"tokLen":6},"end":{"offset":34,"tokLen":1}},"inner":[
                {"id":"0x3","kind":"CXXConstructorDecl","isImplicit":true,"range":{"begin":{"offset":25,"tokLen":1},"end":{"offset":25,"tokLen":1}},"inner":[
                    {"id":"0x4","kind":"ParmVarDecl","range":{"begin":{"offset":25,"tokLen":1},"end":{"offset":25,"tokLen":1}}}]}]},
            {"id":"0x5","kind":"ClassTemplateSpecializationDecl","TemplateInstantiationPattern":"0x2","range":{"begin":{"offset":0,"tokLen":8},"end":{"offset":34,"tokLen":1}},"inner":[
                {"id":"0x6","kind":"FieldDecl","range":{"begin":{"offset":29,"tokLen":1},"end":{"offset":31,"tokLen":1}}}]},
            {"id":"0x7","kind":"ClassTemplateSpecializationDecl","range":{"begin":{"offset":0,"tokLen":8},"end":{"offset":34,"tokLen":1}}}
        ]}"#;
        let kinds: Vec<String> = nodes(json, source).iter().map(|n| n.kind.to_string()).collect();
        assert_eq!(kinds, ["ClassTemplateDecl", "CXXRecordDecl"]);
    }

    #[test]
    fn an_operator_call_takes_the_name_of_its_callee_and_counts_its_operands() {
        let source = "v++;";
        let json = r#"{"id":"0x1","kind":"CXXOperatorCallExpr","range":{"begin":{"offset":0,"file":"/m.cc","tokLen":1},"end":{"offset":1,"tokLen":2}},"inner":[
            {"id":"0x2","kind":"ImplicitCastExpr","range":{"begin":{"offset":1,"tokLen":2},"end":{"offset":1,"tokLen":2}},"inner":[
                {"id":"0x3","kind":"DeclRefExpr","range":{"begin":{"offset":1,"tokLen":2},"end":{"offset":1,"tokLen":2}},
                 "referencedDecl":{"id":"0x9","kind":"CXXMethodDecl","name":"operator++"}}]},
            {"id":"0x4","kind":"DeclRefExpr","range":{"begin":{"offset":0,"tokLen":1},"end":{"offset":0,"tokLen":1}},
             "referencedDecl":{"id":"0x8","kind":"ParmVarDecl","name":"v"}},
            {"id":"0x5","kind":"IntegerLiteral","range":{"begin":{},"end":{}}}
        ]}"#;
        let found = nodes(json, source);
        assert_eq!(found.len(), 2);
        assert_eq!(&*found[0].detail, "operator++");
        assert_eq!(found[0].children, 3);
        assert_eq!(&*found[1].detail, "v");
        // The call has no parent statement in this fixture, so it has no expression statement.
        assert_eq!(fact_labels(&found, source), ["unary x++ 0..3", "name 0..1"]);
    }

    #[test]
    fn the_loop_variable_of_a_range_for_ends_before_the_colon() {
        let source = "for (int e : a) ;";
        let json = r#"{"id":"0x1","kind":"CXXForRangeStmt","range":{"begin":{"offset":0,"file":"/m.cc","tokLen":3},"end":{"offset":16,"tokLen":1}},"inner":[
            {},
            {"id":"0x2","kind":"DeclStmt","range":{"begin":{"offset":13,"tokLen":1},"end":{"offset":13,"tokLen":1}},"inner":[
                {"id":"0x3","kind":"VarDecl","isImplicit":true,"range":{"begin":{"offset":13,"tokLen":1},"end":{"offset":13,"tokLen":1}},"inner":[
                    {"id":"0x4","kind":"DeclRefExpr","range":{"begin":{"offset":13,"tokLen":1},"end":{"offset":13,"tokLen":1}}}]}]},
            {"id":"0x5","kind":"DeclStmt","range":{"begin":{"offset":11,"tokLen":1},"end":{"offset":11,"tokLen":1}}},
            {"id":"0x6","kind":"DeclStmt","range":{"begin":{"offset":11,"tokLen":1},"end":{"offset":11,"tokLen":1}}},
            {"id":"0x7","kind":"BinaryOperator","range":{"begin":{"offset":11,"tokLen":1},"end":{"offset":11,"tokLen":1}}},
            {"id":"0x8","kind":"UnaryOperator","range":{"begin":{"offset":11,"tokLen":1},"end":{"offset":11,"tokLen":1}}},
            {"id":"0x9","kind":"DeclStmt","range":{"begin":{"offset":5,"tokLen":3},"end":{"offset":14,"tokLen":1}},"inner":[
                {"id":"0xa","kind":"VarDecl","range":{"begin":{"offset":5,"tokLen":3},"end":{"offset":11,"tokLen":1}},"inner":[
                    {"id":"0xb","kind":"DeclRefExpr","range":{"begin":{"offset":11,"tokLen":1},"end":{"offset":11,"tokLen":1}}}]}]},
            {"id":"0xc","kind":"NullStmt","range":{"begin":{"offset":16,"tokLen":1},"end":{"offset":16,"tokLen":1}}}
        ]}"#;
        let found = nodes(json, source);
        let kinds: Vec<&str> = found.iter().map(|n| &*n.kind).collect();
        assert_eq!(kinds, ["CXXForRangeStmt", "DeclRefExpr", "VarDecl", "NullStmt"]);
        assert_eq!(
            fact_labels(&found, source),
            [
                "range for 0..17",
                "name 13..14",
                "variable 5..10",
                "null statement 16..17"
            ]
        );
    }

    #[test]
    fn a_closing_shift_token_counts_as_one_angle_bracket() {
        let source = "f<A<int>>";
        let json = r#"{"id":"0x1","kind":"UnresolvedLookupExpr","name":"f","range":{"begin":{"offset":0,"file":"/m.cc","tokLen":1},"end":{"offset":7,"tokLen":2}}}"#;
        let found = nodes(json, source);
        assert_eq!(summary(&found), [("UnresolvedLookupExpr".to_owned(), 0, 8, true)]);
        assert_eq!(fact_labels(&found, source), ["template-id 0..8"]);
    }

    #[test]
    fn an_abbreviated_template_and_an_implicit_statement_have_no_facts() {
        let source = "auto f(auto x) { l: }";
        let json = r#"{"id":"0x1","kind":"FunctionTemplateDecl","range":{"begin":{"offset":0,"file":"/m.cc","tokLen":4},"end":{"offset":20,"tokLen":1}},"inner":[
            {"id":"0x2","kind":"FunctionDecl","range":{"begin":{"offset":0,"tokLen":4},"end":{"offset":20,"tokLen":1}},"inner":[
                {"id":"0x3","kind":"CompoundStmt","range":{"begin":{"offset":15,"tokLen":1},"end":{"offset":20,"tokLen":1}},"inner":[
                    {"id":"0x4","kind":"LabelStmt","name":"l","range":{"begin":{"offset":17,"tokLen":1},"end":{"offset":18,"tokLen":1}},"inner":[
                        {"id":"0x5","kind":"NullStmt","range":{"begin":{"offset":18,"tokLen":1},"end":{"offset":18,"tokLen":1}}}]}]}]},
            {"id":"0x6","kind":"FunctionDecl","range":{"begin":{"offset":0,"tokLen":4},"end":{"offset":20,"tokLen":1}}}
        ]}"#;
        let found = nodes(json, source);
        assert_eq!(found.len(), 5);
        assert_eq!(
            fact_labels(&found, source),
            ["function definition 0..21", "compound 15..21", "label 17..19"]
        );
    }

    #[test]
    fn an_implicit_conversion_call_and_an_unspelled_name_have_no_facts() {
        let source = "{ if (s) M(x); }";
        let json = r#"{"id":"0x1","kind":"CompoundStmt","range":{"begin":{"offset":0,"file":"/m.cc","tokLen":1},"end":{"offset":15,"tokLen":1}},"inner":[
            {"id":"0x2","kind":"CXXMemberCallExpr","range":{"begin":{"offset":6,"tokLen":1},"end":{"offset":6,"tokLen":1}},"inner":[
                {"id":"0x3","kind":"MemberExpr","name":"operator bool","range":{"begin":{"offset":6,"tokLen":1},"end":{"offset":6,"tokLen":1}}}]},
            {"id":"0x4","kind":"NullStmt","range":{"begin":{"offset":13,"tokLen":1},"end":{"offset":13,"tokLen":1}}}
        ]}"#;
        let found = nodes(json, source);
        assert_eq!(found.len(), 4);
        assert_eq!(fact_labels(&found, source), ["compound 0..16"]);
    }

    #[test]
    fn the_block_of_a_dependent_if_exists_has_no_compound_fact() {
        let source = "__if_exists(T::f) { {} }";
        let json = r#"{"id":"0x1","kind":"MSDependentExistsStmt","range":{"begin":{"offset":0,"file":"/m.cc","tokLen":11},"end":{"offset":23,"tokLen":1}},"inner":[
            {"id":"0x2","kind":"CompoundStmt","range":{"begin":{"offset":18,"tokLen":1},"end":{"offset":23,"tokLen":1}},"inner":[
                {"id":"0x3","kind":"CompoundStmt","range":{"begin":{"offset":20,"tokLen":1},"end":{"offset":21,"tokLen":1}}}]}
        ]}"#;
        let found = nodes(json, source);
        assert_eq!(found.len(), 3);
        assert_eq!(fact_labels(&found, source), ["compound 20..22"]);
    }

    #[test]
    fn an_explicit_instantiation_starts_with_template_and_no_angle_bracket() {
        assert!(explicit_instantiation(b"template struct S<int>"));
        assert!(explicit_instantiation(b"extern template void f<int>()"));
        assert!(!explicit_instantiation(b"template<class T> int S<T>::x = 0"));
        assert!(!explicit_instantiation(b"const T foo<T>::x = T()"));
    }

    #[test]
    fn trivia_after_an_expression_includes_comments_and_splices() {
        let source = b"f() /* a */ // b\n \\\n;";
        assert_eq!(skip_trivia(source, 3), source.len() - 1);
    }
}
