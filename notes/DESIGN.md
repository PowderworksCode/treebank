# Treebank — design

Treebank is a set of tree-sitter grammars written from scratch and owned
outright — no upstream grammar repos, no forks, no vendored trees anywhere in
the system. **Initial languages: Python, Rust, TypeScript.**

Four ideas define it:

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
4. **Everything a file can say about itself, and nothing more.** Beyond the
   parse, treebank answers what a grammar alone can answer about one file:
   which definitions it holds, what names they bind, which reference resolves
   to which binding, and what identity those carry across commits (§9). A
   toolchain may check any of it and is never required to produce it. Anything
   needing a package graph or a type checker belongs to a layer above.

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
the closed list). `treebank` expands facet queries at load time —
`(_callable)` becomes the concrete alternation `[(function_definition)
(lambda) …]` — so the query surface is uniform across tiers. Type-level is
the *correct* semantics for facets: a `function_definition` is callable
wherever it occurs.

The facet tier is a compromise forced by the parse table's limits, kept
honest by being closed, machine-checked, and shipped inside the same crate
as the grammar — not a hand-maintained query pack.

### 3.1.1 A term's tier is per-grammar

The tier is a property of the **grammar**, not of the term. A term the
vocabulary marks `either_tier` may be delivered by either mechanism, and
each grammar picks.

The forcing case is `_parameter`. Python's parameter list is ordered by the
grammar itself — `def f(a=1, b)` and `def f(**kw, a)` are SyntaxErrors out
of CPython's own parser, not later semantic checks — and Rust's is too: a
`self` receiver is only ever first, a C-variadic `...` only ever last. A
supertype is one alternation repeated by commas, so a grammar that keeps
`_parameter` in the table tier necessarily accepts all four of those. The
ordering has to be spelled out as a chain of "what may still follow" rules,
and once it is, the parameter node types no longer share a derivation and
tree-sitter cannot collect them under a supertype. TypeScript does not
partition the position, so it keeps `_parameter` as a real supertype.

**Why this is one meaning and not two.** Occurrence-level and type-level
membership agree exactly when every member of the term is a concrete node
type that occurs nowhere else. Python's six (`parameter`,
`star_parameter`, `double_star_parameter`, `keyword_separator`,
`positional_separator`, `tuple_parameter`) and Rust's three (`parameter`,
`self_parameter`, `variadic_parameter`) all satisfy that, so the facet
selects precisely the nodes the supertype would have. That is also the
condition under which the demotion is *permitted* — so the choice is
between two implementations of one meaning, never between two meanings.
`_argument` fails the test in all three languages, because a positional
argument is a bare `_expression` with no type of its own; it stays in the
table tier until it has a node of its own.

**What it costs.** Native queryability stops being uniform: a consumer
using the parser through raw tree-sitter, with none of treebank's crates,
can write `(_parameter)` against TypeScript and not against Python. This
fails *loudly* — a supertype a grammar does not declare is a `QueryError`
at `Query::new`, not a silent zero-match — and everything going through
`treebank`, which expands facets at load time, sees no difference at
all. The rosetta suite asserts that directly: `(_parameter) @p` counts the
same in all three languages with Python and Rust on the facet tier and
TypeScript on the table tier.

**The rule.** *Use the table tier wherever the language leaves the position
unpartitioned; it is stronger and needs no treebank machinery. Demote to the
facet tier only in the grammars that must partition it, and only when every
member of the term is a concrete node type occurring nowhere else.* The
alternative — moving a term globally the first time any language partitions
it — erodes the table tier to whatever no supported language ever
partitions, a race to the bottom set by the most constrained grammar in the
set.

Demotion is declared, not inferred: the grammar lists the term in
`roles.json`'s `demoted` map with the reason its language forced it, and the
checker rejects a demotion the vocabulary does not allow, one without a
reason, one that is also a declared supertype, and one with no facet
members. Without that, dropping a supertype by accident would be
indistinguishable from demoting one on purpose.

### 3.2 The terms

The list is **closed**. A grammar may omit terms its language lacks
(Python declares no `_type`; its annotations are ordinary expressions). It
may not invent terms. Adding a term is a vocabulary change, versioned in
`treebank`, applying to every language at once.

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
| `_callable` | defines something invocable | `function_definition`, `lambda` (py); `function_definition`, `closure_expression` (rs); `function_definition`, `arrow_function`, `method_definition`, `function_expression` (ts) |
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

### 3.2.1 On the vocabulary version

`vocabulary.json` carries a `version`, and every `roles.json` declares the
one it targets. That string is an **identity, not a compatibility promise**,
and it is deliberately not bumped per change while the vocabulary is still
being worked out — a number climbing through 0.4 in a fortnight claims a
stability nothing here has yet, and there is no consumer outside this
repository to claim it to.

Nothing is lost by holding it still, because the version is not what
protects a stale manifest. The structural rules in §3.3 are: a term removed
or renamed fails rule 1 (supertypes ⊆ table tier) or rule 5 (facet keys ⊆
facet tier); a term that moved tier fails the demotion rules; a node left
uncovered fails rule 2. Every *breaking* vocabulary change is caught by what
the manifest says, not by what it claims to target. Omitting a newly added
term is caught by nothing, and should not be — a grammar may always omit
terms its language lacks.

Start versioning for real when something outside this repository depends on
the vocabulary. Until then the field exists to name the vocabulary, not to
grade it.

### 3.3 What the checker enforces (`treebank roles`, in CI)

1. Declared supertypes ⊆ the closed table-tier list; `roles.json` keys ⊆ the
   closed facet list, or a term this grammar declares as `demoted` (§3.1.1).
   A demoted term must be `either_tier` in the vocabulary, must carry a
   reason, must have facet members, and must **not** also be a declared
   supertype — a term lives in exactly one tier per grammar.
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
- Every `grammar.js` imports the vocabulary from `treebank`'s
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

### 4.2 One grammar source per family; tables as narrow as the evidence wants

A language **family** — a language with its versions, or a language with its
dialect siblings — shares one grammar source, forever. What varies is how
many parse tables that source generates and how many rows the registry
presents. Measurement decides both, and `notes/dialects.md` carries the full
argument with the incidents behind it; this section is the rule it produced.

- **Python**: 2.7 ∪ 3.x in one table today. The union adds the py2 `print`
  and `exec` statements, `except E, e:` clauses, backtick repr, and
  old-style octal literals, parsed alongside py3 syntax.
- **Rust**: editions 2015–2024 together. The real work is contextual
  keywords: `async`, `dyn`, `try`, `gen` are identifiers in older editions
  and keywords in newer ones, and the union grammar accepts both readings.
- **TypeScript**: every TS version, **and JavaScript**, in one table —
  `typescript` and `javascript` are two rows over one parser. The dialect
  pair this section once planned (`typescript` / `tsx`) stayed unbuilt
  because `<T>x` casts measure at approximately zero corpus files, and
  `treebank-typescript/ledger.toml` prices that as a standing quote rather
  than a closed question.

#### Two kinds of difference, and only one buys a table

**An accept-set difference** admits text the sibling forbids while building
the same tree wherever both admit it. `print x` builds a `print_statement`
no python3 file can contain, and no python3 parse moves because that rule
sits elsewhere in the table. One table serves both variants, and the
narrower one rejects after the parse (below).

