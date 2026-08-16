# Treebank — design

*Supersedes the vendoring-era `DESIGN.md`, `docs/ONTOLOGY.md` and
`docs/INVARIANT.md` (2026-08-16). Measurements from those documents are cited
where they still hold; their decisions are re-decided here. Fresh slate: no
transition plan, because nothing downstream consumes the old model.*

## 1. The decision

Treebank writes its own tree-sitter grammars, from scratch, for the languages
it cares about. **Initial targets: Python, Rust, TypeScript.**

The reason is not maintenance burden. Owning the grammar means the shared node
vocabulary is **enforced in the parse table itself** — a role a node plays is
a fact of the parse, checked at generate time, not a convention papered on by
a query layer that can drift. Everything else in this document exists to make
that enforcement real, and to make sure an owned grammar is *measured* against
reality harder than a vendored one ever was.

Upstream tree-sitter grammars are not discarded; they are **demoted to test
oracles**. Every grammar we write is differentially tested against the grammar
it replaces, over the full corpus, forever (§6).

## 2. The three layers

```
language-specific syntax          concrete nodes: function_definition, match_arm…
        ↓
shared syntactic supertypes       the vocabulary in §4: _declaration, _loop, _callable…
        ↓
cross-language semantic ontology  Function, Method, NominalType… — NOT in the grammars
```

The shared vocabulary describes **the syntactic role a node plays**, not the
universal semantic concept it represents. That is why `_class`, `_function`,
`_method`, `_variable` are deliberately absent: they are semantic
classifications, they belong in a layer above the syntax, and that layer is
out of scope for the grammar crates (it will consume `roles.json` and the
supertypes, and live in `treebank-core` or above).

Cross-language queries are the acceptance test of the whole design:

```scheme
(_declaration) @decl
(_invocation)  @call
(_loop)        @loop
(_callable)    @fn
(function_definition name: (_name) @n)
```

must mean the same thing over a `.py`, a `.rs` and a `.ts` file.

## 3. What tree-sitter can actually express — measured

The vocabulary's mechanics are dictated by four facts, all measured on
tree-sitter-cli **0.25.10** (the pinned version; 0.26.x ships broken Unicode
tables and stays banned):

1. **An unused supertype rule is silently pruned.** A role rule listed in
   `supertypes:` but referenced by no production survives `grammar.json`,
   vanishes from `parser.c`, and its query matches nothing — no error, no
   warning. *A role must be a real production.*
2. **Nested supertype partitions work.** `_value → _composite | _scalar`
   generates, leaves the parse tree byte-identical, and `(_composite)`,
   `(_scalar)`, `(_value)` all match. A derivation chain gives one occurrence
   several roles: a `while_statement` reached via
   `_statement → _control_flow → _loop` answers all three queries.
3. **Overlapping membership at one position is a hard error.** Two supertypes
   containing the same node, both reachable at the same position, is an
   unresolved conflict and generate fails. Orthogonal roles cannot coexist in
   the table at a single position.
4. **Supertype queries are derivation-based, not type-based.** In a grammar
   where `x` occurs once via supertype `_a` and once directly, `(_a)` matches
   only the first occurrence. A role holds for an occurrence exactly when the
   parse flowed through the role's rule there.

Consequence: roles split into two tiers with different physics, and the tier
assignment is part of the vocabulary, identical for every language.

## 4. The vocabulary

### 4.1 The two tiers

**Table tier** — real supertype rules threaded through the grammar's
productions. Occurrence-level semantics: `(_expression)` matches an
`identifier` only where it is used as an expression, not where it is a
function's name. Enforced at generate time — a grammar that puts a node in a
role position it cannot occupy does not build. Queryable natively by any
tree-sitter consumer.

**Facet tier** — roles that cross-cut derivations and therefore cannot be
supertypes (fact 3). Shipped as `roles.json` in each grammar crate:
type-level membership, generated alongside the grammar, validated in CI
against `node-types.json` (every listed node must exist; every facet must be
from the closed list). `treebank-core` expands facet queries at load time —
`(_callable)` becomes the concrete alternation — so the query surface is
uniform across tiers. Type-level is the *correct* semantics for facets: a
`function_definition` is callable wherever it occurs.

The facet tier is a compromise forced by fact 3, and it is kept honest by
being generated, closed, and CI-checked — not hand-maintained query files.

