//! A third backend: the same SDF3 module as a scannerless parser written
//! with winnow, emitted as a Rust crate.
//!
//! tree-sitter and ANTLR both lex first and parse second, and every
//! capability difference the first two backends measured came from that
//! split: the lexer cannot ask the parser what is valid, `>>` is one
//! token, spacing decisions need a scanner or a variant. SDF3 is
//! scannerless -- its semantics are stated over characters, with `LAYOUT?`
//! between every two context-free symbols -- and winnow lets a parser be
//! written that way directly: a literal is matched where the grammar puts
//! it, a lexical sort is a parser over characters with its restrictions as
//! negative lookahead, and a layout constraint is a check on positions the
//! parser already has. So this backend is the one closest to the source,
//! and where it disagrees with the other two the disagreement is the
//! measurement.
//!
//! The mapping:
//!
//! - A **lexical sort** becomes a winnow parser over `&str`: character
//!   classes are `one_of`/`none_of`, literals `literal`, repetition
//!   `repeat`, alternatives `alt`. A **lexical restriction** `-/-` is
//!   `not(one_of(..))` after it, which is exactly what it says. A keyword
//!   `{reject}` is a check on the matched text.
//! - **LAYOUT** is skipped before every context-free symbol; a LAYOUT
//!   production that is not a character class is a comment, recorded as
//!   an extra with the name the tree-sitter lowering gave it.
//! - A **sort** becomes a parser function trying its productions in order
//!   (`{prefer}` first, `{avoid}` last, source order between), returning
//!   the same node the tree-sitter lowering names. A production whose first
//!   symbol is its own sort is an infix or postfix operator and goes into a
//!   precedence-climbing loop driven by the priority levels the tree-sitter
//!   lowering assigned; one whose last symbol is its own sort is a prefix
//!   operator. `{non-assoc}` is exact here: the loop refuses a second
//!   operator of the same group, where both other backends widened.
//! - A **layout constraint** is checked in place: a relational one after
//!   the later of its two symbols, `indent`/`align` after the aligned
//!   symbol, `align-list` inside the list's loop, `offside` as a column
//!   limit the layout skipper enforces when it crosses a line break. No
//!   variants, no scanner, no token queue.
//! - `keyword = case-insensitive` is `Caseless`.
//! - An **injection** returns the injected node; a **bracket** production
//!   returns the `_bracket` node the tree-sitter lowering deviated to, so the
//!   one corpus serves all three backends.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};

use crate::ast::*;
use crate::lower::{snake, Finding, Kind, Names};

pub struct Emitted {
    /// `src/main.rs` of the generated crate.
    pub source: String,
    /// `Cargo.toml` of the generated crate.
    pub cargo_toml: String,
    pub findings: Vec<Finding>,
}

/// What a production's shape is with respect to its own sort.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// First symbol is the sort: an infix or postfix operator.
    Infix,
    /// Last symbol is the sort and the first is not: a prefix operator.
    Prefix,
    /// Neither: a primary.
    Primary,
}

struct Cx<'m> {
    module: &'m Module,
    names: &'m Names,
    levels: &'m BTreeMap<String, (u32, Option<Attr>)>,
    lexical: BTreeMap<&'m str, Vec<&'m Production>>,
    /// Lexical restrictions: sort -> classes that may not follow.
    follow: BTreeMap<&'m str, Vec<&'m CharClass>>,
    /// Sorts whose text may not be a keyword.
    keyword_reject: BTreeSet<&'m str>,
    /// Word-shaped literals of context-free syntax.
    keywords: BTreeSet<String>,
    ci: bool,
    kw_follow: Option<&'m CharClass>,
    /// Productions the tree-sitter lowering ends with a hidden newline
    /// token: a trailing comment on their line is inside them there, so
    /// their reach for extras runs to the end of the line here.
    terminated: BTreeSet<usize>,
    findings: Vec<Finding>,
    out: String,
}

