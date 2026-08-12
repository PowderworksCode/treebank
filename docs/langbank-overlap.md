# Treebank ↔ langbank: the overlap

Survey only. No behaviour is changed by this document, and nothing below has
been landed. Surveyed against langbank `e803f05`; the proposal in it is rebased
onto `355a7be`, which is `origin/main` since langbank#7 merged. Treebank side is
`use-langbank` off `origin/main`.

## The boundary, as both repos state it

Langbank: *"a registry over static data plus the few functions needed to look
something up. Nothing here walks a filesystem, spawns a process, or parses a
source file."*

Treebank: grammars, corpora, oracles, sweeps — everything that requires running
something.

The test applied to each row below is therefore not "could langbank hold this"
but "is this a static fact, and does treebank's copy of it mean the same thing
as langbank's". Those come apart more often than expected: the largest overlap
by volume — extensions — is a case where the two repos are answering *different
questions* with the same-looking table.

## The table

| # | What treebank knows | Where | Langbank knows it? | Recommendation | Touches `lang/mod.rs`? |
|---|---|---|---|---|---|
| A | Which extensions belong to a language's corpus | `classify()` in each `lang/*.rs` | **Partly — same answers, wider sets** | **Assert against langbank, do not adopt it.** Add a conformance test. | No |
| B | Corpus exclusions: `.min.js`, `_vendor/`, C++ `.h` | `classify()`/`admit()` | No | Stays in treebank — corpus policy, and the C case needs file content | No |
| C | Canonical language spelling (`csharp`) | `LangName`, `ledger.rs` | **Conflicts — langbank says `c-sharp`** | Keep `LangName`; add a fallback-arm id map in a **new file** | No (but see contention note) |
| D | Registry endpoints — crates.io, npm, PyPI, Maven Central, NuGet, Debian pool, sources.debian.org, ecosyste.ms | 16 URLs across `lang/*.rs` | **No** | **Contribute to langbank** as fetch fields on its purl registries; treebank keeps the fetching | No |
| E | Archive shapes: which cache extension, zip-vs-tar, strip-root-or-not | `fetch.rs` | **No — `artifacts.toml` is a different concept** | Contribute as `data/archives.toml`; **do not rewire `extract()`** | No |
| F | C comment/string lexical syntax | `strip_comments_and_strings()` in `c.rs` | **Yes, exactly** | **Look up the table, keep the scanner** | No |
| G | npm serves both JavaScript and TypeScript corpora | `lang/npm.rs` | Partly — npm implies `javascript` only | No action; langbank's "implied language" answers a different question | No |
| H | What a popularity number *means* (traffic / installs / dependent repos) | doc comments + ledger prose | No | Contributed with D — it is the most valuable field in the patch | No |
| I | Include-path policy, `__cplusplus` undefined, oracle invocation, grammar dirs, dialect routing, SLOC thresholds | `c.rs`, `lang/*.rs` | No | **Must not move.** Requires running something | — |

## Detail, where the judgement is not obvious

### A. Extensions — the big one, and the one not to take

Every extension `classify()` accepts resolves through langbank to exactly the
language treebank means. Verified against the real crate, not the TOML:

```
.rs   -> rust         claimants: renderscript, rust, xml
.ts   -> typescript   claimants: typescript, xml
.tsx  -> typescript   claimants: tsx, typescript, xml
.cs   -> c-sharp      claimants: c-sharp, smalltalk
.h    -> c            claimants: c, cpp, objective-c
.py   -> python       claimants: python
```

Four of those are contested tokens that langbank settles by a `primary-extensions`
declaration. Treebank agrees with every one of its verdicts today, and does not
know that it does.

What treebank must **not** take is the reverse direction. The sets are not the
same size:

| language | treebank `classify()` | langbank `extensions` |
|---|---|---|
| rust | 1 (`rs`) | 2 |
| typescript | 4 | 4 |
| javascript | 4 | 25 |
| java | 1 | 3 |
| csharp | 1 | 5 |
| c | 2 (`c`, `h`) | 5 |
| python | 1 (`py`) | 17 |
| php | 1 (`php`) | 10 |

Adopting langbank's sets would add `.pyi`, `.spec`, `.wsgi`, `.gyp`, `.cgi` to
the Python corpus and `.es6`, `.jsm`, `.pac`, `.jss` to JavaScript's. Every
sweep number moves, and — worse — they move for a reason nobody asked for. That
is precisely the "changed what treebank means by *this file is Python*" failure
the brief prohibits.

The two tables answer different questions, and both are right:

- **langbank** answers *what language is this file* — as broadly as the world
  writes it.
- **treebank's `classify()`** answers *does this file belong in the corpus for
  this grammar* — which is deliberately narrowed to what the grammar advertises
  it parses. `python.rs` says so outright: `.pyi` is "left out for now so
  `classify()` matches what the grammar advertises, and adding them is a
  deliberate change with its own sweep evidence rather than a silent widening."

