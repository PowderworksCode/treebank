//! A second backend: the same SDF3 module as an ANTLR4 grammar.
//!
//! The point is not ANTLR. It is that an abstraction with one implementation
//! is not an abstraction (`notes/metagrammar.md` §7), and that the capability
//! table in §3 makes claims about what each backend can and cannot lower
//! that only a second backend can test. Where the tree-sitter lowering needed
//! a generated scanner for layout facts, ANTLR has semantic predicates; where
//! tree-sitter carried an ambiguity with a declared conflict and a weight,
//! ANTLR's ALL(*) takes the first alternative that can match. Each is recorded.
//!
//! The mapping:
//!
//! - A **sort** becomes a parser rule; each **constructor** a labeled
//!   alternative (`# add`), which is ANTLR's own supertype/subtype split: the
//!   rule's context class is the supertype and each label's class a subtype.
//!   Node names are the tree-sitter lowering's, so one corpus serves both.
//! - An **injection** is a labeled alternative too (`# inj_exp_1`), because an
//!   ANTLR parse tree has a context for every rule invocation; the driver
//!   elides those when printing, and the deviation is recorded.
//! - A **priority chain** becomes alternative order in a left-recursive rule,
//!   highest first; `{right}` becomes `<assoc=right>`; `{non-assoc}` has no
//!   form and widens, as in tree-sitter.
//! - `{prefer}` / `{avoid}` move an alternative to the front / back of its own
//!   rule. ALL(*) resolves a true ambiguity to the first alternative that can match,
//!   so this is `prefer` lowered exactly -- within the rule. An ambiguity that
//!   is decided in an ancestor rule is resolved by *that* rule's source
//!   order, which the attribute does not reach. Recorded.
//! - A **layout constraint** becomes a semantic predicate between the two
//!   symbols, `{self.adjacent()}?` or `{self.separated()}?`, comparing token
//!   character offsets. No variants, no propagation, no scanner: the
//!   constraint is checked where SDF3 states it. A lexical sort reached by a
//!   separation (the regex literal) gets a lexer predicate on the character
//!   before it, since the ANTLR lexer cannot ask the parser what is valid.
//! - **LAYOUT** goes to the hidden channel, so predicates can still measure
//!   the gap; comments therefore do not appear in the tree, where tree-sitter
//!   shows them as extras. Recorded.
//! - A literal in a parser rule outranks a lexer rule of the same text in
//!   ANTLR, so keywords are reserved with nothing emitted.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};

use crate::ast::*;
use crate::lower::{Finding, Kind, Names};
use crate::scanner::{self, Cond};

pub struct Emitted {
    pub grammar: String,
    pub findings: Vec<Finding>,
}

