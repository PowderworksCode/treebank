# Scale and language roadmap

What it costs to add a language, which twenty to add next, which hundred
after that, and what in the CI and publishing pipeline breaks on the way.

Every number below was measured on one machine (16 cores, Ubuntu 24.04,
2026-08-09) unless it says otherwise. §8 says how to reproduce each one.
Everything the pipeline does today is marked **implemented**; everything
this document only argues for is marked **proposed**.

---

## 1. The short version

**The oracle is the binding constraint, but not because of what it costs to
run.** A reference parser costs about a second per thousand files. It is
binding because for some languages a per-file reference parser does not
exist, and no amount of money makes one.

That splits every candidate language into three tiers, and the tiers — not
grammar popularity — are what the ranking is built on. Grammar maintenance
is the second axis, now measured across all 307 grammar repos (§3): it does
**not** move the top 20, but 21 of the top 100 have had no upstream push in
over a year.

| Tier | What the oracle can say | Measured cost / 1000 files | Gap counts are |
|---|---|---|---|
| **A** | valid / invalid, per file, no project context | **0.2 – 2 s** | exact |
| **B** | valid / invalid / **indeterminate** — needs an include or config environment | **35 s (C), 1068 s (C++)** | a floor |
| **C** | nothing usable: no per-file parser, or the parser executes the corpus | — | impossible |

Tier A is 70× to 5000× cheaper than Tier B, and the gap is not a tuning
problem — it is the difference between a language that can be judged one
file at a time and one that cannot.

Where the ceiling lands: 266 languages are shipped by at least two of
nvim-treesitter, Helix and Zed, and 143 by all three. Of the top 100 of
those, **93 have a Tier-A oracle** and 3 more are Tier B. So a hundred languages is reachable —
but the back third of the hundred is config and markup formats rather than
programming languages, and several well-known languages (C++, Perl, MATLAB,
Apex, TeX) are either floor-only or excluded outright. §5 has the table.

---

## 2. What an oracle costs, measured

### The five that exist, plus C

| Lang | Reference parser | Had to install | Footprint | Per-file? | **s / 1000 files** |
|---|---|---|---|---|---|
| rust | `syn::parse_file` | nothing (cargo dep) | 0 | yes | **1.28** serial / 0.33 on 16 cores |
| javascript | V8 via `node:vm` + `@babel/parser` | Node 22 | 204 MB + 5.3 MB | yes | **0.21** |
| typescript | `ts.createSourceFile` | Node 22 | 204 MB + 23 MB | yes | **0.57** |
| java | `JavacTask.parse()` | **JDK** 21 (a JRE fails) | 286 MB | yes | **1.63** |
| csharp | Roslyn `CSharpSyntaxTree.ParseText` | **.NET SDK** 8 | 479 MB + 116 MB NuGet | yes | **1.04** |
| c | libclang, category rule | libclang-20-dev | ~200 MB | **no — see below** | **35.5** |

Setup: `npm ci` 0.44 s / 0.48 s from a cold cache; `dotnet build` 3.4 s;
the Java oracle needs no build step at all (JDK single-file source
launcher). C# is the only one that must be compiled before first use.

**All six run parse-only, with no project context.** That is not luck — every
implementation says so deliberately (`-proc:none`, `ParseText` with no
`Compilation`, `createSourceFile` not `transpileModule`, `SourceTextModule`
construction without link or evaluate). It was verified rather than trusted:
1000 Java files with no classpath, 1000 C# files with no `.csproj`, and 1000
TypeScript files with no `tsconfig`, all ripped out of their source trees,
returned 1000, 999 and 1000 valid. All six correctly reject hand-written
garbage.

### The reject path is the one that matters

`validate()` is only ever called on files the grammar already failed
(`sweep.rs:218`), so its real input skews invalid. Measured by truncating
each sample to 60% of its length:

| Lang | 1000 truncated files | vs. the valid path |
|---|---|---|
| javascript | 0.74 s | **10.6× slower** — pays the second V8 mode *and* the babel leg |
| csharp | 0.74 s | 2.5× slower |
| typescript | 0.50 s | 2.5× slower |
| java | 1.50 s | 2.0× slower |
| rust | 0.21 s | **2.8× faster** — `syn` bails at the first error |

Worth knowing when sizing a bad day, but nothing here changes a decision.

### Six more, measured for this document

| Lang | Oracle | Verified property | **s / 1000** |
|---|---|---|---|
| python | `compile(…, 'exec')` | 1000 files, no package context, 0 false rejects | **1.23** |
| go | `go/parser.ParseFile` (`SkipObjectResolution`) | 790 files, no package context, 0 false rejects | **1.94** |
| ruby | `RubyVM::AbstractSyntaxTree.parse` | 441 files, 0 false rejects | **0.29** |
| lua | `luac -p` | missing `require` is not an error | **1.7** |
| bash | `bash -n` | does not execute; `source /absent/file` still valid | **3.6** → **2.4** |
| php | `php -l` | **does not execute**; missing class is not an error | **18.3** → **0.71** |

PHP is the interesting one. `php -l` has no batch mode, so it forks an
interpreter per file: 18.3 ms each, 18.3 s per thousand — 20–90× worse than
every other Tier-A oracle. Running it under `xargs -P16` takes it to
**0.71 s per thousand**, a 25× speedup. This generalizes: an oracle that
must fork per file is not disqualified, it just has to be parallelized. Only
the Rust oracle is parallel today.

Bash belongs in that class too, which the 3.6 above did not say. Re-measured
by the bash session on its own machine over 963 real shell scripts: **2.4 s
per thousand**, decomposed as 0.7 ms of bare process spawn, 1.6 ms to start
bash at all, 2.0 ms for `bash -n` on an *empty* file, and only **~0.4 ms of
actual parsing** — 83% of the cost is the fork. Both figures are therefore
mostly a measurement of `fork+exec` on their own hardware, and the part that
belongs to bash agrees. There is no batch escape (`set -n` inside a
long-lived shell stops it executing the `source` that would read the next
file), so the php lever is the answer: at `-P16` the oracle runs at
**0.12 s per thousand**. See `crates/treebank-bash/ORACLE.md`.

### Where Tier A ends

