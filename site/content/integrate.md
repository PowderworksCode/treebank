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

One command and seven lines. Nothing is elided.

```sh
cargo add treebank
```

```rust
use treebank::Pack;

fn main() -> anyhow::Result<()> {
    let pack = Pack::fetch("python")?;
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

That is the entire integration. `fetch` downloads the grammar, checks it
against the sha256 the manifest publishes, and caches it — so it happens once
rather than on every run, and a substituted or corrupted download is an error
rather than a strange parse later.

If you would rather hold the file yourself — vendored, embedded with
`include_bytes!`, or shipped beside your binary — `Pack::from_path` and
`Pack::from_bytes` take it directly and never reach the network.

### Pinning

`fetch` follows the grammar as it improves. Where that must not happen, name
the version:

```rust
let pack = Pack::fetch_pinned("python", "d82f4fd5c5a9")?;
```

That consults no manifest, so it is reproducible and works offline once the
bytes are cached. It is also what the playground's permalink names, which is
why a hash is the useful thing to quote in a bug report.

### Speed

A grammar is compiled the first time it is loaded and the compiled form is
cached, so later loads are a few milliseconds. Nothing is needed to enable
this.

| | cold | warm |
| --- | --- | --- |
| python, 673 KB | 297 ms | 1 ms |
| C++, 5.0 MB | 362 ms | 15 ms |

Parsing itself is well under a millisecond, so this is the whole startup cost.
Measure it in a **release** build: in a debug build cranelift is unoptimised
and the same load takes about four seconds, which says nothing about what your
users will see. `TREEBANK_NO_COMPILE_CACHE=1` turns the cache off and
`TREEBANK_CACHE` moves it.

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
*facets*, which are lists that must be expanded first. The sources differ,
because they must; the query does not:

```rust
let sources = [
    ("python", "def f(): pass\nclass C: pass\n"),
    ("rust", "fn f() {}\nstruct S;\n"),
    ("typescript", "function f() {}\nclass C {}\n"),
];

for (lang, source) in sources {
    let pack = Pack::fetch(lang)?;
    let tree = pack.parse(source)?;
    for capture in pack.query(&tree, "(_declaration) @decl")? {
        println!("{lang} {} {:?}", capture.kind, capture.range);
    }
}
```

```
python      function_definition, class_definition
rust        function_definition, struct_definition
typescript  function_definition, class_definition
```

`_declaration` is a supertype, matched by derivation rather than by node name.
`_callable`, `_binding`, `_scope` and `_clause` are facets — lists that
cross-cut derivations — and `query` expands them against the manifest the pack
carries before running, so nothing has to be shipped beside the parser.
Either way you write the same query.

Where a pattern constrains a field, members that cannot take that field are
dropped from the alternation: `(_callable name: (_) @n)` keeps
`function_definition` and not `lambda`. Without that the query would not
compile at all, because tree-sitter rejects an alternation with one impossible
branch. The pack carries `node-types.json`, so this needs nothing beside the
parser either.

`expand_query` returns the rewritten query without running it, for a caller
with its own query engine. [The vocabulary](/concepts/two-tiers/) explains why
there are two kinds.

## Features

Both are on by default.

| feature | |
| --- | --- |
| `pack` | load and parse with a grammar; brings a WASI runtime |
| `fetch` | download grammars; implies `pack` |

`pack` brings wasmtime, which needs Rust 1.95. That floor is wasmtime's rather
than this crate's, and it goes away with the runtime. It is kept current
deliberately: a pack is code from the network, and the sandbox running it is
the thing standing between a grammar and the machine.

For a build that must not reach the network, keep `pack` and drop `fetch`:

```toml
treebank = { version = "0.2", default-features = false, features = ["pack"] }
```

For the vocabulary and query expansion alone, with no runtime:

```toml
treebank = { version = "0.2", default-features = false }
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
