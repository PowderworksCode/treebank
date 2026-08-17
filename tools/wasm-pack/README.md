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

## Status

Built and gated on every change; **published nowhere**. The registry
(`packs.json` over GitHub Releases, each entry carrying the sha256 of an
immutable asset) is the next step and is deliberately not wired to anything
that fires.
