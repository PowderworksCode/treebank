use std::collections::BTreeMap;

use treebank::check::{check, dead_terms};
use treebank::expand::{expand, expand_with_types};
use treebank::node_types::NodeTypes;
use treebank::terms::TermsManifest;
use treebank::vocabulary;

fn good_nt() -> NodeTypes {
    NodeTypes::parse(include_str!("fixtures/good-node-types.json")).unwrap()
}

fn good_terms() -> TermsManifest {
    TermsManifest::parse(include_str!("fixtures/good-terms.json")).unwrap()
}

#[test]
fn the_good_fixture_is_conformant() {
    let findings = check(&good_nt(), &good_terms(), vocabulary());
    assert!(findings.is_empty(), "unexpected findings: {findings:#?}");
}

// Every rule must be able to report a violation — a checker that cannot
// say non-zero proves nothing by saying zero. One mutation per rule.

#[test]
fn invented_supertype_is_a_finding() {
    let mut nt = good_nt();
    nt.supertypes
        .insert("_composite".into(), vec!["number".into()]);
    nt.named.insert("_composite".into());
    let f = check(&nt, &good_terms(), vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("`_composite`") && m.contains("not a structural")),
        "{f:#?}"
    );
}

#[test]
fn uncovered_node_is_a_finding() {
    let mut nt = good_nt();
    nt.named.insert("mystery_node".into());
    let f = check(&nt, &good_terms(), vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("`mystery_node`") && m.contains("outside the vocabulary")),
        "{f:#?}"
    );
}

#[test]
fn nominal_term_naming_nonexistent_node_is_a_finding() {
    let mut terms = good_terms();
    terms
        .nominal
        .get_mut("_callable")
        .unwrap()
        .push("ghost".into());
    let f = check(&good_nt(), &terms, vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("`ghost`") && m.contains("not a named node")),
        "{f:#?}"
    );
}

#[test]
fn unknown_nominal_key_is_a_finding() {
    let mut terms = good_terms();
    terms.nominal.insert("_slop".into(), vec!["lambda".into()]);
    let f = check(&good_nt(), &terms, vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("`_slop`") && m.contains("neither a nominal")),
        "{f:#?}"
    );
}

// Demotion (§3.1.1): a structural term a grammar delivers nominally
// instead. Each guard gets a mutation, because a demotion that is not
// checked is indistinguishable from a supertype dropped by accident.

/// `_parameter` demoted the way a real grammar does it: absent from the
/// supertypes, present as a nominal term, declared with a reason.
fn demoting_terms() -> TermsManifest {
    let mut terms = good_terms();
    terms.demoted.insert(
        "_parameter".into(),
        "the language orders its parameter list".into(),
    );
    terms
        .nominal
        .insert("_parameter".into(), vec!["parameter".into()]);
    terms
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
    let f = check(
        &nt_without_parameter_supertype(),
        &demoting_terms(),
        vocabulary(),
    );
    assert!(f.is_empty(), "unexpected findings: {f:#?}");
}

#[test]
fn demoting_a_term_the_vocabulary_pins_is_a_finding() {
    let mut terms = good_terms();
    terms.demoted.insert("_member".into(), "because".into());
    let f = check(&good_nt(), &terms, vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("`_member`") && m.contains("does not allow")),
        "{f:#?}"
    );
}

#[test]
fn demotion_without_a_reason_is_a_finding() {
    let mut terms = demoting_terms();
    terms.demoted.insert("_parameter".into(), "   ".into());
    let f = check(&nt_without_parameter_supertype(), &terms, vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("`_parameter`") && m.contains("no reason")),
        "{f:#?}"
    );
}

#[test]
fn a_term_delivered_both_ways_is_a_finding() {
    // The supertype is still declared, so the grammar would answer
    // `(_parameter)` two ways at once.
    let f = check(&good_nt(), &demoting_terms(), vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("`_parameter`") && m.contains("exactly one way")),
        "{f:#?}"
    );
}

#[test]
fn demotion_without_nominal_members_is_a_finding() {
    let mut terms = demoting_terms();
    terms.nominal.remove("_parameter");
    let f = check(&nt_without_parameter_supertype(), &terms, vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("`_parameter`") && m.contains("no nominal members")),
        "{f:#?}"
    );
}

#[test]
fn broken_containment_is_a_finding() {
    let mut nt = good_nt();
    // Pull `_literal` out of `_expression`: literal members no longer
    // reachable from the outer term.
    nt.supertypes
        .get_mut("_expression")
        .unwrap()
        .retain(|s| s != "_literal");
    let f = check(&nt, &good_terms(), vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("containment violated") && m.contains("`_literal`")),
        "{f:#?}"
    );
}