PHP, which landed mid-survey, is the eighth language and fits the pattern
exactly: `classify()` takes `.php` alone where langbank claims ten, and excludes
`vendor/` for the attribution reason in row B. langbank resolves `.php` to `php`
over Hack's competing claim, so it agrees here too — one more verdict treebank
depends on and does not know it depends on.

So the right relationship is **conformance, not delegation**. A test that asserts
`langbank::language_profile_for_extension(e) == expected_lang` for each of the
15 extensions treebank classifies:

- costs zero behaviour change (it is a test),
- pins treebank's corpus vocabulary to the fleet's,
- and fires if langbank ever re-settles a contested token — if someone declares
  `.rs` primary for RenderScript, or `.cs` for Smalltalk, treebank finds out from
  a red test rather than from a corpus that quietly changed meaning.

That is the one place where "langbank should win" is true *and* free.

### C. `csharp` vs `c-sharp` — and a correction to the hazard note

Langbank's id for C# is `c-sharp`; treebank's `LangName` is `csharp`. This is
the same string that, per `ledger.rs`'s own header comment, once broke every
automated path for that grammar and sat broken for days.

Any langbank lookup therefore needs an id map. It should be written so that
adding a language does **not** require editing it:

```rust
// new file, e.g. lang/langbank_id.rs — one arm, plus identity
pub fn langbank_id(name: LangName) -> &'static str {
    match name.as_str() {
        "csharp" => "c-sharp",
        other => other,   // go, ruby, bash→shell?, php, lua all land here
    }
}
```

