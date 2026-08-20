use std::path::Path;

use anyhow::Result;

use crate::rank::RankedCrate;
use crate::{Ecosystem, LangName};

/// The python 2 corpus is CPython 2.7's own source tree, and that is a
/// deliberate break from how every other corpus here is built.
///
/// Every other language ranks a registry by downloads and fetches the top
/// K, because the top of a registry is what that language's code looks
/// like. Rewinding PyPI to the python 2 era does not give you that, and the
/// measurement is not close: of the top 40 PyPI packages, 28 have a
/// pre-EOL release that declares python 2 support, and ZERO of those 28 are
/// python-2-only — they are all six-style code written in the INTERSECTION
/// of the two languages. Over 1,004 files from ten such releases, 96.9%
/// parse under both python 2 and python 3, and only 1.99% use syntax that
/// python 3 rejects. A corpus like that is ~98% blind to the variant it is
/// supposed to be measuring.
///
/// CPython 2.7.18's tree measures 44.4% python-2-only over 2,182 files —
/// 22 times the density — and `Lib/test/` inside it is adversarial by
/// construction, which is the same reason the SQL plan reaches for engine
/// regression suites rather than for application code (VARIANTS.md §7.5).
///
/// What it is blind to is stated in the ledger and is the mirror of what it
/// is good at: it is ONE codebase, written to one house style, by people
/// who were writing the language rather than using it. Breadth would come
/// from the PyPI rewind above, at 2% discriminating density, and that
/// trade is the ledger's to record rather than this file's to make
/// silently.
pub struct Python2;

/// Pinned like an oracle, because it is one half of a measurement: a
/// corpus that moves makes every number in the ledger unreproducible.
/// 2.7.18 is the final CPython 2 release, so this pin can never go stale
/// in the way a moving `latest` would.
const CPYTHON_VERSION: &str = "2.7.18";

impl Ecosystem for Python2 {
    fn name(&self) -> LangName {
        LangName::Python2
    }

    /// One "package", because there is one source of python 2 that is
    /// unambiguously python 2. `rank` exists to choose among many; here it
    /// reports the single entry so the generic fetch driver works
    /// unchanged, and `k` is ignored rather than pretended about.
    fn rank(&self, _db: &Path, _k: usize) -> Result<Vec<RankedCrate>> {
        Ok(vec![RankedCrate {
            rank: 1,
            name: "cpython".to_string(),
            // Resolved here rather than at fetch time: the pin IS the
            // version, and there is nothing to look up.
            version: CPYTHON_VERSION.to_string(),
            // Not a download count and not pretending to be one. This
            // corpus is not ranked by popularity; it is one source chosen
            // for what it contains.
            downloads: 0,
        }])
    }

    fn resolve(&self, _pkg: &RankedCrate) -> Result<(String, String)> {
        Ok((
            CPYTHON_VERSION.to_string(),
            format!(
                "https://www.python.org/ftp/python/{CPYTHON_VERSION}/Python-{CPYTHON_VERSION}.tgz"
            ),
        ))
    }

    /// Every `.py` in the tree, and the breadth is the point: `Lib/` is the
    /// standard library, `Lib/test/` is the test suite that was written to
    /// break the parser, and `Tools/`, `Demo/` and `setup.py` are ordinary
    /// programs. All of it is python 2 by construction — this is the
    /// interpreter's own source — so unlike the extension-based classifiers
    /// elsewhere there is no question of what language a file is in.
    ///
    /// `lib2to3/tests/data/` is kept even though it holds deliberately
    /// broken and deliberately py3 fixtures. They are what the oracle is
    /// for: it calls them invalid and the sweep books them as noise, which
    /// is the correct outcome and a live check that the adjudication works.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension()?.to_str()? == "py").then_some(None)
    }
}
