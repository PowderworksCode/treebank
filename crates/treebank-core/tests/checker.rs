use std::collections::BTreeMap;

use treebank_core::check::{check, dead_roles};
use treebank_core::expand::expand;
use treebank_core::node_types::NodeTypes;
use treebank_core::roles::RolesManifest;
use treebank_core::vocabulary;

fn good_nt() -> NodeTypes {
    NodeTypes::parse(include_str!("fixtures/good-node-types.json")).unwrap()
}

fn good_roles() -> RolesManifest {
    RolesManifest::parse(include_str!("fixtures/good-roles.json")).unwrap()
}

#[test]
fn the_good_fixture_is_conformant() {
    let findings = check(&good_nt(), &good_roles(), vocabulary());
    assert!(findings.is_empty(), "unexpected findings: {findings:#?}");
}

// Every rule must be able to report a violation — a checker that cannot
// say non-zero proves nothing by saying zero. One mutation per rule.

#[test]
fn invented_supertype_is_a_finding() {
    let mut nt = good_nt();
    nt.supertypes.insert("_composite".into(), vec!["number".into()]);
    nt.named.insert("_composite".into());
    let f = check(&nt, &good_roles(), vocabulary());
    assert!(f.iter().any(|m| m.contains("`_composite`") && m.contains("not a table-tier")), "{f:#?}");
}

#[test]
fn uncovered_node_is_a_finding() {
    let mut nt = good_nt();
    nt.named.insert("mystery_node".into());
    let f = check(&nt, &good_roles(), vocabulary());
    assert!(f.iter().any(|m| m.contains("`mystery_node`") && m.contains("outside the vocabulary")), "{f:#?}");
}

#[test]
fn facet_naming_nonexistent_node_is_a_finding() {
    let mut roles = good_roles();
    roles.facets.get_mut("_callable").unwrap().push("ghost".into());
    let f = check(&good_nt(), &roles, vocabulary());
    assert!(f.iter().any(|m| m.contains("`ghost`") && m.contains("not a named node")), "{f:#?}");
}

#[test]
fn unknown_facet_key_is_a_finding() {
    let mut roles = good_roles();
    roles.facets.insert("_slop".into(), vec!["lambda".into()]);
    let f = check(&good_nt(), &roles, vocabulary());
    assert!(f.iter().any(|m| m.contains("`_slop`") && m.contains("neither a facet-tier")), "{f:#?}");
}

// Tier demotion (§3.1.1): a table-tier term a grammar delivers as a facet
// instead. Each guard gets a mutation, because a demotion that is not
// checked is indistinguishable from a supertype dropped by accident.

/// `_parameter` demoted the way a real grammar does it: absent from the
/// supertypes, present as a facet, declared with a reason.
fn demoting_roles() -> RolesManifest {
    let mut roles = good_roles();
    roles
        .demoted
        .insert("_parameter".into(), "the language orders its parameter list".into());
    roles
        .facets
        .insert("_parameter".into(), vec!["parameter".into()]);
    roles
}

/// What tree-sitter actually generates once the grammar stops threading
/// `_parameter`: the hidden rule is gone from node-types entirely, not
/// merely absent from the supertypes map.
fn nt_without_parameter_supertype() -> NodeTypes {
    let mut nt = good_nt();
    nt.supertypes.remove("_parameter");
    nt.named.remove("_parameter");
    nt
}

#[test]
fn a_declared_demotion_is_conformant() {
    let f = check(&nt_without_parameter_supertype(), &demoting_roles(), vocabulary());
    assert!(f.is_empty(), "unexpected findings: {f:#?}");
}

#[test]
fn demoting_a_term_the_vocabulary_pins_is_a_finding() {
    let mut roles = good_roles();
    roles.demoted.insert("_member".into(), "because".into());
    let f = check(&good_nt(), &roles, vocabulary());
    assert!(
        f.iter().any(|m| m.contains("`_member`") && m.contains("does not allow")),
        "{f:#?}"
    );
}

