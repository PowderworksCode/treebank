# Changelog

Notable changes to the `treebank` crate. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The grammars are not versioned here. They ship as wasm packs, published
continuously and addressed by content, so a grammar improving reaches
consumers without a release — see [treebank.dev/packs](https://treebank.dev/packs/index.json).
Only the Rust API is versioned.

## [Unreleased]

### Added

- `Pack::query` runs a query and returns its captures. Until now the shared
  vocabulary could be *expanded* but not *executed*: `expand_query` handed
  back a string and nothing in a pack could run it, so the one operation the
  vocabulary exists for was the one a consumer could not perform. Needs a pack
  built at `pack_abi` 3 or later.

## [0.2.0] - 2026-08-29

### Added

- `Pack::fetch("python")` downloads a grammar, verifies it against the
  published sha256, and caches it. Using a grammar no longer needs a build
  step or a `curl`.
- `Pack::fetch_pinned("python", "<hash>")` loads an exact version. It consults
  no manifest, so it is reproducible and works offline once the bytes are
  cached — which is what a build that must not vary should call.
- Compiled modules are cached, so loading a grammar is a few milliseconds
  rather than a few hundred. Release-build measurements: python 296ms cold
  against 4ms warm, C++ 370ms against 25ms. The artifact is keyed by the wasm
  bytes and the host, and wasmtime rejects one built by an incompatible
  version, so a stale entry is rebuilt rather than mis-loaded.
  `TREEBANK_NO_COMPILE_CACHE=1` disables it.
- A `fetch` feature, on by default and implying `pack`. Turn it off for a
  build that must not reach the network.

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

[Unreleased]: https://github.com/PowderworksCode/treebank/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/PowderworksCode/treebank/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/PowderworksCode/treebank/releases/tag/v0.1.0
