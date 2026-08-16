//! The sweep-side per-language knowledge: which generated grammars a
//! language's crate carries, how a corpus file routes to one, and what the
//! language knows about its own preprocessor. This is grammar knowledge,
//! not corpus or oracle knowledge, which is why it lives here and not in
//! treebank-corpus or treebank-oracle.

use treebank_lang::LangName;
use treebank_preprocessing::Symbols;

/// Grammar dirs to load, in routing-index order, relative to the grammar
/// crate root. Single-grammar languages get `["."]`.
pub fn grammar_dirs(lang: LangName) -> &'static [&'static str] {
    match lang {
        LangName::Python | LangName::Rust => &["."],
        // One grammar source, two generated parsers (DESIGN.md §4.2):
        // `<T>x` is a cast in .ts and an unclosed JSX element in .tsx.
        LangName::Typescript => &["typescript", "tsx"],
        // Plain JS parses with the tsx dialect; a javascript "grammar dir"
        // exists only so .js corpora can be swept through it.
        LangName::Javascript => &["."],
    }
}

/// Index into `grammar_dirs()` for a file.
pub fn route(lang: LangName, dialect: &Option<String>, rel: &str) -> usize {
    match lang {
        LangName::Typescript => {
            let is_tsx = dialect
                .as_deref()
                .map(|d| d == "tsx")
                .unwrap_or_else(|| rel.ends_with(".tsx"));
            usize::from(is_tsx)
        }
        _ => 0,
    }
}

/// What this language knows for certain about its own preprocessor.
/// `None` means the source is parsed exactly as written, which is right
/// for every current target; the hook (and treebank-preprocessing behind
/// it) exists for the C-family languages that will arrive later.
pub fn preprocessing(lang: LangName) -> Option<&'static Symbols> {
    let _ = lang;
    None
}
