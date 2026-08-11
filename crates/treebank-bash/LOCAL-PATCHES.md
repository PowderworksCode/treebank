# Local patches — treebank-bash

Upstream: [tree-sitter/tree-sitter-bash](https://github.com/tree-sitter/tree-sitter-bash)
pinned at `a06c2e4415e9bc0346c6b86d401879ffb44058f7` (v0.25.1, which is also
`master` — upstream's last push was 2025-12-02).

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md` so anyone who encounters a
materialized or published copy knows it is a generated redistribution and
where to report problems. Touches no grammar code, applies first.

## 0002 — treebank crate identity

Upstream owns `tree-sitter-bash` on crates.io, so the redistribution
publishes as `treebank-grammar-bash`, with treebank's repository, homepage
and description. `[lib] name` is pinned to `tree_sitter_bash` so the crate
stays a drop-in replacement for upstream's, and `include` gains
`LOCAL-PATCHES.md`, `ledger.json` and `patches/*` so provenance travels
inside the published tarball. `Cargo.lock` gets the matching rename and
nothing else — dependency versions are upstream's.

The published version string is deliberately absent: `publish.sh` derives it
from crates.io at publish time. See `PUBLISHING.md`.
