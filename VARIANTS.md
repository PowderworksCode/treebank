# Variants — splitting Python 2 out, and taking on SQL

**Python is done.** Steps 1–3 and 5 of §10 are built and gated: `treebank-python`
ships two parse tables from one grammar source, `treebank crossvariant` is a
gate in `verify` and in CI, and the numbers below marked *measured* are from
the split rather than from a projection. The SQL half (§7, steps 4–6) is still
a proposal.

[`DESIGN.md`](DESIGN.md) §4.2 is superseded for Python by §11 below.

Two questions, one answer. Python should stop being a 2 ∪ 3 union grammar, and
SQL cannot be a union grammar at all — Postgres and MySQL disagree about what a
backtick is, what `||` does, and what `"x"` means, so there is no table that
reads both honestly. Both are the same request: **one language, several parse
tables, one body of grammar source.**

Today the design has exactly one axis — a language is a union of its versions —
and one escape hatch, §4.2 case (2), which rejects an old construct when
admitting it would cost the present. Python 2 has been living in that hatch. The
proposal is to make the second axis first-class instead:

> A language has one or more **variants**. A variant owns a parse table, an
> oracle set, a corpus and a ledger. Versions are unioned **inside** a variant;
> dialects and incompatible version families are variants. The grammar source is
> shared, and sharing is enforced by construction rather than by intent.

`python2` and `python3` become variants of `python`, in exactly the machinery
`sql-postgres` and `sql-sqlite` will use. That is the whole point: one mechanism,
paid for twice.

---

## 1. The split rule

The union rule (§4.2) says when versions share a table. The split rule says when
they do not. It has to be at least as strict, because a split is not free — a
second table is a second thing to keep correct, and the cheap failure mode of
"just make a variant" is eight half-measured grammars where there was one
measured one.

**Split when a shared table cannot serve both readings honestly.** Three
qualifying causes, each of which must be *measured* before the split, never
argued:

1. **Lexical divergence.** The variants disagree below the parser — identifier
   quoting, string and escape forms, comment syntax, an operator that is
   concatenation here and disjunction there. A GLR fork can carry an ambiguous
   *phrase*; it carries an ambiguous *lexicon* badly, because the fork is taken
   at every token of that class in every file. This is the SQL cause.

2. **The union is measurably costing the majority variant.** §4.2 case (2)
   accumulating: constructs rejected on purpose, or conflicts and dynamic
   precedences carried solely to keep a minority reading alive. The threshold is
   evidence, and the evidence format already exists — the TypeScript measurement
   (letting `never`/`unknown`/`symbol` be identifiers fixed 3 files and broke 13)
   is the standard. This is the Python cause.

3. **No single table exists.** Genuine grammatical ambiguity, where the same
   text has two complete parses and no precedence chooses between them. `<T>x`
   as a cast versus an unclosed JSX element is the canonical case — and note
   that TypeScript still did *not* split on it, because corpus incidence of
   angle-bracket casts measured ≈ 0. Cause (3) is necessary, not sufficient: it
   also has to matter.

**Do not split for:** a construct removed in a point release (that is case (2)
of the union rule, and stays there); a variant with no pinnable offline oracle
(§6.3); "it would be cleaner"; or a dialect nobody has a corpus for. A variant
without an oracle and a corpus is a parse table with no way to be wrong, which
is worse than not having it.

Every split records the measurement that forced it in `variants.toml`, next to
the constructs it moved. A split with no numbers is not adoptable.

---

## 2. What a variant is, concretely

| shared per **language** | owned per **variant** |
|---|---|
| grammar source (`common/`) | `grammar.js` — a ~20-line call into `common/` |
| external scanner source | generated `src/` (parser.c, grammar.json, node-types.json) |
| the vocabulary threading | `roles.json` (as a delta, §4) |
| `node_map.json`, `field_map.json` | `ledger.toml` — corpus, oracle, sweep numbers |
| the rosetta programs | `test/corpus/`, `test/negative/` |
| `variants.toml` — the split record | its entry in `variants.toml` |

