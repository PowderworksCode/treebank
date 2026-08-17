# Treebank — design

Treebank is a set of tree-sitter grammars written from scratch and owned
outright — no upstream grammar repos, no forks, no vendored trees anywhere in
the system. **Initial languages: Python, Rust, TypeScript.**

Three ideas define it:

1. **A shared node vocabulary, enforced in the parse table.** Every treebank
   grammar carries the same set of supertypes — `_declaration`, `_loop`,
   `_invocation`, … — alongside its language-specific nodes, so tooling can
   ask cross-language questions without knowing each grammar's concrete node
   names. Because we write the grammars, the vocabulary is a property of the
   parse itself, checked when the parser is generated — not a convention
   maintained by hand in query files that can drift.
2. **One grammar per language, across language versions.** The Python grammar
   parses Python 2.7 and every Python 3; the Rust grammar parses every
   edition; the TypeScript grammar parses every TS version and JavaScript.
3. **Validation by measurement.** Every grammar is swept over large corpora
   of real published code, every failure is adjudicated by the language's
   reference parser, and every claim of correctness is a number over a named
   corpus — never an assertion.

The grammars ship as Rust crates and as wasm, from one repository.

## 1. The three layers

```
language-specific syntax          concrete nodes: function_definition, match_arm, …
        ↓
shared syntactic supertypes       the vocabulary in §3: _declaration, _loop, _callable, …
        ↓
cross-language semantic ontology  Function, Method, NominalType, … — NOT in the grammars
```

The shared vocabulary describes **the syntactic role a node plays**, not the
universal semantic concept it represents. That is why there is no `_class`,
`_function`, `_method`, or `_variable` in it: those are semantic
classifications, they become impossible to define consistently across
languages, and they belong in a layer built on top of the grammars rather
than inside them. A `method_definition` node is a `_declaration`, a
`_member`, and (as a facet) `_callable`; calling it a *Method* is the upper
layer's job.

Cross-language queries are the acceptance test of the whole design:

```scheme
(_declaration) @decl
(_invocation)  @call
(_loop)        @loop
(_callable)    @fn
(function_definition name: (_name) @n)
```

must mean the same thing over a `.py`, a `.rs`, and a `.ts` file.

## 2. What tree-sitter supertypes can express — measured

Tree-sitter's `supertypes` mechanism looks like it could carry any vocabulary
you like. It cannot, and the vocabulary's structure is dictated by four facts,
all measured on tree-sitter-cli 0.26.12, the version treebank pins (§7), and
re-confirmed on it after the pin moved there from 0.25.10:

1. **An unused supertype rule is silently pruned.** A rule listed in
   `supertypes:` but referenced by no production survives into `grammar.json`,
   vanishes from `parser.c`, and its query matches nothing — no error, no
   warning. A role must be a real production, reachable from the root.
2. **Nested supertype partitions work.** Splitting a position into
   `_value → _composite | _scalar` generates cleanly, leaves the parse tree
   byte-identical, and `(_composite)`, `(_scalar)` and `(_value)` all match.
   A derivation chain gives one occurrence several roles at once: a
   `while_statement` reached via `_statement → _control_flow → _loop` answers
   all three queries.
3. **Overlapping membership at one position is a hard error.** Two supertypes
   containing the same node, both reachable at the same position, is an
   unresolved conflict: generate fails. Orthogonal roles cannot coexist in
   the parse table at a single position.
4. **Supertype queries are derivation-based, not type-based.** In a grammar
   where node `x` occurs once via supertype `_a` and once directly, `(_a)`
   matches only the first occurrence. A role holds for an *occurrence*
   exactly when the parse flowed through the role's rule at that position.

Fact 4 is the quiet gift: `(_expression)` matches an `identifier` only where
it is *used* as an expression, not where it is a function's name — role-of-
this-occurrence, which is precisely what a syntactic-role vocabulary should
mean. Facts 1 and 3 are the constraint: roles that cross-cut the derivation
(a `function_definition` is *callable* whether it occurs in statement
position or class-body position) cannot be supertypes at all. So the
vocabulary has two tiers with different physics.

