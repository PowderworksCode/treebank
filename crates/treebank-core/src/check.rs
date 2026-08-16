//! The vocabulary-conformance checker (`treebank roles`, DESIGN.md §3.3).
//!
//! Everything here returns findings rather than failing fast, so one run
//! reports every violation. An empty findings list is conformance.

use std::collections::BTreeSet;

use crate::node_types::NodeTypes;
use crate::roles::RolesManifest;
use crate::Vocabulary;

pub fn check(nt: &NodeTypes, roles: &RolesManifest, vocab: &Vocabulary) -> Vec<String> {
    let mut findings = Vec::new();

    // Rule 0: the manifest was written against this vocabulary.
    if roles.vocabulary != vocab.version {
        findings.push(format!(
            "roles.json targets vocabulary {} but treebank-core carries {}",
            roles.vocabulary, vocab.version
        ));
    }

    // Rule 1: declared supertypes ⊆ the closed table tier.
    for name in nt.supertypes.keys() {
        if !vocab.is_table_term(name) {
            findings.push(format!(
                "supertype `{name}` is not a table-tier vocabulary term"
            ));
        }
    }

    // Rule 5 (checked before coverage so bad keys don't grant coverage):
    // facet keys ⊆ the closed facet tier, members non-empty and existing.
    for (facet, members) in &roles.facets {
        if !vocab.is_facet_term(facet) {
            findings.push(format!("facet `{facet}` is not a facet-tier vocabulary term"));
        }
        if members.is_empty() {
            findings.push(format!("facet `{facet}` has no members; omit it instead"));
        }
        for m in members {
            // Rule 3: every node named in roles.json exists in the grammar.
            if !nt.named.contains(m) {
                findings.push(format!(
                    "facet `{facet}` names `{m}`, which is not a named node of this grammar"
                ));
            }
        }
    }

    // Rule 2: every named node is covered by a table role, or in a facet,
    // or deliberately uncategorised — nothing is silently outside.
    let table_covered: BTreeSet<String> = nt
        .supertypes
        .keys()
        .filter(|s| vocab.is_table_term(s))
        .flat_map(|s| nt.closure(s))
        .collect();
    let facet_covered: BTreeSet<&str> = roles
        .facets
        .iter()
        .filter(|(f, _)| vocab.is_facet_term(f))
        .flat_map(|(_, ms)| ms.iter().map(String::as_str))
        .collect();
    let uncategorised: BTreeSet<&str> =
        roles.uncategorised.iter().map(|u| u.node.as_str()).collect();

    for node in &nt.named {
        if nt.supertypes.contains_key(node) {
            continue; // roles themselves are not subject to coverage
        }
        let covered = table_covered.contains(node) || facet_covered.contains(node.as_str());
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
    for u in &roles.uncategorised {
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

    // Rule 4: required containments hold wherever both terms are declared.
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

/// Role liveness (DESIGN.md §3.3 rule 5): given per-role occurrence counts
/// from a corpus sweep, the roles that never fired. Because supertype
/// matching is derivation-based, a role the grammar author forgot to
/// thread at some position fails silently — zero matches over a large
/// corpus is how it gets caught. The sweep supplies the counts; this just
/// names the dead roles.
pub fn dead_roles<'a>(
    declared: impl Iterator<Item = &'a str>,
    counts: &std::collections::BTreeMap<String, u64>,
) -> Vec<String> {
    declared
        .filter(|r| counts.get(*r).copied().unwrap_or(0) == 0)
        .map(String::from)
        .collect()
}