Crate shape, using Python as the worked example:

```
crates/treebank-python/
  common/
    define-grammar.js     # the whole grammar, parameterized by variant
    rules/                # shared rule modules
    scanner.c             # one scanner, variant flag from a #define
  python3/grammar.js      # module.exports = defineGrammar(PY3)
  python3/src/            # generated, committed
  python3/test/
  python2/grammar.js
  python2/src/
  python2/test/
  variants.toml           # what exists, why, and the measurement
  roles.json              # language-level; variants carry deltas
  bindings/rust/lib.rs    # LANGUAGE (= python3) and LANGUAGE_PYTHON2
```

One crate per language, not one per variant. The alternative was considered and
rejected: N crates duplicate the scanner, `build.rs`, the bindings, the roles
manifest and the ledger, and each duplicate is a place for the dialects to drift
apart silently — which is precisely the failure the rosetta corpus exists to
catch across languages and would then need to catch inside one.

---

## 3. Sharing the source without conditionals rotting it

The naïve parameterization — `if (variant.py2) { ... }` sprinkled through rule
bodies — works for two variants and is unreadable at eight. It also makes the
question "what does the Postgres grammar actually accept?" unanswerable without
mentally executing the module.

The rule: **a variant may add members to a declared extension point, and may
remove them. It may not rewrite the internals of a shared rule.**

`common/` declares extension points explicitly — named rules that are a `choice`
over a list the variant supplies:

```js
// common/rules/statements.js
_statement: $ => choice(
  $.if_statement, $.for_statement, /* … the shared skeleton … */
  ...v.statements,          // variant-only statement forms
),
```

and a variant is a data object, not a code path:

```js
// python2/grammar.js
module.exports = defineGrammar({
  name: 'python2',
  statements: ['print_statement', 'exec_statement'],
  rules: require('./rules'),          // definitions for the names above
  lexicon: { longIntegers: true, backtickRepr: true, unicodeStringPrefix: true },
  keywords: { remove: ['nonlocal', 'await', 'async', 'True', 'False', 'None'] },
});
```

Three properties fall out, and they are the reason for the shape:

- **The variant file is the answer to "what is different here."** It is a
  readable manifest, and it is diffable against its siblings.
- **`defineGrammar` fails at generate time on an unknown extension point or an
  undefined rule name**, the same way `assertTableTerms` already fails on an
  invented vocabulary term. A typo is a generate error, not a CI run.
- **The extension-point list is closed and small.** Adding one is a deliberate
  change to `common/`, reviewed as such. This is what stops the parameterization
  from becoming a second grammar language.

The scanner is shared source with a variant `#define` — the alternative,
runtime-branching on a variant field, puts a branch in the hottest loop in the
parser for no benefit, since the parse tables are separate binaries anyway.

Grammar inheritance (`grammar(base, $ => ({…}))`) is available and is **not**
used for variants. It overrides rules by name, which is exactly the "rewrite a
shared rule's internals" the rule above forbids, and it makes the effective
grammar the result of a merge nobody can read. Reserved for the case it fits: a
language that genuinely extends another language's published grammar.

---

## 4. Manifests: shared, with deltas

`roles.json` is where dialect drift would hide. A role threaded in Postgres and
forgotten in MySQL is invisible — supertype matching is derivation-based, so the
query simply returns fewer nodes and nothing fails.

So: one language-level `roles.json`, and a per-variant `roles.delta.json` that
may only **add** members that are variant-only node types. `treebank roles`
gains two checks:

- every member of a variant delta is a node type that exists **only** in that
  variant (a shared node type belongs in the shared file);
- every shared role is threaded in every variant, or the variant's delta
  declares it absent with a reason.

`ledger.toml` goes per variant, because everything in it is per-parse-table:
corpus size, oracle pin, sweep verdicts, known gaps, shape numbers. A
language-level `ledger.toml` keeps the prose that is genuinely shared. At two
Python variants this is mild bookkeeping; at five SQL dialects a single ledger
would be unreadable, which is the case that decides it.

