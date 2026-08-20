# treebank

Tree-sitter grammars written from scratch and owned outright, carrying a
shared node vocabulary that is enforced in the parse table itself — so
queries like `(_declaration)`, `(_loop)` and `(_callable)` mean the same
thing across languages. Initial languages: **Python, Rust, TypeScript**
(the TypeScript grammar also parses JavaScript).

**[`DESIGN.md`](DESIGN.md) is the authoritative document** — the vocabulary,
its two tiers and the measurements that forced them, the version-union
grammar policy, the testing invariants, and the crate layout. Start there.

## What is in this repo

| path | what it is |
|---|---|
| `DESIGN.md` | the design: vocabulary, invariants, layout, order of work |
| `crates/treebank-python` | Python 2.7 ∪ 3.x in one grammar |
| `crates/treebank-rust` | Rust editions 2015–2024 in one grammar |
| `crates/treebank-typescript` | TypeScript ∪ JavaScript ∪ JSX in one grammar |
| `crates/treebank-java` | Java 8 through 21 in one grammar |
| `crates/treebank-bash` | GNU bash 5.x in one grammar |
| `crates/treebank-zig` | Zig 0.11 through 0.16 in one grammar |
| `crates/treebank-sql` | SQLite ∪ PostgreSQL ∪ MySQL in one grammar — the dialect union, which is what "one grammar per language across versions" means for SQL |
| `crates/treebank-core` | the vocabulary as code and data: the closed term lists, the `roles.json` facet schema, the conformance checker behind `treebank roles`, and facet query expansion |
| `crates/treebank-lang` | the canonical language names every other crate agrees on |
| `crates/treebank-corpus` | corpus acquisition: rank an ecosystem's packages, fetch, extract, write the manifest sweeps consume — self-contained so it can move out of this repo |
| `crates/treebank-oracle` | reference-parser oracles behind one trait, carrying their own oracle programs |
| `crates/treebank-cli` | `treebank` — `rank` · `fetch` · `sweep` · `negative` · `roles` · `rosetta` · `oracle` |
| `crates/treebank-preprocessing` | dead-branch elimination for C-family preprocessors (no current target needs it; kept for the languages that will) |
| `test/rosetta` | the same program in every owned language, with the role counts all three must produce |

Each grammar crate ships its `roles.json` and `ledger.toml` inside the
published package, so a consumer gets the facet membership and the
evidence — versions covered, pinned oracles, corpus numbers, known gaps,
declared deviations — without fetching anything.

## Using a grammar

```rust
let mut parser = tree_sitter::Parser::new();
parser.set_language(&treebank_python::LANGUAGE.into())?;
```

Table-tier roles are queryable straight from the parser, because they are
real supertypes in the parse table:

```scheme
(_declaration) @decl
(_loop) @loop
(function_definition name: (_name) @name)
```

Facet-tier roles (`_callable`, `_binding`, `_scope`, `_clause`) cross-cut
derivations, so they cannot be supertypes; they ship as `ROLES` and are
expanded before the query runs.

## What is checked, on every change

Run them all for one grammar with `treebank verify crates/treebank-<lang>`.

| gate | what it catches |
|---|---|
| reproducible generation | committed `src/` drifting from `grammar.js` at the pinned CLI |
| corpus tests | tree *shape* regressions, not just accept/reject |
| negative corpus | accepts-invalid-code — the direction optimizing a pass rate drifts toward, and the one no corpus of real source can reveal |
| `treebank roles` | vocabulary conformance: closed lists, total node coverage, containments, manifest validity |
| `treebank rosetta` | a role threaded in one grammar and forgotten in another (supertype matching is derivation-based, so a missed thread is otherwise silent) |
| wasm build | a grammar that cannot cross to wasm — caught here, not in a consumer's browser |

Corpus sweeps are not in CI: the corpora are gigabytes and gitignored.
Their numbers live in each grammar's `ledger.toml`, alongside what the
corpus is blind to and the mutation test proving the pipeline can report
non-zero.

## Building

```sh
cargo build --workspace
cargo test --workspace
```

`tree-sitter-cli` is pinned at **0.26.12** for all grammar generation; see
DESIGN.md §7 for why the pin is load-bearing.