## 3. The vocabulary

### 3.1 Two tiers

**Table tier** — real supertype rules threaded through the productions.
Occurrence-level semantics, enforced at generate time: a grammar that puts a
node somewhere its role forbids does not build. Natively queryable by any
tree-sitter consumer with no treebank machinery at all.

**Facet tier** — roles that cross-cut derivations (§2, fact 3). Shipped as a
`roles.json` manifest in each grammar crate: type-level membership,
maintained next to the grammar, validated in CI against the generated
`node-types.json` (every listed node must exist; every facet must be from
the closed list). `treebank-core` expands facet queries at load time —
`(_callable)` becomes the concrete alternation `[(function_definition)
(lambda) …]` — so the query surface is uniform across tiers. Type-level is
the *correct* semantics for facets: a `function_definition` is callable
wherever it occurs.

The facet tier is a compromise forced by the parse table's limits, kept
honest by being closed, machine-checked, and shipped inside the same crate
as the grammar — not a hand-maintained query pack.

### 3.2 The terms

The list is **closed**. A grammar may omit terms its language lacks
(Python declares no `_type`; its annotations are ordinary expressions). It
may not invent terms. Adding a term is a vocabulary change, versioned in
`treebank-core`, applying to every language at once.

All vocabulary names are underscore-prefixed (`_declaration`); concrete node
names never are. Supertypes are hidden nodes either way, and hidden names
are fully queryable — the underscore just marks the shared layer apart from
concrete nodes in every query that mixes them.

**Structural core — table tier**

| term | definition | python | rust | typescript |
|---|---|---|---|---|
| `_statement` | executed for effect as an element of a sequence | `if_statement`, `expression_statement`, … | statements inside blocks | `if_statement`, `expression_statement`, … |
| `_expression` | denotes a value | `binary_operator`, `call_expression`, `lambda`, … | nearly everything | `binary_expression`, `call_expression`, … |
| `_declaration` | introduces a named entity — function, class/type, variable, interface — with or without a body | `function_definition`, `class_definition` | `function_definition`, `struct_definition`, `trait_definition`, trait method signatures | `function_definition`, `class_definition`, `interface_declaration`, `type_alias`, `declare …` |
| `_pattern` | destructuring or matching position | match-case patterns, assignment targets | patterns everywhere | binding patterns, destructuring |
| `_type` | syntax in type position | *(not declared)* | all type syntax | all type syntax |
| `_name` | denotes or refers to a name: identifier, qualified name, path | `identifier`, dotted names in name position | `identifier`, `scoped_identifier`, paths | `identifier`, `nested_identifier`, `qualified_name` |
| `_literal` | value fully determined by its own text, for every instance of the rule | `integer`, `string` *(not f-strings)*, `true` | `integer_literal`, `string_literal`, `char_literal` | `number`, `string` *(not templates)*, `true` |

Two definitional notes. `_declaration` is one term: `fn f() {}`, `fn f();`
and `declare function f(): void` are all declarations; the with-body /
without-body distinction is not encoded (it can be added later as a facet,
additively, without breaking a query). `_literal` quantifies over the rule,
not the instance: Python's `string` rule can carry interpolation, so no
Python string is a `_literal`, while Rust's `string_literal` cannot, so
every one is.

**Positional roles — table tier**

