//! Pretty-printing from templates, through boxes.
//!
//! An SDF3 template is read twice. The parser reads its literals and
//! placeholders and ignores its whitespace; the pretty-printer reads its
//! whitespace and treats it as the layout to produce. Spoofax lowers each
//! template to Box, the layout language of de Jonge's pretty-print
//! toolkit: `H` boxes place their contents on one line, `V` boxes stack
//! theirs, each with an indentation. This module does the same, with one
//! deliberate difference: a `V` box indents relative to the *line* it
//! started on rather than the column, which is what rustfmt, black and
//! prettier all do, and pp-Box's column alignment (`A` boxes) is not
//! needed for any of those styles.
//!
//! The rules, per template:
//!
//! - The template's lines are the `V` box; the first line's indent is the
//!   base and every other line's indent is relative to it.
//! - Within a line, literals and placeholders are an `H` box; two parts
//!   with layout between them are joined by one space, parts with none are
//!   adjacent. An empty part (an absent optional, an empty list) vanishes
//!   with its glue.
//! - A list placeholder alone on its line prints its elements one per
//!   line at that line's indent; a list inline prints them separated by
//!   the separator literal and a space, or by a space alone.
//! - An optional placeholder alone on its line omits the line when absent.
//! - Leading comments print on lines of their own before the term;
//!   a trailing comment follows the term's last line.
//!
//! So the template's whitespace *is* the style: change the indentation in
//! the module and the printer changes, while the parser does not.

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Result};

use crate::ast::*;
use crate::term::Term;

#[derive(Debug, Clone)]
pub enum Box_ {
    Text(String),
    /// Items on one line; `bool` is whether a space precedes the item.
    H(Vec<(bool, Box_)>),
    /// Lines, each with an indent relative to the box's line.
    V(Vec<(usize, Box_)>),
    /// A blank line inside a `V` box.
    Blank,
    Empty,
}

pub struct Printer<'m> {
    templates: BTreeMap<String, &'m Production>,
    /// Injections `From = To {attrs}`: a printing rule that holds for a
    /// term of sort `To` standing where a `From` is expected.
    injections: Vec<(String, String, &'m Production)>,
    /// The comment opener, for the spacing before a trailing comment.
    comment_open: Option<String>,
}

impl<'m> Printer<'m> {
    pub fn new(module: &'m Module) -> Self {
        let mut templates = BTreeMap::new();
        let mut injections = Vec::new();
        for p in module.productions(false) {
            if let Some(r) = p.reference() {
                templates.insert(r, p);
            } else if !p.has(&Attr::Bracket) {
                if let [SymRef::Sym(Symbol::Sort(to))] = p.symbols()[..] {
                    injections.push((p.sort.clone(), to.clone(), p));
                }
            }
        }
        let comment_open = module.productions(true).find_map(|p| {
            if p.sort != "LAYOUT" {
                return None;
            }
            match &p.rhs {
                Rhs::Symbols(s) => match s.first() {
                    Some(Symbol::Lit(l)) => Some(l.clone()),
                    _ => None,
                },
                _ => None,
            }
        });
        Printer {
            templates,
            injections,
            comment_open,
        }
    }

    pub fn print(&self, t: &Term) -> Result<String> {
        let b = self.boxed(t)?;
        let mut out = String::new();
        render(&b, 0, &mut out);
        Ok(out.trim_end().to_string() + "\n")
    }

    pub fn boxed(&self, t: &Term) -> Result<Box_> {
        match t {
            Term::Str(s) => Ok(Box_::Text(s.clone())),
            Term::Opt(None) => Ok(Box_::Empty),
            Term::Opt(Some(inner)) => self.boxed(inner),
            Term::List(items) => self.vertical(items),
            Term::App {
                sort,
                cons,
                args,
                leading,
                trailing,
                ..
            } => {
                let p = self
                    .templates
                    .get(&format!("{sort}.{cons}"))
                    .ok_or_else(|| anyhow!("no template for {sort}.{cons}"))?;
                let parts = crate::term::parts_of(p);
                let mut b = self.template(&parts, args)?;
                if let Some(width) = p.attrs.iter().find_map(|a| match a {
                    Attr::Collapse(w) => Some(*w),
                    _ => None,
                }) {
                    b = collapse(b, width as usize);
                }
                if let Some(c) = trailing {
                    let gap = if self.comment_open.as_deref() == Some("#") {
                        "  "
                    } else {
                        " "
                    };
                    b = append_text(b, format!("{gap}{c}"));
                }
                if !leading.is_empty() {
                    let mut lines: Vec<(usize, Box_)> =
                        leading.iter().map(|c| (0, Box_::Text(c.clone()))).collect();
                    lines.push((0, b));
                    b = Box_::V(lines);
                }
                Ok(b)
            }
        }
    }

