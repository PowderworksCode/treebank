//! What the corpus never shows us (`treebank kinds`).
//!
//! The sweep is only as good as the code it reads. A construct no corpus
//! file contains is a construct the oracle has never been asked about, so a
//! bug there is invisible to every check that starts from real source — the
//! sweep, `mutate`, `roundtrip` and `reformat` alike. Fuzzing is the only
//! thing that can reach it.
//!
//! That makes "which node kinds does real code never produce" the question
//! worth asking, and it is cheap to answer: parse the corpus with our own
//! grammar and count. No oracle is involved, because this measures the
//! CORPUS rather than the grammar.
//!
//! The output is a budget, not a score. Kinds with millions of occurrences
//! are thoroughly checked already and fuzzing them again buys nothing;
//! kinds with none are where the marginal value of a generated program is.
//!
//! **The budget can be spent, and then it has to be written down.** A blind
//! spot is either worth closing or worth accepting, and the third state --
//! measured once, argued about in an issue, forgotten -- is the one that
//! rots. So a grammar may carry a `kinds_policy.toml` naming every kind and
//! token its corpus never produces, with what covers each one instead, and
//! `--check` holds the declaration to the measurement in BOTH directions:
//! an undeclared blind spot fails, and so does a declared one the corpus
//! has since started producing. The second half is the one that makes it a
//! policy rather than an ignore list -- it means growing the corpus deletes
//! entries here, and nobody has to notice by hand.
//!
//! Sections are per LANGUAGE, not per grammar dir, because one grammar can
//! answer for several corpora whose blind spots have nothing in common:
//! `treebank-typescript` sees 7 kinds never built by .ts files and 48 never
//! built by .js files, and the second list is just "TypeScript", which is
//! not a decision anybody needs to record. Without a section for the
//! language the run is advisory, the same way `lint` is without a policy.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};

use treebank_corpus::fetch::Manifest;
use treebank_lang::LangName;

use crate::grammar;

/// `(parent kind, field, child kind)` for every named child. Node kinds
/// answer "does the corpus ever build this"; edges answer the finer
/// question of whether it ever builds it THERE, which is where a grammar
/// that models a node but attaches it in only one of two legal places
/// would hide.
pub fn count_edges(root: Node, out: &mut BTreeMap<(u16, Option<String>, u16), u64>) {
    let mut cursor = root.walk();
    let mut descend = true;
    loop {
        if descend {
            let n = cursor.node();
            if n.is_named() && n.child_count() > 0 {
                let mut c = n.walk();
                if c.goto_first_child() {
                    loop {
                        let child = c.node();
                        // An anonymous child only counts when it sits in a
                        // field: node-types declares `operator: "%"` but says
                        // nothing about the loose punctuation of a rule.
                        let field = c.field_name().map(str::to_string);
                        if child.is_named() || field.is_some() {
                            let key = (n.kind_id(), field, child.kind_id());
                            *out.entry(key).or_insert(0) += 1;
                        }
                        if !c.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
        }
        if descend && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            descend = true;
            continue;
        }
        loop {
            if !cursor.goto_parent() {
                return;
            }
            if cursor.goto_next_sibling() {
                descend = true;
                break;
            }
        }
    }
}

/// Every kind, named or not. The anonymous ones are the TOKENS — every
/// keyword and operator the grammar spells out — and nothing here has ever
/// measured whether the corpus exercises them.
pub fn count_all_kinds(root: Node, out: &mut BTreeMap<u16, u64>) {
    let mut cursor = root.walk();
    let mut descend = true;
    loop {
        if descend {
            *out.entry(cursor.node().kind_id()).or_insert(0) += 1;
        }
        if descend && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            descend = true;
            continue;
        }
        loop {
            if !cursor.goto_parent() {
                return;
            }
            if cursor.goto_next_sibling() {
                descend = true;
                break;
            }
        }
    }
}

pub fn count_kinds(root: Node, out: &mut BTreeMap<u16, u64>) {
    let mut cursor = root.walk();
    let mut descend = true;
    loop {
        if descend {
            let n = cursor.node();
            if n.is_named() {
                *out.entry(n.kind_id()).or_insert(0) += 1;
            }
        }
        if descend && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            descend = true;
            continue;
        }
        loop {
            if !cursor.goto_parent() {
                return;
            }
            if cursor.goto_next_sibling() {
                descend = true;
                break;
            }
        }
    }
}

