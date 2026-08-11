//! What a `ledger.json` is, in one place.
//!
//! The ledger is the grammar contract: it pins upstream, names the patches
//! that are our entire divergence from it, and fixes the CLI that generates
//! the parser. Four shell scripts read it with `jq` — materialize, verify,
//! check, daily — and until now nothing said what a valid one looked like.
//!
//! That is not a hypothetical gap. `daily.sh` and `check.sh` feed the
//! `grammar` field straight to `--lang`, and treebank-csharp's ledger said
//! `"c-sharp"` where the CLI knows `csharp`. Every automated path for that
//! grammar failed, the daily job reported it as "no package list" — which
//! sounded like missing corpus data rather than a typo — and it sat that way
//! for days. A string that must be one of five values should not be a string.
//!
//! So: `LangName` is an enum, serde rejects anything else when the ledger is
//! read, clap rejects anything else on the command line, and `treebank
//! ledger` checks the rest of the file's invariants. CI runs it per grammar
//! via verify.sh, so a ledger that does not describe reality cannot land.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// The canonical name of a supported language. This is the only place the
/// spelling is decided: it is what `--lang` accepts, what a ledger's
/// `grammar` field must hold, and what `corpus/<lang>/` is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, clap::ValueEnum)]
pub enum LangName {
    #[serde(rename = "rust")]
    #[value(name = "rust")]
    Rust,
    #[serde(rename = "typescript")]
    #[value(name = "typescript")]
    Typescript,
    #[serde(rename = "javascript")]
    #[value(name = "javascript")]
    Javascript,
    #[serde(rename = "java")]
    #[value(name = "java")]
    Java,
    #[serde(rename = "csharp")]
    #[value(name = "csharp")]
    Csharp,
    #[serde(rename = "c")]
    #[value(name = "c")]
    C,
    #[serde(rename = "python")]
    #[value(name = "python")]
    Python,
    #[serde(rename = "php")]
    #[value(name = "php")]
    Php,
    #[serde(rename = "go")]
    #[value(name = "go")]
    Go,
    #[serde(rename = "bash")]
    #[value(name = "bash")]
    Bash,
}

impl LangName {
    pub fn as_str(self) -> &'static str {
        match self {
            LangName::Rust => "rust",
            LangName::Typescript => "typescript",
            LangName::Javascript => "javascript",
            LangName::Java => "java",
            LangName::Csharp => "csharp",
            LangName::C => "c",
            LangName::Python => "python",
            LangName::Php => "php",
            LangName::Go => "go",
            LangName::Bash => "bash",
        }
    }
}

