//! The `terms.json` manifest each grammar crate ships (notes/DESIGN.md
//! §3.1): the vocabulary terms this grammar delivers NOMINALLY, as a list
//! of node types rather than as a supertype in the parse table. Nominal
//! membership is decided by name, so it is a property of the node type
//! rather than of the occurrence: a node type is `_callable` wherever it
//! occurs. The manifest also carries the grammar's `uncategorised` list —
//! every named node outside the vocabulary, each with a reason — so that
//! nothing is silently outside it (§3.3 rule 2).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TermsManifest {
    /// The vocabulary version this manifest was written against; must
    /// match the version treebank carries.
    pub vocabulary: String,
    /// Nominal term -> member node types. `facets` is accepted as an
    /// alias so a pack published before the rename still loads; see
    /// notes/vocabulary-naming.md §5.
    #[serde(default, alias = "facets")]
    pub nominal: BTreeMap<String, Vec<String>>,
    /// Structural terms this grammar delivers nominally instead, each with
    /// the reason its language forced the demotion. Only terms the
    /// vocabulary marks `demotable` may appear here, and a demoted term
    /// must be a nominal key rather than a declared supertype.
    #[serde(default)]
    pub demoted: BTreeMap<String, String>,
    /// Named nodes deliberately outside the vocabulary, with a reason each.
    #[serde(default)]
    pub uncategorised: Vec<Uncategorised>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Uncategorised {
    pub node: String,
    pub reason: String,
}

impl TermsManifest {
    pub fn parse(json: &str) -> Result<TermsManifest> {
        serde_json::from_str(json).context("parse terms.json")
    }

    pub fn load(path: &Path) -> Result<TermsManifest> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Self::parse(&text)
    }
}
