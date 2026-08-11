use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::Lang;
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Go;

const PROXY: &str = "https://proxy.golang.org";

impl Lang for Go {
    fn name(&self) -> LangName {
        LangName::Go
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_go(k)
    }

    /// The module proxy, which is the cleanest corpus source treebank has.
    ///
    /// `@latest` names the version, `@v/<version>.zip` serves it. The zip is
    /// **immutable and content-addressed**: the proxy also publishes a
    /// `.info`, a `.mod` and a checksum that the sumdb notarises, and a
    /// version can never be re-uploaded or deleted. Compare what the other
    /// ecosystems make us do — crates.io tarballs need a root directory
    /// stripped, Maven needs a conventionally-named sources jar that may not
    /// exist, PyPI needs an sdist picked out of a list of wheels, Debian
    /// needs three decompressors. Here there is one URL per version and it
    /// always means the same bytes.
    ///
    /// The one wrinkle is case-encoding, and it is not optional: the proxy
    /// serves from case-insensitive filesystems, so every uppercase letter
    /// in the module path and version is escaped to `!` + its lowercase.
    /// Unescaped, `github.com/BurntSushi/toml` is a 404, measured.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let module = escape(&pkg.name);
        let url = format!("{PROXY}/{module}/@latest");
        let doc: serde_json::Value = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let version = doc["Version"]
            .as_str()
            .with_context(|| format!("{}: no Version in @latest", pkg.name))?
            .to_string();
        let zip = format!("{PROXY}/{module}/@v/{}.zip", escape(&version));
        Ok((version, zip))
    }

    /// A module zip prefixes every entry with `<module>@<version>/`, which is
    /// as many path components as the module path has — three for
    /// `github.com/spf13/cobra@v1.10.2/`. A module path may not contain `@`
    /// (the proxy spec reserves it), so the first component that does is
    /// exactly the end of the prefix.
    fn archive_strip(&self, entry: &Path, _is_zip: bool) -> usize {
        entry
            .components()
            .position(|c| c.as_os_str().to_string_lossy().contains('@'))
            .map_or(0, |i| i + 1)
    }

    /// `.go`, minus the trees Go's own toolchain does not compile.
    ///
    /// `go/build` ignores any directory or file whose name begins with `_`
    /// or `.`, and ignores `testdata/` entirely. Those are not treebank
    /// heuristics, they are the language's own rule, and following it keeps
    /// the corpus to files that are actually built. `testdata/` matters most:
    /// it is where a Go project keeps its deliberately-broken fixtures, so
    /// admitting it would pour known-invalid files into the corpus and
    /// inflate the noise count with files no one ever compiles.
    ///
    /// `vendor/` goes for the reason python excludes `_vendor/` and
    /// javascript excludes bundles: it is a verbatim copy of other modules,
    /// so a failure there is attributed to the wrong package, and the same
    /// code is already in the corpus under the module that owns it.
    ///
    /// `_test.go` files stay. They are ordinary Go, compiled by `go test`,
    /// and they carry syntax the rest of a package often does not.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        for c in rel.components() {
            let name = c.as_os_str().to_str()?;
            if matches!(name, "vendor" | "testdata")
                || name.starts_with('_')
                || name.starts_with('.')
            {
                return None;
            }
        }
        (rel.extension()?.to_str()? == "go").then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/go-oracle: `go/parser.ParseFile` with `SkipObjectResolution`,
    /// the parser the Go toolchain runs. Parse-only — no type check, no
    /// import resolution, no package assembly — so an unresolved identifier
    /// is not an error and each file is judged on its own text. Verified
    /// rather than assumed: 8144 corpus and stdlib files returned the same
    /// 8144 valid verdicts after being copied into one flat directory with
    /// mangled names and no `go.mod`. The toolchain version is the language
    /// version and is recorded in ledger.json; see the note there.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let oracle = Path::new("tools/go-oracle/go-oracle");
        if !oracle.exists() {
            eprintln!("oracle: building tools/go-oracle");
            let ok = Command::new("tools/go-oracle/build.sh")
                .status()
                .context("run tools/go-oracle/build.sh — run from the repo root")?
                .success();
            anyhow::ensure!(ok, "tools/go-oracle/build.sh failed");
        }
        super::stdin_oracle::run(
            &oracle.to_string_lossy(),
            &[],
            "spawn tools/go-oracle/go-oracle",
            srcroot,
            paths,
        )
    }
}

/// Module-proxy case-encoding: every uppercase letter becomes `!` plus its
/// lowercase, so the proxy can serve from a case-insensitive filesystem
/// without two modules colliding.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            out.push('!');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Go publishes no download counts anywhere — not on the proxy, which
/// deliberately does not release its logs, and not on pkg.go.dev, whose
/// "known importers" listing is capped (15 for `stretchr/testify`, measured,
/// against a real dependent count in the tens of thousands). deps.dev has
/// the number but its documented API says dependent counts cover "npm,
/// Cargo, Maven and PyPI" — Go is not on that list and the endpoint 404s.
///
/// So Go's metric is the same *kind* as Java's rather than the same kind as
/// crates.io/npm/PyPI: a dependency-graph proxy, not traffic, from the same
/// source Java already uses. The ledger says so.
///
/// One property of this metric is specific to Go and worth knowing when
/// reading the list: a `go.mod` records the whole transitive requirement
/// set, not just direct imports, so modules that are nobody's direct
/// dependency ride high. `github.com/pmezard/go-difflib` sits at #2 because
/// `testify` depends on it. Those are still real, widely compiled Go
/// modules, so they are legitimate corpus — the ranking just should not be
/// read as "what Go programmers import".
fn rank_go(k: usize) -> Result<Vec<RankedCrate>> {
    const PER_PAGE: usize = 100;
    let mut ranked = Vec::new();
    let mut page = 1;
    while ranked.len() < k {
        let url = format!(
            "https://packages.ecosyste.ms/api/v1/registries/proxy.golang.org/packages\
             ?sort=dependent_repos_count&order=desc&per_page={PER_PAGE}&page={page}"
        );
        let batch: Vec<serde_json::Value> = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        if batch.is_empty() {
            break;
        }
        eprintln!("rank: ecosyste.ms go page {page} ({} modules)", batch.len());
        for entry in batch {
            let (Some(name), Some(dependents)) = (
                entry["name"].as_str(),
                entry["dependent_repos_count"].as_u64(),
            ) else {
                continue;
            };
            ranked.push(RankedCrate {
                rank: ranked.len() + 1,
                name: name.to_string(),
                // Resolved at fetch time from the proxy, like java and
                // python: ecosyste.ms' latest_release_number is null for
                // some modules (github.com/golang/protobuf, measured) and
                // the proxy is the authority either way.
                version: String::new(),
                downloads: dependents,
            });
            if ranked.len() == k {
                break;
            }
        }
        page += 1;
    }
    if ranked.is_empty() {
        bail!("go rank list came out empty");
    }
    Ok(ranked)
}
