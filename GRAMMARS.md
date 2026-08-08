# The grammar contract

Every grammar under `crates/treebank-<lang>/` follows the same rules.
Per-grammar facts (upstream pin, patch list) live in that grammar's
`LOCAL-PATCHES.md` and `ledger.json`; everything below is common.

## Layout

```
crates/treebank-<lang>/
├── upstream/          git submodule -> the upstream grammar repo, pinned
│                      at ledger.json's upstream.sha; ALWAYS pristine
├── patches/           our entire divergence, applied in order;
│                      0001 is always the treebank redistribution notice
├── test/negative/     files the reference parser rejects (ours)
├── ledger.json        the pin + per-patch evidence
├── LOCAL-PATCHES.md   human-readable patch descriptions
└── build/             gitignored; materialized working tree
```

Nothing generated is committed. The committed artifacts are the submodule
pointer and `patches/` — the grammar and the patches move independently: a
submodule bump touches no patch, a patch edit touches no pointer, and every
treebank commit is a coherent (sha, patch series) pair. If you maintain the
upstream grammar and want any of our fixes, the patch files apply directly
onto the pinned sha — the patches are the offer, `build/` is the product.

Patches must never be applied inside `upstream/` — anything written there
belongs to the upstream repo's object store, not ours. `materialize.sh`
refuses to run if the submodule is dirty or off the pinned sha.

`ledger.json` fields:

- `upstream.git_url` / `upstream.sha` / `upstream.version` — the pin. The
  sha MUST equal the submodule pointer; materialize/verify assert it.
- `generate_cli` — the exact tree-sitter-cli version used to generate
  `build/` (see below; this is load-bearing).
- `generate_dirs` — dirs to run `generate` in, in grammar-routing order
  (default `["."]`; e.g. `["typescript", "tsx"]`).
- `generate_deps` — non-null when generation needs `npm ci` first (grammars
  that import other grammars). Always `npm ci`, never `npm install`: the
  lockfile stays upstream's.
- `patches[]` — one entry per patch file with origin and evidence
  (repro, first-seen package, before/after sweep numbers).

## Materialization invariant

```
upstream/ submodule @ exactly the ledger's pinned sha, pristine
  + patches/ applied in order
  (+ npm ci, when generate_deps is set)
  + tree-sitter generate (pinned CLI) in each generate_dir
  -> build/
```

`scripts/materialize.sh crates/treebank-<lang>` produces `build/`; it fails
if the submodule pointer disagrees with the ledger, the submodule is dirty,
any patch does not apply, or generate errors. `scripts/verify.sh
crates/treebank-<lang>` runs materialize plus the grammar's own corpus
tests (in `build/`) and our negative corpus. CI runs the same script per
grammar (`.github/workflows/verify-grammars.yml`, submodule checkout).

`build/` is a throwaway git repo with one commit, so after editing grammar
sources there `git -C build diff` is exactly the next patch.

## Why the CLI version is pinned

Regenerating with a different tree-sitter-cli can silently change parsing
behavior. Found in practice: **0.26.x ships Unicode identifier tables that
wrongly drop some XID_Start chars** (e.g. U+212A KELVIN SIGN, which rustc
accepts), which broke `'K'`-style char literals — the corpus sweep is what
caught it. All grammars pin **0.25.10**. Bumping a pin is treated like a
patch: full sweep, before/after numbers, ledger entry.

## The redistribution notice

Patch `0001` of every grammar prepends a warning to upstream's `README.md`:
this tree is an automatically generated, patched redistribution maintained
by [treebank](https://treebank.dev) — so anyone who encounters a
materialized or published copy knows what they are looking at and where to
report problems. It applies first and touches no grammar code.

## Negative corpus

`test/negative/` holds files the reference parser rejects; the grammar must
KEEP rejecting them. Sweeps only catch rejects-valid-code; this catches
accepts-invalid-code, the direction agents drift when optimizing pass rates.

## Changing a grammar

```sh
../../scripts/materialize.sh crates/treebank-<lang>   # or verify.sh once
cd crates/treebank-<lang>
# edit grammar sources in build/, add a corpus test in build/test/corpus/
../../scripts/check.sh          # generate (pinned CLI) + tests + sweep + negative
git -C build diff -- <source files> > patches/NNNN-<title>.patch
../../scripts/verify.sh .       # patches apply from scratch + tests + negative
```

Patches capture source-of-truth files only (grammar.js, scanner.c, corpus
tests), never generated files — `materialize.sh` regenerates those. Add the
ledger entry with evidence, and describe the patch in `LOCAL-PATCHES.md`.

## Bumping upstream

```sh
git -C crates/treebank-<lang>/upstream fetch origin
git -C crates/treebank-<lang>/upstream checkout <new-sha>
# update ledger.json's upstream.sha/version to match
../../scripts/verify.sh crates/treebank-<lang>   # patches must still apply
```

Then commit the pointer + ledger together, with a fresh sweep for evidence.
