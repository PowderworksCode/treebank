# treebank-yaml local patches

Upstream is
[tree-sitter-grammars/tree-sitter-yaml](https://github.com/tree-sitter-grammars/tree-sitter-yaml),
pinned in `ledger.json` at `a1c4812a` — v0.7.2 plus two unreleased scanner
commits. `ledger.json` says why that pin rather than the tag, and records that
the two commits change 0 of 3217 corpus verdicts on this machine.

Three patches: two `"kind": "packaging"` and one parser fix. On the 26,628-file
ranked corpus the grammar now parses 26,627 (**zero grammar gaps**); the single
remaining failure is a file this grammar's oracle rejects too. `ledger.json`'s
`corpus.sweep_upstream` / `corpus.sweep_patched` hold the before/after.

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

Applies before the parser patches, which is how every other grammar in this
repo is numbered (elixir, lua and toml all keep identity at `0002` and append
fixes from `0003`): renumbering it on every new fix would rewrite the whole
series' filenames and break the ledger's per-patch evidence for no gain.

## 0003 — row counters overflow past line 32768

`src/scanner.c` kept its row state in `int16_t`. At row 32768 `row` wraps
negative, the `blk_imp_row != bgn_row` test that decides whether a
block-implicit key opens a new mapping then compares a wrapped row against an
unwrapped one, and every indentation decision after that point is wrong — so
the rest of the file collapses into one `ERROR` node reaching back to `[0, 0]`.
It is an integer overflow, not a YAML construct: the minimal repro is `a: 1`,
32,767 empty lines, `b: 2`, and the bisect is exact (32,766 filler lines parse
clean, 32,767 do not, 2¹⁵ = 32768). Bytes are not the trigger — a 200 KB
single-line file parses clean.

The patch widens exactly five fields to `int32_t` — `row`, `blk_imp_row`, the
`cur_row`/`end_row` temporaries and the `bgn_row` local that feeds
`blk_imp_row` — and reorders `serialize`/`deserialize` so the two persistent
32-bit fields come first, leaving every field naturally aligned and the header
14 bytes instead of 10. The indent stacks are untouched, so the number of
indent levels that fit in `TREE_SITTER_SERIALIZATION_BUFFER_SIZE` does not
change.

**Columns are still `int16_t`, deliberately.** `col`, `blk_imp_col`,
`blk_imp_tab` and both indent stacks hold columns and would wrap the same way
past column 32767, but nothing in 26,628 corpus files or the 3,217-file
measurement corpus demonstrates it, and fixing it properly means widening the
two indent stacks — which are serialized *per indent level* against a fixed
buffer, so it would halve the nesting depth the scanner can round-trip. That
trade wants evidence, and there is none yet.

This closed all four of the ranked corpus's gap files (72,060 / 49,299 /
49,054 / 35,649 lines — generated Kubernetes CRD bundles), which
`corpus/yaml/reports/REPORT.md` booked as three clusters because the wrap lands
on a different construct in each file. Sweep 26,623 → 26,627 passed, 4 → 0 gap
files.

The corpus test in `test/corpus/99_issues.txt` is 32,769 lines, which makes the
patch 69 KB of almost entirely blank filler. That is the irreducible price of a
regression test for a bug whose trigger is a *line number*; the alternative was
to rely on the four CRD bundles, which live in a gitignored corpus that gets
refetched, so nothing committed would hold the grammar to the fix. The test is
verified to fail on the pinned sha and pass with the patch.

Reported upstream as
[tree-sitter-yaml#49](https://github.com/tree-sitter-grammars/tree-sitter-yaml/issues/49);
retire this patch if it merges.
