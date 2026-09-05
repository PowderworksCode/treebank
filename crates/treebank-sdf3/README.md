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
- **`src/scanner.rs`** — the planner for layout constraints tree-sitter's
  grammar cannot express, and the scanner it generates for them.
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
crates/treebank-sdf3/spike/rubyish/verify.sh
crates/treebank-sdf3/spike/cppish/verify.sh   # also asks generate for the carry's conflicts
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

## The second language: the corner of Ruby where the lexer needs the parser

`spike/rubyish/` is the test mini could not be. The same characters lex
differently by spacing and by what the parser could accept: `foo -1` is a
command call with a negative argument, `foo - 1` and `foo-1` subtract; `foo
*a` splats, `foo * a` multiplies; `foo [1]` passes an array, `foo[1]`
indexes; `foo(1)` calls, `foo (1)` passes a parenthesised argument; `a /b/`
passes a regex, `a / b` divides. CRuby decides these in its lexer with
EXPR_ARG state; treebank's ruby grammar decides them in a hand-written
scanner (`notes/field_guide.md` §1, rung 1).

In SDF3 they are layout constraints on productions — `{layout(1.last.col +
1 == 2.first.col)}` for adjacency, `<` for separation — plus `{prefer}`. A
tree-sitter grammar has no way to say "layout required before this token",
so `src/scanner.rs` **splits** every constrained spelling into scanner-owned
variants (`_minus` and `_minus_spaced_tight`), aliases each back to its
spelling so the tree still shows `-`, and **generates `src/scanner.c`**,
which decides between variants by what the parser could accept first and by
the actual spacing second. That is the shape of treebank-ruby's scanner,
derived from the grammar instead of written.

Twelve expectations written from Ruby's semantics; twelve hold, with zero
conflicts. `verify.sh` there regenerates grammar and scanner together.

## The third language: the ambiguity C++ keeps

`spike/cppish/` is the `carry` intent. `a < b > c;` is either the expression
`(a < b) > c` or a declaration of `c` with type `a<b>`, and nothing short of
a symbol table decides it. SDF3 parses both and `{prefer}` on the template
reading picks it when both survive the statement. treebank-cpp carries the
same ambiguity as declared conflicts and cut template arguments in
*expression* position to keep its table converging; this module keeps them
in type position only, as that grammar does.

Two things are new in the lowering. `cppish.sdf3` **imports** `cish.sdf3`
rather than copying it — SDF3 composition is additive, so `Type` gains one
production and every other rule is cish's; the loader (`load_module`)
merges sections, and a finding says so. And `{prefer}` lowers to dynamic
precedence, which tree-sitter only consults inside a **declared conflict**
— which the lowering cannot compute. So `--generate` asks `tree-sitter
generate`, declares the conflict it names, and pins the set in
`tree-sitter.conflicts.json` beside the module: the carry's backend data,
reproducible without the CLI, diffable when generate's view moves.

Eight expectations from C++'s semantics; eight hold. One declared conflict,
`[template_id, _exp]` — it names a supertype, the early-commit shape the
field guide budgets for. `verify.sh` there also runs the post-condition
`notes/metagrammar.md` §3 asks for: `a < b > c;` must actually fork
(`version_count` peaks at 2) and `x = a < b > c;` must not (it stays at 1,
because after `=` no declaration is possible). `vector<vector<int>> v;`
parses with `>>` a single token in the grammar, because tree-sitter's lexer
only offers the tokens the state accepts.

## The second backend: ANTLR

`src/antlr.rs` lowers the same modules to ANTLR4 grammars (`<Name>.g4`,
Python3 target), with the node names the tree-sitter lowering chose so one
corpus serves both, and `tools/antlr_check.py` generates the parser and holds
it to that corpus, writing `antlr-results.md` beside each spike. Sorts become
rules and constructors labeled alternatives — ANTLR's own supertype/subtype
split — priority chains become alternative order in left-recursive rules,
and layout constraints become lexer token variants with lexer predicates,
from the same plan as the generated scanner.

Across the three spikes, 23 of 29 expectations hold under ANTLR, and every
miss is one of three capability differences the design's table predicted
and the runs now measure: the ANTLR lexer cannot ask the parser what is
valid, so spacing decisions that tree-sitter's scanner settled by validity
are rejected (`(a+b) -1`, `z=-1`, `foo((1))`); it lexes without parser
state, so `>>` is one token and closes no template; and trivia goes to the
hidden channel, so comments are absent where tree-sitter shows extras. One
fact was established by a four-line experiment on the way and is recorded:
ANTLR consults a left-edge semantic predicate during prediction in a plain
rule and not in a left-recursive one, which is why parser predicates could
not carry Ruby's spacing rule.

```sh
pip install antlr4-python3-runtime==4.13.2   # the tool jar is fetched on first use
python3 crates/treebank-sdf3/tools/antlr_check.py crates/treebank-sdf3/spike/rubyish
```

## What it is not

Not a grammar crate. There is deliberately no `grammar.js` at this crate's
root, because a `grammar.js` under `crates/treebank-*/` is the repository's
definition of a shipped grammar (`tools/wasm-pack/list-grammars.sh`,
`treebank status`, the site build). The generated parser lives under
`spike/mini/`, `spike/rubyish/` and `spike/cppish/`, and nothing gates on
them but their `verify.sh` scripts.
