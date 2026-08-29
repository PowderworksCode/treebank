---
title: Using a grammar
description: Load a grammar and parse with it, from Rust or any language with a WASI runtime.
order: 6
---

There is one package per *language you write in*, not one per language you
want to parse. A grammar is a file you fetch, so adding Zig support to your
tool is a download rather than a dependency.

That is why there is no `treebank-python` crate and never will be: nine
grammars today and many more later would mean nine version numbers for a
consumer to keep in step, and a release of your tool every time any of them
moved.

## Rust

```sh
cargo add treebank
```

```rust
use treebank::Pack;

let pack = Pack::from_path("treebank-python.wasm")?;
let tree = pack.parse("def f(x):\n    return x + 1\n")?;

let root = tree.root();
println!("{}", root.kind()?);        // module
println!("{}", root.sexp()?);        // (module (function_definition ...
println!("{:?}", root.has_error()?); // false
```

Walking the tree, with the field names a query would use:

```rust
for child in root.named_children()? {
    println!("{} {:?}", child.kind()?, child.byte_range()?);
}

let f = &root.named_children()?[0];
for i in 0..f.child_count(false)? {
    if let Some(name) = f.field_name_for_child(i)? {
        println!("field {name}");
    }
}
```

The `pack` feature is on by default and brings a WASI runtime with it. Turn it
off with `default-features = false` if you only want the vocabulary.

## Any other language

A grammar is one WebAssembly module that imports **only WASI** — six
file-descriptor calls, none of which the parse path reaches. There is no
emscripten glue and no `web-tree-sitter`, so a binding is short in any
language with a WASI runtime:

```sh
curl -O https://treebank.dev/packs/treebank-python.wasm
```

Two complete bindings are in the repository and are the reference the others
were written from:

- [`parse.py`](https://github.com/PowderworksCode/treebank/blob/main/tools/wasm-pack/examples/parse.py) — Python, via `wasmtime`
- [`parse.mjs`](https://github.com/PowderworksCode/treebank/blob/main/tools/wasm-pack/examples/parse.mjs) — Node, via `node:wasi`

In a browser the six imports can be written out by hand — the
[playground](/playground/) does exactly that, in about twenty lines with no
dependency at all.

## Which file to fetch

[`/packs/index.json`](/packs/index.json) lists the current file for every
grammar with its sha256:

```json
{ "packs": { "python": { "sha256": "…", "key": "treebank-python-<hash>.wasm" } } }
```

Two URLs for each grammar:

| | |
| --- | --- |
| `/packs/treebank-python.wasm` | the current grammar; moves when the grammar does |
| `/packs/treebank-python-<hash>.wasm` | those exact bytes, forever |

Packs are byte-reproducible, so the hash is a property of the grammar rather
than of the machine that built it. Pin the hashed URL if you need a parser
that cannot change under you — and if you report a bug, the hash is the most
useful thing to include.

## What a pack knows about itself

Provenance and the facet manifest travel inside the module, so a file found on
disk years from now still answers:

```rust
let p = pack.provenance();
println!("{}, vocabulary {}", p.language, p.vocabulary);
```

The manifest matters if you write queries. Treebank threads a shared
vocabulary through every grammar, so `(_declaration)` finds declarations in
Rust and in Java. Some roles are real supertypes and queryable directly;
others are *facets*, which are lists that have to be expanded before a query
runs:

```rust
let query = pack.expand_query("(_callable)")?;
// -> [(function_definition) (lambda)]  for python
```

That expansion is against the manifest the pack carries, so nothing has to be
shipped beside the parser. The [vocabulary page](/concepts/two-tiers/)
explains why there are two kinds.
