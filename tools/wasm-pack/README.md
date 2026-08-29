# Wasm packs

One standalone WebAssembly module per grammar: the tree-sitter runtime, the
grammar, and a small ABI, statically linked. A pack imports **only WASI**, so
it loads from Python, Go, Ruby, Rust or a browser with no Rust toolchain, no
C compiler, and no emscripten glue at the far end.

```sh
./tools/wasm-pack/build.sh python          # -> dist/wasm/treebank-python.wasm
./tools/wasm-pack/check.sh                 # build + verify every grammar
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
| `tb_roles()` | the facet manifest. Facets (`_callable`, `_binding`, `_scope`, `_clause`) cross-cut derivations, cannot be supertypes, and must be expanded against this manifest. Without it a consumer **cannot write `(_callable)` at all**. |
| `tb_node_types()` | the node manifest, carrying table-tier membership: that `while_statement` derives from `_loop` and `_loop` from `_statement`. Supertypes are visible to a tree-sitter **query**, and this ABI exposes node walking rather than a query engine — so a host walking the tree sees `while_statement` and has no other way to learn it is a `_loop`. Without it a pack cannot answer for the vocabulary that is the point of a treebank grammar. |

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

## Distribution

Packs are published to **R2**, and served from `treebank.dev/packs/` by the
site's Worker. They are content-addressed:

```
treebank-python-<sha256[:12]>.wasm   immutable, cached for a year
treebank-python.wasm                 the moving pointer
index.json                           which hash is current, per grammar
```

A pack is byte-reproducible, so a hashed key is those bytes or does not
exist, and a pinned URL cannot change under a consumer. The plain name is
resolved through `index.json` rather than duplicated as a second object,
because two names for the same bytes is two things that can disagree.
Old hashed objects are never deleted: that is what keeps a pin working.

CI publishes on a push to `main`, uploading the packs the gate above already
built and checked -- hashed objects first, then the manifest that names them,
so the pointer is never live before its target -- and then reads every object
back, because a publish that reports success and leaves the bucket empty is
the failure worth ruling out.

It uploads over the **S3 API** with a token scoped to this bucket alone:
`CLOUDFLARE_ACCOUNT_ID` for the endpoint, plus `R2_ACCESS_KEY_ID` and
`R2_SECRET_ACCESS_KEY`. Wrangler's REST object endpoint was tried first and
refuses a bucket-scoped token with a 403, and Cloudflare's documentation does
not say which permission group it wants; the S3 API is what a bucket-scoped
token is for. The narrow scope is worth the second secret, because this
credential can write WebAssembly that executes in visitors' browsers and
nothing else.

**Why not GitHub Releases** -- it was the obvious answer and it cannot work.
Release assets carry no `access-control-allow-origin`, on either the
`github.com` redirect or the final `release-assets.githubusercontent.com`
object, so a browser cannot fetch them at all. R2 has free egress, and the
Worker makes the packs same-origin, so the question does not arise. Consumers
outside a browser were never blocked by CORS and can fetch the same URLs.

`list-grammars.sh` is the shared source for CI and for publishing: every
`crates/treebank-*/grammar.js` creates a pack obligation, and there is no
matrix to remember to extend when a grammar is added.

`test-consumers.sh` covers what `check.sh` cannot. `check.sh` proves a pack
loads under a WASI runtime; this proves the ABI is usable. It serves the
packs over localhost, fetches them **by URL** as a real consumer does,
verifies sha256 against the bytes that travelled, and drives both example
bindings -- asserting each grammar's sweep-smoke invalid fixture is rejected
and its valid fixture parses clean. A pack that accepted everything would
pass a valid-only check. Those bindings are also the reference the browser's
was ported from, so if they break, the playground breaks with them.

## Status

Built, gated and consumer-checked on every change; published to R2 on every
push to `main`, and read from there by the playground.