impl std::fmt::Display for LangName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Deserialize)]
pub struct Upstream {
    pub git_url: String,
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct Patch {
    pub id: usize,
    pub title: String,
    /// Repo-relative, e.g. `patches/0003-tilde-in-token-trees.patch`.
    pub file: String,
}

/// Only the load-bearing fields are modelled. The descriptive ones —
/// `*_note`, `origin`, `evidence`, `corpus` — are prose for humans and are
/// deliberately not constrained here.
#[derive(Debug, Deserialize)]
pub struct Ledger {
    pub grammar: LangName,
    pub upstream: Upstream,
    pub generate_cli: String,
    #[serde(default)]
    pub generate_dirs: Option<Vec<String>>,
    #[serde(default)]
    pub patches: Vec<Patch>,
}

impl Ledger {
    pub fn generate_dirs(&self) -> Vec<String> {
        self.generate_dirs
            .clone()
            .unwrap_or_else(|| vec![".".to_string()])
    }
}

pub fn load(grammar_dir: &Path) -> Result<Ledger> {
    let path = grammar_dir.join("ledger.json");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// Check one grammar's ledger against the tree it describes. Returns every
/// problem found rather than the first, so a broken ledger takes one pass to
/// fix rather than five.
pub fn check(grammar_dir: &Path) -> Result<Vec<String>> {
    let mut bad = Vec::new();
    let led = load(grammar_dir)?;

    // The directory and the language name have to agree, because daily.sh
    // resolves one from the other in both directions.
    let dir_name = grammar_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let want_dir = format!("treebank-{}", led.grammar);
    if dir_name != want_dir {
        bad.push(format!(
            "grammar {} lives in {dir_name}/, expected {want_dir}/",
            led.grammar
        ));
    }

    if led.upstream.sha.len() != 40 || !led.upstream.sha.chars().all(|c| c.is_ascii_hexdigit()) {
        bad.push(format!(
            "upstream.sha {:?} is not a full 40-character commit sha",
            led.upstream.sha
        ));
    }
    // The ledger names the upstream it pins, and .gitmodules names the one
    // git will actually clone. If those disagree, the ledger is describing a
    // repository nobody is fetching.
    let upstream_dir = grammar_dir.join("upstream");
    if upstream_dir.join(".git").exists() {
        if let Ok(out) = std::process::Command::new("git")
            .args(["-C", &upstream_dir.to_string_lossy(), "remote", "get-url", "origin"])
            .output()
        {
            let actual = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let norm = |u: &str| u.trim_end_matches('/').trim_end_matches(".git").to_string();
            if out.status.success() && !actual.is_empty() && norm(&actual) != norm(&led.upstream.git_url) {
                bad.push(format!(
                    "upstream.git_url is {:?} but the submodule points at {:?}",
                    led.upstream.git_url, actual
                ));
            }
        }
    }

    if led.generate_cli.is_empty() {
        bad.push("generate_cli is empty — the CLI version must be pinned".to_string());
    }

    // Patches are applied in ledger order and numbered by filename, so the
    // two must not be able to disagree: a patch on disk that no ledger entry
    // names would be applied by materialize.sh (it globs patches/*.patch)
    // while being invisible to anyone reading the ledger.
    let mut on_disk = BTreeSet::new();
    let patch_dir = grammar_dir.join("patches");
    if patch_dir.is_dir() {
        for entry in std::fs::read_dir(&patch_dir)? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if name.ends_with(".patch") {
                on_disk.insert(format!("patches/{name}"));
            }
        }
    }
    let mut in_ledger = BTreeSet::new();
    for (i, p) in led.patches.iter().enumerate() {
        let want_id = i + 1;
        if p.id != want_id {
            bad.push(format!(
                "patch {:?} has id {} but is entry {want_id}; ids must be consecutive from 1",
                p.title, p.id
            ));
        }
        let want_prefix = format!("patches/{:04}-", p.id);
        if !p.file.starts_with(&want_prefix) {
            bad.push(format!(
                "patch id {} names {:?}, which does not start with {want_prefix}",
                p.id, p.file
            ));
        }
        if !grammar_dir.join(&p.file).exists() {
            bad.push(format!("patch id {} names {:?}, which is missing", p.id, p.file));
        }
        in_ledger.insert(p.file.clone());
    }
    for orphan in on_disk.difference(&in_ledger) {
        bad.push(format!("{orphan} is on disk but no ledger entry names it"));
    }

    // generate_dirs are relative to the materialized tree; check them against
    // upstream/ when the submodule is present, and say nothing when it is not
    // (a fresh clone has not run `git submodule update` yet).
    let upstream = grammar_dir.join("upstream");
    if upstream.join(".git").exists() {
        for d in led.generate_dirs() {
            if !upstream.join(&d).is_dir() {
                bad.push(format!("generate_dirs names {d:?}, which is not in upstream/"));
            }
        }
    }
    Ok(bad)
}

/// Check one grammar, or every grammar under `crates/` when none is given.
pub fn run(grammar_dir: Option<&Path>) -> Result<()> {
    let dirs: Vec<std::path::PathBuf> = match grammar_dir {
        Some(d) => vec![d.to_path_buf()],
        None => {
            let mut found: Vec<_> = std::fs::read_dir("crates")
                .context("read crates/ — run from the repo root, or name a grammar dir")?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.join("ledger.json").is_file())
                .collect();
            found.sort();
            found
        }
    };
    if dirs.is_empty() {
        bail!("no grammar ledgers found");
    }
    let mut total = 0;
    for dir in &dirs {
        let problems = check(dir)?;
        total += problems.len();
        if problems.is_empty() {
            println!("ledger: {} ok", dir.display());
        } else {
            println!("ledger: {} — {} problem(s)", dir.display(), problems.len());
            for p in problems {
                println!("   {p}");
            }
        }
    }
    if total > 0 {
        bail!("{total} ledger problem(s) across {} grammar(s)", dirs.len());
    }
    Ok(())
}
