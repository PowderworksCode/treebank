# treebank-yaml local patches

Upstream is
[tree-sitter-grammars/tree-sitter-yaml](https://github.com/tree-sitter-grammars/tree-sitter-yaml),
pinned in `ledger.json` at `a1c4812a` — v0.7.2 plus two unreleased scanner
commits. `ledger.json` says why that pin rather than the tag, and records that
the two commits change 0 of 3217 corpus verdicts on this machine.

Two patches, both `"kind": "packaging"`. **There is no parser patch yet**, and
the sweep that would justify one is in `ORACLE.md`: on 2,178 admitted corpus
files the grammar fails 4, of which 3 are files the oracle also rejects. The
remaining one is a real gap and is diagnosed there rather than fixed here.

## 0001 — treebank redistribution notice

Prepends the standard warning to upstream's `README.md`: this tree is an
automatically generated, patched redistribution maintained by
[treebank](https://treebank.dev), so anyone who meets a materialized or
published copy knows what it is and where to report problems. Touches no
grammar code and applies first.

## 0002 — treebank crate identity

Upstream owns `tree-sitter-yaml` on crates.io, so the published crate is
`treebank-grammar-yaml` with our `repository`, `homepage` and a description
that says it is a patched redistribution rather than an upstream release.
`include` is extended so `ledger.json`, `LOCAL-PATCHES.md` and `patches/`
travel inside the tarball. `[lib] name` is pinned to upstream's
`tree_sitter_yaml`, so the crate stays a drop-in replacement.

Applies last, as the contract requires, and will be renumbered if a parser
patch lands before it.
