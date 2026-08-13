# Local patches — treebank-toml

Upstream:
[tree-sitter-grammars/tree-sitter-toml](https://github.com/tree-sitter-grammars/tree-sitter-toml)
pinned at `64b56832c2cffe41758f28e05c756a3a98d16f41` (v0.7.0).

Two patches so far, both packaging. No grammar fix has landed yet; the
known defects are recorded in `ledger.json` under
`grammar_is_narrower_than_the_oracle` and `grammar_accepts_invalid` and are
the queue for the patch series.

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md` so anyone who encounters a
materialized or published copy knows it is a generated redistribution and
where to report problems. Touches no grammar code, applies first.

## 0002 — treebank crate identity

Upstream publishes as `tree-sitter-toml-ng` — note the suffix; it does *not*
own `tree-sitter-toml` on crates.io, which belongs to a third party. The
redistribution publishes as `treebank-grammar-toml`, with treebank's
`repository`, `homepage` and `description`, and `include` extended so
`ledger.json`, `LOCAL-PATCHES.md` and `patches/` travel inside the published
tarball.

`[lib] name` is pinned to `tree_sitter_toml_ng` so the crate stays a drop-in
replacement. This one matters more here than for most grammars: upstream
declares no `[lib] name` at all, so cargo derives it from the package name —
and renaming the package would silently rename the library too, breaking
every `use tree_sitter_toml_ng::LANGUAGE`. Verified by building: the crate
compiles as `treebank-grammar-toml` and emits `libtree_sitter_toml_ng.rlib`.
