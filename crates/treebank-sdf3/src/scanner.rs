//! Layout constraints that tree-sitter's grammar cannot say, and the scanner
//! that says them.
//!
//! An SDF3 layout constraint between two symbols -- `foo` and `-` must be
//! separated, `-` and its operand must be adjacent -- is a fact about
//! whitespace, and tree-sitter's grammar has exactly one whitespace fact,
//! `token.immediate` ("no layout before this token"), which cannot express
//! "layout required before" at all and cannot reach into a nonterminal. So
//! the constrained spelling is **split**: every occurrence of `-` in the
//! grammar becomes one of several external tokens that share the spelling
//! and differ in the layout they require, and a generated scanner picks
//! between them by what the parser could accept first and by the actual
//! spacing second. That is the shape of treebank-ruby's hand-written
//! scanner (`BINARY_MINUS` / `UNARY_MINUS`), derived here from the grammar.
//!
//! The rules the planner applies:
//!
//! - `a.last.col + 1 == b.first.col` with `b == a + 1` is **adjacent**;
//!   `<` is **separated**. Any other constraint shape is reported and
//!   ignored.
//! - Adjacent with a literal at `b` puts *no layout before* on that
//!   occurrence; adjacent with a literal at `a` and a sort at `b` puts *no
//!   layout after* on it. Separated does the same with *required*.
//! - Separated between two sorts propagates *layout required before* to
//!   the first literal of every production reachable at the start of the
//!   second sort, and to any lexical sort opened by a literal there (a
//!   regex literal). The condition is only ever consulted when more than
//!   one variant of the spelling is valid, which is precisely the ambiguous
//!   state the constraint exists to settle, so propagating it through a
//!   production that is also used unconstrained elsewhere is sound.
//! - Every unconstrained occurrence of a split spelling uses the default
//!   variant, which requires nothing.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};

