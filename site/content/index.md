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

## Try one now

The [playground](/playground/) parses in your browser with the same file a
program would download. Paste some code and watch the tree.

<div class="clear-cover"></div>

## Use one

Each grammar is one `.wasm` that imports only WASI. It runs from Python, Go,
Ruby, Rust or a browser with no toolchain at the far end:

```sh
curl -O https://treebank.dev/packs/treebank-python.wasm
```

From Rust that is `cargo add treebank` and three lines. From anywhere else it
is a WASI runtime and about twenty lines — [Using a grammar](/integrate/) has
both, and the complete examples.

There is one package per language you write in, not one per language you
parse. Every file is content-addressed, so `treebank-python-<hash>.wasm` is a
version you can pin and never see change.

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
