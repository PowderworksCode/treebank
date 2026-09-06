//! `cargo run -p treebank-sdf3 --example format -- <spike dir> <file> [--term]`
//!
//! Parse the file with the spike's generated parser (through the pinned
//! tree-sitter CLI), implode the tree to the module's term, and print the
//! term through the templates' layout. With `--term`, print the term
//! instead, in ATerm syntax.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let want_term = args.iter().any(|a| a == "--term");
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [spike, file] = positional[..] else {
        anyhow::bail!("usage: format <spike dir> <file> [--term]");
    };
    let spike = PathBuf::from(spike);
    let file = PathBuf::from(file);
    let name = spike
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("bad spike dir"))?;
    let module = treebank_sdf3::load_module(&spike.join(format!("{name}.sdf3")))?;
    let everything = treebank_sdf3::lower_all(&module)?;
    let names = everything.lowered.names;
    let out = Command::new("tree-sitter")
        .arg("parse")
        .arg(file.canonicalize()?)
        .current_dir(&spike)
        .output()?;
    let sexp = String::from_utf8_lossy(&out.stdout);
    let cst = treebank_sdf3::term::parse_sexp(&sexp)?;
    let source = std::fs::read_to_string(&file)?;
    let term = treebank_sdf3::term::Imploder::new(&module, &names).implode(&cst, &source)?;
    if want_term {
        println!("{}", term.aterm());
        return Ok(());
    }
    let printer = treebank_sdf3::print::Printer::new(&module);
    print!("{}", printer.print(&term)?);
    Ok(())
}