use crate::ast::*;
use crate::lower::{Finding, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cond {
    Any,
    Req,
    Forbid,
}

impl Cond {
    fn merge(self, other: Cond) -> Cond {
        match (self, other) {
            (Cond::Any, o) | (o, Cond::Any) => o,
            (a, _) => a,
        }
    }
    fn suffix<'a>(self, req: &'a str, forbid: &'a str) -> Option<&'a str> {
        match self {
            Cond::Any => None,
            Cond::Req => Some(req),
            Cond::Forbid => Some(forbid),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// The external symbol's name.
    pub name: String,
    pub spelling: String,
    pub before: Cond,
    pub after: Cond,
    /// A lexical sort owned by the scanner: after the opener, consume up to
    /// and including this closer on the same line.
    pub closer: Option<char>,
    /// True for a lexical sort's variant, which is a named node; false for
    /// an operator variant, which is aliased back to its anonymous spelling.
    pub visible: bool,
}

impl Variant {
    fn specificity(&self) -> usize {
        (self.before != Cond::Any) as usize
            + (self.after != Cond::Any) as usize
            + self.closer.is_some() as usize
    }
}

#[derive(Debug, Default)]
pub struct Plan {
    /// (production index, 1-based symbol position) -> external name.
    pub occurrences: BTreeMap<(usize, usize), String>,
    /// Lexical sort -> external name, for sorts the scanner scans whole.
    pub lexical_owned: BTreeMap<String, String>,
    pub variants: Vec<Variant>,
    /// Characters LAYOUT skips; the scanner skips the same ones.
    pub layout_chars: Vec<char>,
    /// The character that opens a line comment in LAYOUT, if one does; the
    /// indentation scanner looks past comment lines.
    pub comment_open: Option<char>,
    /// Block structure by column, when the module's declarative layout
    /// constraints ask for it.
    pub indent: Option<IndentPlan>,
}

/// What `indent`, `align-list`, `align` and `offside` lower to: three
/// external tokens (`_newline`, `_indent`, `_dedent`) emitted by a scanner
/// that keeps a stack of open block columns -- CPython's tokenizer and
/// tree-sitter-python's scanner, derived from the constraints instead of
/// written.
///
/// - `indent a b` wraps the occurrence at `b` as `_indent b _dedent`.
/// - `align-list n` makes the element sort line-aligned: every production
///   of it ends with `_newline` unless it already ends in an indented
///   block, whose `_dedent` closes it. The scanner emits `_newline` at a
///   line break whose next line starts at or left of the open column,
///   `_indent` when the next line is deeper and the parser can open a
///   block, nothing (a continuation) when it is deeper and cannot -- which
///   is the `offside` rule -- and `_dedent`, zero-width, for each open
///   column the next line has left.
/// - `align a b` after an indented block holds by construction: the
///   scanner refuses a dedent to a column no open block has.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IndentPlan {
    /// (production index, symbol position) wrapped in `_indent .. _dedent`.
    pub blocks: BTreeSet<(usize, usize)>,
    /// Production indices that end with `_newline`.
    pub terminated: BTreeSet<usize>,
    /// Literals that immediately precede an indented occurrence. A backend
    /// whose lexer cannot ask the parser whether a block may open decides
    /// by these instead.
    pub openers: BTreeSet<String>,
    /// Sorts whose elements are line-aligned.
    pub aligned: BTreeSet<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.variants.is_empty() && self.indent.is_none()
    }

    /// External names in declaration order, sentinel last.
    pub fn externals(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        if self.indent.is_some() {
            v.extend(["_newline", "_indent", "_dedent"].map(String::from));
        }
        v.extend(self.variants.iter().map(|v| v.name.clone()));
        v.push("_error_sentinel".into());
        v
    }
}

/// A constrained literal occurrence: (production index, symbol position),
/// with its (before, after) layout conditions.
type Occurrence = ((usize, usize), (Cond, Cond));

#[derive(Default)]
struct Constraints {
    /// (production index, symbol position) -> (before, after)
    on: BTreeMap<(usize, usize), (Cond, Cond)>,
    /// lexical sort -> before
    lexical: BTreeMap<String, Cond>,
}

pub fn plan(module: &Module) -> Result<(Plan, Vec<Finding>)> {
    let mut findings = Vec::new();
    let prods: Vec<&Production> = module.productions(false).collect();
    let lexical: BTreeMap<&str, Vec<&Production>> = {
        let mut m: BTreeMap<&str, Vec<&Production>> = BTreeMap::new();
        for p in module.productions(true) {
            m.entry(p.sort.as_str()).or_default().push(p);
        }
        m
    };
    let mut cons = Constraints::default();
    let mut indent = IndentPlan::default();
    let mut offside: BTreeSet<usize> = BTreeSet::new();

    for (pi, p) in prods.iter().enumerate() {
        let symbols = p.symbols();
        for c in p.layout_constraints() {
            if let LayoutConstraint::Decl(d) = c {
                plan_decl(p, pi, d, &symbols, &mut indent, &mut offside, &mut findings)?;
                continue;
            }
            let Some((a, adjacent)) = classify(c) else {
                findings.push(Finding {
                    kind: Kind::Unsupported,
                    what: format!(
                        "{}: layout constraint {} is not an adjacency or separation between consecutive symbols; ignored",
                        p.display(),
                        render(c)
                    ),
                });
                continue;
            };
            let b = a + 1;
            if b > symbols.len() {
                bail!("{}: layout constraint refers to symbol {b}, which the production does not have", p.display());
            }
            let before = if adjacent { Cond::Forbid } else { Cond::Req };
            let after = before;
            let word = if adjacent { "adjacent" } else { "separated" };
            match (symbols[a - 1], symbols[b - 1]) {
                (_, SymRef::Lit(_)) => {
                    let e = cons.on.entry((pi, b)).or_insert((Cond::Any, Cond::Any));
                    e.0 = e.0.merge(before);
                    findings.push(Finding {
                        kind: Kind::Mapped,
                        what: format!(
                            "{}: symbols {a} and {b} {word}: layout {} before the literal at {b}",
                            p.display(),
                            cond_word(before)
                        ),
                    });
                }
                (SymRef::Lit(_), SymRef::Sym(_)) => {
                    let e = cons.on.entry((pi, a)).or_insert((Cond::Any, Cond::Any));
                    e.1 = e.1.merge(after);
                    findings.push(Finding {
                        kind: Kind::Mapped,
                        what: format!(
                            "{}: symbols {a} and {b} {word}: layout {} after the literal at {a}",
                            p.display(),
                            cond_word(after)
                        ),
                    });
                }
                (SymRef::Sym(_), SymRef::Sym(Symbol::Sort(sort))) => {
                    if adjacent {
                        findings.push(Finding {
                            kind: Kind::Unsupported,
                            what: format!("{}: adjacency between two nonterminals ({a}, {b}) has no tree-sitter form; ignored", p.display()),
                        });
                        continue;
                    }
                    let mut visited = BTreeSet::new();
                    let mut reached = Vec::new();
                    first_literals(
                        sort,
                        &prods,
                        &lexical,
                        &mut visited,
                        &mut reached,
                        &mut cons,
                        before,
                    );
                    findings.push(Finding {
                        kind: Kind::Mapped,
                        what: format!(
                            "{}: symbols {a} and {b} separated: layout required before the first token of {sort}, propagated to {}",
                            p.display(),
                            if reached.is_empty() { "nothing".to_string() } else { reached.join(", ") }
                        ),
                    });
                }
                (_, SymRef::Sym(other)) => {
                    findings.push(Finding {
                        kind: Kind::Unsupported,
                        what: format!("{}: layout constraint reaches into {other:?}, which the planner does not follow; ignored", p.display()),
                    });
                }
            }
        }
    }

    let mut plan = Plan::default();
    for p in lexical.get("LAYOUT").into_iter().flatten() {
        if let Rhs::Symbols(s) = &p.rhs {
            if let [Symbol::Lit(open), ..] = s.as_slice() {
                plan.comment_open = single(open);
            }
        }
    }
    if !indent.blocks.is_empty() || !indent.aligned.is_empty() {
        finish_indent(&prods, &mut indent, &offside, &mut findings);
        plan.indent = Some(indent);
    }

    // Which spellings are split: every literal with a constrained occurrence.
    let mut split: BTreeMap<String, Vec<Occurrence>> = BTreeMap::new();
    for (&(pi, pos), &conds) in &cons.on {
        let SymRef::Lit(l) = prods[pi].symbols()[pos - 1] else {
            continue;
        };
        split
            .entry(l.to_string())
            .or_default()
            .push(((pi, pos), conds));
    }
    for (spelling, constrained) in &split {
        if spelling.chars().count() != 1 {
            findings.push(Finding {
                kind: Kind::Unsupported,
                what: format!("layout constraints on the multi-character literal {spelling:?}: the generated scanner handles single characters; ignored"),
            });
            continue;
        }
        let stem = spelling_name(spelling);
        let mut variant_names: BTreeMap<(Cond, Cond), String> = BTreeMap::new();
        for (occ, conds) in constrained {
            let name = variant_names.entry(*conds).or_insert_with(|| {
                let mut n = format!("_{stem}");
                for s in [
                    conds.0.suffix("spaced", "adjacent"),
                    conds.1.suffix("loose", "tight"),
                ]
                .into_iter()
                .flatten()
                {
                    n.push('_');
                    n.push_str(s);
                }
                n
            });
            plan.occurrences.insert(*occ, name.clone());
        }
        // Unconstrained occurrences of the same spelling take the default.
        let mut needs_default = false;
        for (pi, p) in prods.iter().enumerate() {
            for (k, s) in p.symbols().iter().enumerate() {
                let pos = k + 1;
                if matches!(s, SymRef::Lit(l) if *l == spelling)
                    && !cons.on.contains_key(&(pi, pos))
                {
                    plan.occurrences.insert((pi, pos), format!("_{stem}"));
                    needs_default = true;
                }
            }
        }
        for (conds, name) in &variant_names {
            plan.variants.push(Variant {
                name: name.clone(),
                spelling: spelling.clone(),
                before: conds.0,
                after: conds.1,
                closer: None,
                visible: false,
            });
        }
        if needs_default {
            plan.variants.push(Variant {
                name: format!("_{stem}"),
                spelling: spelling.clone(),
                before: Cond::Any,
                after: Cond::Any,
                closer: None,
                visible: false,
            });
        }
        findings.push(Finding {
            kind: Kind::Mapped,
            what: format!(
                "the spelling {spelling:?} is split into scanner-owned variants [{}]; each is aliased back to {spelling:?} in the tree",
                plan.variants.iter().filter(|v| v.spelling == *spelling).map(|v| v.name.as_str()).collect::<Vec<_>>().join(", ")
            ),
        });
    }

    // Lexical sorts the scanner scans whole.
    for (sort, before) in &cons.lexical {
        let defs = &lexical[sort.as_str()];
        let def = defs[0];
        let Rhs::Symbols(syms) = &def.rhs else {
            bail!("lexical sort {sort} is a template")
        };
        let (Some(Symbol::Lit(open)), Some(Symbol::Lit(close))) = (syms.first(), syms.last())
        else {
            bail!("lexical sort {sort} is reached by a layout constraint but is not opened and closed by literals");
        };
        let (Some(open), Some(close)) = (single(open), single(close)) else {
            bail!("lexical sort {sort}: opener and closer must be single characters for the generated scanner");
        };
        // No layout after the opener if the class that follows it excludes
        // every LAYOUT character.
        let after = match syms.get(1) {
            Some(Symbol::CharClass(c)) if !c.contains(' ') && !c.contains('\t') => Cond::Forbid,
            _ => Cond::Any,
        };
        let name = crate::lower::snake(sort);
        plan.variants.push(Variant {
            name: name.clone(),
            spelling: open.to_string(),
            before: *before,
            after,
            closer: Some(close),
            visible: true,
        });
        plan.lexical_owned.insert(sort.clone(), name.clone());
        findings.push(Finding {
            kind: Kind::Mapped,
            what: format!(
                "lexical sort {sort} opens with {open:?}, a split spelling, so the scanner scans it whole as the named token `{name}` (layout {} before, {} after)",
                cond_word(*before),
                cond_word(after)
            ),
        });
        // The opener's default variant may not exist yet (Div's `/` does).
        let stem = spelling_name(&open.to_string());
        if !plan
            .variants
            .iter()
            .any(|v| v.spelling == open.to_string() && !v.visible)
        {
            // Every unconstrained occurrence of this spelling elsewhere.
            let mut any = false;
            for (pi, p) in prods.iter().enumerate() {
                for (k, s) in p.symbols().iter().enumerate() {
                    if matches!(s, SymRef::Lit(l) if l.chars().count() == 1 && l.starts_with(open))
                    {
                        plan.occurrences.insert((pi, k + 1), format!("_{stem}"));
                        any = true;
                    }
                }
            }
            if any {
                plan.variants.push(Variant {
                    name: format!("_{stem}"),
                    spelling: open.to_string(),
                    before: Cond::Any,
                    after: Cond::Any,
                    closer: None,
                    visible: false,
                });
            }
        }
    }

    // Most specific first, default last; the scanner tries them in order.
    plan.variants.sort_by(|a, b| {
        b.specificity()
            .cmp(&a.specificity())
            .then(a.name.cmp(&b.name))
    });
    if plan.variants.iter().any(|v| v.specificity() > 0) {
        findings.push(Finding {
            kind: Kind::Widening,
            what: "the scanner decides by validity first: where only one variant of a spelling is valid it is emitted whatever the spacing, so a layout constraint is enforced only in the states that have a choice -- `y = - 1` parses as a negation where the module's adjacency constraint rejects it, which is Ruby's behaviour and not SDF3's".into(),
        });
    }

    for p in lexical.get("LAYOUT").into_iter().flatten() {
        if let Rhs::Symbols(s) = &p.rhs {
            if let [Symbol::CharClass(c)] = s.as_slice() {
                for (a, b) in &c.ranges {
                    let mut ch = *a;
                    loop {
                        plan.layout_chars.push(ch);
                        if ch >= *b {
                            break;
                        }
                        ch = char::from_u32(ch as u32 + 1).unwrap_or(*b);
                    }
                }
            }
        }
    }
    if !plan.is_empty() && plan.layout_chars.is_empty() {
        findings.push(Finding {
            kind: Kind::Widening,
            what: "no single-class LAYOUT production, so the generated scanner skips no layout and every spacing test reads as adjacent".into(),
        });
    }
    Ok((plan, findings))
}

/// One declarative constraint into the indent plan.
fn plan_decl(
    p: &Production,
    pi: usize,
    d: &LayoutDecl,
    symbols: &[SymRef<'_>],
    indent: &mut IndentPlan,
    offside: &mut BTreeSet<usize>,
    findings: &mut Vec<Finding>,
) -> Result<()> {
    let at = |n: usize| -> Result<SymRef<'_>> {
        symbols
            .get(n.wrapping_sub(1))
            .copied()
            .ok_or_else(|| anyhow::anyhow!("{}: layout constraint refers to symbol {n}, which the production does not have", p.display()))
    };
    let shown = render(&LayoutConstraint::Decl(d.clone()));
    match d.kind {
        LayoutDeclKind::AlignList => {
            let [n] = d.refs[..] else {
                bail!("{}: align-list takes one symbol", p.display());
            };
            let elem = match at(n)? {
                SymRef::Sym(Symbol::Star(e) | Symbol::Plus(e)) => e.as_ref(),
                SymRef::Sym(Symbol::SepList { elem, .. }) => elem.as_ref(),
                _ => {
                    findings.push(Finding {
                        kind: Kind::Unsupported,
                        what: format!("{}: `{shown}` on a symbol that is not a list; ignored", p.display()),
                    });
                    return Ok(());
                }
            };
            let Symbol::Sort(sort) = elem else {
                findings.push(Finding {
                    kind: Kind::Unsupported,
                    what: format!("{}: `{shown}` on a list whose element is not a sort; ignored", p.display()),
                });
                return Ok(());
            };
            indent.aligned.insert(sort.clone());
            findings.push(Finding {
                kind: Kind::Mapped,
                what: format!(
                    "{}: `{shown}`: every {sort} in the list starts a line at the list's column, so each production of {sort} ends with `_newline` unless an indented block already ends it",
                    p.display()
                ),
            });
        }
        LayoutDeclKind::Indent => {
            let [a, rest @ ..] = &d.refs[..] else {
                bail!("{}: indent takes two or more symbols", p.display());
            };
            at(*a)?;
            for b in rest {
                if matches!(at(*b)?, SymRef::Lit(_)) {
                    findings.push(Finding {
                        kind: Kind::Unsupported,
                        what: format!("{}: `{shown}` indents a literal; ignored", p.display()),
                    });
                    continue;
                }
                indent.blocks.insert((pi, *b));
                let opener = match b.checked_sub(2).and_then(|k| symbols.get(k)) {
                    Some(SymRef::Lit(l)) => {
                        indent.openers.insert(l.to_string());
                        format!("after the literal {l:?}")
                    }
                    _ => "with no literal before it".to_string(),
                };
                findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!(
                        "{}: `{shown}`: symbol {b} is wrapped as `_indent .. _dedent`; the scanner opens a block when the next line is deeper than the open column and the parser can accept `_indent` ({opener})",
                        p.display()
                    ),
                });
            }
        }
        LayoutDeclKind::Align => {
            let [a, rest @ ..] = &d.refs[..] else {
                bail!("{}: align takes two or more symbols", p.display());
            };
            at(*a)?;
            for b in rest {
                at(*b)?;
                if b.checked_sub(1).is_some_and(|prev| indent.blocks.contains(&(pi, prev))) {
                    findings.push(Finding {
                        kind: Kind::Mapped,
                        what: format!(
                            "{}: `{shown}`: symbol {b} follows an indented block, so it sits at symbol {a}'s column by the indent stack; a dedent to a column no open block has is an error",
                            p.display()
                        ),
                    });
                } else {
                    findings.push(Finding {
                        kind: Kind::Unsupported,
                        what: format!("{}: `{shown}` between symbols not separated by an indented block; ignored", p.display()),
                    });
                }
            }
        }
        LayoutDeclKind::Offside => {
            for r in &d.refs {
                at(*r)?;
            }
            offside.insert(pi);
            findings.push(Finding {
                kind: Kind::Mapped,
                what: format!(
                    "{}: `{shown}`: a line break followed by a deeper line continues the statement and one at the open column ends it; the scanner decides by the next line's column",
                    p.display()
                ),
            });
        }
        LayoutDeclKind::NewlineIndent | LayoutDeclKind::SingleLine => {
            findings.push(Finding {
                kind: Kind::Unsupported,
                what: format!("{}: `{shown}` has no lowering yet; ignored", p.display()),
            });
        }
    }
    Ok(())
}