The C session's `crates/treebank-c/ORACLE.md` is the document that
establishes the boundary, and its finding drives this whole ranking. C has
no notion of per-file validity: `foo * bar;` is a declaration or a
multiplication depending on a typedef that arrives through `#include`. The
oracle therefore returns a **third verdict, `indeterminate`**, and
`gap_files` becomes a floor rather than a count. On its 20-package Debian
pilot, indeterminate outnumbered valid **2.2 : 1** (11,983 vs 5,502).

Measured here, running that same oracle over 1000 `.c` files from
postgres/curl/git/redis with their own include directories supplied:

| | C (1000 `.c`) | C++ (200 `.cc`/`.cpp`, scaled) |
|---|---|---|
| wall clock | **35.5 s** | **213 s → ~1068 s / 1000** |
| valid | 72.5% | 66.5% |
| **indeterminate** | **27.5%** | **33.5%** |

**C++ is 30× the cost of C and 1000× the cost of a Tier-A oracle, and a
third of its files cannot be adjudicated at all.** Both are parallelizable
(independent translation units), which would bring C to ~2.5 s and C++ to
~70 s per thousand on this machine — still 100× Tier A for C++.

And Tier C, proven rather than assumed. `perl -c`:

```
$ perl -c evil.pl
!! BEGIN BLOCK EXECUTED DURING -c !!      <-- corpus code ran
evil.pl syntax OK

$ perl -c needsmod.pl                      <-- syntactically perfect file
Can't locate Some/Module/... in @INC
BEGIN failed--compilation aborted at needsmod.pl line 1.
```

Two disqualifying failures in one tool: it **executes arbitrary code from
the corpus**, which is unacceptable in a pipeline whose input is downloaded
from a package registry, and it **fails on a missing dependency**, so it is
not per-file either. Perl is below the line and it is not close.

---

## 3. How the ranking is built

Four things are needed to add a language (`GRAMMARS.md`): a vendored
grammar, a `Lang` impl, a corpus ranking source, and a reference parser for
`validate()`. Each language below names **which of the four is the
blocker**.

Popularity is cross-referenced from four independent signals rather than
taken from one:

- **nvim-treesitter** — 323 parsers. The best usage proxy that exists. Its
  `tier` field turns out to be release hygiene (semver + WASM artifacts),
  not popularity: only 9 parsers have tier 1, and they include
  `editorconfig` and `xresources`. Used as membership, not as quality.
- **Helix** — 303 grammars in `languages.toml`.
- **Zed** — 1390 extensions, plus 11 languages bundled in the binary
  (`bash c cpp css go json python rust typescript yaml` + javascript). The
  bundled set is the strongest signal in the whole dataset: it is what an
  editor company paid to ship in the box.
- **crates.io and npm download counts** for `tree-sitter-*`. These
  disagree usefully — bash is #1 on npm (13.3 M/month) and #9 on crates.io;
  C++ is #6 on crates.io and #12 on npm.

198 languages are in both nvim-treesitter and Helix; 143 are in all three
editor sets, and 266 are in at least two. That is the pool the hundred comes
from.

### Grammar maintenance, measured

The first version of this document ranked on oracle and popularity and left
the grammar-health axis as an assertion. It is now measured: GitHub metadata
for **all 307 distinct grammar repositories** in the nvim-treesitter list,
as of 2026-08-10. The raw table is committed at
`docs/data/grammar-health-2026-08-10.tsv` so the ranking can be re-derived
or re-collected later rather than taken on trust.

| last push to the grammar repo | grammars |
|---|---|
| under 90 days | 102 |
| 90 days – 1 year | 96 |
| 1–2 years | 53 |
| over 2 years | 53 |
| archived outright | 3 (`djot`, `hack`, `systemtap`) |

**The top 20 survives this check.** Only `toml` is over a year stale (395
days, 17 stars, 5 open issues — a small grammar for a stable spec, so it is
a mild flag, not a blocker). Everything else in the twenty was pushed within
a year, most within three months: kotlin 7 d, scala and swift within a day,
erlang 9 d, elixir 20 d, php 20 d, lua 51 d. So the ranking did not move —
which is the useful result, since it was the axis with the least evidence
behind it.

**The back half of the hundred does not survive it.** 21 of the 100 have had
no upstream push in over a year:

| grammar | stale | stars | repo |
|---|---|---|---|
| scss | **1469 d** | 33 | `serenadeai/tree-sitter-scss` |
| wgsl | 911 d | 61 | `szebniok/tree-sitter-wgsl` |
| thrift | 841 d | 8 | `tree-sitter-grammars/tree-sitter-thrift` |
| capnp | 841 d | 5 | `tree-sitter-grammars/tree-sitter-capnp` |
| mermaid | 839 d | 46 | `monaqa/tree-sitter-mermaid` |
| graphql | 793 d | 33 | `bkegley/tree-sitter-graphql` |
| luau, puppet, kconfig | 595 d | 6–8 | `tree-sitter-grammars/*` |
| jsonnet | 576 d | 21 | `sourcegraph/tree-sitter-jsonnet` |
| glsl, odin, objc, starlark, kdl, bitbake, svelte, bicep, toml | 395–450 d | 10–50 | `tree-sitter-grammars/*` |
| purescript | 418 d | 23 | `postsolar/tree-sitter-purescript` |
| dockerfile | 368 d | 105 | `camdencheek/tree-sitter-dockerfile` |

`scss` is the one that should actually move: four years untouched, on a
33-star personal repo, while CSS's own grammar sits in the `tree-sitter` org
and was pushed 315 days ago. Its popularity (63 K npm/month) is not evidence
about the grammar behind it.

### The counter-intuitive finding: where a grammar lives predicts its health, backwards

| host | grammars | median days since push | >1y stale | archived |
|---|---|---|---|---|
| `tree-sitter/` (official) | 22 | **244** | 0 (0%) | 0 |
| `tree-sitter-grammars/` (community org) | 75 | **389** | **41 (55%)** | 0 |
| everything else (personal, vendor) | 210 | **170** | 66 (31%) | 3 |

`tree-sitter-grammars/` is the *worst*-maintained of the three, not the
best. More than half its grammars have had no push in a year, and its median
is the oldest of any host. It is where grammars are collected, not where
they are maintained — an adoption signal, not a health one.

