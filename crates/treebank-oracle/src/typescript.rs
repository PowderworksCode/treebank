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
    /// One node process for the whole run. `fuzz` asks about a single
    /// program at a time and again at every shrink step, and node's startup
    /// plus loading the TypeScript compiler is far larger than a parse:
    /// 200 fuzz iterations took 99 seconds through a process per question.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let script = crate::tool("ts-oracle").join("check.mjs");
        stdin_oracle::persistent(
            "ts",
            "node",
            &[script.to_string_lossy().as_ref()],
            "node tools/ts-oracle/check.mjs — is node installed?",
            srcroot,
            paths,
        )
    }
}