/// Decide which productions end with `_newline`, and say what the
/// derived scanner does that the module did not ask for.
fn finish_indent(
    prods: &[&Production],
    indent: &mut IndentPlan,
    offside: &BTreeSet<usize>,
    findings: &mut Vec<Finding>,
) {
    let mut widened = Vec::new();
    for (pi, p) in prods.iter().enumerate() {
        if !indent.aligned.contains(&p.sort) {
            continue;
        }
        let mut visiting = BTreeSet::new();
        if block_ended(prods, &indent.blocks, pi, &mut visiting) {
            continue;
        }
        indent.terminated.insert(pi);
        if !offside.contains(&pi) {
            widened.push(p.display());
        }
    }
    if !widened.is_empty() {
        findings.push(Finding {
            kind: Kind::Widening,
            what: format!(
                "the scanner applies the offside rule to every element of an aligned list, since it ends an element at a line break by the next line's column alone; [{}] declare no `offside` and get it anyway",
                widened.join(", ")
            ),
        });
    }
    findings.push(Finding {
        kind: Kind::Widening,
        what: "where no `_newline` can end a statement the scanner is not consulted and a line break is layout, so inside brackets a line may continue at any column: Python's implicit line joining, which the offside rule rejects".into(),
    });
    findings.push(Finding {
        kind: Kind::Deviation,
        what: "the outermost aligned list is aligned at column 0, as in CPython, where SDF3 aligns it at its first line's column: a file indented throughout parses its second line as a continuation of its first".into(),
    });
    findings.push(Finding {
        kind: Kind::Deviation,
        what: "a tab is one column, as tree-sitter's lexer counts; CPython uses tab stops of eight".into(),
    });
}