### 4.2 The terms

Closed list. A grammar may omit terms its language lacks; it may not invent
terms. Adding a term is a vocabulary change, versioned in `treebank-core`,
applying to every language at once. Spelling is underscore-prefixed
everywhere (`_declaration`, not `declaration`): supertypes are hidden nodes
whether or not they carry the underscore, hidden names are fully queryable
(measured), and the underscore visibly marks the shared layer apart from
concrete nodes in queries. Concrete node names never start with `_`.

**Structural core — table tier**

| term | definition | python | rust | typescript |
|---|---|---|---|---|
| `_statement` | executed for effect as an element of a sequence | `if_statement`, `expression_statement`, … | statements inside blocks | `if_statement`, `expression_statement`, … |
| `_expression` | denotes a value | `binary_operator`, `call`, `lambda`, … | nearly everything | `binary_expression`, `call_expression`, … |
| `_declaration` | introduces a named entity — function, class/type, variable, interface — with or without a body | `function_definition`, `class_definition` | `function_definition`, `struct_definition`, `trait_definition`, trait method signatures | `function_definition`, `class_definition`, `interface_declaration`, `type_alias`, `declare …` |
| `_pattern` | destructuring / matching position | match-case patterns, assignment targets | patterns everywhere | binding patterns, destructuring |
| `_type` | syntax in type position | *(none — annotations are expressions)* | all type syntax | all type syntax |
| `_name` | denotes or refers to a name: identifier, qualified name, path | `identifier`, `attribute` in name position | `identifier`, `scoped_identifier`, path types in name position | `identifier`, `nested_identifier`, `qualified_name` |
| `_literal` | value fully determined by its own text, for every instance of the rule | `integer`, `string` *(not f-strings)*, `true` | `integer_literal`, `string_literal`, `char_literal` | `number`, `string` *(not templates)*, `true` |

`_declaration` is deliberately **one term**: `fn f() {}` and `fn f();` and
`declare function f(): void` are all declarations. The has-a-body split the
previous design made is recoverable later as an additive facet (`_signature`)
without breaking a single query, so it is not baked in now. `_binding` (below)
covers the introduces-a-name axis separately, per the layering document.

**Positional roles — table tier**

| term | definition | examples |
|---|---|---|
| `_parameter` | formal parameter position | `typed_parameter`, `default_parameter` (py); `parameter`, `self_parameter` (rs); `required_parameter`, `optional_parameter` (ts) |
| `_argument` | actual argument position in an invocation | `keyword_argument`, splats; plain expressions thread through it |
| `_member` | element of a type's body | statements in a `class` body (py); `field_declaration`, impl items (rs); `method_definition`, `public_field_definition` (ts) |
| `_clause` | subordinate piece of a larger construct, neither statement nor expression | `elif_clause`, `else_clause`, `except_clause`, `case_clause`, `match_arm`, `where_clause`, `catch_clause`, `finally_clause`, comprehension clauses |
| `_modifier` | keyword-ish marker altering a declaration's meaning | named nodes required: `visibility_modifier`, `mutable_specifier`, `accessibility_modifier`, `async`… (a modifier a query should see must be a named node, not an anonymous token) |
| `_attribute` | annotation attached to a declaration | `decorator` (py, ts); `attribute_item`, `inner_attribute_item` (rs) |
| `_directive` | affects the compilation unit or its environment rather than computing in it | `import_statement`, `import_from_statement` (py); `use_declaration`, `extern_crate_declaration` (rs); `import_statement`, `export_statement` (ts); shebangs, pragmas |
| `_body` | the body position of a definition or control construct | `block` (py, rs); `statement_block`, expression bodies of arrow functions (ts) |

**Operational roles — table tier**, nested inside `_statement` and/or
`_expression` per language:

