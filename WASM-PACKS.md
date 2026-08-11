# Wasm packs

A **pack** is one WebAssembly module carrying the tree-sitter runtime, one
treebank-patched grammar, and a small ABI, statically linked. It imports only
WASI. Loading one needs a wasm runtime and nothing else — no Rust toolchain, no
C compiler, no emscripten glue, no native tree-sitter.

Mechanism: [`scripts/build-wasm.sh`](scripts/build-wasm.sh) and
[`tools/wasm-pack/shim.c`](tools/wasm-pack/shim.c), released by
[`scripts/publish-wasm.sh`](scripts/publish-wasm.sh) and rehearsed by
[`scripts/test-publish-wasm.sh`](scripts/test-publish-wasm.sh).

## What a pack is for, and what it is not

The value is **the grammar, with the corpus-driven patches, and the provenance
to prove it**. Anyone can run `tree-sitter build --wasm` on upstream. What they
cannot do is tell you which upstream, which patches, which toolchain — or show
the corpus evidence that justified each divergence.

A pack does **not** carry the oracle. `validate()` shells out to `javac`,
Roslyn, libclang, `php -l`, `compile()`. A wasm module cannot drive a real
toolchain, so a consumer gets treebank's patched parsing and does **not** get
the sweep verdicts the patching was derived from. The sweep numbers in a pack's
provenance are evidence *recorded at build time*, not something the pack can
re-derive. Observer packs are processes, not modules; this is the grammar half
only, and saying otherwise is the main way this goes wrong.

## Status

**Implemented and measured**: the pack format and ABI, the build with all three
toolchain pins, embedded provenance, byte-reproducibility across every grammar,
the release path, the local rehearsal, and working Python and JavaScript
consumers. Nothing has been published anywhere.

**Proposed**: the query API in the pack ABI, an npm mirror, and per-language
binding packages. Marked as such at the end.

## 1. Is the wasm build reproducible?

**Yes, byte for byte.** Every grammar, two full builds each:

```
treebank-c: IDENTICAL          treebank-python: IDENTICAL
treebank-csharp: IDENTICAL     treebank-rust: IDENTICAL
treebank-java: IDENTICAL       treebank-typescript: IDENTICAL
treebank-javascript: IDENTICAL treebank-tsx: IDENTICAL
```

Pushed harder on python, since a repeat build in one directory is a weak test:

| test | result |
|---|---|
| same tree, built twice | identical |
| built from a different, deeper absolute path | identical |
| full re-materialize (fresh clone → patches → `generate`) → wasm | identical |
| canary: perturb `ts_symbol_names` in `parser.c` | **differs**, so the build really recompiles |

`strings` finds no embedded paths, timestamps or build IDs in the output. The
materialization invariant extends all the way through:

```
upstream @ pinned sha + patches + generate (pinned CLI)
  + runtime @ pinned sha + shim + emscripten @ pinned digest
  -> identical bytes
```

This is the answer everything else depends on. Because it holds, a pack can
carry the same guarantee a crate does, and `SHA256SUMS` in a release is a claim
anyone can re-derive rather than a hash of whatever CI happened to produce.

## 2. Pinning the toolchain

Three pins, all load-bearing:

| pin | value | why |
|---|---|---|
| `generate_cli` | 0.25.10 | the ledger's existing pin; produced `src/parser.c` |
| runtime | `vendor/tree-sitter` @ `da6fe9be` | linked into the pack; must understand the language ABI that CLI emits |
| wasi-sdk | 33.0, sha256 per platform | compiles and links it |
| binaryen | 131, sha256 per platform | packs the data section; see below |

The runtime commit is not a new judgment call: `da6fe9be` is the commit
`tree-sitter --version` reports for 0.25.10, and its `lib/src` is byte-identical
to the crates.io tarball of `tree-sitter 0.25.10` already in `Cargo.lock`.
`build-wasm.sh` refuses to build if the runtime's version disagrees with the
ledger's `generate_cli`, so the two cannot drift apart silently.

Both compilers are pinned **by sha256, per platform**, downloaded to a cache
outside the repo and verified before use — never taken from `PATH`. Compiler
version is load-bearing: measured on emscripten, 3.1.74 and 4.0.4 turned
identical sources into different bytes (551,549 vs 551,777). Bumping either pin
is a toolchain change with the same weight as bumping the CLI.

Two hazards worth naming:

- **A toolchain on `PATH` must not be able to influence the output.**
  `tree-sitter build --wasm` gets this wrong: it prefers a local `emcc` and
  only falls back to Docker, so a contributor with emscripten installed
  silently produces different bytes from CI. `build-wasm.sh` does not offer the
  choice.
