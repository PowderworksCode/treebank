use std::path::Path;

use anyhow::Result;

use crate::npm;
use crate::rank::RankedCrate;
use crate::{Ecosystem, LangName};

pub struct JavaScript;

impl Ecosystem for JavaScript {
    fn name(&self) -> LangName {
        LangName::Javascript
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
}
