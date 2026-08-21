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

use std::sync::LazyLock;

use treebank_lang::LangName;
use treebank_preprocessing::Symbols;

/// What this language knows for certain about its own preprocessor.
/// `None` means the source is parsed exactly as written, which is right for
/// every language whose source has no preprocessor to reduce.
///
/// C is what this hook was built for. `__cplusplus` is not a symbol we are
/// uncertain about: compiling C, it is ALWAYS undefined. Declaring that one
/// fact is what lets the sweep recognise the `extern "C" {`-split-across-
/// `#ifdef` class — where the brace and its partner sit in different
/// conditionals, so no single tree can hold both configurations — which no
/// grammar patch can fix and which would otherwise sit at the top of the
/// fix queue forever. Measured on the local corpus, it is worth 693 of
/// 3,662 files.
///
/// C++ gets the mirror image, and it is just as certain: compiling C++,
/// `__cplusplus` is always DEFINED. Its VALUE is the standard's date and
/// depends on the dialect, so it is declared as `201703L` — the same
/// `gnu++17` the oracle judges with. The two have to agree: a sweep that
/// deleted a `#if __cplusplus >= 202002L` branch the oracle then compiled,
/// or the reverse, would be measuring the grammar against a file neither of
/// them read.
pub fn preprocessing(lang: LangName) -> Option<&'static Symbols> {
    static AS_C: LazyLock<Symbols> = LazyLock::new(|| Symbols::new().undefined("__cplusplus"));
    static AS_CXX: LazyLock<Symbols> = LazyLock::new(|| Symbols::new().defined("__cplusplus", 201703));
    match lang {
        LangName::C => Some(&AS_C),
        LangName::Cpp => Some(&AS_CXX),
        _ => None,
    }
}
