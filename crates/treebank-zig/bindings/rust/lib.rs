//! treebank-zig — the treebank Zig grammar.
//!
//! Beyond the usual tree-sitter crate surface this exposes [`ROLES`], the
//! grammar's `roles.json`: the facet-tier membership (`_callable`,
//! `_binding`, `_scope`, `_clause`, `_comment`, `_identifier`, `_string`)
//! that cannot live in the parse table, plus the nodes deliberately
//! outside the vocabulary and why. It travels inside the published crate
//! so a consumer never has to fetch it separately — the table-tier roles
//! are already queryable from the parser itself.

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_zig() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for this grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_zig) };

/// The generated `node-types.json`, describing every node and field.
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

/// The grammar's facet manifest (`roles.json`).
pub const ROLES: &str = include_str!("../../roles.json");

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
            .expect("load treebank-zig");
    }

    #[test]
    fn ships_its_manifests() {
        let roles: serde_json::Value = serde_json::from_str(super::ROLES).unwrap();
        assert!(roles["facets"].is_object(), "roles.json carries facets");
        let ledger: toml::Value = toml::from_str(super::LEDGER).unwrap();
        assert_eq!(ledger["language"].as_str(), Some("zig"));
    }
}
