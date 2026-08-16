# treebank

Tree-sitter grammars written from scratch and owned outright, carrying a
shared node vocabulary that is enforced in the parse table itself — so
queries like `(_declaration)`, `(_loop)` and `(_callable)` mean the same
thing across languages. Initial languages: **Python, Rust, TypeScript**
(the TypeScript grammar also parses JavaScript).

**[`DESIGN.md`](DESIGN.md) is the authoritative document** — the vocabulary,
its two tiers and the measurements that forced them, the version-union
grammar policy, the testing invariants, and the crate layout. Start there.

## What is in this repo today

The grammars themselves do not exist yet; this tree currently holds the
measurement infrastructure they will be built and validated against:

| path | what it is |
|---|---|
| `DESIGN.md` | the design: vocabulary, invariants, layout, order of work |
| `crates/treebank-cli` | `treebank` — corpus tooling: `rank` (top-K package lists), `fetch` (registry tarballs → source corpus), `sweep` (parse the corpus, adjudicate failures with the language's reference parser), `negative` (assert a directory of invalid files stays rejected), `oracle` (run a reference parser over paths on stdin) |
| `crates/treebank-preprocessing` | dead-branch elimination for languages with a C-style preprocessor, so a parse failure can be judged against the configuration a compiler actually saw (no current target language needs it; kept for the languages that will) |
| `tools/py-oracle`, `tools/ts-oracle`, `tools/js-oracle` | the reference-parser oracles the sweep adjudicates with (CPython `ast.parse`, `tsc`'s parser, V8) |

## Building

```sh
cargo build --workspace
cargo test --workspace
```

`tree-sitter-cli` is pinned at **0.25.10** for all grammar generation; see
DESIGN.md §7 for why the pin is load-bearing.
