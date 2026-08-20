//! Generate programs FROM the grammar and ask the oracle whether they are
//! in the language (DESIGN.md §5.9).
//!
//! Every other check starts from source somebody wrote. The sweep reads the
//! corpus; `mutate` perturbs it; `roundtrip` reprints it. All three are
//! bounded by what the corpus happens to contain, and the accepts-invalid
//! direction is exactly where that bound bites: real source is valid, so no
//! amount of it demonstrates that we reject what the language rejects.
//!
//! `grammar.json` is the generator. It is what `tree-sitter generate`
//! normalises `grammar.js` into, and it is already an EBNF syntax tree, so
//! a random derivation is a walk that chooses branches and emits terminals.
//! No unparser is needed in this direction — the grammar IS the emitter.
//! Then the oracle judges. Anything we accept and it rejects is a widening,
//! and unlike a corpus finding it arrives already minimal.
//!
//! **The soundness argument, because the generator is not faithful.**
//! Joining tokens with spaces is a lie: `'a` is a lifetime and `' a` is not,
//! so some derivations come out as text whose tokenisation differs from the
//! derivation that produced it. That does not weaken a finding. We report a
//! case only when OUR parser accepts the text and the ORACLE rejects it, and
//! that pair is a widening whatever derivation produced the bytes — the
//! grammar accepting a program the language does not is the defect, and how
//! we came to type it is irrelevant. Generator infidelity costs yield, never
//! correctness: an unfaithful derivation that we then reject is discarded
//! before it is ever reported.
//!
//! **Ask the parser, not the compiler.** Where the oracle can separate the
//! two, this uses `validate_syntax_only`. The first python run made the
//! reason obvious: nearly every finding was `break`, `yield` or `* x` at
//! module level — all of which CPython's PARSER accepts and its COMPILER
//! rejects. "`break` outside a loop" is not a syntax error, and a
//! tree-sitter grammar has no business tracking loop nesting to produce
//! one. Judged by `compile()` the fuzzer mostly rediscovers CPython's
//! semantic checks; judged by `ast.parse` it reports what it is for. Where
//! an oracle cannot make the distinction — rust's `syn` has no such mode —
//! it falls back to `validate`, and the fallback is stated in the report so
//! a reader knows which question was asked.
//!
//! **External tokens have no text, so the generator supplies it.** An
//! external token is a hand-written C scanner; `grammar.json` carries only
//! its name. For rust that is a small matter — `float`, `raw_string` and
//! `block_comment` all have obvious spellings. Python is the interesting
//! case, because `_newline`, `_indent` and `_dedent` are LAYOUT: there is no
//! string that means "indent". So the generator carries an indent level,
//! and the renderer reconstitutes layout from it — `_indent` and `_dedent`
//! move the level and emit nothing, while a newline is rendered together
//! with the indentation in force for the line that FOLLOWS it. That falls
//! out of the grammar's own shape, `seq(_newline, _indent, line+, _dedent)`:
//! the dedent lowers the level before the next line is rendered, which is
//! exactly what makes the block close.
//!
//! **Declared widenings.** Some over-acceptance is deliberate: python's
//! grammar is 2.7 ∪ 3.x by design, so `print x` is a widening against py3's
//! parser and is meant to be. Left undeclared, that one decision dominates
//! every run and buries the findings that are not decisions. So each
//! grammar may carry a `fuzz_policy.toml` naming what it accepts on
//! purpose, and the report separates declared from undeclared.
//!
//! Entries match a PREFIX of the shrunk program, and narrowly — the same
//! discipline `shape_policy.toml` uses, for the same reason: a blanket
//! ignore silences the real finding that arrives next week wearing similar
//! clothes. Declaring `print ` is a claim about py2 print statements;
//! declaring nothing at all would have been better than declaring `p`.
//!
//! **Where the corpus cannot see** — behind `--rare`, and off by default,
//! because it was measured across four languages and helps exactly one.
//! Java gains 17.9% more distinct findings (25 paired wins of 30 seeds,
//! sign test p=0.00016); rust LOSES 21.1% and typescript 18.7%, each
//! losing all 8 of 8 paired seeds; python has nothing to steer toward at
//! all, because 297,612 files exercise all 104 of its node kinds.
//!
//! The difference is what the rare set IS. Java's is a coherent
//! under-modelled region -- `guard`, `unnamed_pattern` and `record_pattern`
//! are the whole of java 21 pattern matching, absent from a quarter of a
//! million files. Rust's is `yield_expression`, an unstable feature `syn`
//! rejects anyway, and `shebang`, which is one line. TypeScript's is seven
//! unrelated corners at already-98.8% coverage. Steering concentrates the
//! budget, and concentration only pays when the region is both untested
//! and large enough to hold bugs; otherwise it costs the diversity that
//! finds them.
//!
//! **The original reasoning.** The sweep is only as good as the code
//! it reads: a construct no corpus file contains is one the oracle has
//! never been asked about, so a bug there is invisible to every check that
//! starts from real source. `treebank kinds` measures exactly that, and if
//! its report is present this steers generation toward the node kinds real
//! code never produces — java's `guard` and `unnamed_pattern` appear in
//! zero of 243,573 files, and `record_pattern` in two. Those are the
//! constructs where a generated program is worth the most, because nothing
//! else can reach them.
//!
//! **Shrinking is over the choice tape, not the program.** Generation
//! consumes a byte tape and is deterministic in it, so shrinking is a search
//! for a shorter, smaller tape that still reproduces — Hypothesis's model
//! rather than proptest's typed `Strategy`. That matters here because the
//! grammar is runtime data read from a file: a typed strategy would have to
//! be written per grammar, whereas the tape does not care what it is driving.
//! Reduced examples also collapse together, so the tape doubles as the
//! clustering key: twenty seeds that find one bug shrink to one line.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use tree_sitter::Parser;