**A reading difference** needs the same bytes to become a different token or
a different tree. Zig removed `async`, and keyword extraction happens in the
lexer, so wherever the keyword reading is valid the identifier reading
cannot lex — 116 corpus files want the operator, 54 want the identifier, and
`treebank-zig/ledger.toml` names the 4 files that fall. `<T>x` is a cast in
`.ts` and an unclosed JSX element in `.tsx`. `'It\'s'` closes at different
bytes under MySQL and PostgreSQL, and `SELECT 1--2` is arithmetic to one and
a comment to the other. No single parse table holds either reading pair, and
no manifest repairs one afterwards, because both mechanisms run downstream
of the parse.

**Only a reading difference at measured incidence buys a second parse
table** — measured, because TypeScript priced its split at approximately
zero files and kept one table, and zig priced its at 4 and kept one too.

#### Rows: what a corpus and an oracle earn

A **row** is a registry entry: a canonical name, source extensions, a corpus
lock, an oracle, a negative corpus, sweep numbers in the family ledger, and a
fetchable pack. A dialect or a version family earns one by bringing **its own
corpus and its own oracle** — the rule `crates/treebank-lang` already
applies, and the reason `javascript` holds a row served by the typescript
crate (a different npm population, its own checker) while Terraform amounts
to three file extensions on `hcl` (neither).

A row need not be a parse table of its own. Rows may share one, exactly as
`javascript` shares typescript's.

The version axis takes that rule unmodified. `python3` has its own oracle
(CPython 3, pinned) and its own population; `python2` has CPython 2.7.18
built from source and a vintage population of its own. Both halves of the
union already pay for their oracles, and neither answers to a name.

#### The registration ladder

Take the highest rung that expresses the variant — the discipline
`notes/field_guide.md` §1 applies to ambiguity:

0. **Extensions on an existing row.** The variant adds semantics, not
   syntax: Terraform on `hcl`, `.zon` on `zig`.
1. **A row over a shared parser.** Own corpus and oracle, and the shared
   table already reads its text correctly. `javascript` today. Where the
   row's accept-set is narrower than the table's, a narrowing manifest
   closes the difference.
2. **A row with its own parse table, generated from the family's shared
   source.** A reading difference at measured incidence earns this, and
   nothing else does.
3. **A row with its own crate, extending a base grammar.** The variant is a
   language of its own with a superset community: `cpp` over `c`, through
   tree-sitter's own inheritance, because a second copy is a copy that
   drifts.

**Refusal** is the fifth answer and carries its price in the ledger: HCL's
JSON profile, JSON5, NDJSON, T-SQL, PerfettoSQL. A variant that is a
different grammar wearing a familiar extension, or one whose claim rests on
documentation rather than on an oracle, gets no row.

Two brakes keep the row set closed. A row's corpus must be a population that
exists *because the variant does* — ranked and locked on its own, never a
filter over another row's lock — and its oracle a separately pinned reference
implementation. Minor-version narrowing inside a family (`match` arrives in
3.10) earns a manifest entry, never a `python3.10` row.

#### Within a row, the latest version wins

A row that spans versions accepts their union, and that union is not a
promise to parse every version equally. Three cases, decided differently:

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

Each row records its case-(2) constructs in `version_policy.toml` and,
because a policy nobody checks is a comment, gives each one a file in
`test/negative/` — so the rejection is a gate, not a note. The sweep reads
that file and books matching failures as `version` rather than `gap` (§4.3).
An entry whose `valid_in` lies wholly in another row stops being policy and
becomes that row's ordinary negative fixture.

#### Across siblings, never

Versions form a line, which is what makes "the latest wins" a decision rule.
Siblings form no line: MySQL does not obsolete PostgreSQL, and JSONC does not
obsolete JSON, so nothing arbitrates where their readings collide and the
union becomes a coin somebody weights by hand, per collision, forever.

The cost is not only a missing rule. **A union across siblings blinds the
instruments,** and this repository has measured that twice. A grammar wider
than every oracle that exists is wide exactly where no oracle can contradict
it; `treebank-json/ledger.toml` refuses JSONC on that ground. The concrete
case came first: an abandoned SQLite ∪ PostgreSQL ∪ MySQL grammar accepted
every dialect's divergences everywhere, which surrenders the negative corpus
by construction — one table cannot reject `SELECT a # b` for postgres while
accepting it for mysql — and adding one more oracle to it moved adjudicated
gaps from 48 to 364 in a single sweep, with nothing about the grammar
changed. **A union across siblings is never built.**

#### Narrowing a shared table

A rung-1 row narrower than the table it shares carries `narrowing.json` in
the family crate, one key per row, listing the **out-of-row occurrences** as
patterns — for `python3`: `print_statement`, `exec_statement`, backtick
repr, the old octal literal, the `except E, e:` clause shape. The pack ships
it beside `roles.json` and expands it the way it expands facets, so
`Pack::fetch("python3")` resolves to the shared parser plus the manifest and
a narrowed parse is parse-then-scan: the tree returns carrying its
out-of-row occurrences, or the call refuses the file, at the consumer's
option.

Name the construct rather than its position. `fuzz_policy.toml`'s
`node_kind` matcher already works this way for the same reason a positional
prefix cannot see a construct nested inside another statement, and the
loader there rejects a kind the grammar never produces. The checker holds
`narrowing.json` to the `roles.json` standard: every pattern compiles
against the row's grammar, every pattern matches at least one file in the
row's negative corpus, and the sweep cross-checks the manifest against the
verdict vector — for every corpus file another row's oracle accepts and this
row's rejects, at least one of this row's patterns must fire.

A manifest can never change a reading, and that limit is declared rather
than discovered: a `python2` manifest over the union table still hands its
consumer the call reading of `print (x)`, the py3 tuple reading of
`print >> f, x`, and no parse at all for `True = 5`. That residue is the
rung-2 case for a second table, priced and waiting.

#### Which version a file belongs to

`version_of()` is now buildable and was not before. The sweep records the
per-oracle verdict vector per file (§4.3), the manifests say which
occurrences fall outside which row, and an entry may carry the version bound
it narrows away (`match_statement` → 3.10) so the answer can name a floor as
well as a family. One measured caution bounds what it may claim: declaring
python's five py2-union kinds moved 31 fuzz findings out of undeclared and
six should not have moved, because a py2-only construct nested inside a
py3-only one is valid in *neither* version. A py2-only node kind in a file
therefore does not make the file py2. A manifest entry answers whether an
OCCURRENCE sits outside a row; which row a FILE belongs to stays the
oracle's question.

### 4.3 What "valid" means, per row

A parse failure on real code is only a bug if the code is actually valid, so
every language carries **oracles** — reference parsers, pinned like
compilers: tool + version + flags + declared positions + a smoke test that
runs before any verdict is trusted.

**A row's own oracle defines valid, for that row.** Each row sweeps its own
locked population, and a file its own oracle rejects is noise. Where a row
itself spans versions the legs remain, and a file is valid for the row when
any leg accepts — zig runs its two release endpoints, python runs CPython 3
and CPython 2.7. That union oracle belongs inside a row and never across
siblings, for §4.2's reason: across siblings it would excuse precisely the
over-acceptance the sweep exists to find.

The oracles, one per version family within the row: 

- **python**: CPython 3 `compile(src, path, 'exec')`, and CPython 2.7's
  `compile` where a python2 exists; when CI has no python2 binary, py2
  verdicts come from a frozen battery of known-valid/known-invalid files,
  and the ledger says so.
- **rust**: `syn` in-process for the sweep; `rustc -Zparse-only` per
  `--edition` for adjudicating anything `syn` and the grammar disagree on.
- **typescript**: `tsc` parse per dialect; V8 as a secondary oracle for
  plain `.js`.

Adjudication, over the row's version set and no wider:

