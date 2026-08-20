//! The sweep-side per-language knowledge: which generated grammars a
//! language's crate carries, how a corpus file routes to one, and what the
//! language knows about its own preprocessor. This is grammar knowledge,
//! not corpus or oracle knowledge, which is why it lives here and not in
//! treebank-corpus or treebank-oracle.

use treebank_lang::LangName;
use treebank_preprocessing::Symbols;

/// Grammar dirs to load, in routing-index order, relative to the grammar
/// crate root. `route` below indexes into what this returns.
///
/// One parser per language, so `["."]` for all of them — including
/// TypeScript, the one that looked like it needed two. One parser covers
/// the whole TS ∪ JS ∪ JSX union: the only construct the DESIGN.md §4.2
/// dialect split existed for is the legacy `<T>x` cast, measured at ~zero
/// corpus incidence and carried as a ledgered known-gap instead, and plain
/// JS sweeps point --grammar at the same crate. A language that does need
/// two grows a `match` back.
pub fn grammar_dirs(lang: LangName) -> &'static [&'static str] {
    let _ = lang;
    &["."]
}

/// Index into `grammar_dirs()` for a file.
pub fn route(lang: LangName, dialect: &Option<String>, rel: &str) -> usize {
    let _ = (lang, dialect, rel);
    0
}

/// What this language knows for certain about its own preprocessor.
/// `None` means the source is parsed exactly as written, which is right
/// for every current target; the hook (and treebank-preprocessing behind
/// it) exists for the C-family languages that will arrive later.
pub fn preprocessing(lang: LangName) -> Option<&'static Symbols> {
    let _ = lang;
    None
}
