//! The language registry: one declaration per language, and everything
//! that can be derived from it.
//!
//! This crate exists because adding a language used to mean writing the
//! same fact down in a dozen places — the name in an enum, the name again
//! in `as_str`, the name again in `Display`, the file extensions in the
//! fuzzer, the same extensions again in the shape checker. None of those
//! were decisions; they were transcription, and transcription is where
//! drift comes from.
//!
//! So the facts that are pure data live in the `languages!` block below, once
//! each, and the rest of the repository asks for them. What is left over
//! after that — a corpus ecosystem, a reference oracle, a grammar — is
//! real per-language work, and the exhaustive `match`es that demand it are
//! deliberate: the compiler refusing to build until a new language has an
//! answer is the cheapest reviewer in the repository.

use serde::Deserialize;

/// Declare the languages. Each entry is:
///
/// ```text
/// Variant => "name", exts: ["ext", ...], grammar: Variant;
/// ```
///
/// - `name` is the canonical spelling: what `--lang` accepts, what
///   `corpus/<lang>/` is called, and what `Display` prints.
/// - `exts` are the language's source extensions, most canonical first —
///   the first one is what the fuzzer names its scratch files.
/// - `grammar` names the language whose `crates/treebank-<name>` grammar
///   parses this one. Usually itself; it differs only for a dialect that
///   an existing union grammar already covers.
macro_rules! languages {
    ($(
        $(#[$attr:meta])*
        $variant:ident => $name:literal, exts: [$($ext:literal),+ $(,)?], grammar: $grammar:ident;
    )+) => {
        /// The canonical name of a supported language. This is the only
        /// place the spelling is decided.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, clap::ValueEnum)]
        pub enum LangName {
            $(
                $(#[$attr])*
                #[serde(rename = $name)]
                #[value(name = $name)]
                $variant,
            )+
        }

        impl LangName {
            /// Every language, in declaration order. Iterating this is how
            /// a check covers the whole registry without a list of its own
            /// to fall out of date.
            pub const ALL: &'static [LangName] = &[$(LangName::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $(LangName::$variant => $name,)+
                }
            }

            /// The language's source extensions, most canonical first.
            pub fn extensions(self) -> &'static [&'static str] {
                match self {
                    $(LangName::$variant => &[$($ext),+],)+
                }
            }

            /// Which language's grammar crate parses this one.
            pub fn grammar(self) -> LangName {
                match self {
                    $(LangName::$variant => LangName::$grammar,)+
                }
            }
        }
    };
}

languages! {
    Python => "python", exts: ["py", "pyi"], grammar: Python;
    Rust => "rust", exts: ["rs"], grammar: Rust;
    Typescript => "typescript", exts: ["ts", "tsx", "mts", "cts"], grammar: Typescript;
    /// Not a treebank grammar of its own — the TypeScript grammar's `tsx`
    /// dialect parses plain JS. Kept as a corpus/oracle language so `.js`
    /// files can be fetched, swept and adjudicated (V8) independently.
    Javascript => "javascript", exts: ["js", "jsx", "mjs", "cjs"], grammar: Typescript;
    Java => "java", exts: ["java"], grammar: Java;
    Bash => "bash", exts: ["sh", "bash"], grammar: Bash;
    Ruby => "ruby", exts: ["rb"], grammar: Ruby;
    Zig => "zig", exts: ["zig", "zon"], grammar: Zig;
}

impl LangName {
    /// The canonical extension: what to call a file of this language when
    /// something has to invent one.
    pub fn primary_extension(self) -> &'static str {
        self.extensions()[0]
    }

    /// Every extension the language's GRAMMAR handles, which is a wider
    /// set than the language's own whenever one grammar covers several
    /// dialects — `treebank-typescript` is asked about `.js` too, and a
    /// fixture directory for it may hold either.
    pub fn grammar_extensions(self) -> Vec<&'static str> {
        let grammar = self.grammar();
        LangName::ALL
            .iter()
            .filter(|l| l.grammar() == grammar)
            .flat_map(|l| l.extensions())
            .copied()
            .collect()
    }

    /// The directory name of the grammar crate that parses this language,
    /// relative to `crates/`.
    pub fn grammar_crate(self) -> String {
        format!("treebank-{}", self.grammar())
    }

    /// The language with this canonical name, if any. The inverse of
    /// [`LangName::as_str`], for the places that meet a name as text —
    /// a directory called `treebank-zig`, a CI matrix entry — rather than
    /// through clap or serde.
    pub fn from_name(name: &str) -> Option<LangName> {
        LangName::ALL.iter().copied().find(|l| l.as_str() == name)
    }
}

impl std::fmt::Display for LangName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::LangName;
    use std::collections::HashSet;

    #[test]
    fn names_round_trip_and_are_distinct() {
        let mut seen = HashSet::new();
        for &lang in LangName::ALL {
            assert!(seen.insert(lang.as_str()), "duplicate name {lang}");
            assert_eq!(LangName::from_name(lang.as_str()), Some(lang));
        }
        assert_eq!(LangName::from_name("cobol"), None);
    }

    /// An extension may belong to exactly one language. Two claims on the
    /// same suffix would make a corpus file's language a coin toss, and
    /// the coin would be tossed differently in each caller.
    #[test]
    fn extensions_are_unambiguous() {
        let mut owner = std::collections::HashMap::new();
        for &lang in LangName::ALL {
            for ext in lang.extensions() {
                if let Some(other) = owner.insert(*ext, lang) {
                    panic!("both {other} and {lang} claim .{ext}");
                }
            }
        }
    }

    #[test]
    fn a_dialect_borrows_its_grammars_extensions() {
        let ts = LangName::Typescript.grammar_extensions();
        assert!(ts.contains(&"ts") && ts.contains(&"js"));
        assert_eq!(
            LangName::Javascript.grammar_extensions(),
            ts,
            "a dialect and its grammar see the same fixture extensions"
        );
        assert_eq!(LangName::Javascript.grammar_crate(), "treebank-typescript");
        assert_eq!(LangName::Zig.grammar_extensions(), vec!["zig", "zon"]);
    }
}
