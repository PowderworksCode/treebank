# Changelog

Notable changes to the `treebank` crate. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The grammars are not versioned here. They ship as wasm packs, published
continuously and addressed by content, so a grammar improving reaches
consumers without a release — see [treebank.dev/packs](https://treebank.dev/packs/index.json).
Only the Rust API is versioned.

## [Unreleased]

### Changed

- **The vocabulary's two kinds of term are now `structural` and `nominal`,
  not `table tier` and `facet`.** A structural term is a real supertype in the
  parse table, where membership is decided by structure — the parse went
  through it *here*. A nominal term is a list of node types, where membership
  is decided by name. This renames nothing about the terms themselves:
  `(_callable)` is still `(_callable)`, every membership is unchanged, and no
  parse table moves. The vocabulary version deliberately does not bump, for
  the reason notes/DESIGN.md §3.2.1 gives — the closed term list did not
  change, only the words describing it. See notes/vocabulary-naming.md.
- Renames on this crate's surface, all mechanical: `roles` module →
  `terms`, `RolesManifest` → `TermsManifest` with its `facets` field →
  `nominal` (`facets` kept as a serde alias), `Pack::roles` → `Pack::terms`,
  `Pack::roles_json` → `Pack::terms_json`, `PackRoles` → `PackTerms`,
  `check::dead_roles` → `dead_terms`, and `Vocabulary`'s `table`/`facets`/
  `either_tier` → `structural`/`nominal`/`demotable`.
- Each grammar crate's `roles.json` is now `terms.json`, and its `ROLES`
  const is `TERMS`. A wasm pack exports `tb_terms`/`tb_terms_len`; the old
  `tb_roles`/`tb_roles_len` still export the same document, so a host built
  against the previous ABI keeps working, and `Pack::load` prefers the new
  name and falls back to the old. Packs already published are unaffected —
  they are content-addressed and are never rebuilt in place.
- `treebank roles` is now `treebank terms`, with `roles` kept as an alias.

## [0.3.0] - 2026-08-30

### Changed

- Packs are driven by [wasmer](https://wasmer.io) rather than wasmtime.
  wasmtime's `build.rs` compiles C helpers in every configuration that can
  execute wasm, so building it for musl needed a musl C cross-compiler on
  PATH and no feature avoided it. That made this crate unhostable by a
  consumer shipping static musl binaries. wasmer's tree compiles no C, and
  both `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` now
  cross-build with rust-lld alone. The `pack` feature's dependencies change
  with it: `wasmer` and `blake3` in place of `wasmtime` and `wasmtime-wasi`.
- `fetch` uses ureq 3 with the [graviola](https://github.com/ctz/graviola)
  rustls provider rather than ureq 2 with ring. ring's `build.rs` compiles C
  and assembly through `cc`, so `aarch64-unknown-linux-musl` could not be
  built at all; graviola is Rust and inline assembly with no C. Roots still
  come from the platform verifier, so a proxy presenting its own certificate
  authority keeps working, and proxies still come from the environment.

### Added

- `Pack::roles_json` and `Pack::node_types_json`, returning the facet and node
  manifests as the pack ships them. A consumer building its own role table had
  to reparse this crate's structs back into JSON or reimplement the mapping,
  which is how two consumers come to disagree about what a `_callable` is.
  `node_types_json` is `None` for a pack predating the export. Nothing is
  resolved: the parsed forms this crate uses itself are unchanged.
- `fetch-bytes`: acquisition without an engine. The manifest resolution, the
  content-addressed cache, the sha256 check and the stale-manifest fallback,
  handing back bytes -- for a host that already has its own wasm runtime and
  would otherwise link a second one to reuse any of it. Adds `fetch_bytes` and
  `fetch_pinned_bytes`. `fetch` is now `["fetch-bytes", "pack"]`, so it
  behaves exactly as before.

## [0.2.1] - 2026-08-29

### Fixed

- `Pack::expand_query` now drops facet members that cannot take a field the
  pattern constrains, which is what makes a field-constrained facet query
  compile at all: tree-sitter rejects a whole alternation if any one branch is
  impossible, so `(_callable name: (_) @n)` used to fail wherever a member had
  no `name`. Seven of the nine grammars rejected that query outright; all nine
  run it now. The evidence was already in the pack — `tb_node_types` has
  shipped since `pack_abi` 3 — and is read only when a query needs it, so
  loading a grammar is unchanged.
- `_` in a field's value pattern is the wildcard, not a node type named `_`.
  Read as a type it matched nothing any field declares, so filtering dropped
  every member and `(_callable name: (_) @n)` failed with "no member satisfies
  the field constraint". It constrains presence only, in an alternation too.
  Reachable only through `expand_with_types` before now.

## [0.2.0] - 2026-08-29

The first version to carry a runtime, and so the first with a dependency that
can carry a vulnerability. See Security.

### Added

- `Pack::fetch("python")` downloads a grammar, verifies it against the
  published sha256, and caches it. Using a grammar no longer needs a build
  step or a `curl`.
- `Pack::query` runs a query and returns its captures. Until now the shared
  vocabulary could be *expanded* but not *executed*: `expand_query` handed
  back a string and nothing in a pack could run it, so the one operation the
  vocabulary exists for was the one a consumer could not perform. Needs a pack
  built at `pack_abi` 3 or later.
- `Pack::fetch_pinned("python", "<hash>")` loads an exact version. It consults
  no manifest, so it is reproducible and works offline once the bytes are
  cached — which is what a build that must not vary should call.
- Compiled modules are cached, so loading a grammar is a few milliseconds
  rather than a few hundred. Release-build measurements: python 297ms cold
  against 1ms warm, C++ 362ms against 15ms. The artifact is keyed by the wasm
  bytes and the host, and wasmtime rejects one built by an incompatible
  version, so a stale entry is rebuilt rather than mis-loaded.
  `TREEBANK_NO_COMPILE_CACHE=1` disables it.
- A `fetch` feature, on by default and implying `pack`. Turn it off for a
  build that must not reach the network.

### Security

- `wasmtime` and `wasmtime-wasi` are held at 48, not 38. The versions this
  crate first used are covered by two advisories -- a miscompiled guest heap
  access that allows a sandbox escape (fixed in 42.0.2) and a Winch/aarch64
  issue (fixed in 44.0.2) -- and a sandbox escape is the whole of what a pack
  loader is trusting the runtime for. No release ever shipped the affected
  versions: they were replaced before 0.2.0 was tagged.
- Loading a grammar therefore needs Rust 1.95, which is wasmtime's floor
  rather than this crate's. `default-features = false` drops the runtime along
  with that floor, and still gives the vocabulary and query expansion.

### Changed

- The cache lives under `$TREEBANK_CACHE`, else `$XDG_CACHE_HOME/treebank`,
  else `~/.cache/treebank` — the same place this repository's own toolchain
  caches into, so a checkout and a consumer do not keep two copies.

## [0.1.0] - 2026-08-29

First release.

### Added

- The node vocabulary as code: the closed term lists, `roles.json` parsing,
  and the conformance checker every grammar is held to.
- `expand` for facet queries. `(_callable)` becomes whatever the loaded
  grammar calls its callables, so one query runs against several languages.
- `Pack`, a loader for the wasm grammar packs. It hosts a pack on wasmtime —
  a pack imports only WASI, so that is the whole host — and exposes kinds,
  byte ranges, named children, field names and the error flags.
- `Pack::expand_query`, which expands a facet query against the manifest the
  pack itself carries, so a portable query needs no `roles.json` beside the
  parser.

[Unreleased]: https://github.com/PowderworksCode/treebank/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/PowderworksCode/treebank/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/PowderworksCode/treebank/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/PowderworksCode/treebank/releases/tag/v0.1.0
