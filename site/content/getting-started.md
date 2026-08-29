---
title: Getting started
description: Use a grammar, or build the repository and run the gates.
order: 5
---

## Using a grammar

Each grammar is one WebAssembly file with no dependencies:

```sh
curl -O https://treebank.dev/packs/treebank-python.wasm
```

It imports only WASI, so it loads from Python, Go, Ruby, Rust or a browser
with no toolchain at the far end. The
[examples](https://github.com/PowderworksCode/treebank/tree/main/tools/wasm-pack/examples)
are complete bindings in about a hundred lines each, and the
[playground](/playground/) is the same file running in a browser.

`https://treebank.dev/packs/index.json` lists the current file for every
grammar with its sha256. Each is also available at a content-addressed URL,
`treebank-python-<hash>.wasm`, which never changes.

## Building the repository

Every grammar is a Rust crate that compiles its own `parser.c` and
`scanner.c`, so building the workspace builds all nine parsers.

```sh
git clone https://github.com/PowderworksCode/treebank
cd treebank
cargo build --workspace
cargo test --workspace
```

## Running the gates

```sh
./target/debug/treebank status --check
./target/debug/treebank verify --grammar crates/treebank-python
```

`status --check` prints the inventory: pass rates, gaps, test coverage, and
whether each grammar's evidence is current. `verify` runs every gate one
grammar must pass — reproducible generation, corpus tests, negative corpus,
vocabulary conformance and the rosetta suite.

A sweep needs the corpus the evidence was measured against:

```sh
./target/debug/treebank hydrate --lang python
./target/debug/treebank sweep --lang python --grammar crates/treebank-python
```
