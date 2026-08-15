//! Skeleton — see `super::skeleton`. Rank 33 in ROADMAP.md.
//! Oracle: `make -n --dry-run`, weakly. The roadmap marks this one WEAK
//! because GNU make has no parse-only mode; it needs an oracle decision
//! before it needs code.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{skeleton::not_implemented, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Make;

impl Lang for Make {
    fn name(&self) -> LangName {
        LangName::Make
    }

    fn skeleton(&self) -> bool {
        true
    }

    fn rank(&self, _db: &Path, _k: usize) -> Result<Vec<RankedCrate>> {
        Err(not_implemented(LangName::Make))
    }

    fn resolve(&self, _pkg: &RankedCrate) -> Result<(String, String)> {
        Err(not_implemented(LangName::Make))
    }

    /// The one method that cannot return an error, and so the one place a
    /// skeleton could lie: `None` here means "no file belongs in the
    /// corpus", which is indistinguishable from an empty corpus. It is
    /// unreachable because `lang::require` refuses this language before
    /// `fetch` — its only caller — starts.
    fn classify(&self, _rel: &Path) -> Option<Option<String>> {
        unreachable!("{}", not_implemented(LangName::Make))
    }

    /// A placeholder, not a claim: ocaml (impl + intf) and any other
    /// multi-grammar language settles this when it is implemented.
    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    fn validate(&self, _srcroot: &Path, _paths: &[String]) -> Result<HashMap<String, bool>> {
        Err(not_implemented(LangName::Make))
    }
}