- **The pins are independent, deliberately.** Treebank is on CLI 0.25.10 for a
  measured reason — 0.26.x ships broken Unicode identifier tables — but packs
  already build with wasi-sdk, the toolchain 0.26.x adopted. The wasm toolchain
  and the CLI move separately, so the eventual CLI bump is one change to
  evaluate rather than two at once.

### Why wasi-sdk, and why binaryen is not optional

Treebank originally built packs with emscripten in Docker. That works and is
reproducible, but the image is **2.81 GB**, pulled once per CI job from a
rate-limited registry — which is fine at eight grammars and not at twenty.
wasi-sdk is a 193 MB tarball, needs no Docker at all, and is where upstream
tree-sitter went (0.26.1 replaced emscripten with wasi-sdk for
`build --wasm`).

The catch, found only by checking every grammar rather than one: `lld` emits a
single data segment spanning the whole static image, where emscripten runs
`wasm-opt`, whose memory-packing pass splits it and drops the long zero runs
that parse tables are full of. The effect scales with table size, so it is
nearly invisible on the smallest grammar and enormous on the largest:

| | lld alone | + `wasm-opt -O3` | emscripten |
|---|---:|---:|---:|
| python | 628,003 | 564,805 | 560,956 |
| rust | 1,348,851 | 894,378 | 887,233 |
| **csharp** | **5,647,046** | **3,073,926** | 3,069,052 |

Generalising from python would have shipped a csharp pack 84% larger than it
needed to be. With binaryen pinned alongside, the final numbers land within
**0.5% of emscripten on total size and 0.1% on parse throughput**, for 296 MB
of toolchain instead of 2.81 GB and no Docker anywhere.

Two settings are load-bearing and easy to get wrong:

- **`-O3`, not `-Oz`.** Measured on 497 CPython stdlib files (10.1 MB):
  `-O3` 7,343 bytes/ms, `-O2` 6,608, `-Oz` 5,348. `-Oz` buys 6% size for 28%
  throughput, which is the wrong direction for something meant to parse corpora.
- **No LTO.** `-flto` silently loses the reactor exec model — the module
  exports `_start` instead of `_initialize` and every WASI host refuses to
  instantiate it. It also made the module *bigger*. Do not re-add it without
  checking the exports.

### Why this is not a new ledger field

The brief asked to extend the ledger rather than invent a parallel mechanism.
On inspection the ledger needs no new field, and adding one would be worse:

- `generate_cli` is already there and is per-grammar.
- The runtime pin lives in `vendor/tree-sitter`'s submodule pointer, which is
  the same mechanism `upstream/` uses, checked against `generate_cli` at build
  time.
- The wasi-sdk and binaryen pins are repo-wide, like `scripts/`, and live in
  `tools/wasm-pack/toolchain.sh`. Copying them into seven ledgers would create
  seven copies of one fact that must never diverge.

What *is* per-grammar — the upstream pin, the patch series, `generate_dirs` —
the ledger already carries, and the pack's provenance is generated from it. The
provenance uses `{tool, version, …}` objects to match the shape
`oracle: {tool, version, dialect}` is converging on elsewhere, so it reads as
one pattern.

## 3. Where does it publish

**GitHub Releases.** Measured, rather than assumed:

```
npm  tree-sitter-python@0.25.0   ->  prebuilds/*/tree-sitter-python.node   (native; zero .wasm)
GH   tree-sitter-python v0.25.0  ->  tree-sitter-python.tar.gz, tree-sitter-python.wasm
```

Same for `tree-sitter-bash`, `-rust`, `-json`. **npm's tree-sitter grammar
packages contain no wasm at all**; every upstream grammar publishes its wasm as
a release asset. That is the convention nvim-treesitter's `tier` field reads,
and it is what a consumer already knows how to fetch.

The second reason is that packs are meant to be consumed from bindings in many
languages. A release asset is an HTTPS GET, which every language's standard
library can do. npm would make each non-JS binding implement dist-tag resolution
and tarball extraction to reach a file the JS ecosystem does not put there.

Releases also need **no account and no secret** — `GITHUB_TOKEN` creates them —
where crates.io needed a human to mint a token before anything could ship.

**What it costs to add a second registry later:** little, deliberately. The
artifact is one file; a mirror job uploads the same bytes with the same name and
the same version. What must not fork is the *scheme*, so `publish-wasm.sh`
derives names and versions exactly as `publish.sh` does. The known gap is
mutability: release assets can be replaced, so a pack's identity rests on its
`SHA256SUMS` and its embedded provenance rather than on the URL. OCI would fix
that natively by addressing blobs by digest, and is the obvious second mirror if
digest-addressing ever matters more than reach.

