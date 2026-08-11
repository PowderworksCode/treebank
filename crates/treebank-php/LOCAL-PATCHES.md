# Local patches — tree-sitter-php

Upstream: <https://github.com/tree-sitter/tree-sitter-php> pinned at
`5b5627faaa290d89eb3d01b9bf47c3bb9e797dea` (tag `v0.24.2`).
`ledger.json` is the machine-readable record; this file is the prose.

## Why the pin is a year-old tag rather than master

Upstream master carries three grammar-source commits since `v0.24.2`, two of
them features this corpus wants — PHP 8.4 asymmetric visibility in
constructor promotion, and PHP 8.5 partial function application (`f(?, 2)`) —
plus a fix for undefined behaviour in the scanner's heredoc handling. Master
was the obvious pin, and it does not work here.

Between the tag and master upstream landed `feat!: regenerate with ABI 15 and
Array aliasing changes`, which moves the grammar onto tree-sitter-cli 0.26.x.
Treebank pins `generate_cli` at **0.25.10** on purpose (0.26.x ships Unicode
identifier tables that wrongly drop some XID_Start characters — see the rust
ledger). Generated with our pinned CLI, upstream master fails **its own**
corpus tests:

| pin | generated with 0.25.10 | generated with 0.26.12 |
|---|---|---|
| `v0.24.2` (Aug 2025) | all 142 parses pass | — |
| master `3fda2fb` (Jul 2026) | **2 failures** — "Asymmetric Visibility in Constructor Promotion", "Partial function application" | all pass |

So the two features master adds are exactly the two its tests cannot
reproduce under our CLI. Pinning master would mean shipping a grammar whose
upstream test suite is red, and claiming support for constructs that do not
actually parse. The tag is pinned instead, and the two features are recorded
as known gaps to be patched onto the tag or picked up when the CLI pin moves.

This is the first grammar in treebank where upstream has moved past the
pinned CLI. It is the same class of decision the roadmap anticipated for Lua
— that the toolchain version is a dialect choice, not bookkeeping — arriving
one language earlier than expected.

## The patches

### 0001 — treebank redistribution notice (packaging)

Prepends the standard warning to `README.md`: this tree is an automatically
generated, patched redistribution maintained by
[treebank](https://treebank.dev), not the upstream project. Applies first,
touches no grammar code.

### 0002 — treebank crate identity (packaging)

Upstream owns `tree-sitter-php` on crates.io, so the redistribution publishes
as `treebank-grammar-php` with its own `repository`, `homepage` and
`description`, and `include` gains `ledger.json`, `LOCAL-PATCHES.md` and
`patches/*` so provenance travels inside the published tarball.

`[lib] name` is pinned to `tree_sitter_php`, which matters more here than it
did for other grammars: upstream's `Cargo.toml` declares no `[lib] name`, so
Cargo derives it from the package name. Renaming the package alone would have
silently renamed the library to `treebank_grammar_php` and broken every
consumer's `tree_sitter_php::LANGUAGE_PHP`. `Cargo.lock` carries the matching
rename and nothing else; dependency versions stay upstream's.
