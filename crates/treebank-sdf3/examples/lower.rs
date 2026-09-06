//! `cargo run -p treebank-sdf3 --example lower -- spike/mini/mini.sdf3 [--generate] [--out DIR]`
//!
//! Reads the module and its imports, lowers it, and writes `grammar.json`,
//! `grammar.js` and `findings.md` beside it -- and `src/scanner.c` when the
//! module's layout constraints call for a scanner. With `--out DIR` they go
//! to DIR instead, with a `tree-sitter.json` naming the grammar, which is
//! how one family source lowers to one directory per target
//! (`postgres/15.sdf3` to `targets/postgres-15/`).
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
    let out: Option<PathBuf> = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let path: PathBuf = args
        .iter()
        .enumerate()
        .find(|(i, a)| {
            !a.starts_with("--") && args.get(i.wrapping_sub(1)).map(String::as_str) != Some("--out")
        })
        .map(|(_, a)| a)
        .ok_or_else(|| anyhow::anyhow!("usage: lower <module.sdf3> [--generate] [--out DIR]"))?
        .into();
    let module = treebank_sdf3::load_module(&path)?;
    let everything = treebank_sdf3::lower_all(&module)?;
    let lowered = everything.lowered;
    let beside = path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    let dir: &std::path::Path = out.as_deref().unwrap_or(&beside);
    if out.is_some() {
        std::fs::create_dir_all(dir)?;
        let name = module.symbol_name();
        let camel: String = name
            .split('_')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let mut c = s.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            })
            .collect();
        let manifest = serde_json::json!({
            "grammars": [{
                "name": name,
                "camelcase": camel,
                "scope": format!("source.{name}"),
                "file-types": [name],
            }],
            "metadata": {
                "version": "0.0.0",
                "license": "MIT",
                "description": format!("target {} of an SDF3 family; generated, not a shipped grammar", module.name),
            }
        });
        std::fs::write(
            dir.join("tree-sitter.json"),
            serde_json::to_string_pretty(&manifest)? + "\n",
        )?;
    }
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
        let symbol = module.symbol_name();
        let mut c = symbol.chars();
        c.next()
            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    };
    std::fs::write(dir.join(format!("{gname}.g4")), &antlr.grammar)?;
    std::fs::write(
        dir.join("antlr-findings.md"),
        treebank_sdf3::report(&antlr.findings),
    )?;
    // The third backend: a scannerless winnow parser, as a crate of its own.
    let wn = treebank_sdf3::winnow::emit(&module, &lowered.names, &lowered.levels)?;
    std::fs::create_dir_all(dir.join("winnow/src"))?;
    std::fs::write(dir.join("winnow/Cargo.toml"), &wn.cargo_toml)?;
    std::fs::write(dir.join("winnow/src/main.rs"), &wn.source)?;
    std::fs::write(
        dir.join("winnow-findings.md"),
        treebank_sdf3::report(&wn.findings),
    )?;
    if let Some(v) = &everything.vocab {
        std::fs::write(
            dir.join("roles.json"),
            serde_json::to_string_pretty(&v.roles)? + "\n",
        )?;
    }
    // Bindings, when the module declares any: data plus the query view.
    if let Some(b) = everything.bindings {
        std::fs::write(
            dir.join("bindings.json"),
            serde_json::to_string_pretty(&b.json)? + "\n",
        )?;
        std::fs::create_dir_all(dir.join("queries"))?;
        std::fs::write(dir.join("queries/locals.scm"), &b.locals)?;
        std::fs::write(
            dir.join("bindings-findings.md"),
            treebank_sdf3::report(&b.findings),
        )?;
    }
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