use treebank_lang::LangName;

/// grammar.json, as the sixteen shapes it actually uses.
#[derive(Debug, Clone)]
enum Rule {
    Blank,
    Str(String),
    Pattern(String),
    Symbol(String),
    Seq(Vec<Rule>),
    /// The `usize` is a site id, unique across the grammar. Coverage is
    /// keyed on `(site, alternative)` rather than `(rule, alternative)`
    /// because a rule may hold several `choice`s and they are different
    /// questions.
    Choice(usize, Vec<Rule>),
    Repeat(Box<Rule>),
    Repeat1(Box<Rule>),
    /// PREC / FIELD / ALIAS / TOKEN / … — no bearing on the text emitted.
    Wrap(Box<Rule>),
}

/// Names every choice site as it is parsed: id -> (rule, arity). The
/// report needs this to say WHICH alternative was never taken, which is the
/// half of a coverage number that is actually actionable.
#[derive(Default)]
struct Sites {
    of: Vec<(String, usize)>,
}

fn parse_rule(v: &Value) -> Result<Rule> {
    let mut sites = Sites::default();
    parse_rule_at(v, "", &mut sites)
}

fn parse_rule_at(v: &Value, rule: &str, sites: &mut Sites) -> Result<Rule> {
    let t = v["type"].as_str().context("grammar node without a type")?;
    let mut members = |sites: &mut Sites| -> Result<Vec<Rule>> {
        v["members"]
            .as_array()
            .context("node without members")?
            .iter()
            .map(|m| parse_rule_at(m, rule, sites))
            .collect()
    };
    Ok(match t {
        "BLANK" => Rule::Blank,
        "STRING" => Rule::Str(v["value"].as_str().unwrap_or_default().to_string()),
        "PATTERN" => Rule::Pattern(v["value"].as_str().unwrap_or_default().to_string()),
        "SYMBOL" => Rule::Symbol(v["name"].as_str().unwrap_or_default().to_string()),
        "SEQ" => Rule::Seq(members(sites)?),
        "CHOICE" => {
            let ms = members(sites)?;
            let id = sites.of.len();
            sites.of.push((rule.to_string(), ms.len()));
            Rule::Choice(id, ms)
        }
        "REPEAT" => Rule::Repeat(Box::new(parse_rule_at(&v["content"], rule, sites)?)),
        "REPEAT1" => Rule::Repeat1(Box::new(parse_rule_at(&v["content"], rule, sites)?)),
        "PREC" | "PREC_LEFT" | "PREC_RIGHT" | "PREC_DYNAMIC" | "FIELD" | "ALIAS" | "TOKEN"
        | "IMMEDIATE_TOKEN" | "RESERVED" => {
            Rule::Wrap(Box::new(parse_rule_at(&v["content"], rule, sites)?))
        }
        other => anyhow::bail!("unhandled grammar node {other}"),
    })
}

/// The name of the first rule under `"rules"`, in file order.
fn first_rule_name(text: &str) -> Option<String> {
    let rest = &text[text.find("\"rules\"")?..];
    let rest = &rest[rest.find('{')? + 1..];
    let open = rest.find('"')?;
    let after = &rest[open + 1..];
    let close = after.find('"')?;
    Some(after[..close].to_string())
}

struct Grammar {
    rules: BTreeMap<String, Rule>,
    /// Rules from which a corpus-rare node kind is reachable. Empty when
    /// no `kinds.json` has been produced, which turns the steering off
    /// rather than guessing at rarity.
    reaches_rare: std::collections::HashSet<String>,
    /// Choice site id -> (rule it belongs to, number of alternatives).
    sites: Vec<(String, usize)>,
    start: String,
    /// Shortest derivation length per rule, so a bounded generator can
    /// always pick an alternative that terminates instead of recursing.
    min_len: HashMap<String, usize>,
}

const UNBOUNDED: usize = 1 << 20;

