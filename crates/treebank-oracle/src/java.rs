use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

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
    /// One JVM for the whole run, not one per question.
    ///
    /// Measured: 0.57s of fixed cost per launch against 1.2ms per file. The
    /// sweep never noticed, because it amortises a launch over hundreds of
    /// thousands of files. `fuzz` asks about one program at a time and then
    /// asks again at every step of shrinking, so it spent its life starting
    /// JVMs. Precompiling `Check.java` takes the launch to 0.20s and is not
    /// enough; keeping the process is.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        static ORACLE: OnceLock<Mutex<stdin_oracle::Persistent>> = OnceLock::new();
        let cell = ORACLE.get_or_init(|| {
            Mutex::new(
                stdin_oracle::Persistent::spawn(
                    "java",
                    &[crate::tool("java-oracle/Check.java")
                        .to_string_lossy()
                        .as_ref()],
                    "java tools/java-oracle/Check.java — is a JDK (not just a JRE) installed?",
                )
                .expect("start the java oracle"),
            )
        });
        let mut oracle = cell
            .lock()
            .map_err(|_| anyhow::anyhow!("java oracle poisoned"))?;
        oracle.ask(srcroot, paths)
    }
}