pub fn emit(
    module: &Module,
    names: &Names,
    levels: &BTreeMap<String, (u32, Option<Attr>)>,
) -> Result<Emitted> {
    let mut findings = Vec::new();
    let (plan, _) = scanner::plan(module)?;
    let grammar_name = capitalize(&module.name);
    let mut out = String::new();
    out.push_str(&format!(
        "// GENERATED from {}.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.\ngrammar {grammar_name};\n\n",
        module.name
    ));

    let mut members: Vec<String> = Vec::new();
    if let Some(ind) = &plan.indent {
        out.push_str("// H_ tokens are hidden in the tree, as tree-sitter's `_` externals are.\n\n");
        let openers: Vec<String> = ind.openers.iter().map(|l| literal(l)).collect();
        let comment_open = plan.comment_open.map(|c| c as u32).unwrap_or(0);
        members.push(format!(
            r#"# Indentation, from the module's indent/align-list constraints: the
# indent stack tree-sitter's generated scanner keeps, without validity.
# The lexer cannot ask the parser whether a block may open here, so a
# deeper line opens one only after an opener literal, and continues
# the statement otherwise.
_OPENERS = ({openers},)
_COMMENT_OPEN = {comment_open}
def _ind(self):
    if not hasattr(self, '_stack'):
        self._stack = [0]
        self._queue = []
        self._last = None
    return self._stack
def _make(self, ttype):
    return self._factory.create(self._tokenFactorySourcePair, ttype, '',
        Token.DEFAULT_CHANNEL, self._input.index, self._input.index - 1,
        self.line, self.column)
def nextToken(self):
    stack = self._ind()
    if self._queue:
        return self._queue.pop(0)
    t = super().nextToken()
    if t.type == Token.EOF:
        if self._last is not None and self._last.type not in (self.H_NEWLINE, self.H_DEDENT):
            self._queue.append(self._make(self.H_NEWLINE))
        while len(stack) > 1:
            stack.pop()
            self._queue.append(self._make(self.H_DEDENT))
        if self._queue:
            self._queue.append(t)
            return self._queue.pop(0)
        return t
    if t.channel == Token.DEFAULT_CHANNEL:
        self._last = t
    return t
def on_newline(self):
    stack = self._ind()
    if self._last is None:
        self.skip()  # a break before the first token
        return
    nxt = self._input.LA(1)
    if nxt in (10, 13) or (self._COMMENT_OPEN and nxt == self._COMMENT_OPEN):
        self.skip()  # a blank or comment line: the next break decides
        return
    col = 0 if nxt == -1 else len(self.text.lstrip('\r\n'))
    top = stack[-1]
    if col > top:
        if self._last.text in self._OPENERS:
            stack.append(col)
            self._type = self.H_INDENT
            return
        self.skip()  # a continuation line: the offside rule
        return
    self._type = self.H_NEWLINE
    while col < stack[-1]:
        stack.pop()
        self._queue.append(self._make(self.H_DEDENT))
    if col != stack[-1]:
        # a dedent to a column no open block has: a token no rule accepts
        self._queue.append(self._make(Token.INVALID_TYPE))"#,
            openers = openers.join(", "),
        ));
        findings.push(Finding {
            kind: Kind::Mapped,
            what: "indent/align-list/align/offside became `H_NEWLINE`, `H_INDENT` and `H_DEDENT` from an indent stack in the lexer, as CPython's tokenizer keeps one; the parser rules are shaped exactly as the tree-sitter lowering's".into(),
        });
        findings.push(Finding {
            kind: Kind::Deviation,
            what: format!(
                "the lexer cannot ask the parser whether `_indent` is valid, so a deeper line opens a block only after one of [{}] (the literals before an indented symbol) and continues the statement otherwise; tree-sitter's scanner decides the same question by validity",
                ind.openers.iter().map(|l| format!("{l:?}")).collect::<Vec<_>>().join(", ")
            ),
        });
    }
    if !plan.variants.is_empty() {
        let mut chars: Vec<String> = plan
            .layout_chars
            .iter()
            .map(|c| (*c as u32).to_string())
            .collect();
        chars.sort();
        chars.dedup();
        let layout = chars.join(", ");
        members.push(format!(
            "def gap_before(self):\n    return self._input.LA(-1) in (-1, 10, 13, {layout})\ndef gap_after(self):\n    return self._input.LA(1) in (-1, 10, 13, {layout})"
        ));
        findings.push(Finding {
            kind: Kind::Mapped,
            what: "layout constraints became lexer token variants with lexer predicates on the character before and after, from the same plan as the tree-sitter scanner; the parser has no say in which variant the lexer emits, which is the validity tree-sitter's scanner had and ANTLR's lexer does not".into(),
        });
        findings.push(Finding {
            kind: Kind::Deviation,
            what: "no parser predicate carries a layout constraint: ANTLR consults a left-edge predicate during prediction in a plain rule and not in a left-recursive one (measured), and every expression rule is left-recursive".into(),
        });
        findings.push(Finding {
            kind: Kind::Widening,
            what: "without validity, an unconstrained occurrence takes only the default variant and a constrained one only its own: `(a+b) -1` has no token that subtraction accepts, `z=-1` has no token that negation accepts, and both are rejected where SDF3 accepts them; `y = - 1` is rejected as SDF3 rejects it, where tree-sitter's scanner widened".into(),
        });
    }

    if !members.is_empty() {
        out.push_str(&format!("@lexer::members {{\n{}\n}}\n\n", members.join("\n")));
    }

    // Parser rules, start symbol first.
    let mut cf: BTreeMap<&str, Vec<(usize, &Production)>> = BTreeMap::new();
    let mut order: Vec<&str> = Vec::new();
    for (pi, p) in module.productions(false).enumerate() {
        if !cf.contains_key(p.sort.as_str()) {
            order.push(&p.sort);
        }
        cf.entry(&p.sort).or_default().push((pi, p));
    }
    let starts = module.start_symbols();
    let Some(start) = starts.first() else {
        bail!("no start symbol")
    };
    order.retain(|s| s != start);
    order.insert(0, start);

    let mut inj = 0usize;
    for sort in order {
        let rule = rule_for(names, sort);
        let mut alts = cf[sort].clone();
        // prefer first, avoid last, then priority (highest first), then source.
        alts.sort_by_key(|(pi, p)| {
            let class = if p.has(&Attr::Prefer) {
                0
            } else if p.has(&Attr::Avoid) {
                2
            } else {
                1
            };
            let level = p
                .reference()
                .and_then(|r| levels.get(&r).map(|(l, _)| *l))
                .unwrap_or(0);
            (class, std::cmp::Reverse(level), *pi)
        });
        let mut lines = Vec::new();
        for (pi, p) in &alts {
            let mut alt = String::new();
            if let Some(r) = p.reference() {
                if let Some((_, Some(Attr::Right))) = levels.get(&r) {
                    alt.push_str("<assoc=right> ");
                }
                if let Some((_, Some(Attr::NonAssoc))) = levels.get(&r) {
                    findings.push(Finding {
                        kind: Kind::Widening,
                        what: format!("{r} is non-assoc; ANTLR has no non-associativity, lowered as left-associative"),
                    });
                }
            }
            if p.has(&Attr::Prefer) || p.has(&Attr::Avoid) {
                findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!(
                        "{}: `{{{}}}` became alternative order within `{rule}`; ALL(*) takes the first alternative that can match, so an ambiguity decided in an ancestor rule follows that rule's source order, which this attribute does not reach",
                        p.display(),
                        if p.has(&Attr::Prefer) { "prefer" } else { "avoid" }
                    ),
                });
            }
            let label = if p.has(&Attr::Bracket) {
                format!("{}_bracket", rule_name(sort))
            } else if p.constructor.is_none() {
                inj += 1;
                findings.push(Finding {
                    kind: Kind::Deviation,
                    what: format!("injection into {sort} is a context node in ANTLR's tree (`inj_{}_{inj}`); the driver elides it when printing", rule_name(sort)),
                });
                format!("inj_{}_{inj}", rule_name(sort))
            } else {
                names
                    .node
                    .get(&p.reference().unwrap())
                    .cloned()
                    .unwrap_or_else(|| rule_name(sort))
            };
            alt.push_str(&elements(p, *pi, names, &plan)?);
            if sort == *start {
                alt.push_str(" EOF");
            }
            lines.push(format!("    {alt}  # {label}"));
        }
        let single =
            alts.len() == 1 && alts[0].1.constructor.is_some() && !alts[0].1.has(&Attr::Bracket);
        if single {
            // One constructor: the rule name is the node name, no label needed.
            let (pi, p) = alts[0];
            let mut body = elements(p, pi, names, &plan)?;
            if sort == *start {
                body.push_str(" EOF");
            }
            out.push_str(&format!("{rule}\n    : {body}\n    ;\n\n"));
        } else {
            out.push_str(&format!(
                "{rule}\n    : {}\n    ;\n\n",
                lines.join("\n    | ")
            ));
        }
    }

    // Lexer rules: the variants first, most specific first, so an equal-length
    // match goes to the first rule whose predicates hold.
    for v in plan.variants.iter().filter(|v| !v.visible) {
        let mut rule = String::new();
        if v.before == Cond::Req {
            rule.push_str("{self.gap_before()}? ");
        } else if v.before == Cond::Forbid {
            rule.push_str("{not self.gap_before()}? ");
        }
        rule.push_str(&literal(&v.spelling));
        if v.after == Cond::Req {
            rule.push_str(" {self.gap_after()}?");
        } else if v.after == Cond::Forbid {
            rule.push_str(" {not self.gap_after()}?");
        }
        out.push_str(&format!("{} : {rule} ;\n", variant_token(&v.name)));
    }
    let mut lexical: BTreeMap<&str, Vec<&Production>> = BTreeMap::new();
    for p in module.productions(true) {
        lexical.entry(&p.sort).or_default().push(p);
    }
    let mut layout_n = 0;
    if plan.indent.is_some() {
        // Before WS, so a line break is never whitespace: the action
        // decides what it is.
        out.push_str("H_NEWLINE : ( '\\r'? '\\n' | '\\r' ) [ \\t]* { self.on_newline() } ;\n");
        // Lexer rules rather than `tokens {}`: the Python target's lexer
        // exposes no constant for a declared token, and the action needs
        // the types. No source holds these control characters.
        out.push_str("H_INDENT : '\\u0001' ;\nH_DEDENT : '\\u0002' ;\n");
    }
    for (sort, prods) in &lexical {
        if *sort == "LAYOUT" {
            for p in prods {
                layout_n += 1;
                let is_class = matches!(&p.rhs, Rhs::Symbols(s) if s.len() == 1 && matches!(s[0], Symbol::CharClass(_)));
                let body = match &p.rhs {
                    Rhs::Symbols(s) if is_class && plan.indent.is_some() => {
                        let Symbol::CharClass(c) = &s[0] else { unreachable!() };
                        class(&without(c, &['\n', '\r']))
                    }
                    _ => lexical_body(p, &lexical)?,
                };
                let name = if is_class {
                    format!("WS{layout_n}")
                } else {
                    format!("COMMENT{layout_n}")
                };
                let body = if is_class { format!("{body}+") } else { body };
                out.push_str(&format!("{name} : {body} -> channel(HIDDEN) ;\n"));
            }
            findings.push(Finding {
                kind: Kind::Deviation,
                what: "LAYOUT goes to the hidden channel: comments are absent from ANTLR's tree, where tree-sitter shows them as extras".into(),
            });
            continue;
        }
        let keep: Vec<&&Production> = prods.iter().filter(|p| !p.has(&Attr::Reject)).collect();
        if keep.len() < prods.len() {
            findings.push(Finding {
                kind: Kind::Absorbed,
                what: format!("reject productions on {sort}: a literal in a parser rule outranks a lexer rule of the same text in ANTLR, so keywords are reserved with nothing emitted"),
            });
        }
        if keep.is_empty() {
            continue;
        }
        let alts: Vec<String> = keep
            .iter()
            .map(|p| lexical_body(p, &lexical))
            .collect::<Result<_>>()?;
        let body = if alts.len() == 1 {
            alts[0].clone()
        } else {
            format!("( {} )", alts.join(" | "))
        };
        let pred = match plan
            .variants
            .iter()
            .find(|v| v.visible && v.name == crate::lower::snake(sort))
        {
            Some(v) if v.before == Cond::Req => {
                findings.push(Finding {
                    kind: Kind::Deviation,
                    what: format!("lexical sort {sort} is reached by a separation constraint: the lexer checks the character before it with a predicate, since it cannot ask the parser what is valid; `x =/b/` is rejected where tree-sitter's validity-first scanner accepts it"),
                });
                "{self.gap_before()}? "
            }
            _ => "",
        };
        out.push_str(&format!("{} : {pred}{body} ;\n", token_name(sort)));
    }
    for opt in module.template_options() {
        if let TemplateOption::KeywordReject { sort } = opt {
            findings.push(Finding {
                kind: Kind::Absorbed,
                what: format!("`{sort} = keyword {{reject}}`: parser literals outrank `{}` in ANTLR's lexer by construction", token_name(sort)),
            });
        }
    }
    Ok(Emitted {
        grammar: out,
        findings,
    })
}