impl Grammar {
    fn load(path: &Path) -> Result<Grammar> {
        let text = std::fs::read_to_string(path)?;
        let v: Value =
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        let obj = v["rules"]
            .as_object()
            .context("grammar.json has no rules")?;
        // The start symbol is the FIRST rule as authored, and serde_json's
        // map is sorted, so it cannot be recovered from the parsed value --
        // asking it for the first key yields whatever sorts first, which for
        // rust is `_access`. Read the order back off the raw text.
        let start = first_rule_name(&text)
            .filter(|n| obj.contains_key(n))
            .context("could not find the start rule in grammar.json")?;
        let mut sites = Sites::default();
        let rules = obj
            .iter()
            .map(|(k, r)| Ok((k.clone(), parse_rule_at(r, k, &mut sites)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut g = Grammar {
            rules,
            reaches_rare: Default::default(),
            sites: sites.of,
            start,
            min_len: HashMap::new(),
        };
        g.compute_min_len();
        Ok(g)
    }

    /// Least-fixed-point over rule lengths. A rule only reachable through
    /// itself stays UNBOUNDED, which is exactly what the depth cut-off needs
    /// to know in order to avoid it.
    fn compute_min_len(&mut self) {
        for k in self.rules.keys() {
            self.min_len.insert(k.clone(), UNBOUNDED);
        }
        loop {
            let mut changed = false;
            let names: Vec<String> = self.rules.keys().cloned().collect();
            for name in names {
                let n = self.rule_min(&self.rules[&name]);
                if n < self.min_len[&name] {
                    self.min_len.insert(name, n);
                    changed = true;
                }
            }
            if !changed {
                return;
            }
        }
    }

    /// Which rules can produce a kind the corpus never (or barely) shows.
    /// Least fixed point, the same shape as `min_len`: a rule reaches rare
    /// if it IS rare, or if anything it references does.
    fn mark_rare(&mut self, rare: &std::collections::HashSet<String>) {
        for name in self.rules.keys() {
            if rare.contains(name) {
                self.reaches_rare.insert(name.clone());
            }
        }
        loop {
            let mut grew = false;
            let names: Vec<String> = self.rules.keys().cloned().collect();
            for name in names {
                if self.reaches_rare.contains(&name) {
                    continue;
                }
                if self.rule_reaches_rare(&self.rules[&name]) {
                    self.reaches_rare.insert(name);
                    grew = true;
                }
            }
            if !grew {
                return;
            }
        }
    }

    fn rule_reaches_rare(&self, r: &Rule) -> bool {
        match r {
            Rule::Symbol(n) => self.reaches_rare.contains(n),
            Rule::Seq(ms) | Rule::Choice(_, ms) => ms.iter().any(|m| self.rule_reaches_rare(m)),
            Rule::Repeat(c) | Rule::Repeat1(c) | Rule::Wrap(c) => self.rule_reaches_rare(c),
            _ => false,
        }
    }

    fn rule_min(&self, r: &Rule) -> usize {
        match r {
            Rule::Blank => 0,
            Rule::Str(s) => s.len().max(1),
            Rule::Pattern(_) => 1,
            Rule::Symbol(n) => self.min_len.get(n).copied().unwrap_or(1),
            Rule::Seq(ms) => ms
                .iter()
                .map(|m| self.rule_min(m))
                .fold(0, usize::saturating_add),
            Rule::Choice(_, ms) => ms.iter().map(|m| self.rule_min(m)).min().unwrap_or(0),
            Rule::Repeat(_) => 0,
            Rule::Repeat1(c) => self.rule_min(c),
            Rule::Wrap(c) => self.rule_min(c),
        }
    }
}

/// The choice tape. Generation reads from it; shrinking rewrites it.
/// Running off the end yields 0, the simplest choice — so a truncated tape
/// still produces a COMPLETE program, which is what lets shrinking cut the
/// tape freely without producing half a sentence.
struct Tape<'a> {
    bytes: &'a [u8],
    pos: usize,
    /// When recording, every decision is appended here in order — choices
    /// AND repeat counts. Recording only the choices was tried and produced
    /// a tape that decoded to a different program, because plain generation
    /// consumes a byte at each repeat too and the two runs fell out of step
    /// after the first one.
    rec: Option<Vec<u8>>,
}

impl Tape<'_> {
    fn exhausted(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn choose(&mut self, n: usize) -> usize {
        if n <= 1 {
            if let Some(r) = self.rec.as_mut() {
                r.push(0);
            }
            return 0;
        }
        let b = self.bytes.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        let idx = b as usize % n;
        if let Some(r) = self.rec.as_mut() {
            r.push(idx as u8);
        }
        idx
    }

    /// Take a specific alternative and record it as if it had been chosen.
    fn take(&mut self, idx: usize) -> usize {
        self.pos += 1;
        if let Some(r) = self.rec.as_mut() {
            r.push(idx as u8);
        }
        idx
    }
}

/// Emit a string a regex can match. Handles the constructs tree-sitter
/// grammars actually use and gives up into "a" otherwise; a bad sample is
/// caught by our own parser rejecting the result, never mistaken for a bug.
fn sample_pattern(re: &str) -> String {
    let b: Vec<char> = re.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() && out.len() < 8 {
        match b[i] {
            '[' => {
                // first literal in the class, skipping a negation
                let mut j = i + 1;
                if j < b.len() && b[j] == '^' {
                    out.push('a');
                    while j < b.len() && b[j] != ']' {
                        j += 1;
                    }
                    i = j + 1;
                    continue;
                }
                let mut ch = 'a';
                while j < b.len() && b[j] != ']' {
                    if b[j] == '\\' {
                        j += 1;
                    } else if b[j].is_alphanumeric() {
                        ch = b[j];
                        break;
                    }
                    j += 1;
                }
                out.push(ch);
                while j < b.len() && b[j] != ']' {
                    j += 1;
                }
                i = j + 1;
            }
            '\\' if i + 1 < b.len() => {
                out.push(match b[i + 1] {
                    'd' => '1',
                    'w' => 'a',
                    's' => ' ',
                    c => c,
                });
                i += 2;
            }
            '(' | ')' | '?' | '*' | '+' | '^' | '$' => i += 1,
            '|' => break,
            '.' => {
                out.push('a');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
        // a following quantifier does not need a second copy
        while i < b.len() && matches!(b[i], '*' | '+' | '?') {
            i += 1;
        }
    }
    if out.trim().is_empty() {
        "a".into()
    } else {
        out
    }
}

/// What an external token contributes. `Newline`/`Indent`/`Dedent` are
/// layout: they carry no text of their own and act on the indent level.
enum Ext {
    Text(&'static str),
    Newline,
    Indent,
    Dedent,
    Nothing,
}

fn external(lang: LangName, name: &str) -> Ext {
    match (lang, name) {
        (LangName::Python, "_newline") => Ext::Newline,
        (LangName::Python, "_indent") => Ext::Indent,
        (LangName::Python, "_dedent") => Ext::Dedent,
        (LangName::Python, "_line_start") => Ext::Nothing,
        (LangName::Python, "string_start") | (LangName::Python, "string_end") => Ext::Text("\""),
        (LangName::Python, "string_content") => Ext::Text("s"),
        (LangName::Rust, "float") => Ext::Text("1.0"),
        (LangName::Rust, "raw_string") => Ext::Text("r\"s\""),
        (LangName::Rust, "block_comment") => Ext::Text("/*c*/"),
        // Unknown: stand something in and let our own parser judge it.
        _ => Ext::Text("a"),
    }
}

/// A token plus the indent level in force where it sits. The level travels
/// with the token because a newline's indentation belongs to the line AFTER
/// it, which is not known until that line's first token is emitted.
enum Tok {
    Text(String, usize),
    Newline,
}

/// Choose an alternative nothing has taken yet, when there is one.
///
/// Mutating tapes toward coverage was tried first and measured: at 1,000
/// iterations it was WORSE than pure random (74.7% against 78.7%) and by
/// 8,000 the two were identical. AFL searches for coverage because it
/// cannot see inside the program; a grammar fuzzer can — the uncovered
/// alternatives are a list we already hold, so the useful move is to take
/// them rather than to search for them.
///
/// The result is written back as a TAPE, one byte per choice, so the
/// program stays a deterministic function of a tape and shrinking is
/// unaffected. That constraint is why this seeks by construction rather
/// than by making generation depend on what has been seen.
struct Seeker<'a> {
    seen: &'a std::collections::HashSet<(u32, u32)>,
    /// Prefer alternatives that can reach a corpus-rare kind, over merely
    /// unvisited ones. Rarity beats novelty: an alternative this run has
    /// not taken may still be one the sweep checks a million times a day,
    /// while a rare kind is unchecked by anything else at all.
    rare: bool,
    tape: Vec<u8>,
}

struct Gen<'a, 'b> {
    g: &'a Grammar,
    /// `(site, alternative)` pairs taken during this derivation. This is
    /// the fitness signal: it measures which alternatives of the grammar
    /// the GENERATOR reached, which is what a blind fuzzer cannot see and
    /// is why it re-finds the same widening forty times.
    ///
    /// It measures the generator, not the parser — a derivation can take an
    /// alternative that precedence or GLR then resolves differently. Node
    /// kinds from the resulting tree are reported alongside as the check on
    /// that, never as the thing being optimised.
    cov: Vec<(u32, u32)>,
    /// Set while building a coverage-seeking tape; `None` for ordinary
    /// generation, which must stay a pure function of its tape.
    seek: Option<Box<Seeker<'a>>>,
    lang: LangName,
    tape: &'b mut Tape<'a>,
    out: Vec<Tok>,
    level: usize,
    steps: usize,
}

const MAX_STEPS: usize = 4000;
const MAX_DEPTH: usize = 18;

impl Gen<'_, '_> {
    fn push(&mut self, text: String) {
        let level = self.level;
        self.out.push(Tok::Text(text, level));
    }

    /// Join tokens with spaces within a line, and start each line with the
    /// indentation its FIRST token was emitted at.
    fn render(&self) -> String {
        let mut s = String::new();
        let mut at_line_start = true;
        for tok in &self.out {
            match tok {
                Tok::Newline => {
                    s.push('\n');
                    at_line_start = true;
                }
                Tok::Text(t, level) => {
                    if at_line_start {
                        for _ in 0..*level {
                            s.push_str("    ");
                        }
                        at_line_start = false;
                    } else {
                        s.push(' ');
                    }
                    s.push_str(t);
                }
            }
        }
        s
    }

    fn emit(&mut self, r: &Rule, depth: usize) {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return;
        }
        match r {
            Rule::Blank => {}
            Rule::Str(s) => self.push(s.clone()),
            Rule::Pattern(p) => {
                let t = sample_pattern(p);
                self.push(t);
            }
            Rule::Symbol(n) => match self.g.rules.get(n) {
                // No rule means an external token: a hand-written scanner,
                // of which grammar.json knows only the name.
                None => match external(self.lang, n) {
                    Ext::Text(t) => self.push(t.to_string()),
                    Ext::Newline => self.out.push(Tok::Newline),
                    Ext::Indent => self.level += 1,
                    Ext::Dedent => self.level = self.level.saturating_sub(1),
                    Ext::Nothing => {}
                },
                Some(rule) => {
                    if depth >= MAX_DEPTH {
                        self.push("a".into());
                    } else {
                        let owned = rule.clone();
                        self.emit(&owned, depth + 1);
                    }
                }
            },
            Rule::Seq(ms) => {
                for m in ms {
                    self.emit(m, depth);
                }
            }
            Rule::Choice(site, ms) => {
                // Off the end of the tape, or past the depth bound, take the
                // SHORTEST alternative rather than the first.
                //
                // Taking the first was a real bug and a subtle one: it made
                // a shorter tape produce a BIGGER program, because choice 0
                // of python's `_expression` is `boolean_expression`, whose
                // operands are expressions again — so an exhausted tape
                // recursed into `a or a or a …` all the way to the depth
                // bound. Shrinking then worked against itself, and reported
                // 400-token "minimal" cases. Shrinking needs shorter tapes
                // to mean simpler programs, and `min_len` is what makes that
                // true by construction.
                let mut return_choice = false;
                let pick = if let Some(seek) = self.seek.as_deref_mut() {
                    // Past the bound the shortest alternative still wins:
                    // seeking a deep unreached branch that cannot terminate
                    // is how a generator runs away.
                    let p = if depth >= MAX_DEPTH {
                        ms.iter()
                            .enumerate()
                            .min_by_key(|(_, m)| self.g.rule_min(m))
                            .map(|(i, _)| i)
                            .unwrap_or(0)
                    } else {
                        let mut fresh: Vec<usize> = (0..ms.len())
                            .filter(|a| !seek.seen.contains(&(*site as u32, *a as u32)))
                            .collect();
                        if seek.rare {
                            let toward: Vec<usize> = (0..ms.len())
                                .filter(|a| self.g.rule_reaches_rare(&ms[*a]))
                                .collect();
                            if !toward.is_empty() {
                                fresh = toward;
                            }
                        }
                        if fresh.is_empty() {
                            return_choice = true;
                            0
                        } else {
                            let k = (self.tape.bytes.get(self.tape.pos).copied().unwrap_or(0)
                                as usize)
                                % fresh.len();
                            fresh[k]
                        }
                    };
                    let _ = seek;
                    if return_choice {
                        self.tape.choose(ms.len())
                    } else {
                        self.tape.take(p)
                    }
                } else if depth >= MAX_DEPTH || self.tape.exhausted() {
                    ms.iter()
                        .enumerate()
                        .min_by_key(|(_, m)| self.g.rule_min(m))
                        .map(|(i, _)| i)
                        .unwrap_or(0)
                } else {
                    self.tape.choose(ms.len())
                };
                self.cov.push((*site as u32, pick as u32));
                let chosen = ms[pick].clone();
                self.emit(&chosen, depth);
            }
            Rule::Repeat(c) => {
                let n = if depth >= MAX_DEPTH {
                    0
                } else {
                    self.tape.choose(3)
                };
                for _ in 0..n {
                    self.emit(c, depth + 1);
                }
            }
            Rule::Repeat1(c) => {
                let n = if depth >= MAX_DEPTH {
                    1
                } else {
                    1 + self.tape.choose(2)
                };
                for _ in 0..n {
                    self.emit(c, depth + 1);
                }
            }
            Rule::Wrap(c) => self.emit(c, depth),
        }
    }
}

fn generate(g: &Grammar, lang: LangName, tape_bytes: &[u8]) -> String {
    derive(g, lang, tape_bytes).0
}

/// The program and the alternatives its derivation took.
fn derive(g: &Grammar, lang: LangName, tape_bytes: &[u8]) -> (String, Vec<(u32, u32)>) {
    let mut tape = Tape {
        bytes: tape_bytes,
        pos: 0,
        rec: None,
    };
    let start = g.rules[&g.start].clone();
    let mut gen = Gen {
        g,
        lang,
        tape: &mut tape,
        out: Vec::new(),
        cov: Vec::new(),
        seek: None,
        level: 0,
        steps: 0,
    };
    gen.emit(&start, 0);
    let text = gen.render();
    // A file that does not end in a newline leaves python's scanner mid-line.
    let text = if text.ends_with('\n') {
        text
    } else {
        text + "\n"
    };
    (text, gen.cov)
}

/// Build a tape that reaches alternatives nothing has reached, by taking
/// them. Returns the tape; the caller generates from it as usual, so
/// everything downstream — shrinking included — is unchanged.
fn seek_tape(
    g: &Grammar,
    lang: LangName,
    seen: &std::collections::HashSet<(u32, u32)>,
    rare: bool,
    rng: &mut Rng,
) -> Vec<u8> {
    let random = rng.tape(256);
    let mut tape = Tape {
        bytes: &random,
        pos: 0,
        rec: Some(Vec::new()),
    };
    let start = g.rules[&g.start].clone();
    let mut gen = Gen {
        g,
        lang,
        tape: &mut tape,
        out: Vec::new(),
        cov: Vec::new(),
        seek: Some(Box::new(Seeker {
            seen,
            rare,
            tape: Vec::new(),
        })),
        level: 0,
        steps: 0,
    };
    gen.emit(&start, 0);
    let rec = gen.tape.rec.take().unwrap_or_default();
    rec
}

/// How many tapes to keep. A cap rather than a policy: the corpus is only
/// a source of things to mutate, and an unbounded one just dilutes it.
const MAX_CORPUS: usize = 256;

/// Perturb a tape that is known to reach somewhere. The operators are the
/// shrinking passes run backwards — splice, duplicate, and disturb a byte —
/// which is the whole benefit of generation being a function of a tape.
fn mutate_tape(tape: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut out = tape.to_vec();
    if out.is_empty() {
        return rng.tape(64);
    }
    match rng.next() % 4 {
        // Disturb one choice, leaving every other decision intact.
        0 => {
            let i = (rng.next() as usize) % out.len();
            out[i] = (rng.next() >> 24) as u8;
        }
        // Duplicate a run: reaches deeper nesting than a flat tape does.
        1 => {
            let i = (rng.next() as usize) % out.len();
            let n = ((rng.next() as usize) % 16).min(out.len() - i);
            let chunk: Vec<u8> = out[i..i + n].to_vec();
            out.splice(i..i, chunk);
        }
        // Extend: the tail decides the deepest, least-explored choices.
        2 => out.extend(rng.tape(16)),
        // Truncate: shorter tapes take the shortest alternative, which is
        // where the terminating cases live.
        _ => {
            let n = (rng.next() as usize) % out.len();
            out.truncate(n.max(1));
        }
    }
    out
}

/// xorshift64*, so a run is reproducible from its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn tape(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| (self.next() >> 24) as u8).collect()
    }
}

#[derive(Serialize)]
pub struct Finding {
    /// Set when the grammar declares this shape as deliberate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared: Option<String>,
    /// The shrunk program: the smallest text still reproducing.
    pub program: String,
    pub bytes: usize,
    /// How many independent seeds reduced to this same example.
    pub seeds: usize,
}

/// Which alternatives of the grammar the run reached, and — the half that
/// is actually actionable — which it never did.
#[derive(Serialize)]
pub struct Coverage {
    pub alternatives_total: usize,
    pub alternatives_covered: usize,
    pub percent: f64,
    /// Rules with alternatives no derivation ever took, commonest first.
    /// This doubles as a to-do list for hand-written corpus tests: it names
    /// constructs in our own grammar that nothing has exercised.
    pub uncovered_by_rule: Vec<(String, usize)>,
}

#[derive(Serialize)]
pub struct FuzzReport {
    pub lang: String,
    pub grammar: String,
    /// Which question the oracle was asked. `parser` means the reference
    /// tool has a parse-only mode and we used it; `parser+compiler` means it
    /// has none, so findings may include checks a compiler runs afterwards.
    pub judged_by: &'static str,
    pub iterations: usize,
    /// Generated programs our parser accepted — the only ones worth asking
    /// the oracle about.
    pub we_accepted: usize,
    /// Distinct shapes the accepted programs built. Comparable to the
    /// `kinds` report over the corpus: the interesting number is not either
    /// alone but what fuzzing reaches that no real file does.
    pub built_kinds: usize,
    pub built_tokens: usize,
    pub built_edges: usize,
    pub built_kind_names: Vec<String>,
    pub built_token_names: Vec<String>,
    pub built_edge_names: Vec<String>,
    /// Generated programs our parser rejected. These are generator
    /// infidelity (token spacing, sampled patterns), not grammar defects,
    /// and are discarded rather than reported.
    pub we_rejected: usize,
    /// Accepted by us AND by the oracle: agreement, the expected case.
    pub agreed: usize,
    pub widenings: usize,
    pub coverage: Coverage,
    /// Tapes kept because they reached an alternative nothing had reached.
    pub corpus_size: usize,
    pub findings: Vec<Finding>,
}

/// What a grammar accepts on purpose. Absent file means nothing is declared.
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzPolicy {
    #[serde(default)]
    rule: String,
    #[serde(default)]
    declared: Vec<Declared>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Declared {
    /// Matches a shrunk program that starts with this text.
    starts_with: String,
    why: String,
}

impl FuzzPolicy {
    fn load(grammar_dir: &Path) -> Result<FuzzPolicy> {
        let path = grammar_dir.join("fuzz_policy.toml");
        if !path.exists() {
            return Ok(FuzzPolicy::default());
        }
        let text = std::fs::read_to_string(&path)?;
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }

    fn declared_reason(&self, program: &str) -> Option<&str> {
        self.declared
            .iter()
            .find(|d| program.starts_with(&d.starts_with))
            .map(|d| d.why.as_str())
    }
}

struct Judge<'a> {
    oracle: &'static dyn treebank_oracle::Oracle,
    dir: &'a Path,
    lang: LangName,
}

impl Judge<'_> {
    /// Does the language accept these? Batched, because for every language
    /// but rust the oracle is a subprocess and the cost is per CALL, not
    /// per file: asking about one program 300 times to shrink it spends 300
    /// interpreter startups to do a second of work.
    fn accepts_many(&self, texts: &[String]) -> Result<Vec<bool>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let ext = self.lang.primary_extension();
        let names: Vec<String> = (0..texts.len())
            .map(|i| format!("probe{i}.{ext}"))
            .collect();
        for (name, text) in names.iter().zip(texts) {
            std::fs::write(self.dir.join(name), text)?;
        }
        let verdicts = match self.oracle.validate_syntax_only(self.dir, &names)? {
            Some(v) => v,
            None => self.oracle.validate(self.dir, &names)?,
        };
        let out = names
            .iter()
            .map(|n| verdicts.get(n).copied().unwrap_or(false))
            .collect();
        for name in &names {
            let _ = std::fs::remove_file(self.dir.join(name));
        }
        Ok(out)
    }

    fn accepts(&self, text: &str) -> Result<bool> {
        Ok(self.accepts_many(std::slice::from_ref(&text.to_string()))?[0])
    }
}

