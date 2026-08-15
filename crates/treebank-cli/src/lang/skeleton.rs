//! Reserved names with no implementation behind them yet.
//!
//! Adding a language edits two files every other language PR also edits — a
//! `LangName` variant in `ledger.rs`, and a `mod` + `static` + match arm in
//! `lang/mod.rs`. With several language branches in flight that is a
//! guaranteed conflict per pair, and the rebasing is the whole cost. So the
//! shared edits are paid once, for every language the roadmap ranks Tier A
//! with a named oracle, and implementing one afterwards means filling in its
//! own `lang/<name>.rs` and adding its own `crates/treebank-<name>/` — no
//! shared file, no conflict.
//!
//! What a skeleton must NOT do is fail quietly. `validate()` returning an
//! empty map would read as "every file is valid", which is the failure
//! GRAMMARS.md forbids: a sweep would report zero gaps for a language that
//! does not exist. So every method a skeleton cannot answer returns this
//! error, and `lang::require` refuses `--lang <skeleton>` at the CLI
//! boundary before any of them is reached.
//!
//! The absence of `crates/treebank-<name>/` is load-bearing too: the CI
//! matrix and the oracle smoke test are both derived from
//! `crates/*/ledger.json`, so a skeleton with no directory enrolls in
//! neither.

use crate::ledger::LangName;

/// The one error every skeleton method returns. It names the language and
/// says where the work goes, because the reader is either someone who typed
/// a name the CLI advertises, or someone about to implement it.
pub fn not_implemented(name: LangName) -> anyhow::Error {
    anyhow::anyhow!(
        "{name} is a reserved language name with no implementation yet: \
         crates/treebank-cli/src/lang/{name}.rs is a skeleton, and \
         crates/treebank-{name}/ does not exist. See ROADMAP.md."
    )
}