`version_policy.toml` merges into `variants.toml`, so every split-or-union
decision for a language is in one file:

```toml
[[variants]]
name = 'python3'
versions = '3.0 – 3.13'
default = true

[[variants]]
name = 'python2'
versions = '2.7'
split_evidence = '''…the measurement that forced the split…'''

# §4.2 case (2): rejected on purpose, WITHIN a variant, across its versions.
[[variants.rejections]]
construct = 'f((a)=1) — a parenthesized keyword argument'
…
```

---

## 5. The new gate: cross-variant negatives

Every existing gate asks a question about one grammar. Variants introduce a
failure mode none of them can see: **the dialects quietly collapsing back into
one permissive union.** Each table drifts a little more accepting, every
individual sweep stays green — a corpus of real Postgres never contains MySQL
backticks, so nothing ever rejects — and at the end you have five copies of one
lenient grammar and a `--dialect` flag that does nothing.

`treebank crossvariant`: for each pair of variants, a corpus of files valid in A
and **required to be rejected** by B. *Built* — a CLI command, a step in
`treebank verify`, and its own CI job.

```
crates/treebank-sql/test/crossvariant/
  postgres-not-mysql/     # valid PG, must fail the MySQL table
  mysql-not-postgres/     # `SELECT `x` FROM t`, `# comment`, …
  sqlite-not-postgres/
```

Each file carries the construct's name and the reason, and **both directions are
asserted** — a file variant A also rejects is a broken fixture, not a passing
test, which is what stops the corpus itself from rotting into "files that are
invalid everywhere."

A gate that cannot fail is not a gate, so it was falsified three ways before
being trusted: a fixture nothing accepts is caught, python2 re-admitting
f-strings is caught, and python3 re-admitting the py2 print statement is
caught. The second of those failed to be caught on the first attempt — the
CLI's compiled-parser cache fingerprinted only a variant's own `src/scanner.c`
and not the shared scanner it `#include`s, so a deliberately broken grammar
passed on a stale dylib. Fixed in `grammar.rs`, and it is the third
stale-artifact bug this work turned up (§10).

This runs in `treebank verify` and it is the single most important new check
here — for Python it is small
(the py2 forms must fail python3, and vice versa for `async`/`await`), and for
SQL it is the whole justification for having dialects at all.

---

## 6. Python: retiring the union

### 6.1 What the union currently costs

Measured in the tree as it stands:

- **Three constructs rejected on purpose** (`version_policy.toml`): `f((a)=1)`,
  bare `nonlocal`, bare `await`. Two of the three are py2 constructs rejected
  *because they fork against modern py3 code* — the file's own reasoning for
  bare `await` is that it "forks at every `await` in modern async code — most of
  the corpus."
- **Five declared GLR conflicts** carried for py2 alone
  (`grammar.js:117–124`: `print_statement`/`exec_statement` against
  `_soft_keyword`, `comparison_expression`, `conditional_expression`), plus
  **negative dynamic precedences** on `print_statement` and `exec_statement` so
  the py3 expression reading wins. Every `print` and `exec` token in every py3
  file in the corpus takes that fork and loses it.
- **A declared corpus blind spot** (`ledger.toml`): "python2-only code (modern
  PyPI sdists are effectively py3-only, so the py2 half of the union leans
  entirely on the corpus tests and batteries)". The py2 half of a 297,612-file
  corpus is measured by hand-written fixtures.
- **A degraded oracle**: py2 verdicts come from "a frozen battery of
  known-valid/known-invalid files" when CI has no python2 binary.

That is split-rule cause (2), documented in the repo's own files, with the
evidence already gathered. The py2 half is the least-measured part of the
best-measured grammar here.

### 6.2 The split — built

Done, in three commits, each with its own proof: the `common/` extraction is
byte-identical, the strip is differential-tested, and the second variant is
falsification-tested. What follows is what was planned; the numbers are what
happened.

