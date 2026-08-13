use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::Path;

use anyhow::{bail, Context, Result};
use rayon::prelude::*;

use super::Lang;
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Rust;

impl Lang for Rust {
    fn name(&self) -> LangName {
        LangName::Rust
    }

    /// Top-K crates by all-time downloads from an extracted crates.io db dump.
    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_crates(db, k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        Ok((
            pkg.version.clone(),
            format!(
                "https://static.crates.io/crates/{}/{}-{}.crate",
                pkg.name, pkg.name, pkg.version
            ),
        ))
    }

    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension()?.to_str()? == "rs").then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// syn, in-process. (rustc -Zunpretty is the stricter fallback for
    /// disputed files.)
    ///
    /// An unreadable file is a hard error, not an `invalid` verdict. This is
    /// the same property PR #33 gave the six batch oracles, and rust needed
    /// it just as much while being invisible to that change: #33 enumerated
    /// `tools/`, and this oracle has no subprocess to enumerate. Measured at
    /// the time: of the twelve languages, rust was the only one that
    /// answered `invalid` for a path it could not read.
    ///
    /// Why it matters is worth restating here rather than only in the
    /// commit. `validate` is called ONLY on files the grammar already
    /// failed, and an `invalid` verdict records the file as corpus NOISE. So
    /// an oracle that cannot read its input reports every grammar failure as
    /// noise, drives gap_files to zero, and produces a flawless-looking
    /// sweep. A broken oracle must fail loudly, never quietly agree with us.
    ///
    /// Invalid UTF-8 stays a verdict, because that is a fact about the
    /// file's own bytes: Rust source is UTF-8 by definition, so a file that
    /// is not UTF-8 is not Rust. Only I/O is fatal.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        paths
            .par_iter()
            .map(|p| {
                let full = srcroot.join(p);
                let src = std::fs::read(&full).with_context(|| {
                    format!(
                        "rust oracle: cannot read {} — this is an oracle failure, \
                         not a verdict; check the corpus root",
                        full.display()
                    )
                })?;
                let valid = String::from_utf8(src)
                    .map(|text| syn::parse_file(&text).is_ok())
                    .unwrap_or(false);
                Ok((p.clone(), valid))
            })
            .collect()
    }
}

fn column(headers: &csv::StringRecord, name: &str) -> Result<usize> {
    headers
        .iter()
        .position(|h| h == name)
        .with_context(|| format!("column {name} not found in {:?}", headers))
}

fn reader(path: &Path) -> Result<csv::Reader<File>> {
    Ok(csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .with_context(|| format!("open {}", path.display()))?)
}

fn rank_crates(db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
    // 1. Top-K crate ids by all-time downloads.
    let mut rdr = reader(&db.join("crate_downloads.csv"))?;
    let h = rdr.headers()?.clone();
    let (c_id, c_dl) = (column(&h, "crate_id")?, column(&h, "downloads")?);
    let mut downloads: Vec<(u64, u64)> = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        downloads.push((rec[c_id].parse()?, rec[c_dl].parse()?));
    }
    downloads.sort_by(|a, b| b.1.cmp(&a.1));
    downloads.truncate(k);
    let wanted: HashSet<u64> = downloads.iter().map(|(id, _)| *id).collect();
    eprintln!("rank: top {} crate ids selected", downloads.len());

    // 2. Names for those ids (streams the big crates.csv).
    let mut rdr = reader(&db.join("crates.csv"))?;
    let h = rdr.headers()?.clone();
    let (c_id, c_name) = (column(&h, "id")?, column(&h, "name")?);
    let mut names: HashMap<u64, String> = HashMap::new();
    for rec in rdr.records() {
        let rec = rec?;
        let id: u64 = rec[c_id].parse()?;
        if wanted.contains(&id) {
            names.insert(id, rec[c_name].to_string());
        }
    }

    // 3. Default (latest non-yanked) version id per crate.
    let mut rdr = reader(&db.join("default_versions.csv"))?;
    let h = rdr.headers()?.clone();
    let (c_id, c_vid) = (column(&h, "crate_id")?, column(&h, "version_id")?);
    let mut default_version: HashMap<u64, u64> = HashMap::new();
    for rec in rdr.records() {
        let rec = rec?;
        let id: u64 = rec[c_id].parse()?;
        if wanted.contains(&id) {
            default_version.insert(id, rec[c_vid].parse()?);
        }
    }
    let wanted_versions: HashSet<u64> = default_version.values().copied().collect();

    // 4. Version numbers (streams the huge versions.csv).
    let mut rdr = reader(&db.join("versions.csv"))?;
    let h = rdr.headers()?.clone();
    let (c_vid, c_num) = (column(&h, "id")?, column(&h, "num")?);
    let mut version_num: HashMap<u64, String> = HashMap::new();
    for rec in rdr.records() {
        let rec = rec?;
        let vid: u64 = rec[c_vid].parse()?;
        if wanted_versions.contains(&vid) {
            version_num.insert(vid, rec[c_num].to_string());
        }
    }

    let mut ranked = Vec::new();
    for (i, (id, dl)) in downloads.iter().enumerate() {
        let (Some(name), Some(vid)) = (names.get(id), default_version.get(id)) else {
            eprintln!("rank: skipping crate id {id} (deleted or no default version)");
            continue;
        };
        let Some(num) = version_num.get(vid) else {
            eprintln!("rank: skipping {name} (default version {vid} not in versions.csv)");
            continue;
        };
        ranked.push(RankedCrate {
            rank: i + 1,
            name: name.clone(),
            version: num.clone(),
            downloads: *dl,
        });
    }
    if ranked.is_empty() {
        bail!("rank list came out empty — wrong db dir?");
    }
    Ok(ranked)
}
