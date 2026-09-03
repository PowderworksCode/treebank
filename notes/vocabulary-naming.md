# The vocabulary's words: `role` and `facet`

**Status: accepted and applied.** `structural`/`nominal` was the owner's
choice from a shortlist; §4 records `supertype`/`subtype` and the rest with
the reasons they lost. The rename landed as one PR rather than the four §7
proposed — also the owner's call, and cheaper than §7 assumed once the scope
turned out to be 872 lines rather than 19,000. The "today" and "before" text
throughout describes the state this replaced, and is kept as the argument's
evidence.

> "Find a better name for facets and roles in treebank, those two don't make
> sense to me and I can never remember them."

The complaint is correct and the cause is structural rather than a matter of
taste. This proposes one replacement pair, states what each word denotes, and
lists the candidates that were rejected and why.

## 1. What the two words denote today

Established from the source, not from the docs.

A **role** is a cross-language syntactic category in a closed vocabulary,
spelled `_declaration`, `_loop`, `_callable`. There are 29 of them
(`crates/treebank/vocabulary/vocabulary.json`: 22 under `table`, 7 under
`facets`).

Each grammar delivers a role by one of two mechanisms:

- **Table tier** — a real tree-sitter supertype threaded through the
  productions. Membership is *occurrence-level*: the parse went through it
  here. Enforced at generate time, and queryable from the raw parser with no
  treebank machinery: `(_declaration) @decl`.
- **Facet tier** — an explicit list of concrete node types in `roles.json`,
  substituted into the query before it runs. Membership is *type-level*: a
  `function_definition` is `_callable` wherever it occurs.

The tier is a property of the grammar, not of the role. Nine demotions exist
across the eleven grammars:

| grammar | demoted |
|---|---|
| c, cpp, python | `_parameter` |
| rust, zig | `_modifier`, `_parameter` |
| typescript | `_declaration`, `_modifier` |
| java | `_declaration` |

The criterion is mechanical. Every one of the nine reasons in `roles.json` has
the same two-part shape: *one alternation here would accept this invalid
program*, then *every member is a concrete node type occurring nowhere else,
so facet membership selects exactly the nodes the supertype would have*. Zig's
`_modifier` would accept `threadlocal pub extern const x`; its `_parameter`
would accept `extern fn printf(fmt: [*:0]const u8, ..., n: c_int)`; java's
`_declaration` would make every field ambiguous with every local; typescript's
`_modifier` accepted `override var x` until it was demoted.

So the second tier names *a category the parse table cannot express without
accepting invalid programs, resolved by name list at query time.* Any
replacement has to carry that, and has to make the tier relationship obvious.

## 2. Why the current pair fails

### 2.1 The two names are not on the same axis

`table` names a **mechanism** — the generated parse table. `facet` names an
**epistemic metaphor** — one face of a many-sided thing. They are not
comparable, so a reader cannot derive either from the other, and neither
implies that the two are alternative deliveries of one idea.

This is visible in the data. `vocabulary.json`'s two keys are literally
`"table"` and `"facets"`: a mechanism next to a metaphor, presented as peers.

### 2.2 "Facet" means the wrong thing in the field it is borrowed from

In information retrieval a facet is an *independent axis* you filter along
simultaneously — colour × size × brand. Treebank's facets are not an
independent axis. They classify along the **same** axis as supertypes, by a
different mechanism. The word therefore suggests exactly the misreading the
owner reported: that facets and roles cut the tree in two different ways.

### 2.3 Two nouns of equal standing read as two things

"Roles and facets" is a coordinate construction. Nothing in "facet" says "kind
of role". The owner read them as siblings because the language makes them
siblings.

### 2.4 The filename says something false

`roles.json` contains no table-tier roles. It contains `facets`, `demoted` and
`uncategorised`. The table-tier roles live in `grammar.js`'s `supertypes:`
array. A reader who opens the file named for the genus finds only one of the
two species in it — and concludes the species in the file is the genus's
sibling. **The filename manufactures the exact confusion the owner reported.**