#[test]
fn demotion_without_a_reason_is_a_finding() {
    let mut roles = demoting_roles();
    roles.demoted.insert("_parameter".into(), "   ".into());
    let f = check(&nt_without_parameter_supertype(), &roles, vocabulary());
    assert!(f.iter().any(|m| m.contains("`_parameter`") && m.contains("no reason")), "{f:#?}");
}

#[test]
fn a_term_in_both_tiers_is_a_finding() {
    // The supertype is still declared, so the grammar would answer
    // `(_parameter)` two ways at once.
    let f = check(&good_nt(), &demoting_roles(), vocabulary());
    assert!(
        f.iter().any(|m| m.contains("`_parameter`") && m.contains("exactly one tier")),
        "{f:#?}"
    );
}

#[test]
fn demotion_without_facet_members_is_a_finding() {
    let mut roles = demoting_roles();
    roles.facets.remove("_parameter");
    let f = check(&nt_without_parameter_supertype(), &roles, vocabulary());
    assert!(
        f.iter().any(|m| m.contains("`_parameter`") && m.contains("no facet members")),
        "{f:#?}"
    );
}

#[test]
fn broken_containment_is_a_finding() {
    let mut nt = good_nt();
    // Pull `_literal` out of `_expression`: literal members no longer
    // reachable from the outer term.
    nt.supertypes.get_mut("_expression").unwrap().retain(|s| s != "_literal");
    let f = check(&nt, &good_roles(), vocabulary());
    assert!(f.iter().any(|m| m.contains("containment violated") && m.contains("`_literal`")), "{f:#?}");
}

#[test]
fn stale_uncategorised_entry_is_a_finding() {
    let mut roles = good_roles();
    roles.facets.get_mut("_callable").unwrap().push("comment".into());
    let f = check(&good_nt(), &roles, vocabulary());
    assert!(f.iter().any(|m| m.contains("`comment`") && m.contains("stale")), "{f:#?}");
}

#[test]
fn vocabulary_version_mismatch_is_a_finding() {
    let mut roles = good_roles();
    roles.vocabulary = "0.0.9".into();
    let f = check(&good_nt(), &roles, vocabulary());
    assert!(f.iter().any(|m| m.contains("0.0.9")), "{f:#?}");
}

#[test]
fn dead_roles_names_the_silent_ones() {
    let counts: BTreeMap<String, u64> =
        [("_loop".to_string(), 12u64), ("_branch".to_string(), 0u64)].into();
    let dead = dead_roles(["_loop", "_branch", "_jump"].into_iter(), &counts);
    assert_eq!(dead, vec!["_branch".to_string(), "_jump".to_string()]);
}

// --- facet expansion ---

fn facets() -> BTreeMap<String, Vec<String>> {
    good_roles().facets.into_iter().collect()
}

#[test]
fn bare_facet_expands_to_alternation() {
    let q = expand("(_callable) @fn", &facets()).unwrap();
    assert_eq!(q, "[(function_definition) (lambda)] @fn");
}

#[test]
fn facet_body_is_copied_into_every_branch() {
    let q = expand("(_callable name: (_name) @n)", &facets()).unwrap();
    assert_eq!(
        q,
        "[(function_definition name: (_name) @n) (lambda name: (_name) @n)]"
    );
}

#[test]
fn nested_facets_expand_inside_out() {
    let q = expand("(_scope (_callable) @inner)", &facets()).unwrap();
    assert_eq!(
        q,
        "[(module [(function_definition) (lambda)] @inner) \
         (function_definition [(function_definition) (lambda)] @inner) \
         (lambda [(function_definition) (lambda)] @inner)]"
    );
}

#[test]
fn table_supertypes_strings_and_comments_pass_through() {
    let src = "; find loops\n(_loop \"while\" @kw (_expression))";
    assert_eq!(expand(src, &facets()).unwrap(), src);
}

#[test]
fn facet_name_inside_string_is_not_rewritten() {
    let src = "((identifier) @x (#eq? @x \"(_callable)\"))";
    assert_eq!(expand(src, &facets()).unwrap(), src);
}

#[test]
fn unbalanced_query_errors() {
    assert!(expand("(_callable", &facets()).is_err());
}
