//! `cargo run -p treebank-sdf3 --example lower -- spike/mini/mini.sdf3`
//!
//! Reads the module, lowers it, and writes `grammar.json`, `grammar.js` and
//! `findings.md` beside it.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let path: PathBuf = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: lower <module.sdf3>"))?
        .into();
    let text = std::fs::read_to_string(&path)?;
    let module = treebank_sdf3::parse_module(&text)?;
    let lowered = treebank_sdf3::lower(&module)?;
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::write(
        dir.join("grammar.json"),
        serde_json::to_string_pretty(&lowered.grammar)? + "\n",
    )?;
    std::fs::write(
        dir.join("grammar.js"),
        treebank_sdf3::to_grammar_js(&lowered.grammar),
    )?;
    std::fs::write(
        dir.join("findings.md"),
        treebank_sdf3::report(&lowered.findings),
    )?;
    let rules = lowered.grammar["rules"]
        .as_object()
        .map(|r| r.len())
        .unwrap_or(0);
    eprintln!(
        "{}: {} rules, {} findings -> {}",
        module.name,
        rules,
        lowered.findings.len(),
        dir.display()
    );
    Ok(())
}
