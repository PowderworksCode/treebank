use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use rayon::prelude::*;

use crate::{LangName, Oracle};

pub struct Rust;

impl Oracle for Rust {
    fn name(&self) -> LangName {
        LangName::Rust
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