#[derive(Serialize)]
pub struct KindsReport {
    pub lang: String,
    pub files_parsed: usize,
    pub named_kinds_total: usize,
    pub named_kinds_seen: usize,
    /// Node kinds the grammar can produce that the corpus never did. These
    /// are the blind spot: no oracle has ever been asked about them.
    pub never_seen: Vec<String>,
    /// Kinds seen fewer than `THIN` times — checked, but barely.
    pub thin: Vec<(String, u64)>,
    /// The whole table, for anything that wants to weight by it.
    pub counts: BTreeMap<String, u64>,
    /// Anonymous kinds — the grammar's keywords and operators.
    pub tokens_total: usize,
    pub tokens_seen: usize,
    pub tokens_never: Vec<String>,
    /// `(parent, field, child)` triples the corpus builds, against those
    /// `node-types.json` says are possible.
    pub edges_possible: usize,
    pub edges_seen: usize,
    /// Possible edges the corpus never builds, worst-covered parent first.
    pub edges_never_by_parent: Vec<(String, usize)>,
    pub edges_never: Vec<String>,
}

/// Below this a kind is "thin": present, but on so few files that one
/// unusual spelling of it could still be unchecked.
const THIN: u64 = 20;

/// A grammar's `kinds_policy.toml`: per language, what its corpus is
/// allowed never to build.
#[derive(Deserialize, Default)]
struct KindsPolicy {
    #[serde(flatten)]
    languages: BTreeMap<String, LanguagePolicy>,
}

#[derive(Deserialize, Default)]
struct LanguagePolicy {
    /// Why this corpus cannot reach the list below. Prose, deliberately:
    /// the entries say WHAT and this says why growing the corpus is not
    /// the answer, which is the part a reader needs to re-examine.
    #[serde(default)]
    rule: String,
    #[serde(default)]
    accepted: Vec<Accepted>,
}

/// One kind or token the corpus never produces, and what checks it
/// instead. Every field is evidence somebody measured; a coverage claim
/// nobody can reproduce is the thing this file exists to prevent.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Accepted {
    /// The node kind or anonymous token, spelled as `kinds` reports it.
    item: String,
    /// A minimal program containing it. The point is that a reader can
    /// paste it into the oracle and get `oracle` back; a claim about
    /// coverage that cannot be re-run is the thing this file replaces.
    #[serde(default)]
    spelling: String,
    /// What covers it instead. Free text, but it should name a corpus or
    /// a seed count, not a feeling.
    covered_by: String,
    /// What the reference parser said about `spelling`, when it was asked.
    #[serde(default)]
    oracle: String,
    #[serde(default)]
    why: String,
}

