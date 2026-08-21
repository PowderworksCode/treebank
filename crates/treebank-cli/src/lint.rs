//! `treebank lint` — the grammar-smell detector (FIELD_GUIDE.md §9).
//!
//! Every check here is a mechanical form of a defect that shipped, or
//! nearly shipped, in a treebank grammar. A grammar can pass every
//! behavioural gate — sweep, negative corpus, roles, rosetta — while
//! accumulating exactly these debts, because each one fails on the file
//! that arrives NEXT month: the conflict that finally overlaps a
//! precedence, the unreserved keyword a stray `end` finally exercises,
//! the fork pressure a dense-enough file finally pushes past the GLR
//! version cap.
//!
//! Findings are judged against `lint_policy.toml` in the grammar dir —
//! per-check baselines with a reason, the same ratchet discipline
//! `shape_policy.toml` uses. Without a policy the run is advisory: the
//! report prints and the exit is clean, so a grammar can adopt the gate
//! by writing down its current debts rather than by a flag day. The one
//! exception is scanner/externals drift, which is undefined behaviour
//! and fails with or without a policy.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Deserialize, Default)]
struct LintPolicy {
    #[serde(default)]
    baselines: Baselines,
}

/// Accepted debt per check. Growth past a baseline fails; shrinking
/// prints a reminder to tighten the ratchet, exactly like shape's
/// `baseline_missed`.
#[derive(Deserialize, Default)]
struct Baselines {
    declared_conflicts: Option<usize>,
    early_commit_conflicts: Option<usize>,
    dynamic_weights: Option<usize>,
    same_text_tokens: Option<usize>,
    unreserved_keywords: Option<usize>,
    state_count: Option<usize>,
}

struct Report {
    findings: Vec<String>,
    over: Vec<String>,
    under: Vec<String>,
    hard_failures: Vec<String>,
}

/// Lint is per PARSE TABLE, not per language. Every input it reads is
/// generated for one table — `src/grammar.json`, `src/scanner.c`,
/// `src/parser.c` — and so are the `lint_policy.toml` ratchets: python2's
/// one declared conflict is the measured price of PEP 3105 in that table
/// and is not debt python3 may spend. A multi-variant language
/// (VARIANTS.md §2) therefore gets one run per variant.
///
/// Every table is linted even after one fails. Stopping at the first would
/// print nothing about the second, which reads as clean.
pub fn run(grammar_dir: &Path) -> Result<()> {
    let tables = crate::verify::variant_dirs(grammar_dir);
    let multi = tables.len() > 1;
    let mut failed = Vec::new();
    for dir in &tables {
        let label = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if multi {
            println!("lint: [{label}]");
        }
        if let Err(e) = run_table(dir) {
            failed.push(if multi {
                format!("{label}: {e}")
            } else {
                e.to_string()
            });
        }
    }
    if !failed.is_empty() {
        bail!("{}", failed.join("; "));
    }
    Ok(())
}

fn run_table(grammar_dir: &Path) -> Result<()> {
    let grammar: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(grammar_dir.join("src/grammar.json"))
            .context("read src/grammar.json — generate first")?,
    )?;
    let policy_path = grammar_dir.join("lint_policy.toml");
    let policy: Option<LintPolicy> = match std::fs::read_to_string(&policy_path) {
        Ok(text) => Some(toml::from_str(&text).context("parse lint_policy.toml")?),
        Err(_) => None,
    };
    let b = policy.as_ref().map(|p| &p.baselines);

    let mut r = Report {
        findings: Vec::new(),
        over: Vec::new(),
        under: Vec::new(),
        hard_failures: Vec::new(),
    };

    check_scanner_enum(grammar_dir, &grammar, &mut r);
    let conflicts = check_conflicts(&grammar, &mut r);
    let early = check_early_commit(&grammar, &mut r);
    let weights = count_dynamic(&grammar);
    let same_text = check_same_text_tokens(&grammar, &mut r);
    let unreserved = check_unreserved_keywords(&grammar, &mut r);
    let states = state_count(grammar_dir)?;

    r.findings.push(format!(
        "counts: {conflicts} declared conflict(s) ({early} touching supertypes), \
         {weights} dynamic weight(s), {same_text} same-text token split(s), \
         {unreserved} unreserved keyword-shaped token(s), {states} parse states"
    ));

    if let Some(b) = b {
        ratchet(&mut r, "declared_conflicts", conflicts, b.declared_conflicts);
        ratchet(&mut r, "early_commit_conflicts", early, b.early_commit_conflicts);
        ratchet(&mut r, "dynamic_weights", weights, b.dynamic_weights);
        ratchet(&mut r, "same_text_tokens", same_text, b.same_text_tokens);
        ratchet(&mut r, "unreserved_keywords", unreserved, b.unreserved_keywords);
        ratchet(&mut r, "state_count", states, b.state_count);
    }

    for f in &r.findings {
        println!("lint: {f}");
    }
    for u in &r.under {
        println!("lint: {u}");
    }
    if !r.hard_failures.is_empty() || !r.over.is_empty() {
        for f in r.hard_failures.iter().chain(r.over.iter()) {
            eprintln!("lint: FAIL {f}");
        }
        bail!(
            "{} lint failure(s)",
            r.hard_failures.len() + r.over.len()
        );
    }
    if policy.is_none() {
        println!(
            "lint advisory: no {} — findings reported, nothing enforced; \
             write the policy to turn this into a gate",
            policy_path.display()
        );
    } else {
        println!("lint OK: within every baseline");
    }
    Ok(())
}

