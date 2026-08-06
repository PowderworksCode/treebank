use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{node_oracle, npm, Lang};
use crate::rank::RankedCrate;

pub struct TypeScript;

impl Lang for TypeScript {
    fn name(&self) -> &'static str {
        "typescript"
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        npm::rank(k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        npm::resolve(pkg)
    }

    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        match rel.extension()?.to_str()? {
            "tsx" => Some(Some("tsx".into())),
            "ts" | "mts" | "cts" => Some(None),
            _ => None,
        }
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["typescript", "tsx"]
    }

    fn route(&self, dialect: &Option<String>, rel: &str) -> usize {
        let is_tsx = dialect
            .as_deref()
            .map(|d| d == "tsx")
            .unwrap_or_else(|| rel.ends_with(".tsx"));
        usize::from(is_tsx)
    }

    /// tools/ts-oracle: ts.createSourceFile parseDiagnostics — syntax-only,
    /// and .d.ts-safe (ts.transpileModule throws on declaration files).
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        node_oracle::run(Path::new("tools/ts-oracle"), &[], srcroot, paths)
    }
}