| term | definition | examples |
|---|---|---|
| `_parameter` | formal parameter position | `typed_parameter`, `default_parameter` (py); `parameter`, `self_parameter` (rs); `required_parameter`, `optional_parameter` (ts) |
| `_argument` | actual argument position in an invocation | `keyword_argument`, splats; plain expressions thread through it |
| `_member` | element of a type's body | statements in a `class` body (py); `field_declaration`, impl items (rs); `method_definition`, `public_field_definition` (ts) |
| `_clause` | subordinate piece of a larger construct that is not naturally a statement or expression | `elif_clause`, `else_clause`, `except_clause`, `case_clause`, `match_arm`, `where_clause`, `catch_clause`, `finally_clause`, comprehension clauses |
| `_modifier` | keyword-ish marker altering a declaration's meaning | `visibility_modifier`, `mutable_specifier`, `accessibility_modifier`, `async`, … |
| `_attribute` | annotation attached to a declaration | `decorator` (py, ts); `attribute_item`, `inner_attribute_item` (rs) |
| `_directive` | affects the compilation unit or its environment rather than computing in it | `import_statement`, `import_from_statement` (py); `use_declaration`, `extern_crate_declaration` (rs); `import_statement`, `export_statement` (ts); shebangs, pragmas |
| `_body` | the body position of a definition or control construct | `block` (py, rs); `statement_block`, arrow-function expression bodies (ts) |

A rule the positional roles impose on the grammars: anything a query should
see must be a **named node**. `pub`, `mut`, `async`, `readonly` are named
modifier nodes in treebank grammars, not anonymous tokens, because an
anonymous token can never carry a role.

**Operational roles — table tier**, nested inside `_statement` and/or
`_expression` as each language requires:

| term | definition | notes |
|---|---|---|
| `_control_flow` | alters sequential execution | contains `_branch`, `_loop`, `_jump`, plus `try_statement`, `with_statement` |
| `_branch` | conditional selection | `if`, `match`, `conditional_expression`, `switch` |
| `_loop` | repetition | `for`, `while`, `loop`, do-while |
| `_jump` | non-local transfer | `return`, `break`, `continue`, `raise`/`throw` |
| `_assignment` | stores into a place | `assignment`, `augmented_assignment` (py); `assignment_expression`, `compound_assignment_expr` (rs); `assignment_expression`, `augmented_assignment_expression` (ts) |
| `_invocation` | applies a callable | `call_expression`; `macro_invocation` (rs); `new_expression` (ts) |
| `_access` | reads a place: member or index | `attribute`, `subscript` (py); `field_expression`, `index_expression` (rs); `member_expression`, `subscript_expression` (ts) |

Where a language makes control flow an expression (Rust), `_control_flow`
nests inside `_expression`; where it is a statement (Python), inside
`_statement`; TypeScript threads it wherever its syntax requires. `(_loop)`
does not care — that is the point of the vocabulary.

**Facets — manifest tier** (`roles.json`)

| term | definition | membership sketch |
|---|---|---|
| `_callable` | defines something invokable | `function_definition`, `lambda` (py); `function_definition`, `closure_expression` (rs); `function_definition`, `arrow_function`, `method_definition`, `function_expression` (ts) |
| `_binding` | introduces a name | `function_definition`, `class_definition`, parameters, `assignment` / `let_declaration`, `for` targets, imports, `named_expression` (py `:=`) |
| `_scope` | delimits a lexical scope | module roots, functions, classes (py); blocks, functions, modules (rs); functions, blocks, modules (ts) |

`_declaration` and `_binding` are deliberately different questions. In

```rust
fn foo(x: i32) {
    let y = x;
    for z in values {}
}
```

`foo`, `x`, `y` and `z` all introduce names, from four different constructs;
only `fn foo` is a `_declaration`. Tooling gets to ask *find declarations*
and *find everything that introduces a name* as two queries, not one.

Three facets at launch. A term moves between tiers only with a vocabulary
version bump.

### 3.3 What the checker enforces (`treebank roles`, in CI)

1. Declared supertypes ⊆ the closed table-tier list; `roles.json` keys ⊆ the
   closed facet list.
2. Every named, non-`extras` node type is reachable through at least one
   table role, **or** listed in a facet, **or** recorded in the ledger as
   uncategorised with a one-line reason. Nothing is silently outside the
   vocabulary.
3. Every node named in `roles.json` exists in `node-types.json`.
4. Declared containments hold (`_literal ⊆ _expression`; `_branch`, `_loop`,
   `_jump` ⊆ `_control_flow`).
