use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{node_oracle, npm, Lang};
use crate::rank::RankedCrate;

pub struct JavaScript;

impl Lang for JavaScript {
    fn name(&self) -> &'static str {
        "javascript"
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        npm::rank(k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        npm::resolve(pkg)
    }

    /// The extensions tree-sitter-javascript claims (tree-sitter.json
    /// file-types). Bundled/minified output is excluded: it is generated,
    /// it inlines other packages' code so a failure gets attributed to the
    /// wrong package, and it ships alongside the same code unminified.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        let name = rel.file_name()?.to_str()?;
        if name.ends_with(".min.js") || name.ends_with(".min.mjs") || name.ends_with(".min.cjs") {
            return None;
        }
        match rel.extension()?.to_str()? {
            "js" | "mjs" | "cjs" | "jsx" => Some(None),
            _ => None,
        }
    }

    /// One grammar: tree-sitter-javascript parses JSX too.
    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/js-oracle: V8 via `vm` (CJS wrapper / SourceTextModule, picked
    /// by Node's own module rules), with a JSX-only @babel/parser leg for
    /// the JSX that V8 rejects but this grammar parses. Deliberately NOT
    /// the TypeScript parser: ts.createSourceFile calls `const x: number =
    /// 1` valid, which would turn this grammar's correct rejection of
    /// TypeScript into a reported grammar gap.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        node_oracle::run(
            Path::new("tools/js-oracle"),
            &["--experimental-vm-modules", "--no-warnings"],
            srcroot,
            paths,
        )
    }
}
