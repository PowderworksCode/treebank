# treebank

Tree-sitter grammars written from scratch and owned outright, carrying a
shared node vocabulary that is enforced in the parse table itself — so
queries like `(_declaration)`, `(_loop)` and `(_callable)` mean the same
thing across languages. Languages: **Python, Rust, TypeScript** (the
TypeScript grammar also parses JavaScript), **Java, Ruby, Bash**, **C and
C++** (the C++ grammar extends the C one rather than copying it), **Zig**,
**YAML**, and **HCL** (the HCL2 native syntax, which is what Terraform's
`.tf` and `.tfvars` are written in).

**[`notes/DESIGN.md`](notes/DESIGN.md) is the authoritative document** — the vocabulary,
its two tiers and the measurements that forced them, the version-union
grammar policy, the testing invariants, and the crate layout. Start there.
**[`notes/field_guide.md`](notes/field_guide.md)** is its companion for grammar
authors: what to do and what not to do when writing a parser, each rule
paid for by a measured incident, enforced mechanically by `treebank lint`.

## What is in this repo

| path | what it is |
|---|---|
| `notes/DESIGN.md` | the design: vocabulary, invariants, layout, order of work |
| `crates/treebank-python` | Python 2.7 ∪ 3.x in one grammar |
| `crates/treebank-rust` | Rust editions 2015–2024 in one grammar |
| `crates/treebank-typescript` | TypeScript ∪ JavaScript ∪ JSX in one grammar |
| `crates/treebank-java` | Java 8 through 21 in one grammar |
| `crates/treebank-ruby` | Ruby 3.x in one grammar |
| `crates/treebank-bash` | GNU bash 5.x in one grammar |
| `crates/treebank-c` | C89–C23 with the GNU extensions, preprocessor included |
| `crates/treebank-cpp` | C++98–C++23, extending the C grammar through tree-sitter's own inheritance |
| `crates/treebank-zig` | Zig 0.11 through 0.16 in one grammar |
| `crates/treebank-yaml` | YAML 1.1 and 1.2 in one grammar, structure decided in the scanner because it is decided by columns |
| `crates/treebank-hcl` | HCL2 native syntax in one grammar — `.hcl`, `.tf` and `.tfvars`, because Terraform is a dialect of HCL and adds a schema rather than syntax |
| `crates/treebank` | the vocabulary as code and data: the closed term lists, the `roles.json` facet schema, the conformance checker behind `treebank roles`, and facet query expansion |
| `crates/treebank-lang` | the canonical language names every other crate agrees on |
| `crates/treebank-corpus` | corpus acquisition: rank an ecosystem's packages, fetch, extract, write the manifest sweeps consume — self-contained so it can move out of this repo |
| `crates/treebank-oracle` | reference-parser oracles behind one trait, carrying their own oracle programs |
| `crates/treebank-cli` | `treebank` — `status` · `rank` · `fetch` · `hydrate` · `sweep` · `negative` · `roles` · `rosetta` · `oracle` |
| `crates/treebank-preprocessing` | dead-branch elimination for C-family preprocessors: `__cplusplus` undefined for C and `201703L` for C++, which is what makes the `extern "C" {`-split-across-`#ifdef` class legible as something other than a grammar bug |
| `test/rosetta` | the same program in every participating language, with the role counts all four must produce |

Each grammar crate ships its `roles.json` and `ledger.toml` inside the
published package, so a consumer gets the facet membership and the
evidence — versions covered, pinned oracles, corpus numbers, known gaps,
declared deviations — without fetching anything.

## Using a grammar

One crate, and the grammar is fetched:

```sh
cargo add treebank
```

```rust
use treebank::Pack;

let pack = Pack::fetch("python")?;
let tree = pack.parse(source)?;
println!("{}", tree.root().sexp()?);
```

`fetch` downloads the grammar, verifies it against the published sha256 and
caches it. `Pack::fetch_pinned("python", "<hash>")` names an exact version for
a build that must not vary.

There is deliberately no crate per grammar: nine today and more later would
mean a version to keep in step for each. Only `treebank` is published — the
grammar crates in this repository are `publish = false` and exist to build the
wasm packs.

Inside this workspace a grammar can also be used directly, which is what the
gates do:

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

### Status at a glance

`treebank status` joins the repository's existing sources of truth rather
than introducing another configuration file: the language registry,
`tree-sitter.json`, `roles.json`, every `ledger.toml`, fixture and known-deviation
declarations, corpus locks and canary workflows.

```sh
cargo run -p treebank-cli -- status
cargo run -p treebank-cli -- status --format json
cargo run -p treebank-cli -- status --format markdown
cargo run -p treebank-cli -- status --github
```

The default table shows corpus pass/gap evidence, exact corpus/negative/shape
fixture counts, declared known-gap/widening/deviation queues, reference-tool
capabilities, locks, evidence freshness, canaries and known deviations for
every grammar. Evidence is `current` only when its recorded corpus-lock and
generated-grammar hashes match the checkout and it names a committed grammar
revision; a complete older binding is `stale`, while legacy or incomplete
evidence is `unbound`. Configuration errors are separate; `--check` exits
non-zero on malformed or contradictory configuration and is what CI runs.

`--github` is deliberately opt-in so the ordinary inventory stays offline and
deterministic. With an authenticated `gh` it adds open issues and pull
requests, workflow state and default-branch protection. `--repo OWNER/REPO`
overrides checkout-based repository detection.

