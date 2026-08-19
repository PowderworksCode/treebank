//! A minimal reading of tree-sitter's generated `src/node-types.json`:
//! which named node types exist, and which of them are supertypes (the
//! entries carrying a `subtypes` list).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct RawEntry {
    #[serde(rename = "type")]
    type_name: String,
    named: bool,
    #[serde(default)]
    subtypes: Vec<RawRef>,
}

#[derive(Deserialize)]
struct RawRef {
    #[serde(rename = "type")]
    type_name: String,
    named: bool,
}

#[derive(Debug)]
pub struct NodeTypes {
    /// Every named node type, supertypes included.
    pub named: BTreeSet<String>,
    /// Supertype name -> direct named subtypes.
    pub supertypes: BTreeMap<String, Vec<String>>,
}

impl NodeTypes {
    pub fn parse(json: &str) -> Result<NodeTypes> {
        let raw: Vec<RawEntry> = serde_json::from_str(json).context("parse node-types.json")?;
        let mut named = BTreeSet::new();
        let mut supertypes = BTreeMap::new();
        for e in &raw {
            if !e.named {
                continue;
            }
            named.insert(e.type_name.clone());
            if !e.subtypes.is_empty() {
                supertypes.insert(
                    e.type_name.clone(),
                    e.subtypes
                        .iter()
                        .filter(|s| s.named)
                        .map(|s| s.type_name.clone())
                        .collect(),
                );
            }
        }
        Ok(NodeTypes { named, supertypes })
    }

    pub fn load(path: &Path) -> Result<NodeTypes> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Self::parse(&text)
    }

    /// The transitive subtype closure of a supertype: every named node type
    /// reachable through nested supertypes, the supertypes themselves
    /// included.
    pub fn closure(&self, supertype: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut stack = vec![supertype.to_string()];
        while let Some(name) = stack.pop() {
            if !out.insert(name.clone()) {
                continue;
            }
            if let Some(subs) = self.supertypes.get(&name) {
                stack.extend(subs.iter().cloned());
            }
        }
        out
    }
}