fn elements(p: &Production, pi: usize, names: &Names, plan: &scanner::Plan) -> Result<String> {
    let mut parts = Vec::new();
    let mut pos = 0;
    let lit_at = |pos: usize, l: &str| -> String {
        match plan.occurrences.get(&(pi, pos)) {
            Some(external) => variant_token(external),
            None => literal(l),
        }
    };
    // An indented occurrence is `H_INDENT .. H_DEDENT`, as the tree-sitter
    // lowering wraps it.
    let wrap = |pos: usize, text: String| -> String {
        match &plan.indent {
            Some(ind) if ind.blocks.contains(&(pi, pos)) => format!("H_INDENT {text} H_DEDENT"),
            _ => text,
        }
    };
    match &p.rhs {
        Rhs::Template(tp) => {
            for part in tp {
                match part {
                    TemplatePart::Layout(_) => {}
                    TemplatePart::Lit(l) => {
                        pos += 1;
                        parts.push(lit_at(pos, l));
                    }
                    TemplatePart::Placeholder { label, symbol } => {
                        pos += 1;
                        let t = symbol_text(symbol, label.as_deref(), names, plan)?;
                        parts.push(wrap(pos, t));
                    }
                }
            }
        }
        Rhs::Symbols(syms) => {
            for s in syms {
                pos += 1;
                parts.push(match s {
                    Symbol::Lit(l) => lit_at(pos, l),
                    other => {
                        let t = symbol_text(other, None, names, plan)?;
                        wrap(pos, t)
                    }
                });
            }
        }
    }
    if plan.indent.as_ref().is_some_and(|ind| ind.terminated.contains(&pi)) {
        parts.push("H_NEWLINE".into());
    }
    Ok(parts.join(" "))
}

