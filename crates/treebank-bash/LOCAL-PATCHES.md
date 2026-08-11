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

## 0003 — case patterns with more than three concatenated parts

`_extglob_blob` accepted an extglob pattern, optionally followed by one
string/expansion/command-substitution and one further extglob pattern — and
nothing longer. autoconf emits a five-part pattern in every `configure` it
generates:

```sh
case $ac_user_opts in
  *"
"enable_$ac_useropt"
"*) ;;
```

which is extglob + string + word + expansion + string + extglob and had no
path through the grammar at all. The tail becomes a `repeat` over the same
set plus `word`, `raw_string` and `simple_expansion`. The tree shape does not
change: `_extglob_blob` already emitted more than one `value` field.

**392 files** on the Debian corpus (943 → 576 gaps), **zero** on GitHub —
autoconf is a distribution phenomenon, and this patch is the clearest
evidence in the ledger that the two artifact corpora are different
populations.

## 0004 — substring expansion with a variable offset

`_expansion_max_length` lists what may appear either side of the `:` in
`${var:offset:length}`. The *second* position already allowed a
`simple_expansion`, so `${a:1:$n}` parsed; the first did not, so
`${a:$offset:1}` and `${arr[@]:$keep_count}` had no path. One line, making
the two positions symmetric.

16 files on Debian, 41 on GitHub — the one patch here that both populations
wanted.

## 0005 — for loop with no word list

`for_statement` required a terminator between the loop variable and the
`do`. bash requires one only when there is a word list: `for i do ... done`
is valid and `for i in a b do ... done` is not. The terminator moves inside
the `in` branch. `for i; do`, `for i`+newline+`do` and `for i in a b; do` are
unchanged, and `for i in a b do` is still rejected — checked explicitly,
because this rule also feeds `select` and a widening here is exactly where an
accepts-invalid regression would hide.

38 files on Debian, 5 on GitHub.
