//! HCL's corpus: the Terraform Registry, ranked by download count.
//!
//! **This is the strongest ranking signal in the repository, and it is
//! worth saying so plainly rather than leaving it implicit.** Zig has no
//! registry at all and ranks by GitHub stars, which `lang::github` calls
//! the weakest metric here — attention rather than use, costing nothing and
//! never decaying. The Terraform Registry publishes a real cumulative
//! download count per module, from `terraform init` runs by machines that
//! actually consumed the code. The top module has 408 million of them. A
//! module cannot be starred into this ranking.
//!
//! What that ranking is BLIND to is the more interesting half, and it is
//! not a small blindness. A published registry module is reusable library
//! code: parameterised, documented, `variable`-heavy, written to be called
//! from somewhere else. The Terraform most people write is a ROOT module —
//! the `main.tf` in an infrastructure repository that calls those modules,
//! wires up providers and backends, and is never published anywhere. Add
//! private module registries inside companies, and the population this
//! corpus can see is the minority of Terraform that is public and
//! packaged. The `blind_to` paragraph in ledger.toml says which constructs
//! that biases toward.
//!
//! Ranking and resolution use different API versions, and neither is a
//! preference. The v1 module API is the documented one and is what
//! `resolve` reads for a module's current version and its release tag, but
//! its list endpoint has no ordering — it returns modules in registry
//! order, so the first page is a handful of partner modules with five-digit
//! download counts. The v2 API is what the registry's own browse page
//! calls and takes `sort=-downloads`, which is the ordering this corpus is
//! defined by, so `rank` uses it and records that it did.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::rank::RankedCrate;
use crate::{Ecosystem, LangName};

pub struct Hcl;

const REGISTRY: &str = "https://registry.terraform.io";
const PAGE: usize = 100;

/// Where `rank` leaves each module's source repository for `resolve`, the
/// same arrangement as the GitHub and Debian indexes.
fn index_path() -> String {
    format!("corpus/{}/db/registry-index.json", LangName::Hcl)
}

#[derive(Serialize, Deserialize, Clone)]
struct Module {
    /// `namespace/name/provider`, the registry's own module address.
    full_name: String,
    /// The repository the module is published from.
    source: String,
    downloads: u64,
}

fn agent() -> ureq::Agent {
    // The same timeouts, for the same measured reason `fetch::download`
    // carries them: ureq's default agent has no read timeout, and one
    // wedged API call stops a serial fetch entirely.
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout_read(std::time::Duration::from_secs(60))
        .build()
}

fn get(url: &str) -> Result<serde_json::Value> {
    Ok(agent()
        .get(url)
        .set("User-Agent", "treebank-corpus")
        .call()
        .with_context(|| format!("GET {url}"))?
        .into_json()?)
}

/// The top `k` modules by cumulative downloads.
fn top_modules(k: usize) -> Result<Vec<Module>> {
    let mut modules = Vec::new();
    let mut page = 1;
    while modules.len() < k {
        let url = format!(
            "{REGISTRY}/v2/modules?page%5Bsize%5D={PAGE}&page%5Bnumber%5D={page}&sort=-downloads"
        );
        let doc = get(&url)?;
        let data = doc["data"]
            .as_array()
            .with_context(|| format!("{url}: no data array"))?;
        if data.is_empty() {
            break;
        }
        for entry in data {
            let attributes = &entry["attributes"];
            let (Some(full_name), Some(source)) = (
                attributes["full-name"].as_str(),
                attributes["source"].as_str(),
            ) else {
                continue;
            };
            // A module whose source is not a repository we can fetch an
            // archive from is skipped rather than guessed at. Every module
            // in the top of this ranking is on GitHub; the check is here so
            // that the day one is not, the corpus is short by one module
            // instead of wrong about one.
            if !source.starts_with("https://github.com/") {
                eprintln!("rank: skipping {full_name}: source is not a GitHub repository");
                continue;
            }
            modules.push(Module {
                full_name: full_name.to_string(),
                source: source.trim_end_matches('/').to_string(),
                downloads: attributes["downloads"].as_u64().unwrap_or(0),
            });
        }
        page += 1;
    }
    modules.truncate(k);
    anyhow::ensure!(!modules.is_empty(), "the registry returned no modules");
    Ok(modules)
}

