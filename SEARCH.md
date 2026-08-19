# A code query engine on treebank

A sketch, not a commitment. Assumes an offline indexing pass and is free to
use whatever crates exist.

## 1. Prior art, because most of this has been done

The first draft of this document claimed the structural-index idea had
"never been applied to code." That is false, and the correction matters
more than the design does.

**srcML** (Collard and Maletic, 2002 onward) marks source up as XML and
queries it with XPath and XSLT. That is literally XML query technology
applied to code, and it has been shipping for twenty years.

**Babelfish / bblfsh** (source{d}, ~2017-2019) is the closest prior work by
some distance. It parsed many languages into a *Universal AST* annotated
with language-agnostic **roles** -- the same word this repo uses -- and
queried it with XPath, explicitly so that one query could extract the same
feature across languages. That is the thesis of §3 of DESIGN.md, built and
shipped eight years earlier.

**Kythe** (Google) defines one language-agnostic schema for cross-language
cross-references. **Glean** (Meta, open-sourced 2024) stores schema-defined
facts about code and queries them with Angle, a Datalog-ish logic language,
over RocksDB. **CodeQL**, descended from Semmle's .QL and from CodeQuest
(Hajiyev, Verbaere, de Moor, 2006), compiles an object-oriented query
language down to relational queries over an indexed database of ASTs.

So: indexed structural code search, cross-language shared vocabularies, and
tree-query languages are all well-trodden. Nothing below is a new idea.

### What is actually left

Two things, and they are narrow.

**Where the vocabulary is enforced.** Babelfish applied roles as a
post-hoc annotation layer -- a per-language DSL mapping a native AST onto
UAST roles. That is precisely the "query layer that can drift" this project
exists to avoid; treebank puts the vocabulary in the grammar, so a node
either derives from `_invocation` or the grammar does not build it. This is
a difference in how the vocabulary is *maintained*, not in the idea of
having one.

**Measured per-language confidence.** None of the systems above publish
anything like the sweep numbers -- "this parser agrees with the reference
parser on N of M real files." For a search index that is not a vanity
metric: it is the difference between "no results" and "no results in the
61% of bash files we can read," and §8 turns on it.

### And a warning

source{d} shut down. Babelfish is unmaintained. The hard part was never the
index -- it was keeping N language frontends mapped onto one vocabulary as
N languages kept changing. That is exactly treebank's cost structure, and
it is the risk this design carries.

## 2. The gap that is actually open

Given all that, the useful framing is not "nobody has done this." It is
that the two things people *use* today sit at opposite extremes.

**Lexical.** GitHub's blackbird, Sourcegraph's zoekt, ripgrep. Trigrams
over raw bytes then a regex verification pass. Fast, language-blind,
structurally illiterate.

**Parse-at-query-time.** ast-grep, comby, Semgrep. Structurally literate,
but they parse the corpus per query, so cost scales with corpus size on
every query rather than once at index time.

The indexed-structural middle is where srcML, Kythe, Glean and CodeQL live,
and none of them is what a developer reaches for to search code -- they are
analysis platforms with heavyweight ingest. The open question is whether a
tree-sitter-speed frontend with an enforced shared vocabulary can put
something in that middle that is cheap enough to actually get used.

That is a positioning bet, not a novelty claim.

### Shape

    ingest ──> parse ──> node records ──> segment build ──> query ──> verify
                                              (immutable, merged in background)

Immutable segments with background merge, LSM-style, as tantivy and zoekt
both do. It makes indexing embarrassingly parallel, makes deletion a
tombstone, and makes the grammar-version problem tractable (§8).

## 3. What the indexer emits

One record per named node. The whole design turns on this record being
small enough to keep the posting lists cheap.

| field | width | why |
|---|---|---|
| `node_id` | u64 | `file_id << 24 \| preorder_index`. Sorting by it makes every structural join a merge join. |
| `kind` | u16 | the concrete grammar kind |
| `super` | u32 bitset | the table tier — one bit per §3.2 term |
| `facet` | u32 bitset | the `roles.json` tier, expanded by `treebank-core` |
| `pre`, `post` | u32, u32 | interval label: `a` contains `b` iff `a.pre < b.pre && b.post < a.post` |
| `depth` | u16 | lets `>` (direct child) be a depth check rather than a parent lookup |
| `field` | u16 | the field name binding this node to its parent |
| `name` | u64 | hash of the identifier text, for name-bearing nodes only |

Two things are worth defending.

**Supertype membership is materialised at index time, not query time.**
§2 fact 4 says supertype queries are derivation-based: whether a node is an
`_expression` depends on how it was derived, not on its type. That is not
recoverable from a `(kind, parent)` pair after the fact, so the bit has to
be written while the tree is still in hand. This is the single most
important consequence of treebank's design for the index.