fn ratchet(r: &mut Report, name: &str, actual: usize, baseline: Option<usize>) {
    match baseline {
        None => {}
        Some(base) if actual > base => r.over.push(format!(
            "{name}: {actual} exceeds the baseline of {base} — either the \
             regression is real, or the new debt needs its reason written \
             into lint_policy.toml"
        )),
        Some(base) if actual < base => r.under.push(format!(
            "{name}: {actual} is under the baseline of {base} — tighten the \
             ratchet so the improvement cannot silently unwind"
        )),
        _ => {}
    }
}

/// The scanner body, following the one indirection a variant introduces.
///
/// A multi-variant language's `src/scanner.c` is a stub around the shared
/// `common/scanner.c` (VARIANTS.md §2), so reading the stub alone finds no
/// enum at all and this check reports drift it cannot actually see — on
/// every variant, forever. Same blind spot as the dylib cache that once
/// fingerprinted only the stub and served a stale parser to the
/// crossvariant gate.
///
/// The includes are consulted only when the file has no enum of its own, so
/// every single-file scanner is read exactly as before, and `tree_sitter/`
/// is skipped: the runtime's headers are what every scanner includes and
/// where no scanner declares its TokenType. Both conditions exist so that a
/// missing enum still FAILS rather than matching against something that was
/// never the scanner's.
fn scanner_source(path: &Path) -> std::io::Result<String> {
    let raw = std::fs::read(path)?;
    let mut text = String::from_utf8_lossy(&raw).into_owned();
    if text.contains("enum ") {
        return Ok(text);
    }
    for inc in crate::grammar::local_includes(path, &raw) {
        if inc.components().any(|c| c.as_os_str() == "tree_sitter") {
            continue;
        }
        text.push('\n');
        text.push_str(&String::from_utf8_lossy(&std::fs::read(inc)?));
    }
    Ok(text)
}

/// Externals and the scanner's TokenType enum are matched BY POSITION;
/// drift between them is undefined behaviour that presents as impossible
/// tokens, so this one is a hard failure with or without a policy. (The
/// enum census is textual and deliberately dumb: the first `enum { … }`
/// block in scanner.c, one identifier per comma.)
fn check_scanner_enum(dir: &Path, grammar: &serde_json::Value, r: &mut Report) {
    let externals = grammar["externals"].as_array().map(|a| a.len()).unwrap_or(0);
    if externals == 0 {
        return;
    }
    let Ok(scanner) = scanner_source(&dir.join("src/scanner.c")) else {
        r.hard_failures.push(format!(
            "grammar declares {externals} externals but src/scanner.c is unreadable"
        ));
        return;
    };
    let Some(open) = scanner.find("enum ").and_then(|i| scanner[i..].find('{').map(|j| i + j + 1))
    else {
        r.hard_failures
            .push("externals declared but no enum found in scanner.c".into());
        return;
    };
    let Some(close) = scanner[open..].find('}') else {
        r.hard_failures.push("unterminated enum in scanner.c".into());
        return;
    };
    let names: Vec<&str> = scanner[open..open + close]
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("//"))
        .collect();
    if names.len() != externals {
        r.hard_failures.push(format!(
            "externals/scanner drift: grammar.js declares {externals} externals, \
             scanner.c's enum has {} — they are matched by position, and a skew \
             makes the scanner emit the wrong token kinds",
            names.len()
        ));
    }
}

fn check_conflicts(grammar: &serde_json::Value, r: &mut Report) -> usize {
    let n = grammar["conflicts"].as_array().map(|a| a.len()).unwrap_or(0);
    if n > 0 {
        r.findings.push(format!(
            "{n} declared conflict(s): each is a GLR fork site, and past the \
             runtime's six-version cap ties are culled arbitrarily \
             (FIELD_GUIDE.md §2) — prefer factoring them away (§1)"
        ));
    }
    n
}

/// A conflict naming a supertype or shared hidden tier is the
/// early-commit smell: two parallel derivations for the same token were
/// built where one shared one would parse deterministically. The
/// `[_name, _callee]` family forked at every identifier until the callee
/// was re-spelled through the shared tiers.
fn check_early_commit(grammar: &serde_json::Value, r: &mut Report) -> usize {
    let supers: BTreeSet<&str> = grammar["supertypes"]
        .as_array()
        .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
        .unwrap_or_default();
    let mut n = 0;
    if let Some(conflicts) = grammar["conflicts"].as_array() {
        for c in conflicts {
            let members: Vec<&str> = c
                .as_array()
                .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
                .unwrap_or_default();
            if members.iter().any(|m| supers.contains(m)) {
                n += 1;
                r.findings.push(format!(
                    "conflict {members:?} includes a supertype — an early commit \
                     between parallel tiers; can the members share a derivation?"
                ));
            }
        }
    }
    n
}

