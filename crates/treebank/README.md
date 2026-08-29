# treebank

Parse code with any [Treebank](https://treebank.dev) grammar — bash, C, C++,
Java, Python, Ruby, Rust, TypeScript, Zig — and query them all with one shared
vocabulary.

```sh
cargo add treebank
```

```rust
use treebank::Pack;

let pack = Pack::fetch("python")?;
let tree = pack.parse("def greet(name):\n    return f'hi {name}'\n")?;

println!("{}", tree.root().sexp()?);
# Ok::<(), anyhow::Error>(())
```

That is the whole integration. `fetch` downloads the grammar, verifies it
against the published sha256, and caches it — so it happens once, and a
substituted or corrupted download is an error rather than a strange parse.

## One crate, not one per language

There are nine grammars and there will be more. A crate for each would mean
choosing versions for each, and releasing your tool every time any of them
moved. A grammar is a WebAssembly file instead, so adding a language is a
download rather than a dependency.

```rust
for name in ["python", "rust", "typescript"] {
    let pack = Pack::fetch(name)?;
    println!("{} {}", name, pack.parse(source)?.root().kind()?);
}
# Ok::<(), anyhow::Error>(())
```

## Pinning

`fetch` follows a grammar as it improves. Where a build must not vary, name the
version — no manifest is consulted, so it is reproducible and works offline
once cached:

```rust
let pack = Pack::fetch_pinned("python", "d82f4fd5c5a9")?;
# Ok::<(), anyhow::Error>(())
```

Packs are byte-reproducible, so that hash is a property of the grammar rather
than of the machine that built it.

## Walking the tree

```rust
let tree = pack.parse(source)?;
let root = tree.root();

for child in root.named_children()? {
    println!("{} {:?}", child.kind()?, child.byte_range()?);
}

if root.has_error()? {
    // an ERROR or MISSING node is somewhere below
}
# Ok::<(), anyhow::Error>(())
```

Field names — the edge labels a query uses — belong to the parent's view of a
child, so they are asked for there:

```rust
for i in 0..node.child_count(false)? {
    if let Some(field) = node.field_name_for_child(i)? {
        println!("{field}");   // name, parameters, body, …
    }
}
# Ok::<(), anyhow::Error>(())
```

## One query, several languages

This is the reason the grammars are written rather than collected. Every one
carries the same vocabulary, so a query written once runs against all of them
and finds whatever that language calls the thing. The sources differ, because
they must; the query does not:

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
# Ok::<(), anyhow::Error>(())
```

```
python      function_definition, class_definition
rust        function_definition, struct_definition
typescript  function_definition, class_definition
```

`_declaration` is a **supertype** — a real rule threaded through the
productions, so the match is by derivation rather than by node name.
`_callable`, `_binding`, `_scope` and `_clause` are **facets**: lists that
cross-cut derivations and cannot be supertypes, so `query` expands them
against the manifest each pack carries before running. Where the pattern
constrains a field, members that cannot take it are dropped — `(_callable
name: (_) @n)` keeps `function_definition` and not `lambda`, because
tree-sitter rejects an alternation with one impossible branch in it. Either
way you write the same query.

`expand_query` returns the rewritten query without running it, if you have
your own query engine.

## Speed

A grammar is compiled on first load and the compiled form is cached, so later
loads are a few milliseconds. Release-build figures:

| | cold | warm |
| --- | --- | --- |
| python, 673 KB | 297 ms | 1 ms |
| C++, 5.0 MB | 362 ms | 15 ms |

Parsing is well under a millisecond, so this is the whole startup cost. Measure
it in a **release** build: in debug, cranelift is unoptimised and the same load
takes about four seconds.

`TREEBANK_CACHE` moves the cache, `TREEBANK_NO_COMPILE_CACHE=1` disables it,
and `TREEBANK_PACKS_URL` points at a mirror.

## Features

Both are on by default.

| | |
| --- | --- |
| `pack` | load and parse with a grammar; brings a WASI runtime (`wasmer`) |
| `fetch` | download grammars; implies `pack` |

`pack` brings wasmer, which needs Rust 1.95. That floor is the runtime's, not
this crate's, and turning the runtime off removes it.

wasmer rather than wasmtime because of cross-compilation: wasmtime's build
script compiles C helpers in every configuration that can execute wasm, so
cross-compiling it to musl needs a musl C cross-compiler on `PATH`. wasmer's
tree compiles no C, so a consumer shipping static musl binaries can host a pack
from a plain `cargo build`.

For a build that must not reach the network, keep `pack` and hand the bytes in
yourself with `Pack::from_path` or `Pack::from_bytes`:

```toml
treebank = { version = "0.2", default-features = false, features = ["pack"] }
```

For the vocabulary and query expansion alone, with no runtime:

```toml
treebank = { version = "0.2", default-features = false }
```

## Also in here

`expand` for facet queries, `roles` and `node_types` for the vocabulary, and
`check` — the conformance checker every Treebank grammar is held to.

MIT licensed. Part of [Powderworks](https://powderworks.dev).
[Documentation](https://treebank.dev) · [Changelog](https://github.com/PowderworksCode/treebank/blob/main/CHANGELOG.md)
