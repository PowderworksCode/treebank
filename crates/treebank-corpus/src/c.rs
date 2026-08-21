use std::path::Path;

use anyhow::Result;

use crate::rank::RankedCrate;
use crate::{cxx, debian, Ecosystem};
use treebank_lang::LangName;

pub struct C;

/// C has no registry, so "popular C" has to be borrowed from somewhere, and
/// the choice is **Debian** — see `debian` for the bias that comes with it.
/// What is C-specific is the filter: popcon ranks everything Debian ships,
/// so without one the top of the list spends its bandwidth on LibreOffice
/// (4.4M lines of C++ to 34k of C) and gcc (no C at all).
///
/// `is_c` wants two things: enough C to be worth a download, and **more C
/// than C++**, so that the C++ giants do not enter on their C fringe. The
/// second half is what makes this and [`cxx::is_cxx`] a partition rather
/// than two overlapping filters, and it is why a package appears in one
/// corpus or the other and not both.
pub(crate) fn is_c(s: &debian::Sloc) -> bool {
    s.lines("ansic") >= 2000 && s.lines("ansic") >= s.lines("cpp")
}

impl Ecosystem for C {
    fn name(&self) -> LangName {
        LangName::C
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        debian::rank(LangName::C, db, k, "C", &is_c)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        debian::resolve(LangName::C, pkg)
    }

    /// `.c` and `.h`. Headers are half of C and the half where declaration
    /// syntax lives — typedefs, bit-fields, attributes, macros in
    /// declaration position — which is exactly what a C grammar gets wrong.
    /// `admit` then drops the C++ ones.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        matches!(rel.extension()?.to_str()?, "c" | "h").then_some(None)
    }

    /// `.h` is shared by C and C++ and the extension cannot tell them
    /// apart. The measured reason to filter rather than let the oracle sort
    /// it out: a C++ header comes back **indeterminate**, not `invalid`, so
    /// unfiltered headers would inflate the one bucket whose size decides
    /// whether a C sweep means anything at all.
    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        if rel.extension().and_then(|e| e.to_str()) != Some("h") {
            return true;
        }
        !cxx::header_is_cxx(rel, content)
    }

    /// 250 MB, for the reason `bash` records: the top Debian sources by
    /// popcon include a handful of multi-gigabyte trees that are a small
    /// fraction of the files. Every skip is logged by the fetch driver.
    fn max_artifact_bytes(&self) -> Option<u64> {
        Some(250_000_000)
    }
}
