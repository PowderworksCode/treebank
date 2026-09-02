//! nst/JSONTestSuite as a gate, in all three of its directions.
//!
//! JSON is the only language in this repository that arrives with a
//! negative corpus somebody else wrote and answered. Nicolas Seriot's
//! suite (commit `1ef36fa0`, 2024-11-22, MIT) is 318 files under three
//! prefixes: `y_` must be accepted, `n_` must be rejected, and `i_` is
//! implementation-defined — a conforming parser may go either way, and
//! the RFC says so (§9 on depth limits, §6 on number range, §8.2 on
//! unpaired surrogates).
//!
//! The `n_` files live in `test/negative/`, which is the repository's own
//! must-reject gate, so `treebank negative` and `treebank verify` already
//! run them; the first test here runs them again from `cargo test` so the
//! workspace gate does not depend on the CLI. The `y_` and `i_` files live
//! in `test/jsontestsuite/`, because `test/negative/` is defined as
//! everything-must-fail and these must not.
//!
//! The third test is the one worth explaining. `i_` files have no right
//! answer, so an assertion about them cannot be "conform" — it can only be
//! "do not change silently". The list below is what this grammar does
//! today: 22 of the 35 accepted, against 5 for the pinned oracle, and the
//! two sets agree on 18 files and disagree on 17. Every one of those 17 is
//! a place where a tokenizer and a value parser must part company, because
//! serde_json builds VALUES — a number that will not fit an f64, a `\u`
//! escape that is half a surrogate pair, a nesting depth past its
//! recursion limit. None of those are syntax, and a grammar that claimed
//! to police them would be claiming to evaluate JSON rather than parse it.
//! Pinning the list means a future change to `number` or `escape_sequence`
//! has to come and edit this file and say why.

use std::path::{Path, PathBuf};

use tree_sitter::Parser;

/// Files under `test/jsontestsuite/` whose prefix marks them
/// implementation-defined and which this grammar ACCEPTS. Everything else
/// with an `i_` prefix is rejected. The classes, and why each one is a
/// question about values rather than about syntax, are in ledger.toml.
const IMPLEMENTATION_DEFINED_ACCEPTED: &[&str] = &[
    // Number magnitude: the grammar reads digits, it does not evaluate
    // them, so an exponent no float can hold is still a `number`.
    "i_number_double_huge_neg_exp.json",
    "i_number_huge_exp.json",
    "i_number_neg_int_huge_exp.json",
    "i_number_pos_double_huge_exp.json",
    "i_number_real_neg_overflow.json",
    "i_number_real_pos_overflow.json",
    "i_number_real_underflow.json",
    "i_number_too_big_neg_int.json",
    "i_number_too_big_pos_int.json",
    "i_number_very_big_negative_int.json",
    // Surrogate pairing: `\uD800` is four hex digits and a well-formed
    // escape. Whether two of them pair up is a property of the decoded
    // string, which is one layer above the parse tree.
    "i_object_key_lone_2nd_surrogate.json",
    "i_string_1st_surrogate_but_2nd_missing.json",
    "i_string_1st_valid_surrogate_2nd_invalid.json",
    "i_string_incomplete_surrogate_and_escape_valid.json",
    "i_string_incomplete_surrogate_pair.json",
    "i_string_incomplete_surrogates_escape_valid.json",
    "i_string_invalid_lonely_surrogate.json",
    "i_string_invalid_surrogate.json",
    "i_string_inverted_surrogates_U+1D11E.json",
    "i_string_lone_second_surrogate.json",
    // Nesting depth: RFC 8259 §9 lets an implementation set a limit and
    // this grammar sets none. serde_json's default is 128.
    "i_structure_500_nested_arrays.json",
    // A leading UTF-8 BOM, and this one is not the grammar's doing:
    // tree-sitter's input layer strips a BOM before the lexer runs, which
    // is checkable — the same bytes anywhere but the start of the file are
    // an UNEXPECTED 65279. RFC 8259 §8.1 says an implementation MAY ignore
    // a leading BOM, so accepting it conforms and serde_json rejecting it
    // conforms; the entry is here because the reason lives outside
    // grammar.js and would otherwise look like a rule nobody wrote.
    "i_structure_UTF-8_BOM_empty_object.json",
];

fn suite_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
}

fn files_with_prefix(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| {
            path.extension().is_some_and(|e| e == "json")
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(prefix))
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no {prefix}* files in {}", dir.display());
    paths
}

/// Parses without an ERROR node. Reads BYTES, not text: several files in
/// the suite are deliberately not valid UTF-8, and a harness that decoded
/// first would decide their verdict before the grammar saw them.
fn parses(parser: &mut Parser, path: &Path) -> bool {
    let src = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    parser
        .parse(&src, None)
        .is_some_and(|tree| !tree.root_node().has_error())
}

fn parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&treebank_json::LANGUAGE.into())
        .expect("load treebank-json");
    parser
}

#[test]
fn accepts_every_must_accept_file() {
    let dir = suite_dir("test/jsontestsuite");
    let mut parser = parser();
    let rejected: Vec<String> = files_with_prefix(&dir, "y_")
        .into_iter()
        .filter(|path| !parses(&mut parser, path))
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        rejected.is_empty(),
        "must-accept files the grammar rejected: {rejected:?}"
    );
}

#[test]
fn rejects_every_must_reject_file() {
    let dir = suite_dir("test/negative");
    let mut parser = parser();
    let accepted: Vec<String> = files_with_prefix(&dir, "n_")
        .into_iter()
        .filter(|path| parses(&mut parser, path))
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        accepted.is_empty(),
        "must-reject files the grammar accepted: {accepted:?}"
    );
}

#[test]
fn implementation_defined_verdicts_are_pinned() {
    let dir = suite_dir("test/jsontestsuite");
    let mut parser = parser();
    let accepted: Vec<String> = files_with_prefix(&dir, "i_")
        .into_iter()
        .filter(|path| parses(&mut parser, path))
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    let expected: Vec<String> = IMPLEMENTATION_DEFINED_ACCEPTED
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(
        accepted, expected,
        "the implementation-defined verdicts moved; neither list is wrong by \
         itself, but the change belongs in ledger.toml before it belongs here"
    );
}