fn load_policy(grammar_dir: &Path, lang: LangName) -> Result<Option<LanguagePolicy>> {
    let path = grammar_dir.join("kinds_policy.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let policy: KindsPolicy = toml::from_str(&text).context("parse kinds_policy.toml")?;
    Ok(policy.languages.into_iter().find_map(|(name, section)| {
        (name == lang.as_str()).then_some(section)
    }))
}

/// Hold the declaration to the measurement, both ways.
///
/// Undeclared-but-blind fails because that is the case the file exists to
/// catch. Declared-but-covered fails too, and that direction matters more
/// than it looks: it is what makes a corpus that grows DELETE entries here
/// instead of quietly carrying a claim that stopped being true. The same
/// argument `shape_policy.toml` makes for printing a reminder when its
/// baseline drops, one step firmer, because these are names rather than a
/// count and a stale name is a specific false statement.
fn check_policy(report: &KindsReport, policy: &LanguagePolicy) -> Result<()> {
    let measured: BTreeSet<&str> = report
        .never_seen
        .iter()
        .chain(report.tokens_never.iter())
        .map(String::as_str)
        .collect();
    let declared: BTreeSet<&str> = policy.accepted.iter().map(|a| a.item.as_str()).collect();

    let undeclared: Vec<&&str> = measured.difference(&declared).collect();
    let stale: Vec<&&str> = declared.difference(&measured).collect();

    for item in &undeclared {
        println!("  UNDECLARED  {item} — no corpus file builds it and kinds_policy.toml does not say why");
    }
    for item in &stale {
        println!("  STALE       {item} — the corpus builds it now; delete its kinds_policy.toml entry");
    }
    if undeclared.is_empty() && stale.is_empty() {
        println!(
            "kinds: policy ok — {} accepted blind spot(s), all still blind, none undeclared",
            declared.len()
        );
        return Ok(());
    }
    bail!(
        "kinds: {} undeclared blind spot(s), {} stale declaration(s) in kinds_policy.toml",
        undeclared.len(),
        stale.len()
    );
}

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    manifest_path: &Path,
    limit: Option<usize>,
    out_path: &Path,
    check: bool,
) -> Result<()> {
    // Load before the sweep, not after: a policy that does not parse should
    // say so in a second rather than after every corpus file has been read.
    let policy = load_policy(grammar_dir, lang)?;
    anyhow::ensure!(
        !(check && limit.is_some()),
        "--check needs the whole corpus: a sampled run's never-seen set is \
         an artefact of the sample, and gating on it would declare blind \
         spots that are not blind"
    );
    let manifest = Manifest::load(manifest_path)?;
    let corpus_src = manifest_path.parent().unwrap_or(Path::new(".")).join("src");
    let mut entries = manifest.files();
    entries.sort_by(|a, b| (&a.pkgdir, &a.rel).cmp(&(&b.pkgdir, &b.rel)));
    if let Some(n) = limit {
        entries.truncate(n);
    }

    let (language, _) = grammar::load(grammar_dir)?;

    println!(
        "kinds: {} files — what does real {lang} never contain?",
        entries.len()
    );

    type Tally = (
        usize,
        BTreeMap<u16, u64>,
        BTreeMap<u16, u64>,
        BTreeMap<(u16, Option<String>, u16), u64>,
    );
    let per_file: Vec<Tally> = entries
        .par_iter()
        .map(|f| -> Result<Tally> {
            let rel = format!("{}/{}", f.pkgdir, f.rel);
            let Ok(src) = std::fs::read(corpus_src.join(&rel)) else {
                return Ok((0, BTreeMap::new(), BTreeMap::new(), BTreeMap::new()));
            };
            let mut parser = Parser::new();
            parser.set_language(&language)?;
            let Some(tree) = parser.parse(&src, None) else {
                return Ok((0, BTreeMap::new(), BTreeMap::new(), BTreeMap::new()));
            };
            // Only a CLEAN parse is evidence. tree-sitter salvages nodes
            // from an error tree, so counting those says we cover a
            // construct when what actually happened is that we failed on
            // the file and kept the wreckage. Invisible where the sweep
            // passes (python, java), decisive where it does not: bash
            // fails on 56% of its corpus, so most of its coverage was
            // being read off trees we got wrong.
            if tree.root_node().has_error() {
                return Ok((0, BTreeMap::new(), BTreeMap::new(), BTreeMap::new()));
            }
            let mut counts = BTreeMap::new();
            let mut all = BTreeMap::new();
            let mut edges = BTreeMap::new();
            count_kinds(tree.root_node(), &mut counts);
            count_all_kinds(tree.root_node(), &mut all);
            count_edges(tree.root_node(), &mut edges);
            Ok((1, counts, all, edges))
        })
        .collect::<Result<_>>()?;

    let mut files_parsed = 0;
    let mut totals: BTreeMap<u16, u64> = BTreeMap::new();
    let mut all_totals: BTreeMap<u16, u64> = BTreeMap::new();
    let mut edge_totals: BTreeMap<(u16, Option<String>, u16), u64> = BTreeMap::new();
    for (n, counts, all, edges) in per_file {
        files_parsed += n;
        for (k, v) in counts {
            *totals.entry(k).or_insert(0) += v;
        }
        for (k, v) in all {
            *all_totals.entry(k).or_insert(0) += v;
        }
        for (k, v) in edges {
            *edge_totals.entry(k).or_insert(0) += v;
        }
    }

    // Every named kind the grammar can produce, from the language itself
    // rather than from node-types.json, so the two cannot drift.
    let ts_lang = &language;
    let mut named: Vec<(u16, String)> = Vec::new();
    for id in 0..ts_lang.node_kind_count() as u16 {
        if ts_lang.node_kind_is_named(id) {
            if let Some(name) = ts_lang.node_kind_for_id(id) {
                if !name.starts_with('_') {
                    named.push((id, name.to_string()));
                }
            }
        }
    }

    // SUM across every id sharing a name. tree-sitter gives an aliased
    // node its own symbol, so one name can have several ids — and looking
    // up a single one reported `identifier` as absent from 61,801 rust
    // files, which is how this bug announced itself.
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for (id, name) in &named {
        *counts.entry(name.clone()).or_insert(0) += totals.get(id).copied().unwrap_or(0);
    }
    let never_seen: Vec<String> = counts
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(k, _)| k.clone())
        .collect();
    let mut thin: Vec<(String, u64)> = counts
        .iter()
        .filter(|(_, n)| **n > 0 && **n < THIN)
        .map(|(k, n)| (k.clone(), *n))
        .collect();
    thin.sort_by_key(|(k, n)| (*n, k.clone()));

    // Tokens: the anonymous kinds are a mix of the grammar's literal
    // keywords and operators with tree-sitter's own machinery — hidden
    // rules and `_repeat1` helpers are unnamed too, and counting those as
    // unused tokens is meaningless. The literals are exactly the STRING
    // values in grammar.json, so take the vocabulary from there.
    let gj: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        grammar_dir.join("src/grammar.json"),
    )?)?;
    let mut literals: std::collections::HashSet<String> = Default::default();
    fn walk_strings(v: &serde_json::Value, out: &mut std::collections::HashSet<String>) {
        match v {
            serde_json::Value::Object(m) => {
                if m.get("type").and_then(|t| t.as_str()) == Some("STRING") {
                    if let Some(s) = m.get("value").and_then(|s| s.as_str()) {
                        out.insert(s.to_string());
                    }
                }
                for (_, c) in m {
                    walk_strings(c, out);
                }
            }
            serde_json::Value::Array(a) => {
                for c in a {
                    walk_strings(c, out);
                }
            }
            _ => {}
        }
    }
    walk_strings(&gj["rules"], &mut literals);
    let mut tokens: Vec<(u16, String)> = Vec::new();
    for id in 0..ts_lang.node_kind_count() as u16 {
        if !ts_lang.node_kind_is_named(id) {
            if let Some(name) = ts_lang.node_kind_for_id(id) {
                if literals.contains(name) {
                    tokens.push((id, name.to_string()));
                }
            }
        }
    }
    let mut token_counts: BTreeMap<String, u64> = BTreeMap::new();
    for (id, name) in &tokens {
        *token_counts.entry(name.clone()).or_insert(0) += all_totals.get(id).copied().unwrap_or(0);
    }
    let tokens_never: Vec<String> = token_counts
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(k, _)| k.clone())
        .collect();

    // Edges: what `node-types.json` says is possible, against what was built.
    let nt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        grammar_dir.join("src/node-types.json"),
    )?)?;
    // A supertype never appears in a tree — it is a derivation, so the node
    // carries a concrete kind. Expand the declared child types through it or
    // every supertype-typed slot reads as an edge the corpus never builds.
    let mut subtypes: std::collections::HashMap<String, Vec<String>> = Default::default();
    for n in nt.as_array().cloned().unwrap_or_default() {
        let Some(name) = n["type"].as_str() else {
            continue;
        };
        if let Some(subs) = n["subtypes"].as_array() {
            subtypes.insert(
                name.to_string(),
                subs.iter()
                    .filter_map(|t| t["type"].as_str().map(str::to_string))
                    .collect(),
            );
        }
    }
    let expand = |t: &str| -> Vec<String> {
        let mut out = Vec::new();
        let mut queue = vec![t.to_string()];
        while let Some(x) = queue.pop() {
            match subtypes.get(&x) {
                Some(subs) => queue.extend(subs.iter().cloned()),
                None => out.push(x),
            }
        }
        out
    };
    let mut possible: std::collections::HashSet<(String, Option<String>, String)> =
        Default::default();
    for n in nt.as_array().cloned().unwrap_or_default() {
        let Some(parent) = n["type"].as_str() else {
            continue;
        };
        if n["named"].as_bool() != Some(true) {
            continue;
        }
        if let Some(fields) = n["fields"].as_object() {
            for (field, spec) in fields {
                for t in spec["types"].as_array().cloned().unwrap_or_default() {
                    let Some(child) = t["type"].as_str() else {
                        continue;
                    };
                    for child in expand(child) {
                        possible.insert((parent.to_string(), Some(field.clone()), child));
                    }
                }
            }
        }
        for t in n["children"]["types"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let Some(child) = t["type"].as_str() else {
                continue;
            };
            for child in expand(child) {
                possible.insert((parent.to_string(), None, child));
            }
        }
    }
    let name_of = |id: u16| ts_lang.node_kind_for_id(id).unwrap_or("?").to_string();
    let seen_edges: std::collections::HashSet<(String, Option<String>, String)> = edge_totals
        .keys()
        .map(|(p, f, c)| (name_of(*p), f.clone(), name_of(*c)))
        .collect();
    let mut never_by_parent: BTreeMap<String, usize> = BTreeMap::new();
    let mut edges_never: Vec<String> = Vec::new();
    for e in &possible {
        if !seen_edges.contains(e) {
            *never_by_parent.entry(e.0.clone()).or_insert(0) += 1;
            edges_never.push(match &e.1 {
                Some(f) => format!("{} {}: {}", e.0, f, e.2),
                None => format!("{} · {}", e.0, e.2),
            });
        }
    }
    edges_never.sort();
    let mut edges_never_by_parent: Vec<(String, usize)> = never_by_parent.into_iter().collect();
    edges_never_by_parent.sort_by_key(|(p, n)| (std::cmp::Reverse(*n), p.clone()));
    let edges_seen = possible.iter().filter(|e| seen_edges.contains(*e)).count();

    let report = KindsReport {
        lang: lang.to_string(),
        files_parsed,
        named_kinds_total: counts.len(),
        named_kinds_seen: counts.len() - never_seen.len(),
        never_seen,
        thin,
        counts,
        tokens_total: token_counts.len(),
        tokens_seen: token_counts.len() - tokens_never.len(),
        tokens_never,
        edges_possible: possible.len(),
        edges_seen,
        edges_never_by_parent: edges_never_by_parent.iter().take(15).cloned().collect(),
        edges_never,
    };

    println!(
        "kinds: {} of {} named kinds appear in {} files — {} never do, {} appear fewer than {THIN} times",
        report.named_kinds_seen,
        report.named_kinds_total,
        report.files_parsed,
        report.never_seen.len(),
        report.thin.len(),
    );
    println!(
        "kinds: tokens {}/{} · edges {}/{} of the shapes node-types.json allows",
        report.tokens_seen, report.tokens_total, report.edges_seen, report.edges_possible,
    );
    if !report.tokens_never.is_empty() {
        println!("  tokens never used: {}", report.tokens_never.join(" "));
    }
    for (p, n) in report.edges_never_by_parent.iter().take(6) {
        println!("  unbuilt edges {n:>4}  under {p}");
    }
    if !report.never_seen.is_empty() {
        println!("  never in the corpus: {}", report.never_seen.join(", "));
    }
    for (k, n) in report.thin.iter().take(12) {
        println!("  thin {n:>4}x  {k}");
    }

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&report)?)?;
    println!("kinds: report at {}", out_path.display());

    match (check, &policy) {
        (true, Some(policy)) => {
            if !policy.rule.is_empty() {
                println!("kinds: policy — {}", policy.rule.trim());
            }
            for a in &policy.accepted {
                println!("  accepted {:<20} {}", a.item, a.covered_by);
            }
            check_policy(&report, policy)?;
        }
        (true, None) => println!(
            "kinds: no [{lang}] section in {}/kinds_policy.toml — nothing declared, nothing enforced",
            grammar_dir.display()
        ),
        (false, _) => {}
    }
    Ok(())
}

