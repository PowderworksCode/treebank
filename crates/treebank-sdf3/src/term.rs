//! Terms: SDF3's own product, and the imploder that gets them back from a
//! tree-sitter tree.
//!
//! An SDF3 module is a signature. `Exp.Add = <<left:Exp> + <right:Exp>>`
//! declares `Add : Exp * Exp -> Exp`, and the parse of `y + 1` is the term
//! `Add("y", Int("1"))`: literals and layout gone, lexical sorts collapsed
//! to their text, injections and brackets passed through, lists as lists,
//! optionals as `Some`/`None`. Spoofax calls the pass from parse tree to
//! term *implosion*. tree-sitter produces a tree that has already dropped
//! layout and literals, and the lowering chose every name in it, so the
//! same map runs from that side: node name to constructor through
//! [`Names::node`], field to placeholder through the labels, unlabelled
//! children to unlabelled placeholders in order, the bracket deviation
//! node removed, a wrapper around a token collapsed to the token's text.
//!
//! Comments are LAYOUT and have no place in a term; they survive as
//! annotations on the term that follows them on a new line (leading) or
//! that precedes them on the same line (trailing), so a printer can put
//! them back.

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Result};

use crate::ast::*;
use crate::lower::Names;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// A constructor application: `Sort.Cons` and its arguments.
    App {
        sort: String,
        cons: String,
        args: Vec<Term>,
        /// Comments on lines of their own before this term.
        leading: Vec<String>,
        /// A comment on the same line after this term.
        trailing: Option<String>,
        /// A blank line stood before this term in the source, when it is
        /// an element of a list. Formatters keep one; so does the printer.
        blank_before: bool,
    },
    /// A lexical sort's text.
    Str(String),
    List(Vec<Term>),
    Opt(Option<Box<Term>>),
}

impl Term {
    /// The ATerm-style rendering: `Assign("x", Add("y", Int("1")))`.
    pub fn aterm(&self) -> String {
        match self {
            Term::App { cons, args, .. } => {
                let inner: Vec<String> = args.iter().map(Term::aterm).collect();
                format!("{cons}({})", inner.join(", "))
            }
            Term::Str(s) => format!("{s:?}"),
            Term::List(items) => {
                let inner: Vec<String> = items.iter().map(Term::aterm).collect();
                format!("[{}]", inner.join(", "))
            }
            Term::Opt(None) => "None()".into(),
            Term::Opt(Some(t)) => format!("Some({})", t.aterm()),
        }
    }

    /// Structural equality with comments ignored.
    pub fn same_shape(&self, other: &Term) -> bool {
        match (self, other) {
            (
                Term::App { cons: a, args: x, .. },
                Term::App { cons: b, args: y, .. },
            ) => a == b && x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.same_shape(q)),
            (Term::Str(a), Term::Str(b)) => a == b,
            (Term::List(x), Term::List(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.same_shape(q))
            }
            (Term::Opt(None), Term::Opt(None)) => true,
            (Term::Opt(Some(a)), Term::Opt(Some(b))) => a.same_shape(b),
            _ => false,
        }
    }
}

/// A node of tree-sitter's tree, as `tree-sitter parse` prints it.
#[derive(Debug, Clone)]
pub struct Cst {
    pub kind: String,
    pub field: Option<String>,
    pub start: (usize, usize),
    pub end: (usize, usize),
    pub children: Vec<Cst>,
}

/// Read the CLI's S-expression with positions: `(kind [r, c] - [r, c] ...)`
/// with `field: ` prefixes and `(MISSING "x" ...)` nodes.
pub fn parse_sexp(text: &str) -> Result<Cst> {
    let mut chars = text.chars().peekable();
    let mut stack: Vec<Cst> = Vec::new();
    let mut root: Option<Cst> = None;
    let mut field: Option<String> = None;
    while let Some(c) = chars.next() {
        match c {
            '(' => {
                let mut kind = String::new();
                while let Some(&d) = chars.peek() {
                    if d == ' ' || d == ')' {
                        break;
                    }
                    kind.push(d);
                    chars.next();
                }
                if kind == "MISSING" {
                    // `(MISSING "x" [..] - [..])`: skip the quoted text.
                    while let Some(&d) = chars.peek() {
                        if d == '[' {
                            break;
                        }
                        chars.next();
                    }
                }
                let start = read_pos(&mut chars)?;
                skip_until(&mut chars, '[');
                let end = read_pos(&mut chars)?;
                stack.push(Cst {
                    kind,
                    field: field.take(),
                    start,
                    end,
                    children: Vec::new(),
                });
            }
            ')' => {
                let node = stack.pop().ok_or_else(|| anyhow!("unbalanced parse output"))?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut name = String::new();
                name.push(c);
                while let Some(&d) = chars.peek() {
                    if d == ':' {
                        chars.next();
                        break;
                    }
                    if !(d.is_ascii_alphanumeric() || d == '_') {
                        break;
                    }
                    name.push(d);
                    chars.next();
                }
                field = Some(name);
            }
            _ => {}
        }
    }
    root.ok_or_else(|| anyhow!("no tree in parse output"))
}

