//! Lexical sorts as an NFA the generated scanner simulates.
//!
//! tree-sitter's external scanner reads one character at a time and cannot
//! step back, so a lexical sort it owns cannot be matched by a backtracking
//! matcher. It can be matched by Thompson's construction: every production
//! of the sort becomes a small automaton over character classes, the
//! scanner keeps the set of live states as it advances, and the token ends
//! at the last position where an accepting state was live -- `mark_end` at
//! each such position is the longest match, with no backtracking. A follow
//! restriction on a sort (`DELIM -/- [a-z]`) is a guard on the epsilon
//! edge that leaves the sort's sub-automaton: it may be crossed only when
//! the next character is outside the class. The one capture a scanner
//! needs, the heredoc's delimiter word, is a pair of tagged epsilon edges
//! that record the position where the captured sort begins and ends.

use std::collections::BTreeMap;

use anyhow::{bail, Result};

use crate::ast::*;

#[derive(Debug, Clone, Default)]
pub struct Nfa {
    pub states: Vec<State>,
    /// Character classes the edges refer to, deduplicated.
    pub classes: Vec<CharClass>,
}

#[derive(Debug, Clone, Default)]
pub struct State {
    /// (class index, target)
    pub edges: Vec<(usize, usize)>,
    pub eps: Vec<Eps>,
    pub accept: bool,
}

#[derive(Debug, Clone)]
pub struct Eps {
    pub target: usize,
    /// Class indices; the edge may be crossed only when the next character
    /// is in none of them.
    pub guard: Vec<usize>,
    pub tag: Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    None,
    CapStart,
    CapEnd,
}

pub struct Builder<'m> {
    lexical: BTreeMap<&'m str, Vec<&'m Production>>,
    /// Single-class follow restrictions per sort.
    follow: BTreeMap<&'m str, Vec<&'m CharClass>>,
    pub nfa: Nfa,
    /// Restrictions the automaton could not carry (multi-class lookaheads).
    pub dropped: Vec<String>,
}

impl<'m> Builder<'m> {
    pub fn new(module: &'m Module) -> Self {
        let mut lexical: BTreeMap<&str, Vec<&Production>> = BTreeMap::new();
        for p in module.productions(true) {
            lexical.entry(p.sort.as_str()).or_default().push(p);
        }
        let mut follow: BTreeMap<&str, Vec<&CharClass>> = BTreeMap::new();
        let mut dropped = Vec::new();
        for r in module.restrictions(true) {
            for s in &r.symbols {
                for la in &r.lookaheads {
                    if la.len() == 1 {
                        follow.entry(s.as_str()).or_default().push(&la[0]);
                    } else {
                        dropped.push(s.clone());
                    }
                }
            }
        }
        Builder {
            lexical,
            follow,
            nfa: Nfa::default(),
            dropped,
        }
    }

    fn state(&mut self) -> usize {
        self.nfa.states.push(State::default());
        self.nfa.states.len() - 1
    }

    fn class(&mut self, c: &CharClass) -> usize {
        if let Some(i) = self.nfa.classes.iter().position(|k| k == c) {
            return i;
        }
        self.nfa.classes.push(c.clone());
        self.nfa.classes.len() - 1
    }

    fn eps(&mut self, from: usize, to: usize, guard: Vec<usize>, tag: Tag) {
        self.nfa.states[from].eps.push(Eps {
            target: to,
            guard,
            tag,
        });
    }

    /// The sort as a token: its start state. `capture` names the sort
    /// whose span the tagged edges record.
    pub fn token(&mut self, sort: &str, capture: Option<&str>) -> Result<usize> {
        let start = self.state();
        let end = self.state();
        self.nfa.states[end].accept = true;
        let mut stack = Vec::new();
        self.sort_into(sort, start, end, capture, &mut stack)?;
        Ok(start)
    }