/// Does the production end in an indented block, so that `_dedent` already
/// ends it? Walks optional trailing sorts back to the block.
fn block_ended(
    prods: &[&Production],
    blocks: &BTreeSet<(usize, usize)>,
    pi: usize,
    visiting: &mut BTreeSet<usize>,
) -> bool {
    if !visiting.insert(pi) {
        return false;
    }
    let symbols = prods[pi].symbols();
    for k in (0..symbols.len()).rev() {
        if blocks.contains(&(pi, k + 1)) {
            return true;
        }
        match symbols[k] {
            SymRef::Sym(Symbol::Opt(inner)) => match inner.as_ref() {
                Symbol::Sort(s) if sort_block_ended(prods, blocks, s, visiting) => continue,
                _ => return false,
            },
            SymRef::Sym(Symbol::Sort(s)) => return sort_block_ended(prods, blocks, s, visiting),
            _ => return false,
        }
    }
    false
}

fn sort_block_ended(
    prods: &[&Production],
    blocks: &BTreeSet<(usize, usize)>,
    sort: &str,
    visiting: &mut BTreeSet<usize>,
) -> bool {
    let mut any = false;
    for (qi, q) in prods.iter().enumerate() {
        if q.sort == sort {
            any = true;
            if !block_ended(prods, blocks, qi, &mut visiting.clone()) {
                return false;
            }
        }
    }
    any
}