## 4. Naming and versioning

Matched to the crates.io scheme, because a second scheme is a support burden:

| | crates.io | releases |
|---|---|---|
| name | `treebank-grammar-python` | `treebank-python.wasm` |
| version | `0.25.0-treebank.N` | `0.25.0-treebank.N` |
| tag | `treebank-grammar-python-v…` | `treebank-python-v…` |

The asset name mirrors upstream's own (`tree-sitter-python.wasm` →
`treebank-python.wasm`), so the substitution is obvious at a glance. The version
is the ledger's `upstream.version` plus an incrementing suffix derived from
existing tags — never stored, so it cannot drift. It inherits the crate scheme's
tradeoff, stated in [PUBLISHING.md](PUBLISHING.md): `0.25.0-treebank.1` is a
semver *pre-release*, so consumers must name the exact version.

One release per grammar *directory*, carrying every pack that directory
generates: typescript builds `treebank-typescript.wasm` and `treebank-tsx.wasm`
from one pinned upstream at one version, and separate releases would let them
drift for no reason.

## 5. Only rebuild what changed

`publish-wasm-packs.yml` calls `verify-grammars.yml` and takes its `grammars`
output — the same `scripts/changed-grammars.sh` answer the crate workflow uses.
One definition of "which grammars does this change concern", not three.

Release scope is the narrower question, decided per pack by diffing against the
tag of its own last release, plus the files outside a grammar directory that
change what a pack *contains*:

```sh
ARTIFACT_INPUTS=(scripts/build-wasm.sh tools/wasm-pack/shim.c vendor/tree-sitter)
```

`vendor/tree-sitter` is in that list because the runtime is linked into every
pack, so bumping it really does change all of them. `scripts/verify.sh` is not:
it decides whether a pack may ship, not what is in it.

`vendor/` was also added to `changed-grammars.sh`'s `CORE` list, with a
self-test case. Without it a runtime bump produces an *empty* matrix, the
release job is skipped by its own `if:`, and the run goes green having released
nothing — a failure that looks exactly like success. It costs some wasted
verification on a rare event, which is the right side to err on, and keeps one
definition of "core" rather than a second list that only wasm knows about.

## 6. What a consumer actually writes

Both examples below are the complete binding. Run for real, output pasted.

**Python** ([`tools/wasm-pack/examples/parse.py`](tools/wasm-pack/examples/parse.py)),
`pip install wasmtime`:

```python
from wasmtime import Engine, Linker, Module, Store, WasiConfig

engine = Engine(); store = Store(engine); store.set_wasi(WasiConfig())
linker = Linker(engine); linker.define_wasi()
e = linker.instantiate(store, Module.from_file(engine, "treebank-python.wasm")).exports(store)
e["_initialize"](store)

src = open("some.py", "rb").read()
ptr = e["tb_alloc"](store, len(src))
e["memory"].write(store, src, ptr)
tree = e["tb_parse"](store, ptr, len(src))
```

**JavaScript** ([`tools/wasm-pack/examples/parse.mjs`](tools/wasm-pack/examples/parse.mjs)),
no dependencies at all — Node ships WASI:

```js
import { WASI } from 'node:wasi';
const wasi = new WASI({ version: 'preview1' });
const inst = new WebAssembly.Instance(new WebAssembly.Module(readFileSync('treebank-python.wasm')),
                                      wasi.getImportObject());
inst.exports._initialize();
```

Same binary, both runtimes, identical output:

```
treebank-python  language=python  pack_abi=1
  upstream tree-sitter-python 0.25.0 @ 293fdc02038e
  5 parser-fix patches; sweep 31 -> 3 gap files

  fixtures/patched.py: clean
    (module (class_definition name: (identifier) body: (block (decorated_d...

  fixtures/must-reject.py: 1 error(s)
    1:6  MISSING at ())
```

Every line above the fixtures is read **out of the .wasm itself**. The
provenance is linked into the module, not shipped beside it: a binary vendored
into someone's repo and rediscovered two years later still answers which
upstream, which sha, which patches and which toolchain. A sibling JSON file
cannot promise that, because the file next to the binary is the thing that goes
missing. It deliberately carries no timestamp, build host or treebank commit —
anything ambient would break the reproducibility the provenance exists to make
checkable.

The differential is the whole pitch. Against upstream's own published v0.25.0
asset and the same fixtures:

```
upstream tree-sitter-python.wasm:  patched.py hasError=true    must-reject.py hasError=true
treebank-python.wasm:              patched.py hasError=false   must-reject.py hasError=true
```