- **gap** — the grammar rejects a file that **any** of the row's
  version-oracles accepts, *and* the rejection is not a declared
  version-policy rejection (§4.2). Otherwise a bug; the row must cover every
  version it claims.
- **version** — the grammar rejects a file that only an OLDER version-oracle
  accepts, and `version_policy.toml` declares that construct rejected. Both
  conditions are required: the declaration alone cannot suppress a failure
  that the CURRENT oracle calls valid, so a real gap can never hide behind a
  policy entry.
- **widening** — the row's parser accepts what the row's oracle rejects.
  Scoping this to the row is what keeps `mutate` and `fuzz` able to see: a
  backtick identifier found against a postgres row is a widening again,
  where a three-dialect union had defined it as a feature. Sweeps cannot
  catch this direction (real corpora are almost entirely valid code), which
  is what the negative corpus exists for: `test/negative/` holds files
  invalid under *every* version the row claims, and
  `test/negative/<version>/` files that must stay invalid for that version's
  oracle — the guard against a row quietly becoming "anything parses".
- **mis-shape** — the grammar ACCEPTS a file and builds the wrong tree for
  it. None of the above can see this, and it is not a rare corner: five were
  found in TypeScript within a day of looking, including
  `new Date().getFullYear()` binding as `new (Date().getFullYear())` and the
  `catch` clause vanishing out of `try`/`catch` entirely. §5.6 is the check
  for it.
- **hidden gap** — the grammar rejects a file the oracle calls invalid, but
  the PARSER alone would have accepted it. Python's oracle judges with
  `compile()`, which also runs the checks CPython performs after parsing —
  `return` outside a function, a bare `except:` that is not last — and that
  choice is deliberate, because it keeps deliberately-broken test fixtures
  out of the gap count. Its cost is stated in the same place: a file invalid
  for a post-parse reason AND holding a real grammar gap is recorded as
  noise. The sweep measures that cost rather than assuming it small, by
  asking `ast.parse` about every noise file, and reports the count. It is
  currently zero; the one construct it found is fixed.
- the sweep stores the full per-oracle verdict vector per file. Verdicts
  from OTHER rows' oracles are recorded and adjudicate nothing: they check
  the narrowing manifests and answer `version_of()` (§4.2).

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

`shape_policy.toml` per grammar holds the declared granularity differences,
each with its reasoning, and a `baseline_missed` ratchet. The ratchet is the
part that makes it a gate rather than a report, and it earns its keep: the
fix that raised the type operators above `PREC.cast` also lifted them above
`type_operator`, so `readonly string[] | undefined` silently became
`readonly (string[] | undefined)` in 119 files. The next shape run caught it.

Offsets are bytes on both sides. tsc counts UTF-16 code units, so the
conversion happens in the oracle script where the string is already decoded.

Every language has a span oracle, and each came from a different place:

- **typescript / javascript** — `ts.createSourceFile`, walked with
  `forEachChild`. Positions are UTF-16 code units, converted to bytes in the
  oracle script where the string is already decoded.
- **python** — CPython's `ast`. Columns are already UTF-8 byte offsets within
  a line, so only the line starts have to be added back. Files whose PEP 263
  coding declaration or BOM means `ast` reports offsets into a byte string
  that is not the one on disk are skipped, not guessed at.
- **rust** — `syn`, in-process, with `proc-macro2`'s `span-locations` feature
  turning every span into a file-relative `byte_range()`.

The rust choice is worth writing down, because the obvious candidate is the
wrong one. **HIR is post-desugaring**: `for` and `while` become `loop` plus
`match`, `?` becomes a match on `Try`, closures are rewritten. Comparing a
surface tree against it would report a disagreement at every one of those,
and none of them is a parser defect — HIR answers *what does this mean*, and
this check asks *how is this written*. rustc's own AST is the right level but
not reachable: `-Zast-json` was removed in 2020, and `-Zunpretty=ast-tree`
prints session-global `BytePos` that would have to be mapped back per file,
on nightly. `syn` is the right level, already a dependency, and needs no
subprocess at all.

#### The node mapping

Boundaries are half the question. Where the two parsers agree on the bytes,
what is left is whether they agree on WHAT is there — and they can disagree
completely while covering identical spans. `foo();` parsed as a bodyless
function declaration occupies exactly the bytes of the call it should be, so
nothing about the boundaries is wrong; only the names are, and only a table
can say so.

`node_map.json` per grammar declares what each reference-parser kind is
expected to be in our tree. Three properties make it useful rather than
ceremonial:

- **It is a set, not a function.** A chain like `expression_statement >
  call_expression > identifier` can have three nodes on one span, and the
  oracle names one of them. An oracle node passes when ANY of our kinds at
  its span is listed, which is why only the CORE kind needs an entry and
  wrappers come along free.
- **It is not required to be one-to-one.** Several of our kinds answer
  `Expr::Lit`; `function_definition` answers four TypeScript kinds. What it
  is required to be is **total and declared** — every oracle kind the corpus
  produces has an entry, and anything unlisted is reported as a hole rather
  than assumed away.
- **`"*"` marks a wrapper** whose span coincides with its only child, so the
  child's entry carries the check. One entry uses it: syn's `Stmt::Expr`,
  which for a block's tail expression spans exactly the expression while we
  build no statement node at all. It is a claim about the oracle's shape,
  not a way to silence a kind.

The table is bootstrapped from the corpus — an empty `map` makes every kind
report as unmapped together with the kinds actually found at its span — and
then written by hand, because a table generated from observation encodes
whatever the grammar does today, bugs included. Only the token entries are
mechanical, and they say so: `AmpersandAmpersandToken` is `&&` and there is
no judgement in that.

#### The round trip (`treebank roundtrip`)

The corpus is written by people, and people write a construct the usual way.
A grammar can handle every spelling that appears in 139,205 files and still
miss the one the language's own printer emits — parentheses dropped where
the tree does not need them, quotes and spacing normalised, a trailing comma
gone.

`ast.unparse` and `ts.createPrinter` render the reference tree back to source
in one canonical spelling. Re-parsing that costs a single pass and doubles
the corpus with source no human wrote. A failure is a real gap that no amount
of real source would ever show; an absence of failures is evidence rather
than silence, because the input genuinely differs from what was already
tested.

`syn` has no printer in the dependency set, so Rust is skipped rather than
approximated.

#### The lexical layer

`ast` is not the only oracle CPython ships. `tokenize` is a second one, a
level below, and it is the only reference we have for the LEXER — two
parsers can build identical trees over a token stream they disagree about,
and nothing above this level would notice a numeric literal form, an
operator glued together, or a string prefix read differently.

The claim is one-directional, like the node one: **the reference lexer's
token boundaries must be a subset of ours.** We may be finer — a string is
one token to CPython and `string_start`/`string_content`/`string_end` to us
— but never coarser, because coarser means we glued together two things the
language keeps apart. Where we are coarser on purpose it is declared, and
there are three such places, all in f-strings: `!r` is one `type_conversion`
to us and two tokens to CPython, and `{{` is an `escape_sequence` where
CPython folds it into the surrounding text.

tsc and syn expose no separate token stream with positions, so they report
none and the check is skipped for them.

#### Error positions (`treebank errors`)

Every other check is about which files we accept and what tree we build.
None looks at the REJECTIONS, and a grammar can reject exactly the right
files while pointing at a wildly wrong offset. That costs twice: an editor's
error recovery is only as good as the position it is given, and every gap
investigation starts by reading the first ERROR node, so a misplaced one
sends the reader to the wrong construct.

