# Wasm packs

One standalone WebAssembly module per grammar: the tree-sitter runtime, the
grammar, and a small ABI, statically linked. A pack imports **only WASI**, so
it loads from Python, Go, Ruby, Rust or a browser with no Rust toolchain, no
C compiler, and no emscripten glue at the far end.

```sh
./tools/wasm-pack/build.sh python          # -> dist/wasm/treebank-python.wasm
./tools/wasm-pack/check.sh                 # build + verify all three
```

## Why not `tree-sitter build --wasm`

That emits an emscripten **side module**: grammar tables only, expecting the
tree-sitter runtime to already be present in its linear memory. It is the
right drop-in for web-tree-sitter and unusable from anything else, because a
host that isn't web-tree-sitter must first implement emscripten's
dynamic-linking protocol (`__memory_base`/`__table_base` allocation, data
relocations). Packs exist so a binding in any language is a few dozen lines —
see `examples/parse.py` and `examples/parse.mjs`, which are complete.

## What travels inside the module

| export | why it is *inside* rather than beside |
|---|---|
| `tb_provenance()` | what this pack is and how it was measured. A `.wasm` copied out of a release, vendored into a repo and rediscovered two years later still answers "which grammar, which vocabulary, which CLI, what were the sweep numbers" from its own bytes. The file next to the binary is the thing that goes missing. |
| `tb_roles()` | the facet manifest. Table-tier roles (`_declaration`, `_loop`, …) are real supertypes and queryable straight from the parser; facets (`_callable`, `_binding`, `_scope`, `_clause`) cross-cut derivations, cannot be supertypes, and must be expanded against this manifest. Without it a consumer **cannot write `(_callable)` at all**. |

Provenance is a **source hash**, not an upstream sha and a patch series:
treebank owns its grammars, so there is no upstream to point at.

A pack carries the parser and **not the oracle**. `validate()` drives CPython,
`syn`, `tsc`, V8 — a wasm module cannot. The sweep numbers in provenance are
evidence recorded at build time, not something the pack can re-derive.

## Reproducibility

```
grammar src (committed, CI-checked) + runtime @ pinned sha256
  + shim + wasi-sdk/binaryen @ pinned sha256
  -> identical bytes
```

Verified two ways and falsified once: identical on rebuild, identical from a
different absolute path, and **different** when `parser.c` is perturbed — so
the build really recompiles rather than caching. No embedded paths,
timestamps or build IDs; provenance deliberately carries no build host or git
sha, because anything ambient would break the property it exists to make
checkable.

## Toolchain, pinned and cached

wasi-sdk, binaryen and the tree-sitter runtime are downloaded,
sha256-verified and cached in `${TREEBANK_WASM_CACHE:-$XDG_CACHE_HOME/treebank}`
— never taken from `PATH`. That is deliberate: a contributor with their own
`emcc` or a system tree-sitter would otherwise silently produce different
bytes from CI.

**Binaryen is not optional**, and that only shows up if you check every
grammar rather than one. `lld` emits a single data segment spanning the whole
static image; `wasm-opt`'s memory packing splits it and drops the long zero
runs parse tables are full of. Measured here:

| | lld alone | + wasm-opt | |
|---|---:|---:|---|
| python | 748 KB | 650 KB | −13% |
| rust | 1,103 KB | 923 KB | −16% |
| **typescript** | **2,707 KB** | **1,629 KB** | **−40%** |

The effect scales with table size, so generalising from the smallest grammar
would have shipped the largest one 66% bigger than necessary.

Two build settings are load-bearing:

- **`-O3`, not `-Oz`** — `-Oz` buys about 6% size for 28% throughput, the
  wrong trade for something whose job is parsing corpora.
- **No LTO** — `-flto` silently exports `_start` instead of `_initialize`,
  which loses the WASI reactor exec model, and every host then refuses to
  instantiate the module. It looks like a runtime bug and is a link flag.

## Releases and the index

```sh
./tools/wasm-pack/release.sh                 # stage into dist/release
./tools/wasm-pack/release.sh --publish       # the only networked path
./tools/wasm-pack/index.sh                   # packs.json
./tools/wasm-pack/test-release.sh            # rehearse all of it, publish nothing
```

A release is the `.wasm`, its provenance and roles as sibling JSON, and
`SHA256SUMS` over all three. The siblings are extracted **from the module**,
so they cannot disagree with what it says about itself; the module remains
the source of truth.

**GitHub Releases, not npm** — measured rather than assumed: npm's
tree-sitter grammar packages ship native `.node` prebuilds and contain zero
wasm, while every upstream grammar publishes `<name>.wasm` as a release
asset. Releases also need no account and no secret, and are an HTTPS GET from
any language, which is what matters when packs are consumed from bindings
rather than only from JS.

`packs.json` is published to a **moving `packs-index` tag**, so a consumer has
one stable URL instead of N releases to discover. The index is mutable by
design and nothing in it is trusted: every entry carries the sha256 of an
immutable artifact.

Versions are plain semver from each crate's `Cargo.toml`. The vendoring era's
`<upstream>-treebank.N` scheme tracked an upstream version and a build
counter; treebank owns these grammars, so neither exists.

`test-release.sh` closes what publishing leaves untestable — the tag, the
skip on a re-run, and a consumer actually fetching over HTTP. It stages
every pack under a `rehearsal-wasm/` tag namespace, serves it over
localhost, fetches **by URL**, verifies `SHA256SUMS` against the fetched
bytes, runs both example consumers (asserting each grammar's negative
fixture is still rejected and its rosetta program parses clean), checks the
index's hashes and that its URLs name the real tag rather than the rehearsal
one, and asserts a re-run releases nothing. It publishes nothing and deletes
its tags on exit.

## Status

Built, gated and rehearsed on every change; **published nowhere**. The first
real publish is a `--publish` run, and it is deliberately not wired to
anything that fires.
