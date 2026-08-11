# Treebank ↔ langbank: the overlap

Survey only. No behaviour is changed by this document, and nothing below has
been landed. Written against langbank `e803f05` (`origin/main`) and treebank
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
| D | Registry endpoints — crates.io, npm, PyPI, Maven Central, NuGet, Debian pool, sources.debian.org, ecosyste.ms | 16 URLs across `lang/*.rs` | **No** | **Contribute to langbank** as `data/package-registries/`; treebank keeps the fetching | No |
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

So the right relationship is **conformance, not delegation**. A test that asserts
`langbank::language_profile_for_extension(e) == expected_lang` for each of the
14 extensions treebank classifies:

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

**Correction to the brief's hazard section:** `lang/mod.rs` is not the only
contended file. `LangName` lives in `crates/treebank-cli/src/ledger.rs`, and all
five language sessions must add a variant *and* an `as_str()` arm there too. Any
change that adds a match arm over `LangName` — in either file — conflicts five
ways. The fallback-arm form above avoids that in both.

One open question for whoever lands this: `bash` maps to langbank's `shell`
(there is no `bash` profile — `.bash` and `.sh` both resolve to `shell`). That
is the tbbash session's call, not mine, but the id map is where it surfaces.

### D & E. Registries and archives — langbank does not carry these yet

Two of the brief's "where the overlap probably is" guesses do not survive
contact with the data:

- **There is no `data/registries/` in langbank.** `data/ecosystems/` exists —
  five entries, cargo/npm/pnpm/yarn/bun — and carries manifests, lockfiles,
  gitignore patterns, pin policy and traversal dirs. **It carries no URL.** The
  only `https://` in all of langbank's data is linguist's pinned source digest.
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

The concrete proposal is **`docs/langbank-package-registries.patch`** in this
directory — 839 lines against langbank `e803f05`, verified to apply cleanly and
to build, test, `fmt` and `clippy` clean there. It is not committed to langbank.
See "What would need to be added to langbank" below.

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

## What would need to be added to langbank

Written as a patch rather than a sketch, because a schema argues better when it
compiles. `docs/langbank-package-registries.patch` — apply with
`git apply` from a langbank checkout at `e803f05`.

### Two new registries, in langbank's existing shape

**`data/archives.toml`** — 10 archive shapes. What a registry *serves*, kept
separate from `artifacts.toml`, which is what a build *produces*. Four fields,
each one a fact a consumer cannot reliably guess from a filename:

| field | why it cannot be inferred |
|---|---|
| `container` | `.crate`, `.tgz`, `.gem` and `.orig.tar.xz` are all tar; `.nupkg`, `.jar` and `.whl` are all zip |
| `strip-root` | tarballs wrap entries in one directory, zips do not — stripping a `-sources.jar` drops the whole `com/` of a Java coordinate |
| `member` | a `.gem` is a tar whose payload is `data.tar.gz` *inside* it |
| `carries` | source, or build output. An sdist and a wheel are both Python packages and only one is the tree the author wrote |

**`data/package-registries/`** — 6 files: crates.io, npm, PyPI, Maven Central,
NuGet, Debian. Endpoints, the archives each serves, and where its popularity
number comes from.

A registry is a separate axis from an ecosystem, and the npm entry is the
argument for that: **four of langbank's five ecosystems — npm, pnpm, yarn, bun —
resolve against one registry.** Modelling the registry as a property of the
ecosystem would state that fact four times and let it drift four ways. This is
the same separation langbank already makes between a language and the ecosystem
that publishes it.

### The field that is worth more than the endpoints

```toml
[popularity]
source = "https://packages.ecosyste.ms/api/v1/registries/repo1.maven.org/packages"
publisher = "packages.ecosyste.ms"
first-party = false
metric = "dependent-repos"
```

Treebank ranks six languages by six numbers that are **not the same kind of
number**, and today that fact survives only as prose in four separate doc
comments:

| registry | metric | who publishes it |
|---|---|---|
| crates.io | downloads | crates.io — its own db dump |
| npm | downloads | wooorm/npm-high-impact (third party; npm has no top-N endpoint) |
| PyPI | downloads | hugovk/top-pypi-packages (third party; PyPI serves no counts) |
| Maven Central | **dependent repos** | packages.ecosyste.ms (third party; Central publishes no counts) |
| NuGet | downloads | NuGet's own search service |
| Debian | **installs** | popcon.debian.org |

Two of those six are not traffic at all. A consumer that ranks by
`dependent-repos` while calling it "downloads" is publishing a wrong claim, not
an imprecise one — and every fleet repo that ranks anything needs this
distinction, not just treebank. `first-party` is the second half of it: a
third-party index can lag, change shape, or stop, and four of six here are third
party.

### The other facts the patch carries

- **`source-availability = "source-link"`** on NuGet. A `.nupkg` ships
  assemblies — there is not one `.cs` file in any of the top twenty packages —
  and the repository and commit it was built from are in the `.nuspec`. That is
  why treebank's C# corpus is repository source rather than the published
  artifact, and it is a static fact about NuGet, not a treebank decision.