5. **Role liveness:** every declared role matches at least one occurrence
   over the language's corpus sweep. Because matching is derivation-based
   (§2, fact 4), a role the grammar author forgot to thread at some position
   fails *silently* — zero matches over a large corpus is how it gets caught.
6. The cross-language rosetta suite passes (§5.4).

## 4. The grammars

### 4.1 Construction rules

- One crate per language. Grammar source is `grammar.js` plus `src/scanner.c`
  where the language demands an external scanner — all three initial
  languages do (indentation and f-strings; raw strings; template literals
  and JSX text).
- Every `grammar.js` imports the vocabulary from `treebank-core`'s
  `vocabulary/supertypes.js`. The term list is shared *code*, not shared
  convention: the import provides the closed list and helpers for the
  standard nestings; the grammar supplies the members.
- **Shared concrete names and fields.** The same construct gets the same
  node name and the same field names in every treebank grammar:
  `function_definition` (not `function_item`, not `function_declaration`),
  `class_definition`, `call_expression`, with the fixed field vocabulary
  `name:`, `parameters:`, `body:`, `condition:`, `value:`, `left:`,
  `right:`, `operator:`, `type:`, `arguments:`. So
  `(function_definition name: (_name) @n)` is one query for three languages.
  Grammars diverge only where constructs genuinely differ — never renaming
  or reshaping to force a match, because a tree that lies about syntax is
  worse than a tree that varies.
- `extras` carry comments and whitespace only.

### 4.2 One grammar per language, across versions

A language gets one grammar accepting the **union of its versions** — no
python2 crate, no per-edition Rust grammars.

- **Python**: 2.7 ∪ 3.x. The union adds the py2 `print` and `exec`
  statements, `except E, e:` clauses, backtick repr, and old-style octal
  literals, parsed alongside py3 syntax.
- **Rust**: editions 2015–2024 together. The real work is contextual
  keywords: `async`, `dyn`, `try`, `gen` are identifiers in older editions
  and keywords in newer ones, and the union grammar accepts both readings.
- **TypeScript**: every TS version, **and JavaScript** — TS is the union
  language of JS, so this is the same philosophy applied across a language
  boundary. One grammar source, **two generated parsers**: `typescript`
  (`.ts`, `.mts`, `.cts`) and `tsx` (`.tsx`, `.jsx`, `.js`, `.mjs`,
  `.cjs`). Two parsers because `<T>x` is a cast in `.ts` and an unclosed
  JSX element in `.tsx` — a genuine grammatical ambiguity, not a precedence
  problem; no single parse table exists.

#### When versions conflict, the latest version wins

A union grammar is not a promise to parse every version equally. Three cases,
and they are decided differently:

1. **The same text means different things in different versions.** The latest
   version's reading wins. `print >> f, x` is a py2 print statement and a py3
   expression; it parses as the expression. `print (x)` is a py2 statement
   and a py3 call; it parses as the call.

2. **A construct is valid only in an older version, and admitting it would
   change how CURRENT code parses.** The current language wins and the
   construct is rejected. In a GLR grammar an admitted old form is not an
   extra reading sitting quietly beside the others — it is a **fork at every
   occurrence of the token**, and forks can win. Measured, twice: letting
   `never`/`unknown`/`symbol` be identifiers in TypeScript fixed 3 files and
   broke 13, because the identifier reading created a competing generic-arrow
   fork that beat the type-argument reading. Supporting the past must never
   cost the present.

3. **A construct is valid only in an older version and costs nothing.** It is
   accepted — this is what the union is for. `except E, e:`, the py2 `print`
   and `exec` statements, backtick repr and old-style octal literals have no
   competing reading in any later version, so no current program is at risk
   from them.

The cases that fall under (2) are recorded per grammar in
`version_policy.json` and, because a policy nobody checks is a comment,
each one also gets a file in `test/negative/` — so the rejection is a gate,
not a note. The sweep reads that file and books matching failures as
`version` rather than `gap` (§4.3).