/// Is this a widening? We accept it and the language does not.
fn is_widening(
    g: &Grammar,
    parser: &mut Parser,
    judge: &Judge,
    tape: &[u8],
) -> Result<Option<String>> {
    let text = generate(g, judge.lang, tape);
    if text.trim().is_empty() {
        return Ok(None);
    }
    let Some(tree) = parser.parse(text.as_bytes(), None) else {
        return Ok(None);
    };
    if tree.root_node().has_error() {
        return Ok(None); // our own generator lied; not a finding
    }
    if judge.accepts(&text)? {
        return Ok(None);
    }
    Ok(Some(text))
}

const SHRINK_ROUNDS: usize = 40;

/// Which of these tapes still reproduces? Our own parser is consulted per
/// candidate because it is in-process and cheap; the oracle is asked once
/// for everything that survived, which is what makes shrinking affordable
/// when the oracle is a subprocess.
fn surviving(
    g: &Grammar,
    parser: &mut Parser,
    judge: &Judge,
    tapes: &[Vec<u8>],
) -> Result<Vec<usize>> {
    let mut texts = Vec::new();
    let mut owners = Vec::new();
    for (i, tape) in tapes.iter().enumerate() {
        let text = generate(g, judge.lang, tape);
        if text.trim().is_empty() {
            continue;
        }
        let Some(tree) = parser.parse(text.as_bytes(), None) else {
            continue;
        };
        if tree.root_node().has_error() {
            continue; // our generator lied; not a finding
        }
        texts.push(text);
        owners.push(i);
    }
    let verdicts = judge.accepts_many(&texts)?;
    Ok(owners
        .into_iter()
        .zip(verdicts)
        .filter(|(_, accepted)| !accepted) // the oracle REJECTS it: reproduces
        .map(|(i, _)| i)
        .collect())
}