pub fn emit(
    module: &Module,
    names: &Names,
    levels: &BTreeMap<String, (u32, Option<Attr>)>,
) -> Result<Emitted> {
    let mut lexical: BTreeMap<&str, Vec<&Production>> = BTreeMap::new();
    for p in module.productions(true) {
        lexical.entry(&p.sort).or_default().push(p);
    }
    let mut follow: BTreeMap<&str, Vec<&CharClass>> = BTreeMap::new();
    for r in module.restrictions(true) {
        for s in &r.symbols {
            for la in &r.lookaheads {
                if la.len() == 1 {
                    follow.entry(s.as_str()).or_default().push(&la[0]);
                }
            }
        }
    }
    let (plan, _) = crate::scanner::plan(module)?;
    let mut cx = Cx {
        module,
        names,
        levels,
        lexical,
        follow,
        keyword_reject: BTreeSet::new(),
        keywords: BTreeSet::new(),
        ci: false,
        kw_follow: None,
        terminated: plan
            .indent
            .as_ref()
            .map(|i| i.terminated.clone())
            .unwrap_or_default(),
        findings: Vec::new(),
        out: String::new(),
    };
    if !cx.terminated.is_empty() {
        cx.findings.push(Finding {
            kind: Kind::Mapped,
            what: format!("{} production(s) end with a hidden newline in the tree-sitter lowering, which puts a trailing comment inside them; here their reach for extras runs to the end of their last line, so the trees agree", cx.terminated.len()),
        });
    }
    for opt in module.template_options() {
        match opt {
            TemplateOption::KeywordReject { sort } => {
                cx.keyword_reject.insert(sort);
            }
            TemplateOption::KeywordFollow(c) => cx.kw_follow = Some(c),
            TemplateOption::KeywordCaseInsensitive => cx.ci = true,
            TemplateOption::Tokenize(_) => {}
        }
    }
    for p in module.productions(false) {
        for s in p.symbols() {
            if let SymRef::Lit(l) = s {
                if is_word(l) {
                    cx.keywords.insert(l.to_string());
                }
            }
        }
    }
    cx.findings.push(Finding {
        kind: Kind::Mapped,
        what: "scannerless: literals and lexical sorts are matched where the grammar puts them, with LAYOUT skipped before every context-free symbol, as SDF3 defines the language; no token stream exists to disagree with the parser".into(),
    });
    if cx.ci {
        cx.findings.push(Finding {
            kind: Kind::Mapped,
            what: "`keyword = case-insensitive` became `Caseless` on every word-shaped literal, and the keyword rejection compares case-insensitively".into(),
        });
    }
    if cx.kw_follow.is_none() && !cx.keywords.is_empty() {
        cx.findings.push(Finding {
            kind: Kind::Widening,
            what: "no `keyword -/- [class]`: a keyword literal may be immediately followed by a letter, so `letx` reads as `let x`, as SDF3 says without the restriction".into(),
        });
    }

    cx.prelude();
    cx.layout()?;
    cx.lexical_sorts()?;
    cx.context_free()?;
    cx.driver()?;

    let cargo_toml = format!(
        "# GENERATED by treebank-sdf3's winnow backend from {}.sdf3; regenerated by the spike's verify.sh.\n[package]\nname = \"{}_winnow\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\nwinnow = \"1\"\n\n# Its own root: the crate lives under the treebank workspace tree without being a member.\n[workspace]\n",
        module.name,
        module.symbol_name()
    );
    Ok(Emitted {
        source: cx.out,
        cargo_toml,
        findings: cx.findings,
    })
}

