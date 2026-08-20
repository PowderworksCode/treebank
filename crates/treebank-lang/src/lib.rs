use serde::Deserialize;

/// The canonical name of a supported language. This is the only place the
/// spelling is decided: it is what `--lang` accepts and what
/// `corpus/<lang>/` is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, clap::ValueEnum)]
pub enum LangName {
    #[serde(rename = "python")]
    #[value(name = "python")]
    Python,
    #[serde(rename = "rust")]
    #[value(name = "rust")]
    Rust,
    #[serde(rename = "typescript")]
    #[value(name = "typescript")]
    Typescript,
    /// Not a treebank grammar of its own — the TypeScript grammar's `tsx`
    /// dialect parses plain JS. Kept as a corpus/oracle language so `.js`
    /// files can be fetched, swept and adjudicated (V8) independently.
    #[serde(rename = "javascript")]
    #[value(name = "javascript")]
    Javascript,
    #[serde(rename = "java")]
    #[value(name = "java")]
    Java,
    #[serde(rename = "bash")]
    #[value(name = "bash")]
    Bash,
    #[serde(rename = "zig")]
    #[value(name = "zig")]
    Zig,
    /// One grammar for the dialect union, the way every other language
    /// here gets one grammar for its version union: SQLite, PostgreSQL and
    /// MySQL are to SQL what editions are to Rust.
    #[serde(rename = "sql")]
    #[value(name = "sql")]
    Sql,
}

impl LangName {
    pub fn as_str(self) -> &'static str {
        match self {
            LangName::Python => "python",
            LangName::Rust => "rust",
            LangName::Typescript => "typescript",
            LangName::Javascript => "javascript",
            LangName::Java => "java",
            LangName::Bash => "bash",
            LangName::Zig => "zig",
            LangName::Sql => "sql",
        }
    }
}

impl std::fmt::Display for LangName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
