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
    pub sections: Vec<Section>,
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
    Other(String),
}

/// `{layout(1.last.col + 1 == 2.first.col)}`: a relation between two
/// symbol positions of the production.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutConstraint {
    pub lhs: LayoutPos,
    /// Added to the left-hand side: the `+ 1` in `1.last.col + 1`.
    pub offset: i32,
    pub op: LayoutOp,
    pub rhs: LayoutPos,
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

impl Module {
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