**`python3`** — today's grammar minus the py2-only forms. Removed:
`print_statement`, `exec_statement`, `repr_expression` (backticks), `<>`,
`tuple_parameter`, the py2 `except E, e:` and `raise E, v, tb` arms, old-style
octal, long-integer suffixes, the `ur''` prefix. With them go the five conflicts
and the three dynamic precedences. Expected: a smaller table, a faster sweep,
and — the part worth measuring first — some number of ambiguity-driven
mis-shapes that only existed because a losing fork was on the table.

*Measured*, against the union grammar on `main`: parser.c −8.0% (2,894,567 →
2,663,878 bytes), 2,487 → 2,380 parse states, 215 → 177 large states, 305 → 296
symbols, 45 → 39 declared conflicts — the six carried for `print` and `exec` —
and three negative dynamic precedences gone. Zero mis-shapes surfaced: a
differential parse of 8,761 CPython-3.11-valid files gives byte-identical trees
before and after. Nine constructs it now rejects, all nine of which CPython
3.11 also rejects.

**`python2`** — the shared core plus a py2 rule module. Everything above comes
back as ordinary unambiguous grammar, and the three purpose-rejected constructs
*become accepted*, because there is no py3 reading to protect: bare `nonlocal`
and bare `await` are just identifiers, and `print x` needs no dynamic precedence
when `print` is a keyword. `variants.toml` keeps `f((a)=1)` as a rejection —
that one is py3.0–3.7, an intra-variant version question, and the union rule
still governs it.

*Measured:* 1,714 parse states against python3's 2,380, and 16 declared
conflicts against 39 — fifteen of each are the shared ones in
`common/define-grammar.js`, so what the variant itself adds is 1 against 24.
The two tables disagree about 55.9% of the 8,761-file py3 corpus, and about all
29 crossvariant fixtures.

*Not done:* three python 3 constructs python2 still accepts — `@` matmul, the
PEP 448 unpacking generalizations, and the py3 parameter-list separators. They
are one boundary rather than a backlog: every removal that succeeded was a
**member** of a declared extension point or a token family, and these are the
internal **structure** of the shared parameter chain and collection rules. The
one remaining GAP (a tuple parameter with a default) is the same boundary from
the other direction, which is what makes it a boundary and not a to-do list.
Removing them means rewriting a shared rule's internals behind a variant flag —
the one thing §3 forbids. The mechanism needs a declared extension point for
parameter and collection *shape* before they can go, and that is a design
change, not a line change. Recorded in `python2/ledger.toml` with the probe
that found each.

`treebank_python::LANGUAGE` continues to mean Python 3. Consumers who never
thought about this get the parser they already wanted; `LANGUAGE_PYTHON2` is
opt-in.

### 6.3 Routing, honestly

`.py` does not say which. The rule is **default py3, route to py2 only on
positive evidence**, and the evidence is ranked:

1. package metadata in the sdist (`python_requires`, the
   `Programming Language :: Python :: 2` classifier) — package-level, which is
   the right granularity and mirrors how `dialect` is already assigned in the
   corpus manifest;
2. a `python2`/`python2.7` shebang — file-level, and it overrides the package,
   the same way `treebank-corpus`'s bash ecosystem lets a `#!/usr/bin/env zsh`
   veto a `.sh` extension;
3. nothing else. No content sniffing for `print x`. A heuristic that reads the
   syntax to decide which syntax to expect is an oracle wearing a disguise, and
   it would make the sweep's gap numbers a measurement of the heuristic.

The sweep already stores a per-file verdict vector across oracles. Extend it to
store a verdict vector across *tables* on the ambiguous set — which variants
accept this file — and two things follow: a misroute becomes visible (accepted
by the py2 table, rejected by py3, oracle says py2-valid → routing, not gap),
and the vector is the data a future `version_of()` would be built from. §4.2
declines to answer "which version is this file"; this makes the declining
cheaper to reverse.

