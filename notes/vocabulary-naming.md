# The vocabulary's words: `role` and `facet`

A recommendation, not a change. Nothing in this document has been applied.

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
> ways: as a **supertype** — a real rule in its parse table — or as a **list**
> — a set of node types in `terms.json`, substituted into the query before it
> runs. Which way is a fact about the grammar, not about the term. Moving a
> term from the first to the second is a **demotion**, and it is forced rather
> than chosen: a supertype there would widen the grammar.

| today | proposed |
|---|---|
| role, vocabulary term | **term** |
| table tier, table-tier role | **supertype term** — "delivered as a supertype" |
| facet, facet tier, facet-tier role | **list term** — "delivered as a list" |
| demotion | **demotion** (unchanged) |
| `roles.json` | `terms.json` |
| `"facets": {…}` | `"lists": {…}` |
| `vocabulary.json` `"table"` / `"facets"` | `"supertype"` / `"list"` |
| `either_tier` | `demotable` |
| `TABLE_TIER` / `FACET_TIER` (js) | `SUPERTYPE_TERMS` / `LIST_TERMS` |
| `assertTableTerms` | `assertSupertypeTerms` |
| `ROLES` const, `tb_roles` export | `TERMS`, `tb_terms` |
| `treebank roles` | `treebank terms` |
| `roles_note` (ledger) | `vocabulary_note` |
| `[[oracles]].role` | **unchanged** — the one correct use of the word here |

One-line gloss, for the docs: *supertype membership is structural — the
derivation went through it. List membership is nominal — the node's name is on
a list.*

### Why it beats the current pair

1. **It cannot produce the owner's misreading.** "Supertype terms and list
   terms" are visibly two kinds of one thing. "Roles and facets" are two nouns.
2. **Both names sit on one axis** — how the grammar delivers the term — and
   both are literally true, checkable statements about the artifact. No
   metaphor.
3. **It removes a synonym rather than adding one.** The code already calls this
   object a `Term`; "role" was the third word for it.
4. **It fixes the filename.** `terms.json`, holding `lists`, `demoted` and
   `uncategorised`, is what the file is: this grammar's statement about the
   vocabulary's terms.
5. **It frees "role"** for `oracles[].role`, and stops colliding with semantic
   roles.
6. **The existing prose improves.** All nine demotion reasons end with the same
   clause. Today: *"so facet membership selects exactly the nodes the supertype
   would have"* — a metaphor measured against a mechanism. After: *"so list
   membership selects exactly the nodes the supertype would have."* That is a
   plain sentence a reader can check.

## 4. Candidates rejected

**Keep `role`/`facet` and document them better.** Rejected — see §6. They are
documented six times over; the six do not agree on the words.

**Rename `role`→`term`, keep `facet`.** Rejected. The metaphor-versus-mechanism
mismatch (§2.1) is the actual defect, and this leaves it in place.

**`virtual supertype` / `pseudo-supertype` / `soft supertype`.** The most
tempting: it carries the tier relationship perfectly and needs no new noun.
Rejected on the repository's own argument — *"a role that looks enforced and is
not is worse than a list that is honest about being one"*
(`site/content/concepts/two-tiers.md`). Naming it a supertype of any kind hides
that it enforces nothing, which is the one property a consumer must know.

**`alias` / `macro` / `shorthand`.** Instantly clear, and true of the
mechanism. Rejected: an alias implies exact two-way substitutability, and this
substitution is one-way and lossy — a list cannot say anything about position.
It also says nothing about *why* the thing is not a supertype.

**`overlay` / `projection` / `view`.** Rejected: database metaphors, all of
which imply a derived thing computed from a base. Backwards — the list is
primary, hand-maintained data, and the supertype is what could not be built.

**`nominal` / `structural`.** The most precise pair available; it names the
membership rule exactly. Rejected as the headline for being philosophy jargon
in a repository read by grammar authors. Kept as the one-line gloss above,
where its precision earns its keep.

**`aspect`.** Same metaphor family as facet, same failure.

**`kind`.** Collides with `kinds_check` / `kinds_coverage` (a different
measurement, in two ledgers) and with tree-sitter's own "node kind".

**`category`.** Accurate and colourless. `(_loop)` reads badly as "a category".

**`class`, `tag`, `concept`.** `class` collides with the languages being
parsed; `tag` implies something a tool attached rather than something the
parser derived, which is wrong for supertypes; `concept` is vaguer than
`facet`.

**`table tier` / `list tier`.** Keeps "tier", a fourth word that adds nothing
and implies a ranking. The ranking happens to be real — the table tier is
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

## 7. How to land it

Four PRs, one concern each, in this order. Nothing before PR 1 is approved.

1. **Prose only** — `notes/DESIGN.md`, `README.md`, `site/content/`,
   `queries/*.scm` headers, module docs. No identifier moves. Cheapest to
   review, and it is where the confusion actually reaches a reader. If the
   owner wants to stop after this one, the naming problem is 80% solved.
2. **The vocabulary crate** — `vocabulary.json` keys, `supertypes.js`
   constants, `check.rs`, with serde aliases so nothing downstream moves yet.
3. **The manifests** — `roles.json` → `terms.json` in 11 crates, `facets` →
   `lists`, `ROLES` → `TERMS`, the pack export and the site's reader.
4. **The CLI and ledgers** — `treebank roles` → `treebank terms` with `roles`
   kept as a hidden alias; `roles_note` → `vocabulary_note` in 11 ledgers.

Gates on each: `cargo build --workspace`, `cargo test --workspace`,
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`treebank verify`.