Run them all for one grammar with `treebank verify crates/treebank-<lang>`.

| gate | what it catches |
|---|---|
| registered | a grammar the rest of the repo cannot reach: a crate no language names, or a `tree-sitter.json` that does not admit to parsing the files the registry routes at it |
| reproducible generation | committed `src/` drifting from `grammar.js` at the pinned CLI |
| corpus tests | tree *shape* regressions, not just accept/reject |
| negative corpus | accepts-invalid-code — the direction optimizing a pass rate drifts toward, and the one no corpus of real source can reveal |
| `treebank roles` | vocabulary conformance: closed lists, total node coverage, containments, manifest validity |
| `treebank rosetta` | a role threaded in one grammar and forgotten in another (supertype matching is derivation-based, so a missed thread is otherwise silent) |
| `treebank lint` | the notes/field_guide.md smells: conflict growth, early commits between parallel tiers, same-text token splits, unreserved keywords, scanner/externals drift — ratcheted per grammar by `lint_policy.toml` |
| wasm build | a grammar that cannot cross to wasm — caught here, not in a consumer's browser |

The full corpora are gigabytes and gitignored, so per-change CI sweeps a
checked-in two-file corpus for every language through the production path.
Full-corpus numbers live in each grammar's `ledger.toml`, alongside what the
corpus is blind to and the mutation test proving the pipeline can report
non-zero. A weekly or manually dispatched matrix canary hydrates every
committed corpus lock, sweeps every admitted file, and fails if the generated
evidence differs from its grammar's ledger.

### Reproducible corpora

A sweep is release evidence only when another machine can recreate its exact
inputs. `fetch` therefore records both levels of provenance: the immutable
archive URL, byte count and SHA-256, then the path, byte count and SHA-256 of
every admitted source file. Write a committable lock while fetching:

```sh
cargo run -p treebank-cli -- fetch --lang rust \
  --lock-out corpus-locks/rust.json
```

When only the committable identity is needed, `--lock-only` discards each
downloaded package after hashing it instead of retaining a multi-gigabyte
working corpus:

```sh
cargo run -p treebank-cli -- fetch --lang rust \
  --lock-out corpus-locks/rust.json --lock-only
```

A clean machine recreates the corpus from the lock without resolving package
versions again:

```sh
cargo run -p treebank-cli -- hydrate --lang rust
```

Hydration stages the complete source tree and publishes it only after every
archive and extracted file matches. It refuses to overwrite a non-empty
corpus and refuses older manifests without archive provenance; those describe
a past run but cannot reproduce one. See [`corpus-locks/README.md`](corpus-locks/README.md)
for the lock update contract.

`treebank sweep` writes the binding into the language's `[corpus.*sweep]`
ledger block: an exact-byte SHA-256 of the corpus lock, a SHA-256 of the generated
`parser.c` plus `scanner.c`, and the last committed revision that changed those
grammar inputs. If the grammar inputs are dirty there is no honest Git revision
to name; the same is true in a shallow checkout without their history. The
sweep records the content hashes but omits the revision in either case. Commit
the grammar in a full checkout, rerun the cached sweep, and then commit the
bound ledger update.

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
| C / C++ | yes | — | — |
| Zig | — | `zig fmt` | — |
| YAML | yes | — | — |
| HCL / Terraform | yes | `tofu fmt` | — |

YAML's two dashes are one fact stated twice: the language has no owning
implementation, so there is no formatter and no printer to be the
language's own — `prettier` and every library's re-serializer are third
party, and this project does not substitute one for the other. Its spans
come from the same `yaml` package the verdict oracle's 1.2 leg uses, which
is the most conformant implementation available rather than a reference,
and `shape_policy.toml` says what that makes the check blind to.

HCL's one dash is the same kind of fact: `hclwrite` is a token-preserving
tree, and this project does not call a token-preserving formatter an AST
printer. Its spans and its formatter come from different places for a
reason ledger.toml records — the boundaries from the MPL `hcl` library
that IS the reference parser, the formatting from OpenTofu, because
`tofu fmt`'s alignment rules live in Terraform and its fork rather than in
HCL.

The remaining dashes are real toolchain gaps, not forgotten registrations:
the project does not substitute a third-party style formatter for a
language-owned formatter, and does not call a token-preserving formatter an
AST printer. A stable Zig AST surface is the next span dependency; the Zig
toolchain currently exposes formatting and validation but no supported tree
dump carrying source extents.

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
   the notes/field_guide.md smells once the grammar has settled, the second
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
6. **Add a row** to the table at the top of this file, and a **playground
   sample** in [`site/src/samples.mjs`](site/src/samples.mjs) — a few lines
   that show the language being itself. The playground's grammar list is
   derived from `crates/`, so a language with no sample arrives in the menu,
   loads its parser and shows an empty editor;
   `site/tests/playground.test.ts` is what turns that into a failing test
   instead of something nobody notices.

Steps 3, 4 and 5 are exhaustive `match`es, so the compiler asks for them;
step 2 is what `treebank verify` checks; step 1 is the reason the language
is being added at all.

## Building

```sh
cargo build --workspace
cargo test --workspace
```

`tree-sitter-cli` is pinned at **0.26.12** for all grammar generation; see
notes/DESIGN.md §7 for why the pin is load-bearing.
