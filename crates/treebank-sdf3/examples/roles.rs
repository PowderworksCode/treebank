//! `cargo run -p treebank-sdf3 --example roles -- spike/pyish`
//!
//! Hold a spike's generated `src/node-types.json` and lowered `roles.json`
//! to `treebank::check::check`: the same code `treebank roles` runs over
//! every shipped grammar in CI. Prints the summary the CLI prints, or the
//! findings, and fails on any.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let dir: PathBuf = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: roles <spike dir>"))?
        .into();
    let vocab = treebank::vocabulary();
    let nt = treebank::node_types::NodeTypes::load(&dir.join("src/node-types.json"))?;
    let roles = treebank::roles::RolesManifest::load(&dir.join("roles.json"))?;
    let findings = treebank::check::check(&nt, &roles, vocab);
    if !findings.is_empty() {
        for f in &findings {
            eprintln!("roles: {f}");
        }
        anyhow::bail!("{} roles finding(s)", findings.len());
    }
    let table: Vec<&String> = nt
        .supertypes
        .keys()
        .filter(|s| vocab.is_table_term(s))
        .collect();
    println!(
        "roles: {} of {} table-tier terms as supertypes [{}], {} facet(s), {} named node(s), {} uncategorised (vocabulary {})",
        table.len(),
        vocab.table.len(),
        table.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "),
        roles.facets.len(),
        nt.named.len() - nt.supertypes.len(),
        roles.uncategorised.len(),
        vocab.version,
    );
    Ok(())
}
