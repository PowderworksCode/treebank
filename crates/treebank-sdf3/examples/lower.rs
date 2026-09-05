//! `cargo run -p treebank-sdf3 --example lower -- spike/mini/mini.sdf3 [--generate]`
//!
//! Reads the module and its imports, lowers it, and writes `grammar.json`,
//! `grammar.js` and `findings.md` beside it -- and `src/scanner.c` when the
//! module's layout constraints call for a scanner.
//!
//! With `--generate`, runs `tree-sitter generate` and, while it reports an
//! unresolved conflict, declares the conflict it names and tries again. The
//! set it ends with is pinned in `tree-sitter.conflicts.json` beside the
//! module, so the lowering is reproducible without the CLI: that file is
//! the `carry` intent's backend data.

use std::path::PathBuf;
use std::process::Command;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let generate = args.iter().any(|a| a == "--generate");
    let path: PathBuf = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .ok_or_else(|| anyhow::anyhow!("usage: lower <module.sdf3> [--generate]"))?
        .into();
    let module = treebank_sdf3::load_module(&path)?;
    let lowered = treebank_sdf3::lower(&module)?;
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let sidecar = dir.join("tree-sitter.conflicts.json");

    let mut grammar = lowered.grammar;
    let mut findings = lowered.findings;
    let mut conflicts = treebank_sdf3::read_conflicts(&sidecar)?.unwrap_or_default();
    if !conflicts.is_empty() {
        treebank_sdf3::apply_conflicts(&mut grammar, &conflicts);
    }
    // The conflict findings are appended once, after the loop settles.
    if generate {
        let mut rounds = 0;
        loop {
            write_grammar(dir, &grammar)?;
            let out = Command::new("tree-sitter")
                .arg("generate")
                .arg("grammar.json")
                .current_dir(dir)
                .output()?;
            let stderr = String::from_utf8_lossy(&out.stderr);
            if out.status.success() {
                break;
            }
            let suggested = treebank_sdf3::conflicts_suggested(&stderr);
            rounds += 1;
            if suggested.is_empty() || rounds > 12 {
                eprintln!("{stderr}");
                anyhow::bail!("tree-sitter generate failed without a conflict to declare");
            }
            for set in suggested {
                if !conflicts.contains(&set) {
                    eprintln!("declaring conflict {set:?}, as generate suggested");
                    conflicts.push(set);
                }
            }
            treebank_sdf3::apply_conflicts(&mut grammar, &conflicts);
        }
        if conflicts.is_empty() {
            if sidecar.exists() {
                std::fs::remove_file(&sidecar)?;
            }
        } else {
            let pinned = serde_json::json!({
                "note": "Declared conflicts the tree-sitter lowering needs, each named by `tree-sitter generate` as unresolved and pinned here so the lowering is reproducible without the CLI. Regenerate with `--generate`; a diff means generate's view of the ambiguity moved.",
                "conflicts": conflicts,
            });
            std::fs::write(&sidecar, serde_json::to_string_pretty(&pinned)? + "\n")?;
        }
    }
    findings.extend(treebank_sdf3::apply_conflicts(&mut grammar, &conflicts));

    write_grammar(dir, &grammar)?;
    std::fs::write(
        dir.join("grammar.js"),
        treebank_sdf3::to_grammar_js(&grammar),
    )?;
    std::fs::write(dir.join("findings.md"), treebank_sdf3::report(&findings))?;
    if let Some(c) = &lowered.scanner {
        std::fs::create_dir_all(dir.join("src"))?;
        std::fs::write(dir.join("src/scanner.c"), c)?;
    }
    // The second backend, from the same module and the same names.
    let antlr = treebank_sdf3::antlr::emit(&module, &lowered.names, &lowered.levels)?;
    let gname = {
        let mut c = module.name.chars();
        c.next()
            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    };
    std::fs::write(dir.join(format!("{gname}.g4")), &antlr.grammar)?;
    std::fs::write(
        dir.join("antlr-findings.md"),
        treebank_sdf3::report(&antlr.findings),
    )?;
    let rules = grammar["rules"].as_object().map(|r| r.len()).unwrap_or(0);
    eprintln!(
        "{}: {} rules, {} findings, {} conflicts{} -> {}",
        module.name,
        rules,
        findings.len(),
        conflicts.len(),
        if lowered.scanner.is_some() {
            ", generated scanner"
        } else {
            ""
        },
        dir.display()
    );
    Ok(())
}

fn write_grammar(dir: &std::path::Path, grammar: &serde_json::Value) -> anyhow::Result<()> {
    std::fs::write(
        dir.join("grammar.json"),
        serde_json::to_string_pretty(grammar)? + "\n",
    )?;
    Ok(())
}