**Confirmed since:** PHP merged to `main` (#25) while this was being written,
and its commit touches both `lang/mod.rs` *and* `ledger.rs` — a `LangName`
variant, an `as_str()` arm, a `get()` arm and a `mod` line. Four of the five
language sessions are still to land.

**Correction to the brief's hazard section:** `lang/mod.rs` is not the only
contended file. `LangName` lives in `crates/treebank-cli/src/ledger.rs`, and all
five language sessions must add a variant *and* an `as_str()` arm there too. Any
change that adds a match arm over `LangName` — in either file — conflicts five
ways. The fallback-arm form above avoids that in both.

One open question for whoever lands this: `bash` maps to langbank's `shell`
(there is no `bash` profile — `.bash` and `.sh` both resolve to `shell`). That
is the tbbash session's call, not mine, but the id map is where it surfaces.

### D & E. Registries and archives — what langbank carries, and what it does not

Two of the brief's "where the overlap probably is" guesses do not survive
contact with the data:

- ~~**There is no `data/registries/` in langbank.**~~ **Retracted — see "Rebased
  onto langbank #7" below.** True of `origin/main` at `e803f05`, and wrong about
  where langbank was going: an open PR added exactly that directory and has
  since merged. The brief described the in-flight state and I surveyed the
  merged one.
- **`data/artifacts.toml` is not archive shapes.** It is what a *build produces*
  — `binary`, `napi`, `site`, `tauri` — used to say what a tool's command
  emits. A `.gem` being a tar containing `data.tar.gz` has no home in it, and
  putting one there would conflate "what a build outputs" with "how a registry
  packages a download".

So D and E are not "look it up in langbank"; they are **contributions to
langbank**, which is exactly where langbank's own README points:

> **Direction, item 3:** "Absorb treebank's registry data — crates.io dumps,
> npm, Maven Central, NuGet, Debian popcon, `packages.ecosyste.ms` — which is
> `rank`/`resolve` today and is plainly data."

`data/artifacts.toml` still is not archive shapes, and #7 did not change that:
langbank carries registries now, and carries nothing about what a download
arrives in. The concrete proposal is **`docs/langbank-registry-fetch.patch`** —
692 lines against langbank `355a7be`, verified to apply cleanly and to build,
test, `fmt` and `clippy` clean there. It is not committed to langbank.

### F. Comment syntax — the one real duplicate

`c.rs::strip_comments_and_strings()` hardcodes C's lexical syntax to blank
comments and strings before scanning a header for C++ markers. Langbank's table
for `c` is identical, checked at runtime:

```
line=["//"]  block=[("/*", "*/")]  quotes=['"', '\'']  multi=[]
```

This is a true duplicate of a static fact, it is contained entirely within
`c.rs`, and it touches nothing the five sessions are editing. `CXX_MARKERS`
(`namespace `, `template<`, `public:` …) is **not** comment syntax — it is C++
dialect discrimination and stays in treebank.

Caveat that governs how it lands: this function feeds `looks_like_cxx()` →
`admit()` → corpus membership. It drops 365 of 12,767 headers today. A
langbank-driven rewrite must reproduce that count exactly before it lands, or it
is a behaviour change wearing a refactor's clothes.

## Rebased onto langbank #7

**[langbank#7 `purl-registries`](https://github.com/PowderworksCode/langbank/pull/7)
merged as `355a7be`.** It did the structural half of what this survey proposed,
from the other end: it split registry from ecosystem via package *identity*
(purl, SBOM tooling) where this survey reached the same split via package
*fetching*. It carries `data/registries/` with all 42 purl types, a
`PackageRegistry` struct, and a `registry` pointer on `EcosystemProfile`.

It was open before this survey began and I did not look for it — I read
`origin/main` and not the open PRs, in a week whose entire hazard is that
everything is in flight. The earlier draft of this document "corrected" the
brief on `data/registries/`; the brief was right and the correction is
withdrawn.

The contribution is therefore a **follow-up on top of #7**, not a parallel
model: `docs/langbank-registry-fetch.patch`, 692 lines against `355a7be`.

### What #7 settled, and what it left

| fact | #7 | this |
|---|---|---|
| registry as its own axis, 42 purl types | **yes** | — |
| ecosystem → registry pointer | **yes** | — |
| namespace/name/version rules and case sensitivity | **yes** | — |
| kept current against upstream in CI (`sync-purl.py`) | **yes** | — |
| canonical host (`default-repository`) | **yes** | — |
| archive shapes | no | **yes** |
| popularity source, metric, first-party | no | **yes** |
| fetchable endpoints | no | **yes** |
| source availability (NuGet SourceLink) | no | **yes** |
| language → registry | no | **yes** |

Three fields the pre-#7 draft carried are **dropped**, because #7 makes them
unnecessary or wrong:

- `coordinate = "{group}:{artifact}"` — #7 already says Maven's namespace is
  required. The `group:artifact` spelling is a treebank-side detail.
- `name-case = "lower"` for NuGet — #7 says NuGet names are case-sensitive,
  which is true of *identity*. Lowercasing is a property of one URL, so
  treebank keeps its `to_lowercase()` rather than langbank asserting two
  contradictory-looking things about case.
- `sloc-url` for Debian — sources.debian.org's per-language SLOC endpoint is
  how treebank *measures* whether a package is really C. Measurement belongs to
  whoever measures; it stays in `c.rs`.

`RegistryRole` is dropped too: `default_repository` and an empty `languages`
already say what "Debian is a distribution" needed to say.

### The thing #7 makes visible: an endpoint is not a host

purl's `default-repository` is where a package is *named*. The host that serves
the artifacts is routinely a different one, which is why these endpoints are
carried whole rather than as paths appended to a repository:

| registry | `default-repository` (#7) | where the artifact actually is |
|---|---|---|
| maven | `repo.maven.apache.org/maven2/` | `repo1.maven.org/maven2/` |
| nuget | `www.nuget.org` | `api.nuget.org/v3-flatcontainer/` |
| npm | `registry.npmjs.org/` | **not derivable** — `dist.tarball` inside the metadata |

That last row is the load-bearing one. A schema that assumed every registry has
a templatable download URL would be wrong about the largest registry there is.

### The five sibling languages get their ids for free

Because #7 followed purl, the registries the other sessions need already exist
with the right ids — `golang`, `gem`, `composer`, `luarocks` — carrying identity
rules and, where purl knows one, a default repository. What none of them carries
yet is the fetch half, which is exactly the per-language work those sessions are
doing. `archives.toml` already holds `gem` with its nested `data.tar.gz` member,
because treebank's ROADMAP knows that fact and nothing else in the fleet does.

### What treebank deletes when it lands

| file | what goes | lines |
|---|---|---|
| `lang/c.rs` | `POPCON`, `MIRROR`, `SOURCES` consts | ~5 |
| `lang/csharp.rs` | flat-container URL, search URL | ~6 |
| `lang/java.rs` | `CENTRAL`, metadata and sources-jar URLs, ecosyste.ms URL | ~6 |
| `lang/npm.rs` | two registry URLs, the Accept header | 3 |
| `lang/python.rs` | JSON API URL, top-pypi-packages URL | 2 |
| `lang/rust.rs` | static.crates.io URL | 3 |
| `scripts/bootstrap.sh` | `DUMP_URL` default | 1 |
| **total** | | **~26 lines, replaced by ~26 lookups** |

Down from the pre-#7 estimate of 28, because the two sources.debian.org URLs
now stay in `c.rs` on purpose.

**692 lines added to langbank to delete 26 from treebank is not a line-count
argument and is not offered as one.** The case is that three repos need these
facts and one has them; that `ledger.json` could then state a corpus's metric
mechanically rather than by hand, which is the failure mode `ledger.rs` exists
to prevent; and that the metric distinction — two of six numbers are installs
and dependent-repos rather than traffic, and four of six come from a third
party — survives today only as prose in four doc comments.

**What does not move:** every response parser (`doc["info"]["version"]`,
`<repository url=…>`, popcon's columns, `Sources.gz` stanzas), all caching, the
SLOC threshold and the endpoint behind it, the NuGet monorepo dedup, and
`fetch.rs`'s magic-byte sniffing. Endpoints are data; reading what comes back is
not.

### Verified, not asserted

Everything above was run, in a throwaway copy of langbank at `355a7be` — the
`langbank/` worktree was never written to:

```
cargo build         clean
cargo test          50 tests pass (8 new, in tests/registries_fetch.rs)
cargo fmt --check   clean
cargo clippy        clean, under langbank's unwrap/expect/panic/print denials
git apply --check   applies cleanly to langbank 355a7be
```

One of the new tests exists only to prove the follow-up is additive: all 42
purl registries still resolve with no language, no archive and no popularity
source declared. Only the six treebank fetches from say anything.

A consumer crate outside langbank was then pointed at the result and made to
print what treebank's `rank`/`resolve` would look up for all six languages,
including `c-sharp` resolving to NuGet and NuGet correctly reporting that it
serves no source archive at all.

### Two decisions that are langbank's owner's to make

*(The directory question is answered by #7: `data/registries/`, theirs.)*

1. **Are URL templates data?** They are strings with `{name}`-shaped holes, and
   nothing in langbank substitutes into them or opens a socket — the same
   standing as the pinned linguist URL already in `data/sources/`. But it is the
   closest this crate comes to describing *how to fetch*, and it is worth an
   explicit yes rather than an assumed one.
2. **Whether `languages` belongs on a registry at all.** #7 reaches a language
   through the ecosystem, but PyPI, Maven, NuGet and Debian have no ecosystem in
   langbank, so `python -> pypi` is otherwise underivable. The alternative is an
   ecosystem per registry, which would be inventing managers that do not exist
   here to carry one edge.

## What must not move, and is not being proposed

Listed because a reader six months from now will wonder whether it was
considered:

- Oracle invocation and its three-valued verdict collapse (`c.rs`), include-path
  policy (`-iquote` over `-I`, measured), preprocessing symbols
  (`__cplusplus` undefined), grammar dirs and dialect routing, the SLOC
  thresholds that decide a Debian source is "really C", the seen-sources
  dedup for NuGet monorepos. All of these require running something, or are
  facts about a grammar rather than about a language.
- `classify()`'s exclusions (B). `_vendor/` and `.min.js` are excluded for an
  attribution reason — the same code is already in the corpus under the package
  that owns it — not because those files are not Python or JavaScript. Langbank
  would be wrong to hold that, and its `traversal_directories` registry
  (`node_modules`, `dist`, `target`, …) is about *generated output during a
  walk*, which is a third distinct question.

## Cost of the dependency itself

- Langbank is **not published** (`version = "0.0.0"`). Consuming it means a git
  dependency pinned to a rev — `355a7be` today — and treebank's `Cargo.lock`
  gains langbank plus `inventory`; `toml` enters as a build-dependency of
  langbank only.
- Langbank is `edition = "2024"`, `rust-version = "1.85"`. Treebank has no
  `rust-toolchain.toml` and builds on the ambient 1.96.1. **Verified: langbank
  compiles and its full public API works from a 2021-edition consumer** — every
  figure in this document was produced by a throwaway crate doing exactly that.
- A git dependency means the daily cron needs network to build, not just to
  fetch corpora. Worth a sentence in `bootstrap.sh` if this lands.

## Sequencing

Nothing recommended above touches `lang/mod.rs`, and the one thing that would
have (an id map as a `match` over `LangName`) is written specifically to avoid
both `mod.rs` and `ledger.rs`. So on the current reading, **none of this is
blocked by the five language branches** — which is the outcome the hazard
section was hoping for.

Ordered by value per unit of risk:

1. **A — the conformance test.** Zero behaviour change, new file only, catches a
   real class of future breakage. Do this first.
2. **F — comment syntax lookup in `c.rs`.** Contained, deletes a genuine
   duplicate, must be proven against the 365-header count.
3. **D/E/H — the langbank contribution.** Written and verified as
   `docs/langbank-registry-fetch.patch`, rebased onto the merged #7; it needs a
   review and a merge in langbank before treebank can look anything up. Blocked
   on langbank, not on the five branches.

The honest summary of the whole survey: **the overlap is real but shallow.**
The biggest-looking overlap (extensions) is one treebank must not take, because
the two repos are answering different questions; the overlaps where langbank
should plainly win (registry endpoints, archive shapes) are ones langbank did
not carry when this began, and now half-carries: #7 brought the registries, and
the fetch half is the follow-up written here. What is available today is one duplicate fact worth deleting, one
vocabulary worth binding to, and one data contribution worth writing up.
