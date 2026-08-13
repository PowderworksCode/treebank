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
    #[serde(rename = "zig")]
    #[value(name = "zig")]
    Zig,
    #[serde(rename = "lua")]
    #[value(name = "lua")]
    Lua,
    #[serde(rename = "ruby")]
    #[value(name = "ruby")]
    Ruby,
    #[serde(rename = "elixir")]
    #[value(name = "elixir")]
    Elixir,
    #[serde(rename = "yaml")]
    #[value(name = "yaml")]
    Yaml,
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
            LangName::Zig => "zig",
            LangName::Lua => "lua",
            LangName::Ruby => "ruby",
            LangName::Elixir => "elixir",
            LangName::Yaml => "yaml",
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

/// The reference parser a sweep's verdicts were produced with.
///
/// This is `generate_cli` for the oracle, and it is load-bearing for exactly
/// the same reason. `generate_cli` exists because regenerating with a
/// different tree-sitter-cli silently changes what the grammar accepts; this
/// exists because running a different reference parser silently changes what
/// "invalid" means, and a sweep's gap/noise split is only interpretable
/// against the parser that produced it.
///
/// Lua is the cheapest language that forces the point — 5.1, 5.2, 5.3, 5.4,
/// LuaJIT and Luau are genuinely different syntaxes, `goto` is 5.2+ and
/// integer division is 5.3+, so which `luac` is installed decides verdicts —
/// but it is not the first language to need it. C's answer is only meaningful
/// as "libclang 20.1.2, given `-std=gnu17`", and Scala (2 vs 3), Haskell
/// (per-package `LANGUAGE` pragmas) and Zig (a moving target) all need it
/// later. Hence `flags`: the dialect is not always a version number.
#[derive(Debug, Deserialize)]
pub struct Oracle {
    /// What runs, concretely enough to re-run — e.g. `tools/lua-oracle/check.lua`.
    pub tool: String,
    /// The exact version whose verdicts the ledger's sweep numbers describe.
    pub version: String,
    /// The language dialect that version implies, named in the language's own
    /// terms: "PUC-Rio Lua 5.4", "GNU C17", "Python 3.12".
    pub dialect: String,
    /// Flags that select the dialect where a version alone does not, e.g.
    /// C's `-std=gnu17`. Empty when the tool's version settles it.
    #[serde(default)]
    pub flags: Vec<String>,

    /// Whether this oracle's verdicts are a **fact** or a **choice**.
    ///
    /// `reference` — the default, and true of every grammar before yaml —
    /// means the language has an implementation everyone appeals to: rust's
    /// `syn`, CPython, `php -l`, PUC-Rio Lua, `std.zig.Ast`. Disagreement
    /// there is a bug in something.
    ///
    /// `position` means it does not, and the choice is ours. YAML is the
    /// first: measured over the official conformance suite, libyaml scores
    /// 83.3%, go-yaml 81.8%, eemeli `yaml` 99.3% and js-yaml 100%, and they
    /// disagree with each other on 67 of 402 cases. A reader has to be able
    /// to tell those two situations apart in one word, because it decides
    /// whether `gap_files` is a measurement or a measurement-relative-to-us.
    #[serde(default)]
    pub authority: Authority,

    /// The specification revision the tool implements — "YAML 1.2.2",
    /// "TOML 1.0.0" — which is NOT the tool's version and cannot be derived
    /// from it. Optional because most languages have one live revision.
    #[serde(default)]
    pub spec: Option<String>,

    /// Which stage of the parser is called: `parse`, `compose`, `load`,
    /// `parse-to-table`. Optional, but fill it even when the stages agree:
    /// yaml measured 73 of 3217 real files changing verdict between one
    /// parser's parse and load stages, against 0 between two different
    /// parsers at the same stage, and python's `ast.parse`-vs-`compile`
    /// finding was the same axis. A field that records "checked, and it did
    /// not matter" is worth having.
    #[serde(default)]
    pub stage: Option<String>,

    /// Alternatives that were **measured** against, tools and stages alike.
    /// Required non-empty when `authority` is `position`, allowed always: a
    /// rejected candidate with a one-line excuse and no numbers is exactly
    /// the evidence that evaporates.
    #[serde(default)]
    pub considered: Vec<serde_json::Value>,

    /// Required when `authority` is `position`: why this one, and what the
    /// verdicts are relative to. Prose, so it is not modelled further.
    #[serde(default)]
    pub position: Option<serde_json::Value>,
}

/// See `Oracle::authority`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Authority {
    #[default]
    Reference,
    Position,
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
    /// Absent on grammars that predate the field; `check` reports it as a
    /// gap rather than a hard error so adding it stays a per-grammar change
    /// with its own evidence, exactly like adopting a new patch.
    #[serde(default)]
    pub oracle: Option<Oracle>,
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

    // An oracle field that exists must be complete: a half-filled one is
    // worse than none, because it reads as a recorded dialect while leaving
    // the load-bearing part blank.
    match &led.oracle {
        None => bad.push(
            "no oracle field — the reference parser's tool/version/dialect must be recorded \
             alongside the verdicts it produced (see crates/treebank-lua/ledger.json)"
                .to_string(),
        ),
        Some(o) => {
            for (field, value) in [
                ("tool", &o.tool),
                ("version", &o.version),
                ("dialect", &o.dialect),
            ] {
                if value.trim().is_empty() {
                    bad.push(format!("oracle.{field} is empty"));
                }
            }
            // An oracle declared a POSITION owes evidence, because that is
            // the entire difference between the two values: "we chose this"
            // with nothing measured against is indistinguishable from "we
            // used whatever was installed". See yaml's ledger, which is the
            // first to set it.
            if o.authority == Authority::Position {
                if o.considered.is_empty() {
                    bad.push(
                        "oracle.authority is \"position\" but oracle.considered is empty — a \
                         disputed oracle must record the alternatives it was measured against"
                            .to_string(),
                    );
                }
                if o.position.is_none() {
                    bad.push(
                        "oracle.authority is \"position\" but oracle.position is missing — say \
                         why this one and what the verdicts are relative to"
                            .to_string(),
                    );
                }
            }
        }
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
            let led = load(dir)?;
            // The pinned dialect belongs on CI's stdout next to the grammar
            // it judges: a sweep number quoted without the reference parser
            // that produced it is not a claim this repo makes.
            let oracle = led
                .oracle
                .map(|o| {
                    let flags = if o.flags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", o.flags.join(" "))
                    };
                    // `position` is printed and `reference` is not, so the
                    // line stays as it was for the eleven grammars whose
                    // oracle is nobody's opinion, and says so loudly for the
                    // ones where "invalid" is a choice this repo made. The
                    // spec revision and the stage ride along with it because
                    // together they are what the verdicts are relative to.
                    let stance = match o.authority {
                        Authority::Reference => String::new(),
                        Authority::Position => {
                            let mut parts = vec!["POSITION".to_string()];
                            if let Some(spec) = &o.spec {
                                parts.push(format!("spec {spec}"));
                            }
                            if let Some(stage) = &o.stage {
                                parts.push(format!("stage {stage}"));
                            }
                            format!(" [{}]", parts.join(", "))
                        }
                    };
                    format!(" — oracle {} ({}){flags}{stance}", o.version, o.dialect)
                })
                .unwrap_or_default();
            println!("ledger: {} ok{oracle}", dir.display());
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