Better on real code, and no looser on invalid code.

`scripts/test-publish-wasm.sh` runs exactly this as a gate: it releases to a
local HTTP server, fetches the assets by URL, verifies `SHA256SUMS`, and parses
each grammar's patch-repro fixture with **both** consumers, asserting the
negative corpus is still rejected. It publishes nothing.

Fixtures come from [`tools/consumer-test/grammars.json`](tools/consumer-test/grammars.json)
— the same file the crate rehearsal reads, so adding a language is one edit
rather than two. A grammar directory that generates several parsers has its
cases split by label, which is the `generate_dir`'s basename, so `tsx` is
proved by `patched.tsx` and `typescript` by `patched.ts`.

Each pack is also checked against the sources it claims to come from — three
**exact** comparisons, no normalisation:

| | compared | catches |
|---|---|---|
| `upstream.sha` | provenance vs `ledger.json` | built from the wrong upstream commit |
| `language_name` | provenance vs `src/grammar.json` | built from the wrong grammar |
| `language_name` | provenance vs the running module | the artifact is not the one its provenance describes |

The authority for what a grammar is called is `src/grammar.json`, which
`tree-sitter generate` writes from `grammar.js`. It is **not** the directory
name, and the two are allowed to differ: the C# grammar declares `c_sharp`
while its directory is `csharp` — the same split the crate has between
`treebank-grammar-csharp` and the `tree_sitter_c_sharp` library. So the
provenance records `grammar` (the directory name, `csharp`) and `language_name`
(the declared name, `c_sharp`) as two separate fields, and each comparison uses
the right one.

That distinction is the whole point. Comparing a pack's declared name against
the directory name looks like a near-miss and invites a fuzzy comparison, and a
check that accepts near-matches cannot catch a pack built from the wrong
grammar — which is precisely the failure it exists for. The entry point is
derived the same way, `tree_sitter_<declared name>`, and `build-wasm.sh`
asserts `parser.c` actually defines that symbol rather than pattern-matching
the source for something that looks like one. That check needs no per-language fixture, which is why it
is the one that scales — and it catches the failure a fixture cannot, since a
pack accidentally built from the wrong grammar still parses *something*
cleanly.

## The index

Twenty-two GitHub Releases are not a distribution. `scripts/wasm-index.sh`
builds a `packs.json` listing every published pack, republished to a moving
`packs-index` tag so there is one stable URL:

```
https://github.com/<owner>/<repo>/releases/download/packs-index/packs.json
```

```json
{ "schema": "treebank-packs-index/1", "pack_abi": 1, "format": "standalone",
  "packs": [ { "pack": "treebank-python", "version": "0.25.0-treebank.1",
               "upstream": { "git_url": "…", "sha": "293fdc02…", "version": "0.25.0" },
               "sha256": "139538c3…",
               "urls": { "wasm": "…", "provenance": "…", "sha256sums": "…", "queries": "…" } } ] }
```

The index is mutable — that is what an index is for — but nothing in it is
trusted: every entry carries the sha256 of an immutable artifact, so a tampered
index can misdirect a consumer but cannot make them accept bytes the project
did not build. It is derived from the release tags that exist, so it cannot
list a pack that was never released, and the rehearsal asserts that every hash
in it matches the artifact it points at and that no URL leaks the rehearsal tag
namespace.

Releases also carry `queries.tar.gz` — upstream's highlight, injection and tag
queries, which every grammar ships and every editor needs. It is built with
sorted entries, zeroed mtimes and `gzip -n`, so it is byte-reproducible too; a
timestamped archive would make `SHA256SUMS` churn and every release look
changed when nothing was.

## 7. Size and licensing

| pack | standalone | side module | Δ | gzipped |
|---|---:|---:|---:|---:|
| java | 395 KB | 440 KB | −10% | 106 KB |
| javascript | 439 KB | 402 KB | +9% | 105 KB |
| c | 523 KB | 611 KB | −14% | 122 KB |
| python | 548 KB | 492 KB | +11% | 118 KB |
| rust | 866 KB | 1.14 MB | −26% | 204 KB |
| typescript | 1.05 MB | 1.39 MB | −25% | 234 KB |
| tsx | 1.06 MB | 1.42 MB | −25% | 240 KB |
| csharp | 2.93 MB | 5.26 MB | −44% | 600 KB |
| **total** | **7.7 MB** | **11.1 MB** | **−30%** | 1.7 MB |

