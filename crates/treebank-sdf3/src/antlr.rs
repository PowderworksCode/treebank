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
    levels: &BTreeMap<usize, (u32, Option<Attr>)>,
) -> Result<Emitted> {
    let mut findings = Vec::new();
    let (plan, _) = scanner::plan(module)?;
    let nfa_builder = crate::nfa::Builder::new(module);
    let layout_like: BTreeSet<String> = plan
        .owned
        .iter()
        .filter(|o| {
            !plan.layout_chars.is_empty()
                && nfa_builder.alphabet(&o.sort).is_some_and(|a| {
                    !a.is_empty() && a.iter().all(|c| plan.layout_chars.contains(c))
                })
        })
        .map(|o| o.sort.clone())
        .collect();
    let kernel_owned: BTreeSet<String> = plan
        .owned
        .iter()
        .filter(|o| !layout_like.contains(&o.sort))
        .map(|o| o.sort.clone())
        .collect();
    let kw_prefer: Option<(String, String)> = module.template_options().find_map(|o| match o {
        TemplateOption::KeywordPrefer { sort } => Some((
            sort.clone(),
            format!("h_{}_kw", token_name(sort).to_ascii_lowercase()),
        )),
        _ => None,
    });
    let ecx = ElemCx {
        module,
        layout_like,
        kernel_owned,
        kw_prefer,
    };
    let grammar_name = capitalize(&module.symbol_name());
    let mut out = String::new();
    out.push_str(&format!(
        "// GENERATED from {}.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.\ngrammar {grammar_name};\n\n",
        module.name
    ));
    if module
        .template_options()
        .any(|o| matches!(o, TemplateOption::KeywordCaseInsensitive))
    {
        out.push_str("options { caseInsensitive = true; }\n\n");
        findings.push(Finding {
            kind: Kind::Widening,
            what: "`keyword = case-insensitive` became the grammar option `caseInsensitive`, which folds every literal and character class in the lexer, not only the keywords; here nothing but keywords has a case".into(),
        });
    }

    let mut members: Vec<String> = Vec::new();
    if let Some(ind) = &plan.indent {
        out.push_str(
            "// H_ tokens are hidden in the tree, as tree-sitter's `_` externals are.\n\n",
        );
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
        out.push_str(&format!(
            "@lexer::members {{\n{}\n}}\n\n",
            members.join("\n")
        ));
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
            let level = levels.get(pi).map(|(l, _)| *l).unwrap_or(0);
            (class, std::cmp::Reverse(level), *pi)
        });
        let mut lines = Vec::new();
        for (pi, p) in &alts {
            let mut alt = String::new();
            if let Some(r) = p.reference() {
                if let Some((_, Some(Attr::Right))) = levels.get(pi) {
                    alt.push_str("<assoc=right> ");
                }
                if let Some((_, Some(Attr::NonAssoc))) = levels.get(pi) {
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
            // ANTLR refuses an alternative label spelled like its rule, which
            // a constructor with several productions produces; the driver
            // strips the `_altN` suffix.
            let label = if label == rule {
                format!("{label}_alt{}", lines.len() + 1)
            } else {
                label
            };
            if sort == *start {
                alt.push_str(&lead_layout(&ecx));
            }
            alt.push_str(&elements(p, *pi, names, &plan, &ecx)?);
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
            let mut body = elements(p, pi, names, &plan, &ecx)?;
            if sort == *start {
                body = format!("{}{body} EOF", lead_layout(&ecx));
            }
            out.push_str(&format!("{rule}\n    : {body}\n    ;\n\n"));
        } else {
            out.push_str(&format!(
                "{rule}\n    : {}\n    ;\n\n",
                lines.join("\n    | ")
            ));
        }
    }

    if let Some((sort, rule)) = &ecx.kw_prefer {
        let mut kws: Vec<String> = Vec::new();
        for p in module.productions(false) {
            for sym in p.symbols() {
                if let SymRef::Lit(l) = sym {
                    if is_word_lit(l) && !kws.contains(&l.to_string()) {
                        kws.push(l.to_string());
                    }
                }
            }
        }
        let alts: Vec<String> = std::iter::once(token_name(sort))
            .chain(kws.iter().map(|k| literal(k)))
            .collect();
        out.push_str(&format!("{rule}\n    : {}\n    ;\n\n", alts.join(" | ")));
        findings.push(Finding {
            kind: Kind::Mapped,
            what: format!(
                "`{sort} = keyword {{prefer}}`: every {sort} position goes through `{rule}`, which admits the {} keyword literals as well, since ANTLR's lexer gives a literal its own token everywhere; where both readings are viable ALL(*) takes the earlier alternative, the keyword's where its production precedes the identifier's, and where they are alternatives of different rules the outer rule's order decides",
                kws.len()
            ),
        });
        findings.push(Finding {
            kind: Kind::Widening,
            what: format!(
                "through `{rule}` a keyword is an identifier wherever an identifier is admitted, even where SDF3's `{{prefer}}` would take the keyword reading: `[for, a]` parses as a tuple here and is rejected by tree-sitter's keyword extraction"
            ),
        });
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
    // The first characters of the layout-like sorts are theirs, not
    // whitespace: `\n` is a token, `H_NL*` stands wherever layout is admitted.
    let mut claimed: Vec<char> = Vec::new();
    {
        let mut b = crate::nfa::Builder::new(module);
        for sort in &ecx.layout_like {
            if let Ok(start) = b.token(sort, None) {
                for c in ['\n', '\r', ' ', '\t', '\u{c}'] {
                    if b.can_start(start, c) && !claimed.contains(&c) {
                        claimed.push(c);
                    }
                }
            }
        }
    }
    for sort in &ecx.layout_like {
        let prods = lexical.get(sort.as_str()).cloned().unwrap_or_default();
        let alts: Vec<String> = prods
            .iter()
            .map(|p| lexical_body(p, &lexical))
            .collect::<Result<_>>()?;
        out.push_str(&format!("{} : ( {} ) ;
", token_name(sort), alts.join(" | ")));
        findings.push(Finding {
            kind: Kind::Mapped,
            what: format!(
                "lexical sort {sort}'s text is LAYOUT: it is the token `{}`, its first characters are no longer whitespace, and `{}*` stands at every position where layout is admitted -- SDF3's `LAYOUT?` between context-free symbols, made explicit for the one kind of layout that is also a token",
                token_name(sort),
                token_name(sort)
            ),
        });
    }
    if !ecx.kernel_owned.is_empty() {
        findings.push(Finding {
            kind: Kind::Unsupported,
            what: format!(
                "kernel syntax reaches [{}] where no layout may precede them; tree-sitter's scanner lexes them in a mode of their own, and ANTLR would need lexer modes, which this lowering does not derive. Their tokens are declared unmatchable so the grammar compiles, and every construct that needs them is a parse error here",
                ecx.kernel_owned.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
        });
    }
    let mut unreachable_n = 2u32;
    let cf_referenced = crate::lower::cf_referenced_sorts(module);
    let mut fragments: Vec<String> = Vec::new();
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
        if ecx.layout_like.contains(*sort) {
            continue;
        }
        if ecx.kernel_owned.contains(*sort) {
            unreachable_n += 1;
            out.push_str(&format!(
                "{} : '\\u{:04x}' ;  // kernel-owned: needs a lexer mode\n",
                token_name(sort),
                unreachable_n
            ));
            continue;
        }
        if *sort == "LAYOUT" {
            for p in prods {
                layout_n += 1;
                let mut chars = Vec::new();
                let is_ws = p.constructor.is_none()
                    && matches!(&p.rhs, Rhs::Symbols(s) if s.iter().all(|sym| scanner::whitespace_alphabet(sym, &mut chars)));
                if is_ws && chars.iter().any(|c| claimed.contains(c)) {
                    let single = matches!(&p.rhs, Rhs::Symbols(s) if s.len() == 1 && matches!(s[0], Symbol::CharClass(_)));
                    if !single {
                        // Its text is the layout-like token's; nothing left.
                        continue;
                    }
                }
                let is_class = matches!(&p.rhs, Rhs::Symbols(s) if s.len() == 1 && matches!(s[0], Symbol::CharClass(_)));
                let body = match &p.rhs {
                    Rhs::Symbols(s) if is_class && (plan.indent.is_some() || !claimed.is_empty()) => {
                        let Symbol::CharClass(c) = &s[0] else {
                            unreachable!()
                        };
                        let mut drop: Vec<char> = claimed.clone();
                        if plan.indent.is_some() {
                            drop.extend(['\n', '\r']);
                        }
                        let trimmed = without(c, &drop);
                        if trimmed.ranges.is_empty() {
                            continue;
                        }
                        class(&trimmed)
                    }
                    _ => lexical_body(p, &lexical)?,
                };
                let is_class = is_ws;
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
        // A sort only other tokens' text refers to is a fragment: a lexer
        // rule of its own would compete with the real tokens (`DELIM`
        // against `IDENTIFIER`) and win on declaration order.
        let fragment = if cf_referenced.contains(*sort) {
            ""
        } else {
            fragments.push(sort.to_string());
            "fragment "
        };
        out.push_str(&format!("{fragment}{} : {pred}{body} ;\n", token_name(sort)));
    }
    if !fragments.is_empty() {
        findings.push(Finding {
            kind: Kind::Mapped,
            what: format!(
                "lexical sorts referenced by lexical syntax only became `fragment` rules: [{}]",
                fragments.join(", ")
            ),
        });
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

/// What the element emitter needs of the module beyond the plan: the
/// lexical sorts whose text is layout, and which sorts may begin with one.
pub struct ElemCx<'m> {
    pub module: &'m Module,
    pub layout_like: BTreeSet<String>,
    pub kernel_owned: BTreeSet<String>,
    /// `ID = keyword {prefer}`: the sort, and the hidden rule that admits
    /// every keyword literal beside it.
    pub kw_prefer: Option<(String, String)>,
}

impl<'m> ElemCx<'m> {
    fn may_start_layout_like(&self, sym: &Symbol, seen: &mut BTreeSet<String>) -> bool {
        match sym {
            Symbol::Sort(n) if self.layout_like.contains(n) => true,
            Symbol::Sort(n) => {
                if self.module.productions(true).any(|p| p.sort == *n) || !seen.insert(n.clone()) {
                    return false;
                }
                self.module
                    .productions(false)
                    .filter(|p| p.sort == *n)
                    .any(|p| match p.symbols().first() {
                        Some(SymRef::Sym(f)) => self.may_start_layout_like(f, seen),
                        _ => false,
                    })
            }
            Symbol::Star(i) | Symbol::Plus(i) | Symbol::Opt(i) => self.may_start_layout_like(i, seen),
            Symbol::SepList { elem, .. } => self.may_start_layout_like(elem, seen),
            Symbol::Group(alts) => alts
                .iter()
                .filter_map(|a| a.first())
                .any(|f| self.may_start_layout_like(f, seen)),
            Symbol::Lit(_) | Symbol::CharClass(_) => false,
        }
    }

    /// `H_NL*` before a symbol where layout is admitted, since a line
    /// break is a token here; nothing in kernel syntax, and nothing before
    /// a symbol that may itself begin with the newline token.
    fn layout_before(&self, p: &Production, sym: Option<&Symbol>) -> String {
        if self.layout_like.is_empty() || p.is_kernel() {
            return String::new();
        }
        let mut seen = BTreeSet::new();
        if sym.is_some_and(|s| self.may_start_layout_like(s, &mut seen)) {
            return String::new();
        }
        let toks: Vec<String> = self.layout_like.iter().map(|s| token_name(s)).collect();
        format!("{}* ", toks.join("* "))
    }
}

/// `H_NL*` at the start of the start rule: a file may begin with a break.
fn lead_layout(ecx: &ElemCx) -> String {
    if ecx.layout_like.is_empty() {
        return String::new();
    }
    let toks: Vec<String> = ecx.layout_like.iter().map(|s| token_name(s)).collect();
    format!("{}* ", toks.join("* "))
}

fn elements(
    p: &Production,
    pi: usize,
    names: &Names,
    plan: &scanner::Plan,
    ecx: &ElemCx,
) -> Result<String> {
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
                        if pos > 1 {
                            parts.push(ecx.layout_before(p, None));
                        }
                        parts.push(lit_at(pos, l));
                    }
                    TemplatePart::Placeholder { label, symbol } => {
                        pos += 1;
                        if scanner::is_layout_symbol(SymRef::Sym(symbol)) {
                            let toks: Vec<String> =
                                ecx.layout_like.iter().map(|s| token_name(s)).collect();
                            if !toks.is_empty() {
                                parts.push(format!("{}*", toks.join("* ")));
                            }
                            continue;
                        }
                        if pos > 1 {
                            parts.push(ecx.layout_before(p, Some(symbol)));
                        }
                        let t = symbol_text(symbol, label.as_deref(), names, plan, ecx)?;
                        parts.push(wrap(pos, t));
                    }
                }
            }
        }
        Rhs::Symbols(syms) => {
            for s in syms {
                pos += 1;
                if scanner::is_layout_symbol(SymRef::Sym(s)) {
                    let toks: Vec<String> =
                        ecx.layout_like.iter().map(|x| token_name(x)).collect();
                    if !toks.is_empty() {
                        parts.push(format!("{}*", toks.join("* ")));
                    }
                    continue;
                }
                if pos > 1 {
                    parts.push(ecx.layout_before(
                        p,
                        match s {
                            Symbol::Lit(_) => None,
                            other => Some(other),
                        },
                    ));
                }
                parts.push(match s {
                    Symbol::Lit(l) => lit_at(pos, l),
                    other => {
                        let t = symbol_text(other, None, names, plan, ecx)?;
                        wrap(pos, t)
                    }
                });
            }
        }
    }
    if plan
        .indent
        .as_ref()
        .is_some_and(|ind| ind.terminated.contains(&pi))
    {
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
    ecx: &ElemCx,
) -> Result<String> {
    // ANTLR refuses an element label spelled like a rule; suffix it, and the
    // driver strips the suffix when it prints the field.
    let owned: Option<String> = label.map(|l| {
        if names
            .sort_rule
            .keys()
            .any(|sort| rule_for(names, sort) == l)
        {
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
            let r = if ecx.kw_prefer.as_ref().is_some_and(|(sort, _)| sort == name) {
                // The identifier position admits the keywords too.
                ecx.kw_prefer.as_ref().map(|(_, r)| r.clone()).unwrap_or_default()
            } else if names.lexical.contains(name) || plan.lexical_owned.contains_key(name) {
                token_name(name)
            } else {
                rule_for(names, name)
            };
            // ANTLR holds one label to one rule type across a rule's
            // alternatives; a label on a hidden rule (`operator=h_bin_op_mul`,
            // `operator=h_bin_op_add`) is suffixed with the rule, and the
            // driver strips it.
            if r.starts_with("h_") {
                if let Some(l) = label {
                    return Ok(format!("{l}__{r}={r}"));
                }
            }
            lab("=", r)
        }
        Symbol::Lit(l) => lab("=", literal(l)),
        Symbol::Star(inner) => {
            let i = symbol_text(inner, label.map(|_| "").filter(|_| false), names, plan, ecx)?;
            match label {
                Some(l) => format!("({l}+={i})*"),
                None => format!("{i}*"),
            }
        }
        Symbol::Plus(inner) => {
            let i = symbol_text(inner, None, names, plan, ecx)?;
            match label {
                Some(l) => format!("({l}+={i})+"),
                None => format!("{i}+"),
            }
        }
        Symbol::Opt(inner) => {
            let i = symbol_text(inner, None, names, plan, ecx)?;
            match label {
                Some(l) => format!("({l}={i})?"),
                None => format!("{i}?"),
            }
        }
        Symbol::SepList { elem, sep, min } => {
            let e = symbol_text(elem, None, names, plan, ecx)?;
            let sp = symbol_text(sep, None, names, plan, ecx)?;
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
                        .map(|s| symbol_text(s, None, names, plan, ecx))
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

/// A hidden sort's rule cannot start with `_` in ANTLR; `h_` marks it, and
/// the driver elides it as it does injections.
pub fn rule_name(sort: &str) -> String {
    let s = crate::lower::snake(sort);
    if s.starts_with('_') {
        format!("h_{}", s.trim_start_matches('_'))
    } else {
        s
    }
}

/// The parser rule for a sort: the tree-sitter rule name without its
/// hidden-marker underscore, so a single-constructor sort is named for its
/// constructor there and here alike (`Else.ElseClause` is `else_clause`).
fn rule_for(names: &Names, sort: &str) -> String {
    if sort.starts_with('_') {
        // A hidden sort's rule keeps its marker as `h_`, which the
        // driver elides.
        return rule_name(sort);
    }
    names
        .sort_rule
        .get(sort)
        .map(|r| r.trim_start_matches('_').to_string())
        .unwrap_or_else(|| rule_name(sort))
}

pub fn token_name(sort: &str) -> String {
    if sort.starts_with('_') {
        format!("H_{}", sort.trim_start_matches('_').to_ascii_uppercase())
    } else {
        sort.to_ascii_uppercase()
    }
}

fn is_word_lit(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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