fn is_word(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn rust_str(s: &str) -> String {
    format!("{s:?}")
}

fn rust_char(c: char) -> String {
    format!("{c:?}")
}

/// A character class as a `Fn(char) -> bool`, which winnow accepts as a
/// token set.
fn class_fn(c: &CharClass) -> String {
    if c.ranges.is_empty() {
        return if c.negated {
            "|_c: char| true".into()
        } else {
            "|_c: char| false".into()
        };
    }
    let arms: Vec<String> = c
        .ranges
        .iter()
        .map(|(a, b)| {
            if a == b {
                rust_char(*a)
            } else {
                format!("{}..={}", rust_char(*a), rust_char(*b))
            }
        })
        .collect();
    let m = format!("matches!(c, {})", arms.join(" | "));
    if c.negated {
        format!("|c: char| !{m}")
    } else {
        format!("|c: char| {m}")
    }
}

fn ident(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    out
}

impl<'m> Cx<'m> {
    fn line(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn prelude(&mut self) {
        let module = self.module.name.clone();
        self.line(&format!(
            "// GENERATED from {module}.sdf3 by treebank-sdf3's winnow backend. Do not edit."
        ));
        self.line(PRELUDE);
    }

    /// `LAYOUT` productions: whitespace classes skipped, comments recorded.
    fn layout(&mut self) -> Result<()> {
        let prods: Vec<&Production> = self.lexical.get("LAYOUT").cloned().unwrap_or_default();
        let mut arms = Vec::new();
        let mut comment_n = 0;
        for p in &prods {
            let is_class = matches!(&p.rhs, Rhs::Symbols(s) if s.len() == 1 && matches!(s[0], Symbol::CharClass(_)));
            let body = self.lexical_body(p)?;
            if is_class {
                arms.push(format!(
                    "        if let Ok(()) = run(({body}).void(), i) {{ progressed = true; continue; }}"
                ));
            } else {
                comment_n += 1;
                let name = if comment_n == 1 {
                    "comment".to_string()
                } else {
                    format!("comment_{comment_n}")
                };
                arms.push(format!(
                    "        {{ let s = pos(i); if let Ok(()) = run(({body}).void(), i) {{ let e = pos(i); i.state.comments.insert(s, (e, {})); progressed = true; continue; }} }}",
                    rust_str(&name)
                ));
            }
        }
        if comment_n > 0 {
            self.findings.push(Finding {
                kind: Kind::Mapped,
                what: format!("{comment_n} comment LAYOUT production(s) are recorded as extras when the layout skipper consumes them, and attached after the parse to the innermost node whose span holds them; tree-sitter attaches an extra to the node being reduced, which differs when a hidden token follows the comment"),
            });
        }
        self.line("fn layout(i: &mut In) -> ModalResult<()> {");
        self.line("    let before = pos(i);");
        self.line("    loop {");
        self.line("        let mut progressed = false;");
        for a in &arms {
            self.line(a);
        }
        self.line("        if !progressed { break; }");
        self.line("    }");
        self.line("    let after = pos(i);");
        self.line("    if let Some(&limit) = i.state.offside.last() {");
        self.line("        if i.state.src[before..after].contains('\\n') && after < i.state.src.len() && col(i, after) <= limit {");
        self.line("            return Err(bt());");
        self.line("        }");
        self.line("    }");
        self.line("    Ok(())");
        self.line("}");
        Ok(())
    }

    fn lexical_body(&self, p: &Production) -> Result<String> {
        let Rhs::Symbols(syms) = &p.rhs else {
            bail!("lexical syntax for {} uses a template; unsupported", p.sort)
        };
        self.lexical_seq(syms)
    }

    fn lexical_seq(&self, syms: &[Symbol]) -> Result<String> {
        let parts: Vec<String> = syms
            .iter()
            .map(|s| self.lexical_symbol(s))
            .collect::<Result<_>>()?;
        Ok(format!("({},).void()", parts.join(", ")))
    }

    fn lexical_symbol(&self, s: &Symbol) -> Result<String> {
        Ok(match s {
            Symbol::CharClass(c) => {
                if c.negated {
                    format!(
                        "none_of({})",
                        class_fn(&CharClass {
                            negated: false,
                            ranges: c.ranges.clone()
                        })
                    )
                } else {
                    format!("one_of({})", class_fn(c))
                }
            }
            Symbol::Lit(l) => format!("literal({})", rust_str(l)),
            Symbol::Sort(name) => {
                if !self.lexical.contains_key(name.as_str()) {
                    bail!("lexical sort {name} referenced but not defined");
                }
                format!("lxb_{}", ident(name))
            }
            Symbol::Star(i) => format!("star({})", self.lexical_symbol(i)?),
            Symbol::Plus(i) => format!("plus({})", self.lexical_symbol(i)?),
            Symbol::Opt(i) => format!("optional({})", self.lexical_symbol(i)?),
            Symbol::Group(alts) => {
                let inner: Vec<String> = alts
                    .iter()
                    .map(|a| self.lexical_seq(a))
                    .collect::<Result<_>>()?;
                if inner.len() == 1 {
                    inner[0].clone()
                } else {
                    format!("alt(({},))", inner.join(", "))
                }
            }
            Symbol::SepList { .. } => bail!("a separated list in lexical syntax is unsupported"),
        })
    }

    fn lexical_sorts(&mut self) -> Result<()> {
        let sorts: Vec<&str> = self
            .lexical
            .keys()
            .copied()
            .filter(|s| *s != "LAYOUT")
            .collect();
        for sort in sorts {
            let prods = self.lexical[sort].clone();
            let keep: Vec<&Production> = prods
                .iter()
                .copied()
                .filter(|p| !p.has(&Attr::Reject))
                .collect();
            let mut rejects: Vec<String> = Vec::new();
            for p in prods.iter().filter(|p| p.has(&Attr::Reject)) {
                match &p.rhs {
                    Rhs::Symbols(s) if s.len() == 1 => {
                        if let Symbol::Lit(w) = &s[0] {
                            rejects.push(w.clone());
                            continue;
                        }
                    }
                    _ => {}
                }
                self.findings.push(Finding {
                    kind: Kind::Unsupported,
                    what: format!("a reject production on {sort} that is not a single literal has no form here"),
                });
            }
            if self.keyword_reject.contains(sort) {
                rejects.extend(self.keywords.iter().cloned());
            }
            if keep.is_empty() {
                continue;
            }
            let alts: Vec<String> = keep
                .iter()
                .map(|p| self.lexical_body(p))
                .collect::<Result<_>>()?;
            let body = if alts.len() == 1 {
                alts[0].clone()
            } else {
                format!("alt(({},))", alts.join(", "))
            };
            let id = ident(sort);
            self.line(&format!(
                "fn lxb_{id}(i: &mut In) -> ModalResult<()> {{ run({body}, i) }}"
            ));
            // The token: body, restrictions, rejection, node.
            let kind = self
                .names
                .sort_rule
                .get(sort)
                .cloned()
                .unwrap_or_else(|| format!("_{}", snake(sort)));
            let mut checks = String::new();
            if let Some(classes) = self.follow.get(sort) {
                for c in classes {
                    checks.push_str(&format!("    run(not(one_of({})), i)?;\n", class_fn(c)));
                }
            }
            let reject_list: Vec<String> = rejects.iter().map(|w| rust_str(w)).collect();
            let cmp = if self.ci { "eq_ci" } else { "eq_cs" };
            self.line(&format!(
                "fn lx_{id}(i: &mut In) -> ModalResult<Node> {{\n    let start = pos(i);\n    i.state.furthest = i.state.furthest.max(start);\n    lxb_{id}(i)?;\n{checks}    let end = pos(i);\n    let text = &i.state.src[start..end];\n    const REJECT: &[&str] = &[{}];\n    if REJECT.iter().any(|k| {cmp}(k, text)) {{ return Err(bt()); }}\n    token_end(i, end);\n    Ok(Node::leaf({}, start, end))\n}}",
                reject_list.join(", "),
                rust_str(&kind)
            ));
            if !rejects.is_empty() {
                self.findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!("{sort} rejects [{}]: the matched text is compared against the list, which is SDF3's reject production exactly", rejects.join(", ")),
                });
            }
        }
        Ok(())
    }

    fn cons_name(&self, p: &Production) -> String {
        if p.has(&Attr::Bracket) {
            format!("{}_bracket", snake(&p.sort))
        } else if let Some(r) = p.reference() {
            self.names
                .node
                .get(&r)
                .cloned()
                .unwrap_or_else(|| snake(p.constructor.as_deref().unwrap_or(&p.sort)))
        } else {
            format!("_inj_{}", snake(&p.sort))
        }
    }

    fn shape(&self, p: &Production) -> Shape {
        let syms = p.symbols();
        let first = syms.first();
        let last = syms.last();
        let is_self = |s: &SymRef| matches!(s, SymRef::Sym(Symbol::Sort(n)) if *n == p.sort);
        if first.is_some_and(is_self) {
            Shape::Infix
        } else if last.is_some_and(is_self) && syms.len() > 1 {
            Shape::Prefix
        } else {
            Shape::Primary
        }
    }

    fn level(&self, p: &Production) -> (u32, Option<Attr>) {
        p.reference()
            .and_then(|r| self.levels.get(&r).cloned())
            .unwrap_or((0, None))
    }

    /// `(label, symbol)` per symbol position, template layout dropped.
    fn labelled(p: &Production) -> Vec<(Option<String>, Symbol)> {
        match &p.rhs {
            Rhs::Symbols(s) => s.iter().map(|s| (None, s.clone())).collect(),
            Rhs::Template(parts) => parts
                .iter()
                .filter_map(|part| match part {
                    TemplatePart::Lit(l) => Some((None, Symbol::Lit(l.clone()))),
                    TemplatePart::Placeholder { label, symbol } => {
                        Some((label.clone(), symbol.clone()))
                    }
                    TemplatePart::Layout(_) => None,
                })
                .collect(),
        }
    }

    fn context_free(&mut self) -> Result<()> {
        let mut cf: BTreeMap<&str, Vec<(usize, &Production)>> = BTreeMap::new();
        let mut order: Vec<&str> = Vec::new();
        for (pi, p) in self.module.productions(false).enumerate() {
            if !cf.contains_key(p.sort.as_str()) {
                order.push(&p.sort);
            }
            cf.entry(&p.sort).or_default().push((pi, p));
        }
        for sort in order {
            let mut prods = cf[sort].clone();
            // prefer first, avoid last, source order between.
            prods.sort_by_key(|(pi, p)| {
                let class = if p.has(&Attr::Prefer) {
                    0
                } else if p.has(&Attr::Avoid) {
                    2
                } else {
                    1
                };
                (class, *pi)
            });
            for (_, p) in &prods {
                if p.has(&Attr::Prefer) || p.has(&Attr::Avoid) {
                    self.findings.push(Finding {
                        kind: Kind::Mapped,
                        what: format!(
                            "{}: `{{{}}}` became ordered choice within `{sort}`; a PEG takes the first alternative that matches, so an ambiguity decided in an ancestor sort follows that sort's source order, which the attribute does not reach",
                            p.display(),
                            if p.has(&Attr::Prefer) { "prefer" } else { "avoid" }
                        ),
                    });
                }
            }
            let sid = ident(sort);
            let shapes: Vec<Shape> = prods.iter().map(|(_, p)| self.shape(p)).collect();
            let has_ops = shapes.contains(&Shape::Infix);
            // Operators, highest level first.
            let mut ops: Vec<(usize, &Production, u32, Option<Attr>)> = prods
                .iter()
                .zip(&shapes)
                .filter(|(_, s)| **s == Shape::Infix)
                .map(|((pi, p), _)| {
                    let (l, a) = self.level(p);
                    (*pi, *p, l, a)
                })
                .collect();
            ops.sort_by_key(|(pi, _, l, _)| (std::cmp::Reverse(*l), *pi));
            // Primaries and prefixes, in the ordered-choice order.
            let heads: Vec<(usize, &Production, Shape)> = prods
                .iter()
                .zip(&shapes)
                .filter(|(_, s)| **s != Shape::Infix)
                .map(|((pi, p), s)| (*pi, *p, *s))
                .collect();

            // The sort's parser.
            self.line(&format!(
                "fn r_{sid}(i: &mut In) -> ModalResult<Node> {{ r_{sid}_prec(i, 0) }}"
            ));
            self.line(&format!(
                "fn r_{sid}_prec(i: &mut In, min: u32) -> ModalResult<Node> {{"
            ));
            self.line("    layout(i)?;");
            self.line("    let start = pos(i);");
            self.line("    let cp = save(i);");
            self.line("    // Longest match among the primaries: ordered choice would let an");
            self.line("    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the");
            self.line("    // first in prefer/source order.");
            self.line("    let mut best: Option<(usize, Node, Cp)> = None;");
            for (pi, _, _) in &heads {
                self.line(&format!("    restore(i, &cp); if let Ok(n) = c_{pi}(i) {{ let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) {{ best = Some((e, n, save(i))); }} }}"));
            }
            self.line("    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };");
            if has_ops {
                self.line("    #[allow(unused_assignments)]");
                self.line("    let mut block: Option<u32> = None;");
                self.line("    loop {");
                for (pi, _, l, a) in &ops {
                    let nonassoc = matches!(a, Some(Attr::NonAssoc));
                    self.line(&format!(
                        "        if {l} >= min && block != Some({l}) {{ let cp = save(i); match t_{pi}(i, &left, start) {{ Ok(n) => {{ left = n; block = if {nonassoc} {{ Some({l}) }} else {{ None }}; continue; }} Err(_) => restore(i, &cp) }} }}"
                    ));
                }
                self.line("        break;");
                self.line("    }");
                self.line("    let _ = block;");
            }
            self.line("    let _ = min;");
            self.line("    Ok(left)");
            self.line("}");

            for (pi, p, shape) in &heads {
                self.production(*pi, p, *shape, None)?;
            }
            for (pi, p, l, a) in &ops {
                self.production(*pi, p, Shape::Infix, Some((*l, a.clone())))?;
            }
            for (_, p, _, a) in &ops {
                if matches!(a, Some(Attr::NonAssoc)) {
                    self.findings.push(Finding {
                        kind: Kind::Mapped,
                        what: format!("{} is non-assoc, and the precedence loop refuses a second operator of its group after it: `a == b == c` is a syntax error, as SDF3 says, where tree-sitter and ANTLR widened to left", p.display()),
                    });
                }
            }
        }
        Ok(())
    }

    /// One production as a parser function. A `Shape::Infix` production is
    /// a tail parser `t_<pi>(i, left, start)` whose first symbol is already
    /// parsed; the others are `c_<pi>(i)`.
    fn production(
        &mut self,
        pi: usize,
        p: &Production,
        shape: Shape,
        level: Option<(u32, Option<Attr>)>,
    ) -> Result<()> {
        let syms = Self::labelled(p);
        let sid = ident(&p.sort);
        let kind = self.cons_name(p);
        let injection = p.constructor.is_none()
            && !p.has(&Attr::Bracket)
            && syms.len() == 1
            && matches!(syms[0].1, Symbol::Sort(_));
        let own_level = self.level(p).0;
        let (name, signature) = match shape {
            Shape::Infix => (
                format!("t_{pi}"),
                format!("fn t_{pi}(i: &mut In, left: &Node, start: usize) -> ModalResult<Node>"),
            ),
            _ => (
                format!("c_{pi}"),
                format!("fn c_{pi}(i: &mut In) -> ModalResult<Node>"),
            ),
        };
        let _ = &name;
        // Wrapper that restores the offside stack on any exit.
        self.line(&format!("{signature} {{"));
        self.line("    let guard = i.state.offside.len();");
        let call = match shape {
            Shape::Infix => format!("{}_body(i, left, start)", name),
            _ => format!("{}_body(i)", name),
        };
        self.line(&format!("    let r = {call};"));
        self.line("    i.state.offside.truncate(guard);");
        self.line("    r");
        self.line("}");
        self.line(&format!("{} {{", signature.replace("(i:", "_body(i:")));
        self.line("    #[allow(unused_mut)]");
        self.line("    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();");
        self.line("    #[allow(unused_mut)]");
        self.line("    let mut sp: Vec<(usize, usize)> = Vec::new();");
        self.line("    #[allow(unused_mut)]");
        self.line("    let mut pr: Vec<bool> = Vec::new();");
        if shape != Shape::Infix {
            self.line("    layout(i)?;");
            self.line("    let start = pos(i);");
        }
        // Which symbols get an offside limit pushed before them, and which
        // constraints are checked after which symbol.
        let mut offside_at: BTreeSet<usize> = BTreeSet::new();
        let mut align_list: BTreeSet<usize> = BTreeSet::new();
        let mut checks_after: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for c in p.layout_constraints() {
            match c {
                LayoutConstraint::Rel(r) => {
                    let after = r.lhs.symbol.max(r.rhs.symbol);
                    let lhs = pos_expr(&r.lhs);
                    let rhs = pos_expr(&r.rhs);
                    let op = match r.op {
                        LayoutOp::Eq => "==",
                        LayoutOp::Lt => "<",
                        LayoutOp::Gt => ">",
                    };
                    checks_after.entry(after).or_default().push(format!(
                        "if pr[{}] && pr[{}] && !(({lhs}) + ({}) {op} ({rhs})) {{ return Err(bt()); }}",
                        r.lhs.symbol - 1,
                        r.rhs.symbol - 1,
                        r.offset
                    ));
                }
                LayoutConstraint::Decl(d) => match d.kind {
                    LayoutDeclKind::Indent | LayoutDeclKind::Align => {
                        let Some(&a) = d.refs.first() else { continue };
                        let op = if d.kind == LayoutDeclKind::Indent {
                            ">"
                        } else {
                            "=="
                        };
                        for &b in &d.refs[1..] {
                            checks_after.entry(a.max(b)).or_default().push(format!(
                                "if pr[{}] && pr[{}] && !(col(i, sp[{}].0) {op} col(i, sp[{}].0)) {{ return Err(bt()); }}",
                                a - 1,
                                b - 1,
                                b - 1,
                                a - 1
                            ));
                        }
                    }
                    LayoutDeclKind::AlignList => {
                        for &a in &d.refs {
                            align_list.insert(a);
                        }
                    }
                    LayoutDeclKind::Offside => {
                        if let Some(&a) = d.refs.first() {
                            offside_at.insert(a);
                        }
                    }
                    LayoutDeclKind::NewlineIndent | LayoutDeclKind::SingleLine => {
                        self.findings.push(Finding {
                            kind: Kind::Unsupported,
                            what: format!(
                                "{}: `newline-indent`/`single-line` is not lowered here",
                                p.display()
                            ),
                        });
                    }
                },
            }
        }
        if p.layout_constraints().next().is_some() {
            self.findings.push(Finding {
                kind: Kind::Mapped,
                what: format!("{}: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state", p.display()),
            });
        }
        for (k, (label, sym)) in syms.iter().enumerate() {
            let position = k + 1;
            let label_expr = match label {
                Some(l) => format!("Some({})", rust_str(l)),
                None => "None".into(),
            };
            if shape == Shape::Infix && k == 0 {
                self.line("    sp.push((left.start, left.end)); pr.push(true);");
                self.line(&format!("    ch.push(({label_expr}, left.clone()));"));
                continue;
            }
            self.line("    layout(i)?;");
            if offside_at.contains(&position) {
                self.line("    { let c = col(i, pos(i)); i.state.offside.push(c); }");
            }
            self.line("    { let s = pos(i);");
            // The operand of a prefix operator, or the right operand of an
            // infix one, parses at the level the priorities say.
            let is_operand = matches!(sym, Symbol::Sort(n) if *n == p.sort)
                && ((shape == Shape::Prefix && k == syms.len() - 1)
                    || (shape == Shape::Infix && k == syms.len() - 1));
            let expr = if is_operand {
                let min = match (shape, &level) {
                    (Shape::Infix, Some((l, Some(Attr::Right)))) => *l,
                    (Shape::Infix, Some((l, _))) => l + 1,
                    (Shape::Prefix, _) => own_level,
                    _ => 0,
                };
                format!("vec![r_{sid}_prec(i, {min})?]")
            } else {
                self.symbol_expr(sym, align_list.contains(&position))?
            };
            self.line(&format!("      let ns: Vec<Node> = {expr};"));
            self.line("      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);");
            self.line(&format!(
                "      for n in ns {{ ch.push(({label_expr}, n)); }} }}"
            ));
            if let Some(checks) = checks_after.get(&position) {
                for c in checks {
                    self.line(&format!("    {c}"));
                }
            }
        }
        self.line("    let _ = (&sp, &pr);");
        if injection {
            self.line("    let (_, n) = ch.pop().unwrap();");
            self.line("    Ok(n)");
        } else {
            let reach = if self.terminated.contains(&pi) {
                "line_end(i, end)"
            } else {
                "end"
            };
            self.line(&format!(
                "    let end = i.state.last_end.max(start);\n    Ok(Node {{ kind: {}, start, end, reach: {reach}, children: ch }})",
                rust_str(&kind)
            ));
        }
        self.line("}");
        Ok(())
    }

    /// The parse of one context-free symbol occurrence, as an expression
    /// of type `Vec<Node>` (a literal yields none, a list several).
    fn symbol_expr(&mut self, sym: &Symbol, aligned: bool) -> Result<String> {
        Ok(match sym {
            Symbol::Lit(l) => {
                let word = is_word(l);
                let m = if self.ci && word {
                    format!("literal(Caseless({}))", rust_str(l))
                } else {
                    format!("literal({})", rust_str(l))
                };
                let follow = match (&self.kw_follow, word) {
                    (Some(c), true) => format!("run(not(one_of({})), i)?;", class_fn(c)),
                    _ => String::new(),
                };
                format!("{{ i.state.furthest = i.state.furthest.max(pos(i)); run({m}, i)?; {follow} let e = pos(i); token_end(i, e); Vec::new() }}")
            }
            Symbol::Sort(n) => {
                if self.lexical.contains_key(n.as_str()) {
                    format!("vec![lx_{}(i)?]", ident(n))
                } else if self.names.sort_rule.contains_key(n)
                    || self.module.productions(false).any(|p| p.sort == *n)
                {
                    format!("vec![r_{}(i)?]", ident(n))
                } else {
                    bail!("reference to undefined sort {n}")
                }
            }
            Symbol::Opt(inner) => {
                let e = self.symbol_expr(inner, false)?;
                format!("{{ let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {{ layout(i)?; Ok({e}) }})(i) {{ Ok(v) => v, Err(_) => {{ restore(i, &cp); Vec::new() }} }} }}")
            }
            Symbol::Star(inner) | Symbol::Plus(inner) => {
                let e = self.symbol_expr(inner, false)?;
                let min = if matches!(sym, Symbol::Plus(_)) { 1 } else { 0 };
                let col_check = if aligned {
                    "let c = col(i, pos(i)); match col0 { None => col0 = Some(c), Some(c0) if c != c0 => { restore(i, &cp); break; } _ => {} }"
                } else {
                    ""
                };
                format!(
                    "{{ let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop {{ let cp = save(i); if layout(i).is_err() {{ restore(i, &cp); break; }} {col_check} match (|i: &mut In| -> ModalResult<Vec<Node>> {{ Ok({e}) }})(i) {{ Ok(ns) => v.extend(ns), Err(_) => {{ restore(i, &cp); break; }} }} }} if v.len() < {min} {{ return Err(bt()); }} v }}"
                )
            }
            Symbol::SepList { elem, sep, min } => {
                let e = self.symbol_expr(elem, false)?;
                let s = self.symbol_expr(sep, false)?;
                let col_check = if aligned {
                    "let c = col(i, pos(i)); match col0 { None => col0 = Some(c), Some(c0) if c != c0 => { restore(i, &cp); break; } _ => {} }"
                } else {
                    ""
                };
                format!(
                    "{{ let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop {{ let cp = save(i); if !first {{ if layout(i).is_err() {{ restore(i, &cp); break; }} if (|i: &mut In| -> ModalResult<Vec<Node>> {{ Ok({s}) }})(i).is_err() {{ restore(i, &cp); break; }} }} if layout(i).is_err() {{ restore(i, &cp); break; }} {col_check} match (|i: &mut In| -> ModalResult<Vec<Node>> {{ Ok({e}) }})(i) {{ Ok(ns) => {{ v.extend(ns); first = false; }} Err(_) => {{ restore(i, &cp); break; }} }} }} if v.len() < {min} {{ return Err(bt()); }} v }}"
                )
            }
            Symbol::Group(alts) => {
                let mut arms = Vec::new();
                for a in alts {
                    let mut body = String::from("{ let mut v: Vec<Node> = Vec::new();");
                    for s in a {
                        let e = self.symbol_expr(s, false)?;
                        body.push_str(&format!(" layout(i)?; v.extend({e});"));
                    }
                    body.push_str(" Ok(v) }");
                    arms.push(format!("(|i: &mut In| -> ModalResult<Vec<Node>> {body})"));
                }
                let mut s = String::from("{ let cp = save(i); 'g: {");
                for a in &arms {
                    s.push_str(&format!(
                        " if let Ok(v) = {a}(i) {{ break 'g v; }} restore(i, &cp);"
                    ));
                }
                s.push_str(" return Err(bt()); } }");
                s
            }
            Symbol::CharClass(_) => {
                bail!("a character class in context-free syntax is unsupported")
            }
        })
    }

    fn driver(&mut self) -> Result<()> {
        let starts = self.module.start_symbols();
        let Some(start) = starts.first() else {
            bail!("no context-free start-symbols")
        };
        let sid = ident(start);
        self.line(&format!(
            "fn parse_root(src: &str) -> Result<Node, usize> {{\n    let mut i: In = Stateful {{ input: LocatingSlice::new(src), state: St::new(src) }};\n    let r = (|i: &mut In| -> ModalResult<Node> {{ let n = r_{sid}(i)?; layout(i)?; run(eof, i)?; Ok(n) }})(&mut i);\n    match r {{\n        Ok(mut root) => {{ root.start = 0; root.end = src.len(); root.reach = src.len(); let comments: Vec<(usize, usize, &'static str)> = i.state.comments.iter().map(|(s, (e, n))| (*s, *e, *n)).collect(); for (s, e, n) in comments {{ attach(&mut root, Node::leaf(n, s, e)); }} Ok(root) }}\n        Err(_) => Err(i.state.furthest.max(pos(&i))),\n    }}\n}}"
        ));
        self.line(DRIVER);
        Ok(())
    }
}