| term | definition | notes |
|---|---|---|
| `_control_flow` | alters sequential execution | contains `_branch`, `_loop`, `_jump`, plus `try_statement`, `with_statement` |
| `_branch` | conditional selection | `if`, `match`, `conditional_expression`, `switch` |
| `_loop` | repetition | `for`, `while`, `loop`, do-while |
| `_jump` | non-local transfer | `return`, `break`, `continue`, `raise`/`throw` |
| `_assignment` | stores into a place | `assignment`, `augmented_assignment` (py); `assignment_expression`, `compound_assignment_expr` (rs); `assignment_expression`, `augmented_assignment_expression` (ts) |
| `_invocation` | applies a callable | `call` (py); `call_expression`, `macro_invocation` (rs); `call_expression`, `new_expression` (ts) |
| `_access` | reads a place: member or index | `attribute`, `subscript` (py); `field_expression`, `index_expression` (rs); `member_expression`, `subscript_expression` (ts) |

Where a language makes control flow an expression (Rust), `_control_flow`
nests inside `_expression`; where it is a statement (Python), inside
`_statement`; TypeScript threads it in both as its syntax requires. The query
`(_loop)` does not care — that is the point.

**Facets — manifest tier** (`roles.json`)

| term | definition | membership sketch |
|---|---|---|
| `_callable` | defines something invokable | `function_definition`, `lambda` (py); `function_definition`, `closure_expression` (rs); `function_definition`, `arrow_function`, `method_definition`, `function_expression` (ts) |
| `_binding` | introduces a name | `function_definition`, `class_definition`, `_parameter` members, `assignment`/`let_declaration`, `for` targets, imports, `named_expression` (py `:=`) |
| `_scope` | delimits a lexical scope | module roots, `function_definition`, `class_definition` (py); blocks, functions, modules (rs); functions, blocks, modules (ts) |

Three facets at launch. A term moves tiers only by a vocabulary version bump.

### 4.3 Rules the checker enforces (`treebank roles`)

1. Declared supertypes ⊆ the closed table-tier list; `roles.json` keys ⊆ the
   closed facet list.
2. Every named, non-`extras` node type is reachable through at least one
   table role **or** listed in a facet **or** ledgered in
   `ledger.roles.uncategorised` with a one-line reason. Nothing is silently
   outside the vocabulary.
3. Every node type in `roles.json` exists in `node-types.json`.
4. Declared containments hold (`_literal ⊆ _expression`; `_branch`, `_loop`,
   `_jump` ⊆ `_control_flow`).
5. **Role liveness:** every declared role matches ≥ 1 occurrence over the
   language's corpus sweep. A role that never fires is a threading bug (fact
   4: derivation-based matching means a missed thread is silent otherwise).
6. **Rosetta agreement (§6.5):** the cross-language fixture suite passes.

## 5. The grammars

### 5.1 Construction rules

- One crate per language: `treebank-python`, `treebank-rust`,
  `treebank-typescript`. Grammar source is ours, from scratch: `grammar.js`
  (+ `src/scanner.c` where the language demands one — all three do).
- Every `grammar.js` imports the vocabulary from `treebank-core`'s
  `vocabulary/supertypes.js` — the term list is shared *code*, not shared
  convention. The import provides the closed list and helpers for the
  standard nestings; the grammar supplies the members.
- **Shared concrete names.** Same construct, same node name, same field
  names, in every grammar we own: `function_definition` (not `function_item`,
  not `function_declaration`), `class_definition`, `call_expression`… with the
  fixed field vocabulary `name:`, `parameters:`, `body:`, `condition:`,
  `value:`, `left:`, `right:`, `operator:`, `type:`, `arguments:`. Diverge
  only where constructs genuinely differ, never to force a match — shapes
  must not lie about syntax.
- Modifiers a query should see are **named nodes**, not anonymous tokens.
- `word:` set per grammar; `extras` carry comments and whitespace only.

### 5.2 One grammar per language, across versions

A language gets **one grammar accepting the union of its versions**. No
python2 crate, no per-edition rust grammars, no per-TS-version anything.

- **Python**: 2.7 ∪ 3.x. The union adds py2's `print` statement, `exec`
  statement, `except E, e:` clauses, backtick repr, old octal literals — all
  parseable alongside py3 with contextual handling.
- **Rust**: editions 2015–2024. The real work is contextual keywords:
  `async`, `dyn`, `try`, `gen` are identifiers in older editions and keywords
  in newer ones; the union grammar accepts both readings.