/// Search for a shorter, smaller tape that still reproduces. Two passes in
/// the order that pays: delete runs of choices first, since that collapses
/// a program to its core, then walk individual bytes toward zero, which
/// moves each surviving choice to the grammar's first alternative.
///
/// Each pass proposes every candidate at once and takes the best that
/// survives, rather than testing candidates one at a time. That is a
/// slightly different search from strict greedy descent and a very
/// different cost: one oracle call per pass instead of one per candidate.
fn shrink(g: &Grammar, parser: &mut Parser, judge: &Judge, tape: &[u8]) -> Result<Vec<u8>> {
    let mut best = tape.to_vec();

    for _ in 0..SHRINK_ROUNDS {
        let mut improved = false;

        let mut width = best.len().max(1);
        while width > 0 {
            let candidates: Vec<Vec<u8>> = (0..best.len())
                .step_by(width.max(1))
                .map(|i| {
                    let mut c = best.clone();
                    c.drain(i..(i + width).min(c.len()));
                    c
                })
                .collect();
            let alive = surviving(g, parser, judge, &candidates)?;
            if let Some(&k) = alive.iter().min_by_key(|&&k| candidates[k].len()) {
                best = candidates[k].clone();
                improved = true;
            } else {
                width /= 2;
            }
        }

        // Lower each byte as far as it will go, all positions proposed at once.
        let candidates: Vec<Vec<u8>> = best
            .iter()
            .enumerate()
            .filter(|(_, b)| **b > 0)
            .map(|(i, _)| {
                let mut c = best.clone();
                c[i] = 0;
                c
            })
            .collect();
        let alive = surviving(g, parser, judge, &candidates)?;
        for k in alive {
            // Re-apply on top of `best` so several positions can drop together.
            let mut merged = best.clone();
            for (i, b) in candidates[k].iter().enumerate() {
                if i < merged.len() && *b == 0 {
                    merged[i] = 0;
                }
            }
            if !surviving(g, parser, judge, &[merged.clone()])?.is_empty() {
                if merged != best {
                    best = merged;
                    improved = true;
                }
            }
        }

        if !improved {
            break;
        }
    }
    Ok(best)
}

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    iterations: usize,
    seed: u64,
    unguided: bool,
    rare: bool,
    out_path: &Path,
) -> Result<()> {
    let mut g = Grammar::load(&grammar_dir.join("src/grammar.json"))?;
    let policy = FuzzPolicy::load(grammar_dir)?;

    // `treebank kinds` writes this. Absent, the steering is simply off.
    let kinds_path = PathBuf::from(format!("corpus/{lang}/reports/kinds.json"));
    let rare_kinds: std::collections::HashSet<String> = if !rare {
        Default::default()
    } else {
        match std::fs::read_to_string(&kinds_path) {
            Ok(text) => {
                let v: serde_json::Value = serde_json::from_str(&text)?;
                let never = v["never_seen"].as_array().cloned().unwrap_or_default();
                let thin = v["thin"].as_array().cloned().unwrap_or_default();
                never
                    .iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .chain(
                        thin.iter()
                            .filter_map(|x| x.as_array()?.first()?.as_str().map(String::from)),
                    )
                    .collect()
            }
            Err(_) => Default::default(),
        }
    };
    if !rare_kinds.is_empty() {
        g.mark_rare(&rare_kinds);
        println!(
            "fuzz: steering toward {} kind(s) the corpus never or barely shows — {}",
            rare_kinds.len(),
            {
                let mut v: Vec<&str> = rare_kinds.iter().map(String::as_str).collect();
                v.sort_unstable();
                v.join(", ")
            }
        );
    }
    let dirs = crate::routing::grammar_dirs(lang);
    let (language, _) = crate::grammar::load(&grammar_dir.join(dirs[0]))?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let oracle = treebank_oracle::get(lang);
    let tmp = std::env::temp_dir().join(format!("treebank-fuzz-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let judge = Judge {
        oracle,
        dir: &tmp,
        lang,
    };

    let syntax_mode = oracle.validate_syntax_only(&tmp, &[])?.is_some();
    println!(
        "fuzz: {iterations} programs derived from the {lang} grammar (seed {seed}), judged by the {}",
        if syntax_mode { "parser alone" } else { "parser and compiler together" }
    );

    let mut rng = Rng(seed | 1);
    let mut we_accepted = 0usize;
    let mut built_kinds: std::collections::BTreeMap<u16, u64> = Default::default();
    let mut built_tokens: std::collections::BTreeMap<u16, u64> = Default::default();
    let mut built_edges: std::collections::BTreeMap<(u16, Option<String>, u16), u64> =
        Default::default();
    let mut we_rejected = 0usize;
    let mut agreed = 0usize;
    let mut by_program: BTreeMap<String, usize> = BTreeMap::new();

    // AFL's shape, with the tape as the input and grammar alternatives as
    // the edge map. The tape is what makes this cheap: generation is
    // deterministic in it, so the tape IS the input, and shrinking already
    // proved it survives surgery.
    let mut seen: std::collections::HashSet<(u32, u32)> = Default::default();
    let mut corpus: Vec<Vec<u8>> = Vec::new();

    for i in 0..iterations {
        // Half the budget explores from scratch and half mutates a tape
        // already known to reach somewhere new. Pure mutation converges on
        // one shape; pure random never builds on anything.
        let tape = if unguided {
            rng.tape(64)
        } else if i % 3 == 0 {
            // Keep exploring: seeking alone would walk the same frontier.
            rng.tape(64)
        } else if i % 3 == 1 || corpus.is_empty() {
            seek_tape(&g, lang, &seen, rare && i % 6 == 1, &mut rng)
        } else {
            let pick = (rng.next() as usize) % corpus.len();
            mutate_tape(&corpus[pick], &mut rng)
        };
        let (text, cov) = derive(&g, lang, &tape);
        if cov.iter().any(|c| !seen.contains(c)) {
            for c in &cov {
                seen.insert(*c);
            }
            if corpus.len() < MAX_CORPUS {
                corpus.push(tape.clone());
            }
        }
        if text.trim().is_empty() {
            continue;
        }
        let Some(tree) = parser.parse(text.as_bytes(), None) else {
            continue;
        };
        if tree.root_node().has_error() {
            we_rejected += 1;
            continue;
        }
        we_accepted += 1;
        // What did this program actually BUILD? Alternative coverage says
        // which choices the generator took; this says which shapes the
        // parser ended up with — the same yardstick `kinds` puts on the
        // corpus, so the two are directly comparable.
        crate::kinds::count_kinds(tree.root_node(), &mut built_kinds);
        crate::kinds::count_all_kinds(tree.root_node(), &mut built_tokens);
        crate::kinds::count_edges(tree.root_node(), &mut built_edges);
        if judge.accepts(&text)? {
            agreed += 1;
            continue;
        }
        let small = shrink(&g, &mut parser, &judge, &tape)?;
        let program = generate(&g, lang, &small);
        *by_program.entry(program).or_insert(0) += 1;
    }

    let mut findings: Vec<Finding> = by_program
        .into_iter()
        .map(|(program, seeds)| Finding {
            declared: policy.declared_reason(&program).map(String::from),
            bytes: program.len(),
            program,
            seeds,
        })
        .collect();
    findings.sort_by_key(|f| (f.bytes, f.program.clone()));

    let syntax_only = oracle.validate_syntax_only(&tmp, &[])?.is_some();
    let total: usize = g.sites.iter().map(|(_, n)| *n).sum();
    let mut uncovered: BTreeMap<String, usize> = BTreeMap::new();
    for (site, (rule, arity)) in g.sites.iter().enumerate() {
        let missing = (0..*arity)
            .filter(|a| !seen.contains(&(site as u32, *a as u32)))
            .count();
        if missing > 0 {
            *uncovered.entry(rule.clone()).or_insert(0) += missing;
        }
    }
    let mut uncovered_by_rule: Vec<(String, usize)> = uncovered.into_iter().collect();
    uncovered_by_rule.sort_by_key(|(r, n)| (std::cmp::Reverse(*n), r.clone()));

    let report = FuzzReport {
        lang: lang.to_string(),
        grammar: grammar_dir.display().to_string(),
        judged_by: if syntax_only {
            "parser"
        } else {
            "parser+compiler"
        },
        iterations,
        we_accepted,
        built_kinds: built_kinds.len(),
        built_tokens: built_tokens.len(),
        built_edges: built_edges.len(),
        built_kind_names: built_kinds
            .keys()
            .filter_map(|k| language.node_kind_for_id(*k).map(str::to_string))
            .collect(),
        built_token_names: built_tokens
            .keys()
            .filter(|k| !language.node_kind_is_named(**k))
            .filter_map(|k| language.node_kind_for_id(*k).map(str::to_string))
            .collect(),
        built_edge_names: built_edges
            .keys()
            .map(|(pa, f, c)| {
                let n = |i: u16| language.node_kind_for_id(i).unwrap_or("?").to_string();
                match f {
                    Some(f) => format!("{} {}: {}", n(*pa), f, n(*c)),
                    None => format!("{} · {}", n(*pa), n(*c)),
                }
            })
            .collect(),
        we_rejected,
        agreed,
        widenings: findings.iter().map(|f| f.seeds).sum(),
        coverage: Coverage {
            alternatives_total: total,
            alternatives_covered: seen.len(),
            percent: if total == 0 {
                0.0
            } else {
                (seen.len() as f64 * 1000.0 / total as f64).round() / 10.0
            },
            uncovered_by_rule: uncovered_by_rule.iter().take(20).cloned().collect(),
        },
        corpus_size: corpus.len(),
        findings,
    };

    let undeclared: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.declared.is_none())
        .collect();
    let declared_count = report.findings.len() - undeclared.len();
    println!(
        "fuzz: {} accepted by us ({} discarded as unfaithful), {} agreed, {} widening(s) in {} distinct program(s) — {} undeclared, {} declared",
        report.we_accepted,
        report.we_rejected,
        report.agreed,
        report.widenings,
        report.findings.len(),
        undeclared.len(),
        declared_count,
    );
    if !policy.rule.is_empty() && declared_count > 0 {
        println!("  ({} declared by fuzz_policy.toml)", declared_count);
    }
    println!(
        "fuzz: coverage — {}/{} grammar alternatives ({}%), {} tape(s) kept",
        report.coverage.alternatives_covered,
        report.coverage.alternatives_total,
        report.coverage.percent,
        report.corpus_size,
    );
    for (rule, n) in report.coverage.uncovered_by_rule.iter().take(8) {
        println!("  unreached {n:>4}  in {rule}");
    }
    for f in undeclared.iter().take(20) {
        println!("  {:>3}x  {}", f.seeds, f.program.replace('\n', " ⏎ "));
    }

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&report)?)?;
    println!("fuzz: report at {}", out_path.display());
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

pub fn default_out(lang: LangName) -> PathBuf {
    PathBuf::from(format!("corpus/{lang}/reports/fuzz.json"))
}