    fn template(&self, parts: &[TemplatePart], args: &[Term]) -> Result<Box_> {
        // Split into lines at newlines in layout parts, keeping each
        // line's indent.
        struct Line {
            indent: usize,
            items: Vec<(bool, Item)>,
        }
        enum Item {
            Lit(String),
            Arg(usize, Symbol),
        }
        let mut lines: Vec<Line> = vec![Line {
            indent: 0,
            items: Vec::new(),
        }];
        let mut space = false;
        let mut arg_i = 0;
        for part in parts {
            match part {
                TemplatePart::Layout(l) => {
                    if let Some(after) = l.rfind('\n') {
                        let indent = l[after + 1..].chars().count();
                        lines.push(Line {
                            indent,
                            items: Vec::new(),
                        });
                        space = false;
                    } else {
                        space = true;
                    }
                }
                TemplatePart::Lit(s) => {
                    lines
                        .last_mut()
                        .unwrap()
                        .items
                        .push((space, Item::Lit(s.clone())));
                    space = false;
                }
                TemplatePart::Placeholder { symbol, .. } => {
                    lines
                        .last_mut()
                        .unwrap()
                        .items
                        .push((space, Item::Arg(arg_i, symbol.clone())));
                    arg_i += 1;
                    space = false;
                }
            }
        }
        // Drop empty first/last lines (the newline after `<` and before `>`).
        lines.retain(|l| !l.items.is_empty());
        let base = lines.first().map(|l| l.indent).unwrap_or(0);
        let mut out_lines: Vec<(usize, Box_)> = Vec::new();
        for line in &lines {
            let rel = line.indent.saturating_sub(base);
            let alone = line.items.len() == 1;
            let mut items: Vec<(bool, Box_)> = Vec::new();
            for (sp, item) in &line.items {
                let b = match item {
                    Item::Lit(s) => Box_::Text(s.clone()),
                    Item::Arg(i, symbol) => {
                        let arg = args
                            .get(*i)
                            .ok_or_else(|| anyhow!("missing argument {i}"))?;
                        self.argument(arg, symbol, alone)?
                    }
                };
                items.push((*sp, b));
            }
            let line_box = if items.len() == 1 {
                items.pop().unwrap().1
            } else {
                Box_::H(items)
            };
            if matches!(line_box, Box_::Empty) {
                continue; // an absent optional or empty list alone on its line
            }
            out_lines.push((rel, line_box));
        }
        Ok(match out_lines.len() {
            0 => Box_::Empty,
            1 => out_lines.pop().unwrap().1,
            _ => Box_::V(out_lines),
        })
    }

    /// Blank lines a term asks for around itself in a vertical list.
    fn separation(&self, t: &Term) -> u32 {
        let Term::App { sort, cons, .. } = t else {
            return 0;
        };
        self.templates
            .get(&format!("{sort}.{cons}"))
            .and_then(|p| {
                p.attrs.iter().find_map(|a| match a {
                    Attr::Separate(n) => Some(*n),
                    _ => None,
                })
            })
            .unwrap_or(0)
    }

