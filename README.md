# treebank

Tree-sitter grammars written from scratch and owned outright, carrying a
shared node vocabulary that is enforced in the parse table itself — so
queries like `(_declaration)`, `(_loop)` and `(_callable)` mean the same
thing across languages. Languages: **Python, Rust, TypeScript** (the
TypeScript grammar also parses JavaScript), **Java, Ruby, Bash**, **C and
C++** (the C++ grammar extends the C one rather than copying it), and
**Zig**.

**[`DESIGN.md`](DESIGN.md) is the authoritative document** — the vocabulary,
its two tiers and the measurements that forced them, the version-union
grammar policy, the testing invariants, and the crate layout. Start there.
**[`FIELD_GUIDE.md`](FIELD_GUIDE.md)** is its companion for grammar
authors: what to do and what not to do when writing a parser, each rule
paid for by a measured incident, enforced mechanically by `treebank lint`.

## What is in this repo

| path | what it is |
|---|---|
| `DESIGN.md` | the design: vocabulary, invariants, layout, order of work |
| `VARIANTS.md` | proposal: one parse table per dialect/version-family from shared grammar source — splitting Python 2 out, and taking on SQL |
| `crates/treebank-python` | Python 2.7 ∪ 3.x in one grammar |
| `crates/treebank-rust` | Rust editions 2015–2024 in one grammar |
| `crates/treebank-typescript` | TypeScript ∪ JavaScript ∪ JSX in one grammar |
| `crates/treebank-java` | Java 8 through 21 in one grammar |
| `crates/treebank-ruby` | Ruby 3.x in one grammar |
| `crates/treebank-bash` | GNU bash 5.x in one grammar |
| `crates/treebank-c` | C89–C23 with the GNU extensions, preprocessor included |
| `crates/treebank-cpp` | C++98–C++23, extending the C grammar through tree-sitter's own inheritance |
| `crates/treebank-zig` | Zig 0.11 through 0.16 in one grammar |
| `crates/treebank-core` | the vocabulary as code and data: the closed term lists, the `roles.json` facet schema, the conformance checker behind `treebank roles`, and facet query expansion |
| `crates/treebank-lang` | the canonical language names every other crate agrees on |
| `crates/treebank-corpus` | corpus acquisition: rank an ecosystem's packages, fetch, extract, write the manifest sweeps consume — self-contained so it can move out of this repo |
| `crates/treebank-oracle` | reference-parser oracles behind one trait, carrying their own oracle programs |
| `crates/treebank-cli` | `treebank` — `rank` · `fetch` · `sweep` · `negative` · `roles` · `rosetta` · `oracle` |
| `crates/treebank-preprocessing` | dead-branch elimination for C-family preprocessors: `__cplusplus` undefined for C and `201703L` for C++, which is what makes the `extern "C" {`-split-across-`#ifdef` class legible as something other than a grammar bug |
| `test/rosetta` | the same program in every participating language, with the role counts all four must produce |

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
| registered | a grammar the rest of the repo cannot reach: a crate no language names, or a `tree-sitter.json` that does not admit to parsing the files the registry routes at it |
| reproducible generation | committed `src/` drifting from `grammar.js` at the pinned CLI |
| corpus tests | tree *shape* regressions, not just accept/reject |
| negative corpus | accepts-invalid-code — the direction optimizing a pass rate drifts toward, and the one no corpus of real source can reveal |
| `treebank roles` | vocabulary conformance: closed lists, total node coverage, containments, manifest validity |
| `treebank rosetta` | a role threaded in one grammar and forgotten in another (supertype matching is derivation-based, so a missed thread is otherwise silent) |
| `treebank lint` | the FIELD_GUIDE.md smells: conflict growth, early commits between parallel tiers, same-text token splits, unreserved keywords, scanner/externals drift — ratcheted per grammar by `lint_policy.toml` |
| wasm build | a grammar that cannot cross to wasm — caught here, not in a consumer's browser |

Corpus sweeps are not in CI: the corpora are gigabytes and gitignored.
Their numbers live in each grammar's `ledger.toml`, alongside what the
corpus is blind to and the mutation test proving the pipeline can report
non-zero.

### Reference-tool capabilities

Every language has a validity oracle. The deeper checks depend on what its
reference toolchain exposes; absence is explicit rather than a silent no-op.

| language | node spans (`shape`) | own formatter (`reformat`) | AST printer (`roundtrip`) |
|---|---:|---:|---:|
| Python | yes | Black | `ast.unparse` |
| Rust | yes | rustfmt | prettyplease over syn |
| TypeScript / JavaScript | yes | TypeScript language service | TypeScript printer |
| Java | yes | — | — |
| Bash | yes | — | — |
| Ruby | yes | — | — |
| C / C++ | — | — | — |
| Zig | — | `zig fmt` | — |

The remaining dashes are real toolchain gaps, not forgotten registrations:
the project does not substitute a third-party style formatter for a
language-owned formatter, and does not call a token-preserving formatter an
AST printer. C/C++ source extents and a stable Zig AST surface are the next
span work when their adapters can make the same guarantees as the existing
oracles.

## Adding a language

Deliberately short, and kept that way. The workspace picks a new crate up
from `crates/*`, CI builds its gate matrix from which directories contain a
`grammar.js`, the shape gate turns itself on when `test/shape` appears, and
file extensions are read from the registry rather than restated in the
fuzzer and the shape checker. What is left is the work with a decision in
it:

1. **Write the grammar.** `crates/treebank-<lang>/` — `grammar.js`,
   `tree-sitter.json`, `roles.json`, `ledger.toml`, `build.rs`, the Rust
   bindings, and `test/corpus` + `test/negative`. `lint_policy.toml` and
   `shape_policy.toml` are optional and arrive later: the first ratchets
   the FIELD_GUIDE.md smells once the grammar has settled, the second
   declares where the reference parser groups the tree differently on
   purpose. Both are advisory until written.
2. **Register the language.** One line in the `languages!` block in
   [`crates/treebank-lang/src/lib.rs`](crates/treebank-lang/src/lib.rs):
   canonical name, source extensions, and which grammar parses it.
3. **Give it a corpus.** Implement `Ecosystem` in
   `crates/treebank-corpus/src/<lang>.rs`. Where the ranking comes from and
   which files count is nobody else's decision to make.
4. **Give it an oracle.** Implement `Oracle` in
   `crates/treebank-oracle/src/<lang>.rs`, with its checker program under
   that crate's `tools/`. A grammar with no reference parser cannot be
   swept: every failure would be its own excuse.
5. **Answer the three optional capabilities** in
   [`crates/treebank-oracle/src/capabilities.rs`](crates/treebank-oracle/src/capabilities.rs)
   — node boundaries, a formatter, a tree printer. `None` is a real answer
   as long as it comes with the sentence saying why.
6. **Add a row** to the table at the top of this file.

Steps 3, 4 and 5 are exhaustive `match`es, so the compiler asks for them;
step 2 is what `treebank verify` checks; step 1 is the reason the language
is being added at all.

## Building

```sh
cargo build --workspace
cargo test --workspace
```

`tree-sitter-cli` is pinned at **0.26.12** for all grammar generation; see
DESIGN.md §7 for why the pin is load-bearing.