### 2.5 "Role" is already three other things

- `[[oracles]].role` in all eleven ledgers means "this oracle's job in the
  sweep". Different concept, same word, same repository.
- In the literature this project is named after, a *role* is a semantic role
  (PropBank `ARG0`/`ARG1`) — a per-occurrence relation between an argument and
  its predicate. Treebank's roles are categories, not relations. The nearest
  neighbouring field has already taken the word for the opposite kind of thing.
- The code itself calls this object a `Term`: `Vocabulary { table: Vec<Term> }`,
  `table_terms()`, `assertTableTerms`, "one vocabulary term".

### 2.6 The repository has never settled on one wording

Six independent explanations, six different word-sets:

| where | words used |
|---|---|
| `notes/DESIGN.md` §3.1 | Table tier / Facet tier |
| `crates/treebank/src/lib.rs` | Table tier / Facet tier, "roles that cross-cut" |
| `crates/treebank/src/expand.rs` | "Facet roles", "Table-tier supertypes" |
| `README.md` | "Table-tier roles" / "Facet-tier roles" |
| `queries/highlights.scm` | "A SUPERTYPE is…" / "A FACET is…" |
| `site/content/concepts/two-tiers.md` | "Supertypes" / "Facets"; "role" survives only in a passing clause and in the command name |
| `site/src/grammar-viewer.mjs` | headings "Supertypes" / "Facets" |

Note the direction of the drift: **every user-facing surface drops "role" and
"tier" and teaches with "supertype" and "facet".** The genus word survives only
in identifiers. That is the repository telling us which half of the pair is
load-bearing.

## 3. Proposal

**One noun, two adjectives.**

> The vocabulary has 29 **terms**. Each grammar delivers a term one of two
> ways. A **structural** term is a real supertype in its parse table:
> membership is decided by structure — the parse went through it *here*. A
> **nominal** term is a list of node types in `terms.json`, substituted into
> the query before it runs: membership is decided by name — the node's type is
> on the list. Which way is a fact about the grammar, not about the term.
> Moving a term from structural to nominal is a **demotion**, and it is forced
> rather than chosen: a supertype there would widen the grammar.

| today | proposed |
|---|---|
| role, vocabulary term | **term** |
| table tier, table-tier role | **structural term** |
| facet, facet tier, facet-tier role | **nominal term** |
| occurrence-level / type-level membership | **structural / nominal membership** |
| demotion | **demotion** (unchanged) |
| `roles.json` | `terms.json` |
| `"facets": {…}` | `"nominal": {…}` |
| `vocabulary.json` `"table"` / `"facets"` | `"structural"` / `"nominal"` |
| `either_tier` | `demotable` |
| `TABLE_TIER` / `FACET_TIER` (js) | `STRUCTURAL_TERMS` / `NOMINAL_TERMS` |
| `assertTableTerms` | `assertStructuralTerms` |
| `RolesManifest` | `TermsManifest` |
| `ROLES` const, `tb_roles` export | `TERMS`, `tb_terms` |
| `Pack::roles()`, `PackRoles` | `Pack::terms()`, `PackTerms` |
| `treebank roles` | `treebank terms` |
| `roles_note` (ledger) | `vocabulary_note` |
| `[[oracles]].role` | **unchanged** — the one correct use of the word here |

Note that **`supertype` survives, and is doing the right job**. It stops being
a tier name and goes back to being what it is in tree-sitter: the mechanism a
structural term is delivered by. "A structural term is a real supertype" is a
sentence; "the table tier" was not.

### Why it beats the current pair

1. **It cannot produce the misreading.** "Structural terms and nominal terms"
   are visibly two kinds of one thing. "Roles and facets" are two nouns.
2. **Both names sit on one axis** — how membership is decided — and each is
   derivable from the word itself. Structure put it there, or its name is on a
   list. No metaphor, and nothing to memorise.