The `tree-sitter/` org is the only host with **zero** grammars over a year
stale. Everything else is bimodal: a good median dragged by a long dead
tail, which is why a median alone is the wrong statistic and the >1y count
is quoted beside it.

**How this should feed the ranking:** treat `tree-sitter/` membership as a
positive, `tree-sitter-grammars/` membership as neutral-to-negative, and a
personal repo as requiring the staleness check individually. Do not treat
"it is in the tree-sitter-grammars org" as a quality signal, which is the
intuitive reading and is measurably wrong.

---

## 4. The next 20

Ordered so the early ones de-risk the later ones. Six languages are already
done and on `main` (rust, typescript, javascript, java, csharp, c),
so these are #7 through #26.

### Wave 1 — free wins that prove the new CI at scale (all Tier A, all verified here)

**1. Python** · crates.io 12.8 M · npm 4.1 M · all 3 editors
Oracle `compile(src, path, 'exec')`, **1.23 s/1000, measured**. PyPI sdists
ship source.
*Blocker: none.* *Teaches: nothing new — that is the point.* It is the
control that proves a 7th grammar flows through the derived matrix without
anyone editing a workflow. The largest language treebank does not cover.

**2. Go** · crates.io 10.6 M · npm 2.4 M · all 3 editors
Oracle `go/parser.ParseFile` with `SkipObjectResolution`, **1.94 s/1000,
measured, zero false rejects on 790 files with no package context**.
*Blocker: none.* *Teaches: the module-proxy corpus.* Go has no tarball
registry; `proxy.golang.org/<module>/@v/<ver>.zip` is an immutable zip per
version, which is a cleaner corpus source than anything already wired up.
Also introduces build-tag dialects (`//go:build`), the first case where a
file is legitimately excluded by content rather than by path.

**3. Ruby** · crates.io 5.5 M · npm 779 K · all 3 editors
Oracle `RubyVM::AbstractSyntaxTree.parse`, **0.29 s/1000, measured**.
*Blocker: none.* *Teaches: nested-archive packaging* — a `.gem` is a tar
containing `data.tar.gz` — and version-gated syntax, since the oracle's Ruby
version decides what is valid.

**4. Bash** · npm **13.3 M/month, the single most-downloaded tree-sitter
grammar on npm** · all 3 editors
Oracle `bash -n`, **2.4 s/1000 forked, re-measured; 0.12 s/1000 at -P16;
does not execute** — verified that `source /absent/file` and `rm -rf` in a
script are not run, along with command and process substitution, heredoc
bodies, `eval` and `BASH_ENV`.
*Blocker: **corpus source**, not oracle.* Bash has no package registry at
all. *Teaches: the artifact corpus.* This is the first language whose corpus
must come from artifacts (Debian packages, GitHub repos) rather than a
registry, which de-risks C and every config language later in the list. Do
it here, where the oracle is trivial, rather than discovering it under C.

### Wave 2 — new oracle shapes

**5. PHP** · crates.io 4.1 M · npm 615 K · all 3 editors
Oracle `php -l`, verified parse-only and non-executing.
*Blocker: none.* *Teaches: the fork-per-file oracle class and its fix.*
Measured 18.3 s/1000 serial → **0.71 s/1000 at `-P16`**. That parallel lever
is what makes lua, fish, awk, and a dozen more of the hundred affordable, so
it should be built into the generic oracle driver here. Corpus: Packagist
ships source.

**6. Lua** · crates.io 3.1 M · all 3 editors
Oracle `luac -p`, **1.7 ms/file, measured**.
*Blocker: **grammar/dialect**.* *Teaches: the oracle version is a dialect
choice.* Lua 5.1 / 5.2 / 5.3 / 5.4 / LuaJIT / Luau are genuinely different
syntaxes (`goto` is 5.2+, integer division 5.3+), so which `luac` is
installed decides verdicts. This generalizes `generate_cli` into an
`oracle: {tool, version, dialect}` ledger field — which C already needs for
libclang and `-std=gnu17`, and which Scala, Haskell and Zig all need later.
Building it here, on the cheapest language that needs it, is the point.

**7. C++** · crates.io 9.7 M · npm 600 K · all 3 editors · **Zed bundles it**
Oracle: libclang, the same tool C uses. **1068 s/1000 measured, 33.5%
indeterminate.**
*Blocker: **oracle cost and adjudicability**.* *Teaches: where the ceiling
actually is.* It is here at #7 not because it is cheap but because its
number must be known before ninety more languages are queued behind it. On
the measurements above, a 100-package C++ sweep is hours, not seconds, and a
third of the answers are "cannot say". Realistic options are a much smaller
corpus, a real `compile_commands.json` from an actual build, or accepting
that C++ coverage is sampled rather than swept. **This is a decision to take
deliberately, not to discover in month four.**

**8. Kotlin** · crates.io 1.85 M · npm 253 K · all 3 editors
Oracle: `kotlin-compiler-embeddable`'s PSI parser — JVM, so the java oracle's
pattern and the already-installed JDK both carry over.
*Blocker: **the grammar**.* This is the first language where the grammar,
not the oracle, is the problem: there are three live competitors
(`tree-sitter-kotlin`, `-ng`, `-sg`) with 386 K, 1.84 M and 1.85 M crates.io
downloads and no clear winner. *Teaches: how to choose between forks, and
how `ledger.json` should record that choice and its evidence.* Corpus is
free — Maven Central, already implemented for java.

### Wave 3 — where the oracle is a library, and "invalid" gets slippery

**9. JSON** · crates.io 3.9 M · npm 4.5 M · all 3 editors · **Zed bundles it**
*Blocker: none.* *Teaches: the negative control.* The grammar should be
perfect and the sweep should find nothing. A pipeline that reports gaps here
is broken, which makes JSON the cheapest end-to-end test of the whole loop.
Dialects (JSON5, JSONC) are the follow-on.

**10. YAML** · crates.io 3.4 M · npm 246 K · all 3 editors · **Zed bundles it**
*Blocker: **oracle authority**.* *Teaches: that no single reference parser is
authoritative.* YAML 1.1 and 1.2 differ, and libyaml, PyYAML, go-yaml and
snakeyaml genuinely disagree on real documents. Whichever is chosen has to
be declared in the ledger as a *position*, not a fact. First language where
"valid" is a choice.