fn index() -> Result<HashMap<String, Module>> {
    let path = index_path();
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("{path}: run `treebank rank --lang hcl` first"))?;
    let modules: Vec<Module> =
        serde_json::from_str(&text).with_context(|| format!("parse {path}"))?;
    Ok(modules
        .into_iter()
        .map(|m| (m.full_name.clone(), m))
        .collect())
}

impl Ecosystem for Hcl {
    fn name(&self) -> LangName {
        LangName::Hcl
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        let modules = top_modules(k)?;
        std::fs::create_dir_all(db)?;
        std::fs::write(
            db.join("registry-index.json"),
            serde_json::to_string_pretty(&modules)?,
        )?;
        std::fs::write(
            db.join("source.json"),
            serde_json::json!({
                "source": "terraform-registry",
                "requested_k": k,
                "ranked": modules.len(),
                "note": "registry.terraform.io, ordered by cumulative download count. \
                         A real consumption metric rather than a popularity one — and \
                         one that sees only PUBLISHED modules, so root modules and \
                         private infrastructure, which is most of the Terraform anyone \
                         writes, are outside it. See ledger.toml's blind_to.",
            })
            .to_string(),
        )?;
        Ok(modules
            .into_iter()
            .enumerate()
            .map(|(i, m)| RankedCrate {
                rank: i + 1,
                name: m.full_name,
                // Resolved at fetch time: the registry's current version for
                // a module moves, and pinning it here would put a version in
                // the ranking that the lock then contradicts.
                version: String::new(),
                downloads: m.downloads,
            })
            .collect())
    }

    /// The module's current version, and the GitHub archive of the tag it
    /// was published from.
    ///
    /// The registry's own `/download` endpoint answers with an
    /// `X-Terraform-Get` header naming a `git::` URL, which is a clone
    /// instruction rather than an archive — so the tag is taken from the
    /// module's metadata and turned into the tarball GitHub serves for it.
    /// The lock records that archive's SHA-256, so a tag that later moves is
    /// caught by hydration rather than silently fetched.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let index = index()?;
        let module = index.get(&pkg.name).with_context(|| {
            format!(
                "{}: not in {} — re-run `treebank rank --lang hcl`",
                pkg.name,
                index_path()
            )
        })?;
        let doc = get(&format!("{REGISTRY}/v1/modules/{}", pkg.name))?;
        let version = doc["version"]
            .as_str()
            .with_context(|| format!("{}: registry reports no version", pkg.name))?;
        let tag = doc["tag"].as_str().unwrap_or(version);
        let repo = module
            .source
            .strip_prefix("https://github.com/")
            .with_context(|| format!("{}: source is not a GitHub repository", pkg.name))?;
        if tag.is_empty() {
            bail!("{}: registry reports an empty release tag", pkg.name);
        }
        Ok((
            version.to_string(),
            format!("https://codeload.github.com/{repo}/tar.gz/refs/tags/{tag}"),
        ))
    }

    /// The extensions this grammar's own `tree-sitter.json` claims.
    ///
    /// `.tofu` is OpenTofu's own spelling of `.tf` and is the same syntax,
    /// so it is here for the same reason `.tf` is.
    ///
    /// The `.json` forms of all of them — `.tf.json`, `.tfvars.json`,
    /// `.tofu.json` — are deliberately absent: they are HCL's JSON PROFILE,
    /// a different concrete syntax for the same information model, and this
    /// grammar parses the native one. A JSON file admitted here would be a
    /// gap in every sweep for a construct the grammar never claimed.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        let ext = rel.extension()?.to_str()?;
        matches!(ext, "hcl" | "tf" | "tofu" | "tfvars").then_some(None)
    }

    /// A NUL means the file is not source. There is nothing else to filter:
    /// unlike bash, the extension settles what an HCL file is, and unlike
    /// javascript there is no minified build output to exclude — HCL is
    /// written, not generated.
    ///
    /// `.terraform.lock.hcl` is the one generated file that reaches this,
    /// and it is KEPT. It is real HCL that every Terraform user has in
    /// their repository, it exercises the block-with-two-labels and
    /// heredoc-free corner of the language, and excluding a file because a
    /// tool wrote it would be excluding it for a reason the grammar cannot
    /// see.
    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        let _ = rel;
        !content.contains(&0)
    }
}
