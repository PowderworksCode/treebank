---
title: Treebank
description: A growing collection of well-tested Tree-sitter grammars
---

<style>
/* The shared theme sizes a cover for an engraving beside a paragraph, at
   16rem. This plate is fifteen trees to scale and the whole point of it is
   the comparison, which does not survive being shrunk to a thumbnail. Scoped
   to this page rather than pushed into the theme, because the theme's default
   is right for the sites using it. */
.cover-wide { max-width: 52%; }
.cover-wide img { max-width: 26rem; }
@media (max-width: 60rem) {
  .cover-wide { float: none; max-width: 100%; margin: .5rem 0 1.5rem; }
  .cover-wide img { max-width: 100%; }
}
/* The theme clears floats at every h2, which is right for a page whose
   opening runs longer than its picture. This one does not, so clearing left
   160px of nothing between the intro and the first heading. Let the short
   sections run beside the plate instead, and clear once, where the code block
   starts -- a fenced block squeezed into the remaining column is worse than
   the gap was. */
main h2 { clear: none; }
.clear-cover { clear: both; }
</style>

<p class="cover cover-wide"><img src="/cover.png" alt="A plate of fifteen trees drawn to scale, each with a small figure beside it for size"></p>

Treebank maintains nine Tree-sitter grammars — bash, C, C++, Java, Python,
Ruby, Rust, TypeScript and Zig — and runs each of them over hundreds to
thousands of the top packages and libraries for its language, checking the
result against that language's own compiler or parser.

Every grammar ships as a single WebAssembly file with no dependencies, and
every measurement is published, including the failures.

Try one in the [playground](/playground/) — it parses in your browser with the
same file a program would download.

To use one, add the crate and ask for a grammar by name:

```sh
cargo add treebank
```

```rust
use treebank::Pack;

let pack = Pack::fetch("python")?;
let tree = pack.parse(source)?;

for capture in pack.query(&tree, "(_declaration) @decl")? {
    println!("{} {:?}", capture.kind, capture.range);
}
```

`fetch` downloads the grammar, checks it against the published sha256 and
caches it. The query finds declarations in Rust and TypeScript too, unchanged,
because every grammar carries the same vocabulary — so there is one package
per language you write in rather than one per language you parse, and adding a
language is a download rather than a dependency.

Each grammar is also just one `.wasm` importing only WASI, so it runs from
Python, Go, Ruby or a browser in about twenty lines with no toolchain at the
far end. [Using a grammar](/integrate/) has all of it, and every file is
content-addressed, so a version can be pinned and never change.

## What is here

**[Grammar reference](/grammars/)** — every production in all nine grammars,
as EBNF and as a railroad diagram, generated from the parse table itself.
Each page also carries that grammar's current pass rate, its known gaps, and
what it declares about itself.

**[How it works](/concepts/)** — the corpus, the reference parsers, and what
gets measured beyond a pass rate.

**[Reference](/reference/)** — the CLI, and the gates a grammar has to pass.

## Why it is built this way

A grammar is easy to test in one direction: throw valid code at it and count
what parses. That number goes up while two others stay invisible — whether
the grammar rejects what the language rejects, and whether the tree it builds
is the right one.

Treebank measures all three separately, against the language's own toolchain,
over a corpus pinned by a lockfile so a number can be reproduced later. Where
a grammar is wrong, it says so on the page for that grammar.