**11. TOML** · crates.io 1.37 M · all 3 editors
Oracle: `toml`/`taplo`. *Blocker: none.* The clean version of #10 — one
spec, one well-tested parser. Put it after YAML so the ledger's oracle-
declaration format is already designed.

**12. HCL / Terraform** · crates.io 1.97 M · nvim + Helix
Oracle: `hclparse` (Go library) — reuses the Go toolchain from #2.
*Blocker: **corpus ranking source**.* The Terraform Registry has modules but
no download-count API comparable to crates.io. *Teaches: ranking when the
registry will not rank for you*, which is the same problem java solved via
ecosyste.ms and which most of the back half of the hundred will have.

**13. CSS** · crates.io 3.6 M · npm 245 K · all 3 editors · **Zed bundles it**
Oracle: `csstree` or `lightningcss`. *Blocker: **oracle rejection power**.*
*Teaches: the failure mode `GRAMMARS.md` warns about, deliberately.* CSS's
specification *mandates* error recovery, so almost nothing is invalid and
the oracle has near-zero rejection power — a `validate()` that says
everything is valid makes every grammar failure look like a gap. Meeting
this on purpose, on a language that matters, is better than meeting it by
accident. The negative corpus is what has to carry the weight here.

### Wave 4 — heavier SDKs, reusing toolchains already installed

**14. Swift** · crates.io 4.2 M · npm 241 K · all 3 editors
Oracle: `swift-syntax`, per-file, no project needed. *Blocker: **oracle
footprint** — a ~2 GB toolchain, four times the .NET SDK.* *Teaches: the
provisioning limit.* Corpus is SwiftPM, which is git-based, so C#'s
SourceLink pattern applies directly.

**15. Scala** · crates.io 3.8 M · npm 279 K · all 3 editors
Oracle: `scalameta`, per-file, JVM. Corpus: Maven Central, already
implemented. *Blocker: **dialect**.* Scala 2.13 and Scala 3 are different
languages and scalameta requires the dialect to be declared per file, with
nothing in the path to tell you which. *Teaches: dialect routing that
`classify()` cannot resolve from the filename* — the problem SQL has in its
extreme form at #20.

**16. Haskell** · crates.io 2.4 M · npm 95 K · all 3 editors
Oracle: `ghc-lib-parser`. *Blocker: **out-of-file configuration**.*
*Teaches: the second Tier-B-shaped case after C, in a milder form.* GHC's
parser behaviour depends on `LANGUAGE` extensions, and real projects set
many of them in the `.cabal` file rather than in the source. A file that
parses inside its project fails alone. Unlike C this is tractable — read the
cabal file, which the corpus contains — and doing it here builds the
"per-file parse plus package-level configuration" machinery cheaply.

**17. Elixir** · crates.io 3.0 M · all 3 editors
Oracle: `Code.string_to_quoted/2`, pure and per-file. Hex ships source.
*Blocker: none.* *Teaches: the BEAM toolchain*, which #18 then reuses.

**18. Erlang** · crates.io 299 K · all 3 editors
Oracle: `epp_dodger` — which exists *precisely* to parse without macro
expansion. *Blocker: none.* *Teaches: the preprocessor problem with a happy
ending.* Worth doing immediately after C for the contrast: the same hazard
that makes C Tier B is a solved problem in Erlang because the ecosystem
shipped a tool for it. That contrast is what to look for in every later
candidate.

### Wave 5 — the frontier

**19. Zig** · crates.io 692 K · all 3 editors
Oracle: `std.zig.Ast.parse`, in the standard library, per-file, fast.
*Blocker: **language instability**.* *Teaches: pinning an oracle to a moving
target.* Zig's syntax changes between 0.11/0.12/0.13/0.14, so the oracle
version is not a detail, it is the definition of valid. The `oracle` ledger
field from #6 gets its hardest test here.

**20. SQL** · crates.io 690 K · all 3 editors
Oracle: excellent, per dialect — `libpg_query`, `sqlite3_prepare`, MySQL's
own parser. *Blocker: **routing, and corpus source**.* *Teaches: that there
is no such language as SQL.* PostgreSQL, MySQL, SQLite, T-SQL and BigQuery
are different languages sharing an extension, and nothing in the file path
says which. There is also no SQL registry — the corpus is embedded inside
other languages' packages, so it has to be mined from corpora already
fetched. Last because it needs everything before it.

### Why this order

Each wave hands the next one something it needs. Wave 1 proves the derived
CI matrix on languages whose oracles cost nothing, and gets the
no-registry corpus problem solved on bash where it is cheap. Wave 2 builds
the two levers the back half depends on — oracle parallelism (php) and
oracle-version-as-dialect (lua) — and gets C++'s true cost on the record
before anything is committed to it. Wave 3 works out how to report honestly
when the oracle is weak, which is a documentation and negative-corpus
problem, not a code one. Waves 4 and 5 spend the levers.

---

## 5. The next 100

Ranked by editor breadth first (nvim-treesitter / Helix / Zed), then by the
higher of crates.io all-time and npm monthly downloads. **Oracle** is the
tier from §2: **A** per-file and cheap; **B** needs an environment and
returns indeterminate; **C** none viable. Tiers marked ✓ were measured or
verified in this document; the rest are named-but-unverified and are the
first thing to check before starting one.

The six already done are excluded. The top 20 are marked ★.