fn classify(c: &LayoutConstraint) -> Option<(usize, bool)> {
    let LayoutConstraint::Rel(c) = c else {
        return None;
    };
    let adjacent = match c.op {
        LayoutOp::Eq => true,
        LayoutOp::Lt => false,
        LayoutOp::Gt => return None,
    };
    let ok = c.lhs.end == LayoutEnd::Last
        && c.lhs.axis == LayoutAxis::Col
        && c.rhs.end == LayoutEnd::First
        && c.rhs.axis == LayoutAxis::Col
        && c.offset == 1
        && c.rhs.symbol == c.lhs.symbol + 1;
    ok.then_some((c.lhs.symbol, adjacent))
}

pub(crate) fn render(c: &LayoutConstraint) -> String {
    let c = match c {
        LayoutConstraint::Rel(r) => r,
        LayoutConstraint::Decl(d) => {
            let word = match d.kind {
                LayoutDeclKind::Indent => "indent",
                LayoutDeclKind::Align => "align",
                LayoutDeclKind::AlignList => "align-list",
                LayoutDeclKind::Offside => "offside",
                LayoutDeclKind::NewlineIndent => "newline-indent",
                LayoutDeclKind::SingleLine => "single-line",
            };
            let refs: Vec<String> = d.refs.iter().map(|r| r.to_string()).collect();
            return format!("{word} {}", refs.join(" "));
        }
    };
    let pos = |p: &LayoutPos| {
        format!(
            "{}.{}.{}",
            p.symbol,
            match p.end {
                LayoutEnd::First => "first",
                LayoutEnd::Last => "last",
            },
            match p.axis {
                LayoutAxis::Col => "col",
                LayoutAxis::Line => "line",
            }
        )
    };
    let op = match c.op {
        LayoutOp::Eq => "==",
        LayoutOp::Lt => "<",
        LayoutOp::Gt => ">",
    };
    if c.offset == 0 {
        format!("{} {op} {}", pos(&c.lhs), pos(&c.rhs))
    } else {
        format!("{} + {} {op} {}", pos(&c.lhs), c.offset, pos(&c.rhs))
    }
}

