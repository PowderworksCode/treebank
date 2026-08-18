//! `treebank mutate` — does the grammar accept things the language does not?
//!
//! The sweep measures ONE direction. It takes 139,205 real files, asks which
//! ones we reject, and adjudicates each with a reference parser. That is a
//! strong measurement of rejects-valid-code and it says nothing at all about
//! the other direction, because a corpus of real source is almost entirely
//! valid: there is nothing in it for a too-permissive grammar to trip over.
//!
//! The other direction has been measured against `test/negative/` — eighteen
//! hand-written files for python, fourteen for rust, thirteen for typescript.
//! Set against 139,205, that asymmetry is the weakest part of the claim, and
//! it points the wrong way: optimising a pass rate drifts TOWARD accepting
//! more, and the only guard is a list somebody has to think of entries for.
//!
//! Every widening found so far turned up by accident while chasing something
//! else — `{ a: X b: Y }`, annotated lambda parameters, the three files the
//! `or_test` restriction closed. This looks for them on purpose.
//!
//! The method is differential fuzzing, and it does not need the mutants to be
//! reliably invalid. Mutate a real file mechanically, parse it, and ask the
//! oracle only about the ones WE ACCEPT. Where the oracle rejects what we
//! accept, that is a widening — whatever the mutation happened to produce.
//! Mutants both parsers accept are simply uninteresting, and there is no need
//! to know in advance which is which.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};

use treebank_corpus::fetch::Manifest;
use treebank_lang::LangName;

use crate::grammar;

#[derive(Serialize, Deserialize)]
pub struct Widening {
    /// Corpus file the mutant came from.
    pub origin: String,
    /// What was done to it.
    pub mutation: String,
    /// Byte offset of the edit, and the source around it.
    pub offset: usize,
    pub before: String,
    pub after: String,
}

#[derive(Serialize, Deserialize)]
pub struct MutateCluster {
    pub signature: String,
    pub count: usize,
    pub files: usize,
    pub examples: Vec<Widening>,
}

#[derive(Serialize, Deserialize)]
pub struct MutateReport {
    pub lang: String,
    pub grammar: String,
    pub files: usize,
    pub mutants: usize,
    /// Mutants we rejected. Nothing to ask about: rejecting a mutant is
    /// never evidence of a widening.
    pub rejected: usize,
    /// Mutants we accepted and the oracle also accepted. The mutation
    /// happened to produce valid code; uninteresting, and most of these.
    pub agreed: usize,
    /// Mutants we accepted and the oracle rejected. Widenings.
    pub widenings: usize,
    pub clusters: Vec<MutateCluster>,
}

/// xorshift64*, so a run is reproducible from its seed. A fuzzer nobody can
/// re-run is a fuzzer whose findings cannot be confirmed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

/// Leaf spans of our own parse, used as the token boundaries to mutate at.
///
/// Deliberately OUR tokens rather than a byte offset: cutting a file in the
/// middle of an identifier mostly produces a different identifier, which is
/// still valid and teaches nothing. Cutting at a token boundary produces the
/// shapes a grammar actually gets wrong.
fn tokens(root: Node) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut cursor = root.walk();
    let mut recurse = true;
    loop {
        let node = cursor.node();
        if recurse && node.child_count() == 0 {
            if node.end_byte() > node.start_byte() {
                out.push((node.start_byte(), node.end_byte()));
            }
        }
        if recurse && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            recurse = true;
            continue;
        }
        if !cursor.goto_parent() {
            break;
        }
        recurse = false;
    }
    out
}

/// One mechanical edit. Returns the mutated source and a label for it.
fn mutate(src: &[u8], toks: &[(usize, usize)], rng: &mut Rng) -> Option<(Vec<u8>, String, usize)> {
    if toks.len() < 3 {
        return None;
    }
    let i = rng.below(toks.len());
    let (s, e) = toks[i];
    let text = String::from_utf8_lossy(&src[s..e]).into_owned();
    let mut out = Vec::with_capacity(src.len() + 16);
    let label;
    match rng.below(4) {
        // Delete a token. The commonest real typo, and the one a grammar
        // with an over-optional rule swallows.
        0 => {
            out.extend_from_slice(&src[..s]);
            out.extend_from_slice(&src[e..]);
            label = format!("delete {text:?}");
        }
        // Duplicate it. Catches repeats a `repeat()` should not allow.
        1 => {
            out.extend_from_slice(&src[..e]);
            out.extend_from_slice(&src[s..e]);
            out.extend_from_slice(&src[e..]);
            label = format!("duplicate {text:?}");
        }
        // Swap with the next token. Catches order a rule leaves free.
        2 => {
            if i + 1 >= toks.len() {
                return None;
            }
            let (s2, e2) = toks[i + 1];
            if s2 < e {
                return None;
            }
            out.extend_from_slice(&src[..s]);
            out.extend_from_slice(&src[s2..e2]);
            out.extend_from_slice(&src[e..s2]);
            out.extend_from_slice(&src[s..e]);
            out.extend_from_slice(&src[e2..]);
            let next = String::from_utf8_lossy(&src[s2..e2]).into_owned();
            label = format!("swap {text:?} {next:?}");
        }
        // Replace it with another token from the same file, so the
        // replacement is always something the language spells somewhere.
        _ => {
            let j = rng.below(toks.len());
            let (s2, e2) = toks[j];
            if j == i {
                return None;
            }
            out.extend_from_slice(&src[..s]);
            out.extend_from_slice(&src[s2..e2]);
            out.extend_from_slice(&src[e..]);
            let with = String::from_utf8_lossy(&src[s2..e2]).into_owned();
            label = format!("replace {text:?} with {with:?}");
        }
    }
    // A mutation that changes nothing teaches nothing.
    if out == src {
        return None;
    }
    Some((out, label, s))
}