fn count_dynamic(grammar: &serde_json::Value) -> usize {
    fn walk(v: &serde_json::Value, n: &mut usize) {
        match v {
            serde_json::Value::Object(o) => {
                if o.get("type").and_then(|t| t.as_str()) == Some("PREC_DYNAMIC") {
                    *n += 1;
                }
                o.values().for_each(|v| walk(v, n));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|v| walk(v, n)),
            _ => {}
        }
    }
    let mut n = 0;
    walk(&grammar["rules"], &mut n);
    n
}

/// One spelling, two token definitions (a plain string and an
/// IMMEDIATE_TOKEN of the same text) — the lexer produces exactly one of
/// them per position, so a GLR fork needing the other starves silently.
/// `def f(a)` failed on every file this way, before the parameter list
/// took the same immediate `(` the call argument list uses.
fn check_same_text_tokens(grammar: &serde_json::Value, r: &mut Report) -> usize {
    let mut plain = BTreeSet::new();
    let mut immediate = BTreeSet::new();
    fn walk<'v>(
        v: &'v serde_json::Value,
        in_immediate: bool,
        plain: &mut BTreeSet<&'v str>,
        immediate: &mut BTreeSet<&'v str>,
    ) {
        match v {
            serde_json::Value::Object(o) => {
                let ty = o.get("type").and_then(|t| t.as_str());
                if ty == Some("STRING") {
                    if let Some(s) = o.get("value").and_then(|s| s.as_str()) {
                        if in_immediate {
                            immediate.insert(s);
                        } else {
                            plain.insert(s);
                        }
                    }
                }
                let now_immediate = in_immediate || ty == Some("IMMEDIATE_TOKEN");
                o.values()
                    .for_each(|v| walk(v, now_immediate, plain, immediate));
            }
            serde_json::Value::Array(a) => a
                .iter()
                .for_each(|v| walk(v, in_immediate, plain, immediate)),
            _ => {}
        }
    }
    walk(&grammar["rules"], false, &mut plain, &mut immediate);
    let both: Vec<&&str> = plain.intersection(&immediate).collect();
    for t in &both {
        r.findings.push(format!(
            "`{t}` is both a plain token and a token.immediate — two tokens, one \
             spelling; a fork that needs the one the lexer did not pick starves \
             at the lexer (FIELD_GUIDE.md §4)"
        ));
    }
    both.len()
}

/// Keyword-shaped string tokens not in any reserved set: where the
/// keyword is invalid, the word token lexes the same text as a name, so
/// a stray `end` becomes a variable read (FIELD_GUIDE.md §5). Only
/// meaningful for grammars that declare `word`; intentional soft
/// keywords go in the policy baseline with their reason.
fn check_unreserved_keywords(grammar: &serde_json::Value, r: &mut Report) -> usize {
    if grammar["word"].as_str().is_none() {
        return 0;
    }
    let mut reserved = BTreeSet::new();
    if let Some(sets) = grammar["reserved"].as_object() {
        for set in sets.values() {
            if let Some(arr) = set.as_array() {
                for t in arr {
                    if let Some(s) = t["value"].as_str() {
                        reserved.insert(s.to_string());
                    }
                }
            }
        }
    }
    let mut keywords = BTreeSet::new();
    fn walk(v: &serde_json::Value, out: &mut BTreeSet<String>) {
        match v {
            serde_json::Value::Object(o) => {
                if o.get("type").and_then(|t| t.as_str()) == Some("STRING") {
                    if let Some(s) = o.get("value").and_then(|s| s.as_str()) {
                        let word_shaped = s.len() > 1
                            && s.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
                            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                        if word_shaped {
                            out.insert(s.to_string());
                        }
                    }
                }
                o.values().for_each(|v| walk(v, out));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    walk(&grammar["rules"], &mut keywords);
    let missing: Vec<&String> = keywords.difference(&reserved).collect();
    if !missing.is_empty() {
        r.findings.push(format!(
            "{} keyword-shaped token(s) outside every reserved set: {} — where \
             the keyword is invalid these lex as plain names (a stray `end` \
             parsed as a variable until ruby reserved its keywords); soft \
             keywords are fine, with their count in the policy baseline",
            missing.len(),
            missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    missing.len()
}

fn state_count(dir: &Path) -> Result<usize> {
    let parser = std::fs::read_to_string(dir.join("src/parser.c"))
        .context("read src/parser.c — generate first")?;
    let needle = "#define STATE_COUNT ";
    let Some(i) = parser.find(needle) else {
        bail!("no STATE_COUNT in parser.c");
    };
    let rest = &parser[i + needle.len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    Ok(digits.parse()?)
}