#[test]
fn stale_uncategorised_entry_is_a_finding() {
    let mut terms = good_terms();
    terms
        .nominal
        .get_mut("_callable")
        .unwrap()
        .push("comment".into());
    let f = check(&good_nt(), &terms, vocabulary());
    assert!(
        f.iter()
            .any(|m| m.contains("`comment`") && m.contains("stale")),
        "{f:#?}"
    );
}

#[test]
fn vocabulary_version_mismatch_is_a_finding() {
    let mut terms = good_terms();
    terms.vocabulary = "0.0.9".into();
    let f = check(&good_nt(), &terms, vocabulary());
    assert!(f.iter().any(|m| m.contains("0.0.9")), "{f:#?}");
}

#[test]
fn dead_terms_names_the_silent_ones() {
    let counts: BTreeMap<String, u64> =
        [("_loop".to_string(), 12u64), ("_branch".to_string(), 0u64)].into();
    let dead = dead_terms(["_loop", "_branch", "_jump"].into_iter(), &counts);
    assert_eq!(dead, vec!["_branch".to_string(), "_jump".to_string()]);
}

// --- nominal expansion ---

fn nominal() -> BTreeMap<String, Vec<String>> {
    good_terms().nominal.into_iter().collect()
}

#[test]
fn a_bare_nominal_term_expands_to_alternation() {
    let q = expand("(_callable) @fn", &nominal()).unwrap();
    assert_eq!(q, "[(function_definition) (lambda)] @fn");
}

#[test]
fn a_nominal_body_is_copied_into_every_branch() {
    let q = expand("(_callable name: (_name) @n)", &nominal()).unwrap();
    assert_eq!(
        q,
        "[(function_definition name: (_name) @n) (lambda name: (_name) @n)]"
    );
}

/// `_` is tree-sitter's wildcard, not a node type. Read as a type name it
/// matches nothing any field declares, so filtering drops every member and a
/// perfectly ordinary query dies with "no member satisfies the field
/// constraint".
///
/// A differential against the browser port cannot catch this: both sides
/// agreed, because both had it. Parity is agreement, not correctness, so the
/// expected answer is written out here.
#[test]
fn a_wildcard_field_value_constrains_presence_only() {
    let types = node_types_for_wildcards();
    let nominal = nominal();

    // `lambda` has no `name` and is dropped; `function_definition` has one and
    // survives, whatever type the wildcard stands for.
    let q = expand_with_types("(_callable name: (_) @n)", &nominal, Some(&types)).unwrap();
    assert_eq!(q, "[(function_definition name: (_) @n)]");

    // A wildcard anywhere in an alternation makes the whole constraint
    // presence-only.
    let q = expand_with_types(
        "(_callable name: [(_) (identifier)])",
        &nominal,
        Some(&types),
    )
    .unwrap();
    assert_eq!(q, "[(function_definition name: [(_) (identifier)])]");

    // A bare `_` with no parens was already presence-only; it stays that way.
    let q = expand_with_types("(_callable name: _)", &nominal, Some(&types)).unwrap();
    assert_eq!(q, "[(function_definition name: _)]");

    // And a real type name still filters on the type, not merely presence.
    let q = expand_with_types("(_callable name: (identifier) @n)", &nominal, Some(&types)).unwrap();
    assert_eq!(q, "[(function_definition name: (identifier) @n)]");
}

/// Two members, one of which has a `name` field.
fn node_types_for_wildcards() -> NodeTypes {
    NodeTypes::parse(
        r#"[
          {"type": "function_definition", "named": true,
           "fields": {"name": {"types": [{"type": "identifier", "named": true}]}}},
          {"type": "lambda", "named": true, "fields": {}},
          {"type": "identifier", "named": true, "fields": {}}
        ]"#,
    )
    .unwrap()
}

#[test]
fn nested_nominal_terms_expand_inside_out() {
    let q = expand("(_scope (_callable) @inner)", &nominal()).unwrap();
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
    assert_eq!(expand(src, &nominal()).unwrap(), src);
}

#[test]
fn a_term_name_inside_a_string_is_not_rewritten() {
    let src = "((identifier) @x (#eq? @x \"(_callable)\"))";
    assert_eq!(expand(src, &nominal()).unwrap(), src);
}

#[test]
fn unbalanced_query_errors() {
    assert!(expand("(_callable", &nominal()).is_err());
}