Which version a given file belongs to is deliberately **not** answered now.
The sweep records per-version oracle verdicts anyway (§4.3) — that verdict
vector is the hook a future `version_of()` builds on, and nothing more is
built today.

### 4.3 What "valid" means with multiple versions

A parse failure on real code is only a bug if the code is actually valid, so
every language carries **oracles** — reference parsers, pinned like
compilers: tool + version + flags + declared positions + a smoke test that
runs before any verdict is trusted.

With a version-union grammar there is one oracle per version family:

- **python**: CPython 3 `compile(src, path, 'exec')`, and CPython 2.7's
  `compile` where a python2 exists; when CI has no python2 binary, py2
  verdicts come from a frozen battery of known-valid/known-invalid files,
  and the ledger says so.
- **rust**: `syn` in-process for the sweep; `rustc -Zparse-only` per
  `--edition` for adjudicating anything `syn` and the grammar disagree on.
- **typescript**: `tsc` parse per dialect; V8 as a secondary oracle for
  plain `.js`.

Adjudication over the version set:

- **gap** — the grammar rejects a file that **any** version-oracle accepts,
  *and* the rejection is not a declared version-policy rejection (§4.2).
  Otherwise a bug; the union must cover every version it claims.
- **version** — the grammar rejects a file that only an OLDER version-oracle
  accepts, and `version_policy.json` declares that construct rejected. Both
  conditions are required: the declaration alone cannot suppress a failure
  that the CURRENT oracle calls valid, so a real gap can never hide behind a
  policy entry.
- **widening** — the grammar accepts a file that **every** version-oracle
  rejects. Sweeps cannot catch this direction (real corpora are almost
  entirely valid code), which is what the negative corpus exists for:
  `test/negative/` holds files invalid under *every* version, and
  `test/negative/<version>/` files that must stay invalid for that
  version's oracle — the guard against the union quietly becoming
  "anything parses".
- **mis-shape** — the grammar ACCEPTS a file and builds the wrong tree for
  it. None of the above can see this, and it is not a rare corner: five were
  found in TypeScript within a day of looking, including
  `new Date().getFullYear()` binding as `new (Date().getFullYear())` and the
  `catch` clause vanishing out of `try`/`catch` entirely. §5.6 is the check
  for it.
- the sweep stores the full per-oracle verdict vector per file.

### 5.6 The shape check (`treebank shape`)

A yes/no oracle can only catch the grammar REJECTING valid code. It is
structurally blind to the grammar ACCEPTING code and building the wrong tree
for it: those files parse cleanly, sweep cleanly, and ship. Before this
check, every silent mis-parse in this repository was found by accident, from
an adjacent file where the wrong reading happened to be illegal — `x as A & B`
parsed as `(x as A) & B` corpus-wide and surfaced only because
`x as A & { c?: B }` puts a `?` where an object literal cannot have one.

The reference parser already builds a tree and the sweep throws it away.
Keep the node BOUNDARIES from it and one property becomes checkable over
the whole corpus:

> for every node the reference parser reports, our tree has a node with
> exactly that byte span.

Four things make it work in practice:

1. **Boundaries, not names.** Comparing node names needs a correspondence
   table per language, which is where this kind of check usually dies — the
   table is large, subjective, and rots. Boundaries need no table: if tsc
   says something spans 15..20 and we have no node there, we disagree about
   the shape of the code, whatever either of us calls it.
2. **One-directional.** Our tree may have nodes the oracle does not; finer
   granularity is fine. What it may not do is fail to see a boundary the
   reference parser sees.
3. **Separator-insensitive.** Two parsers can agree completely and still put
   a `;` on different sides of a boundary. That is punctuation bookkeeping,
   and it is thousands of hits — trimmed away by rule, on both sides, rather
   than by allowlist entry.
