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
    Choice(Vec<Rule>),
    Repeat(Box<Rule>),
    Repeat1(Box<Rule>),
    /// PREC / FIELD / ALIAS / TOKEN / … — no bearing on the text emitted.
    Wrap(Box<Rule>),
}

fn parse_rule(v: &Value) -> Result<Rule> {
    let t = v["type"].as_str().context("grammar node without a type")?;
    let members = || -> Result<Vec<Rule>> {
        v["members"]
            .as_array()
            .context("node without members")?
            .iter()
            .map(parse_rule)
            .collect()
    };
    let content = || -> Result<Rule> { parse_rule(&v["content"]) };
    Ok(match t {
        "BLANK" => Rule::Blank,
        "STRING" => Rule::Str(v["value"].as_str().unwrap_or_default().to_string()),
        "PATTERN" => Rule::Pattern(v["value"].as_str().unwrap_or_default().to_string()),
        "SYMBOL" => Rule::Symbol(v["name"].as_str().unwrap_or_default().to_string()),
        "SEQ" => Rule::Seq(members()?),
        "CHOICE" => Rule::Choice(members()?),
        "REPEAT" => Rule::Repeat(Box::new(content()?)),
        "REPEAT1" => Rule::Repeat1(Box::new(content()?)),
        "PREC" | "PREC_LEFT" | "PREC_RIGHT" | "PREC_DYNAMIC" | "FIELD" | "ALIAS" | "TOKEN"
        | "IMMEDIATE_TOKEN" | "RESERVED" => Rule::Wrap(Box::new(content()?)),
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
    start: String,
    /// Shortest derivation length per rule, so a bounded generator can
    /// always pick an alternative that terminates instead of recursing.
    min_len: HashMap<String, usize>,
}

const UNBOUNDED: usize = 1 << 20;

impl Grammar {
    fn load(path: &Path) -> Result<Grammar> {
        let text = std::fs::read_to_string(path)?;
        let v: Value = serde_json::from_str(&text)
            .with_context(|| format!("parse {}", path.display()))?;
        let obj = v["rules"].as_object().context("grammar.json has no rules")?;
        // The start symbol is the FIRST rule as authored, and serde_json's
        // map is sorted, so it cannot be recovered from the parsed value --
        // asking it for the first key yields whatever sorts first, which for
        // rust is `_access`. Read the order back off the raw text.
        let start = first_rule_name(&text)
            .filter(|n| obj.contains_key(n))
            .context("could not find the start rule in grammar.json")?;
        let rules = obj
            .iter()
            .map(|(k, r)| Ok((k.clone(), parse_rule(r)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut g = Grammar { rules, start, min_len: HashMap::new() };
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

    fn rule_min(&self, r: &Rule) -> usize {
        match r {
            Rule::Blank => 0,
            Rule::Str(s) => s.len().max(1),
            Rule::Pattern(_) => 1,
            Rule::Symbol(n) => self.min_len.get(n).copied().unwrap_or(1),
            Rule::Seq(ms) => ms.iter().map(|m| self.rule_min(m)).fold(0, usize::saturating_add),
            Rule::Choice(ms) => ms.iter().map(|m| self.rule_min(m)).min().unwrap_or(0),
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
}

impl<'a> Tape<'a> {
    fn choose(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        let b = self.bytes.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b as usize % n
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

struct Gen<'a, 'b> {
    g: &'a Grammar,
    tape: &'b mut Tape<'a>,
    out: Vec<String>,
    steps: usize,
}

const MAX_STEPS: usize = 4000;
const MAX_DEPTH: usize = 18;

impl Gen<'_, '_> {
    fn emit(&mut self, r: &Rule, depth: usize) {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return;
        }
        match r {
            Rule::Blank => {}
            Rule::Str(s) => self.out.push(s.clone()),
            Rule::Pattern(p) => self.out.push(sample_pattern(p)),
            Rule::Symbol(n) => match self.g.rules.get(n) {
                // An external token has no rule and therefore no text. Its
                // name is the only thing we know, so stand something in and
                // let our own parser judge the result.
                None => self.out.push("a".into()),
                Some(rule) => {
                    if depth >= MAX_DEPTH {
                        self.out.push("a".into());
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
            Rule::Choice(ms) => {
                let pick = if depth >= MAX_DEPTH {
                    // Past the bound, take the shortest alternative. With
                    // min_len as the guide this always terminates.
                    ms.iter()
                        .enumerate()
                        .min_by_key(|(_, m)| self.g.rule_min(m))
                        .map(|(i, _)| i)
                        .unwrap_or(0)
                } else {
                    self.tape.choose(ms.len())
                };
                let chosen = ms[pick].clone();
                self.emit(&chosen, depth);
            }
            Rule::Repeat(c) => {
                let n = if depth >= MAX_DEPTH { 0 } else { self.tape.choose(3) };
                for _ in 0..n {
                    self.emit(c, depth + 1);
                }
            }
            Rule::Repeat1(c) => {
                let n = if depth >= MAX_DEPTH { 1 } else { 1 + self.tape.choose(2) };
                for _ in 0..n {
                    self.emit(c, depth + 1);
                }
            }
            Rule::Wrap(c) => self.emit(c, depth),
        }
    }
}

fn generate(g: &Grammar, tape_bytes: &[u8]) -> String {
    let mut tape = Tape { bytes: tape_bytes, pos: 0 };
    let start = g.rules[&g.start].clone();
    let mut gen = Gen { g, tape: &mut tape, out: Vec::new(), steps: 0 };
    gen.emit(&start, 0);
    gen.out.join(" ")
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
    /// The shrunk program: the smallest text still reproducing.
    pub program: String,
    pub bytes: usize,
    /// How many independent seeds reduced to this same example.
    pub seeds: usize,
}

#[derive(Serialize)]
pub struct FuzzReport {
    pub lang: String,
    pub grammar: String,
    pub iterations: usize,
    /// Generated programs our parser accepted — the only ones worth asking
    /// the oracle about.
    pub we_accepted: usize,
    /// Generated programs our parser rejected. These are generator
    /// infidelity (token spacing, sampled patterns), not grammar defects,
    /// and are discarded rather than reported.
    pub we_rejected: usize,
    /// Accepted by us AND by the oracle: agreement, the expected case.
    pub agreed: usize,
    pub widenings: usize,
    pub findings: Vec<Finding>,
}

struct Judge<'a> {
    oracle: &'static dyn treebank_oracle::Oracle,
    dir: &'a Path,
    lang: LangName,
}

impl Judge<'_> {
    /// Does the language accept this text? One file at a time: shrinking is
    /// inherently sequential, and for rust the oracle is in-process anyway.
    fn accepts(&self, text: &str) -> Result<bool> {
        let name = format!("probe.{}", crate::fuzz::ext(self.lang));
        std::fs::write(self.dir.join(&name), text)?;
        let verdicts = self.oracle.validate(self.dir, &[name.clone()])?;
        Ok(verdicts.get(&name).copied().unwrap_or(false))
    }
}

fn ext(lang: LangName) -> &'static str {
    match lang {
        LangName::Rust => "rs",
        LangName::Python => "py",
        LangName::Typescript => "ts",
        LangName::Javascript => "js",
    }
}

/// Is this a widening? We accept it and the language does not.
fn is_widening(
    g: &Grammar,
    parser: &mut Parser,
    judge: &Judge,
    tape: &[u8],
) -> Result<Option<String>> {
    let text = generate(g, tape);
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

const SHRINK_STEPS: usize = 300;

/// Search for a shorter, smaller tape that still reproduces. Two passes,
/// in the order that pays: delete runs of choices first, because that is
/// what collapses a program to its core, then lower individual bytes toward
/// zero, which walks each surviving choice down to the grammar's first
/// alternative.
fn shrink(g: &Grammar, parser: &mut Parser, judge: &Judge, tape: &[u8]) -> Result<Vec<u8>> {
    let mut best = tape.to_vec();
    let mut budget = SHRINK_STEPS;

    let mut width = best.len().max(1);
    while width > 0 && budget > 0 {
        let mut i = 0;
        while i < best.len() && budget > 0 {
            let mut candidate = best.clone();
            let end = (i + width).min(candidate.len());
            candidate.drain(i..end);
            budget -= 1;
            if is_widening(g, parser, judge, &candidate)?.is_some() {
                best = candidate;
            } else {
                i += width;
            }
        }
        width /= 2;
    }

    for i in 0..best.len() {
        if budget == 0 {
            break;
        }
        let mut lo = 0u8;
        while lo < best[i] && budget > 0 {
            let mid = lo + (best[i] - lo) / 2;
            let mut candidate = best.clone();
            candidate[i] = mid;
            budget -= 1;
            if is_widening(g, parser, judge, &candidate)?.is_some() {
                best = candidate;
            } else {
                lo = mid + 1;
            }
        }
    }
    Ok(best)
}

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    iterations: usize,
    seed: u64,
    out_path: &Path,
) -> Result<()> {
    let g = Grammar::load(&grammar_dir.join("src/grammar.json"))?;
    let dirs = crate::routing::grammar_dirs(lang);
    let (language, _) = crate::grammar::load(&grammar_dir.join(dirs[0]))?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let oracle = treebank_oracle::get(lang);
    let tmp = std::env::temp_dir().join(format!("treebank-fuzz-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let judge = Judge { oracle, dir: &tmp, lang };

    println!("fuzz: {iterations} programs derived from the {lang} grammar (seed {seed})");

    let mut rng = Rng(seed | 1);
    let mut we_accepted = 0usize;
    let mut we_rejected = 0usize;
    let mut agreed = 0usize;
    let mut by_program: BTreeMap<String, usize> = BTreeMap::new();

    for _ in 0..iterations {
        let tape = rng.tape(64);
        let text = generate(&g, &tape);
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
        if judge.accepts(&text)? {
            agreed += 1;
            continue;
        }
        let small = shrink(&g, &mut parser, &judge, &tape)?;
        let program = generate(&g, &small);
        *by_program.entry(program).or_insert(0) += 1;
    }

    let mut findings: Vec<Finding> = by_program
        .into_iter()
        .map(|(program, seeds)| Finding { bytes: program.len(), program, seeds })
        .collect();
    findings.sort_by_key(|f| (f.bytes, f.program.clone()));

    let report = FuzzReport {
        lang: lang.to_string(),
        grammar: grammar_dir.display().to_string(),
        iterations,
        we_accepted,
        we_rejected,
        agreed,
        widenings: findings.iter().map(|f| f.seeds).sum(),
        findings,
    };

    println!(
        "fuzz: {} accepted by us ({} discarded as unfaithful), {} agreed, {} widening(s) in {} distinct program(s)",
        report.we_accepted,
        report.we_rejected,
        report.agreed,
        report.widenings,
        report.findings.len()
    );
    for f in report.findings.iter().take(20) {
        println!("  {:>3}x  {}", f.seeds, f.program);
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
