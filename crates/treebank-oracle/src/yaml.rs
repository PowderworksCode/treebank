use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Yaml;

/// One leg of the union, as a node script under `tools/yaml-oracle`.
///
/// `stdin_oracle::run_node` hardcodes `check.mjs`, so the 1.1 leg goes
/// through `node_lines` and the verdicts are read here. Same protocol,
/// same process handling; only the script name differs.
fn node_leg(script: &str, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
    let lines = stdin_oracle::node_lines(&crate::tool("yaml-oracle"), script, &[], srcroot, paths)?;
    let mut verdicts = HashMap::new();
    for line in &lines {
        if let Some((path, verdict)) = line.rsplit_once('\t') {
            verdicts.insert(stdin_oracle::relativize(path, srcroot), verdict == "valid");
        }
    }
    Ok(verdicts)
}

impl Oracle for Yaml {
    fn name(&self) -> LangName {
        LangName::Yaml
    }

    /// YAML has no owning implementation. The spec is the owner, every
    /// implementation disagrees with it somewhere, and they disagree with
    /// each other — so "valid YAML" is not a question one program can
    /// answer and the oracle is a union of three legs. A file is valid if
    /// ANY of them accepts it, which is the conservative direction: a more
    /// permissive oracle books MORE of our failures as gaps, never fewer.
    ///
    /// 1. `yaml` 2.9.0 at version 1.2. The reference-grade processor and
    ///    the one every file is judged by first.
    /// 2. The same parser at version 1.1. This is the version union
    ///    (DESIGN.md §4.2) and it is here to be MEASURED rather than
    ///    assumed: it is the only implementation that can be asked both
    ///    questions with everything else held constant, so the count of
    ///    files it rescues is the size of YAML's version union as a grammar
    ///    can see it. Over yaml-test-suite that count is one case in 406.
    /// 3. PyYAML through `yaml.parse`, which is the libyaml lineage — the
    ///    state machine that PyYAML, the C bindings, Ruby's Psych and Go's
    ///    yaml packages all descend from. This is NOT a version; it is the
    ///    other half of the installed base, and it is the leg that matters
    ///    most in practice. A file that the whole Python and Ruby world
    ///    reads without complaint is a file this grammar had better parse,
    ///    whatever the spec says about it.
    ///
    /// The ledger records what each leg rescues over the real corpus, so
    /// the third leg has to earn its place rather than be argued for.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let mut verdicts = node_leg("check.mjs", srcroot, paths)?;

        let rejected = |v: &HashMap<String, bool>| -> Vec<String> {
            paths
                .iter()
                .filter(|p| v.get(*p).copied() == Some(false))
                .cloned()
                .collect()
        };

        let legacy = rejected(&verdicts);
        if legacy.is_empty() {
            return Ok(verdicts);
        }
        for (path, valid) in node_leg("check11.mjs", srcroot, &legacy)? {
            if valid {
                verdicts.insert(path, true);
            }
        }

        let still = rejected(&verdicts);
        if still.is_empty() {
            return Ok(verdicts);
        }
        let libyaml = stdin_oracle::run(
            "python3",
            &[crate::tool("yaml-oracle/check.py")
                .to_string_lossy()
                .as_ref()],
            "python3 tools/yaml-oracle/check.py — PyYAML is REQUIRED for the \
             libyaml half of the oracle; without it every file the spec-first \
             parser rejects and the installed base reads is booked as noise",
            srcroot,
            &still,
        )?;
        for (path, valid) in libyaml {
            if valid {
                verdicts.insert(path, true);
            }
        }
        Ok(verdicts)
    }

    /// The 1.2 leg alone: what is still valid under the current version of
    /// the specification, with no help from an older version and none from
    /// an implementation that predates it.
    fn validate_current(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        node_leg("check.mjs", srcroot, paths)
    }
}