fn read_pos(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<(usize, usize)> {
    skip_until(chars, '[');
    chars.next();
    let mut nums = Vec::new();
    let mut cur = String::new();
    for d in chars.by_ref() {
        if d.is_ascii_digit() {
            cur.push(d);
        } else if d == ',' || d == ']' {
            if !cur.is_empty() {
                nums.push(cur.parse::<usize>()?);
                cur.clear();
            }
            if d == ']' {
                break;
            }
        }
    }
    if nums.len() != 2 {
        bail!("bad position in parse output");
    }
    Ok((nums[0], nums[1]))
}

fn skip_until(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, stop: char) {
    while let Some(&d) = chars.peek() {
        if d == stop {
            break;
        }
        chars.next();
    }
}

/// What the imploder needs from the module and the lowering: the
/// productions by node name, with their placeholders in order.
pub struct Imploder<'m> {
    pub module: &'m Module,
    /// node name -> production
    by_node: BTreeMap<String, &'m Production>,
    lexical: BTreeMap<String, String>,
    bracket_nodes: Vec<String>,
    comment_nodes: Vec<String>,
}

impl<'m> Imploder<'m> {
    pub fn new(module: &'m Module, names: &Names) -> Self {
        let mut by_node = BTreeMap::new();
        for p in module.productions(false) {
            if let Some(r) = p.reference() {
                if let Some(n) = names.node.get(&r) {
                    by_node.insert(n.clone(), p);
                }
            }
        }
        // token name -> lexical sort
        let mut lexical = BTreeMap::new();
        for s in &names.lexical {
            if let Some(rule) = names.sort_rule.get(s) {
                lexical.insert(rule.clone(), s.clone());
            }
        }
        let bracket_nodes: Vec<String> = module
            .productions(false)
            .filter(|p| p.has(&Attr::Bracket))
            .map(|p| format!("{}_bracket", crate::lower::snake(&p.sort)))
            .collect();
        Imploder {
            module,
            by_node,
            lexical,
            bracket_nodes,
            comment_nodes: vec!["comment".into()],
        }
    }

    pub fn implode(&self, cst: &Cst, source: &str) -> Result<Term> {
        let lines: Vec<&str> = source.split('\n').collect();
        self.node(cst, &lines)
    }

    fn text(&self, n: &Cst, lines: &[&str]) -> String {
        let (r0, c0) = n.start;
        let (r1, c1) = n.end;
        if r0 == r1 {
            return lines.get(r0).map(|l| slice(l, c0, c1)).unwrap_or_default();
        }
        let mut s = slice(lines.get(r0).copied().unwrap_or(""), c0, usize::MAX);
        for l in lines.iter().take(r1).skip(r0 + 1) {
            s.push('\n');
            s.push_str(l);
        }
        s.push('\n');
        s.push_str(&slice(lines.get(r1).copied().unwrap_or(""), 0, c1));
        s
    }

