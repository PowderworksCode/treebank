# treebank-sdf3

The spike behind `notes/metagrammar.md` §11: can SDF3, as Spoofax documents
it, be adopted as treebank's meta-grammar and lowered to tree-sitter without
losing what it says?

Three pieces:

- **`src/parse.rs`** — a reader for SDF3 modules, written with winnow.
  Sections, both production forms (productive and template), `{Elem Sep}*`
  lists, character classes with SDF's escaping, attributes, priority chains
  with associativity groups, restrictions, template options. What it does
  not understand it refuses, loudly.
- **`src/lower.rs`** — the lowering to a tree-sitter `grammar.json`, and a
  `Finding` for every place the lowering is not exact. Sorts become
  supertypes; constructors become named nodes; injections become supertype
  members with no node; priority chains become `prec.left` levels;
  `template options` become `word` plus `reserved`; LAYOUT becomes `extras`.
- **`spike/mini/`** — a small imperative language in `mini.sdf3`, the
  generated `grammar.json`, a readable `grammar.js`, `findings.md`, the
  generated parser under `src/`, and `test/corpus/mini.txt` — expectations
  written from the SDF3 semantics, which `tree-sitter test` then holds the
  generated parser to.

## Running it

```sh
# lower mini.sdf3 -> grammar.json, grammar.js, findings.md (committed)
cargo run -p treebank-sdf3 --example lower -- crates/treebank-sdf3/spike/mini/mini.sdf3

# the committed output is what the reader and lowering produce
cargo test -p treebank-sdf3

# generate the parser and hold it to the expectations (needs tree-sitter 0.26.12)
crates/treebank-sdf3/spike/mini/verify.sh
```

## What it found

Everything in `findings.md`, in short: the SDF3 semantics survive the trip
except in three named places. Non-associativity has no tree-sitter form and
lowers to `prec.left` (a widening). A `{bracket}` production cannot be a
hidden supertype member, because tree-sitter requires such a member to have
exactly one visible child and `( Exp )` has three, so brackets become a named
node SDF3's AST does not have (a deviation). And SDF3 has no field labels, so
the reader accepts `<left:Exp>` as a treebank extension.

One treebank extension, one tree-sitter constraint, one true widening. The
rest lowered exactly, and `notes/metagrammar.md` §13 records the numbers.

## What it is not

Not a grammar crate. There is deliberately no `grammar.js` at this crate's
root, because a `grammar.js` under `crates/treebank-*/` is the repository's
definition of a shipped grammar (`tools/wasm-pack/list-grammars.sh`,
`treebank status`, the site build). The generated parser lives under
`spike/mini/` and nothing gates on it but `verify.sh`.
