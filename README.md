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
| `crates/treebank-core` | the vocabulary as code and data: `vocabulary/vocabulary.json` (the closed 22-term table tier + 3 facets), `vocabulary/supertypes.js` (the JS face every grammar.js imports, with a generate-time term assert), the `roles.json` manifest schema, the vocabulary-conformance checker behind `treebank roles`, and facet query expansion (`(_callable)` → the concrete alternation) |
| `crates/treebank-lang` | the canonical language names every other crate agrees on |
| `crates/treebank-corpus` | corpus acquisition: `rank` an ecosystem's packages (PyPI, crates.io, npm), `fetch` their tarballs, extract source files, write the manifest sweeps consume — self-contained, with no grammar or oracle knowledge, so it can move out of this repo |
| `crates/treebank-oracle` | reference-parser oracles behind one trait: is this file valid \<language\>? Carries its own oracle programs in `tools/` (CPython `ast.parse`, `syn`, `tsc`'s parser, V8) — equally self-contained and movable |
| `crates/treebank-cli` | `treebank` — the thin binary: `rank` / `fetch` (drivers over treebank-corpus), `sweep` (parse the corpus, adjudicate failures via treebank-oracle), `negative` (assert a directory of invalid files stays rejected), `oracle` (run a reference parser over paths on stdin), plus the grammar-routing knowledge that belongs to neither library |
| `crates/treebank-preprocessing` | dead-branch elimination for languages with a C-style preprocessor, so a parse failure can be judged against the configuration a compiler actually saw (no current target language needs it; kept for the languages that will) |

## Building

```sh
cargo build --workspace
cargo test --workspace
```

`tree-sitter-cli` is pinned at **0.25.10** for all grammar generation; see
DESIGN.md §7 for why the pin is load-bearing.
