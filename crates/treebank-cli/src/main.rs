mod fetch;
mod grammar;
mod lang;
mod ledger;
mod rank;
mod sweep;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::ledger::LangName;

#[derive(Parser)]
#[command(name = "treebank", about = "Treebank corpus and grammar sweep tooling")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build the top-K package list for a language's ecosystem
    Rank {
        #[arg(long, value_enum, default_value_t = LangName::Rust)]
        lang: LangName,
        /// rust only: dir with the extracted crates.io db dump CSVs
        /// [default: corpus/<lang>/db]
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long, default_value_t = 1000)]
        k: usize,
        /// [default: corpus/<lang>/top-k.json]
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Download package tarballs, extract source files, write the manifest
    Fetch {
        #[arg(long, value_enum, default_value_t = LangName::Rust)]
        lang: LangName,
        /// [default: corpus/<lang>/top-k.json]
        #[arg(long)]
        list: Option<PathBuf>,
        /// How many packages from the top of the list to fetch
        #[arg(long, default_value_t = 100)]
        limit: usize,
        /// [default: corpus/<lang>]
        #[arg(long)]
        corpus: Option<PathBuf>,
    },
    /// Sweep the corpus with a grammar, adjudicate failures with the
    /// reference parser, and write sweep.json + an agent-ready REPORT.md
    Sweep {
        #[arg(long, value_enum, default_value_t = LangName::Rust)]
        lang: LangName,
        /// Grammar dir. rust: a generated grammar repo. typescript: the
        /// treebank-typescript root (contains typescript/ and tsx/; .tsx
        /// files route to the tsx grammar)
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/manifest.json]
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [default: corpus/<lang>/reports/sweep.json; REPORT.md lands
        /// alongside]
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Validate ledger.json against the tree it describes — one grammar dir,
    /// or every grammar under crates/ when given none
    Ledger {
        grammar: Option<PathBuf>,
    },
    /// Assert that every file in a directory FAILS to parse (negative corpus)
    Negative {
        #[arg(long)]
        grammar: PathBuf,
        #[arg(long)]
        dir: PathBuf,
    },
}

fn lang_path(lang: LangName, given: Option<PathBuf>, suffix: &str) -> PathBuf {
    given.unwrap_or_else(|| {
        let mut p = PathBuf::from("corpus").join(lang.as_str());
        if !suffix.is_empty() {
            p = p.join(suffix);
        }
        p
    })
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::Rank { lang, db, k, out } => rank::run(
            lang::get(lang),
            &lang_path(lang, db, "db"),
            k,
            &lang_path(lang, out, "top-k.json"),
        ),
        Cmd::Fetch { lang, list, limit, corpus } => fetch::run(
            lang::get(lang),
            &lang_path(lang, list, "top-k.json"),
            limit,
            &lang_path(lang, corpus, ""),
        ),
        Cmd::Sweep { lang, grammar, manifest, out } => sweep::run(
            lang::get(lang),
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/sweep.json"),
        ),
        Cmd::Ledger { grammar } => ledger::run(grammar.as_deref()),
        Cmd::Negative { grammar, dir } => sweep::negative(&grammar, &dir),
    }
}
