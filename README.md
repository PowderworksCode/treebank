# Treebank

Tooling that keeps tree-sitter grammars at 100% coverage for real-world code.
See [DESIGN.md](DESIGN.md) for the design and [GRAMMARS.md](GRAMMARS.md) for
the vendoring contract every grammar follows (no embedded git, ledger-pinned
upstream + CLI, `patches/` as the offer to upstream maintainers). Current
grammars:

- `crates/treebank-rust` — tree-sitter-rust 0.24.2, 4 patches
- `crates/treebank-typescript` — tree-sitter-typescript 0.23.2 (typescript +
  tsx grammars), no patches yet

## The loop

Every corpus command takes `--lang <rust|typescript>` (default `rust`) and
keeps its data under `corpus/<lang>/`.

```sh
cargo build --release
alias tb=./target/release/treebank

# Corpus
#   rust:       needs the extracted crates.io db dump CSVs in corpus/rust/db/
#   typescript: pulls the npm-high-impact download ranking, resolves versions
#               from the npm registry; .tsx routes to the tsx grammar
tb rank  --lang rust --k 1000
tb fetch --lang rust --limit 100

# Sweep: parses everything, adjudicates failures with the reference parser
# (rust: syn; typescript: tools/ts-oracle), and writes
# corpus/<lang>/reports/sweep.json + an agent-ready REPORT.md.
tb sweep --lang rust       --grammar crates/treebank-rust
tb sweep --lang typescript --grammar crates/treebank-typescript

# Grammar-side verification (reconstruct + corpus tests + negative corpus).
# One generic script, driven by each grammar's ledger.json; CI runs the same
# thing per grammar (.github/workflows/verify-grammars.yml).
scripts/verify.sh crates/treebank-rust
scripts/verify.sh crates/treebank-typescript
```

`tb negative --grammar <dir> --dir <dir>` asserts every file in a directory
FAILS to parse — the accepts-invalid-code direction sweeps can't catch.

## Fixing gaps with an agent

`REPORT.md` is the handoff: it lists each gap cluster (valid code the
grammar rejects) with repro snippets, the exact files that must pass, and
the fix/verify workflow. Point an agent at it:

```sh
claude "Read corpus/rust/reports/REPORT.md and fix gap cluster 1, following
the report's instructions. Work in crates/treebank-rust."
```

`scripts/check.sh` (run from the grammar dir) is the agent's one-command
verifier: regenerate with the pinned CLI, corpus tests, sweep-beats-baseline,
negative corpus — `CHECK OK` or `CHECK FAILED`.

## The daily checker

`scripts/daily.sh` is the cron entrypoint: for every vendored grammar it
re-fetches the corpus (npm re-resolves latest versions, so new releases
arrive on their own), sweeps, and — only when the report shows grammar
gaps — launches one fix agent for that language, then re-sweeps and
verifies. If verify passes and the grammar changed, the script commits the
fixes on a `grammar-fixes/<lang>-<date>` branch and opens a PR (the agent
never touches git; merging stays human; verify failures stay in the working
tree, unpushed).

```
0 6 * * * cd /Users/zackmaril/powderworks/treebank && scripts/daily.sh >> daily.log 2>&1
```

Sweeps are incremental: `corpus/<lang>/sweep-cache.json` remembers which
file hashes passed under the current grammar build (fingerprinted from the
compiled parser sources), so a daily run only parses new or changed files —
a no-change sweep is milliseconds. Any grammar change invalidates the whole
cache and forces a full re-sweep.

## Current status (2026-08-06, top-100 packages per ecosystem)

| corpus | grammar | passed | failed |
|---|---|---:|---:|
| crates.io top-100 (4,626 files) | upstream tree-sitter-rust 0.24.2 | 4,605 | 21 |
| crates.io top-100 (4,626 files) | treebank-rust (4 patches) | **4,613** | **13** |
| npm top-100 (680 files) | treebank-typescript 0.23.2 | **673** | **7** |

Every remaining failure is oracle-confirmed valid code (zero corpus noise in
both top-100s). The gap clusters are queued as jobs in `jobs/queue/`.
