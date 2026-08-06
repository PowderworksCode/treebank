# The grammar vendoring contract

Every vendored grammar under `crates/treebank-<lang>/` follows the same
rules. Per-grammar facts (upstream pin, patch list) live in that grammar's
`LOCAL-PATCHES.md` and `ledger.json`; everything below is common.

## Layout

A grammar repo carries **no git history**. Upstream is identified purely by
the ledger's `git_url` + `sha`; our entire divergence is the `patches/`
directory, applied in order. If you maintain the upstream grammar and want
any of our fixes, the patch files apply directly onto the pinned sha — the
patches are the offer, the vendored tree is the product.

`ledger.json` fields:

- `upstream.git_url` / `upstream.sha` / `upstream.version` — the pin.
- `generate_cli` — the exact tree-sitter-cli version used to generate this
  tree (see below; this is load-bearing).
- `generate_dirs` — dirs to run `generate` in, in grammar-routing order
  (default `["."]`; e.g. `["typescript", "tsx"]`).
- `generate_deps` — non-null when generation needs `npm ci` first (grammars
  that import other grammars). Always `npm ci`, never `npm install`: the
  lockfile stays upstream's.
- `patches[]` — one entry per patch file with origin and evidence
  (repro, first-seen package, before/after sweep numbers).

## Reconstruction invariant

```
upstream @ pinned sha
  + patches/ applied in order
  (+ npm ci, when generate_deps is set)
  + tree-sitter generate (pinned CLI) in each generate_dir
  == the vendored tree, byte for byte
```

`scripts/verify.sh crates/treebank-<lang>` checks this (upstream cached
under `~/.cache/treebank/upstream/`), plus the grammar's own corpus tests
and our negative corpus. CI runs the same script per grammar
(`.github/workflows/verify-grammars.yml`).

## Why the CLI version is pinned

Regenerating with a different tree-sitter-cli can silently change parsing
behavior. Found in practice: **0.26.x ships Unicode identifier tables that
wrongly drop some XID_Start chars** (e.g. U+212A KELVIN SIGN, which rustc
accepts), which broke `'K'`-style char literals — the corpus sweep is what
caught it. All grammars pin **0.25.10**. Bumping a pin is treated like a
patch: full sweep, before/after numbers, ledger entry.

## Negative corpus

`test/negative/` holds files the reference parser rejects; the grammar must
KEEP rejecting them. Sweeps only catch rejects-valid-code; this catches
accepts-invalid-code, the direction agents drift when optimizing pass rates.

## Changing a grammar

```sh
cd crates/treebank-<lang>
# edit the grammar source, add a corpus test in test/corpus/
../../scripts/check.sh          # generate (pinned CLI) + tests + sweep + negative
../../scripts/verify.sh .       # full reconstruction check
```

Then capture the change as a `patches/NNNN-*.patch` file (source-of-truth
files only, never generated files) and add its ledger entry.
