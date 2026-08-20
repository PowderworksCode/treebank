//! Re-rendering a file from the reference parser's own tree.
//!
//! Every other check reads the corpus as written. This one asks a question
//! the corpus cannot: whether we handle each construct in the form the
//! language's own tools EMIT, rather than only in the form its authors
//! happened to write.
//!
//! `ast.unparse` and `ts.createPrinter` both print in one canonical spelling
//! — no comments, normalised quotes and spacing, parentheses only where the
//! tree needs them. A construct we parse in its common spelling and not in
//! its canonical one is a real gap that no amount of real source will show,
//! because real source is written by people who write it the usual way.
//!
//! Rust's printer is `prettyplease` over the `syn` tree the validity oracle
//! already builds, so it runs in-process with no subprocess at all. It was
//! chosen over `rustfmt`, which is the tool people reach for first and the
//! wrong one here: rustfmt is text-to-text. It reformats a token stream it
//! never stopped holding, so it keeps comments, keeps redundant parentheses,
//! and keeps whatever spelling the author used — exactly the thing this
//! check exists to get away from. `prettyplease` renders from the tree and
//! never consults the original bytes, which is what makes it an answer to
//! "do we handle the form the toolchain EMITS".
//!
//! It canonicalises less than `ast.unparse` does, and the reason is worth
//! knowing rather than discovering: `syn` models parentheses as a real AST
//! node (`Expr::Paren`), so `((x))` survives a round trip that Python's
//! `ast` would flatten to `x`. Comments do not survive, spacing does not
//! survive, and layout is regenerated. Weaker than Python's printer, still
//! categorically different from a formatter.
//!
//! One operational wrinkle: `syn` parks constructs it does not fully model
//! as `Verbatim` token streams, and `prettyplease` calls `unimplemented!()`
//! on several of those (`async fn f(&self);` in a trait, for one). That is
//! a panic, and one panic on file 400 would otherwise take the whole
//! 27,000-file run with it. Those are caught and reported as skips — but
//! only those: the hook checks the panic came from inside `prettyplease`
//! before swallowing it, so a panic in our own code still crashes the run
//! the way it should.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Deserialize;

use crate::{stdin_oracle, LangName};

pub struct Rendered {
    pub source: Option<String>,
    pub skipped: Option<String>,
}

pub trait Unparser: Sync {
    fn unparse(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Rendered>>;
}

/// `None` where the language's toolchain has no printer we can drive.
/// Declared in [`crate::capabilities`], with the reason for each `None`.
pub fn get(name: LangName) -> Option<&'static dyn Unparser> {
    crate::capabilities::get(name).unparse
}

pub(crate) struct PythonUnparser;
pub(crate) struct TypeScriptUnparser;
pub(crate) struct RustUnparser;

#[derive(Deserialize)]
struct RawRendered {
    path: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    skipped: Option<String>,
}

fn decode(lines: &[String], srcroot: &Path) -> Result<HashMap<String, Rendered>> {
    let mut out = HashMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let raw: RawRendered = serde_json::from_str(line)
            .with_context(|| format!("parse unparse output: {line:.200}"))?;
        out.insert(
            stdin_oracle::relativize(&raw.path, srcroot),
            Rendered {
                source: raw.source,
                skipped: raw.skipped,
            },
        );
    }
    Ok(out)
}

impl Unparser for PythonUnparser {
    fn unparse(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Rendered>> {
        let script = crate::tool("py-oracle/unparse.py");
        let lines = stdin_oracle::run_lines(
            "python3",
            &[script.to_string_lossy().as_ref()],
            "python3 tools/py-oracle/unparse.py — is python3 installed?",
            srcroot,
            paths,
        )?;
        decode(&lines, srcroot)
    }
}

impl Unparser for TypeScriptUnparser {
    fn unparse(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Rendered>> {
        let lines = stdin_oracle::node_lines(
            &crate::tool("ts-oracle"),
            "unparse.mjs",
            &[],
            srcroot,
            paths,
        )?;
        decode(&lines, srcroot)
    }
}

/// Silence panic reporting for panics raised inside `prettyplease`, and
/// only those. Installed once; the default hook still runs for everything
/// else, so this hides expected noise without hiding a real crash.
fn quiet_prettyplease_panics() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let from_printer = info
                .location()
                .is_some_and(|l| l.file().contains("prettyplease"));
            if !from_printer {
                previous(info);
            }
        }));
    });
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .unwrap_or_else(|| "panicked".to_string())
}

impl Unparser for RustUnparser {
    fn unparse(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Rendered>> {
        quiet_prettyplease_panics();
        // No subprocess: `syn` and `prettyplease` are both in this binary.
        // A file `syn` will not parse comes back `skipped` rather than
        // counted as a failure, for the same reason the span oracle skips
        // it — the printer cannot render a tree it never built, and that is
        // a fact about the oracle, not about our grammar.
        Ok(paths
            .par_iter()
            .map(|rel| {
                let rendered = match std::fs::read_to_string(srcroot.join(rel)) {
                    Ok(src) => match syn::parse_file(&src) {
                        Ok(ast) => {
                            let printed =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    prettyplease::unparse(&ast)
                                }));
                            match printed {
                                Ok(text) => Rendered {
                                    source: Some(text),
                                    skipped: None,
                                },
                                Err(p) => Rendered {
                                    source: None,
                                    skipped: Some(format!("prettyplease: {}", panic_message(&p))),
                                },
                            }
                        }
                        Err(e) => Rendered {
                            source: None,
                            skipped: Some(format!("syn: {e}")),
                        },
                    },
                    Err(e) => Rendered {
                        source: None,
                        skipped: Some(format!("read: {e}")),
                    },
                };
                (rel.clone(), rendered)
            })
            .collect())
    }
}
