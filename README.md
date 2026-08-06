# Treebank

Tooling that keeps tree-sitter grammars at 100% coverage for real-world code.
See [DESIGN.md](DESIGN.md) for the design and [GRAMMARS.md](GRAMMARS.md) for
the contract every grammar follows: upstream is a pristine `upstream/` git
submodule pinned by `ledger.json`, our entire divergence is `patches/` (the
offer to upstream maintainers), and `scripts/materialize.sh` produces the
gitignored working tree `build/` that everything else consumes. Current
grammars:

- `crates/treebank-rust` — tree-sitter-rust 0.24.2, 12 grammar patches
- `crates/treebank-typescript` — tree-sitter-typescript 0.23.2 (typescript +
  tsx grammars), 2 grammar patches
- `crates/treebank-javascript` — tree-sitter-javascript 0.25.0 (JSX
  included), 2 grammar patches
- `crates/treebank-java` — tree-sitter-java 0.23.5, 2 grammar patches
- `crates/treebank-csharp` — tree-sitter-c-sharp 0.23.5, 1 grammar patch

```sh
git clone --recurse-submodules <repo>   # or: git submodule update --init
scripts/materialize.sh crates/treebank-rust        # -> build/ (sweeps/tests use this)
scripts/materialize.sh crates/treebank-typescript
scripts/materialize.sh crates/treebank-javascript
scripts/materialize.sh crates/treebank-java
scripts/materialize.sh crates/treebank-csharp
```

## Setting up a machine

```sh
# The pinned CLI. 0.25.10 exactly — see GRAMMARS.md for why this is not a
# style choice. verify.sh and check.sh shell out to it.
npm install -g tree-sitter-cli@0.25.10

# Corpus bootstrap: ranks each ecosystem, and for rust downloads the
# crates.io db dump (~1.7 GB, ~2.7 GB of CSVs kept). Run once per machine;
# re-run with TREEBANK_REFRESH_DUMP=1 to pull a newer dump.
scripts/bootstrap.sh
```

Also needed: `cargo`, `node`/`npm`, `jq`, `cc`, `git`, and `gh` authenticated
against github.com (the daily job pushes branches through
`gh auth git-credential`).

## The loop

Every corpus command takes `--lang
<rust|typescript|javascript|java|csharp>` (default `rust`) and keeps its
data under `corpus/<lang>/`.

```sh
cargo build --release
alias tb=./target/release/treebank

# Corpus
#   rust:       needs the extracted crates.io db dump CSVs in corpus/rust/db/
#   typescript: pulls the npm-high-impact download ranking, resolves versions
#               from the npm registry; .tsx routes to the tsx grammar
#   csharp:     ranks NuGet by downloads, then follows each package's nuspec
#               SourceLink metadata to the git commit it was built from —
#               NuGet ships assemblies, not source (see the ledger)
tb rank  --lang rust --k 1000
tb fetch --lang rust --limit 100

# Sweep: parses everything, adjudicates failures with the reference parser
# (rust: syn; typescript: tools/ts-oracle; javascript: tools/js-oracle, which
# is V8 via node's vm plus a JSX-only babel leg — NOT the TypeScript parser,
# which calls `const x: number = 1` valid JavaScript; java: tools/java-oracle,
# javac's own parser via JavacTask.parse), and writes
# corpus/<lang>/reports/sweep.json + an agent-ready REPORT.md.
tb sweep --lang rust       --grammar crates/treebank-rust/build
tb sweep --lang typescript --grammar crates/treebank-typescript/build
tb sweep --lang javascript --grammar crates/treebank-javascript/build
tb sweep --lang java       --grammar crates/treebank-java/build
tb sweep --lang csharp     --grammar crates/treebank-csharp/build

# Grammar-side verification (materialize + corpus tests + negative corpus).
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
0 6 * * * $HOME/treebank/scripts/daily.sh >> $HOME/treebank/daily.log 2>&1
```

The line carries no `cd` and no `PATH`: `daily.sh` cds to its own repo root
and prepends `~/.cargo/bin`, `~/.local/bin` and `/usr/local/bin` itself.
That matters — cron runs with `PATH=/usr/bin:/bin`, which contains none of
`cargo`, `node`, `npx`, `claude` or `tree-sitter`, so a line that relies on
an interactive PATH dies on the first command.

Knobs, all optional:

| env | default | |
|---|---|---|
| `TREEBANK_LIMIT` | `100` | packages fetched per ecosystem |
| `TREEBANK_RANK_K` | `1000` | length of the ranked package list |
| `TREEBANK_AGENT` | `1` | `0` runs fetch/sweep only — no agent, no PR |
| `TREEBANK_AGENT_TIMEOUT` | `3600` | wall-clock seconds per agent session |
| `TREEBANK_AGENT_BUDGET_USD` | `10` | dollar cap per agent session |
| `CLAUDE_BIN` / `CLAUDE_MODEL` | `claude` / `sonnet` | |
| `TREEBANK_LOCK` | `/tmp/treebank-daily.lock` | one run at a time |

Only one run happens at a time (`flock`); if yesterday's agent is somehow
still going, today's run logs that and exits rather than racing it.

Sweeps are incremental: `corpus/<lang>/sweep-cache.json` remembers which
file hashes passed under the current grammar build (fingerprinted from the
compiled parser sources), so a daily run only parses new or changed files —
a no-change sweep is milliseconds. Any grammar change invalidates the whole
cache and forces a full re-sweep.

## Current status (2026-08-06, measured on Linux)

Top-100 per ecosystem — what the daily job sweeps, and what the ledgers'
`corpus.sweep_patched` records:

| corpus | grammar | passed | failed |
|---|---|---:|---:|
| crates.io top-100 (4,626 files) | upstream tree-sitter-rust 0.24.2 | 4,605 | 21 |
| crates.io top-100 (4,626 files) | treebank-rust (12 patches) | **4,613** | **13** |
| npm top-100 (680 .ts files) | treebank-typescript (2 patches) | **680** | **0** |
| npm top-100 (720 .js files) | upstream tree-sitter-javascript 0.25.0 | 718 | 2 |
| npm top-100 (720 .js files) | treebank-javascript (2 patches) | **720** | **0** |
| Maven top-89 (21,049 files) | upstream tree-sitter-java 0.23.5 | 20,993 | 56 |
| Maven top-89 (21,049 files) | treebank-java (2 patches) | **21,049** | **0** |
| NuGet top-100 sources (860,590 files, 50 repos) | upstream tree-sitter-c-sharp 0.23.5 | 849,118 | 11,472 |
| NuGet top-100 sources (860,590 files, 50 repos) | treebank-csharp (1 patch) | **852,917** | **7,673** |

Every remaining rust/typescript/javascript/java failure is oracle-confirmed
valid code (zero corpus noise). The gap clusters are queued as jobs in
`jobs/queue/`.

C# is the exception and its numbers need reading with care. Of its 7,148
oracle-valid failures, 4,617 parse cleanly once the file is reduced to the
configuration Roslyn actually parsed — they fail only because Roslyn
adjudicates the active `#if` branch while tree-sitter parses every branch
into one tree. That class is inherent rather than fixable; see
`crates/treebank-csharp/LOCAL-PATCHES.md`. The actionable queue is the other
**2,531** files.

At the daily job's default `TREEBANK_LIMIT=100` this is a regression detector:
the fix agent fires only for a language whose top-100 sweep reports a gap, so
a clean language costs nothing and a new package release, a new grammar, or a
grammar change that introduces a gap is what wakes it.
