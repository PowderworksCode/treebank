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
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.variants.is_empty()
    }

    /// External names in declaration order, sentinel last.
    pub fn externals(&self) -> Vec<String> {
        let mut v: Vec<String> = self.variants.iter().map(|v| v.name.clone()).collect();
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

    for (pi, p) in prods.iter().enumerate() {
        let symbols = p.symbols();
        for c in p.layout_constraints() {
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
    let mut plan = Plan::default();
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

fn classify(c: &LayoutConstraint) -> Option<(usize, bool)> {
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

fn render(c: &LayoutConstraint) -> String {
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
    out.push_str("#include \"tree_sitter/parser.h\"\n#include <stdbool.h>\n\n");
    out.push_str("enum TokenType {\n");
    for name in plan.externals() {
        out.push_str(&format!("  {},\n", enum_name(&name)));
    }
    out.push_str("};\n\n");
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
    out.push_str("static bool ends_token(int32_t c) {\n  return c == 0 || c == '\\n' || c == '\\r' || is_layout(c);\n}\n\n");
    let fname = |suffix: &str| format!("tree_sitter_{language}_external_scanner_{suffix}");
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
    out.push_str(&format!(
        "bool {}(void *payload, TSLexer *lexer, const bool *valid) {{\n",
        fname("scan")
    ));
    out.push_str(
        r#"  (void)payload;
  // During error recovery every symbol is marked valid (the sentinel is
  // never produced, so seeing it valid is the tell). Decide by spacing then.
  bool recovery = valid[ERROR_SENTINEL];

  bool space_before = false;
  while (is_layout(lexer->lookahead)) {
    lexer->advance(lexer, true);
    space_before = true;
  }
  int32_t c = lexer->lookahead;

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
        let c = LayoutConstraint {
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
        assert_eq!(classify(&c), Some((1, true)));
        let sep = LayoutConstraint {
            op: LayoutOp::Lt,
            ..c.clone()
        };
        assert_eq!(classify(&sep), Some((1, false)));
        let skip = LayoutConstraint {
            rhs: LayoutPos {
                symbol: 3,
                end: LayoutEnd::First,
                axis: LayoutAxis::Col,
            },
            ..c
        };
        assert_eq!(classify(&skip), None);
    }
}