fn pos_expr(p: &LayoutPos) -> String {
    let s = p.symbol - 1;
    let at = match p.end {
        LayoutEnd::First => format!("sp[{s}].0"),
        LayoutEnd::Last => format!("sp[{s}].1.saturating_sub(1).max(sp[{s}].0)"),
    };
    match p.axis {
        LayoutAxis::Col => format!("col(i, {at}) as i64"),
        LayoutAxis::Line => format!("line(i, {at}) as i64"),
    }
}

const PRELUDE: &str = r#"
#![allow(dead_code, unused_imports, unused_variables, unused_mut, unused_assignments, clippy::all)]
use std::collections::BTreeMap;
use winnow::prelude::*;
use winnow::ascii::Caseless;
use winnow::combinator::{alt, eof, not, opt, repeat};
use winnow::error::{ContextError, ErrMode};
use winnow::stream::{LocatingSlice, Location, Stateful, Stream};
use winnow::token::{literal, none_of, one_of};
use winnow::ModalResult;

type In<'a> = Stateful<LocatingSlice<&'a str>, St<'a>>;

#[derive(Debug)]
struct St<'a> {
    src: &'a str,
    line_starts: Vec<usize>,
    /// comment start -> (end, node name)
    comments: BTreeMap<usize, (usize, &'static str)>,
    /// offside limits: a token on a later line must sit at a greater column
    offside: Vec<usize>,
    last_end: usize,
    furthest: usize,
}

