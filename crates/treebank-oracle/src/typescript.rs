use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct TypeScript;

impl Oracle for TypeScript {
    fn name(&self) -> LangName {
        LangName::Typescript
    }

    /// tools/ts-oracle: ts.createSourceFile parseDiagnostics — syntax-only,
    /// and .d.ts-safe (ts.transpileModule throws on declaration files).
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run_node(&crate::tool("ts-oracle"), &[], srcroot, paths)
    }
}
