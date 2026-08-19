use std::path::Path;

use anyhow::Result;

use crate::npm;
use crate::rank::RankedCrate;
use crate::{Ecosystem, LangName};

pub struct TypeScript;

impl Ecosystem for TypeScript {
    fn name(&self) -> LangName {
        LangName::Typescript
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
}
