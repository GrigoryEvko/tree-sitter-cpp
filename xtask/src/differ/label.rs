//! The categories that the two trees share, and the table of the forms that are ambiguous by design.

use std::fmt;

/// The group of a category in the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    Declaration,
    Statement,
    Expression,
    Other,
}

impl Group {
    /// The name of the group in the report.
    pub fn name(self) -> &'static str {
        match self {
            Group::Declaration => "declaration",
            Group::Statement => "statement",
            Group::Expression => "expression",
            Group::Other => "other",
        }
    }
}

/// Declare the categories with their names and groups in one table.
macro_rules! categories {
    ($($variant:ident $name:literal $group:ident,)*) => {
        /// A category of a syntax node. Clang nodes and our nodes get categories from this set.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Category {
            $($variant,)*
        }

        impl Category {
            /// All the categories, in the order of the report.
            pub const ALL: &[Category] = &[$(Category::$variant,)*];

            /// The name of the category in the report.
            pub fn name(self) -> &'static str {
                match self {
                    $(Category::$variant => $name,)*
                }
            }

            /// The group of the category.
            pub fn group(self) -> Group {
                match self {
                    $(Category::$variant => Group::$group,)*
                }
            }
        }
    };
}

categories! {
    FunctionDefinition "function definition" Declaration,
    FunctionDeclaration "function declaration" Declaration,
    Variable "variable" Declaration,
    Parameter "parameter" Declaration,
    Field "field" Declaration,
    Class "class" Declaration,
    Enum "enum" Declaration,
    Enumerator "enumerator" Declaration,
    Namespace "namespace" Declaration,
    NamespaceAlias "namespace alias" Declaration,
    TypedefAlias "typedef or alias" Declaration,
    Template "template" Declaration,
    TemplateParameter "template parameter" Declaration,
    Concept "concept" Declaration,
    StaticAssert "static_assert" Declaration,
    Using "using" Declaration,
    Friend "friend" Declaration,
    Linkage "linkage specification" Declaration,
    AccessSpecifier "access specifier" Declaration,
    ExplicitInstantiation "explicit instantiation" Declaration,
    Export "export" Declaration,
    Import "import" Declaration,
    DeclarationStatement "declaration statement" Statement,
    Compound "compound" Statement,
    If "if" Statement,
    For "for" Statement,
    RangeFor "range for" Statement,
    While "while" Statement,
    Do "do" Statement,
    Switch "switch" Statement,
    Case "case" Statement,
    Return "return" Statement,
    Break "break" Statement,
    Continue "continue" Statement,
    Goto "goto" Statement,
    Label "label" Statement,
    Try "try" Statement,
    Catch "catch" Statement,
    CoReturn "co_return" Statement,
    ExpressionStatement "expression statement" Statement,
    NullStatement "null statement" Statement,
    AttributedStatement "attributed statement" Statement,
    Call "call" Expression,
    MemberAccess "member access" Expression,
    Binary "binary" Expression,
    Unary "unary" Expression,
    Assignment "assignment" Expression,
    Conditional "conditional" Expression,
    Cast "cast" Expression,
    FunctionalCast "functional cast" Expression,
    CompoundLiteral "compound literal" Expression,
    New "new" Expression,
    Delete "delete" Expression,
    Lambda "lambda" Expression,
    Subscript "subscript" Expression,
    Sizeof "sizeof" Expression,
    Alignof "alignof" Expression,
    Throw "throw" Expression,
    Literal "literal" Expression,
    InitList "braced list" Expression,
    DesignatedInit "designated initializer" Expression,
    TemplateId "template-id" Expression,
    Name "name" Expression,
    This "this" Expression,
    Paren "parentheses" Expression,
    Typeid "typeid" Expression,
    Noexcept "noexcept" Expression,
    Fold "fold" Expression,
    PackExpansion "pack expansion" Expression,
    PackIndex "pack index" Expression,
    Requires "requires" Expression,
    CoAwait "co_await" Expression,
    CoYield "co_yield" Expression,
    StatementExpression "statement expression" Expression,
    TypeTrait "type trait" Expression,
    Offsetof "offsetof" Expression,
    Attribute "attribute" Other,
    Type "type" Other,
    TypeOperand "type operand" Other,
    ParenType "parenthesized type" Other,
}

