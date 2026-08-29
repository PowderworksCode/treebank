//! The `roles.json` facet manifest each grammar crate ships (DESIGN.md
//! §3.1). Facets are type-level: a node type is `_callable` wherever it
//! occurs. The manifest also carries the grammar's `uncategorised` list —
//! every named node outside the vocabulary, each with a reason — so that
//! nothing is silently outside it (§3.3 rule 2).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RolesManifest {
    /// The vocabulary version this manifest was written against; must
    /// match the version treebank carries.
    pub vocabulary: String,
    /// Facet term -> member node types.
    #[serde(default)]
    pub facets: BTreeMap<String, Vec<String>>,
    /// Table-tier terms this grammar delivers as facets instead, each with
    /// the reason its language forced the demotion. Only terms the
    /// vocabulary marks `either_tier` may appear here, and a demoted term
    /// must be a facet key rather than a declared supertype.
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

impl RolesManifest {
    pub fn parse(json: &str) -> Result<RolesManifest> {
        serde_json::from_str(json).context("parse roles.json")
    }

    pub fn load(path: &Path) -> Result<RolesManifest> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Self::parse(&text)
    }
}