fn snippet(src: &[u8], at: usize) -> String {
    let lo = src[..at.min(src.len())]
        .iter()
        .rposition(|b| *b == b'\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let hi = src[at.min(src.len())..]
        .iter()
        .position(|b| *b == b'\n')
        .map(|i| at + i)
        .unwrap_or(src.len());
    String::from_utf8_lossy(&src[lo..hi.min(src.len())])
        .chars()
        .take(90)
        .collect()
}

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    manifest_path: &Path,
    out: &Path,
    files: usize,
    per_file: usize,
    seed: u64,
) -> Result<()> {
    let manifest = Manifest::load(manifest_path)?;
    let corpus_root = manifest_path.parent().unwrap_or(Path::new("."));
    let corpus_src = corpus_root.join("src");
    let entries = manifest.files();
    anyhow::ensure!(!entries.is_empty(), "empty corpus manifest");

    // A deterministic spread rather than the first N, so the sample is not
    // all one package.
    let step = (entries.len() / files.max(1)).max(1);
    let sample: Vec<_> = entries.iter().step_by(step).take(files).collect();

    // Only mutate files the ORACLE accepts. Without this the whole method is
    // unsound: a file the reference parser already rejects produces mutants
    // it also rejects, and every one of them reads as a widening. The first
    // run of this command reported exactly that — protobuf blobs and py2-only
    // sources, all of them noise wearing a finding's clothes.
    let sample_paths: Vec<String> = sample
        .iter()
        .map(|f| format!("{}/{}", f.pkgdir, f.rel))
        .collect();
    let base = treebank_oracle::get(lang).validate(&corpus_src, &sample_paths)?;
    let chosen: Vec<_> = sample
        .into_iter()
        .zip(&sample_paths)
        .filter(|(_, p)| base.get(*p).copied() == Some(true))
        .map(|(f, _)| f)
        .collect();
    println!(
        "mutate: {} of {} sampled files are valid to the oracle; mutating those",
        chosen.len(),
        sample_paths.len()
    );

    let dirs = crate::routing::grammar_dirs(lang);
    let langs: Vec<tree_sitter::Language> = dirs
        .iter()
        .map(|d| grammar::load(&grammar_dir.join(d)).map(|(l, _)| l))
        .collect::<Result<_>>()?;

    println!(
        "mutate: {} files x {} mutants, seed {} ({})",
        chosen.len(),
        per_file,
        seed,
        grammar_dir.display()
    );

    // Generate and parse. Only the mutants WE ACCEPT are worth an oracle
    // call, which is what keeps this affordable.
    struct Candidate {
        origin: String,
        label: String,
        offset: usize,
        before: String,
        after: String,
        bytes: Vec<u8>,
        ext: String,
    }
    let produced: Vec<(usize, Vec<Candidate>)> = chosen
        .par_iter()
        .enumerate()
        .map(|(fi, f)| -> Result<(usize, Vec<Candidate>)> {
            let rel = format!("{}/{}", f.pkgdir, f.rel);
            let full = corpus_src.join(&rel);
            let Ok(src) = std::fs::read(&full) else {
                return Ok((0, Vec::new()));
            };
            let idx = crate::routing::route(lang, &f.dialect, &f.rel);
            let mut parser = Parser::new();
            parser.set_language(&langs[idx])?;
            let Some(tree) = parser.parse(&src, None) else {
                return Ok((0, Vec::new()));
            };
            // Mutating a file we already fail on tells us nothing about
            // accepting too much.
            if tree.root_node().has_error() {
                return Ok((0, Vec::new()));
            }
            let toks = tokens(tree.root_node());
            let ext = Path::new(&f.rel)
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_else(|| "txt".into());
            let mut rng = Rng(seed ^ ((fi as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
            let mut rejected = 0usize;
            let mut kept = Vec::new();
            for _ in 0..per_file {
                let Some((bytes, label, at)) = mutate(&src, &toks, &mut rng) else {
                    continue;
                };
                if std::str::from_utf8(&bytes).is_err() {
                    continue;
                }
                let Some(t) = parser.parse(&bytes, None) else { continue };
                if t.root_node().has_error() {
                    rejected += 1;
                    continue;
                }
                kept.push(Candidate {
                    origin: rel.clone(),
                    label,
                    offset: at,
                    before: snippet(&src, at),
                    after: snippet(&bytes, at),
                    bytes,
                    ext: ext.clone(),
                });
            }
            Ok((rejected, kept))
        })
        .collect::<Result<_>>()?;

    let rejected: usize = produced.iter().map(|(r, _)| *r).sum();
    let candidates: Vec<Candidate> = produced.into_iter().flat_map(|(_, c)| c).collect();
    println!(
        "mutate: {} accepted by the grammar, {} rejected — asking the oracle about the {} we accepted",
        candidates.len(),
        rejected,
        candidates.len()
    );

    // Write the accepted mutants out and adjudicate them in one batch.
    let scratch = std::env::temp_dir().join(format!("treebank-mutate-{lang}-{seed}"));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch)?;
    let mut paths = Vec::with_capacity(candidates.len());
    for (i, c) in candidates.iter().enumerate() {
        let name = format!("m{i}.{}", c.ext);
        std::fs::write(scratch.join(&name), &c.bytes)
            .with_context(|| format!("write mutant {name}"))?;
        paths.push(name);
    }
    let verdicts = if paths.is_empty() {
        Default::default()
    } else {
        treebank_oracle::get(lang).validate(&scratch, &paths)?
    };

    let mut by_sig: BTreeMap<String, Vec<Widening>> = BTreeMap::new();
    let mut agreed = 0usize;
    for (i, c) in candidates.iter().enumerate() {
        match verdicts.get(&paths[i]).copied() {
            // The oracle rejects what we accept. That is the finding.
            Some(false) => {
                // Cluster by the SHAPE of the mutation, not its text, so
                // `delete ";"` from a thousand files is one entry.
                let sig = c
                    .label
                    .split_once(' ')
                    .map(|(verb, rest)| {
                        let head: String = rest.chars().take(24).collect();
                        format!("{verb} {head}")
                    })
                    .unwrap_or_else(|| c.label.clone());
                by_sig.entry(sig).or_default().push(Widening {
                    origin: c.origin.clone(),
                    mutation: c.label.clone(),
                    offset: c.offset,
                    before: c.before.clone(),
                    after: c.after.clone(),
                });
            }
            Some(true) => agreed += 1,
            // Every mutant must get a verdict. A missing one read as
            // "accepted" would hide a widening, which is the direction this
            // whole command exists to stop flattering.
            None => anyhow::bail!("oracle returned no verdict for mutant {}", paths[i]),
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);

    let mut clusters: Vec<MutateCluster> = by_sig
        .into_iter()
        .map(|(signature, ws)| {
            let files: std::collections::HashSet<&str> =
                ws.iter().map(|w| w.origin.as_str()).collect();
            MutateCluster {
                signature,
                count: ws.len(),
                files: files.len(),
                examples: ws.into_iter().take(3).collect(),
            }
        })
        .collect();
    clusters.sort_by(|a, b| (b.count, b.files).cmp(&(a.count, a.files)));

    let widenings: usize = clusters.iter().map(|c| c.count).sum();
    let report = MutateReport {
        lang: lang.to_string(),
        grammar: grammar_dir.display().to_string(),
        files: chosen.len(),
        mutants: rejected + candidates.len(),
        rejected,
        agreed,
        widenings,
        clusters,
    };

    println!(
        "mutate: {} mutants — {} rejected, {} agreed, {} WIDENINGS in {} cluster(s)",
        report.mutants, report.rejected, report.agreed, report.widenings, report.clusters.len()
    );
    for c in report.clusters.iter().take(15) {
        println!("  {:>5}x {:>4} files  {}", c.count, c.files, c.signature);
        if let Some(e) = c.examples.first() {
            println!("           {}", e.origin);
            println!("           - {}", e.before.trim());
            println!("           + {}", e.after.trim());
        }
    }
    std::fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(out, serde_json::to_string_pretty(&report)?)?;
    println!("mutate: report at {}", out.display());
    Ok(())
}

pub fn default_out(lang: LangName) -> PathBuf {
    PathBuf::from(format!("corpus/{lang}/reports/mutate.json"))
}
