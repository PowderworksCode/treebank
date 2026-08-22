mod errpos;
mod fuzz;
mod grammar;
mod incremental;
mod kinds;
mod lint;
mod mutate;
mod recovery;
mod reformat;
mod rosetta;
mod roundtrip;
mod routing;
mod shape;
mod sweep;
mod verify;

use std::path::PathBuf;

use anyhow::Context as _;
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
        /// Also write a committable exact corpus lock here
        #[arg(long)]
        lock_out: Option<PathBuf>,
    },
    /// Recreate and verify the exact corpus pinned by a committed lock
    Hydrate {
        #[arg(long, value_enum, default_value_t = LangName::Rust)]
        lang: LangName,
        /// [default: corpus-locks/<lang>.json]
        #[arg(long)]
        lock: Option<PathBuf>,
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
    /// When the grammar rejects a file, does it reject in the right place?
    /// Compares our first ERROR node against where the reference parser
    /// reported its first error, over the files both reject.
    Errors {
        #[arg(long, value_enum, default_value_t = LangName::Python)]
        lang: LangName,
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/manifest.json]
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [default: corpus/<lang>/reports/errors.json]
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Derive programs FROM the grammar and ask the oracle whether they
    /// are in the language. The sweep, `mutate` and `roundtrip` are all
    /// bounded by what the corpus contains; this is not, which matters most
    /// for accepts-invalid — real source is valid, so no amount of it shows
    /// that we reject what the language rejects. Failures arrive shrunk.
    Fuzz {
        #[arg(long, value_enum, default_value_t = LangName::Rust)]
        lang: LangName,
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/reports/fuzz.json]
        #[arg(long)]
        out: Option<PathBuf>,
        /// Programs to derive
        #[arg(long, default_value_t = 2000)]
        iterations: usize,
        /// Reproduces a run exactly
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Derive every program from a fresh random tape, keeping nothing.
        /// For measuring what the coverage guidance is worth.
        #[arg(long, default_value_t = false)]
        unguided: bool,
        /// Steer toward node kinds `treebank kinds` found the corpus never
        /// produces. Measured across four languages and it helps exactly
        /// one — see the note at the top of fuzz.rs before turning it on.
        #[arg(long, default_value_t = false)]
        rare: bool,
    },
    /// Reformat every corpus file with the language's own formatter and
    /// assert our tree is unchanged. A formatter preserves the program and
    /// rewrites its layout, so a tree that moves is our bug: a rule reading
    /// layout it should not, or a token that only lexes when it abuts its
    /// neighbour.
    Reformat {
        #[arg(long, value_enum, default_value_t = LangName::Rust)]
        lang: LangName,
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/manifest.json]
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [default: corpus/<lang>/reports/reformat.json]
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Parse, edit, reparse incrementally, and compare against a fresh
    /// parse of the edited text. tree-sitter's contract is that the two are
    /// indistinguishable; every other check here parses from scratch, so a
    /// grammar can pass all of them and still hand a broken tree to an
    /// editor. The usual cause is an external scanner whose serialize and
    /// deserialize do not round-trip.
    Incremental {
        #[arg(long, value_enum, default_value_t = LangName::Python)]
        lang: LangName,
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/manifest.json]
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [default: corpus/<lang>/reports/incremental.json]
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Delete one token from a file that parses cleanly and measure how
    /// much of the file lands inside an ERROR. Editors spend most of their
    /// time on broken source, and what they can do with it depends on how
    /// much structure survives — a property no other check here looks at.
    Recovery {
        #[arg(long, value_enum, default_value_t = LangName::Python)]
        lang: LangName,
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/manifest.json]
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [default: corpus/<lang>/reports/recovery.json]
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Count node kinds over the corpus and report which ones real code
    /// never produces. Those are the blind spot: no oracle has been asked
    /// about them, because every corpus-driven check starts from code that
    /// does not contain them.
    Kinds {
        #[arg(long, value_enum, default_value_t = LangName::Python)]
        lang: LangName,
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/manifest.json]
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [default: corpus/<lang>/reports/kinds.json]
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
    },
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
    /// Re-render every file through the language's own printer and reparse
    /// it. Finds constructs we handle in the spelling people write and not
    /// in the one the toolchain emits.
    Roundtrip {
        #[arg(long, value_enum, default_value_t = LangName::Python)]
        lang: LangName,
        #[arg(long)]
        grammar: PathBuf,
        /// [default: corpus/<lang>/manifest.json]
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [default: corpus/<lang>/reports/roundtrip.json]
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
    },
    Sweep {
        #[arg(long, value_enum, default_value_t = LangName::Rust)]
        lang: LangName,
        /// The grammar crate to sweep with, e.g. crates/treebank-rust
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
    /// Check a grammar for the structural smells FIELD_GUIDE.md names:
    /// declared-conflict growth, early commits between parallel tiers,
    /// same-text token splits, unreserved keywords, scanner/externals
    /// drift, and parse-table growth — judged against the grammar's
    /// lint_policy.toml baselines (advisory when there is none)
    Lint {
        /// Grammar crate root: reads src/grammar.json, src/parser.c,
        /// src/scanner.c and lint_policy.toml
        grammar: PathBuf,
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
    let mut findings = treebank_core::check::check(&nt, &roles, vocab);
    findings.extend(ledger_vocabulary_finding(grammar_dir, &vocab.version));
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

/// The ledger states the vocabulary it was written against, as part of
/// being a standalone provenance record. Nothing checked it, and it had
/// already drifted — every ledger said 0.3.0 while the vocabulary said
/// 0.4.0. A documentation field nobody verifies is a field that lies, so
/// verify it: the ledger is the artifact a consumer reads to find out what
/// this grammar is, and it is worth less than nothing when it is wrong.
fn ledger_vocabulary_finding(grammar_dir: &std::path::Path, expected: &str) -> Option<String> {
    let text = std::fs::read_to_string(grammar_dir.join("ledger.toml")).ok()?;
    let v: toml::Value = toml::from_str(&text).ok()?;
    let stated = v.get("vocabulary")?.as_str()?;
    (stated != expected).then(|| {
        format!("ledger.toml states vocabulary {stated} but treebank-core carries {expected}")
    })
}

fn roles_cmd(grammar_dir: &std::path::Path) -> anyhow::Result<()> {
    let vocab = treebank_core::vocabulary();
    let nt = treebank_core::node_types::NodeTypes::load(&grammar_dir.join("src/node-types.json"))?;
    let roles = treebank_core::roles::RolesManifest::load(&grammar_dir.join("roles.json"))?;
    let mut findings = treebank_core::check::check(&nt, &roles, vocab);
    findings.extend(ledger_vocabulary_finding(grammar_dir, &vocab.version));
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

#[cfg(test)]
mod roles_tests {
    use super::roles_check;
    use std::path::Path;

    #[test]
    fn programmatic_roles_check_includes_the_ledger_gate() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../treebank-python");
        let dir = std::env::temp_dir().join(format!(
            "treebank-roles-ledger-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::copy(
            source.join("src/node-types.json"),
            dir.join("src/node-types.json"),
        )
        .unwrap();
        std::fs::copy(source.join("roles.json"), dir.join("roles.json")).unwrap();
        std::fs::write(dir.join("ledger.toml"), "vocabulary = \"0.0.0\"\n").unwrap();

        let error = roles_check(&dir).unwrap_err().to_string();
        assert!(
            error.contains("ledger.toml states vocabulary 0.0.0"),
            "unexpected error: {error}"
        );

        std::fs::remove_dir_all(dir).unwrap();
    }
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

fn lock_path(lang: LangName, given: Option<PathBuf>) -> PathBuf {
    given.unwrap_or_else(|| PathBuf::from("corpus-locks").join(format!("{}.json", lang.as_str())))
}

/// Rayon workers get the platform default stack (2 MiB), and two of the
/// recursive descents that run on them are unbounded in the corpus rather
/// than in our code: `syn`'s visitor over a deeply nested Rust expression,
/// and `serde_json`'s over a deeply nested oracle record. 2 MiB is enough
/// for almost every file, which is the bad case -- the overflow depends on
/// how rayon happened to nest stolen tasks, so it shows up as an
/// intermittent abort on a corpus that passed an hour earlier rather than
/// as a reproducible failure on one file. Ask for room once, here, so every
/// command gets it. This is address space, not resident memory.
const WORKER_STACK: usize = 64 * 1024 * 1024;

fn main() -> anyhow::Result<()> {
    rayon::ThreadPoolBuilder::new()
        .stack_size(WORKER_STACK)
        .build_global()
        .context("configure the rayon worker pool")?;

    match Cli::parse().cmd {
        Cmd::Rank { lang, db, k, out } => treebank_corpus::rank::run(
            treebank_corpus::get(lang),
            &lang_path(lang, db, "db"),
            k,
            &lang_path(lang, out, "top-k.json"),
        ),
        Cmd::Fetch {
            lang,
            list,
            limit,
            corpus,
            lock_out,
        } => treebank_corpus::fetch::run(
            treebank_corpus::get(lang),
            &lang_path(lang, list, "top-k.json"),
            limit,
            &lang_path(lang, corpus, ""),
            lock_out.as_deref(),
        ),
        Cmd::Hydrate { lang, lock, corpus } => treebank_corpus::fetch::hydrate(
            treebank_corpus::get(lang),
            &lock_path(lang, lock),
            &lang_path(lang, corpus, ""),
        ),
        Cmd::Shape {
            lang,
            grammar,
            manifest,
            out,
            limit,
            dir,
        } => shape::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/shape.json"),
            limit,
            dir.as_deref(),
        ),
        Cmd::Errors {
            lang,
            grammar,
            manifest,
            out,
            limit,
        } => errpos::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/errors.json"),
            limit,
        ),
        Cmd::Reformat {
            lang,
            grammar,
            manifest,
            out,
            limit,
        } => reformat::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            limit,
            &out.unwrap_or_else(|| reformat::default_out(lang)),
        ),
        Cmd::Incremental {
            lang,
            grammar,
            manifest,
            out,
            limit,
            seed,
        } => incremental::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            limit,
            seed,
            &out.unwrap_or_else(|| incremental::default_out(lang)),
        ),
        Cmd::Recovery {
            lang,
            grammar,
            manifest,
            out,
            limit,
        } => recovery::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            limit,
            &out.unwrap_or_else(|| recovery::default_out(lang)),
        ),
        Cmd::Kinds {
            lang,
            grammar,
            manifest,
            out,
            limit,
        } => kinds::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            limit,
            &out.unwrap_or_else(|| kinds::default_out(lang)),
        ),
        Cmd::Fuzz {
            lang,
            grammar,
            out,
            iterations,
            seed,
            unguided,
            rare,
        } => fuzz::run(
            lang,
            &grammar,
            iterations,
            seed,
            unguided,
            rare,
            &out.unwrap_or_else(|| fuzz::default_out(lang)),
        ),
        Cmd::Mutate {
            lang,
            grammar,
            manifest,
            out,
            files,
            per_file,
            seed,
        } => mutate::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/mutate.json"),
            files,
            per_file,
            seed,
        ),
        Cmd::Roundtrip {
            lang,
            grammar,
            manifest,
            out,
            limit,
        } => roundtrip::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/roundtrip.json"),
            limit,
        ),
        Cmd::Sweep {
            lang,
            grammar,
            manifest,
            out,
        } => sweep::run(
            lang,
            &grammar,
            &lang_path(lang, manifest, "manifest.json"),
            &lang_path(lang, out, "reports/sweep.json"),
        ),
        Cmd::Lint { grammar } => lint::run(&grammar),
        Cmd::Roles { grammar } => roles_cmd(&grammar),
        Cmd::Rosetta { dir, crates } => rosetta::run(&dir, &crates),
        Cmd::Verify {
            grammar,
            crates,
            rosetta,
        } => verify::run(&grammar, &crates, &rosetta),
        Cmd::Negative { grammar, dir } => sweep::negative(&grammar, &dir),
        Cmd::Oracle { lang, srcroot } => oracle_cmd(lang, &srcroot),
    }
}
