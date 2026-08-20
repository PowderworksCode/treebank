//! The sweep-side per-language knowledge: what a language knows about its
//! own preprocessor. Grammar knowledge, not corpus or oracle knowledge,
//! which is why it lives here and not in treebank-corpus or
//! treebank-oracle.
//!
//! This used to also carry `grammar_dirs` and `route`, which let one
//! language's crate hold several parsers and picked one per corpus file.
//! Nothing ever used it: `grammar_dirs` returned `["."]` for every
//! language and `route` returned `0` for every file, so nine callers built
//! a one-element `Vec<Language>` and indexed it at zero. The construct it
//! was built for was the TypeScript/JavaScript dialect split, and
//! DESIGN.md §4.2 settled that with one union grammar — the legacy `<T>x`
//! cast measured at ~zero corpus incidence and carried as a ledgered
//! known-gap. Generality kept for a case that was measured and rejected
//! costs a `Vec` and an index at every call site and buys nothing; a
//! language that genuinely needs two parsers can reintroduce both, with a
//! caller that does something with them.

use treebank_lang::LangName;
use treebank_preprocessing::Symbols;

/// What this language knows for certain about its own preprocessor.
/// `None` means the source is parsed exactly as written, which is right
/// for every current target; the hook (and treebank-preprocessing behind
/// it) exists for the C-family languages that will arrive later.
pub fn preprocessing(lang: LangName) -> Option<&'static Symbols> {
    let _ = lang;
    None
}
