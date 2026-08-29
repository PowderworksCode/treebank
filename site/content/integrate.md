---
title: Using a grammar
description: From nothing to a parsed tree, in Rust or any language with a WASI runtime.
order: 6
---

There is one package per *language you write in*, not one per language you
want to parse. A grammar is a file you fetch, so adding Zig support to your
tool is a download rather than a dependency.

That is why there is no `treebank-python` crate and never will be. Nine
grammars today and many more later would mean nine version numbers to keep in
step, and a release of your tool every time any of them moved.

## The whole thing, in Rust

Two commands and eight lines. Nothing is elided.

```sh
cargo add treebank
curl -O https://treebank.dev/packs/treebank-python.wasm
```

```rust
use treebank::Pack;

fn main() -> anyhow::Result<()> {
    let pack = Pack::from_path("treebank-python.wasm")?;
    let tree = pack.parse("def greet(name):\n    return f'hi {name}'\n")?;

    println!("{}", tree.root().sexp()?);
    Ok(())
}
```

```
(module (function_definition name: (identifier) parameters: (parameters
  (parameter name: (identifier))) body: (block (return_statement (string
  (string_start) (string_content) (interpolation expression: (identifier))
  (string_end))))))
```

That is the entire integration. The `.wasm` is data your program reads —
ship it beside your binary, embed it with `include_bytes!`, or fetch it at
startup with [`Pack::from_bytes`](https://docs.rs/treebank/latest/treebank/pack/struct.Pack.html).

## Walking the tree

```rust
let tree = pack.parse(source)?;
let root = tree.root();

for child in root.named_children()? {
    println!("{} {:?}", child.kind()?, child.byte_range()?);
}
```

Field names are the edge labels a query uses, and they belong to the parent's
view of a child rather than to the child:

```rust
for i in 0..node.child_count(false)? {
    if let Some(field) = node.field_name_for_child(i)? {
        println!("{field}");        // name, parameters, body, …
    }
}
```

## Finding the mistakes

`has_error` is a flag on the node rather than a walk, so checking whether a
file parsed cleanly is cheap:

```rust
if tree.root().has_error()? {
    // something in here is an ERROR or a MISSING node
}
```

`is_error` distinguishes the node itself. Walk with `child_count(false)`
rather than `named_children` when hunting them: a `MISSING` node is usually
anonymous, and named-only traversal skips exactly what you are looking for.

## Queries that work across languages

Every grammar carries the same vocabulary, so one query can run against
several. Some roles are real supertypes and queryable directly; others are
*facets*, which are lists that must be expanded first:

```rust
let query = pack.expand_query("(_callable)")?;
// python -> [(function_definition) (lambda)]
// rust   -> [(function_definition) (closure_expression)]
```

The expansion uses the manifest the pack carries, so nothing has to be
shipped beside the parser. [The vocabulary](/concepts/two-tiers/) explains why
there are two kinds.

## Without a parser

The `pack` feature is on by default and brings a WASI runtime with it. If you
only want the vocabulary and the query expansion:

```toml
treebank = { version = "0.1", default-features = false }
```

## Any other language

A grammar imports **only WASI** — six file-descriptor calls, none of which the
parse path reaches. There is no emscripten glue and no `web-tree-sitter`, so a
binding is short anywhere with a WASI runtime.

Two complete ones are in the repository, and are the reference the others were
written from:

- [`parse.py`](https://github.com/PowderworksCode/treebank/blob/main/tools/wasm-pack/examples/parse.py) — Python, via `wasmtime`
- [`parse.mjs`](https://github.com/PowderworksCode/treebank/blob/main/tools/wasm-pack/examples/parse.mjs) — Node, via `node:wasi`

In a browser the six imports can be written out by hand. The
[playground](/playground/) does exactly that, in about twenty lines with no
dependency at all.

## Which file to fetch

Two URLs for every grammar:

| | |
| --- | --- |
| `/packs/treebank-python.wasm` | the current grammar; moves when the grammar does |
| `/packs/treebank-python-<hash>.wasm` | those exact bytes, forever |

[`/packs/index.json`](/packs/index.json) lists the current file and sha256 for
each grammar, so a build can resolve and verify one without hard-coding a
hash:

```json
{ "packs": { "python": { "sha256": "…", "key": "treebank-python-<hash>.wasm" } } }
```

Packs are byte-reproducible, so a hash is a property of the grammar rather
than of the machine that built it — `tools/wasm-pack/build.sh` on your laptop
produces the same bytes CI published. Pin the hashed URL if you need a parser
that cannot change under you, and quote the hash if you report a bad parse.
