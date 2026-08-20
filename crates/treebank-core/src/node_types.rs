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
    #[serde(default)]
    fields: BTreeMap<String, serde_json::Value>,
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
    /// Node type -> field name -> the types that field may hold. Facet
    /// expansion uses this to drop members a field-constrained pattern
    /// cannot match: `(_declaration body: (_body))` must not conjure a
    /// `body` onto `import_alias`, nor keep `class_definition` whose body
    /// is a `class_body` no `_body` subtype covers -- either way the
    /// expanded query is an impossible pattern and refuses to compile.
    /// This mirrors what tree-sitter itself checks for a NATIVE supertype
    /// pattern, where any one subtype satisfying the constraint keeps it.
    pub fields: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
}

impl NodeTypes {
    pub fn parse(json: &str) -> Result<NodeTypes> {
        let raw: Vec<RawEntry> = serde_json::from_str(json).context("parse node-types.json")?;
        let mut named = BTreeSet::new();
        let mut supertypes = BTreeMap::new();
        let mut fields: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
        for e in &raw {
            if !e.named {
                continue;
            }
            named.insert(e.type_name.clone());
            let mut per_field: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for (fname, fval) in &e.fields {
                let mut types = BTreeSet::new();
                if let Some(ts) = fval.get("types").and_then(|v| v.as_array()) {
                    for t in ts {
                        if let Some(n) = t.get("type").and_then(|v| v.as_str()) {
                            types.insert(n.to_string());
                        }
                    }
                }
                per_field.insert(fname.clone(), types);
            }
            fields.insert(e.type_name.clone(), per_field);
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
        Ok(NodeTypes { named, supertypes, fields })
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