/// A category with an optional detail: an operator, or the kind of a cast or of a literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Label {
    pub category: Category,
    pub detail: &'static str,
}

impl Label {
    /// A label with no detail.
    pub const fn new(category: Category) -> Label {
        Label { category, detail: "" }
    }

    /// A label with the static detail of `text`. Refer to [`detail`].
    pub fn with(category: Category, text: &str) -> Label {
        Label {
            category,
            detail: detail(text),
        }
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.detail.is_empty() {
            f.write_str(self.category.name())
        } else {
            write!(f, "{} {}", self.category.name(), self.detail)
        }
    }
}

/// The details that a label can have. A text that is not in this list becomes `?`.
const DETAILS: &[&str] = &[
    "+",
    "-",
    "*",
    "/",
    "%",
    "^",
    "&",
    "|",
    "~",
    "!",
    "=",
    "<",
    ">",
    "+=",
    "-=",
    "*=",
    "/=",
    "%=",
    "^=",
    "&=",
    "|=",
    "<<",
    ">>",
    "<<=",
    ">>=",
    "==",
    "!=",
    "<=",
    ">=",
    "<=>",
    "&&",
    "||",
    "++x",
    "x++",
    "--x",
    "x--",
    ",",
    ".*",
    "->*",
    "__extension__",
    "__real",
    "__imag",
    "c-style",
    "static_cast",
    "dynamic_cast",
    "reinterpret_cast",
    "const_cast",
    "addrspace_cast",
    "number",
    "char",
    "string",
    "bool",
    "nullptr",
    "user-defined",
];

/// The static text of a detail. An alternative token becomes its primary token, for example `bitand` becomes `&`.
pub fn detail(text: &str) -> &'static str {
    let text = match text {
        "" => return "",
        "and" => "&&",
        "or" => "||",
        "not" => "!",
        "bitand" => "&",
        "bitor" => "|",
        "xor" => "^",
        "compl" => "~",
        "and_eq" => "&=",
        "or_eq" => "|=",
        "xor_eq" => "^=",
        "not_eq" => "!=",
        "__real__" => "__real",
        "__imag__" => "__imag",
        other => other,
    };
    DETAILS.iter().find(|known| **known == text).copied().unwrap_or("?")
}

/// A pattern of a label. An empty list of details agrees with all details.
struct Pattern {
    category: Category,
    details: &'static [&'static str],
}

impl Pattern {
    /// True when the label has the category of the pattern and one of its details.
    fn accepts(&self, label: Label) -> bool {
        label.category == self.category && (self.details.is_empty() || self.details.contains(&label.detail))
    }
}

/// One direction of a form with two correct trees: the Clang label, then our label.
struct Ambiguity {
    clang: Pattern,
    ours: Pattern,
}

/// An entry of the table of ambiguous forms.
const fn ambiguity(
    clang: Category,
    clang_details: &'static [&'static str],
    ours: Category,
    our_details: &'static [&'static str],
) -> Ambiguity {
    Ambiguity {
        clang: Pattern {
            category: clang,
            details: clang_details,
        },
        ours: Pattern {
            category: ours,
            details: our_details,
        },
    }
}

