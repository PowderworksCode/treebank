---
title: Getting started
description: Build the workspace, run the gates, read the inventory.
order: 5
---

Treebank is a Rust workspace. Every grammar is a real crate compiling its own
`parser.c` and `scanner.c`, so building the workspace builds all nine parsers.

```sh
git clone https://github.com/PowderworksCode/treebank
cd treebank
cargo build --workspace
cargo test --workspace
```

## The inventory

```sh
./target/debug/treebank status --check
```

One generated table joining registry configuration, ledgers, fixtures,
policies, locks and canaries. `--check` fails on missing or contradictory
required configuration; warnings remain visible without pretending optional
coverage is broken.

## Checking one grammar

```sh
./target/debug/treebank verify --grammar crates/treebank-python
```

`verify` runs every gate a grammar must pass: reproducible generation, the
grammar's own corpus tests, the negative corpus, vocabulary conformance and
the rosetta suite. The same gates run per grammar in CI, and the CI matrix is
derived from the checkout — a directory under `crates/` with a `grammar.js` in
it *is* a grammar — so a new one is gated the day it lands rather than the day
somebody remembers to add it to a list.

## Running a sweep

A sweep needs a corpus, and a corpus is pinned by a lock:

```sh
./target/debug/treebank hydrate --lang python
./target/debug/treebank sweep --lang python --grammar crates/treebank-python
```

`hydrate` recreates and verifies the exact corpus the lock names, so the sweep
measures what the committed evidence measured.
