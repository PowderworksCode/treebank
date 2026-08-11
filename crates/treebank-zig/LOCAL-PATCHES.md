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

## 0003 — Zig 0.15: async/await as identifiers, and struct-literal asm clobbers

Two halves of one release. Zig 0.15 removed `async` and `await` as keywords,
so from that release on they are ordinary identifiers and may name a
declaration, a struct field, an enum member, an initializer field or a value.
Separately, 0.15 changed `asm` clobbers from a string list to a struct
literal: `::: "memory"` became `::: .{ .memory = true }`.

**The keyword rules are kept.** `async_expression` and `await_expression` are
untouched, so `async g()` and `await frame` still parse to their own nodes.
The lexer decides on lookahead — an operand follows, it is the keyword form;
nothing follows, it is a name — so no conflict declaration is needed, and a
corpus test pins both readings.

Keeping them is the whole point of the patch, and the reason is worth
recording because the alternative *scores better*. Deleting the `async` and
`await` keywords outright — what a grammar targeting only 0.15+ would do —
measured over the same 45,242 files gives **113 gap files against this
patch's 119**. It was rejected anyway: `gap_files` fell by 31 while
`noise_files` **rose by 27**. Those 27 are files the grammar previously
parsed correctly that now produce error trees, and because the 0.16.0 oracle
also rejects pre-0.15 async source, every one is booked as corpus noise
rather than a regression. `lithdew/pike`'s `await self.frame;` is real Zig
that real repositories are still written in.

The pinned-oracle metric cannot see that class of regression. A sweep that
only ever asks "did gap_files go down" would take the change.

232 files closed, `noise_files` unchanged at 136.

## 0004 — a pointer to an if type expression

`pointer_type` admitted only `$.type_expression` as its pointee, and
`if_type_expression` is a sibling of that rule rather than a member of it. So
`*if (builtin.link_libc) c_int else u32` — ordinary conditional-type code,
used in std and ghostty for platform-varying fields — had no path, and the
parser fell through to a range expression looking for `..`.

One file, zero regressions, and reported as measured: the
`range_expression > MISSING ..` cluster it came from is heterogeneous, and
the 22 files still under that signature are a different shape.
