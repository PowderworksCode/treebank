//! The HCL oracle: `hclsyntax.ParseConfig`, and deliberately nothing above
//! it.
//!
//! **The reference parser is a library, not a binary, and that is the
//! finding rather than a shortcut.** `terraform fmt` and `tofu fmt` are the
//! obvious candidates and both of them gate on this same call before they
//! will touch a file — OpenTofu's `processFile` runs
//! `hclsyntax.ParseConfig` and returns its diagnostics unformatted if it
//! has errors. So the verdict a formatter would give IS this verdict, and
//! taking it directly costs a ~100 MB binary less.
//!
//! It also settles the licence question that a Terraform-shaped dependency
//! would otherwise raise. HashiCorp moved Terraform to BUSL-1.1 in August
//! 2023 (1.5.7 is the last MPL release, and IBM's acquisition did not
//! change the terms), while the `hcl` library stayed MPL-2.0 along with the
//! rest of HashiCorp's libraries. Nothing under BUSL is pinned anywhere in
//! this repository. The one thing the library cannot do is FORMAT —
//! `tofu fmt` is `hclwrite.ParseConfig` followed by OpenTofu's own
//! `formatBody` walk, and those rules live in Terraform and its fork rather
//! than in HCL — so the `reformat` capability pins OpenTofu, which is the
//! MPL-2.0 implementation of them. See ledger.toml.
//!
//! **This is the syntactic question and only the syntactic question.**
//! `terraform validate` resolves providers, modules and types and would
//! need a `terraform init` to say anything at all; it answers whether a
//! configuration is well FORMED, which is not what a grammar is measured
//! against. `hclsyntax.ParseConfig` reads one file's bytes, follows no
//! `source`, and knows nothing about `aws_instance`. The boundary is the
//! library's, not our discipline's.
//!
//! What it does check beyond pure syntax is duplicate attribute names
//! within one body — `a = 1` twice is "Attribute redefined" — which no
//! parse table can express. That is a widening on our side rather than a
//! gap, so the sweep cannot see it; it is declared in ledger.toml and has
//! its own negative fixture.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{LangName, Oracle};

pub struct Hcl;

impl Oracle for Hcl {
    fn name(&self) -> LangName {
        LangName::Hcl
    }

    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let oracle = ensure_oracle()?;
        crate::stdin_oracle::run(
            oracle.to_string_lossy().as_ref(),
            &[],
            "spawn hcl-oracle — run tools/hcl-oracle/build.sh (needs go)",
            srcroot,
            paths,
        )
    }
}

/// Build the Go program on first use, and rebuild it when its source or its
/// pin moves. The same arrangement as the C oracle's libclang build and the
/// node oracles' `npm ci`: the toolchain is a dependency of the gate, and
/// the gate says so out loud rather than failing with a missing file.
pub(crate) fn ensure_oracle() -> Result<PathBuf> {
    let oracle = crate::tool("hcl-oracle/hcl-oracle");
    let build = crate::tool("hcl-oracle/build.sh");
    let inputs = [
        crate::tool("hcl-oracle/check.go"),
        crate::tool("hcl-oracle/spans.go"),
        crate::tool("hcl-oracle/go.mod"),
        crate::tool("hcl-oracle/go.sum"),
        build.clone(),
    ];

    let stale = || -> Result<bool> {
        if !oracle.exists() {
            return Ok(true);
        }
        let built = oracle.metadata()?.modified()?;
        for input in &inputs {
            if input.metadata()?.modified()? > built {
                return Ok(true);
            }
        }
        Ok(false)
    };

    if stale()? {
        eprintln!("oracle: building {} (hashicorp/hcl)", build.display());
        let ok = std::process::Command::new("bash")
            .arg(&build)
            .status()
            .with_context(|| format!("run {}", build.display()))?
            .success();
        anyhow::ensure!(ok, "hcl-oracle build failed: {}", build.display());
    }
    Ok(oracle)
}