pub fn default_out(lang: LangName) -> PathBuf {
    PathBuf::from(format!("corpus/{lang}/reports/kinds.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(never_kinds: &[&str], never_tokens: &[&str]) -> KindsReport {
        KindsReport {
            lang: "typescript".into(),
            files_parsed: 1,
            named_kinds_total: 0,
            named_kinds_seen: 0,
            never_seen: never_kinds.iter().map(|s| s.to_string()).collect(),
            thin: Vec::new(),
            counts: BTreeMap::new(),
            tokens_total: 0,
            tokens_seen: 0,
            tokens_never: never_tokens.iter().map(|s| s.to_string()).collect(),
            edges_possible: 0,
            edges_seen: 0,
            edges_never_by_parent: Vec::new(),
            edges_never: Vec::new(),
        }
    }

    fn policy(items: &[&str]) -> LanguagePolicy {
        LanguagePolicy {
            rule: String::new(),
            accepted: items
                .iter()
                .map(|i| Accepted {
                    item: i.to_string(),
                    spelling: String::new(),
                    covered_by: "fuzz".into(),
                    oracle: String::new(),
                    why: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_declaration_matching_the_measurement_passes() {
        let r = report(&["with_statement"], &["%="]);
        assert!(check_policy(&r, &policy(&["with_statement", "%="])).is_ok());
    }

    /// The case the file exists for: a blind spot nobody wrote down.
    #[test]
    fn an_undeclared_blind_spot_fails() {
        let r = report(&["with_statement", "static_block"], &[]);
        let err = check_policy(&r, &policy(&["with_statement"]))
            .expect_err("an undeclared blind spot must fail");
        assert!(err.to_string().contains("1 undeclared"), "{err}");
    }

    /// The direction that keeps the file honest as the corpus grows: a
    /// construct real code now contains must not keep its excuse.
    #[test]
    fn a_declaration_the_corpus_outgrew_fails() {
        let r = report(&[], &[]);
        let err = check_policy(&r, &policy(&["debugger_statement"]))
            .expect_err("a stale declaration must fail");
        assert!(err.to_string().contains("1 stale"), "{err}");
    }

    /// Tokens and node kinds share one namespace in the policy; a token
    /// declared where only a kind is blind is still stale.
    #[test]
    fn tokens_and_kinds_are_checked_together() {
        let r = report(&["with_statement"], &["%="]);
        let err = check_policy(&r, &policy(&["with_statement", "&&="]))
            .expect_err("wrong token must fail");
        let msg = err.to_string();
        assert!(msg.contains("1 undeclared") && msg.contains("1 stale"), "{msg}");
    }

    /// The committed policy must parse and must name the language it is
    /// for. A section keyed by a name no LangName spells would silently
    /// mean "advisory" forever.
    #[test]
    fn the_committed_typescript_policy_loads() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("treebank-typescript");
        let loaded = load_policy(&dir, LangName::Typescript)
            .expect("kinds_policy.toml must parse")
            .expect("it must carry a [typescript] section");
        assert!(!loaded.accepted.is_empty());
        assert!(!loaded.rule.is_empty(), "an accepted list needs its reason");
        for a in &loaded.accepted {
            assert!(!a.covered_by.is_empty(), "{} has no coverage", a.item);
        }
        // The grammar answers for javascript too, and its blind spots are
        // a different question; leaving it advisory is deliberate.
        assert!(load_policy(&dir, LangName::Javascript).unwrap().is_none());
    }
}