- **npm has no derivable download URL.** The tarball is `dist.tarball` in the
  metadata document, so `download-url` is `None` and `metadata-accept` records
  the abbreviated-metadata header. A schema that assumed every registry has a
  templatable download URL would be wrong about the largest one.
- **Debian claims no language.** A distribution ships all of them, and which one
  a source package actually contains is measured (`sloc-url`), not static. The
  `role` field keeps it in the registry with an honest label rather than forcing
  it into a package-registry shape.
- **`compression = ["gzip", "xz", "bzip2"]`** on Debian tarballs — the measured
  reason `fetch.rs` sniffs magic bytes instead of trusting a name.

### What treebank deletes when it lands

Precisely, and it is not much:

| file | what goes | lines |
|---|---|---|
| `lang/c.rs` | `POPCON`, `MIRROR`, `SOURCES` consts, two sources.debian.org URLs | ~7 |
| `lang/csharp.rs` | flat-container URL, search URL | ~6 |
| `lang/java.rs` | `CENTRAL`, metadata and sources-jar URLs, ecosyste.ms URL | ~6 |
| `lang/npm.rs` | two registry URLs, the Accept header | 3 |
| `lang/python.rs` | JSON API URL, top-pypi-packages URL | 2 |
| `lang/rust.rs` | static.crates.io URL | 3 |
| `scripts/bootstrap.sh` | `DUMP_URL` default | 1 |
| **total** | | **~28 lines, replaced by ~28 lookups** |

**839 lines added to langbank to delete 28 from treebank is not a line-count
argument, and it should not be sold as one.** The case is:

1. Three repos need these facts and exactly one has them. entl and propbank
   cannot reach treebank's `lang/*.rs` and should not have to.
2. The metric/first-party distinction stops being prose that only holds while
   someone re-reads four doc comments. `ledger.json` could then state a corpus's
   metric mechanically instead of by hand — which is the failure mode
   `ledger.rs` already exists to prevent.
3. Every language the five sibling sessions are adding needs a registry entry
   anyway. Go's module proxy, Packagist, RubyGems and the `.gem` shape are
   already designed for here — `gem` is in `archives.toml` with its nested
   `data.tar.gz` member, because treebank's own ROADMAP knows that fact and
   nothing else in the fleet does.

**What does not move, and is why the deletion is small:** every response parser
(`doc["info"]["version"]`, `<repository url=…>`, popcon's column layout,
`Sources.gz` stanza parsing), all caching, the SLOC threshold that decides a
Debian source is really C, the NuGet monorepo dedup, and `fetch.rs`'s magic-byte
sniffing. Endpoints are data; reading what comes back is not.

### Verified, not asserted

Everything above was run, in a throwaway copy of langbank — the `langbank/`
worktree was never written to:

```
cargo build         clean
cargo test          45 tests pass (8 new, in tests/package_registries.rs)
cargo fmt --check   clean
cargo clippy        clean, under langbank's unwrap/expect/panic/print denials
git apply --check   applies cleanly to langbank e803f05
```

A consumer crate outside langbank was then pointed at the result and made to
print what treebank's `rank`/`resolve` would look up for all six languages,
including `c-sharp` resolving to NuGet and NuGet correctly reporting that it
serves no source archive at all.

### Three decisions that are langbank's owner's to make

1. **The directory is `data/package-registries/`, not `data/registries/`.**
   "Registries" is already langbank's word for its own inventory registries —
   `OUT_DIR/registries.rs`, `tests/generated_registries.rs`, "the registries are
   the expected size". A second meaning in the same tree would be confusing in
   exactly the place clarity is cheapest. Easy to rename if the owner disagrees.
2. **Are URL templates data?** They are strings with `{name}`-shaped holes, and
   nothing in langbank substitutes into them or opens a socket — the same
   standing as the pinned linguist URL already in `data/sources/`. But it is the
   closest this crate comes to describing *how to fetch*, and it is worth an
   explicit yes rather than an assumed one.
3. **How much of a coordinate to model.** Maven's `{group}:{artifact}` and
   NuGet's lowercased ids are in; npm's scope separator and treebank's
   `pkg_dir()` sanitisation are not. That line is drawn where a fact stops
   describing the registry and starts describing what a consumer does about it,
   and it is a judgement rather than a rule.

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
  dependency pinned to a rev — `e803f05` today — and treebank's `Cargo.lock`
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
   `docs/langbank-package-registries.patch`; it needs a decision from langbank's
   owner and a merge there before treebank can look anything up. Blocked on
   langbank, not on the five branches.

The honest summary of the whole survey: **the overlap is real but shallow.**
The biggest-looking overlap (extensions) is one treebank must not take, because
the two repos are answering different questions; the overlaps where langbank
should plainly win (registry endpoints, archive shapes) are ones langbank does
not carry yet. What is available today is one duplicate fact worth deleting, one
vocabulary worth binding to, and one data contribution worth writing up.