/// The lexer token for a scanner-plan variant: `_minus_spaced_tight` becomes
/// `V_MINUS_SPACED_TIGHT`, which the driver knows to omit from the tree.
fn variant_token(external: &str) -> String {
    format!(
        "V_{}",
        external.trim_start_matches('_').to_ascii_uppercase()
    )
}

fn symbol_text(
    s: &Symbol,
    label: Option<&str>,
    names: &Names,
    plan: &scanner::Plan,
) -> Result<String> {
    // ANTLR refuses an element label spelled like a rule; suffix it, and the
    // driver strips the suffix when it prints the field.
    let owned: Option<String> = label.map(|l| {
        if names.sort_rule.keys().any(|sort| rule_for(names, sort) == l) {
            format!("{l}_")
        } else {
            l.to_string()
        }
    });
    let label = owned.as_deref();
    let lab = |op: &str, inner: String| match label {
        Some(l) => format!("{l}{op}{inner}"),
        None => inner,
    };
    Ok(match s {
        Symbol::Sort(name) => {
            let r = if names.lexical.contains(name) || plan.lexical_owned.contains_key(name) {
                token_name(name)
            } else {
                rule_for(names, name)
            };
            lab("=", r)
        }
        Symbol::Lit(l) => lab("=", literal(l)),
        Symbol::Star(inner) => {
            let i = symbol_text(inner, label.map(|_| "").filter(|_| false), names, plan)?;
            match label {
                Some(l) => format!("({l}+={i})*"),
                None => format!("{i}*"),
            }
        }
        Symbol::Plus(inner) => {
            let i = symbol_text(inner, None, names, plan)?;
            match label {
                Some(l) => format!("({l}+={i})+"),
                None => format!("{i}+"),
            }
        }
        Symbol::Opt(inner) => {
            let i = symbol_text(inner, None, names, plan)?;
            match label {
                Some(l) => format!("({l}={i})?"),
                None => format!("{i}?"),
            }
        }
        Symbol::SepList { elem, sep, min } => {
            let e = symbol_text(elem, None, names, plan)?;
            let sp = symbol_text(sep, None, names, plan)?;
            let one = match label {
                Some(l) => format!("{l}+={e} ({sp} {l}+={e})*"),
                None => format!("{e} ({sp} {e})*"),
            };
            if *min == 0 {
                format!("({one})?")
            } else {
                one
            }
        }
        Symbol::Group(alts) => {
            let inner: Vec<String> = alts
                .iter()
                .map(|a| {
                    a.iter()
                        .map(|s| symbol_text(s, None, names, plan))
                        .collect::<Result<Vec<_>>>()
                        .map(|v| v.join(" "))
                })
                .collect::<Result<_>>()?;
            format!("({})", inner.join(" | "))
        }
        Symbol::CharClass(_) => bail!("a character class in context-free syntax is unsupported"),
    })
}