The surprise is that bundling a whole runtime usually makes the artifact
*smaller*. Both formats are data-dominated, and the side module's data section
is nearly double: it is position-independent, so table entries holding pointers
are stored as zeroed slots plus relocations and patched at load. The runtime
costs a fixed ~90 KB, which only dominates for the three smallest grammars.

That reverses on the wire: those zeroed slots compress superbly, so csharp
gzips to 600 KB as a pack against 314 KB as a side module. **Standalone wins on
disk and in memory; the side module wins on download.** For release assets,
served as-is, the pack numbers are the ones that apply.

Build cost is small and materialization dominates it. The wasm step, including Docker container startup, is **5.1 s for python** and **8.4 s for csharp**, the largest grammar here — against 1.1 s and 84.5 s respectively to materialize them. Nothing about packs makes CI slower in a way worth engineering around; not rebuilding unchanged grammars still matters far more.

**Licensing.** These grammars are MIT, which requires the notice to accompany
redistributions — and a `.wasm` is a redistribution of that source in object
form. Every release therefore carries upstream's `LICENSE` verbatim, plus
`LOCAL-PATCHES.md` and `patches/`, for the same reason they ship inside the
crate tarball: the entire divergence is readable without leaving the release.
`publish-wasm.sh` **fails** if it cannot find a LICENSE to copy, rather than
releasing a binary with no attribution. Patch `0001` — the redistribution notice
every grammar carries — is in the patch series inside the pack's provenance, so
the "this is a patched redistribution" statement travels in the binary too.

## Prior art

Prebuilt tree-sitter wasm is not new. Provenanced, reproducible prebuilt wasm
appears to be.

| project | grammar pins | toolchain pins | provenance | patched |
|---|---|---|---|---|
| [Gregoor/tree-sitter-wasms](https://github.com/Gregoor/tree-sitter-wasms) | `^0.21.0` caret ranges | `tree-sitter-cli: ^0.20.8` | none | no |
| [@sourcegraph](https://github.com/sourcegraph/tree-sitter-wasms), @cursorless, @repomix | forks of the above | — | none | no |
| [microsoft/vscode-tree-sitter-wasm](https://github.com/microsoft/vscode-tree-sitter-wasm) | `^0.25.0` caret ranges | emsdk pinned to 3.1.64 at a git sha | `cgmanifest.json`, runtime commit only | no |
| upstream release assets | n/a | **none recorded** | none | no |
| [malivvan/tree-sitter](https://github.com/malivvan/tree-sitter) (Go/wazero) | via go-tree-sitter | none documented | none | no |
| treebank packs | submodule @ sha | all three, emscripten by digest | embedded, per-artifact | **yes** |

The dominant distribution resolves its grammars through caret ranges, so a
rebuild picks up whatever npm serves that day, and its package version has no
relationship to any grammar version — you cannot tell which python grammar is
inside `tree-sitter-wasms@0.1.13`. Microsoft's is the most careful: it pins
emsdk to a git sha and records the tree-sitter runtime commit in a
`cgmanifest.json`. It still caret-ranges the grammars and records nothing about
them, and its pinned emsdk 3.1.64 disagrees with the CLI's own Docker default —
the local-`emcc`-wins hazard, in production.

The sharpest data point is upstream's own: our locally built pristine upstream
python came to 457,906 bytes; upstream's published v0.25.0 asset is 457,883.
Close, and not equal, because no emscripten or CLI version is recorded anywhere
in that release. **Nobody can reproduce the tree-sitter project's own published
wasm.** That is the gap.

Directly relevant, and moving: [py-tree-sitter#272](https://github.com/tree-sitter/py-tree-sitter/pull/272)
adds `Language.from_wasm` on wasmtime — opened July 2024, still a draft, last
touched April 2025, absent from released `tree-sitter` 0.26.0. If it lands, a
Python consumer could load side modules directly and half the argument for the
standalone format weakens. It has not landed in two years.

## Proposed, not built

- **Queries in the pack ABI.** `tb_query_*` over `ts_query_new`/`ts_query_cursor_*`
  would let a pack answer highlight and structural-search queries, which is what
  most consumers actually want. It is additive and would take `pack_abi` to 2.
- **An npm mirror.** A `@treebank/*` package per grammar embedding the same
  `.wasm`, for the JS audience that expects `npm i`. The bytes and the version
  would be the release's; only the wrapper is new.
- **A side-module artifact alongside the pack.** `tree-sitter build --wasm`
  output, named to mirror upstream's asset, as a drop-in for existing
  web-tree-sitter consumers and editors. It costs one more build per grammar and
  is the cheapest way to reach the largest existing audience.
- **Bindings.** The point of the format is that a binding is small; none is
  written yet beyond the two examples.
