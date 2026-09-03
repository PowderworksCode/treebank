//! treebank-python — the treebank Python grammar.
//!
//! Beyond the usual tree-sitter crate surface this exposes [`TERMS`], the
//! grammar's `terms.json`: the vocabulary terms this grammar delivers
//! NOMINALLY (`_callable`, `_binding`, `_scope`, `_clause`), as a list of
//! node types rather than as a supertype in the parse table, plus the nodes
//! deliberately outside the vocabulary and why. It travels inside the
//! published crate so a consumer never has to fetch it separately — the
//! structural terms are already queryable from the parser itself.

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_python() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for this grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_python) };

/// The generated `node-types.json`, describing every node and field.
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

/// The grammar's nominal manifest (`terms.json`).
pub const TERMS: &str = include_str!("../../terms.json");

/// The grammar's evidence file (`ledger.toml`): versions covered, pinned
/// oracles, corpus and sweep numbers, known gaps and declared deviations.
///
/// TOML rather than JSON because the content is mostly prose — a paragraph
/// explaining why a deviation exists is one escaped line in JSON and a
/// readable block in TOML, and this file is meant to be read by whoever is
/// deciding whether to trust the grammar.
pub const LEDGER: &str = include_str!("../../ledger.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn can_load_grammar() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("load treebank-python");
    }

    #[test]
    fn ships_its_manifests() {
        let terms: serde_json::Value = serde_json::from_str(super::TERMS).unwrap();
        assert!(
            terms["nominal"].is_object(),
            "terms.json carries its nominal terms"
        );
        let ledger: toml::Value = toml::from_str(super::LEDGER).unwrap();
        assert_eq!(ledger["language"].as_str(), Some("python"));
    }
}