| # | Language | crates.io | npm/mo | ed | Oracle | Named oracle | Blocker |
|---|---|---|---|---|---|---|---|
| 1★ | python | 12.8 M | 4.1 M | 3 | **A** ✓ | `compile(…, 'exec')` | — |
| 2★ | go | 10.6 M | 2.4 M | 3 | **A** ✓ | `go/parser` | — |
| 3★ | bash | 8.9 M | 13.3 M | 3 | **A** ✓ | `bash -n` | corpus |
| 4★ | ruby | 5.5 M | 779 K | 3 | **A** ✓ | `RubyVM::AbstractSyntaxTree` | — |
| 5★ | php | 4.1 M | 615 K | 3 | **A** ✓ | `php -l` (parallelize) | — |
| 6★ | swift | 4.2 M | 241 K | 3 | A | `swift-syntax` | oracle size (~2 GB) |
| 7★ | json | 3.9 M | 4.5 M | 3 | A | any conformant parser | — |
| 8★ | scala | 3.8 M | 279 K | 3 | A | `scalameta` | dialect (2 vs 3) |
| 9★ | css | 3.6 M | 245 K | 3 | A | `csstree` | oracle has no rejection power |
| 10 | html | 3.5 M | 135 K | 3 | A | `html5ever`/`parse5` | recovery-by-spec; like css |
| 11★ | yaml | 3.4 M | 246 K | 3 | A | libyaml / PyYAML | no authoritative parser |
| 12★ | lua | 3.1 M | 23 K | 3 | **A** ✓ | `luac -p` | dialect (5.1–5.4/JIT/Luau) |
| 13★ | elixir | 3.0 M | 87 K | 3 | A | `Code.string_to_quoted/2` | — |
| 14★ | haskell | 2.4 M | 95 K | 3 | A | `ghc-lib-parser` | per-package `LANGUAGE` config |
| 15 | nix | 2.1 M | 2 K | 3 | A | `rnix` / `nix-instantiate --parse` | corpus (nixpkgs is one repo) |
| 16 | powershell | 2.0 M | 284 K | 3 | A | `Parser.ParseFile` (.NET, installed) | corpus (PSGallery ships scripts) |
| 17 | solidity | 2.0 M | 9 K | 3 | A | `solc --stop-after parsing` | version fan-out (0.4–0.8) |
| 18★ | c++ | 9.7 M | 600 K | 3 | **B** ✓ | libclang | **cost + 33.5% indeterminate** |
| 19★ | kotlin | 1.85 M | 253 K | 3 | A | `kotlin-compiler-embeddable` | **grammar**: 3 competing forks |
| 20★ | toml | 1.37 M | 1 K | 3 | A | `toml` / `taplo` | — |
| 21★ | hcl | 1.97 M | — | 2 | A | `hclparse` (Go) | ranking source |
| 22 | markdown | 1.18 M | 3 K | 2 | A | `cmark-gfm` | recovery-by-spec: nothing is invalid |
| 23★ | sql | 690 K | 6 K | 3 | A | `libpg_query`, `sqlite3_prepare` | **routing**: no such language as SQL |
| 24★ | zig | 692 K | 1 K | 3 | A | `std.zig.Ast.parse` | language still changing |
| 25 | dart | 654 K | 29 K | 3 | A | `package:analyzer` parse-only | oracle size (Dart SDK) |
| 26 | r | 640 K | 2 K | 3 | A | `parse(keep.source=)` | — |
| 27 | ocaml | 524 K | 12 K | 3 | A | `compiler-libs` Parse | 2 grammars (impl + intf) |
| 28 | xml | 518 K | — | 3 | A | libxml2 (well-formedness) | — |
| 29 | scss | 22 K | 63 K | 3 | A | `sass` / `lightningcss` | **grammar: 4 years stale**, 33-star personal repo |
| 30 | proto | 465 K | — | 3 | A | `protoc --descriptor_set_out` | syntax 2 vs 3 |
| 31 | julia | 305 K | 17 K | 3 | A | `JuliaSyntax.jl` | — |
| 32★ | erlang | 299 K | — | 3 | A | `epp_dodger` | — |
| 33 | make | 275 K | 2 K | 3 | A | `make -n --dry-run`? | **weak** — GNU make has no parse-only mode |
| 34 | svelte | 235 K | 18 K | 3 | A | `svelte/compiler` parse | embedded-language routing |
| 35 | graphql | 217 K | — | 3 | A | `graphql-js` parse | — |
| 36 | clojure | 215 K | — | 3 | A | `tools.reader` | reader macros |
| 37 | fsharp | 194 K | 1 K | 3 | A | FSharp.Compiler.Service | .NET SDK, already installed |
| 38 | groovy | 189 K | 10 K | 3 | A | Groovy `AstBuilder` | JDK, already installed |
| 39 | elm | 181 K | 2 K | 3 | A | `elm-syntax` | corpus (small registry) |
| 40 | fortran | 178 K | — | 3 | A | `gfortran -fsyntax-only` | fixed vs free form; `include` |
| 41 | dockerfile | 155 K | — | 3 | A | `buildkit` parser | corpus |
| 42 | pascal | 221 K | — | 3 | A | `fpc -s` | dialect (FPC/Delphi) |
| 43 | gdscript | 57 K | 17 K | 3 | A | Godot `--check-only` | needs the Godot binary |
| 44 | matlab | 35 K | 10 K | 3 | **C** | — | **proprietary; Octave ≠ MATLAB** |
| 45 | gleam | 119 K | — | 3 | A | `gleam` compiler parse | — |
| 46 | odin | 114 K | — | 3 | A | `odin check` | project-scoped, needs checking |
| 47 | perl | 111 K | — | 3 | **C** ✓ | — | **`perl -c` executes the corpus** |
| 48 | ini | 111 K | — | 3 | A | trivial | no single spec |
| 49 | vue | 29 K | 7 K | 3 | A | `@vue/compiler-sfc` | embedded routing |
| 50 | luau | 28 K | 6 K | 3 | A | `luau-analyze` | — |
| 51 | nickel | 100 K | — | 3 | A | `nickel` parse | tiny corpus |
| 52 | scheme | 73 K | — | 3 | A | any R7RS `read` | dialect sprawl |
| 53 | just | 72 K | — | 3 | A | `just --dump` | — |
| 54 | fish | 65 K | — | 3 | A | `fish -n` | corpus |
| 55 | glsl | 61 K | — | 3 | A | `glslangValidator` | version + stage pragmas |
| 56 | cmake | 512 K | — | 2 | A | `cmake -P` parse? | **weak** — no parse-only mode |
| 57 | regex | 547 K | 2 K | 2 | A | any engine's compile step | flavour fan-out |
| 58 | objc | 535 K | 27 K | 2 | **B** | libclang | same as C, plus frameworks |
| 59 | diff | 507 K | — | 2 | A | trivial | not really a language |
| 60 | jsdoc | 420 K | 6 K | 2 | A | `@babel/parser` comments | embedded only |
| 61 | erb | 697 K | — | 1 | A | `Erubi` + ruby | embedded routing |
| 62 | verilog | 134 K | 9 K | 2 | A | `verilator --lint-only` | project-scoped |
| 63 | systemverilog | 44 K | 3 K | 2 | A | `verilator --lint-only` | project-scoped |
| 64 | bicep | 51 K | 1 K | 3 | A | `bicep build` | .NET, installed |
| 65 | devicetree | 51 K | 2 K | 3 | A | `dtc -O dts` | `/include/` resolution |
| 66 | thrift | 47 K | — | 3 | A | `thrift --gen` parse | — |
| 67 | ada | 47 K | — | 3 | A | `gnatmake -gnats` | — |
| 68 | d | 45 K | — | 3 | A | `dmd -o- -c` | — |
| 69 | puppet | 41 K | — | 3 | A | `puppet parser validate` | — |
| 70 | vhdl | 35 K | — | 3 | A | `ghdl -s` | project-scoped libraries |
| 71 | starlark | 148 K | — | 2 | A | `starlark-go` parse | — |
| 72 | ql | 155 K | — | 2 | A | CodeQL CLI | proprietary-ish licence |
| 73 | asm | 129 K | — | 1 | A | `gas`/`nasm` | **dialect chaos**: att/intel/arm/… |
| 74 | apex | 116 K | — | 1 | **C** | — | **Salesforce-proprietary compiler** |
| 75 | elisp | 85 K | 1 K | 2 | A | `read` in batch Emacs | — |
| 76 | qmljs | 75 K | — | 2 | A | `qmllint` | Qt install |
| 77 | cuda | 46 K | 7 K | 1 | **B** | libclang / nvcc | as C++, worse |
| 78 | nginx | 19 K | 3 K | 3 | A | `nginx -t` | needs a valid full config |
| 79 | awk | — | 2 K | 3 | A | `awk -f /dev/null`? gawk `--lint` | **weak** — no clean parse-only |
| 80 | hare | 32 K | — | 3 | A | `hare parse` | tiny ecosystem |
| 81 | beancount | 31 K | — | 3 | A | `bean-check` | — |
| 82 | rst | 24 K | — | 3 | A | `docutils` | recovery-by-spec |
| 83 | kconfig | 15 K | — | 3 | A | `kconfiglib` | `source` resolution |
| 84 | templ | 14 K | — | 3 | A | `templ fmt` | embedded Go |
| 85 | wgsl | 10 K | — | 3 | A | `naga` | — |
| 86 | capnp | 9 K | — | 3 | A | `capnp compile` | import resolution |
| 87 | agda | 7 K | — | 3 | A | `agda --only-scope-checking` | very slow; project-scoped |
| 88 | kdl | 6 K | — | 3 | A | `kdl-rs` | — |
| 89 | bitbake | 5 K | — | 3 | A | bitbake parser | project-scoped |
| 90 | v | 5 K | — | 3 | A | `v -check-syntax` | — |
| 91 | prisma | 3 K | 426 | 3 | A | `prisma format` | — |
| 92 | jsonnet | — | — | 3 | A | `jsonnet fmt` | import resolution |
| 93 | nim | — | — | 3 | A | `nim check` | project-scoped |
| 94 | purescript | — | — | 3 | A | `purs` parse | — |
| 95 | rego | — | — | 3 | A | `opa parse` | — |
| 96 | rescript | — | — | 3 | A | `rescript format` | — |
| 97 | cue | — | 13 | 3 | A | `cue vet` | import resolution |
| 98 | latex | — | 20 | 3 | **C** | — | **TeX is macro-expansion; "syntax" is undefined** |
| 99 | mermaid | — | — | 3 | A | `mermaid` parser | — |
| 100 | meson | — | — | 3 | A | `meson --internal` ? | **weak** |