impl<'a> St<'a> {
    fn new(src: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (k, c) in src.char_indices() {
            if c == '\n' { line_starts.push(k + 1); }
        }
        St { src, line_starts, comments: BTreeMap::new(), offside: Vec::new(), last_end: 0, furthest: 0 }
    }
}

#[derive(Clone, Debug)]
struct Node {
    kind: &'static str,
    start: usize,
    end: usize,
    /// How far the node claims extras: `end`, or the end of its last line
    /// for a production the tree-sitter lowering terminates with a hidden
    /// newline.
    reach: usize,
    children: Vec<(Option<&'static str>, Node)>,
}

impl Node {
    fn leaf(kind: &'static str, start: usize, end: usize) -> Node {
        Node { kind, start, end, reach: end, children: Vec::new() }
    }
}
fn line_end(i: &In, p: usize) -> usize {
    i.state.src[p..].find('\n').map(|k| p + k + 1).unwrap_or(i.state.src.len())
}

fn bt() -> ErrMode<ContextError> { ErrMode::Backtrack(ContextError::new()) }
type Cp<'a> = (<In<'a> as Stream>::Checkpoint, usize);
/// A checkpoint that also remembers the last token end, which winnow's
/// reset would not restore.
fn save<'a>(i: &In<'a>) -> Cp<'a> { (i.checkpoint(), i.state.last_end) }
fn restore<'a>(i: &mut In<'a>, cp: &Cp<'a>) { i.reset(&cp.0); i.state.last_end = cp.1; }
/// Pins the error type of an inline parser expression.
fn run<'a, O>(mut p: impl Parser<In<'a>, O, ErrMode<ContextError>>, i: &mut In<'a>) -> ModalResult<O> { p.parse_next(i) }
fn pos(i: &In) -> usize { i.current_token_start() }
fn token_end(i: &mut In, end: usize) { i.state.last_end = end; i.state.furthest = i.state.furthest.max(end); }
fn line(i: &In, p: usize) -> usize {
    match i.state.line_starts.binary_search(&p) { Ok(l) => l, Err(l) => l - 1 }
}
fn col(i: &In, p: usize) -> usize {
    let l = line(i, p);
    i.state.src[i.state.line_starts[l]..p].chars().count()
}
fn eq_cs(a: &str, b: &str) -> bool { a == b }
fn eq_ci(a: &str, b: &str) -> bool { a.eq_ignore_ascii_case(b) }

