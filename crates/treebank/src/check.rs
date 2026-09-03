//! The vocabulary-conformance checker (`treebank terms`, notes/DESIGN.md §3.3).
//!
//! Everything here returns findings rather than failing fast, so one run
//! reports every violation. An empty findings list is conformance.

use std::collections::BTreeSet;

use crate::node_types::NodeTypes;
use crate::terms::TermsManifest;
use crate::Vocabulary;

pub fn check(nt: &NodeTypes, terms: &TermsManifest, vocab: &Vocabulary) -> Vec<String> {
    let mut findings = Vec::new();

    // Rule 0: the manifest was written against this vocabulary.
    if terms.vocabulary != vocab.version {
        findings.push(format!(
            "terms.json targets vocabulary {} but treebank carries {}",
            terms.vocabulary, vocab.version
        ));
    }

    // Rule 1: declared supertypes ⊆ the closed structural list.
    for name in nt.supertypes.keys() {
        if !vocab.is_structural_term(name) {
            findings.push(format!(
                "supertype `{name}` is not a structural vocabulary term"
            ));
        }
    }

    // Rule 1b: demotion is declared, justified, and exclusive. A structural
    // term this grammar cannot express as one alternation may be delivered
    // nominally instead, but only if the vocabulary marks it `demotable`,
    // only with a reason, and never both ways at once — otherwise dropping
    // a supertype by accident is indistinguishable from demoting one on
    // purpose.
    for (term, reason) in &terms.demoted {
        if !vocab.is_demotable(term) {
            findings.push(format!(
                "`{term}` is demoted to nominal, but the vocabulary does not allow \
                 that term's delivery to vary by grammar"
            ));
        }
        if reason.trim().is_empty() {
            findings.push(format!("demoted term `{term}` has no reason"));
        }
        if nt.supertypes.contains_key(term) {
            findings.push(format!(
                "`{term}` is demoted but is also declared as a supertype; a term is \
                 delivered exactly one way per grammar"
            ));
        }
        if !terms.nominal.contains_key(term) {
            findings.push(format!(
                "`{term}` is demoted to nominal but has no nominal members"
            ));
        }
    }

    // Rule 5 (checked before coverage so bad keys don't grant coverage):
    // nominal keys ⊆ the closed nominal list, members non-empty and existing.
    for (name, members) in &terms.nominal {
        if !vocab.is_nominal_term(name) && !terms.demoted.contains_key(name) {
            findings.push(format!(
                "`{name}` is neither a nominal vocabulary term nor a declared demotion"
            ));
        }
        if members.is_empty() {
            findings.push(format!(
                "nominal term `{name}` has no members; omit it instead"
            ));
        }
        for m in members {
            // Rule 3: every node named in terms.json exists in the grammar.
            if !nt.named.contains(m) {
                findings.push(format!(
                    "nominal term `{name}` names `{m}`, which is not a named node of this grammar"
                ));
            }
        }
    }

    // Rule 2: every named node is covered structurally, or nominally, or
    // deliberately uncategorised — nothing is silently outside.
    let structural_covered: BTreeSet<String> = nt
        .supertypes
        .keys()
        .filter(|s| vocab.is_structural_term(s))
        .flat_map(|s| nt.closure(s))
        .collect();
    let nominal_covered: BTreeSet<&str> = terms
        .nominal
        .iter()
        .filter(|(n, _)| vocab.is_nominal_term(n) || terms.demoted.contains_key(*n))
        .flat_map(|(_, ms)| ms.iter().map(String::as_str))
        .collect();
    let uncategorised: BTreeSet<&str> = terms
        .uncategorised
        .iter()
        .map(|u| u.node.as_str())
        .collect();

    for node in &nt.named {
        if nt.supertypes.contains_key(node) {
            continue; // the terms themselves are not subject to coverage
        }
        let covered = structural_covered.contains(node) || nominal_covered.contains(node.as_str());
        if !covered && !uncategorised.contains(node.as_str()) {
            findings.push(format!(
                "named node `{node}` is outside the vocabulary and not ledgered as uncategorised"
            ));
        }
        if covered && uncategorised.contains(node.as_str()) {
            findings.push(format!(
                "`{node}` is listed uncategorised but is covered — remove the stale entry"
            ));
        }
    }
    for u in &terms.uncategorised {
        if !nt.named.contains(&u.node) {
            findings.push(format!(
                "uncategorised entry `{}` is not a named node of this grammar",
                u.node
            ));
        }
        if u.reason.trim().is_empty() {
            findings.push(format!("uncategorised entry `{}` has no reason", u.node));
        }
    }

    // Rule 4: required containments hold wherever both terms are declared
    // as supertypes. A demoted term has no derivation closure to check, so
    // its containments are unenforceable and deliberately skipped.
    for (inner, outer) in &vocab.containments {
        let (Some(_), Some(_)) = (nt.supertypes.get(inner), nt.supertypes.get(outer)) else {
            continue;
        };
        let outer_closure = nt.closure(outer);
        let inner_closure = nt.closure(inner);
        let holds = outer_closure.contains(inner)
            || inner_closure.iter().all(|n| outer_closure.contains(n));
        if !holds {
            findings.push(format!(
                "containment violated: `{inner}` must nest inside `{outer}`"
            ));
        }
    }

    findings
}

/// Term liveness (notes/DESIGN.md §3.3 rule 5): given per-term occurrence
/// counts from a corpus sweep, the terms that never fired. Because
/// structural matching is derivation-based, a term the grammar author
/// forgot to thread at some position fails silently — zero matches over a
/// large corpus is how it gets caught. The sweep supplies the counts; this
/// just names the dead terms.
pub fn dead_terms<'a>(
    declared: impl Iterator<Item = &'a str>,
    counts: &std::collections::BTreeMap<String, u64>,
) -> Vec<String> {
    declared
        .filter(|r| counts.get(*r).copied().unwrap_or(0) == 0)
        .map(String::from)
        .collect()
}
