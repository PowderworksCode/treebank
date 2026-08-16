mod grammar;
mod routing;
mod sweep;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use treebank_lang::LangName;

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
    /// Assert that every file in a directory FAILS to parse (negative corpus)
    Negative {
        #[arg(long)]
        grammar: PathBuf,
        #[arg(long)]
        dir: PathBuf,
    },
    /// Run a language's reference parser over paths on stdin, one per line,
    /// writing "<path>\tvalid|invalid". This is `Lang::validate` and nothing
    /// else — the same call `sweep` adjudicates failures with.
    ///
    /// It exists so every oracle has ONE entry point regardless of shape.
    /// The oracles are otherwise four different things: batch processes
    /// reading stdin (python, go, zig...), a JSON three-valued one (c),
    /// fork-per-file exec oracles (bash, php), and `syn` in-process with no
    /// subprocess at all (rust). Smoke checks drive this rather than each tool
    /// directly, so they test the path `sweep` really takes, drivers and
    /// wiring included, rather than a parallel invocation that can drift.
    Oracle {
        #[arg(long, value_enum)]
        lang: LangName,
        /// Resolve stdin's paths against this root [default: the cwd].
        #[arg(long, default_value = ".")]
        srcroot: PathBuf,
    },
}

/// `treebank oracle`: stdin paths -> `Lang::validate` -> stdout verdicts.
///
/// Errors propagate, so an oracle that cannot answer exits non-zero with no
/// verdicts on stdout. That is the property `oracle-smoke.sh` asserts, and it
/// is the one that matters: `validate` is only ever called on files the
/// grammar ALREADY failed, so a verdict of `invalid` records the file as
/// corpus noise. An oracle that answers `invalid` for files it could not read
/// turns every grammar failure into noise and reports a flawless grammar.
fn oracle_cmd(lang: LangName, srcroot: &std::path::Path) -> anyhow::Result<()> {
    use std::io::{BufRead, Write};
    let paths: Vec<String> = std::io::stdin()
        .lock()
        .lines()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let verdicts = treebank_oracle::get(lang).validate(srcroot, &paths)?;
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for p in &paths {
        // A path the oracle declined to answer for is not silently dropped:
        // the caller asked about it, so say so and fail.
        let Some(v) = verdicts.get(p) else {
            anyhow::bail!("{lang} oracle returned no verdict for {p}");
        };
        writeln!(out, "{p}\t{}", if *v { "valid" } else { "invalid" })?;
    }
    out.flush()?;
    Ok(())
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
        Cmd::Rank { lang, db, k, out } => treebank_corpus::rank::run(
            treebank_corpus::get(lang),
            &lang_path(lang, db, "db"),
            k,
            &lang_path(lang, out, "top-k.json"),
        ),
        Cmd::Fetch { lang, list, limit, corpus } => treebank_corpus::fetch::run(
            treebank_corpus::get(lang),
            &lang_path(lang, list, "top-k.json"),
            limit,
            &lang_path(lang, corpus, ""),
        ),
        Cmd::Sweep { lang, grammar, manifest, out } => sweep::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/sweep.json"),
        ),
        Cmd::Negative { grammar, dir } => sweep::negative(&grammar, &dir),
        Cmd::Oracle { lang, srcroot } => oracle_cmd(lang, &srcroot),
    }
}
