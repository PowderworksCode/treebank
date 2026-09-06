//! The shape of an SDF3 module, as the reader produces it.
//!
//! This is SDF3 as documented for Spoofax -- sections, productions with
//! constructors, templates, priorities, restrictions, layout constraints,
//! template options -- narrowed to what the spikes need. One thing is not
//! SDF3 and is marked as such: a template placeholder may carry a `name:`
//! label ([`TemplatePart::Placeholder::label`]), which lowers to a
//! tree-sitter field. SDF3's own AST is positional.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub imports: Vec<String>,
    /// treebank extension: `hiding a/module Sort.Cons ...` after `imports`
    /// subtracts from the composition -- every production a named module
    /// declares itself, or the production a `Sort.Cons` reference names.
    /// SDF3's own imports are additive only.
    pub hiding: Vec<String>,
    pub sections: Vec<Section>,
    /// Sorts declared but left without productions in this composition,
    /// closed by the loader: see [`Hole`].
    pub holes: Vec<Hole>,
}

/// A declared sort no module in the composition gave a production. It is a
/// dialect point that this target does not fill: `<limit:Limit?>` in a
/// core `Select` where the target imports no `sql/limit`. SDF3 semantics
/// make such a sort match nothing, so the loader rewrites the composition
/// to say so: an optional or starred occurrence becomes nothing, and a
/// production that needs the sort is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    pub sort: String,
    /// `Sort.Cons` (or `Sort`) of productions whose optional occurrence of
    /// the hole was removed.
    pub blanked: Vec<String>,
    /// Productions dropped because they needed the hole.
    pub dropped: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Section {
    StartSymbols(Vec<String>),
    Sorts(Vec<String>),
    LexicalSorts(Vec<String>),
    ContextFreeSorts(Vec<String>),
    LexicalSyntax(Vec<Production>),
    ContextFreeSyntax(Vec<Production>),
    LexicalRestrictions(Vec<Restriction>),
    ContextFreeRestrictions(Vec<Restriction>),
    ContextFreePriorities(Vec<PriorityChain>),
    TemplateOptions(Vec<TemplateOption>),
    /// treebank extension: `vocabulary` binds the shared vocabulary's terms
    /// to this module's sorts and constructors.
    Vocabulary(Vec<VocabTerm>),
}

/// `Sort.Constructor = rhs {attrs}`, or `Sort = rhs {attrs}` for an
/// injection or bracket production.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Production {
    pub sort: String,
    pub constructor: Option<String>,
    pub rhs: Rhs,
    pub attrs: Vec<Attr>,
}