fn cond_word(c: Cond) -> &'static str {
    match c {
        Cond::Any => "unconstrained",
        Cond::Req => "required",
        Cond::Forbid => "forbidden",
    }
}

fn single(s: &str) -> Option<char> {
    let mut it = s.chars();
    let c = it.next()?;
    it.next().is_none().then_some(c)
}

/// Walk to the first symbol of every production reachable at the start of
/// `sort`, applying `before` to first literals and to lexical sorts opened
/// by a literal.
fn first_literals(
    sort: &str,
    prods: &[&Production],
    lexical: &BTreeMap<&str, Vec<&Production>>,
    visited: &mut BTreeSet<String>,
    reached: &mut Vec<String>,
    cons: &mut Constraints,
    before: Cond,
) {
    if !visited.insert(sort.to_string()) {
        return;
    }
    if let Some(defs) = lexical.get(sort) {
        if let Some(Rhs::Symbols(s)) = defs.first().map(|d| &d.rhs) {
            if matches!(s.first(), Some(Symbol::Lit(_))) {
                let e = cons.lexical.entry(sort.to_string()).or_insert(Cond::Any);
                *e = e.merge(before);
                reached.push(format!("lexical {sort}"));
            }
        }
        return;
    }
    for (pi, p) in prods.iter().enumerate() {
        if p.sort != sort {
            continue;
        }
        match p.symbols().first() {
            Some(SymRef::Lit(_)) => {
                let e = cons.on.entry((pi, 1)).or_insert((Cond::Any, Cond::Any));
                e.0 = e.0.merge(before);
                reached.push(p.display());
            }
            Some(SymRef::Sym(Symbol::Sort(s))) => {
                first_literals(s, prods, lexical, visited, reached, cons, before);
            }
            _ => {}
        }
    }
}

pub fn spelling_name(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '-' => "minus".to_string(),
            '+' => "plus".into(),
            '*' => "star".into(),
            '/' => "slash".into(),
            '%' => "percent".into(),
            '[' => "lbracket".into(),
            ']' => "rbracket".into(),
            '(' => "lparen".into(),
            ')' => "rparen".into(),
            '{' => "lbrace".into(),
            '}' => "rbrace".into(),
            '<' => "lt".into(),
            '>' => "gt".into(),
            '=' => "eq".into(),
            '!' => "bang".into(),
            '&' => "amp".into(),
            '|' => "pipe".into(),
            '?' => "question".into(),
            ':' => "colon".into(),
            '.' => "dot".into(),
            ',' => "comma".into(),
            '^' => "caret".into(),
            '~' => "tilde".into(),
            c if c.is_ascii_alphanumeric() || c == '_' => c.to_string(),
            c => format!("u{:x}", c as u32),
        })
        .collect::<Vec<_>>()
        .join("_")
}