fn lexical_body(p: &Production, lexical: &BTreeMap<&str, Vec<&Production>>) -> Result<String> {
    let Rhs::Symbols(syms) = &p.rhs else {
        bail!("lexical syntax for {} uses a template", p.sort)
    };
    let parts: Vec<String> = syms
        .iter()
        .map(|s| lexical_symbol(s, lexical))
        .collect::<Result<_>>()?;
    Ok(parts.join(" "))
}

fn lexical_symbol(s: &Symbol, lexical: &BTreeMap<&str, Vec<&Production>>) -> Result<String> {
    Ok(match s {
        Symbol::CharClass(c) => class(c),
        Symbol::Lit(l) => literal(l),
        Symbol::Sort(name) => {
            if !lexical.contains_key(name.as_str()) {
                bail!("lexical sort {name} referenced but not defined");
            }
            token_name(name)
        }
        Symbol::Star(i) => format!("({})*", lexical_symbol(i, lexical)?),
        Symbol::Plus(i) => format!("({})+", lexical_symbol(i, lexical)?),
        Symbol::Opt(i) => format!("({})?", lexical_symbol(i, lexical)?),
        Symbol::Group(alts) => {
            let inner: Vec<String> = alts
                .iter()
                .map(|a| {
                    a.iter()
                        .map(|s| lexical_symbol(s, lexical))
                        .collect::<Result<Vec<_>>>()
                        .map(|v| v.join(" "))
                })
                .collect::<Result<_>>()?;
            format!("({})", inner.join(" | "))
        }
        Symbol::SepList { .. } => bail!("a separated list in lexical syntax is unsupported"),
    })
}