- **TypeScript**: all TS versions, **and JavaScript** — TS is the union
  language of JS, so this is the same philosophy applied across a language
  boundary. One grammar source, **two generated parsers**: `typescript`
  (`.ts`, `.mts`, `.cts`) and `tsx` (`.tsx`, `.jsx`, `.js`, `.mjs`, `.cjs`).
  Two parsers because `<T>x` is a cast in `.ts` and an unclosed JSX element
  in `.tsx` — a genuine grammatical ambiguity, not a precedence problem; no
  single table exists.

Version *identification* (which versions accept this file) is deliberately
not built now. The sweep already records per-oracle verdicts (§5.3), which is
the hook a future `version_of()` builds on; nothing more today.

### 5.3 What "valid" means with multiple versions

One oracle per **version family**, each pinned like today's oracles
(tool + version + flags + declared positions + smoke test):

- python: CPython 3 `compile(src, f, 'exec')` (measured 1.23 s/1000 files) +
  CPython 2.7's `compile` where a python2 exists; if no python2 binary is
  available in CI, py2 verdicts come from a frozen battery and the ledger
  says so.
- rust: `syn` in-process (already built, measured) + `rustc -Zparse-only`
  under each `--edition` for adjudicating disagreements.
- typescript: `tsc` parse (measured 0.50 s/1000) per dialect; V8 as the
  secondary oracle for plain `.js`.

Adjudication over the version set:

- **gap** — grammar rejects ∧ **any** version-oracle accepts. Always a bug.
- **widening** — grammar accepts ∧ **every** version-oracle rejects. Caught
  only by the negative corpus and batteries, so those are per-version too:
  `test/negative/` holds files invalid under *every* version, plus
  `test/negative/<version>/` for files that must stay invalid in that
  version's oracle (guarding against the union quietly becoming "anything").
- the sweep records the per-oracle verdict vector per file — free now,
  needed later.

## 6. Testing — the invariant

Owning grammars destroys the old credibility claim (reproducible derivation
from upstream). The replacement is behavioural, and it is stronger for a
consumer because it answers the consumer's actual question — *will this parse
my code* — instead of *where did this come from*. Five checks, all in
`verify.sh`, all in CI, per grammar:

**I1 — Reproducible generation.** `grammar.js` + `scanner.c` +
tree-sitter-cli 0.25.10 → `src/`, byte for byte.

**I2 — Corpus sweep with oracle adjudication.** Full-corpus parse; failures
adjudicated by the version oracles; `gap_files = 0` or a ledgered list. The
zero is falsified by mutation (delete a rule, sweep must report gaps — the
technique that validated the json pipeline: 0 gaps unmutated, 3 with `null`
deleted).

**I3 — Differential against the replaced grammar.** The upstream tree-sitter
grammar we replace is pinned as a fixture (`reference/`, a submodule that
`materialize` never touches). Verdict-level differential over the full
corpus: K disagreements, every one adjudicated by the oracles and either
fixed or ledgered in `differential.divergences` with construct, direction and
reason. **K_unadjudicated = 0** is the invariant; K is a number, not a
threshold. Measured cost: a full 5,657-file corpus differential runs in
2.7 s single-threaded — this check is cheaper than the build. The unit is the
verdict, not the tree: our trees differ from upstream's by design (shared
names, roles), so tree equality would measure the wrong thing.

**I4 — Negative corpus + conformance suites.** Per-version negative files
(§5.3). Where an official suite exists, it runs and every known failure is
ledgered. Candidates: CPython's `test_grammar`/`test_tokenize` corpora,
rustc's parser ui tests, TypeScript's compiler conformance suite.

**I5 — Vocabulary conformance.** `treebank roles` (§4.3): closed lists,
total coverage, containments, manifest validity, role liveness, rosetta.

### 6.5 The rosetta corpus

A directory of small parallel programs — the same behaviour written in
Python, Rust and TypeScript — with one expected-roles file per program:
assertions like *"exactly 2 `_loop`, 3 `_declaration`, 1 `_callable` facet
match, and `(function_definition name: (_name))` yields `f` in all three."*
This is the executable form of the promise that the vocabulary means the same
thing everywhere, and it is what catches a role threaded in one grammar and
forgotten in another (fact 4 makes that failure silent otherwise).

### Corpora