### 6.4 The py2 corpus

The split is not adoptable while py2 is measured by fixtures. PyPI's JSON API
lists every release of every package; for each of the top-ranked packages, the
last release carrying the py2 classifier is a real py2 sdist. That is a
corpus of the same shape and provenance as the py3 one, acquired by the same
`Ecosystem` machinery, and it retires the declared blind spot rather than
inheriting it. Its own blind spot — py2 code that was never published to PyPI,
which is most of the py2 that still runs — gets declared in the ledger.

Oracle: CPython 2.7's `compile`, pinned like the others, in a container so CI is
not asking for a python2 binary on the runner. The frozen battery stops being
the oracle and becomes what it should have been, a smoke test that the oracle
works before its verdicts are trusted.

---

## 7. SQL

SQL is the reason to build the mechanism properly. It is also where the
temptation to over-build is strongest, so the constraints come first.

### 7.1 Dialects are gated by oracles, not by ambition

A variant with no pinnable offline oracle does not exist. This one rule decides
the dialect list, and it decides it against what most people would guess:

| dialect | oracle | verdict |
|---|---|---|
| **postgres** | `libpg_query` — the real PG parser as a library, in-process, per-major-version builds | best oracle available for any SQL; also emits a parse tree, so `treebank shape` works |
| **duckdb** | in-process; `json_serialize_sql()` returns a parse tree | shape oracle for free; cheap to run |
| **sqlite** | `sqlite3_prepare_v2` against an in-memory database | yes/no only, and see §7.4 |
| **mysql** | server in a container; needs measurement before it is committed to | provisional |
| **t-sql** | `ScriptDom` (.NET), which produces an AST | provisional — a .NET dependency in CI is a real cost |
| **bigquery, snowflake, redshift** | dry-run APIs: network, credentials, unpinnable | **no variant** |

The last row is the rule doing its job. A BigQuery grammar with no oracle could
never be swept, so its ledger would have no numbers, so nobody could tell
whether it worked. Better to not ship it than to ship it unfalsifiable.

The ordering follows from the table: **postgres → sqlite → duckdb → mysql →
t-sql**, best-measured first, exactly as Python went first among the languages
because its oracle was cheapest to extend.

### 7.2 Layering

```
crates/treebank-sql/
  common/
    core.js        # the shared skeleton: SELECT/FROM/JOIN/WHERE/GROUP/HAVING,
                   # set ops, CTEs, window functions, DDL, the extension points
    lexicon.js     # parameterized token layer — see below
    scanner.c      # dollar-quoting, dialect string/identifier forms
  ansi/            # generated from core.js alone
  postgres/  sqlite/  duckdb/  …
```

**`ansi/` is generated and gated like any other variant.** The core could have
been a library only, imported and never generated; then it would have no test
suite of its own, and every core change would be validated only through the
dialects that happen to exercise it. Generating it makes the shared skeleton a
thing that can fail on its own. Its oracle is the SQL:2016 conformance material
plus the intersection of the dialect oracles — weak, and declared weak.

**`lexicon.js` is where dialects actually differ**, and separating it from the
rule structure is the single most load-bearing decision in the SQL layout.
Quoted-identifier form (`"x"` / `` `x` `` / `[x]`), string escape rules,
`||` as concatenation versus disjunction, `#` line comments, dollar-quoting,
parameter markers (`$1` / `?` / `@p` / `:name`) — these vary independently of
whether the dialect has `LATERAL`. Keeping them in a separate parameterized
layer means the ~80% of SQL that is genuinely shared stays written once.

**Versions union inside a dialect.** Postgres 12–17 is one table, governed by
the existing §4.2 rules with no changes. This is the axis the union rule was
written for and it still works.

### 7.3 Two things to decide early, before they decide themselves

