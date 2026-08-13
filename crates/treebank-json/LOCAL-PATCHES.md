# Local patches — treebank-json

Upstream:
[tree-sitter/tree-sitter-json](https://github.com/tree-sitter/tree-sitter-json)
pinned at `001c28d7a29832b06b0e831ec77845553c89b56d` (Cargo.toml still reads
`0.24.8`; the pin is seven commits past the `v0.24.8` tag).

Three patches: two packaging, and one grammar fix found by this crate's own
sweep.

The pin is the commit **nvim-treesitter and Helix both pin**, rather than the
tag. Six of the seven commits between them are CI churn; the seventh,
[`46aa487b`](https://github.com/tree-sitter/tree-sitter-json/pull/49), adds
`"LICENSE"` to `Cargo.toml`'s `include` list. A redistribution has to ship its
licence, so pinning the tag would have meant carrying that fix as a local
patch — which is exactly the patch `treebank-lua` carries for exactly that
omission.

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md` so anyone who encounters a
materialized or published copy knows it is a generated redistribution and
where to report problems. Touches no grammar code, applies first.

## 0002 — treebank crate identity

Upstream owns `tree-sitter-json` on crates.io, so the redistribution publishes
as `treebank-grammar-json`, with treebank's repository, homepage and
description. `[lib] name` is pinned to `tree_sitter_json` so the crate stays a
drop-in replacement for upstream's, and `include` gains `LOCAL-PATCHES.md`,
`ledger.json` and `patches/*` so provenance travels inside the published
tarball. `Cargo.lock` gets the matching rename and nothing else.

The published version string is deliberately absent: `publish.sh` derives it
from crates.io at publish time. See `PUBLISHING.md`.

## 0003 — exponent plus sign

**The grammar rejected `1e+1`.**

```js
-      const signedInteger = seq(optional('-'), decimalDigits);
+      const signedInteger = seq(optional(choice('-', '+')), decimalDigits);
```

`signedInteger` is the exponent's, and only the exponent's, so `1e-1` and
`1e1` parsed while `1e+1`, `1E+1` and `0.5e+1` did not. RFC 8259 §6 spells the
exponent `exp = e [ minus / plus ] 1*DIGIT` and ECMA-404 agrees; V8, CPython
and serde_json all accept `1e+1`. This is a plain grammar bug, not a dialect
question, and no upstream issue mentions it (all 63 were checked — the two
number-rule issues, #1 and #18, are about the mantissa). It is a one-token fix
that would apply cleanly upstream and is worth offering as a PR.

The mantissa's sign comes from `decimalIntegerLiteral`'s own `optional('-')`,
so `{"a": +1}` stays invalid — it is in `test/negative/`, along with three
cases added specifically to guard this widening: `1e+` with no digits,
`1E++1`, and `1e+.5`.

Evidence: 5,564 → 5,565 files passing over 5,657 npm `.json` files, gap files
1 → 0, no regressions; upstream's corpus tests 6/6 → 7/7 with the exponent
case added. First seen in `jsonparse@1.3.1`'s own `samplejson/basic.json`,
which packs `0.5e+1`, `2E+1`, `0.8e-0` and `2E10` into a single line — a fair
sample of what a JSON-parser author thinks is worth testing, and a reminder
that the corpus files most likely to exercise a grammar's edges are the ones
written by people implementing the same spec.

### Why a "perfect" grammar had a bug

JSON is treebank's negative control: the grammar was expected to be perfect
and the sweep was expected to find nothing. The first corpus — top 800 npm
packages, 1,426 files — did find nothing, and would have been reported as a
clean run. Widening to 3,000 packages (5,657 files) found this in one file.
76% of that first corpus was `package.json`, a single npm-authored schema of
strings and objects that contains essentially no numbers and no exponents at
all. The corpus axis that mattered was **width, not size**. See
`ledger.json`'s `negative_control`.