**Below the line — no viable oracle at any price.** These have grammars, and
several have real usage, and they must not be swept, because a sweep without
a working oracle reports corpus noise as grammar gaps and the numbers stop
meaning anything.

| Language | Why |
|---|---|
| **perl** | `perl -c` executes BEGIN blocks from the corpus and fails on missing modules — **measured, §2** |
| **matlab** | Reference implementation is proprietary and not scriptable per-file. Octave is a different language |
| **apex** | Salesforce's compiler is server-side and proprietary |
| **latex / tex** | Validity is macro-expansion-dependent; there is no syntax to check |
| **vbdotnet** | Roslyn does have a VB parser (so it is really Tier A) — listed here only because nothing ranks VB corpora |
| **cfml, sourcepawn, hoon, and most of the long tail** | Parser exists but nothing per-file and no corpus ranking source |
| **c, c++, objc, cuda** | *Not* below the line — Tier B. Sweepable, but `gap_files` is permanently a floor and must never be quoted without the indeterminate count beside it |

**Where the ceiling lands.** Counting the table exactly: **93 are Tier A**,
**3 are Tier B** (c++, objc, cuda — c itself is already done, and haskell is
a milder variant of the same shape), and **4 are Tier C** and must be
dropped. So **96 of the 100 are sweepable**, three of them only with a
permanent floor on `gap_files`.

That is a better answer than expected, and it moves the constraint
elsewhere. The back third of the table is config and markup formats whose
grammars are small and whose corpora are hard to *rank*. The project is not
oracle-limited at 100 — it is **corpus-ranking-limited**.
More than half the languages past #40 have no download-ranked registry, and
solving that (the ecosyste.ms pattern java uses, generalized) is the work
that unlocks the second fifty.

---

## 6. What breaks at 100 — CI