4. **The allowlist is keyed on PAIRS**, `"<TscKind> <- <our_kind>"`, not on
   the oracle's kind alone. Ignoring `PropertySignature` outright would also
   silence a real `PropertySignature` disagreement elsewhere, which is how a
   check like this quietly stops working.

`shape_policy.json` per grammar holds the declared granularity differences,
each with its reasoning, and a `baseline_missed` ratchet. The ratchet is the
part that makes it a gate rather than a report, and it earns its keep: the
fix that raised the type operators above `PREC.cast` also lifted them above
`type_operator`, so `readonly string[] | undefined` silently became
`readonly (string[] | undefined)` in 119 files. The next shape run caught it.

Offsets are bytes on both sides. tsc counts UTF-16 code units, so the
conversion happens in the oracle script where the string is already decoded.

Today only TypeScript and JavaScript have a span oracle. `treebank shape`
says so rather than passing vacuously for the others.

Two oracle rules, both absolute. **An unreadable file is never an invalid
file**: an oracle that cannot read its input exits non-zero with no verdict,
because a verdict of "invalid" books the file as corpus noise, and an oracle
that answers "invalid" for files it could not open would silently convert
every grammar failure into noise and report a flawless grammar. And **an
oracle is proved by a negative battery, never by agreement**: agreement on
clean library code is worth nothing; only files that *should* be rejected
test whether the oracle can reject.

## 5. Testing — the invariant

Treebank's credibility claim is behavioural: *this grammar parses N real
files, agrees with the reference parsers on all but a ledgered list, rejects
what every version rejects, and carries the shared vocabulary in full.* Four
checks, run per grammar by `verify.sh` locally and in CI:

**I1 — Reproducible generation.** `grammar.js` + `scanner.c` +
tree-sitter-cli 0.26.12 reproduce the committed `src/` byte for byte. What
is published is exactly what the source generates.

**I2 — Corpus sweep, oracle-adjudicated.** Parse the full corpus; every
failure goes to the version oracles; the result is `gap_files = 0` or a
ledgered list with a reproduction for each. **A zero must be falsified, not
trusted**: the sweep is re-run against a deliberately mutated grammar (one
rule deleted), and must report gaps — a pipeline that cannot report non-zero
proves nothing by reporting zero.

**I3 — Negative corpus + conformance suites.** The per-version negative
files of §4.3 must all still be rejected. Where an official suite exists it
runs, and every known failure is ledgered: CPython's grammar and tokenizer
test corpora, rustc's parser test suite, the TypeScript compiler's
conformance suite.

**I4 — Vocabulary conformance.** `treebank roles` (§3.3): closed lists,
total node coverage, containments, manifest validity, role liveness, and
the rosetta suite.

### 5.4 The rosetta corpus

A directory of small parallel programs — the same behaviour written in
Python, Rust and TypeScript — each with an expected-roles file: assertions
like *"exactly 2 `_loop`, 3 `_declaration`, 1 `_callable`, and
`(function_definition name: (_name))` yields `f`, in all three languages."*
This is the executable form of the promise that the vocabulary means the
same thing everywhere, and it is the only check that catches a role threaded
in one grammar and forgotten in another before a consumer does.

### 5.5 Corpora

Per language, thousands of packages from the ecosystem's registry (PyPI,
crates.io, npm), fetched by rank, extracted to source files, swept whole.
Corpus composition is *declared* in the ledger, including what the corpus is
blind to — a registry corpus is biased toward well-formed, modern,
machine-formatted code, so python2 forms, encoding edge cases, and
deliberately hostile input are covered by the negative corpus and
conformance suites, and the ledger says which population covers what. A
clean sweep over a biased corpus is weak evidence on its own; the ledger is
where that weakness is written down instead of discovered.

## 6. Code organization

