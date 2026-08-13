//! Hex, the BEAM's package registry — shared by elixir and erlang.
//!
//! It is not an Elixir registry and not an Erlang one: both ecosystems
//! publish to the same namespace and rank against each other. Measured over
//! the top 500 by recent downloads, 107 packages (21.4%) carry no `.ex`/
//! `.exs` file and 370 carry no `.erl`/`.hrl` file — so each language's
//! corpus is mostly the other's discards, which is the reason this module
//! exists rather than a second copy of the ranking inside `erlang.rs`.

use anyhow::{Context, Result};

use crate::rank::RankedCrate;

/// Hex publishes download counts through its own API and will sort by them,
/// so unlike PyPI (an external dataset) and LuaRocks (scraped HTML) this is
/// a first-party ranking, and unlike Java's dependent-repos proxy it is
/// traffic. `sort=recent_downloads` is the order hex.pm's own package list
/// shows, and `downloads.recent` is the figure behind it.
///
/// Two biases worth stating because they are invisible in the number. It is
/// *recent* rather than all-time, so it favours what is in use now over what
/// was downloaded once — the opposite of LuaRocks' cumulative count. And a
/// large share of Hex traffic is CI re-downloading transitive dependencies,
/// so the top of the list is weighted toward small libraries that everything
/// depends on rather than toward large applications.
pub fn rank(k: usize) -> Result<Vec<RankedCrate>> {
    // 100 per page is the API's fixed page size. The guard is far above the
    // ~180 pages the registry has today; it exists so a change in pagination
    // shape cannot turn this into an unbounded walk.
    const PER_PAGE: usize = 100;
    const MAX_PAGES: usize = 500;
    let mut ranked: Vec<RankedCrate> = Vec::new();
    for page in 1..=MAX_PAGES {
        if ranked.len() >= k {
            break;
        }
        let url = format!("https://hex.pm/api/packages?sort=recent_downloads&page={page}");
        let doc: serde_json::Value = ureq::get(&url)
            // Hex asks API clients to identify themselves; an anonymous
            // flood is what its rate limiter is for.
            .set("User-Agent", "treebank (https://treebank.dev)")
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let rows = doc
            .as_array()
            .with_context(|| format!("{url}: expected an array of packages"))?;
        if rows.is_empty() {
            eprintln!("rank: hex listing ended at page {page} ({} packages)", ranked.len());
            break;
        }
        for row in rows {
            let Some(name) = row["name"].as_str() else { continue };
            // `latest_stable_version` is null for packages that have only
            // ever published pre-releases, and falling back to
            // `latest_version` is deliberate: skipping them would silently
            // drop real packages, while a pre-release is still the author's
            // source. Both are already in this response, so neither costs a
            // request.
            let Some(version) = row["latest_stable_version"]
                .as_str()
                .or_else(|| row["latest_version"].as_str())
            else {
                continue;
            };
            let downloads = row["downloads"]["recent"].as_u64().unwrap_or(0);
            ranked.push(RankedCrate {
                rank: ranked.len() + 1,
                name: name.to_string(),
                version: version.to_string(),
                downloads,
            });
        }
        if rows.len() < PER_PAGE {
            break;
        }
    }
    anyhow::ensure!(
        !ranked.is_empty(),
        "hex.pm package listing came out empty — has the API changed?"
    );
    ranked.truncate(k);
    Ok(ranked)
}

/// Resolve a ranked package to its tarball. Pure: `rank` already has the
/// version, because Hex's listing endpoint carries `latest_stable_version`
/// per package and a second request per package would buy nothing.
///
/// Hex has no separate "sdist": every release IS source, which is why the
/// roadmap calls the corpus conventional for both BEAM languages.
pub fn resolve(pkg: &RankedCrate) -> Result<(String, String)> {
    Ok((
        pkg.version.clone(),
        format!(
            "https://repo.hex.pm/tarballs/{}-{}.tar",
            pkg.name, pkg.version
        ),
    ))
}
