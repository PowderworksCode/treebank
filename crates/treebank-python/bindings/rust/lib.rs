//! treebank-python — the treebank Python grammars.
//!
//! Two parsers, one per variant (VARIANTS.md): [`LANGUAGE`] is Python 3
//! and [`LANGUAGE_PYTHON2`] is Python 2.7. `LANGUAGE` keeps its name and
//! its meaning — python 3 is what it has always been — so nothing
//! downstream changes by the split.
//!
//! Beyond the usual tree-sitter crate surface this exposes [`ROLES`], the
//! grammar's `roles.json`: the facet-tier membership (`_callable`,
//! `_binding`, `_scope`, `_clause`) that cannot live in the parse table,
//! plus the nodes deliberately outside the vocabulary and why. It travels
//! inside the published crate so a consumer never has to fetch it
//! separately — the table-tier roles are already queryable from the
//! parser itself.

use tree_sitter_language::LanguageFn;

extern "C" {
    fn tree_sitter_python() -> *const ();
    fn tree_sitter_python2() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for Python 3 — the default variant.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_python) };

/// The tree-sitter [`LanguageFn`] for Python 2.7.
///
/// A separate parse table rather than a corner of the python 3 one: the
/// two languages disagree about whether `print` is a keyword, what `0777`
/// and `10L` mean, and whether `f"…"` is a string, and a single table can
/// only answer one way. See VARIANTS.md §6 and `variants.toml`.
pub const LANGUAGE_PYTHON2: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_python2) };

/// The generated `node-types.json` for Python 3, describing every node and
/// field.
pub const NODE_TYPES: &str = include_str!("../../python3/src/node-types.json");

/// The generated `node-types.json` for Python 2.7.
pub const NODE_TYPES_PYTHON2: &str = include_str!("../../python2/src/node-types.json");

/// The facet manifest (`roles.json`), shared by the variants.
pub const ROLES: &str = include_str!("../../roles.json");

/// Python 2's additions to [`ROLES`]: the facet members that exist only in
/// that variant, and the shared roles it declares absent. A delta rather
/// than a second manifest, so a role threaded in one variant and forgotten
/// in the other cannot hide (VARIANTS.md §4).
pub const ROLES_PYTHON2_DELTA: &str = include_str!("../../python2/roles.delta.json");

/// What variants exist, why, and the measurement behind each split.
pub const VARIANTS: &str = include_str!("../../variants.toml");

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
    fn can_load_python2() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE_PYTHON2.into())
            .expect("load treebank-python2");
    }

    /// The split is only real if the two tables disagree. `print "x"` is a
    /// statement in one and a syntax error in the other, and that is the
    /// whole point of there being two.
    #[test]
    fn the_variants_are_different_languages() {
        let py2_only = b"print \"x\", y\n";
        let mut p3 = tree_sitter::Parser::new();
        p3.set_language(&super::LANGUAGE.into()).unwrap();
        let mut p2 = tree_sitter::Parser::new();
        p2.set_language(&super::LANGUAGE_PYTHON2.into()).unwrap();

        assert!(
            p3.parse(py2_only, None).unwrap().root_node().has_error(),
            "python 3 must reject the py2 print statement"
        );
        let tree = p2.parse(py2_only, None).unwrap();
        assert!(!tree.root_node().has_error(), "python 2 must accept it");
        assert_eq!(tree.root_node().child(0).unwrap().kind(), "print_statement");
    }

    #[test]
    fn ships_its_manifests() {
        let roles: serde_json::Value = serde_json::from_str(super::ROLES).unwrap();
        assert!(roles["facets"].is_object(), "roles.json carries facets");
        let ledger: toml::Value = toml::from_str(super::LEDGER).unwrap();
        assert_eq!(ledger["language"].as_str(), Some("python"));
        let delta: serde_json::Value = serde_json::from_str(super::ROLES_PYTHON2_DELTA).unwrap();
        assert!(delta["adds"].is_object(), "the py2 delta carries its additions");
        let variants: toml::Value = toml::from_str(super::VARIANTS).unwrap();
        assert_eq!(variants["variants"].as_array().map(|v| v.len()), Some(2));
    }
}
