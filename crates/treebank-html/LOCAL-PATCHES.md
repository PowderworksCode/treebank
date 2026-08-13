# treebank-html local patches

Upstream: [tree-sitter/tree-sitter-html](https://github.com/tree-sitter/tree-sitter-html)
pinned at `73a3947324f6efddf9e17c0ea58d454843590cc0`.

Two patches, both packaging. **There is no grammar patch yet**, and the
reason is worth stating rather than reading as "nothing was found": on this
language the sweep is a weak instrument by construction, and what it found
instead is a queue in the *other* direction. See
[`ledger.json`](ledger.json)'s `accepts_invalid_markup`.

## 0001 — treebank redistribution notice

The standard first patch of every grammar: a warning at the top of upstream's
`README.md` saying that this tree is an automatically generated, patched
redistribution maintained by [treebank](https://treebank.dev), so anyone who
meets a materialized or published copy knows what it is and where to report
problems. Touches no grammar code.

## 0002 — treebank crate identity

The standard last patch. Upstream owns `tree-sitter-html` on crates.io, so the
redistribution publishes under `treebank-grammar-html` with its own
`repository`, `homepage` and `description`, and extends `include` so
`ledger.json`, `LOCAL-PATCHES.md` and `patches/` travel inside the published
tarball. `[lib] name` stays `tree_sitter_html`, so the crate is a drop-in
replacement.

Nothing here corrects upstream's `include` list, unlike lua's equivalent
patch: `LICENSE` reached it in commit `cbb91a0` (PR #117), which is one of the
five commits between the `v0.23.2` tag and the pinned sha — and is part of why
the pin is HEAD rather than the tag.

## Why the pin is HEAD and not the tag

`73a3947` is five commits past `v0.23.2`, and all five are CI bumps, a
`FUNDING.yml` and the `LICENSE` fix — **no grammar change**. It is also the
commit that **both nvim-treesitter and Helix pin**, so the pin follows the
editors rather than the tag. Zed is the odd one out at `bfa075d`, an older
commit.

## What a grammar patch here would have to be

Recorded so the next agent does not start from the wrong end. The sweep on
this language answers "what valid HTML does the grammar reject", and the
oracle's rejection power is deliberately small, so that queue is thin. The
queue that is *not* thin is "what malformed markup does the grammar accept",
found by the negative battery: six classes, each with a repro, listed in
`ledger.json` under `accepts_invalid_markup`. Those are the candidate patches.

Fixing one is a real change to `grammar.js` or `src/scanner.c` and needs the
usual evidence — a corpus test in `build/test/corpus/`, before/after sweep
numbers, and a check that the fix does not make the grammar reject any of the
spec-recovered constructs in `tools/consumer-test/fixtures/patched.html`,
which is the failure mode that matters most here.
