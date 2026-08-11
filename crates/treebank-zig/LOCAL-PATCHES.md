# Local patches — treebank-zig

Upstream: [tree-sitter-grammars/tree-sitter-zig](https://github.com/tree-sitter-grammars/tree-sitter-zig)
pinned at `6479aa13f32f701c383083d8b28360ebd682fb7d` (master HEAD at pin time).

Both patches here are packaging, not grammar.

## Why the pin is a commit on master and not the newest tag

`v1.1.2` is dated **2024-12-21**. Master carries nine further months of work
on top of it, including `feat!: update parser to ABI 15` — the ABI
tree-sitter 0.25 emits, which is the CLI this repository pins — and
`ci: update corpus to Zig 0.15.1`. Pinning the tag would pin a grammar that
predates both the ABI and the language era the oracle adjudicates. Submodule
pointers are commits either way, so nothing is lost by naming the commit
directly; `ledger.json`'s `sha_note` records the reasoning.

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md` so anyone who encounters a
materialized or published copy knows it is a generated redistribution and
where to report problems. Touches no grammar code, applies first.

## 0002 — treebank crate identity

Upstream owns `tree-sitter-zig` on crates.io, so the redistribution publishes
as `treebank-grammar-zig`, with treebank's repository, homepage and
description. `[lib] name` is pinned to `tree_sitter_zig` so the crate stays a
drop-in replacement for upstream's, and `include` gains `LOCAL-PATCHES.md`,
`ledger.json` and `patches/*` so provenance travels inside the published
tarball. `Cargo.lock` gets the matching rename and nothing else — dependency
versions are upstream's.

The published version string is deliberately absent: `publish.sh` derives it
from crates.io at publish time. See `PUBLISHING.md`.

**One mechanical difference from the other grammars.** The `Cargo.lock` hunk
was produced by diffing against upstream's committed blob rather than by
`git -C build diff`. tree-sitter-zig force-tracks `Cargo.lock` while its own
`.gitignore` ignores it, so `materialize.sh`'s throwaway `build/` repo — which
stages with `git add -A`, honouring that `.gitignore` — never tracks the file,
and a diff taken there comes back **empty rather than failing**. `git apply`
is unaffected, because it patches the working tree and not the index, so
`verify.sh` reconstructs the file correctly. Anyone adding a third patch that
touches `Cargo.lock` needs to know this; anyone adding one that does not can
use `git -C build diff` as usual.
