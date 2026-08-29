# treebank

Parse code with any [Treebank](https://treebank.dev) grammar, and share one
vocabulary across all of them.

There is one crate rather than one per language. A grammar is a WebAssembly
file fetched at runtime, so adding a language is a download rather than a
dependency:

```rust
use treebank::Pack;

let pack = Pack::from_path("treebank-python.wasm")?;
let tree = pack.parse("def f(x):\n    return x + 1\n")?;
println!("{}", tree.root().sexp()?);
# Ok::<(), anyhow::Error>(())
```

Grammars are at `https://treebank.dev/packs/`, listed with their hashes in
[`index.json`](https://treebank.dev/packs/index.json). Each is also served at
a content-addressed URL that never changes.

## What is in here

- **`pack`** — load a `.wasm` grammar and parse with it. Needs a WASI runtime,
  which this crate brings via `wasmtime`. Turn off the `pack` feature if you
  only want the vocabulary.
- **`expand`** — facet queries. `(_callable)` becomes whatever the loaded
  grammar calls its callables, so one query runs against several languages.
- **`roles`, `node_types`, `check`** — the vocabulary itself, and the
  conformance checker that keeps every grammar honest about it.

MIT licensed. Part of [Powderworks](https://powderworks.dev).