**Procedural extensions.** PL/pgSQL, T-SQL batches, and MySQL routine bodies are
different languages that arrive inside a string literal or a dollar-quoted
block. Recommendation: v1 captures the body as one opaque token, ledgered as a
known gap, and the injection is a later variant (`postgres-plpgsql`) if a corpus
justifies it. Parsing PL/pgSQL inside the SQL table would roughly double the
grammar and would import a control-flow vocabulary (`_loop`, `_control_flow`)
that the declarative core has no other use for.

**Templated SQL.** Most real-world analytics SQL in public repositories is dbt
Jinja — `{{ ref('x') }}`, `{% if %}` — and is not SQL. The bash ecosystem hit
this precisely ("a template that renders to a shell script is not a shell
script, and no per-file oracle can tell the difference"). Same resolution:
excluded at `admit`, with the excluded count reported rather than quietly
dropped.

### 7.4 The oracle problem SQL has and the others do not

A SQL parser oracle usually wants a schema. `sqlite3_prepare_v2` rejects
`SELECT a FROM t` with "no such table: t" — a *post-parse* failure, exactly the
class the Python ledger already tracks under `oracle_blind_spot` and
`hidden gap`. The discipline exists; it just has to be applied from day one
rather than discovered:

- classify each oracle's errors into syntactic and semantic **before** any
  verdict is trusted, and pin the classification with the oracle;
- where an engine offers a parse-only mode, use it (`SET PARSEONLY ON`,
  `libpg_query`'s `pg_query_parse`, DuckDB's `json_serialize_sql`);
- where it does not, create the referenced objects from the corpus itself, and
  measure the residue the way Python measures its noise files with `ast.parse`
  — report the number every sweep so it cannot grow unnoticed.

### 7.5 Corpus

There is no package registry for SQL. Two sources, with different jobs:

- **Engine regression suites** (`postgres/src/test/regress/sql`, sqlite's
  `test/`, `duckdb/test/sql`, `mysql-test`) — adversarial, exhaustive, and
  **dialect-labeled by construction**, which solves the routing problem that
  makes SQL hard. The dialect comes from the repository, and lands in the
  manifest `dialect` field that already exists.
- **Real-world SQL** from GitHub repositories — representative, and unlabeled.
  Dialect is inferred from the repository's own dependencies (a `psycopg`
  dependency, a `postgresql://` URL in CI config), recorded as *inferred*, and a
  file whose inferred dialect rejects it while another dialect's oracle accepts
  it is booked as a misroute, not a gap.

Declared blind spot, up front: engine regression suites are adversarial and
unrepresentative of the SQL people write, and the real-world half is
template-heavy and therefore thinned by `admit`. The corpus will over-represent
edge syntax. Saying so in the ledger is the difference between a known bias and
a wrong number.

### 7.6 Vocabulary

SQL will exercise the vocabulary harder than any language here, because it is
declarative. `_declaration` covers `CREATE TABLE`/`VIEW`/`FUNCTION`; `_binding`
covers CTE names, aliases and column definitions; `_scope` covers subqueries and
CTEs; `_clause` — already moved to the facet tier on evidence from three
imperative languages — gets its strongest confirmation here, since SQL is very
nearly nothing but clauses. `_loop` and `_control_flow` appear only in the
procedural extensions that §7.3 defers, which is a further argument for
deferring them.

Expect the vocabulary to need a term SQL forces and the imperative languages
never did. That is a vocabulary version bump on `treebank-core`, handled the way
`_clause`'s tier move was: gathered as evidence in the ledgers first, changed
once, across all grammars.

---

## 8. Plumbing

Most of the hooks already exist, which is the argument for doing this now rather
than after five more languages.

