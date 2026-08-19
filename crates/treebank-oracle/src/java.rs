use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Java;

impl Oracle for Java {
    fn name(&self) -> LangName {
        LangName::Java
    }

    /// javac's own parser, via `JavacTask.parse()` run through the JDK's
    /// single-file source launcher — no build step, no jar to keep in sync.
    ///
    /// `parse()` runs the parser and stops. It never attributes, so an
    /// unresolved import or a missing classpath entry is not an error and a
    /// file is judged on its own text, exactly as `ts.createSourceFile` does
    /// for TypeScript and `ast.parse` for python. Only ERROR diagnostics
    /// count; deprecation and unchecked warnings do not.
    ///
    /// The source level is the JDK's own latest, which is a decision rather
    /// than a default: a file javac rejects there is not valid modern Java.
    /// `enum`, `assert` or `_` used as an identifier is 1.4-era code, and
    /// booking it as corpus noise is the right answer under the
    /// latest-version-wins policy (DESIGN.md §4.2).
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run(
            "java",
            &[crate::tool("java-oracle/Check.java").to_string_lossy().as_ref()],
            "java tools/java-oracle/Check.java — is a JDK (not just a JRE) installed?",
            srcroot,
            paths,
        )
    }
}
