---
title: Playground
description: Parse something with a real treebank grammar, in your browser.
order: 7
---

The parser below is the same artifact a consumer downloads: one wasm pack,
byte-reproducible, carrying its own provenance. Nothing is re-implemented for
the web and nothing is approximated. If this page and the `treebank` CLI
disagree about a tree, one of them is wrong.

Node types link into the grammar reference — see a `function_definition` in
your own code, click it, and read the production that admitted it.

<link rel="stylesheet" href="/grammar.css">
<div class="playground"><p class="dim">Loading…</p></div>
<script type="module" src="/playground.mjs"></script>

## How it loads

A pack imports **only WASI**, and only six calls of it — all file-descriptor
stubs the parse path never reaches. So the entire browser host is about twenty
lines, with no dependency: `tree-sitter build --wasm` emits an emscripten side
module that only web-tree-sitter can load, and packs exist precisely so a
consumer does not have to be web-tree-sitter.

The same module answers for itself through `tb_provenance()` and `tb_roles()`,
which is where the line under the panes comes from — not from a caption
written beside it.
