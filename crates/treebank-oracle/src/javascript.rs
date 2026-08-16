use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct JavaScript;

impl Oracle for JavaScript {
    fn name(&self) -> LangName {
        LangName::Javascript
    }

    /// tools/js-oracle: V8 via `vm` (CJS wrapper / SourceTextModule, picked
    /// by Node's own module rules), with a JSX-only @babel/parser leg for
    /// the JSX that V8 rejects but this grammar parses. Deliberately NOT
    /// the TypeScript parser: ts.createSourceFile calls `const x: number =
    /// 1` valid, which would turn this grammar's correct rejection of
    /// TypeScript into a reported grammar gap.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run_node(
            &crate::tool("js-oracle"),
            &["--experimental-vm-modules", "--no-warnings"],
            srcroot,
            paths,
        )
    }
}
