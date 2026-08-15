# treebank-yaml local patches

Upstream is
[tree-sitter-grammars/tree-sitter-yaml](https://github.com/tree-sitter-grammars/tree-sitter-yaml),
pinned in `ledger.json` at `a1c4812a` — v0.7.2 plus two unreleased scanner
commits. `ledger.json` says why that pin rather than the tag, and records that
the two commits change 0 of 3217 corpus verdicts on this machine.

Seven patches: two `"kind": "packaging"` and five parser fixes. On the
26,636-file ranked corpus the grammar parses 26,634 with **zero grammar gaps**
(both remaining failures are files this grammar's oracle rejects too), and over
yaml-test-suite `data-2022-01-17` it is **1 accepts-invalid, 0 rejects-valid**
across 402 cases. `ledger.json`'s `corpus.sweep_upstream` /
`corpus.sweep_patched` hold the before/after.

The two populations found different things and neither substitutes for the
other. The corpus sweep found `0003` — a 32k-line overflow no conformance suite
would ever contain — and is blind to all of `0004`–`0006`. The suite found
`0004`–`0007`, which are all the other direction (**accepts-invalid**, the one
GRAMMARS.md says agents drift toward), and three of them move zero corpus files.
`0007` is the one that does both, and it *lowers* the sweep's pass count on
purpose — see below.

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

## 0004 — at most one %YAML directive per document

`_drs_doc` was `seq(repeat1($._s_dir), $._exp_doc)`, so any number of `%YAML`
directives parsed. YAML 1.2.2 §6.8.1 allows **at most one** per document, while
`%TAG` and reserved directives may repeat and may sit on either side of it —
a regular language, so the fix is a restructured choice and needs no new state.

The grammar change alone is a regression, which is why this patch also touches
the scanner. The `'%'` branch of the scanner's dispatch was gated on
`valid_symbols[S_DIR_YML_BGN]` alone, even though `scn_dir_bgn` picks between
all three directive-start symbols by reading the name. Once the grammar stopped
admitting a second `%YAML`, that gate went false and the scanner went blind to
`%` **entirely**, so a `%TAG` after a `%YAML` stopped scanning. The new corpus
test — a `%YAML` followed by two `%TAG`s — is what catches it.

Suite 6 → 5 accepts-invalid. Zero corpus movement: no file in 26,636 carries a
duplicate `%YAML`.

## 0005 — deficient indentation in multi-line double-quoted scalars

The `BR_DQT_STR_CTN` branch read `(is_br || has_nwl)`. `is_br` is the real test
(`has_nwl && leading_spaces > cur_ind`) and `|| has_nwl` waives it, so a
continuation line of a multi-line double-quoted scalar was accepted at **any**
indentation — including column 0 under a block mapping key, and including a
tab-indented line, since `leading_spaces` stops counting at the first tab. Suite
cases QB6E and DK95/01 are one defect, and dropping the waiver fixes both.

The construct upstream added the waiver for still works: at the document root
the required indent is `-1` in this scanner's convention, so `is_br` is already
true and an unindented multi-line scalar scans without it — checked for the
folded form, the escaped-newline form, inside a sequence item, and nested under
a key.

**This patch rewrites one of upstream's own corpus tests.** `99_issues.txt`'s
"Double-quote newline escape (#10)" asserts that `root: "a\` … parses clean,
and measured against this grammar's oracle that input is *invalid* — the same
text with one leading space per continuation is valid. The test is replaced by
two that keep the issue's intent (the same escaped-newline construct unindented
**at the root**, where it is genuinely valid, plus the indented-under-a-key
form) rather than deleted. Expect a conflict here on an upstream bump.

Suite 5 → 3. Zero corpus movement — and the near miss is worth knowing: the
ledger's `what_the_oracle_choice_costs` describes six real corpus files of a
*related* deficient-indentation class (multi-line flow **collections** whose
closing bracket sits in the key's column, which libyaml and go-yaml accept).
Those are a different production and this patch deliberately leaves them alone.

## 0006 — tab used as block scalar indentation

`scn_blk_str_bgn`'s auto-detection loop only advances over `' '`, so a tab fell
into the branch that treats the character as the start of content. With the tab
at column 0 under a mapping at indent 0, the detected indentation came out equal
to the parent's, the block scalar was emitted empty, and the tab line was then
swallowed by the generic whitespace skip — it appeared in **no node at all**.

**The bound is strict (`< cur_ind`) and the off-by-one is the whole patch.** The
suite carries the adjacent valid case: `Y79Y/001` is byte-for-byte `Y79Y/000`
with one leading space, and it is valid, because that space puts the tab past
the parent's indentation where it is ordinary content. Written first as
`<= cur_ind`, which scored 4 rejects-valid (`96NN/00`, `96NN/01`, `R4YG`,
`Y79Y/001`) — every one a tab sitting one column past the parent.

Suite 3 → 2. Zero corpus movement.

## 0007 — leading empty line deeper than the block scalar's content

YAML 1.2.2 §8.1.1.1: it is an error for any of a block scalar's leading empty
lines to contain more spaces than its first non-empty line, because the
auto-detected indentation would then come from whitespace rather than content.
The detection loop raised its running `ind` from every empty line and never
compared it against the first content line. The fix tracks the deepest leading
empty line separately, with a `-2` sentinel so that a scalar with no leading
empty lines, and one that reaches EOF before any content, are both untouched —
the two cases a naive comparison against `ind` breaks, since `ind` starts at the
parent's indentation. The predicate was derived from a 9-case battery checked
against the oracle *before* the fix was written; grammar and oracle now agree
9/9.

**The sweep's pass count goes down by one, and that is the point.** Every other
patch in this repo is judged by that number going up. The file that stops
passing is `Sigmmma/c20`'s `params.yml`, whose line 50 is `values: |-` followed
by a six-space empty line and then content at indent 4; the oracle independently
rejects it for the same reason it rejects S98Z. So it is booked as **noise**,
`gap_files` stays 0, and the grammar's accepts-invalid over the real corpus goes
6 → 5. `scripts/check.sh` gates on `passed > PASS_BEFORE`, which is the right
gate for a gap-closing patch and the wrong shape for this one.

Suite 2 → 1.

## What is left: QLJ7

One suite case remains accepted: a tag shorthand `!prefix!` declared by a `%TAG`
directive in the first document and used in later ones. js-yaml's reason is
"undeclared tag handle" — **name resolution, not syntax**. It needs a
per-document symbol table of declared handles, which a tree-sitter grammar has
no place holding. Left unfixed deliberately rather than left unnoticed.
