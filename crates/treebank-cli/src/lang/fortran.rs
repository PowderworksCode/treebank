//! Skeleton — see `super::skeleton`. Rank 40 in ROADMAP.md.
//! Oracle: `gfortran -fsyntax-only`. The dialect axis is fixed vs free
//! form.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{skeleton::not_implemented, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Fortran;

impl Lang for Fortran {
    fn name(&self) -> LangName {
        LangName::Fortran
    }

    fn skeleton(&self) -> bool {
        true
    }

    fn rank(&self, _db: &Path, _k: usize) -> Result<Vec<RankedCrate>> {
        Err(not_implemented(LangName::Fortran))
    }

    fn resolve(&self, _pkg: &RankedCrate) -> Result<(String, String)> {
        Err(not_implemented(LangName::Fortran))
    }

    /// The one method that cannot return an error, and so the one place a
    /// skeleton could lie: `None` here means "no file belongs in the
    /// corpus", which is indistinguishable from an empty corpus. It is
    /// unreachable because `lang::require` refuses this language before
    /// `fetch` — its only caller — starts.
    fn classify(&self, _rel: &Path) -> Option<Option<String>> {
        unreachable!("{}", not_implemented(LangName::Fortran))
    }

    /// A placeholder, not a claim: ocaml (impl + intf) and any other
    /// multi-grammar language settles this when it is implemented.
    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    fn validate(&self, _srcroot: &Path, _paths: &[String]) -> Result<HashMap<String, bool>> {
        Err(not_implemented(LangName::Fortran))
    }
}