    fn node(&self, n: &Cst, lines: &[&str]) -> Result<Term> {
        if self.lexical.contains_key(&n.kind) {
            return Ok(Term::Str(self.text(n, lines)));
        }
        if self.bracket_nodes.contains(&n.kind) {
            let inner = n
                .children
                .iter()
                .find(|c| !self.comment_nodes.contains(&c.kind))
                .ok_or_else(|| anyhow!("empty bracket node"))?;
            return self.node(inner, lines);
        }
        if n.kind == "ERROR" || n.kind == "MISSING" {
            bail!("the tree has an error at {:?}", n.start);
        }
        let p = self
            .by_node
            .get(&n.kind)
            .ok_or_else(|| anyhow!("no production for node `{}`", n.kind))?;
        let parts = parts_of(p);
        let placeholders: Vec<(&Option<String>, &Symbol)> = parts
            .iter()
            .filter_map(|part| match part {
                TemplatePart::Placeholder { label, symbol } => Some((label, symbol)),
                _ => None,
            })
            .collect();
        // Which children are elements of a list placeholder: a comment
        // beside one of those belongs to the element; any other comment
        // belongs to this term.
        let list_labels: Vec<Option<&str>> = placeholders
            .iter()
            .filter(|(_, s)| is_list(s))
            .map(|(l, _)| l.as_deref())
            .collect();
        let is_element = |c: &Cst| list_labels.contains(&c.field.as_deref());
        // Children, with comments lifted into annotations on their
        // neighbours.
        let mut kids: Vec<(Cst, Vec<String>, Option<String>)> = Vec::new();
        let mut pending: Vec<String> = Vec::new();
        let mut own_leading: Vec<String> = Vec::new();
        let mut own_trailing: Option<String> = None;
        for c in &n.children {
            if self.comment_nodes.contains(&c.kind) {
                let text = self.text(c, lines);
                match kids.last_mut() {
                    Some((prev, _, trailing)) if prev.end.0 == c.start.0 && is_element(prev) && trailing.is_none() => {
                        *trailing = Some(text);
                    }
                    Some((prev, _, _)) if prev.end.0 == c.start.0 && own_trailing.is_none() => {
                        own_trailing = Some(text);
                    }
                    Some(_) => pending.push(text),
                    None => own_leading.push(text),
                }
                continue;
            }
            kids.push((c.clone(), std::mem::take(&mut pending), None));
        }
        if !pending.is_empty() {
            // Comments after the last element belong to that element.
            if let Some(last) = kids.last_mut() {
                last.1.extend(pending);
            }
        }
        let mut args = Vec::new();
        let mut unlabelled: Vec<&(Cst, Vec<String>, Option<String>)> =
            kids.iter().filter(|(c, _, _)| c.field.is_none()).collect();
        // A comment attached to a token child has no term to sit on; it
        // belongs to this term instead.
        let mut leading = own_leading;
        let mut trailing = own_trailing;
        for (label, symbol) in &placeholders {
            let mine: Vec<&(Cst, Vec<String>, Option<String>)> = match label {
                Some(l) => kids.iter().filter(|(c, _, _)| c.field.as_deref() == Some(l.as_str())).collect(),
                None => {
                    let take = if is_list(symbol) { unlabelled.len() } else { usize::from(!unlabelled.is_empty()) };
                    unlabelled.drain(..take.min(unlabelled.len())).collect()
                }
            };
            for k in &mine {
                if self.lexical.contains_key(&k.0.kind) {
                    leading.extend(k.1.iter().cloned());
                    if k.2.is_some() {
                        trailing = k.2.clone();
                    }
                }
            }
            args.push(self.argument(symbol, &mine, lines)?);
        }
        Ok(Term::App {
            sort: p.sort.clone(),
            cons: p.constructor.clone().unwrap_or_default(),
            args,
            leading,
            trailing,
            blank_before: false,
        })
    }

    fn argument(
        &self,
        symbol: &Symbol,
        kids: &[&(Cst, Vec<String>, Option<String>)],
        lines: &[&str],
    ) -> Result<Term> {
        let one = |k: &(Cst, Vec<String>, Option<String>), prev_end: Option<usize>| -> Result<Term> {
            let mut t = self.node(&k.0, lines)?;
            if let Term::App { leading, trailing, blank_before, .. } = &mut t {
                let mut l = k.1.clone();
                l.append(leading);
                *leading = l;
                if k.2.is_some() {
                    *trailing = k.2.clone();
                }
                // A blank line between the previous element and this one's
                // first line (or its first leading comment).
                let first_row = k.0.start.0.saturating_sub(k.1.len());
                *blank_before = prev_end.is_some_and(|e| first_row > e + 1);
            }
            Ok(t)
        };
        match symbol {
            Symbol::Star(_) | Symbol::Plus(_) | Symbol::SepList { .. } => {
                let mut items = Vec::new();
                let mut prev_end: Option<usize> = None;
                for k in kids {
                    items.push(one(k, prev_end)?);
                    prev_end = Some(k.0.end.0);
                }
                Ok(Term::List(items))
            }
            Symbol::Opt(_) => Ok(Term::Opt(match kids.first() {
                Some(k) => Some(Box::new(one(k, None)?)),
                None => None,
            })),
            _ => match kids.first() {
                Some(k) => one(k, None),
                None => bail!("missing child for {symbol:?}"),
            },
        }
    }
}

/// A production's parts for implosion and printing: a template's own, or
/// for the productive form `Exp.Int = INT`, one unlabelled placeholder
/// per symbol with a space of layout between them.
pub fn parts_of(p: &Production) -> Vec<TemplatePart> {
    match &p.rhs {
        Rhs::Template(parts) => parts.clone(),
        Rhs::Symbols(syms) => {
            let mut parts = Vec::new();
            for (i, s) in syms.iter().enumerate() {
                if i > 0 {
                    parts.push(TemplatePart::Layout(" ".into()));
                }
                parts.push(match s {
                    Symbol::Lit(l) => TemplatePart::Lit(l.clone()),
                    other => TemplatePart::Placeholder {
                        label: None,
                        symbol: other.clone(),
                    },
                });
            }
            parts
        }
    }
}

fn is_list(s: &Symbol) -> bool {
    matches!(s, Symbol::Star(_) | Symbol::Plus(_) | Symbol::SepList { .. })
}

fn slice(line: &str, a: usize, b: usize) -> String {
    line.chars().skip(a).take(b.saturating_sub(a)).collect()
}