    fn sort_into(
        &mut self,
        sort: &str,
        from: usize,
        to: usize,
        capture: Option<&str>,
        stack: &mut Vec<String>,
    ) -> Result<()> {
        if stack.iter().any(|s| s == sort) {
            bail!("lexical sort {sort} is recursive; the scanner's automaton cannot carry it");
        }
        let Some(prods) = self.lexical.get(sort).cloned() else {
            bail!("lexical sort {sort} referenced but not defined");
        };
        stack.push(sort.to_string());
        let guard: Vec<usize> = self
            .follow
            .get(sort)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|c| self.class(c))
            .collect();
        let captured = capture == Some(sort);
        let inner_from = if captured {
            let s = self.state();
            self.eps(from, s, Vec::new(), Tag::CapStart);
            s
        } else {
            from
        };
        let inner_to = if captured || !guard.is_empty() {
            let s = self.state();
            self.eps(
                s,
                to,
                guard,
                if captured { Tag::CapEnd } else { Tag::None },
            );
            s
        } else {
            to
        };
        for p in prods {
            if p.has(&Attr::Reject) {
                continue;
            }
            let Rhs::Symbols(syms) = &p.rhs else {
                bail!("lexical sort {sort} uses a template; unsupported");
            };
            self.seq_into(syms, inner_from, inner_to, capture, stack)?;
        }
        stack.pop();
        Ok(())
    }

    fn seq_into(
        &mut self,
        syms: &[Symbol],
        from: usize,
        to: usize,
        capture: Option<&str>,
        stack: &mut Vec<String>,
    ) -> Result<()> {
        if syms.is_empty() {
            self.eps(from, to, Vec::new(), Tag::None);
            return Ok(());
        }
        let mut cur = from;
        for (k, s) in syms.iter().enumerate() {
            let next = if k + 1 == syms.len() {
                to
            } else {
                self.state()
            };
            self.sym_into(s, cur, next, capture, stack)?;
            cur = next;
        }
        Ok(())
    }

    fn sym_into(
        &mut self,
        s: &Symbol,
        from: usize,
        to: usize,
        capture: Option<&str>,
        stack: &mut Vec<String>,
    ) -> Result<()> {
        match s {
            Symbol::CharClass(c) => {
                let k = self.class(c);
                self.nfa.states[from].edges.push((k, to));
            }
            Symbol::Lit(l) => {
                let chars: Vec<char> = l.chars().collect();
                let mut cur = from;
                for (i, ch) in chars.iter().enumerate() {
                    let next = if i + 1 == chars.len() {
                        to
                    } else {
                        self.state()
                    };
                    let k = self.class(&CharClass {
                        negated: false,
                        ranges: vec![(*ch, *ch)],
                    });
                    self.nfa.states[cur].edges.push((k, next));
                    cur = next;
                }
                if chars.is_empty() {
                    self.eps(from, to, Vec::new(), Tag::None);
                }
            }
            Symbol::Sort(n) => self.sort_into(n, from, to, capture, stack)?,
            Symbol::Star(inner) => {
                let s1 = self.state();
                let s2 = self.state();
                self.eps(from, s1, Vec::new(), Tag::None);
                self.sym_into(inner, s1, s2, capture, stack)?;
                self.eps(s2, s1, Vec::new(), Tag::None);
                self.eps(s1, to, Vec::new(), Tag::None);
            }
            Symbol::Plus(inner) => {
                let s1 = self.state();
                let s2 = self.state();
                self.eps(from, s1, Vec::new(), Tag::None);
                self.sym_into(inner, s1, s2, capture, stack)?;
                self.eps(s2, s1, Vec::new(), Tag::None);
                self.eps(s2, to, Vec::new(), Tag::None);
            }
            Symbol::Opt(inner) => {
                self.eps(from, to, Vec::new(), Tag::None);
                self.sym_into(inner, from, to, capture, stack)?;
            }
            Symbol::Group(alts) => {
                for a in alts {
                    self.seq_into(a, from, to, capture, stack)?;
                }
            }
            Symbol::SepList { .. } => bail!("a separated list in lexical syntax is unsupported"),
        }
        Ok(())
    }

    /// Whether a token whose automaton starts at `start` can begin with
    /// `c`: the closure of the start state (guards read `c` as the next
    /// character) has an edge that accepts `c`.
    pub fn can_start(&self, start: usize, c: char) -> bool {
        let mut seen = vec![false; self.nfa.states.len()];
        let mut stack = vec![start];
        while let Some(s) = stack.pop() {
            if seen[s] {
                continue;
            }
            seen[s] = true;
            let st = &self.nfa.states[s];
            for (k, _) in &st.edges {
                if self.nfa.classes[*k].contains(c) {
                    return true;
                }
            }
            for e in &st.eps {
                if e.guard.iter().any(|g| self.nfa.classes[*g].contains(c)) {
                    continue;
                }
                stack.push(e.target);
            }
        }
        false
    }

    /// Every lexical sort a sort's definition refers to, transitively.
    pub fn referenced(&self, sort: &str) -> Vec<String> {
        fn walk(s: &Symbol, out: &mut Vec<String>) {
            match s {
                Symbol::Sort(n) => out.push(n.clone()),
                Symbol::Star(i) | Symbol::Plus(i) | Symbol::Opt(i) => walk(i, out),
                Symbol::SepList { elem, sep, .. } => {
                    walk(elem, out);
                    walk(sep, out);
                }
                Symbol::Group(alts) => alts.iter().flatten().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        let mut todo = vec![sort.to_string()];
        while let Some(s) = todo.pop() {
            for p in self.lexical.get(s.as_str()).into_iter().flatten() {
                if let Rhs::Symbols(syms) = &p.rhs {
                    let mut refs = Vec::new();
                    syms.iter().for_each(|x| walk(x, &mut refs));
                    for r in refs {
                        if !out.contains(&r) {
                            out.push(r.clone());
                            todo.push(r);
                        }
                    }
                }
            }
        }
        out
    }

    /// Every character the sort's text can contain, or None if a negated
    /// class makes the alphabet open.
    pub fn alphabet(&self, sort: &str) -> Option<Vec<char>> {
        fn walk(s: &Symbol, out: &mut Vec<char>) -> bool {
            match s {
                Symbol::CharClass(c) => {
                    if c.negated {
                        return false;
                    }
                    for (a, b) in &c.ranges {
                        let mut ch = *a;
                        loop {
                            out.push(ch);
                            if ch >= *b {
                                break;
                            }
                            ch = char::from_u32(ch as u32 + 1).unwrap_or(*b);
                        }
                    }
                    true
                }
                Symbol::Lit(l) => {
                    out.extend(l.chars());
                    true
                }
                Symbol::Sort(_) => true,
                Symbol::Star(i) | Symbol::Plus(i) | Symbol::Opt(i) => walk(i, out),
                Symbol::SepList { elem, sep, .. } => walk(elem, out) && walk(sep, out),
                Symbol::Group(alts) => alts.iter().flatten().all(|s| walk(s, out)),
            }
        }
        let mut out = Vec::new();
        let mut sorts = vec![sort.to_string()];
        sorts.extend(self.referenced(sort));
        for s in sorts {
            for p in self.lexical.get(s.as_str()).into_iter().flatten() {
                if let Rhs::Symbols(syms) = &p.rhs {
                    if !syms.iter().all(|x| walk(x, &mut out)) {
                        return None;
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        Some(out)
    }
}

/// The automaton as C tables: `CLASSES`, `STATES`, and the matcher the
/// scanner drives (see `scanner::c_source`).
pub fn c_tables(nfa: &Nfa, state_tok: &[usize]) -> String {
    let mut out = String::new();
    out.push_str("// The character that stands for end of input in a guard.\n#define EOF_CP 0x10FFFF\n\n");
    out.push_str("typedef struct { int32_t lo, hi; } Range;\ntypedef struct { const Range *ranges; uint16_t n; bool negated; } Class;\n");
    for (i, c) in nfa.classes.iter().enumerate() {
        if c.ranges.is_empty() {
            continue;
        }
        let rs: Vec<String> = c
            .ranges
            .iter()
            .map(|(a, b)| format!("{{{}, {}}}", *a as u32, *b as u32))
            .collect();
        out.push_str(&format!("static const Range R_{i}[] = {{{}}};\n", rs.join(", ")));
    }
    out.push_str("static const Class CLASSES[] = {\n");
    for (i, c) in nfa.classes.iter().enumerate() {
        if c.ranges.is_empty() {
            out.push_str(&format!("  {{NULL, 0, {}}},\n", c.negated));
        } else {
            out.push_str(&format!(
                "  {{R_{i}, {}, {}}},\n",
                c.ranges.len(),
                c.negated
            ));
        }
    }
    out.push_str("};\n\n");
    out.push_str("typedef struct { uint16_t target; int16_t cls; } Edge;\ntypedef struct { uint16_t target; int16_t guard[4]; uint8_t n_guard; uint8_t tag; } Eps;\ntypedef struct { const Edge *edges; uint8_t n_edges; const Eps *eps; uint8_t n_eps; bool accept; uint8_t tok; } State;\n");
    for (i, s) in nfa.states.iter().enumerate() {
        if !s.edges.is_empty() {
            let es: Vec<String> = s
                .edges
                .iter()
                .map(|(k, t)| format!("{{{t}, {k}}}"))
                .collect();
            out.push_str(&format!("static const Edge E_{i}[] = {{{}}};\n", es.join(", ")));
        }
        if !s.eps.is_empty() {
            let es: Vec<String> = s
                .eps
                .iter()
                .map(|e| {
                    let mut g: Vec<String> = e.guard.iter().map(|k| k.to_string()).collect();
                    while g.len() < 4 {
                        g.push("-1".into());
                    }
                    let tag = match e.tag {
                        Tag::None => 0,
                        Tag::CapStart => 1,
                        Tag::CapEnd => 2,
                    };
                    format!(
                        "{{{}, {{{}}}, {}, {}}}",
                        e.target,
                        g.join(", "),
                        e.guard.len().min(4),
                        tag
                    )
                })
                .collect();
            out.push_str(&format!("static const Eps P_{i}[] = {{{}}};\n", es.join(", ")));
        }
    }
    out.push_str(&format!(
        "#define N_STATES {}\nstatic const State STATES[N_STATES] = {{\n",
        nfa.states.len()
    ));
    for (i, s) in nfa.states.iter().enumerate() {
        out.push_str(&format!(
            "  {{{}, {}, {}, {}, {}, {}}},\n",
            if s.edges.is_empty() {
                "NULL".to_string()
            } else {
                format!("E_{i}")
            },
            s.edges.len(),
            if s.eps.is_empty() {
                "NULL".to_string()
            } else {
                format!("P_{i}")
            },
            s.eps.len(),
            s.accept,
            state_tok.get(i).copied().unwrap_or(0)
        ));
    }
    out.push_str("};\n\n");
    out.push_str(
        r#"static bool class_has(int cls, int32_t c) {
  const Class *k = &CLASSES[cls];
  bool in = false;
  for (unsigned i = 0; i < k->n; i++) {
    if (c >= k->ranges[i].lo && c <= k->ranges[i].hi) { in = true; break; }
  }
  // End of input is in a class only when the class names it.
  if (c == EOF_CP) return in && !k->negated;
  return in != k->negated;
}

"#,
    );
    out
}