/// The class minus the given characters, for a whitespace rule that must
/// leave line breaks to the indentation rule.
fn without(c: &CharClass, drop: &[char]) -> CharClass {
    let mut ranges = Vec::new();
    for &(a, b) in &c.ranges {
        let mut start = a;
        let mut ch = a;
        loop {
            if drop.contains(&ch) {
                if start < ch {
                    ranges.push((start, char::from_u32(ch as u32 - 1).unwrap_or(start)));
                }
                start = char::from_u32(ch as u32 + 1).unwrap_or(ch);
            }
            if ch >= b {
                break;
            }
            ch = char::from_u32(ch as u32 + 1).unwrap_or(b);
        }
        if start <= b && !drop.contains(&b) {
            ranges.push((start, b));
        }
    }
    CharClass {
        negated: c.negated,
        ranges,
    }
}

fn class(c: &CharClass) -> String {
    let mut s = String::new();
    if c.negated {
        s.push('~');
    }
    s.push('[');
    for (a, b) in &c.ranges {
        s.push_str(&class_char(*a));
        if a != b {
            s.push('-');
            s.push_str(&class_char(*b));
        }
    }
    s.push(']');
    s
}

fn class_char(c: char) -> String {
    match c {
        '\n' => "\\n".into(),
        '\r' => "\\r".into(),
        '\t' => "\\t".into(),
        '\\' | ']' | '[' | '-' => format!("\\{c}"),
        _ => c.to_string(),
    }
}

fn literal(l: &str) -> String {
    let mut s = String::from("'");
    for c in l.chars() {
        match c {
            '\'' => s.push_str("\\'"),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\t' => s.push_str("\\t"),
            c => s.push(c),
        }
    }
    s.push('\'');
    s
}

pub fn rule_name(sort: &str) -> String {
    crate::lower::snake(sort)
}

/// The parser rule for a sort: the tree-sitter rule name without its
/// hidden-marker underscore, so a single-constructor sort is named for its
/// constructor there and here alike (`Else.ElseClause` is `else_clause`).
fn rule_for(names: &Names, sort: &str) -> String {
    names
        .sort_rule
        .get(sort)
        .map(|r| r.trim_start_matches('_').to_string())
        .unwrap_or_else(|| rule_name(sort))
}

pub fn token_name(sort: &str) -> String {
    sort.to_ascii_uppercase()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[allow(dead_code)]
fn _unused(_: &BTreeSet<String>) {}