/// The scanner as C source, for `src/scanner.c`.
pub fn c_source(plan: &Plan, language: &str) -> String {
    let mut out = String::new();
    out.push_str("// GENERATED by treebank-sdf3 from the module's layout constraints. Do not\n// edit: the lowering regenerates it. See src/scanner.rs for the rules.\n\n");
    out.push_str("#include \"tree_sitter/parser.h\"\n#include <stdbool.h>\n#include <stdlib.h>\n#include <string.h>\n\n");
    out.push_str("enum TokenType {\n");
    for name in plan.externals() {
        out.push_str(&format!("  {},\n", enum_name(&name)));
    }
    out.push_str("};\n\n");
    let has_variants = !plan.variants.is_empty();
    let has_indent = plan.indent.is_some();

    out.push_str("static bool is_layout(int32_t c) {\n  return ");
    if plan.layout_chars.is_empty() {
        out.push_str("false");
    } else {
        let parts: Vec<String> = plan
            .layout_chars
            .iter()
            .map(|c| format!("c == {}", c_char(*c)))
            .collect();
        out.push_str(&parts.join(" || "));
    }
    out.push_str(";\n}\n\n");
    out.push_str("static bool is_break(int32_t c) {\n  return c == '\\n' || c == '\\r';\n}\n\n");
    out.push_str(&format!(
        "// The character that opens a line comment in LAYOUT, or 0.\nstatic const int32_t COMMENT_OPEN = {};\n\n",
        plan.comment_open.map(c_char).unwrap_or_else(|| "0".into())
    ));

    if has_variants {
        out.push_str("typedef struct {\n  int32_t ch;\n  bool before_req, before_forbid, after_req, after_forbid;\n  int32_t closer;\n  enum TokenType sym;\n} Variant;\n\n");
        out.push_str("// Most specific first; the default for a spelling is last.\nstatic const Variant VARIANTS[] = {\n");
        for v in &plan.variants {
            let ch = v.spelling.chars().next().unwrap_or('\0');
            out.push_str(&format!(
                "  {{{}, {}, {}, {}, {}, {}, {}}},\n",
                c_char(ch),
                v.before == Cond::Req,
                v.before == Cond::Forbid,
                v.after == Cond::Req,
                v.after == Cond::Forbid,
                v.closer.map(c_char).unwrap_or_else(|| "0".into()),
                enum_name(&v.name)
            ));
        }
        out.push_str("};\n");
        out.push_str(
            "static const unsigned VARIANT_COUNT = sizeof(VARIANTS) / sizeof(VARIANTS[0]);\n\n",
        );
        out.push_str("static bool ends_token(int32_t c) {\n  return c == 0 || is_break(c) || is_layout(c);\n}\n\n");
    }

    let fname = |suffix: &str| format!("tree_sitter_{language}_external_scanner_{suffix}");
    if has_indent {
        out.push_str(
            r#"// The open block columns, outermost first. Column 0 is always open.
#define MAX_DEPTH 64
typedef struct {
  uint16_t depth;
  uint16_t cols[MAX_DEPTH];
} Indent;

static bool on_stack(const Indent *s, int col) {
  for (unsigned i = 0; i < s->depth; i++) {
    if (s->cols[i] == col) return true;
  }
  return false;
}

// Past a line break: the column of the next line that holds a token,
// looking over blank lines and comment lines, or -1 at end of input. Moves
// the lexer without marking, so the caller's token ends where it chose.
static int next_line_column(TSLexer *lexer) {
  for (;;) {
    int col = 0;
    for (;;) {
      if (lexer->lookahead == ' ') {
        col++;
      } else if (lexer->lookahead == '\t') {
        col++;
      } else {
        break;
      }
      lexer->advance(lexer, false);
    }
    if (lexer->lookahead == 0) return -1;
    if (COMMENT_OPEN && lexer->lookahead == COMMENT_OPEN) {
      while (lexer->lookahead != 0 && !is_break(lexer->lookahead)) lexer->advance(lexer, false);
      if (lexer->lookahead == 0) return -1;
    }
    if (is_break(lexer->lookahead)) {
      if (lexer->lookahead == '\r') lexer->advance(lexer, false);
      if (lexer->lookahead == '\n') lexer->advance(lexer, false);
      continue;
    }
    return col;
  }
}

"#,
        );
        out.push_str(&format!(
            "void *{}(void) {{\n  Indent *s = calloc(1, sizeof(Indent));\n  s->depth = 1;\n  return s;\n}}\n",
            fname("create")
        ));
        out.push_str(&format!(
            "void {}(void *payload) {{ free(payload); }}\n",
            fname("destroy")
        ));
        out.push_str(&format!(
            "unsigned {}(void *payload, char *buffer) {{\n  Indent *s = payload;\n  unsigned n = sizeof(uint16_t) * (1 + s->depth);\n  memcpy(buffer, s, n);\n  return n;\n}}\n",
            fname("serialize")
        ));
        out.push_str(&format!(
            "void {}(void *payload, const char *buffer, unsigned length) {{\n  Indent *s = payload;\n  s->depth = 1;\n  s->cols[0] = 0;\n  if (length >= sizeof(uint16_t)) memcpy(s, buffer, length);\n}}\n\n",
            fname("deserialize")
        ));
    } else {
        out.push_str(&format!(
            "void *{}(void) {{ return NULL; }}\n",
            fname("create")
        ));
        out.push_str(&format!(
            "void {}(void *payload) {{ (void)payload; }}\n",
            fname("destroy")
        ));
        out.push_str(&format!(
            "unsigned {}(void *payload, char *buffer) {{ (void)payload; (void)buffer; return 0; }}\n",
            fname("serialize")
        ));
        out.push_str(&format!("void {}(void *payload, const char *buffer, unsigned length) {{ (void)payload; (void)buffer; (void)length; }}\n\n", fname("deserialize")));
    }

    out.push_str(&format!(
        "bool {}(void *payload, TSLexer *lexer, const bool *valid) {{\n",
        fname("scan")
    ));
    out.push_str(
        r#"  (void)payload;
  // During error recovery every symbol is marked valid (the sentinel is
  // never produced, so seeing it valid is the tell).
  bool recovery = valid[ERROR_SENTINEL];

  bool space_before = false;
  while (is_layout(lexer->lookahead) && !is_break(lexer->lookahead)) {
    lexer->advance(lexer, true);
    space_before = true;
  }
  int32_t c = lexer->lookahead;
"#,
    );
    if has_indent {
        out.push_str(
            r#"
  // Block structure by column. `_newline` is the line break that ends a
  // statement; `_indent` is the break that opens a block, and consumes it;
  // `_dedent` is zero-width, one per closed block, at the first token of
  // the line that closed them. A deeper line that opens no block is a
  // continuation and gets no token at all.
  Indent *s = payload;
  bool want_nl = valid[NEWLINE] || recovery;
  bool want_in = valid[INDENT] && !recovery;
  bool want_de = valid[DEDENT] || recovery;
  if (want_nl || want_in || want_de) {
    int top = s->cols[s->depth - 1];
    bool at_break = c == 0 || is_break(c);
    // A comment before the break is an extra the parser wants to see;
    // come back for the break after it.
    if (COMMENT_OPEN && c == COMMENT_OPEN && (want_nl || want_in)) return false;
    lexer->mark_end(lexer);
    int col;
    if (at_break) {
      if (c == '\r') lexer->advance(lexer, false);
      if (lexer->lookahead == '\n') lexer->advance(lexer, false);
      if (want_nl || want_in) lexer->mark_end(lexer);
      col = next_line_column(lexer);
    } else if (COMMENT_OPEN && c == COMMENT_OPEN) {
      // A comment line while only `_dedent` is wanted: decide by the line
      // after it, and leave the comment to the parser.
      while (lexer->lookahead != 0 && !is_break(lexer->lookahead)) lexer->advance(lexer, false);
      if (lexer->lookahead == '\r') lexer->advance(lexer, false);
      if (lexer->lookahead == '\n') lexer->advance(lexer, false);
      col = next_line_column(lexer);
    } else {
      col = (int)lexer->get_column(lexer);
    }
    if (at_break && want_in && col > top && s->depth < MAX_DEPTH) {
      s->cols[s->depth++] = (uint16_t)col;
      lexer->result_symbol = INDENT;
      return true;
    }
    if (at_break && want_nl) {
      if (col > top) return false;  // a continuation line: the offside rule
      lexer->result_symbol = NEWLINE;
      return true;
    }
    if (want_de && s->depth > 1 && col < top) {
      if (col >= 0 && !on_stack(s, col)) {
        // A dedent to a column no open block has: `else` off its `if`.
        lexer->result_symbol = ERROR_SENTINEL;
        return true;
      }
      s->depth--;
      lexer->result_symbol = DEDENT;
      return true;
    }
    if (at_break) return false;
  }
"#,
        );
    }
    if has_variants {
        out.push_str(
            r#"
  unsigned n_valid = 0;
  for (unsigned i = 0; i < VARIANT_COUNT; i++) {
    if (VARIANTS[i].ch == c && (recovery || valid[VARIANTS[i].sym])) n_valid++;
  }
  if (n_valid == 0) return false;

  lexer->advance(lexer, false);
  lexer->mark_end(lexer);
  bool space_after = ends_token(lexer->lookahead);

  for (unsigned i = 0; i < VARIANT_COUNT; i++) {
    const Variant *v = &VARIANTS[i];
    if (v->ch != c || !(recovery || valid[v->sym])) continue;
    // With one candidate the parser has already decided; spacing only
    // arbitrates between several.
    bool ok = n_valid == 1
      || ((!v->before_req || space_before) && (!v->before_forbid || !space_before)
          && (!v->after_req || space_after) && (!v->after_forbid || !space_after));
    if (!ok) continue;
    if (v->closer) {
      // A lexical sort scanned whole: consume to the closer on this line.
      // If it does not close, fall through to the next candidate with the
      // token still ending at the opener.
      while (lexer->lookahead != v->closer && lexer->lookahead != '\n' && lexer->lookahead != 0) {
        lexer->advance(lexer, false);
      }
      if (lexer->lookahead != v->closer) continue;
      lexer->advance(lexer, false);
      lexer->mark_end(lexer);
    }
    lexer->result_symbol = v->sym;
    return true;
  }
  return false;
}
"#,
        );
    } else {
        out.push_str("  (void)space_before;\n  (void)c;\n  return false;\n}\n");
    }
    out
}