```
crates/
  treebank-core/              # the vocabulary, as code and as data:
    vocabulary/supertypes.js  #   the closed term list grammars import
    src/                      #   roles.json schema, facet query expansion,
                              #   the `treebank roles` checker
  treebank-python/
    grammar.js  src/scanner.c  src/ (generated, committed)
    roles.json  ledger.json
    test/corpus/  test/negative/  test/negative/<version>/
    bindings/                 # rust crate + wasm
  treebank-rust/              # same shape
  treebank-typescript/        # common/define-grammar.js + typescript/ + tsx/
  treebank-cli/               # fetch · rank · sweep · oracle · roles ·
                              # negative · ledger
tools/consumer-test/          # downstream crate + wasm smoke tests
test/rosetta/                 # parallel programs + expected-roles files
```

- **treebank-core** is the single source of truth for the vocabulary: the JS
  module every grammar imports, the Rust library the CLI and consumers use,
  and — compiled to wasm — the same facet expansion in the browser.
  Vocabulary versions are semver on this crate.
- **Generated `src/` is committed** in each grammar crate; I1 keeps it
  honest. There is no build directory, no patch series, no vendored
  anything.
- **wasm is a first-class artifact**: every grammar crate publishes
  `tree-sitter-<lang>.wasm` alongside the Rust crate, and `treebank-core`
  ships a JS/wasm package that loads them and provides facet-aware queries.
  `roles.json` travels inside both the crate and the npm package.
- **`ledger.json`** is each grammar's evidence file, machine-validated by
  `treebank ledger`: language, versions covered, one pinned oracle per
  version family, corpus description and its declared blind spots, sweep
  results, conformance-suite results, and the vocabulary's uncategorised
  list. Every number this document promises lives there, next to how it was
  measured.

## 7. Design decisions

1. **Two tiers, because the parse table forces it.** Structural roles are
   supertypes (occurrence-level, generate-time-enforced); the three
   cross-cutting facets ship as a checked manifest with query expansion in
   `treebank-core`. The alternative — restricting the vocabulary to only
   what threads — was rejected to keep the facet queries; the other
   alternative — everything in query files — was rejected because it is
   exactly the drifting query layer this design exists to kill.
2. **One `_declaration`,** with or without body; `_binding` is a separate
   facet. A `_signature` facet can be added later, additively.
3. **`treebank-typescript` covers JavaScript,** as two dialect parsers
   generated from one source.
4. **Shared concrete names and fields across grammars,** diverging only
   where syntax genuinely differs.
5. **Underscore spelling for every vocabulary term;** concrete nodes never
   start with an underscore.
6. **tree-sitter-cli pinned at 0.26.12**, matching the `tree-sitter`
   runtime library consumers link, so the version that generates and the
   version that runs are the same. Bumping the pin is treated like a
   grammar change — full sweep, before/after numbers, ledger entry —
   because regenerating with a different CLI can silently change what the
   grammar accepts. The pin sat at 0.25.10 while 0.26.x shipped Unicode
   identifier tables that wrongly dropped XID_Start characters; 0.26.12
   was re-measured against that (15 XID_Start probe characters, identical
   behaviour) and against all four corpora — 80,391 files, byte-identical
   verdicts — before the bump.

   That skew was not free while it lasted: a *query* valid under 0.25
   could be an impossible pattern under 0.26, which is how the
   supertype-field rule in `treebank-rust/ledger.json` came to light.

## 8. Order of work

`treebank-core` (vocabulary + checker + expansion) → **Python** → **Rust** →
**TypeScript**.

Python first: the corpus infrastructure is cheapest to stand up, the oracle
is the cheapest to extend across versions, and its external scanner
(indentation, f-strings) is the hardest *small* scanner — the right first
test of whether from-scratch scanners are sustainable. For calibration,
mature tree-sitter grammars for these languages run roughly 1,300–1,800
lines of `grammar.js` plus a 400–550-line scanner each, and the version
union adds real size on top (py2 statement forms; Rust's edition-contextual
keywords). The scanners are where the risk concentrates: indentation, raw
strings, template literals, regex-vs-division and other ASI-adjacent token
decisions, JSX text. One language at a time, with the sweep and the roles
checker live from the first week, is what keeps that risk measured.