    /// Elements one per line, with the blank lines the source kept (at
    /// most one) or the elements ask for, whichever is more.
    pub fn vertical(&self, items: &[Term]) -> Result<Box_> {
        let mut lines: Vec<(usize, Box_)> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                let kept = matches!(
                    item,
                    Term::App {
                        blank_before: true,
                        ..
                    }
                ) as u32;
                let asked = self.separation(item).max(self.separation(&items[i - 1]));
                for _ in 0..kept.max(asked) {
                    lines.push((0, Box_::Blank));
                }
            }
            lines.push((0, self.boxed(item)?));
        }
        Ok(Box_::V(lines))
    }

    fn argument(&self, arg: &Term, symbol: &Symbol, alone: bool) -> Result<Box_> {
        match (arg, symbol) {
            (Term::List(items), Symbol::Star(_) | Symbol::Plus(_)) => {
                if items.is_empty() {
                    return Ok(Box_::Empty);
                }
                if alone {
                    self.vertical(items)
                } else {
                    let boxes: Vec<Box_> =
                        items.iter().map(|i| self.boxed(i)).collect::<Result<_>>()?;
                    Ok(Box_::H(
                        boxes
                            .into_iter()
                            .enumerate()
                            .map(|(i, b)| (i > 0, b))
                            .collect(),
                    ))
                }
            }
            (Term::List(items), Symbol::SepList { sep, .. }) => {
                let sep_text = match sep.as_ref() {
                    Symbol::Lit(l) => l.clone(),
                    other => bail!("separator {other:?} is not a literal"),
                };
                let mut h: Vec<(bool, Box_)> = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        h.push((false, Box_::Text(sep_text.clone())));
                    }
                    h.push((i > 0, self.boxed(item)?));
                }
                Ok(if h.is_empty() {
                    Box_::Empty
                } else {
                    Box_::H(h)
                })
            }
            (Term::Opt(None), _) => Ok(Box_::Empty),
            (Term::Opt(Some(inner)), sym) => self.in_context(inner, sym),
            (t, sym) => self.in_context(t, sym),
        }
    }

    /// A term standing where the placeholder expects another sort got
    /// there through an injection, and the injection may carry a printing
    /// rule for that position: `Exp = Block {collapse(100)}` is rustfmt's
    /// "a block expression fits on one line", which a block as a function
    /// body never does.
    fn in_context(&self, t: &Term, expected: &Symbol) -> Result<Box_> {
        let b = self.boxed(t)?;
        let (Term::App { sort, .. }, Symbol::Sort(from)) = (t, expected) else {
            return Ok(b);
        };
        if sort == from {
            return Ok(b);
        }
        let rule = self
            .injections
            .iter()
            .find(|(f, to, _)| f == from && to == sort)
            .map(|(_, _, p)| *p);
        if let Some(p) = rule {
            if let Some(width) = p.attrs.iter().find_map(|a| match a {
                Attr::Collapse(w) => Some(*w),
                _ => None,
            }) {
                return Ok(collapse(b, width as usize));
            }
        }
        Ok(b)
    }
}

/// Box's `HV`: a vertical box whose lines hold no vertical list joins its
/// lines with spaces when the result fits in `width` columns.
fn collapse(b: Box_, width: usize) -> Box_ {
    let Box_::V(lines) = &b else {
        return b;
    };
    fn single_line(b: &Box_) -> bool {
        match b {
            Box_::Text(_) | Box_::Empty => true,
            Box_::H(items) => items.iter().all(|(_, i)| single_line(i)),
            Box_::V(_) | Box_::Blank => false,
        }
    }
    if !lines.iter().all(|(_, l)| single_line(l)) {
        return b;
    }
    let items: Vec<(bool, Box_)> = lines.iter().map(|(_, l)| (true, l.clone())).collect();
    let h = Box_::H(items);
    let mut probe = String::new();
    render(&h, 0, &mut probe);
    if probe.chars().count() <= width {
        h
    } else {
        b
    }
}

fn append_text(b: Box_, text: String) -> Box_ {
    match b {
        Box_::V(mut lines) => {
            if let Some(last) = lines.pop() {
                lines.push((last.0, append_text(last.1, text)));
            }
            Box_::V(lines)
        }
        Box_::H(mut items) => {
            items.push((false, Box_::Text(text)));
            Box_::H(items)
        }
        Box_::Text(s) => Box_::Text(s + &text),
        Box_::Empty | Box_::Blank => Box_::Text(text.trim_start().to_string()),
    }
}

/// Render at the given line indent. `out` ends at the current column.
pub fn render(b: &Box_, indent: usize, out: &mut String) {
    match b {
        Box_::Empty => {}
        Box_::Text(s) => out.push_str(s),
        Box_::H(items) => {
            let mut first = true;
            for (space, item) in items {
                if matches!(item, Box_::Empty) {
                    continue;
                }
                if *space && !first && !out.ends_with(' ') && !out.ends_with('\n') {
                    out.push(' ');
                }
                render(item, indent, out);
                first = false;
            }
        }
        Box_::Blank => {}
        Box_::V(lines) => {
            let mut first = true;
            for (rel, line) in lines {
                if matches!(line, Box_::Empty) {
                    continue;
                }
                if matches!(line, Box_::Blank) {
                    out.push('\n');
                    continue;
                }
                let at = indent + rel;
                if !first {
                    out.push('\n');
                    out.push_str(&" ".repeat(at));
                }
                render(line, at, out);
                first = false;
            }
        }
    }
}
