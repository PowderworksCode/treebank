mod grammar;
mod rosetta;
mod routing;
mod verify;
mod mutate;
mod shape;
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
    /// Compare our node BOUNDARIES against the reference parser's over the
    /// corpus. Catches silent mis-parses: files that parse cleanly and build
    /// the wrong tree, which the sweep is structurally blind to.
    Shape {
        #[arg(long, value_enum, default_value_t = LangName::Typescript)]
        lang: LangName,
        /// Grammar dir, as for `sweep`
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/manifest.json]
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [default: corpus/<lang>/reports/shape.json]
        #[arg(long)]
        out: Option<PathBuf>,
        /// Check only the first N files, for a quick look
        #[arg(long)]
        limit: Option<usize>,
        /// Check a directory of committed fixtures instead of the corpus.
        /// The ceiling is ZERO there: every file is a mis-parse that was
        /// fixed, so any miss is it coming back.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Mutate corpus files and ask whether the grammar accepts things the
    /// language does not. The sweep measures rejects-valid over the whole
    /// corpus; this measures the other direction, which `test/negative/`
    /// has been measuring with a dozen hand-written files.
    Mutate {
        #[arg(long, value_enum, default_value_t = LangName::Python)]
        lang: LangName,
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/manifest.json]
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [default: corpus/<lang>/reports/mutate.json]
        #[arg(long)]
        out: Option<PathBuf>,
        /// Corpus files to sample, spread evenly through the manifest
        #[arg(long, default_value_t = 2000)]
        files: usize,
        /// Mutants per file
        #[arg(long, default_value_t = 10)]
        per_file: usize,
        /// Reproducibility: the same seed gives the same mutants
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
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
    /// Check a grammar's vocabulary conformance (DESIGN.md §3.3): declared
    /// supertypes from the closed table tier, every named node covered or
    /// deliberately uncategorised, required containments, and a valid
    /// roles.json facet manifest
    Roles {
        /// Grammar crate root: reads src/node-types.json and roles.json
        grammar: PathBuf,
    },
    /// Run the rosetta gate: the same program in every owned language must
    /// yield the same role counts (DESIGN.md §5.4)
    Rosetta {
        /// Directory of rosetta cases [default: test/rosetta]
        #[arg(long, default_value = "test/rosetta")]
        dir: PathBuf,
        /// Where the grammar crates live [default: crates]
        #[arg(long, default_value = "crates")]
        crates: PathBuf,
    },
    /// Run every gate a grammar must pass: reproducible generation, corpus
    /// tests, negative corpus, vocabulary conformance, and the rosetta suite
    Verify {
        /// Grammar crate root
        grammar: PathBuf,
        #[arg(long, default_value = "crates")]
        crates: PathBuf,
        #[arg(long, default_value = "test/rosetta")]
        rosetta: PathBuf,
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

/// `treebank roles`: the vocabulary-conformance gate, one grammar crate at
/// a time. Prints every finding rather than the first, and exits non-zero
/// on any — an empty report is conformance.
pub fn roles_check(grammar_dir: &std::path::Path) -> anyhow::Result<String> {
    let vocab = treebank_core::vocabulary();
    let nt = treebank_core::node_types::NodeTypes::load(&grammar_dir.join("src/node-types.json"))?;
    let roles = treebank_core::roles::RolesManifest::load(&grammar_dir.join("roles.json"))?;
    let findings = treebank_core::check::check(&nt, &roles, vocab);
    if !findings.is_empty() {
        anyhow::bail!("{}", findings.join("; "));
    }
    Ok(format!(
        "{} supertypes, {} facet(s), {} named node(s), {} uncategorised (vocabulary {})",
        nt.supertypes.len(),
        roles.facets.len(),
        nt.named.len() - nt.supertypes.len(),
        roles.uncategorised.len(),
        vocab.version,
    ))
}

fn roles_cmd(grammar_dir: &std::path::Path) -> anyhow::Result<()> {
    let vocab = treebank_core::vocabulary();
    let nt = treebank_core::node_types::NodeTypes::load(&grammar_dir.join("src/node-types.json"))?;
    let roles = treebank_core::roles::RolesManifest::load(&grammar_dir.join("roles.json"))?;
    let findings = treebank_core::check::check(&nt, &roles, vocab);
    for f in &findings {
        eprintln!("roles: {f}");
    }
    if !findings.is_empty() {
        anyhow::bail!("{} vocabulary conformance finding(s)", findings.len());
    }
    println!(
        "roles OK: {} supertypes, {} facet(s), {} named node(s), {} uncategorised (vocabulary {})",
        nt.supertypes.len(),
        roles.facets.len(),
        nt.named.len() - nt.supertypes.len(),
        roles.uncategorised.len(),
        vocab.version,
    );
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
        Cmd::Shape { lang, grammar, manifest, out, limit, dir } => shape::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/shape.json"),
            limit,
            dir.as_deref(),
        ),
        Cmd::Mutate { lang, grammar, manifest, out, files, per_file, seed } => mutate::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/mutate.json"),
            files,
            per_file,
            seed,
        ),
        Cmd::Sweep { lang, grammar, manifest, out } => sweep::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/sweep.json"),
        ),
        Cmd::Roles { grammar } => roles_cmd(&grammar),
        Cmd::Rosetta { dir, crates } => rosetta::run(&dir, &crates),
        Cmd::Verify { grammar, crates, rosetta } => verify::run(&grammar, &crates, &rosetta),
        Cmd::Negative { grammar, dir } => sweep::negative(&grammar, &dir),
        Cmd::Oracle { lang, srcroot } => oracle_cmd(lang, &srcroot),
    }
}