**Interval labels, not parent pointers.** `(pre, post)` turns ancestor
containment into an integer comparison, so `(_declaration (_invocation))`
is a merge join between two sorted lists with no tree walk. The cost is that
insertion renumbers a file — which is fine, because segments are immutable
and a changed file is a new segment.

## 4. Index structures and the crates for them

- **Posting lists** — `roaring` for the sparse sets, `bitpacking` for the
  delta-coded `node_id` runs. Keyed on `(lang, kind)` and on each supertype
  and facet bit.
- **Name dictionary** — `fst`. A minimal-perfect automaton over identifiers
  gives prefix, fuzzy and regex-constrained lookup for free, which is what
  makes `#match?` predicates cheap. Same structure ripgrep and tantivy use.
- **Trigram index over raw source** — `tantivy`, used as a *prefilter* only.
  Structural queries almost always carry a literal (a function name, a
  string), and a trigram intersection cuts the candidate file set by orders
  of magnitude before any structural work happens.
- **Doc store** — `zstd`-compressed blocks, `memmap2` for zero-copy reads,
  `rkyv` for the record arrays so a segment loads by pointer cast.
- **Metadata** — `redb`. Pure rust, single file, no compaction surprises.
- **Parallelism** — `rayon`, as the rest of the repo already does. Note the
  64 MiB stack pool in `main.rs` exists for exactly this kind of recursive
  tree work.

## 5. The query language

Tree-sitter's own S-expression query syntax, because it already exists, the
`tree-sitter` crate already implements it, and users already know it. Two
extensions:

    ; works in all five languages, unchanged
    (_invocation function: (_name) @f
      (#match? @f "^(execute|executemany)$"))

    ; facets compose with the table tier
    (_declaration (_callable) @inner)

    ; language-scoped when you need it
    ((call_expression) @c (#lang? python))

The point of the first example is that it is one query, not five. That is
the whole product.

## 6. Execution

1. **Plan.** Order the pattern's terms by selectivity. The estimator is
   free: `corpus/<lang>/reports/kinds.json` already carries per-kind counts
   *and* per-`(parent, field, child)` edge counts over the real corpus. A
   planner that knows `raise_statement cause: dictionary` never occurs can
   discard that branch before touching a posting list. The edge table built
   for coverage measurement turns out to be a cardinality estimator.
2. **Prefilter.** Intersect trigram postings for any literal in the query.
3. **Structural join.** TwigStack over the interval-labelled lists. Linear
   in the input postings for ancestor-descendant-only patterns, which is
   what most code queries are.
4. **Verify.** Re-parse the surviving files and run the real tree-sitter
   `Query` against them.

Step 4 is not a fallback, it is the correctness argument. The index is
allowed to be approximate — lossy hashes, over-broad supertype bits, a
stale segment — because the parser is the arbiter and it runs on a candidate
set small enough to afford. This is exactly zoekt's trigram-then-regex
contract, and it is what lets the index be fast without being trusted.

## 7. Ranking

Structural queries return fewer, better-typed results than lexical ones, so
ranking matters less than it does for text — but the signals available are
unusually good:

- **definition over reference** — the `_declaration` bit is already in the
  record, and a hit on a declaration is nearly always what was wanted
- **specificity** — a match binding four captures beats one binding one
- **centrality** — in-degree in the call graph, computed at index time from
  the same edge records
- **BM25** on the enclosing declaration's text, via tantivy, as a tiebreak

## 8. The problems this design has

Listed because they are the parts that would actually decide whether it
works.

**Grammar version skew.** A grammar change invalidates every segment built
against it. Segment metadata must carry the grammar sha *and* the
`vocabulary.json` version (§3.2.1), and a mismatch must force a rebuild of
that segment rather than silently serving stale structure. This is the
maintenance cost of owning the grammars, and it is real.

**Per-language confidence is not uniform, and the engine must say so.**
The sweep numbers are the honest statement of how much of each corpus we
parse cleanly. Python and java are at four nines; bash is not. An index that
silently drops the files it failed to parse reports "no results" for a query
whose answer sits in a file we could not read. Results need a per-language
coverage figure attached, and unparsed files need to fall through to the
lexical index rather than vanishing.

**Error trees.** Related and sharper: tree-sitter salvages nodes from a
failed parse, and indexing those puts structurally wrong records in the
posting lists. The `kinds` measurement hit this exact bug — counting
error-tree nodes inflated bash's coverage from 49% to 83%. The index must
take clean parses only, for the same reason.

**Scale.** Five languages is a demo, not an engine. The path to more is the
same path this repo is already on, and it is slow by construction.

## 9. Smallest thing worth building

Skip segments, skip merge, skip ranking. Index one language into a single
immutable file, implement ancestor-descendant joins only, and verify with
the real parser. That is enough to answer the question the whole design
rests on: **does one supertype query return the right thing in five
languages at once?** If it does not, none of the rest is worth writing.