| crate | change | size |
|---|---|---|
| `treebank-lang` | `LangName` stays the language; add `Variant { lang, name }` with the canonical spellings, `--variant` on the CLI | small |
| `treebank-cli/routing.rs` | `grammar_dirs()` and `route()` **already take a dialect and already return an index** — this fills in the stubs rather than adding a mechanism | small |
| `treebank-corpus` | manifest `dialect` field already exists and is already threaded; `Ecosystem::classify` already returns it. Adds: py2 sdist selection, the SQL repo-list ecosystems | medium |
| `treebank-oracle` | one module per variant family; the existing `stdin_oracle` shape fits `libpg_query` and DuckDB directly | medium |
| `treebank-core` | roles delta validation (§4) | small |
| `treebank-cli` | `crossvariant` gate; `verify` iterates variants; every `route()` caller is already written to take the index | medium |
| `.github/workflows/ci.yml` | the `grammars:` matrix keys on `crates/treebank-<lang>` with `grammar.js` at the crate root; becomes a `(lang, variant)` matrix with `grammar.js` one level down | small |

The one genuinely new thing is `crossvariant`. Everything else is filling in a
seam the design already cut.

**What actually shipped, where it differs from the plan above.** Two rows were
wrong, and both in the same direction — the seam that was already cut turned out
to be cut somewhere else:

- **`treebank-lang`** did not grow a `Variant { lang, name }` type or a
  `--variant` flag. `python2` is an ordinary `LangName` whose `grammar` column
  points at `Python`, exactly as `javascript` points at `Typescript`. A variant
  that has its own corpus and its own oracle *is* a language to every caller
  that fetches, sweeps or adjudicates; what it shares is a grammar crate, and
  the registry already had a column for that.
- **`routing.rs`'s `grammar_dirs()`/`route()` were deleted upstream** before
  this landed, as generality for a case that had been measured and rejected.
  They are not resurrected: with `python2` in the registry, a caller takes the
  grammar directory and the default variant directly. `verify::variant_dirs`
  reads `tree-sitter.json`, which is where tree-sitter itself looks, so the gate
  and the generator cannot disagree about what exists.

The CI row was right, and understated: discovery walks both depths and emits
`{grammar, variant, dir}`, so a new variant needs no CI edit at all.

---

## 9. What this costs

**Repository size.** Committed generated `src/` is load-bearing (I1), so each
variant adds a parser.c. Current: python 2.8 MB, rust 6.1 MB, typescript
11.2 MB, bash 3.1 MB, java 1.7 MB. Splitting Python adds roughly one python-sized
table — and the py3 table should *shrink*, since the five py2 conflicts come out
of it. *Measured:* python3 2.66 MB and python2 1.73 MB against a 2.89 MB union,
so the pair costs 1.50 MB more than the union did and the py3 half did shrink,
by 8.0%. Five SQL dialects is the real bill, and it is why §7.1's oracle gate
matters as a size discipline too, not only as a correctness one.

**CI time.** Every gate multiplies by variant count. Mitigation: the matrix is
already per-grammar and parallel; the expensive gates (sweep, shape, mutate) are
already out of CI and live in the ledgers.

**Cross-variant consistency work.** Real, and the reason §5 exists. The rosetta
corpus gains a within-language dimension — the same query over the same program
in every SQL dialect must return the same role counts. That is a stronger check
than the cross-language rosetta and costs almost nothing given the machinery.

**The failure mode to watch.** Not "too few dialects." It is five tables that
have each drifted toward accepting everything, passing every individual gate,
with `--dialect` a label rather than a difference. §5 is the only thing that
catches it, so it ships with the first split, not after.

---

## 10. Order of work

1. ✅ `variants.toml`, the roles delta check, and `treebank crossvariant` — the
   mechanism, against the existing single-variant grammars, where it must be a
   no-op. It is: rust, typescript, java and bash verify unchanged, and
   `crossvariant` is silent for them.
2. ✅ `common/` extraction for Python, still generating one `python3` table.
   Reproducible generation (I1) proved the refactor changed nothing:
   `grammar.json`, `node-types.json` and `parser.c` regenerated byte-identical.
