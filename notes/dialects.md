# Dialects and versions — rows, and what buys a second parse table

Status: proposal. This note decides how treebank carries **language
variation**: versions of one language (Python 2.7 against 3.x, MySQL 5.7
against 8.4) and dialect siblings (MySQL against PostgreSQL, JSON against
JSONC). It revises `notes/DESIGN.md` §4.2–4.3, which answer the version
half of the question and are silent on the sibling half, and it stands on
evidence already in this repository: eleven grammars and their
ledgers, one shelved dialect split (typescript/tsx), one measured
impossibility (zig's `async`), one refusal argued in three legs (JSON
against JSONC, #273) — and one failed attempt this note exists to not
repeat (#162, SQL as a three-dialect union).

The one-sentence version: **a language family shares one grammar source
forever; what earns a registry row is a corpus and an oracle; what earns a
second parse table is a reading, not a name; and a union across siblings
is never built again.**

## 1. Where the union policy stands after eleven grammars

§4.2's rule — one grammar per language, accepting the union of its
versions, the latest winning where readings collide — rests on
measurement, and it works where its premise holds. Python 2.7 ∪ 3.x
sweeps 297,257 of 298,354 files with two gap files. Rust carries
editions 2015–2024 in one table. Java, C and YAML the same. The premise
is an **ordering**: versions form a line, so "the latest version wins"
is a decision rule, and `version_policy.toml` names the losses.

Four findings mark where the premise ends.

**1. Siblings have no ordering, so the union has no arbitration rule.**
The JSON decision (#273) is the clean statement: admitting JSONC's six
must-reject files would cost nothing in conflicts and everything in
meaning, "and JSONC does not obsolete JSON, so there is no version
ordering to justify spending them." MySQL does not obsolete PostgreSQL
either. Every place their readings collide — and §3 shows they collide at
the *lexer* — latest-wins has nothing to say, and the union becomes a coin
toss somebody has to weight by hand, per collision, forever.

**2. Nothing can narrow a union, and the widening is by design.** The
python grammar's own `fuzz_policy.toml` declares `print `, `exec ` and
`lambda (` as over-acceptance committed to on purpose: a py2 statement
form is a widening against py3's parser *and is meant to be one*. That is
the correct entry for a union table and the wrong answer for most
consumers: a tool that knows its input is py3 — which today is nearly
every tool — cannot ask this repository for a parser that rejects
`print x`. The union is a one-way door: it can accept the past cheaply,
and it cannot reject the past for the consumer whose present is all there
is.

**3. At the measured extreme, the union drops files outright.** Zig
removed `async`/`await`; keyword extraction is a lexer decision, so
wherever the keyword reading is valid the identifier reading cannot lex.
116 corpus files want the operator, 54 want the identifier, the grammar
cannot hold both, and 4 files fail — with §4.2's latest-wins rule
*deliberately not applied*, because applying it would trade 116 files for
those 4. That known-gap entry in `treebank-zig/ledger.toml` is the first
place a treebank ledger records "no single parse table exists for this
union," which is precisely the sentence §4.2 already wrote about
typescript/tsx and `<T>x`.

**4. Across dialects, the union blinds the instruments.** #273's third
leg names the trap in the abstract: a grammar wider than every oracle that
exists is wide *exactly where no oracle can contradict it*. #162 measured
it in the concrete. That attempt built SQLite ∪ PostgreSQL ∪ MySQL as one
grammar with a union oracle ("valid if either dialect accepts"), and
every dialect's divergences were "accepted everywhere" — backticks,
`::` casts, `#` comments, `ON DUPLICATE KEY UPDATE`, in every file alike.
That surrendered the negative corpus by construction: one table
cannot reject `SELECT a # b` for postgres while accepting it for mysql,
and no widening in the mysql-shaped region of a postgres file is visible
to any oracle the sweep consults. The same PR measured what visibility
costs when it arrives late: adding the MySQL oracle took adjudicated gaps
from 48 to 364 in one sweep — "a missing oracle doesn't make a grammar
look worse, it makes it look finished."

None of this says the union was a mistake. It says the union is **one
point on a ladder**, correct where versions form a line and the past is
cheap, and the repository has been using it as the only rung.

## 2. Rows: one registration rule for both axes

The rule already exists, in `crates/treebank-lang` and argued at length in
the HCL ledger: **a dialect earns a `LangName` when it brings its own
corpus and its own oracle.** JavaScript brings both — a different npm
population, its own checker — so it gets a registry row served by the
typescript crate, with its own corpus lock (`corpus-locks/javascript.json`)
and its own sweep block (`[corpus.javascript_sweep]`). Terraform brings
neither, so it amounts to three file extensions on the `hcl` row.

This note generalizes that rule to the version axis, because it already
fits without modification. A **version family** earns a row the same way a
dialect does: `python3` has its own oracle (CPython 3, pinned) and its own
population (modern PyPI); `python2` has its own oracle (CPython 2.7.18,
built from source) and its own population (final py2-compatible releases —
which the current ledger names as the union's declared blind spot: "the
py2 half of the union leans entirely on the corpus tests and batteries").
Both halves of the union already pay for their oracles; no consumer can
ask for either by name.

A **row** is: a canonical name, source extensions, a corpus lock, an
oracle, a negative corpus, sweep numbers in the family ledger, and a
fetchable pack. What a row is *not*, necessarily, is a parse table of its
own — rows may share a parser, exactly as `javascript` shares
typescript's today.

The registration ladder, lowest cost first — use the highest rung that can
express the variant, the same discipline `notes/field_guide.md` §1 applies
to ambiguity:

- **Rung 0 — extensions on an existing row.** The variant adds semantics,
  not syntax. Terraform on `hcl`; `.zon` on `zig`.
- **Rung 1 — a row over a shared parser.** Own corpus and oracle; the
  shared table already reads its text correctly. `javascript` today;
  `python3`/`python2` in phase 0 (§7). Where the row's accept-set is
  narrower than the table's, a narrowing manifest (§4) closes the gap.
- **Rung 2 — a row with its own parse table, generated from the family's
  shared source.** Earned by a *reading conflict* at measured incidence
  (§3). This is the typescript/tsx mechanism §4.2 planned and shelved —
  one grammar source, N generated parsers — used at last.
- **Rung 3 — a row with its own crate, extending a base grammar.** The
  variant is a language of its own with a superset community: `cpp` over
  `c`, through tree-sitter's grammar inheritance, "because a second copy
  is a copy that drifts."
- **Refusal.** The variant is a different grammar wearing a familiar
  extension, or rests on documentation rather than measurement. HCL's JSON
  profile (a different grammar, not this grammar with more rules — its
  ledger's own words, nearly), JSON5, NDJSON, T-SQL, PerfettoSQL. A refusal is
  recorded with its price, the way #273's `dialect_note` and the C macro
  story (§8.2) record theirs.

Two brakes keep the row set closed. A row's corpus must be a population
that exists *because the variant does* — ranked and locked on its own, not
a filter over another row's lock — and its oracle a separately pinned
reference implementation. And a row must serve consumers the existing rows
cannot: minor-version narrowing inside a family (`match` is 3.10+) is a
manifest entry (§4), never a `python3.10` row. The registry stays the
single source of truth; its `grammar` column becomes crate **plus parser**,
and extension uniqueness relaxes from per-row to per-family (§9), which is
the `.h` precedent — C owns `.h` while C++ headers exist — applied to
`.sql`.

## 3. What buys a table: a reading, not a name

Two rows sharing a family differ in one of two ways, and the ladder turns
on which.

**An accept-set difference** — text one row admits and the other rejects,
with the *same* tree wherever both admit — stays on rung 1. The shared
table parses everything; the narrower row rejects after the fact, by
manifest (§4). `print x` builds a `print_statement` no py3 file can
contain; narrowing is a membership test, not a parse.

**A reading difference** — the same bytes needing a different token or a
different tree in the two rows — is the one thing no shared table and no
manifest can deliver, because both mechanisms run downstream of the parse.
The evidence is the repository's hardest incidents: zig's `async`
(keyword extraction is a lexer decision; whichever reading the table
carries, the other cannot lex), typescript's `<T>x` against JSX (§4.2:
"a genuine grammatical ambiguity, not a precedence problem; no single
parse table exists"), and python's `print (x)`, which the union reads as
a call and a py2 consumer needs as a statement. A reading difference at
measured incidence buys rung 2: a second parse table from the same source.

The threshold stays a measurement, because the repository has already
priced both directions. TypeScript's cast measured ~0 corpus files, so
the split stayed shelved and the cast is a ledgered gap. Zig's `async`
measured 4 sacrificed files, and the ledger priced the trade and kept one
table; under this note that entry becomes a *standing quote* for a
`zig-legacy` row rather than a policy violation — purchasable the day the
4 grows. SQL needs no corpus to reach the threshold, because its reading
conflicts are lexical and fire in essentially every file:

- `'It\'s'` — one string in MySQL (backslash escapes by default), a
  string ending at the second `'` in PostgreSQL
  (`standard_conforming_strings`). Same bytes, different **token
  boundaries** — the strongest form of conflict, and the exact class
  behind #162's measured 45-second file (one mis-lexed escape sent 5 MB
  through error recovery).
- `SELECT 1--2` — `1 - (-2)` in MySQL (`--` comments require trailing
  whitespace there), a comment from `--` in PostgreSQL. Same bytes,
  different tree.
- `"name"` — a string literal in MySQL, an identifier in PostgreSQL. `#`
  — a comment opener in MySQL, an operator in PostgreSQL.
- Reserved-word sets that overlap but do not nest, in both directions,
  and move per **version** within each dialect (8.0 reserves `RANK`,
  `LATERAL`, `RECURSIVE`, `FUNCTION` where 5.7 did not) — #162 could
  reserve only a 29-word compromise list for three dialects at once.

That makes the sibling rule absolute where the version rule was
conditional:
**a union across siblings is never built.** Within one row, §4.2 survives
intact — a row that spans versions unions them, latest wins, and its
`version_policy.toml` declares the losses. The three §4.2 cases become
per-row law; entries whose `valid_in` lies wholly in another row (bare
`nonlocal`, valid only in py2) stop being policy and become that row's
ordinary negative fixtures.

## 4. Narrowing without a second table

Rung 1 needs one new artifact: `narrowing.json`, per family crate, keyed
by row. Each entry is a list of tree-sitter query patterns naming
**out-of-row occurrences** — for `python3`: `(print_statement)`,
`(exec_statement)`, the backtick repr, the old octal literal, the
`except E, e:` clause shape. The pack ships it beside `terms.json` and
expands it the way it already expands nominal terms; `Pack::fetch("python3")`
resolves to the shared parser plus the manifest, and a narrowed parse is
parse-then-scan: the tree comes back with its out-of-row occurrences, or
the call refuses the file, at the consumer's option. Names are stable from day
one — a row that later earns rung 2 keeps its name and quietly stops
needing its manifest.

The checker (`treebank terms` grows three rules, or a sibling
`treebank narrowing` command) holds it to the standard `terms.json` set:

1. Every pattern compiles against the row's grammar — a pattern naming a
   node the table cannot produce is dead text, refused.
2. **Liveness:** every pattern matches at least one file in the row's
   negative corpus (`test/negative/<row>/`), so a narrowing nobody can
   trip fails the gate the way a role nobody threads does.
3. **The sweep cross-checks the manifest against the verdict vector.**
   §4.2 already records per-oracle verdicts per file and calls that
   vector "the hook a future `version_of()` builds on." This is that
   hook, built: for every corpus file that an *other* row's oracle
   accepts and this row's rejects, at least one of this row's patterns
   must fire. A py2-only file that trips no `python3` pattern is a hole
   in the manifest, found mechanically.

An entry may carry the version bound it narrows away (`match_statement`
→ 3.10) so `version_of()` can report a floor as well as a family.
Nothing further builds on that today — §4.2's restraint, kept.

**The matcher is not hypothetical, and neither is its limit.** `fuzz`
grew `node_kind` for the same problem one rung over: a `starts_with`
prefix is positional, so a declared construct nested inside another
statement escapes its entry, and the way out that keeps the aim is to
name the construct rather than its position. It matches wherever the
construct appears and nowhere else, anonymous kinds included (py2's `<>`
is as declarable as a statement), and the loader rejects a kind the
grammar never produces — rule 1 above, already built. `narrowing.json`
should match the same way, and the two files should agree about what a
py2 construct is rather than each keeping a list.

The measurement that arrived with it bounds what any narrowing can
claim, which is why it belongs here. Declaring the five py2-union kinds
on python moved 31 fuzz findings out of undeclared, and **six should not
have moved**: `async def XX((XX)): XX` and ``async with `XX` < XX: XX``
put a py2-only construct inside a py3-only one, so they are valid in
NEITHER version — a real finding wearing a declared one's clothes.
Read as a narrowing rule, that says a py2-only node kind in a file does
not make the file py2. A manifest entry answers *this occurrence sits
outside the row*, never *this file belongs to the other row*; the
second
question is the oracle's, which is why rule 3 checks the manifest
against the verdict vector rather than deriving a verdict from it.

What a manifest can never do is change a reading — a limit to declare,
not to discover: a `python2` manifest over the union
table still hands a py2 consumer the call reading of `print (x)`, the
py3 tuple reading of `print >> f, x`, and no parse at all for `True = 5`
or `(True, False) = (1, 0)` — every one already recorded in the python
ledger as a deviation or an undeclared gap. That residue list *is* the
rung 2 case for a `python2` table, priced and waiting (§7).

## 5. Adjudication per row

§4.3 defined gap, version, widening and the rest against "the version
set" because the table under test was a union. Rows simplify it:

- **A row's oracle defines valid, for that row.** Each row sweeps its own
  locked population; a file its own oracle rejects is noise, exactly as
  today. The union oracle — "valid if any leg accepts" — dissolves,
  except *inside* a row that itself spans versions, where version legs
  remain (zig's two endpoints; a mysql row judging 8.x primary with a
  5.7 leg), along with the honest blind spot the zig ledger already
  declares for anything that lived only between the endpoints.
- **Widening tightens to the row.** Today's definition — accepted by us,
  rejected by *every* version oracle — was the union's; per row it
  becomes simply "the row's parser accepts what the row's oracle
  rejects."
  `treebank mutate` and `treebank fuzz` run per row against that single
  honest oracle, which is what restores their sight: a postgres-row fuzz
  finding of a backtick identifier is a widening again, where #162's
  union defined it as a feature.
- **Cross-row verdicts stay recorded** (rule 3 of §4) but adjudicate
  nothing; they check manifests and power `version_of()`.

## 6. SQL, planned as a family

SQL is the forcing case: the first target whose variation is siblings
first and versions second, and the subject of the one attempt (#162) that
applied §4.2 unchanged and closed at 265 adjudicated gaps with its
per-dialect instruments structurally blind. The plan:

**One crate, `treebank-sql`.** `common/` holds the core — the statement
skeleton (SELECT/INSERT/UPDATE/DELETE/MERGE, joins, subqueries, CTEs,
CASE, the expression ladder, the DDL shapes) with the vocabulary threaded
through it **once** for the whole family. The core is a grammar module
nobody generates directly. Each dialect is `grammar(core, …)` in its own
subdirectory — `postgres/grammar.js`, `mysql/grammar.js` — owning its
lexical layer (quoting, comments, escapes, operators), its reserved-word
list, and its dialect statements, each generating its own `src/` and
shipping its own wasm pack. A fix to how a join parses lands in every
dialect or in none — the cpp argument, inside one crate.

**No `sql` row, ever.** "SQL" names the crate and the shared source, not
a fetchable thing. The standard has no reference parser, and no sweep
can adjudicate a row without an oracle — #162's T-SQL fringe "rests on
documentation rather than measurement, and `deviations` says so," which
is the epitaph for any oracle-less row. `version_of()`'s sibling serves
the generic-`.sql` consumer: parse against the dialect rows and report
which accept, which is a *stronger* answer than one permissive table,
because a real oracle backs each verdict.

**The `postgres` row** comes first, because its oracle is the best in the
family: libpg_query — the server's own parser, extracted, pinned per
PostgreSQL major, giving verdict and tree in-process. Two majors give the
row its version legs the way zig's two binaries do. Node *starts* are
available where full spans are not; the capabilities entry says exactly
what that narrows the shape check to, rather than skipping the sentence.
Versions inside the row are §4.2's easy case — additive (`MERGE` in 15,
SQL/JSON in 16–17) — so one table unions them, and the manifest carries
the floors.

**The `mysql` row** salvages the best artifact of #162: the oracle. A
throwaway `mysqld --skip-networking` judging `PREPARE` by **error code,
not message** — 1064/1149 are the parser's; 1046/1049/1146/1054 are
about a schema it deliberately doesn't have; 1295 means the statement
parsed and the protocol won't carry it — already exists, proven and
written up. Versions inside the row are rust's edition problem (8.0's newly
reserved words against 5.7 identifiers) and take rust's solution:
soft/reserved keyword machinery in one table, `version_policy.toml` for
the collisions, a 5.7 oracle leg when the row claims 5.7. MySQL's
versioned comments (`/*!80000 … */` — code to one version, comment to
the rest) are the row's hardest single call, and its ledger decides
them, not this note.

**A `sqlite` row** is the natural third — #162's sqlite oracle (with its
unterminated-fragment fix) is the cheapest in the family — and it lands
as a registration, not a prerequisite.

**Corpora, two populations each, per the field guide's two-corpora
rule.** First-party: each engine's own test suite per released major —
the zig upstream-tarball pattern, which is the population a version claim
most needs — with the corpus adapter deciding what counts as SQL in
mixed harness files (that judgment is the `Ecosystem` trait's whole job).
Second: the migration/schema files of ranked OSS ecosystems, which
leans modern and machine-formatted, and says so in the ledger. #162's
Debian walk survives as the *mixed* population that exercises routing and
`version_of()`, not as a sweep population — sweeping unrouted
mixed-dialect files is how the union trap re-enters through the corpus.

**Refusals, recorded with prices:** T-SQL and PerfettoSQL (different
languages, no obtainable oracle — #162 measured PerfettoSQL at 22% of the
Debian corpus, which is a fact about that corpus, not an obligation).
PL/pgSQL and stored-routine bodies are **opaque spans at launch** — a
dollar-quoted or `BEGIN…END` body is one node, exactly the boundary #162
drew when the corpus corrected its trigger claim. libpg_query parses
PL/pgSQL, so a `plpgsql` row with tree-sitter injection into those spans
is a future registration that points at its own grammar — the Packer
sentence from the HCL ledger, transposed.

**What else #162 hands the family:** the negative fixtures, the ≥500-line
Debian floor and cap lesson, `NOT NULL` needing lexical precedence
before match length, the stranded-`ON` join fix, the quadratic-splitter
diagnosis method ("ask which process was actually busy"), and 15/22
structural terms threaded with `_modifier` demoted — the vocabulary work
transfers to `common/` nearly whole. The union grammar itself is quarry,
not foundation.

## 7. Python, in two phases

**Phase 0 — rows over the union table, no grammar change.** Register
`python3` and `python2` (rung 1), each with the oracle leg it already
pays for, its own lock (`python3` inherits the current modern-PyPI lock;
`python2` gets a vintage population of final py2-compatible releases,
ranked by historical judgment and declared as such — closing the ledger's
"leans entirely on the corpus tests and batteries" blind spot), its own
negative directory (seeded from `test/negative-oracle/py3|py2`, which
already exist), and its narrowing manifest. The `python` union row stays,
unchanged, serving the consumer who genuinely doesn't know. Deliverables:
`Pack::fetch("python3")` that *rejects* `print x` — the first narrowed
artifact this repository ships — and `version_of()` validated by the
sweep's verdict vectors.

**Phase 1 — tables, when the residue asks.** Parameterize the family:
`common/` core plus per-row deltas, the typescript/tsx mechanism
in-crate. The `python3` table drops the py2 statement forms *and their
soft-keyword machinery* — `print`/`exec` stop being soft keywords, and
the table can finally take the field guide §5 reserved list at full
strength, which the python ledger has wished for since it recorded
`return yield x` parsing as a variable read. The `python2` table restores
every reading the union sacrificed and the ledger already itemizes —
`print (x)` as a statement, `print >> f, x` as chevron print, `True = 5`,
`(True, False) = (1, 0)` (the known undeclared gap), bare
`await`/`nonlocal` as identifiers — and then **freezes**: the language
ended, so the table is write-once evidence with a dead-cheap canary.

Predictions phase 1 must check, stated now so the sweep can falsify them:
per-table parse states and declared conflicts drop below the union's
(soft keywords are fork pressure, §field-guide 2); `fuzz_policy.toml`'s
three declared-union entries disappear from the `python3` row (the
finding class becomes real rejections); the `python2` row's first sweep
books files the union has been mis-reading, not just mis-rejecting.

**The union's retirement is a measurement, not a promise.** When the
`python3` row's sweep over the py3 population matches the union's
numbers, `python` can alias `python3` and the third table can go; a
mixed-vintage consumer is better served by two honest verdicts than one
wide table. Until the numbers say that, the union stays put.

## 8. The rest of the fleet

Nothing moves uninvited. Rust, Java, C and YAML carry ordered version
unions, measured and cheap — §4.2 remains their whole story, now as the
within-row rule. TypeScript's un-split stands on its own ledger's
re-argument (the generic-arrow fix that made the split unnecessary at
measured incidence); its cast gap and its JSX-in-`.ts` deviation are now
*standing quotes* for a rung 2 split, re-priced by each sweep instead of
re-litigated. Zig's `async` entry becomes the template for a priced,
refused row (`zig-legacy`: oracle exists, corpus exists, 4 files do not
buy a table). C++ stays rung 3. And the JSON refusal (#273) becomes this
note's refusal rung, argued once and cited thereafter.

## 9. The diff, by subsystem

- **`treebank-lang`:** rows for `python3`/`python2` (phase 0), `postgres`
  /`mysql` (with the family). The `grammar` column becomes crate + parser
  name. Extension uniqueness relaxes to per-family: the family claims
  `.sql` with one designated default row (postgres, by the `.h`
  precedent), and corpus adapters route their own files.
- **Family crates:** `common/` + per-row `grammar.js` and `src/`;
  `tree-sitter.json` lists every generated grammar (upstream typescript's
  own layout, at the pinned CLI).
- **CI and packs:** "a directory with a grammar.js IS a grammar" gains
  one depth level (`crates/*/ */grammar.js`); `list-grammars.sh` prints
  parser names, not crate suffixes; one wasm pack per row; the pack
  manifest keys stay flat names.
- **Manifests:** `narrowing.json` per family crate, shipped in the pack
  beside `terms.json`; checker rules of §4 in `treebank terms` or a
  sibling command; expansion in `treebank` next to nominal expansion.
- **Ledgers:** per-row sweep blocks and locks — the
  `[corpus.javascript_sweep]` + `corpus-locks/javascript.json` precedent,
  applied everywhere a row lands. `version_policy.toml` stays per row.
- **DESIGN.md:** §4.2 retitled ("one *source* per family; tables as
  narrow as the evidence wants"), the ladder and the sibling rule added,
  §4.3 revised per §5 of this note. The rewrite lands as its own small
  PR once this note settles.

## 10. Order of work

1. **This note**, reviewed; then the DESIGN.md §4.2/§4.3 rewrite, alone.
2. **Python phase 0** — proves rows, manifests, per-row sweeps and
   `version_of()` with zero grammar risk, and ships the first narrowed
   pack (`python3`).
3. **`treebank-sql` with the `postgres` row** — proves the multi-parser
   crate mechanics greenfield, where there is no regression to cause:
   `common/` core, postgres lexical layer, libpg_query oracle, first-party
   corpus.
4. **The `mysql` row** — #162's oracle and fixtures salvaged, the
   reserved-word version machinery, the second population.
5. **Python phase 1**, if phase 0's residue numbers ask for it; the
   `sqlite` row; `zig-legacy` re-priced — each a measurement away, none a
   flag day.

Every step lands with its evidence, and no step depends on the one after
it. The first time two rows disagree about a file, that disagreement is a
feature with a name — `version_of()` — instead of a conflict one parse
table had to swallow.