fn enum_name(external: &str) -> String {
    external.trim_start_matches('_').to_ascii_uppercase()
}

fn c_char(c: char) -> String {
    match c {
        '\'' => "'\\''".into(),
        '\\' => "'\\\\'".into(),
        '\n' => "'\\n'".into(),
        '\t' => "'\\t'".into(),
        '\r' => "'\\r'".into(),
        c if c.is_ascii_graphic() || c == ' ' => format!("'{c}'"),
        c => format!("{}", c as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spelling_names_are_identifiers() {
        assert_eq!(spelling_name("-"), "minus");
        assert_eq!(spelling_name("["), "lbracket");
        assert_eq!(spelling_name("=="), "eq_eq");
    }

    #[test]
    fn adjacency_and_separation_are_recognised() {
        let c = LayoutRel {
            lhs: LayoutPos {
                symbol: 1,
                end: LayoutEnd::Last,
                axis: LayoutAxis::Col,
            },
            offset: 1,
            op: LayoutOp::Eq,
            rhs: LayoutPos {
                symbol: 2,
                end: LayoutEnd::First,
                axis: LayoutAxis::Col,
            },
        };
        assert_eq!(classify(&LayoutConstraint::Rel(c.clone())), Some((1, true)));
        let sep = LayoutRel {
            op: LayoutOp::Lt,
            ..c.clone()
        };
        assert_eq!(classify(&LayoutConstraint::Rel(sep)), Some((1, false)));
        let skip = LayoutRel {
            rhs: LayoutPos {
                symbol: 3,
                end: LayoutEnd::First,
                axis: LayoutAxis::Col,
            },
            ..c
        };
        assert_eq!(classify(&LayoutConstraint::Rel(skip)), None);
    }
}