/// The forms that have two correct trees when the parser does not know which names are types.
///
/// Tree-sitter has no name lookup. For each form, the grammar selects one tree, and Clang selects
/// the other tree when a name has the other kind. Each entry gives one direction.
const AMBIGUOUS: &[Ambiguity] = &[
    // `T(x)` is a call when `T` is a function or an object, and a functional cast when `T` is a type.
    ambiguity(Category::FunctionalCast, &[], Category::Call, &[]),
    // `S<int>(x)`: the grammar reads `S<int>` as a type, but `S` can also name a function template.
    ambiguity(Category::Call, &[], Category::FunctionalCast, &[]),
    // `a * b;`, `a(b);`, and `a<b> c;` declare a variable when `a` is a type, and are expressions when `a` is a value.
    // A statement that starts with a type keyword, for example `int(x);`, is a declaration with no lookup.
    ambiguity(
        Category::DeclarationStatement,
        &[],
        Category::ExpressionStatement,
        &[""],
    ),
    ambiguity(Category::ExpressionStatement, &[], Category::DeclarationStatement, &[]),
    // `T x(a);` declares a variable when `a` is a value, and a function when `a` is a type.
    // Arguments that all start with a type keyword, for example `T x(int(a));`, declare a function with no lookup.
    ambiguity(Category::Variable, &[], Category::FunctionDeclaration, &[]),
    ambiguity(Category::FunctionDeclaration, &[], Category::Variable, &[""]),
    // `a * b` in the first clause of `for` declares `b` when `a` is a type, and multiplies when `a` is a value.
    ambiguity(Category::Variable, &[], Category::Binary, &["*", "&", "&&"]),
    ambiguity(Category::Binary, &["*", "&", "&&"], Category::Variable, &[]),
    // `(a) - b` and `(a)(b)` are casts when `a` is a type, and a binary operator or a call when `a` is a value.
    ambiguity(
        Category::Binary,
        &["+", "-", "*", "&", "&&"],
        Category::Cast,
        &["c-style"],
    ),
    ambiguity(Category::Call, &[], Category::Cast, &["c-style"]),
    ambiguity(
        Category::Cast,
        &["c-style"],
        Category::Binary,
        &["+", "-", "*", "&", "&&"],
    ),
    ambiguity(Category::Cast, &["c-style"], Category::Call, &[]),
    // `f<a>(b)` is a call of a template-id when `f` is a template, and two comparisons when `f` is a value.
    ambiguity(Category::Binary, &[">"], Category::Call, &[]),
    ambiguity(Category::Call, &[], Category::Binary, &[">"]),
    ambiguity(Category::Binary, &["<"], Category::TemplateId, &[]),
    ambiguity(Category::TemplateId, &[], Category::Binary, &["<"]),
    // `sizeof(x)` has a parenthesized expression when `x` is a value, and a type when `x` is a type.
    ambiguity(Category::Paren, &[], Category::ParenType, &[]),
    // `typeid(x)` and a template argument `x` hold an expression when `x` is a value, and a type when `x` is a type.
    ambiguity(Category::Name, &[], Category::TypeOperand, &[]),
    ambiguity(Category::TemplateId, &[], Category::TypeOperand, &[]),
    // `__is_same(T, U)` has the syntax of a call. The grammar does not know the keywords of the builtin traits.
    ambiguity(Category::TypeTrait, &[], Category::Call, &[]),
    // `1.0i` is a GNU imaginary constant for Clang, and a user-defined literal of `std::complex_literals` for GCC.
    ambiguity(Category::Literal, &["number"], Category::Literal, &["user-defined"]),
];

/// True when a Clang label and our label at the same range are the two trees of an ambiguous form.
pub fn is_ambiguous(clang: Label, ours: Label) -> bool {
    AMBIGUOUS
        .iter()
        .any(|entry| entry.clang.accepts(clang) && entry.ours.accepts(ours))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_alternative_token_has_the_detail_of_its_primary_token() {
        assert_eq!(detail("bitand"), "&");
        assert_eq!(detail("not_eq"), "!=");
        assert_eq!(detail("<=>"), "<=>");
        assert_eq!(detail("@@"), "?");
        assert_eq!(Label::with(Category::Binary, "and").to_string(), "binary &&");
    }

    #[test]
    fn the_table_accepts_only_its_directions_and_details() {
        let cast = Label::new(Category::FunctionalCast);
        let call = Label::new(Category::Call);
        assert!(is_ambiguous(cast, call));
        assert!(is_ambiguous(
            Label::with(Category::Binary, "*"),
            Label::new(Category::Variable)
        ));
        assert!(!is_ambiguous(
            Label::with(Category::Binary, "/"),
            Label::new(Category::Variable)
        ));
        assert!(!is_ambiguous(Label::new(Category::Lambda), call));
    }

    #[test]
    fn each_category_has_a_unique_name() {
        let mut names: Vec<&str> = Category::ALL.iter().map(|c| c.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }
}
