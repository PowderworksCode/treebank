use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::lang::Lang;

#[derive(Serialize, Deserialize, Clone)]
pub struct RankedCrate {
    pub rank: usize,
    pub name: String,
    /// Empty when the ecosystem resolves versions at fetch time.
    pub version: String,
    pub downloads: u64,
}

pub fn run(lang: &dyn Lang, db: &Path, k: usize, out: &Path) -> Result<()> {
    let ranked = lang.rank(db, k)?;
    std::fs::create_dir_all(out.parent().unwrap())?;
    std::fs::write(out, serde_json::to_string_pretty(&ranked)?)?;
    println!(
        "rank: wrote {} {} packages to {} (top: {})",
        ranked.len(),
        lang.name(),
        out.display(),
        ranked[0].name
    );
    Ok(())
}