The sweep infrastructure (fetch/rank/sweep/oracle in `treebank-cli`) carries
over as is. On disk today: python — 70,423 files from 492 top-PyPI packages
(2.1 GB); rust — the crates.io top-3000 corpus (2.7 GB); typescript — needs a
fetch, npm infra already built. Corpus composition biases get declared in the
ledger (`blind_to`, monoculture warnings) exactly as the json and toml
ledgers do now; python2 files in modern package corpora are rare, so the py2
side leans on batteries and the negative corpus, and the ledger says so.

## 7. Code organization

```
crates/
  treebank-core/            # the vocabulary, as code and as data:
    vocabulary/supertypes.js#   the closed term list grammars import
    src/                    #   roles.json schema, facet query expansion,
                            #   the `treebank roles` checker, node-types loading
  treebank-python/
    grammar.js  src/scanner.c  src/ (generated, committed)
    roles.json  ledger.json
    test/corpus/  test/negative/  test/negative/<version>/
    reference/              # pinned upstream tree-sitter-python, fixture only
    bindings/  (rust crate + wasm)
  treebank-rust/            # same shape
  treebank-typescript/      # common/define-grammar.js + typescript/ + tsx/
  treebank-cli/             # fetch · rank · sweep · oracle · differential ·
                            # roles · negative · ledger
tools/consumer-test/        # downstream crate + wasm smoke tests
test/rosetta/               # parallel programs + expected-roles files
```

- **treebank-core** is the single source of truth for the vocabulary — the
  JS module the grammars import, the Rust library the CLI and consumers use,
  and (via wasm) the same expansion logic in the browser. Vocabulary versions
  are semver on this crate.
- **Generated `src/` is committed** in each grammar crate (I1 makes it
  honest). There is no `build/` indirection, no patches, no materialization —
  those existed to manage someone else's tree.
- **wasm is a first-class artifact**: every grammar crate ships
  `tree-sitter-<lang>.wasm` alongside the Rust crate, and `treebank-core`
  ships a JS/wasm package that loads them and provides facet-aware queries.
  The tbwasm session's pipeline is the build path; this design adds only the
  requirement that `roles.json` travels inside both the crate and the npm
  package.
- `ledger.json`, rewritten for ownership: `language`, `versions[]`,
  `oracles[]` (one per version family), `corpus`, `sweeps`, `differential`
  (reference pin + K + divergences), `roles.uncategorised[]`,
  `generate_cli`. The `upstream`/`patches` machinery is gone.

## 8. Decisions log

Settled 2026-08-16, superseding the eight-term ontology of `docs/ONTOLOGY.md`:

1. **Facets ship as a manifest + core-expanded queries; structural roles are
   table supertypes.** Forced by measurement (§3); the alternative — a
   vocabulary of only threadable roles — was rejected to keep the layering
   document's query set whole.
2. **One `_declaration`.** The definition/declaration has-a-body split is
   superseded; recoverable additively as a `_signature` facet if wanted.
3. **`treebank-typescript` covers JS**, two generated dialect parsers from
   one source; `.js` routes to `tsx`.
4. **Shared concrete names and fields** across owned grammars
   (`function_definition` everywhere); divergence only where syntax genuinely
   differs.
5. **Underscore spelling** for all vocabulary terms (reverses the earlier
   public-spelling decision: the layering document's queries use underscores,
   hidden supertypes are fully queryable — measured — and facets are not
   grammar rules at all, so one convention covers both tiers).
6. Carried forward unchanged: tree-sitter-cli pinned at 0.25.10; oracles
   pinned with declared positions and loud-failure rules; corpus biases
   declared in ledgers; zeros falsified by mutation.

## 9. Order of work and honest sizing

`treebank-core` (vocabulary + checker + expansion) → **Python** → **Rust** →
**TypeScript**.

Python first: its corpus is already on disk and largest, its oracle is
cheapest to extend to versions, and its scanner (indentation, f-strings) is
the hardest *small* scanner — the right first test of whether from-scratch
scanners are sustainable. For scale calibration, the grammars being replaced
are: python 1,275-line grammar + 540-line scanner; rust 1,789 + 403;
typescript 1,194 common + the 1,339-line javascript base + a shared scanner.
Ours will differ in shape but not in order of magnitude, and the version
union adds real size (py2 forms; rust contextual keywords). The scanners are
the risk concentrate: indentation, raw strings, template literals,
ASI-adjacent token decisions, JSX text. Sequencing one language at a time,
with the differential harness live from the first week, is what keeps that
risk measured.