The corpus already exists and was being discarded — the files the sweep
books as *noise* are exactly the ones both parsers reject. No claim is made
that the offsets should be equal: two parsers legitimately notice a problem
at different points, and a token or two apart is normal. What is worth
knowing is the distribution, and especially the tail, because a rejection
hundreds of bytes from where the reference parser looked is one nobody can
act on.

#### Mutation (`treebank mutate`)

The sweep measures ONE direction. It takes the corpus, asks which files we
reject, and adjudicates each with a reference parser — a strong measurement
of rejects-valid-code that says nothing about the other direction, because a
corpus of real source is almost entirely valid and offers a too-permissive
grammar nothing to trip over.

The other direction was measured against `test/negative/`: eighteen
hand-written files for python, fourteen for rust, thirteen for typescript.
Set against 139,205, that asymmetry was the weakest part of the claim, and
it pointed the wrong way — optimising a pass rate drifts *toward* accepting
more, and the only guard was a list somebody had to think of entries for.

`treebank mutate` mutates real files mechanically (delete a token, duplicate
one, swap adjacent ones, substitute another token from the same file), parses
each mutant, and asks the oracle **only about the ones we accept**. Where the
oracle rejects what we accept, that is a widening.

Three things make it sound and affordable:

- **The mutants do not have to be reliably invalid.** Mutants both parsers
  accept are simply uninteresting, and there is no need to know in advance
  which is which.
- **Only files the oracle already accepts are mutated.** Without that the
  method is unsound: a file the reference parser rejects produces mutants it
  also rejects, and every one reads as a widening. The first run reported
  exactly that.
- **Only accepted mutants cost an oracle call**, which is most of the reason
  it is cheap — roughly three quarters get rejected by the grammar first.

Mutation happens at OUR token boundaries rather than at byte offsets: cutting
in the middle of an identifier mostly yields a different identifier, which is
still valid and teaches nothing. Runs are seeded and reproducible, because a
fuzzer nobody can re-run is a fuzzer whose findings cannot be confirmed.

#### The field mapping

Nodes are still only part of the structure. Two trees can agree on every
span, every kind, and every nesting relation and still attach the children
under different names — `orelse` where `body` belongs is a program and its
opposite, with nothing else to tell them apart. Field names are also what a
consumer reads: they are how a query asks for the *condition* of an `if`
rather than its third child.

`field_map.json` per grammar declares where each reference-parser labelled
edge lands in ours, in four forms:

- `["right"]` — the oracle's child IS the child under our `right` field.
- `["body>"]` — it lives somewhere UNDER our `body` field. This is what a
  list field looks like when we wrap it: CPython's `ClassDef.body` is a list
  of statements and ours is one `block` node holding them. The weaker claim
  is the true one there, and it still checks something.
- `["="]` — the oracle's child has the same span as its parent, so there is
  no edge of ours to label. tsc's `TypeReference.typeName` on a bare `Foo`
  points at `Foo` itself.
- `[]` — we attach this child positionally, with no field name at all.

The last form is the interesting one. Roughly half of CPython's labelled
edges and more than half of tsc's are `[]` here, and that is a finding
rather than a formality: **an unnamed edge is one a consumer cannot query by
role.** A call's `function` is labelled and its arguments are not; a
`return`'s value has no name. Some are deliberate — positional lists, and
slice bounds where the empty forms have to stay expressible — and some are
gaps. Each entry carries which.