3. ✅ The `python2` variant, **with** an oracle and a corpus — though not the
   ones this section originally proposed, and the difference is the finding.

   The oracle is not a CPython 2.7 container: `typed_ast.ast27` is 2.7's own
   parser vendored as a C extension for python3, so it is a pinned wheel
   rather than an EOL interpreter pulled per CI job. 29/29 on the
   crossvariant fixtures. A real 2.7 stays useful as the adjudicator,
   mirroring `syn` + `rustc -Zparse-only`.

   The corpus is not the PyPI rewind §6.4 proposed, because that was
   measured and it does not work: of the top 40 packages, 28 have a pre-EOL
   release declaring py2 support and **zero are py2-only** — all six-style
   code in the intersection of the two languages, 96.9% of it parsing under
   both, only 1.99% using syntax py3 rejects. Ranking by downloads and
   rewinding gives a corpus ~98% blind to the variant it is measuring.
   CPython 2.7.18's own tree is **44.4% py2-only over 2,182 files**, 22× the
   density, and its `Lib/test/` is adversarial by construction — the same
   reasoning §7.5 already applies to SQL, arriving a language early.

   First contact found 22 gaps in 13 clusters and **21 are fixed** (2,177 of
   2,182 pass). The survivor is `def spam(a, (e, (f,))=(4, (5,))):` — a
   tuple parameter with a default — which is the §3 boundary showing up in a
   real corpus rather than in an argument.

   **Three things went wrong here, and every one of them reported success**
   while being wrong. They are listed because the pattern is the lesson, not
   the bugs: each was a *stale or missing artifact* that made a check pass
   vacuously.

   - tree-sitter caches its compiled parser at
     `~/.cache/tree-sitter/lib/<grammar-name>.so`. Both variants were briefly
     named `python`, so the first differential loaded **one** library twice and
     reported the two grammars identical — the answer I was hoping for.
   - The shared scanner spelled its entry points
     `tree_sitter_python_external_scanner_*`, which do not link against a
     parser named `python2`. Every python2 parse failed to **link**, and the
     probe harness read "no output" as "accepts" — so the first python2 result
     claimed it accepted all of Python 3. The harness now refuses to report a
     verdict for a parse that did not run.
   - The CLI's dylib cache had the same blind spot as the first bug, one layer
     down (above, §5).

   Moving rules between modules also silently duplicated three `choice`
   members; nothing but a diff of the regenerated `grammar.json` against the
   previous commit could have seen it, because the grammar still worked.
4. `treebank-sql`: `common/` + `ansi/` + `postgres/`, with `libpg_query` and the
   Postgres regression corpus. One dialect, fully gated, before a second.
   *Not started.* The Python half of this document is now evidence that the
   mechanism holds; §7 is still a plan.
5. `sqlite`, then `duckdb`. Two more dialects is where the `crossvariant` corpus
   starts earning its keep and where the lexicon layer gets its real test.
6. `mysql` and `t-sql` only after their oracles are measured, not assumed.

Python first again, and for the same reason it went first originally: the
mechanism gets proven on the language with the best corpus, the best oracle and
the most gates already running, before it is asked to carry five dialects of a
language none of that exists for yet.

---

## 11. What this replaces in DESIGN.md

- §4.2's opening — "A language gets one grammar accepting the union of its
  versions — no python2 crate, no per-edition Rust grammars" — becomes: one
  grammar *source* per language, one parse table per variant; versions union
  inside a variant. Rust is unaffected and stays a single variant; editions are
  a version axis and the union rule governs them unchanged.
- §4.2's three-case conflict policy survives intact as the **intra-variant**
  rule. §1 above is the inter-variant rule sitting above it.
- §4.3's adjudication gains one bucket: **misroute** — a file the variant's own
  table rejects, another variant's table accepts, and that other variant's
  oracle calls valid. Today such a file would be booked as a gap.
- §6's layout gains the `common/` + per-variant directory shape, which §6
  already anticipated for TypeScript (`common/define-grammar.js`).
- §7 decision 3 (TypeScript as two dialect parsers) is already superseded in the
  tree by the measured decision to ship one; under this proposal that is not an
  exception, it is the split rule returning "no" on cause (3).