3. **The repository already draws this distinction and already has a name for
   it.** `roles.rs` ("Facets are type-level: a node type is `_callable`
   wherever it occurs"), `lib.rs`, `python/grammar.js` and `notes/DESIGN.md`
   §3.1 and §3.1.1 all reason in *occurrence-level* versus *type-level*. This
   is not a new distinction: it is the existing one, spelled in one word each
   instead of two hyphenated ones, and **promoted from the explanation to the
   name**. That the repository needed a second private vocabulary to explain
   its public one is the clearest evidence the public one was wrong.
4. **It removes a synonym rather than adding one.** The code already calls this
   object a `Term`; "role" was the third word for it.
5. **It fixes the filename.** `terms.json`, holding `nominal`, `demoted` and
   `uncategorised`, is what the file is.
6. **The existing prose improves.** All nine demotion reasons end with the same
   clause. Today: *"so facet membership selects exactly the nodes the supertype
   would have"* — a metaphor measured against a mechanism. After: *"so nominal
   membership selects exactly the nodes the structural supertype would have."*
   And DESIGN.md §3.1.1's soundness condition reads *"structural and nominal
   membership agree exactly when every member is a concrete node type occurring
   nowhere else"*, which is the whole demotion rule in one line.

### Two honest caveats

**The justification is the plain-English reading, not the type-theory one.** In
type theory, nominal typing means membership by declaration and structural
typing means membership by shape — and by that reading both tiers are nominal,
since a grammar declares its supertypes as surely as a manifest lists its node
types. The pair earns its place here on the ordinary meanings of the words: by
*structure*, meaning where the node sits in the parse; by *name*, meaning what
the node type is called. That is a feature rather than a compromise — it means
a grammar author who has never read a type-systems paper can still derive the
distinction from the words, which is the whole point.

**"Nominal" carries a faint pejorative — "in name only" — and that is
correct.** A nominal term enforces nothing; the manifest asserts membership and
the parse table has no opinion. The site already argues this is the property a
consumer must not be allowed to forget: *"a role that looks enforced and is not
is worse than a list that is honest about being one."* The overtone does that
work at no cost.

**Dilution, not collision.** "Structural" appears about twenty times in the
repository as an ordinary adjective — "structurally blind to accepts-invalid",
"structural debt" in every `lint_policy.toml`, "the fix direction stays
structural, NOT weights". None is a competing technical term, but after this
lands "a structural term" is jargon sitting next to "structurally blind" which
is not. "Nominal" is cleaner: two uses, both meaning a nominal-versus-real
version union, in the zig and yaml ledgers.

## 4. Candidates rejected

**`supertype` / `subtype`.** Rejected on hard evidence: `subtypes` is already a
key in every generated `node-types.json`, where it means *the concrete members
of a supertype* — `_access` carries `subtypes: [member_expression,
subscript_expression]` — and `crates/treebank/src/node_types.rs` reads it (19
such entries in rust alone). Worse, it inverts the containment. `_callable`
stands to `function_definition` in exactly the relation `_access` stands to
`member_expression`: its members *are* subtypes, and `_callable` is the
would-be supertype, the one the parse table could not hold. Naming the tier for
the wrong end of its own relation would make the demotion reasons unreadable.

**`supertype` / `list`.** The first version of this document proposed it.
Rejected as lopsided: one word carries the entire mechanism and the other
carries almost nothing, so the pair does not read as a pair — which is the
defect it was meant to fix.

**`parser-side` / `query-side`.** Genuinely good, and the only pair you can
re-derive with no memory at all: it names where the term is resolved and
therefore answers the practical question directly ("can I use this from raw
tree-sitter?"). Rejected in favour of the chosen pair, which names *why* rather
than *where* — a term is query-side **because** its membership is nominal, and
naming the cause makes the demotion reasons write themselves.

**`supertype` / `roster`.** A roster is precisely an explicit list of named
members, so the word is accurate and memorable. Rejected for the same lopsidedness
as `list`, and for being a coinage where `nominal` is a borrowing.

**Keep `role`/`facet` and document them better.** Rejected — see §6. They are
documented six times over; the six do not agree on the words.

**`virtual supertype` / `pseudo-supertype` / `soft supertype`.** Tempting: it
carries the tier relationship perfectly and needs no new noun. Rejected on the
repository's own argument — *"a role that looks enforced and is not is worse
than a list that is honest about being one"*. Naming it a supertype of any kind
hides that it enforces nothing, which is the one property a consumer must know.

**`alias` / `macro` / `shorthand`.** Instantly clear, and true of the
mechanism. Rejected: an alias implies exact two-way substitutability, and this
substitution is one-way and lossy — a list cannot say anything about position.
It also says nothing about *why* the thing is not a supertype.

**`overlay` / `projection` / `view`.** Rejected: database metaphors, all of
which imply a derived thing computed from a base. Backwards — the list is
primary, hand-maintained data, and the supertype is what could not be built.

**`occurrence-level` / `type-level`.** The repository's own pair, and the
literal meaning of the chosen one. Rejected as the tier names for length: they
are two hyphenated compounds that do not survive being used forty times in a
paragraph, which is presumably why they stayed in the explanations and never
became the names.

**`aspect`.** Same metaphor family as facet, same failure.

**`kind`.** Collides with `kinds_check` / `kinds_coverage` (a different
measurement, in two ledgers) and with tree-sitter's own "node kind".

**`category`.** Accurate and colourless. `(_loop)` reads badly as "a category".

**`class`, `tag`, `concept`.** `class` collides with the languages being
parsed; `tag` implies something a tool attached rather than something the
parser derived, which is wrong for supertypes; `concept` is vaguer than
`facet`.

**`table tier` / `list tier`.** Keeps "tier", a fourth word that adds nothing
and implies a ranking. The ranking happens to be real — the structural tier is
stronger — but that belongs in a sentence, not in every noun phrase.

**Renaming the terms themselves** (`_callable`, `_binding`, …). Out of scope
and not asked. Spellings and membership are unchanged by this proposal.

## 5. What it costs

**Scope is 872 lines across 136 files, not 19,000.** The 19,000 figure counts
corpus-lock file paths: `corpus-locks/yaml.json` alone contains 14,982
occurrences of `role`, every one of them an Ansible `roles/` directory in a
measured repository. Excluding `corpus-locks/` and the generated
`site/public/status.json`, the real footprint is 136 files and 872 lines, of
which roughly two-thirds are prose. There are 11 `roles.json` files and 11
`ledger.toml` files, not 12 and 11.

**No query changes.** `(_callable)` stays `(_callable)`. Parse tables are
byte-identical; `node-types.json` is untouched; no grammar regenerates.

**Vocabulary version: do not bump.** The closed term list, the tiers, and every
membership are unchanged — only the words describing them move.
`notes/DESIGN.md` §3.2.1's own argument applies directly: the version is an
identity for the term list, and what protects a stale manifest is the
structural checking in `check.rs`, not the string. Bumping it here would claim
a semantic change that did not happen.

**The one real consumer cost is the pack wire format.** Grammar crates are
`publish = false`; they ship as content-addressed wasm packs published
continuously. A pack exports `tb_roles()` returning JSON with a `"facets"` key,
and `crates/treebank/src/pack.rs` and `site/src/grammar.mjs` both read it. Old
packs stay on the CDN. Mitigation is two lines: `#[serde(alias = "facets")]` on
the renamed field, and export `tb_terms` alongside `tb_roles` for one cycle.

`treebank` itself is published, so `Pack::roles()` → `Pack::terms()` and
`RolesManifest` → `TermsManifest` are breaking changes to one method and one
type on a 0.x crate. CHANGELOG entry, minor bump.

## 6. The negative result, tested

The brief asks whether the words are actually right and merely undocumented.
They are not undocumented. `notes/DESIGN.md` §3.1 and §3.1.1 spend some 75 lines on
them; there is a dedicated site page; `queries/highlights.scm`, `lib.rs`,
`expand.rs`, `README.md` and `build.sh` each explain them again. Six
explanations.

The table in §2.6 is the disproof. Six explanations, six different word-sets,
and every user-facing one abandons the genus word entirely. A vocabulary that
six earnest attempts cannot state the same way twice is not under-documented;
its words do not fit. Adding a seventh explanation would first have to choose
which of the six wordings is correct — which is this document.

The partial negative result worth recording: **"facet" is not the harder word.
"Role" is.** "Facet" at least has one consistent referent. "Role" has four —
the genus, `oracles[].role`, the semantic-role meaning a reader brings, and the
filename that promises the genus and delivers one species. If only one word
moves, move `role`.

## 7. How it landed

One PR, at the owner's direction. What moved:

- **The vocabulary** — `vocabulary.json`'s `table`/`facets`/`either_tier` →
  `structural`/`nominal`/`demotable`; `vocabulary/supertypes.js` →
  `vocabulary/terms.js` with `STRUCTURAL_TERMS`, `NOMINAL_TERMS`, `DEMOTABLE`
  and `assertStructuralTerms`.
- **The crate** — `roles.rs` → `terms.rs`, `RolesManifest` → `TermsManifest`
  (`facets` field → `nominal`, with `#[serde(alias = "facets")]`),
  `Pack::roles`/`roles_json`/`PackRoles` → `terms`/`terms_json`/`PackTerms`,
  `check::dead_roles` → `dead_terms`.
- **The manifests** — `roles.json` → `terms.json` in 11 crates, key `facets`
  → `nominal`, `ROLES` → `TERMS`.
- **The pack ABI** — packs now export `tb_terms`/`tb_terms_len`, and keep
  `tb_roles`/`tb_roles_len` returning the same document for one cycle.
  `Pack::load` prefers the new name and falls back, so a pack already on the
  CDN still loads; `check.sh` asserts the two exports agree.
- **The CLI** — `treebank roles` → `treebank terms`, with `roles` kept as a
  clap alias so no muscle memory or script breaks. Its summary line now reads
  `terms OK: 17 structural, 9 nominal, …`.
- **The ledgers** — `roles_note` → `vocabulary_note` in all 11, and the tier
  words in their prose.
- **Everything else** — `notes/DESIGN.md` §3 and §3.3, `README.md`,
  `queries/highlights.scm`'s header and the 22 regenerated per-grammar files,
  the site's vocabulary page (`two-tiers.md` → `terms.md`), the grammar viewer
  and playground, `tools/wasm-pack`, the rosetta fixtures, and CI.

Gates run: `cargo build --workspace`, `cargo test --workspace`,
`treebank verify` on all 11 grammars, `treebank terms` on all 11,
`treebank status --check`, `treebank queries --check`, and the site's
`bun test` / `typecheck` / `lint`.

`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` each fail
on exactly one file, `crates/treebank-oracle/tests/java_oracle_is_not_stale.rs`,
which this branch does not touch and which fails identically on `main`. CI runs
neither, which is how it got there. Left alone: folding an unrelated formatting
and lint fix into a 136-file rename would make both harder to review.

## 8. Appendix: how it reads in situ

Every "before" below is the current text, verbatim. Nothing has been applied.

### A demotion reason — `crates/treebank-zig/roles.json`

Before:

> Zig orders its modifiers rather than pooling them: `pub` precedes a
> declaration, `export`/`extern` follow it, `threadlocal` sits between that and
> `var`, `comptime`/`noalias` belong to a parameter, and
> `const`/`volatile`/`allowzero` to a pointer. One alternation across all of
> them accepts `threadlocal pub extern const x` and every other permutation of
> a list the language fixes. Each member is a concrete node type occurring
> nowhere but a modifier slot, **so facet membership selects exactly the nodes
> the supertype would have.**

After — only the last clause moves, in all nine demotion reasons:

> …Each member is a concrete node type occurring nowhere but a modifier slot,
> **so nominal membership selects exactly the nodes the structural supertype
> would have.**

### The soundness condition — `notes/DESIGN.md` §3.1.1

Before:

> **Why this is one meaning and not two.** Occurrence-level and type-level
> membership agree exactly when every member of the term is a concrete node
> type that occurs nowhere else.

After:

> **Why this is one meaning and not two.** Structural and nominal membership
> agree exactly when every member of the term is a concrete node type that
> occurs nowhere else.

That sentence is the demotion rule, and it now uses the tier names rather than
a second private vocabulary invented to explain them.

### The site page — `site/content/concepts/two-tiers.md`

Before:

> The vocabulary comes in two kinds, and the split is decided by what
> Tree-sitter can enforce rather than by what would read nicely.
>
> **Supertypes** are threaded through the productions and enforced when the
> parser is generated. A query for `(_expression)` matches where the parse
> actually went through it — matching is by derivation, not by node type. …
>
> **Facets** are lists of node types in `roles.json`, expanded into a concrete
> alternation when a query loads. A facet cannot say anything about position,
> because it does not exist in the parse table.
>
> Where a term can be threaded it is a supertype. Where it cannot, it is a
> facet, because a role that looks enforced and is not is worse than a list
> that is honest about being one.

After:

> Every term is one of two kinds, and which one is decided by what Tree-sitter
> can enforce rather than by what would read nicely.
>
> **Structural** terms are threaded through the productions as real supertypes
> and enforced when the parser is generated. A query for `(_expression)`
> matches where the parse actually went through it — membership is decided by
> structure, not by node type. That lets a structural term say something a list
> cannot: that *this position in this production* is an expression.
>
> **Nominal** terms are lists of node types in `terms.json`, expanded into a
> concrete alternation when a query loads. Membership is decided by name: a
> `function_definition` is `_callable` wherever it occurs. A nominal term
> cannot say anything about position, because it does not exist in the parse
> table.
>
> Where a term can be threaded it is structural. Where it cannot, it is
> nominal, because a term that looks enforced and is not is worse than a list
> that is honest about being one.

The page's file name goes with it: `two-tiers.md` → `terms.md`, and its
description — currently "Supertypes and facets, and why there are two kinds" —
becomes "Structural and nominal terms, and why there are two kinds".

### The README

Before:

> Table-tier roles are queryable straight from the parser, because they are
> real supertypes in the parse table…
>
> Facet-tier roles (`_callable`, `_binding`, `_scope`, `_clause`) cross-cut
> derivations, so they cannot be supertypes; they ship as `ROLES` and are
> expanded before the query runs.

After:

> Structural terms are queryable straight from the parser, because they are
> real supertypes in the parse table…
>
> Nominal terms (`_callable`, `_binding`, `_scope`, `_clause`) cross-cut
> derivations, so they cannot be supertypes; they ship as `TERMS` and are
> expanded before the query runs.

### The manifest — `crates/treebank-zig/terms.json`

```json
{
  "vocabulary": "0.1.0",
  "demoted": {
    "_modifier": "Zig orders its modifiers rather than pooling them: … so nominal membership selects exactly the nodes the structural supertype would have.",
    "_parameter": "The C-variadic `...` is only ever the last parameter … so nominal membership selects the same nodes."
  },
  "nominal": {
    "_callable": ["function_declaration", "function_type", "test_declaration"],
    "_scope": ["block", "container_declaration", "function_declaration", "source_file", "test_declaration"]
  },
  "uncategorised": [ … ]
}
```

### The checker's output

Before:

```
roles OK: 17 supertypes, 9 facet(s), 98 named node(s), 11 uncategorised (vocabulary 0.1.0)
```

After:

```
terms OK: 17 structural, 9 nominal, 98 named node(s), 11 uncategorised (vocabulary 0.1.0)
```

### The grammar viewer's headings — `site/src/grammar-viewer.mjs`

`Supertypes` / `Facets` become `Structural` / `Nominal`, over a sentence that
now states the rule instead of naming two mechanisms: *"A structural term is
threaded through the productions, so `(_expression)` matches where the parse
went through it. A nominal term is a list of node types in `terms.json`,
expanded when the query loads."*