Matching a parent is not simply matching a span, because the two parsers
disagree about where a node begins and ends in four separate ways, each of
which a real difference forced: the oracle may start a node LATER than we do
(CPython's `FunctionDef` at `def`, ours at the first decorator), start it
EARLIER (tsc keeps `export` inside the declaration, we wrap it), END it
earlier (CPython's `arg` is `x: int` where our `parameter` is `x: int = 1`),
or place its child one level INSIDE ours (CPython has no
parenthesised-expression node).

`syn` reports no edges at all — it has no generic field reflection — so Rust
asks nothing here rather than inventing labels.

Three comparison rules keep the signal usable, and all three are rules
rather than allowlist entries — each was added for one language and then
left the others' numbers unchanged, which is the test that it describes
trivia rather than papering over a difference:

1. **Separator-insensitive** — two parsers can agree completely and still put
   a `;` on different sides of a boundary.
2. **Trailing trivia** — a comment has to belong to somebody and they need
   not agree on whom. Uses tree-sitter's own extra flag, so the checker knows
   nothing about comments in any particular language.
3. **Leading trivia** — the mirror, and Rust forces it: `syn` turns a `///`
   doc comment into a `#[doc]` attribute *inside* the item, while we keep it
   as an extra in front. Every prefix of the run of leading extras counts,
   not just the longest, because a file that opens with `//!` and then
   documents its first item has two contiguous extras and the oracle takes
   only the second.

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

### 5.9 Generating from the grammar (`treebank fuzz`)

Every other check starts from source somebody wrote. The sweep reads the
corpus, `mutate` perturbs it, `roundtrip` reprints it — all three are
bounded by what the corpus happens to contain. That bound bites hardest in
the accepts-invalid direction, because real source is *valid*: no quantity
of it can demonstrate that we reject what the language rejects.

So generate instead. `grammar.json` is already an EBNF syntax tree, which
makes it a generator as well as a description — a random derivation is a
walk that chooses branches and emits terminals. **No unparser is needed in
this direction; the grammar is the emitter.** Then the oracle judges, and
anything we accept that it rejects is a widening.

**Soundness, given that the generator is not faithful.** Joining tokens with
spaces is a lie — `'a` is a lifetime and `' a` is not — so some derivations
produce text whose tokenisation differs from the derivation behind it. This
does not weaken a finding. A case is reported only when *our parser accepts
the text* and *the oracle rejects it*, and that pair is a widening whatever
derivation produced the bytes: accepting a program the language does not is
the defect, and how we came to type it is irrelevant. Infidelity costs
yield, never correctness — an unfaithful derivation we then reject is
discarded before it can be reported, which is most of them.

**Shrinking is over the choice tape, not the program.** Generation consumes
a byte tape and is deterministic in it, so shrinking searches for a shorter,
smaller tape that still reproduces — Hypothesis's model rather than
proptest's typed `Strategy`. That is the right fit here because the grammar
is *runtime data*: a typed strategy would have to be written per grammar,
while a tape does not care what it drives. Running off the end of the tape
yields the first alternative, so a truncated tape still produces a complete
program and shrinking can cut freely without emitting half a sentence.

**Ask the parser, not the compiler.** Where an oracle can separate the two,
`fuzz` uses `validate_syntax_only`. The first python run made the reason
plain: nearly every finding was `break`, `yield` or `* x` at module level —
all of which CPython's *parser* accepts and its *compiler* rejects. "`break`
outside a loop" is not a syntax error, and a tree-sitter grammar has no
business tracking loop nesting in order to produce one. Judged by
`compile()` the check mostly rediscovers CPython's semantic pass; judged by
`ast.parse` it reports what it is for. Where the reference tool has no
parse-only mode — rust's `syn` has none — it falls back, and the report says
which question was asked.

**Coverage.** The fuzzer reports which of the grammar's alternatives its
derivations reached, keyed on `(choice site, alternative)` — a number the
check had no way to state before, and an uncovered list that names
constructs in our own grammar nothing has exercised. That list is the
actionable half: it doubles as a to-do list for hand-written corpus tests.

Guidance is AFL's idea, and the measured result is worth recording because
it is smaller than the idea suggests. Mutating tapes toward coverage was
tried first and was WORSE than pure random at low budget (74.7% against
78.7% at 1,000 iterations) and identical by 8,000: AFL searches for
coverage because it cannot see inside the program, while a grammar fuzzer
already holds the list of unreached alternatives and can simply take them.
Seeking them by construction does win — 82% against 78.7% at 1,000, 98.9%
against 97.9% at 8,000 — but only after a bug was fixed that made the whole
idea look worthless: the seeker recorded a byte per choice while plain
generation also consumes one per repeat, so the tape it produced decoded
to a different program.

And the honest limit: **better coverage did not find more widenings.** 166
distinct findings either way at 8,000 iterations. The widenings cluster in
a few families that random derivation already reaches, and the alternatives
guidance adds are ones where the grammar happens to be right. The coverage
number and the uncovered list are worth having; the guidance is worth about
a thousand iterations of budget, and no more than that on this evidence.

**Steering at what the corpus never shows** (`treebank kinds`, then
`fuzz --rare`) is off by default, because it was measured across four
languages and helps exactly one.

A construct no corpus file contains is one no oracle has ever been asked
about, so a bug there survives the sweep, `mutate`, `roundtrip` and
`reformat` alike — every check that starts from real source. That argument
is sound, and it is not enough:

| | rare kinds | findings | paired |
|---|---|---|---|
| java | 2 never, 2 thin | **+17.9%** | 25 wins of 30 |
| rust | 2 never, 2 thin | −21.1% | 0 wins of 8 |
| typescript | 7 never, 11 thin | −18.7% | 0 wins of 8 |
| python | **none** | — | 297,612 files exercise all 104 kinds |

What decides it is what the rare set *is*. Java's is a coherent
under-modelled region: `guard`, `unnamed_pattern` and `record_pattern` are
the whole of java 21 pattern matching, absent from a quarter of a million
files. Rust's is `yield_expression`, an unstable feature `syn` rejects
anyway, and `shebang`, which is one line. TypeScript's is seven unrelated
corners at already-98.8% coverage.

Steering concentrates the budget, and concentration pays only when the
region is both untested and large enough to hold bugs. Otherwise it costs
the diversity that finds them. Python's row is the cleanest evidence that
the corpus can simply be complete: there is nothing to steer toward.

**Declared widenings.** Some over-acceptance is deliberate: python's grammar
is 2.7 ∪ 3.x by design, so `print x` is a widening against py3's parser and
is meant to be one. Left undeclared, that single decision dominates every
run and buries the findings that are not decisions. Each grammar may carry a
`fuzz_policy.toml` naming what it accepts on purpose, matched narrowly
against a prefix of the shrunk program — the same discipline
`shape_policy.toml` uses, for the same reason: a blanket ignore silences the
real finding that arrives next month wearing similar clothes.

Minimal examples also collapse together, so the tape doubles as the
clustering key. The first run over rust reduced 474 findings to 156 distinct
programs, and those to a handful of causes — one alternation putting `mut`
where only visibility belongs, `metavariable` reachable outside a macro
body, an `extern` ABI accepting a byte string. This is the difference from
`mutate`, which reports *a corpus file that does it*: here the report is
`mut use r#XX ;`.

### 5.10 Reformat invariance (`treebank reformat`)

The sibling of the round trip, asking the opposite question of the same two
tools. A *printer* renders from the tree and never sees the original bytes,
so it asks whether we handle the canonical spelling. A *formatter* is
text-to-text — it reflows a token stream it never stopped holding, keeps
comments and keeps the author's spelling — so it asks whether **layout moves
our tree**, which it must not. A rule that reads whitespace it should not,
or a token that only lexes when it abuts its neighbour, shows up here and
nowhere else.

**Only files the formatter changed in whitespace alone are compared**, and
that restriction is the check rather than a detail of it, because a
formatter is *not* tree-preserving. rustfmt reorders `use` declarations,
rewrites `extern {` into `extern "C" {`, adds a semicolon after a tail
`return`, and collapses `|x| { f() }` to `|x| f()`. Every one is
semantically neutral and syntactically real: the tree moves and nothing is
wrong. Two of those are configured off; the rest cannot be.

The alternative was to compare everything and keep a list of the formatter's
known rewrites, and it is worse — the list is open-ended, and each entry is
a blanket that also silences a genuine finding wearing the same node pair.
Comparing the two texts with all whitespace removed costs a little yield (a
file where black added a trailing comma is skipped) and buys a question with
an unambiguous answer: a divergence that survives was caused by layout, and
is therefore ours.

Measured: rust 171 whitespace-only reformats of 2,000 files, **0 diverged**;
python 157 of 1,200, **0 diverged**. Python matters most of the three,
because python's layout is load-bearing and the invariant is not obvious
there the way it is in a free-form language. TypeScript has no formatter
here — tsc exposes formatting only through the language service and prettier
is not vendored — and the command says so rather than substituting
something else.

### 5.11 The reparse path (`treebank incremental`)

Every other check here parses from scratch. tree-sitter's reason for
existing is that you can edit a file and reparse only what changed, and the
contract is that the result is indistinguishable from a fresh parse — so a
grammar can pass all 204,000 corpus files and still hand a broken tree to
the editor actually using it.

This is the one check with a **hard invariant** rather than an oracle:
parse, edit, reparse incrementally, parse the edited text fresh, compare
kinds and byte ranges. Both halves matter — a reparse with the right shape
at the wrong offsets is still wrong, and is the likelier failure.

Measured over 26,898 edits across the three grammars: **27 divergences, all
of them in rust, and none on text that still parses cleanly.** That split is
the finding. An incremental reparse is exact whenever the edit leaves the
file valid; the divergences are all on text the edit broke, where error
recovery and subtree reuse can stitch the same wreckage together more than
one way and a fresh parse need not choose as a reused one did. The gate is
the clean half; the broken half is reported and not failed.

The usual suspect for this class of bug is an external scanner whose
`serialize`/`deserialize` does not round-trip, which is why python — whose
scanner carries an indent stack — was the language expected to fail. It did
not; rust, whose scanner is stateless, is the one that diverges.

### 5.12 Error recovery (`treebank recovery`)

Editors spend most of their time on text the language does not accept:
source is broken between one keystroke and the next, and what a tool can do
with it depends on how much structure survives. A parser that turns one
missing brace into a file-length ERROR is useless there while scoring
perfectly on every other check in this document.

There is no oracle. CPython, `syn` and tsc all stop at the first error and
return a message; none produces a recovered tree, so there is nothing to
compare against. So this measures a **property**: take a file that parses
cleanly, delete exactly one token, and see how much of the file lands inside
an ERROR. One token rather than one byte, because deleting a byte usually
just shortens an identifier, while deleting a token is the smallest edit
that reliably breaks a parse and is what a half-typed line looks like.

The result is a distribution, not a number, because the shape is the point —
a parser can have an excellent median and still shred one file in fifty, and
the tail is what a user notices:

| | median | p90 | p99 | shredded |
|---|---|---|---|---|
| python | 0.3% | 17.1% | 99.1% | 240 |
| rust | 0.1% | 5.6% | 78.9% | 116 |
| typescript | 0.5% | 17.5% | 94.1% | 60 |
| c | 0.1% | 10.8% | 90% | 250 |

"Shredded" means more than half of a file of at least 1 KiB ended up inside
an error. **The size floor was added after the first run reported nearly
twice as many.** Deleting `import` from `from a b` errors for two lines and
recovers — correct behaviour — but on a four-line file two lines is more
than half of it. A percentage measures the file as much as the parser when
the file is small, and the count was measuring both.

Shredding is reported by the token that caused it, because a bare count is
not actionable: losing a quote and losing an identifier are the same number
and completely different problems. Python's largest cause is `"""` (59),
which is inherent — an unterminated docstring genuinely does swallow the
rest of the file. Rust's and TypeScript's are brackets, which are inherent
for the same reason. What is left after those is where improvement lives.

## 6. Code organization

```
crates/
  treebank/              # the vocabulary, as code and as data:
    vocabulary/supertypes.js  #   the closed term list grammars import
    src/                      #   roles.json schema, facet query expansion,
                              #   the `treebank roles` checker
  treebank-python/
    grammar.js  src/scanner.c  src/ (generated, committed)
    roles.json  ledger.toml
    test/corpus/  test/negative/  test/negative/<version>/
    bindings/                 # rust crate + wasm
  treebank-rust/              # same shape
  treebank-typescript/        # common/define-grammar.js + typescript/ + tsx/
  treebank-cli/               # fetch · rank · sweep · oracle · roles ·
                              # negative · ledger
tools/consumer-test/          # downstream crate + wasm smoke tests
test/rosetta/                 # parallel programs + expected-roles files
```

- **treebank** is the single source of truth for the vocabulary: the JS
  module every grammar imports, the Rust library the CLI and consumers use,
  and — compiled to wasm — the same facet expansion in the browser.
  Vocabulary versions are semver on this crate.
- **Generated `src/` is committed** in each grammar crate; I1 keeps it
  honest. There is no build directory, no patch series, no vendored
  anything.
- **wasm is a first-class artifact**: every grammar crate publishes
  `tree-sitter-<lang>.wasm` alongside the Rust crate, and `treebank`
  ships a JS/wasm package that loads them and provides facet-aware queries.
  `roles.json` travels inside both the crate and the npm package.
- **`ledger.toml`** is TOML rather than JSON because it is mostly prose. A
  paragraph explaining why a deviation exists is one escaped line in JSON
  and a readable block in TOML, and this is the file someone reads when
  deciding whether to trust the grammar. The machine-readable manifests
  next to it — `roles.json`, `node_map.json`, `field_map.json` — stay JSON:
  they are lists of node names, where TOML buys nothing.
- **`ledger.toml`** is each grammar's evidence file, machine-validated by
  `treebank ledger`: language, versions covered, one pinned oracle per
  version family, corpus description and its declared blind spots, sweep
  results, conformance-suite results, and the vocabulary's uncategorised
  list. Every number this document promises lives there, next to how it was
  measured.

## 7. Design decisions

1. **Two tiers, because the parse table forces it.** Structural roles are
   supertypes (occurrence-level, generate-time-enforced); the three
   cross-cutting facets ship as a checked manifest with query expansion in
   `treebank`. The alternative — restricting the vocabulary to only
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
   supertype-field rule in `treebank-rust/ledger.toml` came to light.

7. **Symbols are SCIP, with every symbol local.** The descriptor grammar is
   adopted rather than invented, and `local <id>` makes a locals-only index a
   valid index — so treebank emits identity without waiting on a package
   graph, and the layer above promotes rather than reformats (§9.3). The
   alternative, a treebank-specific symbol spelling, was rejected because it
   forfeits rust-analyzer's index as a free oracle.
8. **Fields join the closed vocabulary** (§9.4.2). Node names are already
   shared by decision 4; leaving fields unchecked means a traversal can read
   `name:` and silently return nothing on a grammar that spells it `pattern:`.
   Where syntax genuinely differs the answer is two fields with a documented
   relationship, never one field that lies.
9. **Resolution is lexical only, and says so in three columns.** Scope and
   binding tables ship per grammar and one resolver consumes them (§9.6).
   `a.b` is refused rather than guessed, and the ledger separates *resolved*,
   *refused by design* and *unknown* — only the third is a defect.
10. **A second coverage number, on the analysis denominator** (§9.5). The
    editor-query coverage table answers a different question with a wider
    tolerance; a highlighter that misses a node leaves it uncoloured, while a
    resolver that misses a binding construct is wrong about every reference to
    it.

## 8. Order of work

`treebank` (vocabulary + checker + expansion) → **Python** → **Rust** →
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

### 8.1 C and C++, and what the C++ parse table costs — measured

C landed as the sixth grammar and it is the first whose **preprocessor is
part of the syntax**: a conditional does not enclose a construct, it
encloses a run of whatever was there, so the five conditional rules are
generated once per context they may interrupt. That, two declarator
hierarchies rather than four, and the GNU dialect throughout are the whole
of its shape; `crates/treebank-c/ledger.toml` carries the numbers.

C++ is the seventh, and it **extends** C through tree-sitter's own grammar
inheritance rather than copying it. That is the right architecture and was
never in doubt: C++ genuinely is C's declarator grammar with more on top,
and a second copy of the declaration specifiers, the four declarator
shapes, the whole preprocessor and every GNU extension is a copy that
drifts. What was in doubt, for a long time, was whether the **parse table**
could be built at all — and that is the part worth recording, because it is
the part a second C-family grammar will hit again.

A first version generated, after 49 declared conflicts, to a **65 MB
`parser.c`** — six times TypeScript's and fifteen times C's — taking about
twenty minutes. Reproducible generation is CI's first gate, so a
twenty-minute generate is disqualifying on its own. Every round after that
plateaued in the same place: about twenty declared conflicts, after which
each additional conflict cost ten minutes and bought one more. A declared
conflict SPLITS the parse state and carries both readings; twenty of them
over the same three symbols multiply.

What brought it to 44 MB and about a minute, in the order it was measured,
because each of these is worth doing FIRST next time:

1. **Static precedence instead of a declared conflict**, wherever one
   reading is simply right. `Widget(` at the head of a member is a
   constructor, never a field of type `Widget` with a parenthesised
   declarator, and saying so with `prec` rather than a conflict removed
   three of the most expensive splits outright.
2. **Confining a rule to where it can occur.** A no-return-type declarator
   offered wherever a declaration goes gives every `f(x)` in the language a
   second reading; offered only from `_member`, it gives nothing a second
   reading, because a member function declaration needs a return type
   before its name. The out-of-line definition is the same story with a
   sharper edge: C++ has no nested function definitions, and while that
   rule was reachable from a statement its `prec(2)` won inside every
   block, so `std::__terminate();` read as a declaration of
   `std::__terminate` and errored on its own semicolon.
3. **Not duplicating the base grammar's own alternatives.** The `struct`
   extension made the base clause optional, which made it a second,
   identical reading of every plain `struct X { … }` in the corpus.
4. **One rule where there were two identical ones.** `template_type` and
   `template_function` had the same body and differed only in which
   alternation reached them, so every instantiation carried both.
5. **Leaving out what C conceded to unpreprocessed source.** C admits a
   bare macro at file scope, a K&R identifier list and a type where an
   argument goes. Each earns its keep in C and costs multiples of that in
   C++, where the same shapes are already a constructor call, a functional
   cast and a template argument.

What did NOT help is worth as much: cutting features. Fold expressions,
user-defined literals, concepts in the function-suffix position, the
parenthesised initialiser, condition declarations and pack expansion as an
ordinary expression were all removed, and the plateau did not move until
the five structural changes above were made. The cost was never C++'s
feature count; it is the declaration-versus-expression ambiguity that C
already has, multiplied by `::`, `&` and `<`.

Six constructs stayed cut, and `crates/treebank-cpp/ledger.toml` names each
with what it was costing. The C++ corpus is libstdc++, which is a floor
rather than a typical case: the standard library's own implementation is
the most macro- and template-dense C++ there is.

### 8.2 The unexpanded macro, and why a refusal is not a finding

C shipped at 2,184 of 3,662 files with 422 grammar gaps, and nearly every
one of those gaps was the same thing: a macro standing where the
preprocessor would have put a keyword. The ledger named three positions for
it and refused all three, with a reason — a bare identifier admitted after
a declarator makes `int x y;` parse, which is the accepts-invalid direction
the negative corpus exists to catch.

The reason was correct about the rule being refused. It was not a finding
about the position. Admitting the macro **after a declarator that ends in
`)` or `]`**, and **before the type** where a keyword type cannot be, and
inside a `{` that `struct` or `enum` has already opened, and after
`typedef` — four contexts a juxtaposed pair of names cannot reach — took
the same corpus to **2,648 files and 119 gaps**, with `int x y;` still
rejected in every position the negative corpus asks about.

Three things generalise from that, and none of them is about C:

1. **A refusal is only as good as its scope.** "This rule admits `int x y;`"
   is a fact about a rule, not about a construct. The useful question is
   always which committed context the construct can be confined to, and the
   grammar is what says whether such a context exists. For the macro that
   stands in for a whole parameter list — `void f BASE64_ENC_PARAMS { … }` —
   there is no such context, the tokens ARE `int x y;`, and it stays refused.
2. **The corpus reclassifies as the grammar improves.** 42 of the remaining
   119 gaps are reported at an `extern "C" {` line that no grammar can fix,
   because it is the file's FIRST error and its real gap is further down.
   A cluster report is a queue, not a diagnosis, and the top of the queue
   is the least reliable entry in it.
3. **The pass rate is not the only number that moves.** Error recovery got
   worse — p90 5.7% to 10.8%, 182 shreds to 250 — and part of that is real
   rather than population: a rule that admits a bare identifier where a
   keyword was expected gives the parser one more way to keep going wrongly
   after a lost brace. §5.12's table and the C ledger both record it. A
   grammar that only ever measures what it accepts will trade this away
   without noticing.

## 9. What a file can say about itself

Sections 1–8 describe grammars and how they are validated. This section
describes what treebank is being widened to do with them: to answer, from a
grammar and one file, the questions a consumer would otherwise reach for a
toolchain to answer — which definitions a file contains, what names they bind,
which reference resolves to which binding, and what identity all of that has
across commits.

The boundary is worth stating once and not relitigating. Work that needs only
a grammar is treebank's; work that needs a real toolchain — a package graph, a
type checker, anything that reads a manifest — belongs to propbank, and
per-language facts that neither a grammar nor a toolchain yields belong to
langbank. Everything below stays on the near side of that line.

### 9.1 The invariant: toolchains on the bench, never in the box

§4 already validates every grammar against the language's own reference
parser, and §5 turns that into numbers. The same arrangement governs
everything in this section: **a toolchain may be used to check what treebank
produces and may never be required to produce it.** What ships is still one
wasm file per language with no dependencies, and a consumer with neither
`python` nor `rustc` installed gets the same answers as one with both.

That is the only placement rule needed. If answering a question at run time
requires a second file, a manifest, or a compiler, the question is not
treebank's — however syntactic it looks.

### 9.2 Identity, and the failure that motivates it

A traversal over `_scope` and `_callable` yields a scope chain for every
definition, and the same traversal works across grammars. Measured with the
published 0.3.0 packs, one 40-line walk over Python and TypeScript:

```
python      ["helper", "main"]
typescript  ["helper", "main"]
```

The same walk over the Python file after a refactor that moves `helper` into
a class:

```
before  ["helper", "main"]
after   ["Util", "Util::helper", "main"]
```

That is the problem in one line. `helper` and `Util::helper` are the same
function; the chain says they are two. Anything keyed on the chain — a stored
baseline, a diff between two commits, a join between a fact derived here and
an observation made elsewhere — reads a move as a deletion plus an addition.
An identity that carries the file path fails the same way on a rename.

A chain is the right raw material and not the whole answer. What is missing is
a canonical spelling with a documented rule for what survives a move, and that
is a vocabulary question rather than a traversal question.

### 9.3 The output format is SCIP, and it is not invented here

SCIP's symbol grammar is:

```
<symbol>     ::= <scheme> ' ' <package> ' ' (<descriptor>)+ | 'local ' <local-id>
<descriptor> ::= <namespace> | <type> | <term> | <method> | <parameter> | …
<namespace>  ::= <name> '/'      <type>   ::= <name> '#'
<term>       ::= <name> '.'      <method> ::= <name> '(' (<disambiguator>)? ').'
```

The second alternative is what makes it usable here. `local <id>` is a
first-class symbol, so **an index in which every symbol is local is a valid
index**. Package-qualified symbols need a `<package>`, which needs a manifest,
which is propbank's — and treebank does not have to wait for it or invent a
placeholder for it.

The layering rule to preserve: each layer emits a valid index, and later
layers **promote** symbols rather than reformat them. Treebank's index carries
definitions, intra-file references, and every symbol local. Propbank's is the
same index with locals promoted where a package can be named. Two consequences
worth having: a consumer can stop at treebank and still hold something usable,
and the difference between the two indexes is a number — how many locals were
promoted — measurable the same way as everything else in this document.

Adopting the grammar rather than inventing one also buys an oracle. rust-
analyzer emits SCIP and is maintained by the Rust project; its index over the
same source is a check on treebank's Rust identities that costs nothing to
run. That is the §4 arrangement again, applied to symbols instead of parses.

### 9.4 Vocabulary additions

Four terms are wanted. The list in §3.2 is closed, so each is a vocabulary
change applying to every language at once, and each is stated here in the form
§3.2 requires: what the term means syntactically, and what it refuses to mean.

#### 9.4.1 Binding kind

`_binding` says a node introduces a name. It does not say what kind of thing
the name denotes, and the SCIP descriptor suffix depends on exactly that: `/`
for a namespace, `#` for a type, `.` for a term, `().` for a method.

§1 rules out `_class`, `_function` and `_variable` as semantic
classifications, and rightly — they do not survive contact with eleven
languages. The distinction wanted here is narrower and stays syntactic: not
*is this a Method* but *which descriptor suffix does the name this construct
binds take*. That is decided once per node type, by the construct rather than
by the occurrence, so it belongs as a field on each `_binding` member in
`roles.json` rather than as four new facets. Python's `_binding` has 23
members today; this adds one field to each.

Where a construct genuinely binds more than one kind, the honest answer is to
record both and let the consumer refuse rather than pick.

#### 9.4.2 The field vocabulary — measured

§7 decision 4 commits to shared concrete names and fields across grammars,
diverging only where syntax genuinely differs. Node names hold up under that.
Fields are close and unchecked, and the gap is silent.

Parsed with the published 0.3.0 packs, the same program in each language:

```
python      (function_definition name: … parameters: (parameters
              (parameter name: (identifier))) body: (block …))
typescript  (function_definition name: … parameters: (parameters
              (parameter pattern: (identifier) type: …)) body: (block …))

python      (assignment left: (identifier) right: (call_expression …))
typescript  (variable_declaration (variable_declarator
              name: (identifier) value: (call_expression …)))
```

Every node name matches. `parameter` carries `name:` in Python and `pattern:`
in TypeScript; Python's `assignment` uses `left:`/`right:` where TypeScript's
`variable_declarator` uses `name:`/`value:`. A traversal that reads `name:`
works on Python and returns nothing on TypeScript — no error, no warning,
which is the failure mode §2 fact 1 exists to prevent for supertypes and which
fields have no equivalent protection against.

`field_map.json` does not cover this. It maps one grammar's fields onto its
oracle's labelled edges — a conformance artifact, per grammar — and its own
`unlabelled_note` already names half the problem: "an unnamed edge is one a
consumer cannot query by role." A *differently* named edge is the other half,
and it is worse, because an absent field announces itself and a wrong one does
not.

So fields need what the terms got: a closed list in `vocabulary.json`, a
per-grammar declaration, and a rule in `treebank roles` that every declared
field exists in `node-types.json` and every vocabulary field a grammar can
express is spelled the same way.

One case will not unify by renaming. A TypeScript parameter can be a
destructuring pattern where a Python parameter is a name, so `pattern:` and
`name:` are two claims and not one — the same judgment §3.1.1 made for
`_parameter` across tiers, and the same resolution: two fields with a
documented relationship beats one field that lies.

#### 9.4.3 `_discarded`

An expression whose value goes nowhere. Measured on the 0.3.0 packs with one
pattern carrying no language-specific node name, against CPython's own
bytecode as the oracle:

```
(expression_statement (call_expression) @discarded)

treebank, python       helper(3)  ·  print(total)  ·  xs.append(1)
CPython CALL + POP_TOP the same three, same lines
treebank, typescript   helper(3)  ·  console.log(total)  ·  xs.push(1)
```

Exact where it applies. It applies by tree shape rather than by role, which is
what makes it the wrong mechanism: it works because Python and TypeScript both
wrap the call in `expression_statement`, and in an expression-oriented
language a discarded value need not be a statement at all — Rust's
block-trailing expression is the case that breaks it. A shape match that holds
in two grammars and fails in a third is precisely what the vocabulary exists
to replace.

`_discarded` threads where the parse can carry it and lists as a facet where
it cannot, per §3.1.1.

#### 9.4.4 Parameter binding mode

Whether a callee can write through a parameter: `&mut` in Rust, every object
reference in Python and TypeScript, `const` and by-value in C++. Syntax
answers part of this and types answer the rest, so the term has to say which
part it answers — the refusal `_literal` already makes by quantifying over the
rule rather than the instance. A grammar that cannot decide from syntax
declares the term absent rather than guessing.

### 9.5 Coverage, on the denominator that decides this

The published table measures the generated editor query file against every
named node: 61.6% for bash, 46.9% for Python, 37.0% for C. That is the right
number for the question it answers — how much of a file a highlight query
colours — and the wrong one for this section.

Analysis has a different denominator and a much narrower tolerance. The
question is what share of **callable, parameter, invocation, binding, access,
loop, jump and branch sites** the vocabulary covers, and the difference in
tolerance is the point: a highlighter that misses a node leaves it
uncoloured, while a resolver that misses one binding construct returns a wrong
answer for every reference to it. Degradation is graceful in one case and
silent in the other.

The ledger gains a second coverage table over those roles, on the same
corpora. It is cheap — the corpora and the sweep exist — and until it exists
there is no evidence that the vocabulary can carry §9.6.

### 9.6 Scope tables, and resolution

Resolution is three passes: `_scope` nodes build the scope tree, `_binding`
nodes place names into scopes, and every `_identifier` that is not a binding's
own name walks the chain outward. The traversal is shared and language-
agnostic. What diverges is where a binding lands and when it becomes visible,
and that is data:

```json
"parameter":        { "into": "own-body", "visible": "whole", "rebind": "new",   "ns": "value" },
"assignment":       { "into": "function", "visible": "whole", "rebind": "write", "ns": "value" },
"for_statement":    { "into": "function", "visible": "after", "rebind": "write", "ns": "value" },
"global_statement": { "into": "module",                       "rebind": "alias" }
```

- `into` — which enclosing scope receives the name. Python has no block scope,
  so an assignment targets the nearest *function*; Rust's `let` targets the
  block; parameters land in the callable's own body.
- `visible` — from the top of the scope (Rust items, JS `function`
  declarations) or from the binding position onward (Rust `let`,
  JS `let`/`const`).
- `rebind` — whether re-binding makes a new symbol (Rust shadowing) or writes
  to the existing one (Python assignment). This is SCIP's `WriteAccess` role.
- `ns` — Rust's `struct Foo` and `fn Foo` coexist, so resolution keys on name
  and namespace together.

Around 25 rows per grammar, next to `roles.json`, checked the same way. One
key will not always be a node type: JavaScript's `variable_declaration` needs
different rules depending on whether its keyword child is `var` or `let`, so
the table admits a node type plus a discriminating child.

The oracles are per language and already exist. CPython's `symtable` is its
own scope analysis, exposed: over a nested example it reports the
comprehension as its own scope, `c` free through two levels, `nonlocal`
resolved, and `total` global. TypeScript's checker and rust-analyzer's index
answer the same question for the other two. This is §4's arrangement pointed
at scopes instead of parses.

What resolution refuses is as important as what it does. `a.b` is not scope
resolution — `b` depends on the type of `a` — and treebank does not attempt
it. Import targets bind the local name and leave the target unresolved,
marked as an import. So the ledger records three columns per grammar:
**resolved**, **refused by design**, and **unknown**. Only the third is a
defect, and a tool that distinguishes them is doing something the established
indexers largely do not.

### 9.7 Analysis query packs

`treebank queries` already writes one source file of patterns in the shared
vocabulary out to every grammar's own node names, regenerates under
`--check`, and compiles each result against its grammar so an impossible
pattern fails at build time rather than matching nothing at run time. That
machine is not specific to highlighting.

An analysis query pack is the same source-and-expand pipeline with capture
names that identify facts rather than highlight groups — `@callable`,
`@call`, `@binding`, `@access` — shipped in the grammar crate and hashed with
it. Consumers stop reimplementing the same traversal once per language, and
the pack's digest becomes part of the provenance of anything derived through
it, which a hand-written traversal in a consumer's own source cannot offer.

### 9.8 What stays out

Types. Inference. Member resolution. Cross-file anything. Control-flow
*semantics* — treebank publishes `_branch`, `_loop` and `_jump` as syntax and
does not say what a jump means, because unwinding, early return and `?`
differ in ways no vocabulary reconciles.

The line is §9.1's and it does not move: a question needing a second file, a
manifest or a compiler at run time is not treebank's, and answering it here
would trade the one property that makes a grammar pack worth fetching.

## 10. API changes the expansion requires

Five, found by writing a consumer against 0.3.0 rather than by reading the
crate:

1. **A capture carries no node.** `Capture` is `{name, kind, range, pattern}`,
   so a consumer cannot walk from a match into the tree and has to run a
   second, independent traversal to reach anything a pattern did not capture.
   A capture that yields a `Node`, or a `query_nodes` beside `query`, removes
   that pass.
2. **No `child_by_field_name`.** Reaching a field means looping over
   `child_count` and `field_name_for_child`. Every consumer will write that
   loop, and §9.4.2 makes fields load-bearing enough that it should not be
   theirs to write.
3. **`Node` is not exported.** `lib.rs` re-exports `Pack` alone, so the type
   cannot be named in a helper's signature and recursive traversals have to be
   restructured around an explicit stack.
4. **Positions are byte ranges only.** SCIP occurrences need line and column,
   and so does every report a person reads.
5. **`kind()` allocates.** It returns `Result<String>` — acceptable per node
   on a small file, expensive once a traversal crosses a repository. An
   interned identifier or a borrowed `&str` pays for itself at the first
   corpus sweep.
