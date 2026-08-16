use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::Ecosystem;

#[derive(Serialize, Deserialize, Clone)]
pub struct RankedCrate {
    pub rank: usize,
    pub name: String,
    /// Empty when the ecosystem resolves versions at fetch time.
    pub version: String,
    pub downloads: u64,
}

pub fn run(eco: &dyn Ecosystem, db: &Path, k: usize, out: &Path) -> Result<()> {
    let ranked = eco.rank(db, k)?;
    std::fs::create_dir_all(out.parent().unwrap())?;
    std::fs::write(out, serde_json::to_string_pretty(&ranked)?)?;
    println!(
        "rank: wrote {} {} packages to {} (top: {})",
        ranked.len(),
        eco.name(),
        out.display(),
        ranked[0].name
    );
    Ok(())
}