fn star<'a, O, P: Parser<In<'a>, O, ErrMode<ContextError>>>(p: P) -> impl Parser<In<'a>, (), ErrMode<ContextError>> {
    repeat::<_, _, (), _, _>(0.., p)
}
fn plus<'a, O, P: Parser<In<'a>, O, ErrMode<ContextError>>>(p: P) -> impl Parser<In<'a>, (), ErrMode<ContextError>> {
    repeat::<_, _, (), _, _>(1.., p)
}
fn optional<'a, O, P: Parser<In<'a>, O, ErrMode<ContextError>>>(p: P) -> impl Parser<In<'a>, (), ErrMode<ContextError>> {
    opt(p).void()
}

/// An extra goes to the innermost node whose span strictly holds it.
fn attach(n: &mut Node, extra: Node) {
    for (_, c) in n.children.iter_mut() {
        if c.start < extra.start && extra.start < c.reach {
            return attach(c, extra);
        }
    }
    let at = n.children.iter().position(|(_, c)| c.start > extra.start).unwrap_or(n.children.len());
    n.children.insert(at, (None, extra));
}

fn sexp(n: &Node, field: Option<&str>, out: &mut String) {
    if n.kind.starts_with('_') {
        for (f, c) in &n.children { sexp(c, f.as_deref().or(field), out); }
        return;
    }
    if !out.is_empty() && !out.ends_with('(') { out.push(' '); }
    if let Some(f) = field { out.push_str(f); out.push_str(": "); }
    out.push('(');
    out.push_str(n.kind);
    for (f, c) in &n.children { sexp(c, f.as_deref(), out); }
    out.push(')');
}
"#;

const DRIVER: &str = r#"
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let src = match args.first() {
        Some(p) => std::fs::read_to_string(p).expect("read"),
        None => { let mut s = String::new(); std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).expect("stdin"); s }
    };
    match parse_root(&src) {
        Ok(root) => { let mut out = String::new(); sexp(&root, None, &mut out); println!("{out}"); }
        Err(at) => {
            let line = src[..at].matches('\n').count() + 1;
            let col = at - src[..at].rfind('\n').map(|k| k + 1).unwrap_or(0);
            println!("ERROR at {line}:{col}");
            std::process::exit(1);
        }
    }
}
"#;