/// One symbol position of a production, as layout constraints count them:
/// 1-based over literals and placeholders alike, template layout excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymRef<'a> {
    Lit(&'a str),
    Sym(&'a Symbol),
}

impl Production {
    pub fn has(&self, attr: &Attr) -> bool {
        self.attrs.contains(attr)
    }

    /// `Sort.Cons`, the name a priority declaration refers to it by.
    pub fn reference(&self) -> Option<String> {
        self.constructor
            .as_ref()
            .map(|c| format!("{}.{}", self.sort, c))
    }

    /// The name used in findings: `Sort.Cons`, or the sort for an injection.
    pub fn display(&self) -> String {
        self.reference().unwrap_or_else(|| self.sort.clone())
    }

    pub fn symbols(&self) -> Vec<SymRef<'_>> {
        match &self.rhs {
            Rhs::Symbols(s) => s
                .iter()
                .map(|s| match s {
                    Symbol::Lit(l) => SymRef::Lit(l),
                    other => SymRef::Sym(other),
                })
                .collect(),
            Rhs::Template(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    TemplatePart::Lit(l) => Some(SymRef::Lit(l)),
                    TemplatePart::Placeholder { symbol, .. } => Some(SymRef::Sym(symbol)),
                    TemplatePart::Layout(_) => None,
                })
                .collect(),
        }
    }

    pub fn layout_constraints(&self) -> impl Iterator<Item = &LayoutConstraint> {
        self.attrs.iter().filter_map(|a| match a {
            Attr::Layout(c) => Some(c),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rhs {
    /// The productive form: `Exp "+" Exp`.
    Symbols(Vec<Symbol>),
    /// The template form: `<<Exp> + <Exp>>`. Layout between parts is kept
    /// so a printer could use it; the parser lowering drops it.
    Template(Vec<TemplatePart>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplatePart {
    /// A run of literal text with no whitespace in it: one token.
    Lit(String),
    /// Whitespace and newlines between parts.
    Layout(String),
    Placeholder {
        /// treebank extension, not SDF3: `<left:Exp>`.
        label: Option<String>,
        symbol: Symbol,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Symbol {
    Sort(String),
    Lit(String),
    CharClass(CharClass),
    Star(Box<Symbol>),
    Plus(Box<Symbol>),
    Opt(Box<Symbol>),
    /// `{Elem Sep}*` (`min` 0) or `{Elem Sep}+` (`min` 1).
    SepList {
        elem: Box<Symbol>,
        sep: Box<Symbol>,
        min: usize,
    },
    /// `(a b | c)`: alternatives, each a sequence.
    Group(Vec<Vec<Symbol>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharClass {
    pub negated: bool,
    pub ranges: Vec<(char, char)>,
}

impl CharClass {
    pub fn contains(&self, c: char) -> bool {
        let inside = self.ranges.iter().any(|(a, b)| *a <= c && c <= *b);
        inside != self.negated
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attr {
    Left,
    Right,
    NonAssoc,
    Assoc,
    Bracket,
    Reject,
    Prefer,
    Avoid,
    Layout(LayoutConstraint),
    /// treebank extension: this production's node delimits a lexical
    /// scope. `scope` or `scope(function)`.
    Scope(Option<String>),
    /// treebank extension: `binds(target -> enclosing)`, `binds(names ->
    /// module as var)`: the names under the labelled field are bound in
    /// the named scope.
    Binds(Binding),
    /// treebank extension: `refers(1)` or `refers(name)`: the name at the
    /// position or field is a reference, resolved against the bindings
    /// in scope.
    Refers(String),
    /// treebank extension for the printer: `separate(2)` puts this many
    /// blank lines around the term when it is an element of a vertical
    /// list, as black does around top-level definitions.
    Separate(u32),
    /// treebank extension for the printer: `collapse(100)` prints the
    /// template on one line when none of its lines holds a vertical list
    /// and the result fits in this many columns -- Box's `HV`, with the
    /// refinement rustfmt applies to blocks.
    Collapse(u32),
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The placeholder label whose names are bound.
    pub label: String,
    pub target: BindTarget,
    /// `var`, `function`, `parameter`: treebank's locals vocabulary.
    pub kind: Option<String>,
    /// When the binding takes effect within its scope.
    pub effect: BindEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindTarget {
    /// The nearest scope node that is a proper ancestor of the binding
    /// node -- for a scope node's own name, the scope around it.
    Enclosing,
    /// The nearest enclosing scope of this kind: `module`, `function`.
    Kind(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindEffect {
    /// The whole scope, before and after the binding: Python's names, a
    /// JavaScript `var`, `let` or function declaration, a Rust `fn` item.
    /// Several whole-scope bindings of one name in one scope are one slot.
    #[default]
    Whole,
    /// From the end of the binding node onward: a Rust `let`, whose
    /// initializer sees the previous binding and which shadows it after.
    /// Each is a new slot.
    After,
}

/// One constraint of a `{layout(...)}` attribute. SDF3 has two forms: the
/// explicit relation between two positions, and the declarative
/// constraints of Spoofax's layout-sensitive SDF3 (Amorim et al., SLE
/// 2018), which name the common shapes. A `layout(a, b)` or
/// `layout(a && b)` attribute reads as one `Attr::Layout` per constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutConstraint {
    Rel(LayoutRel),
    Decl(LayoutDecl),
}

/// `1.last.col + 1 == 2.first.col`: a relation between two symbol
/// positions of the production.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutRel {
    pub lhs: LayoutPos,
    /// Added to the left-hand side: the `+ 1` in `1.last.col + 1`.
    pub offset: i32,
    pub op: LayoutOp,
    pub rhs: LayoutPos,
}

/// `indent 1 4`, `align 1 5`, `align-list 1`, `offside 1`: a declarative
/// constraint over symbol positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutDecl {
    pub kind: LayoutDeclKind,
    /// 1-based symbol positions, in the order written.
    pub refs: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDeclKind {
    /// `indent a b..`: each of b.. starts at a column greater than a's.
    Indent,
    /// `align a b..`: each of b.. starts at a's column.
    Align,
    /// `align-list a`: every element of the list at a starts at one column.
    AlignList,
    /// `offside a` (`offside a b..`): every token of a (of b..) after the
    /// first is at a column greater than a's first column.
    Offside,
    /// `newline-indent a b`: b starts on a later line, indented past a.
    NewlineIndent,
    /// `single-line a b..`: all on one line.
    SingleLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutPos {
    /// 1-based symbol index.
    pub symbol: usize,
    pub end: LayoutEnd,
    pub axis: LayoutAxis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutEnd {
    First,
    Last,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutAxis {
    Col,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutOp {
    Eq,
    Lt,
    Gt,
}

/// `ID -/- [a-zA-Z0-9]`, or `LAYOUT? -/- [\/].[\/]`: each restricted symbol
/// may not be followed by any of the lookaheads, and a lookahead is a
/// sequence of character classes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restriction {
    pub symbols: Vec<String>,
    pub lookaheads: Vec<Vec<CharClass>>,
}

/// `{Exp.Neg Exp.Not} > {left: Exp.Mul Exp.Div} > ...`: groups from highest
/// priority to lowest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorityChain {
    pub groups: Vec<PriorityGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorityGroup {
    pub assoc: Option<Attr>,
    /// `Sort.Cons` references.
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateOption {
    /// `ID = keyword {reject}`: every template literal that could lex as an
    /// ID is rejected as one.
    KeywordReject { sort: String },
    /// `keyword -/- [a-zA-Z0-9]`: a template literal may not be directly
    /// followed by these characters.
    KeywordFollow(CharClass),
    /// `tokenize: "()"`.
    Tokenize(String),
}

/// `_statement = Stmt`, `_branch = Stmt.If Stmt.Match`, `_control_flow =
/// _branch _loop _jump`: a vocabulary term and what of this module it
/// names. A member is a sort (every production of it), a `Sort.Cons`
/// constructor, a lexical sort, or another term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VocabTerm {
    pub term: String,
    pub members: Vec<String>,
}

impl Module {
    /// The module name as an identifier: `mysql/5.7` is `mysql_5_7`, the
    /// name the generated parser, scanner and ANTLR grammar go by.
    pub fn symbol_name(&self) -> String {
        self.name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect()
    }

    /// Every sort named in a `sorts` or `context-free sorts` section.
    pub fn declared_sorts(&self) -> Vec<&str> {
        self.sections
            .iter()
            .flat_map(|s| match s {
                Section::Sorts(v) | Section::ContextFreeSorts(v) => {
                    v.iter().map(String::as_str).collect::<Vec<_>>()
                }
                _ => Vec::new(),
            })
            .collect()
    }

    pub fn vocabulary(&self) -> impl Iterator<Item = &VocabTerm> {
        self.sections.iter().flat_map(|s| match s {
            Section::Vocabulary(v) => v.iter(),
            _ => [].iter(),
        })
    }

    pub fn productions(&self, lexical: bool) -> impl Iterator<Item = &Production> {
        self.sections.iter().flat_map(move |s| match s {
            Section::LexicalSyntax(p) if lexical => p.iter(),
            Section::ContextFreeSyntax(p) if !lexical => p.iter(),
            _ => [].iter(),
        })
    }

    pub fn start_symbols(&self) -> Vec<&str> {
        self.sections
            .iter()
            .flat_map(|s| match s {
                Section::StartSymbols(v) => v.iter().map(String::as_str).collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect()
    }

    pub fn priorities(&self) -> impl Iterator<Item = &PriorityChain> {
        self.sections.iter().flat_map(|s| match s {
            Section::ContextFreePriorities(c) => c.iter(),
            _ => [].iter(),
        })
    }

    pub fn template_options(&self) -> impl Iterator<Item = &TemplateOption> {
        self.sections.iter().flat_map(|s| match s {
            Section::TemplateOptions(o) => o.iter(),
            _ => [].iter(),
        })
    }

    pub fn restrictions(&self, lexical: bool) -> impl Iterator<Item = &Restriction> {
        self.sections.iter().flat_map(move |s| match s {
            Section::LexicalRestrictions(r) if lexical => r.iter(),
            Section::ContextFreeRestrictions(r) if !lexical => r.iter(),
            _ => [].iter(),
        })
    }
}