Written against the five-grammar workflow. While this was in progress,
`main` landed `scripts/changed-grammars.sh` (PR #9), which fixed two of the
four problems independently — and fixed them better than the first draft of
this branch did, by putting the rule in one self-tested script that both
workflows share instead of inline in YAML. The table below says who fixed
what, because the difference matters for what is left to do.

### Already fixed on main (PR #9, not this branch)

| # | Was | Now |
|---|---|---|
| 1 | matrix a hardcoded literal of 5 names | derived from `crates/*/ledger.json`; a new grammar needs no workflow edit and cannot be silently missing |
| 2 | path filter `crates/**` ran every grammar | `scripts/changed-grammars.sh` classifies a change as concerning one grammar, all of them (a core path), or none; both workflows consume the same answer |

### Implemented on this branch

Both were still present on main after PR #9. `scripts/verify.sh` was run
end-to-end with **every submodule deinited** to prove the second is safe.

| # | Was | Now | Measured saving at 100 grammars |
|---|---|---|---|
| 3 | `cargo build --release` in every matrix job | built once in its own job, downloaded as an artifact; verify.sh needs only the binary, so the matrix jobs carry no Rust toolchain at all | 99 redundant builds removed |
| 4 | `submodules: true` fetched every upstream repo into every job | `submodules: false`; `materialize.sh` already initializes only its own | **2.3 s and 13 MB per grammar, per job** → ~230 s and ~1.3 GB saved *in each of* 100 jobs |

Also added, none of it touching the selection rule: a `concurrency` group so
superseded pushes cancel; `max-parallel: 10`; per-job timeouts; and a
`verified` roll-up status check that is green for a matrix of any size
including zero — a dynamic matrix cannot be named in branch protection, and
a skipped required check blocks a PR.

**Confirmed in a real run** (run 31333943195, all 8 jobs green): the
`changes` job took 3 s, `build-cli` 74 s once, and in each verify job
`actions/checkout@v4` completed in **1 second** with no submodules, against
verify times of 19–94 s. Together with PR #9, a one-grammar change goes from
~100 jobs and on the order of 7.5 hours of runner time to **one verify job
of about 20 seconds** plus a shared build.

### Verified limits (the brief's open questions)

- **Matrix cap is 256 jobs per workflow run.** Confirmed in GitHub's
  documentation. 100 grammars fits. 100 with dialect variants would not
  necessarily — typescript already needs two generate dirs, and if variants
  ever became matrix legs rather than in-job loops the cap is reachable.
  Keeping dialects *inside* one job per grammar, as `generate_dirs` does
  today, is what keeps this safe, and is worth stating as a rule.
- **Concurrency is 20 jobs, and it is shared across the entire
  organization**, not per repository — GitHub Free, and public repos get no
  increase. So a 100-grammar matrix does not run 100-wide; it runs 20-wide
  in 5 waves, *and* it starves every other repo in the org while it does.
  `max-parallel: 10` now leaves half the org's capacity for the daily fix
  PRs and for publishing.
- **Generate time varies enough to matter.** Measured, pinned CLI 0.25.10:
  `tree-sitter generate` alone runs 1.14 s (java) to 15.35 s (csharp), a
  **13.5× spread**; full `materialize.sh` runs 1.6 s to 47.5 s, a **30×
  spread** (typescript pays `npm ci` plus two generate dirs); full
  `verify.sh` runs 2.5 s to 50.8 s, a **20× spread**. A per-grammar timeout
  is therefore justified *in principle* — but since even the slowest is
  under a minute, a flat `timeout-minutes: 15` bounds a runaway without
  discriminating, and that is what is implemented. **Proposed:** add an
  optional `timeout_minutes` to `ledger.json` only if a grammar actually
  approaches the cap, rather than raising it for everyone.

### Proposed, not implemented

- **Cache the tree-sitter CLI.** `materialize.sh` invokes
  `npx -y tree-sitter-cli@<pinned>` once per generate dir. Warm, that costs
  0.33 s against 0.045 s for an installed binary; cold in CI it is a
  download per job. Install the pinned CLI once and put it on PATH. Small,
  but it is 100–200 npx resolutions per full fan-out.
- **A scheduled full run.** Now that pushes only verify what a change
  concerns, nothing re-verifies untouched grammars against toolchain drift.
  A weekly scheduled run with every grammar in scope restores that, and is
  the right place to catch a submodule host going away or npm yanking a
  generate dep.
- **Bump the pinned actions.** `checkout@v4`, `setup-node@v4` and
  `upload/download-artifact@v4` all emit Node 20 deprecation warnings and are
  being force-run on Node 24. Harmless today; a failure later.

---

## 7. What breaks at 100 — publishing

**Neither problem this section originally reported still stands.** Both were
fixed on main while this was being written, and the record is kept here
because the reasoning still applies to whatever is built next.

`scripts/publish.sh` was never the problem. It decides what to publish per
crate, by diffing that crate's directory against the tag of its own last
publish (`<crate>-v<version>`), *before* materializing — the expensive part.
One grammar changing publishes one crate. That is the correct design and it
needed no change.

The `plan` and `rehearse` jobs around it did package every grammar on every
PR, serially, in one job. PR #9 fixed that too: both now take
`needs.verify.outputs.grammars` and pass only those crate dirs to
`publish.sh` / `test-publish.sh`. `--force` remains in `plan`, which is
correct — it means "package even if unchanged since the tag", now applied to
a selected set rather than to everything.

**What is left is narrower, and real.** When a *core* path changes —
`scripts/`, `treebank-cli`, `tools/`, the workflows — `changed-grammars.sh`
correctly puts every grammar back in scope, and then:

- `plan` runs `publish.sh --dry-run --force` over all of them **serially in
  one job**, and
- `rehearse` runs `test-publish.sh` over all of them, **also serially in one
  job**,

both with `submodules: true`, so each job also checks out every upstream
grammar repo. At five grammars that is a couple of minutes. At 100, on the
measured materialize times alone (1.6–47.5 s, mean ~14 s), `plan` is over 20
minutes before `cargo package` is counted, and a core-path change is not
rare — it is what every scripts/ edit does.

**Proposed:** make `plan` and `rehearse` matrix jobs over the same grammar
list rather than loops inside one job, and give them `submodules: false` as
`verify` now has. The rehearsal's purpose — exercising the tag, the
skip-on-rerun and the suffix increment — is proved by one crate, so a
core-path change could also rehearse a fixed control grammar rather than all
100. Neither risks an unwanted upload: `publish` cannot run on a
`pull_request` at all, and the real publish job re-verifies independently.

One thing to preserve: the `plan`/`rehearse` pair is what makes an
irreversible operation reviewable, and thinning it must not become skipping
it.

---

## 8. The daily sweep at 100 languages

Measured end-to-end, `TREEBANK_LIMIT=100`, two ecosystems:

```
javascript  fetch 100 pkgs ->     720 files,  18 MB    6.0 s
            sweep          ->     720 passed, 0 failed 0.2 s

java        fetch 100 pkgs ->  21,049 files, 249 MB   16.0 s   (89 resolved; 11 ship no sources jar)
            sweep          ->  21,049 passed, 0 failed 1.2 s
```

Two structural facts make this cheap and keep it cheap:
tarballs are cached under `corpus/<lang>/cache/`, and the sweep keeps a
per-file sha256 pass-cache keyed to the grammar build fingerprint
(`sweep.rs`), so a steady-state day only parses what actually changed.

**But `TREEBANK_LIMIT` as one global number does not survive contact with
100 languages.** The same limit of 100 produced **720 files from npm and
21,049 from Maven Central — a 29× spread, both measured above.** Across the
ecosystems in play it is worse than that:

| Ecosystem | 100 packages ≈ | Source |
|---|---|---|
| npm | **720 files, 18 MB** | measured |
| Maven Central | **21,049 files, 249 MB** | measured |
| NuGet → git repos | ~7,000 files / 74 MB for **five** repos | measured; each is a monorepo |
| Debian C sources | ~40,000 files for **twenty** | C session's pilot |

npm's top packages are tiny single-purpose utilities; C#'s resolve to
monorepos like `dotnet/dotnet`; a Debian source package is an entire
project. A limit that gives a useful corpus for npm gives a trivial one for
Maven and an unaffordable one for C.

**Proposed:** make the limit a per-language ledger field with a global
default, expressed as a **file budget** rather than a package count — the
thing that actually costs money is files parsed and files adjudicated, and
that is the number that should be capped. `Lang::rank` already returns an
ordered list, so the fetch driver can stop when the budget is hit rather
than at a fixed package count.

**Bandwidth, disk and wall clock at 100 languages.** The two measured
languages bracket the range at 18 MB and 249 MB per language per limit-100
corpus. Taking java as the realistic weighting for compiled languages and
npm for the config/markup tail, 100 languages lands at roughly **10–25 GB**
of cached tarballs plus a similar amount extracted, and — because of both
caches — **on the order of an hour of cold daily work, minutes once warm**
(java's 21,049 files swept in 1.2 s; the sweep parses ~17,000 files/second).
Neither is a constraint against the 282 GB free here. The agent sessions,
not the sweeps, remain the cost centre, exactly as `DESIGN.md` says.

The one number that does grow badly is the C/C++ oracle. At 35.5 s and
1068 s per thousand *adjudicated* files, and with adjudication running only
on grammar failures, a bad C++ day is hours where every other language is
seconds. That belongs in the same decision as §10.1.

---

## 9. Reproducing the numbers

- **Oracle throughput.** Build a list of absolute paths, one per line, feed
  it to the oracle's stdin, time it. Beware two traps that were hit and
  fixed while producing this document: relative paths make every oracle
  report "invalid" very fast, and `grep -c 'valid$'` also matches
  `invalid` — count with `grep -c $'\tinvalid$'`.
- **Parse-only is not always strict enough.** Python's first sweep used
  `ast.parse`, and 11 of the 30 files it called grammar gaps were files
  CPython would refuse to run: `return`/`await` outside a function, starred-
  expression misuse, a bare `except:` not last. Those are SyntaxErrors raised
  *after* the parse stage. `compile(src, path, 'exec')` catches them at a
  27% throughput cost, and the same question is worth asking of every oracle
  in §5 before its numbers are believed.
- **Fixed vs marginal cost.** Run each oracle with empty stdin to get its
  startup cost, then subtract. Every oracle here is startup-dominated at
  1000 files (java 0.40 s of its 1.63 s; typescript 0.34 s of 0.57 s), so
  quoting a per-file rate without separating the two is misleading.
- **C / C++ adjudicability.** `tools/c-oracle` (now on `main`),
  built with `TREEBANK_LLVM_DIR=/usr/lib/llvm-20`. Feed
  `<path>\t-xc++\t-std=gnu++20\t-ferror-limit=0\t-w\t-iquote<dir>\t-I<pkg>`
  per line; count `.verdict`.
- **CI.** `git submodule deinit -f --all`, then `scripts/verify.sh
  crates/treebank-<lang>` — it passes, initializing only its own submodule,
  which is what makes `submodules: false` safe.
- **Grammar health.** `gh api repos/<slug>` for each of the 307 distinct
  GitHub repos in nvim-treesitter's `parsers.lua`, reading `pushed_at`,
  `archived`, `stargazers_count` and `open_issues_count`. Quote the >1y
  count alongside any median: every host group is bimodal, so a median alone
  hides the dead tail.
- **Popularity.** crates.io `?q=tree-sitter&sort=downloads` (8 pages, 2308
  hits), npm `api.npmjs.org/downloads/point/last-month/<comma-list>`,
  `nvim-treesitter/lua/nvim-treesitter/parsers.lua`, Helix `languages.toml`,
  Zed `extensions.toml` plus the bundled set in `crates/languages/src`.
  Grammar-crate names were mapped to languages by hand where they diverge
  (`tree-sitter-c-sharp`, `-kotlin-ng`, `-toml-ng`, `-sequel`, `-md`,
  `-embedded-template`, `-sfapex`), and non-grammar crates
  (`tree-sitter-language`, `-highlight`, `-loader`, …) filtered out.

## 10. Open questions for the human

1. **C++ is the one that needs a decision.** 1068 s per thousand files and
   33.5% unadjudicable is a different kind of language from everything else
   in the top 20. Sweep it small, sweep it with real build metadata, or
   accept sampled coverage — but pick before it is started.
2. **The ceiling is 96 sweepable languages of the top 100**, which is
   higher than feared — so the binding constraint past #40 turns out to be
   **corpus ranking**, not oracles. Generalizing the ecosyste.ms pattern is
   probably worth more than the next ten grammars.
3. **`TREEBANK_LIMIT` should become a file budget.** It is the one change
   in this document that alters existing behaviour for existing languages.
4. **Twenty-one of the top 100 grammars are over a year stale**, and the
   `tree-sitter-grammars` org — the intuitive place to look for a good
   grammar — is the worst-maintained host of the three (§3). For most of
   those the answer is "vendor it and carry the patches", which is exactly
   what treebank is for. But `scss` in particular is popular on a
   four-year-dead 33-star repo, and is ranked at #29 on popularity that says
   nothing about the grammar. It should probably drop or be re-pointed
   before anyone works through the list that far.
