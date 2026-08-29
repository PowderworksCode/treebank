---
title: Playground
description: Parse code with a real Treebank grammar, in your browser.
order: 7
---

Pick a grammar, paste some code, and watch it parse. Node types link into the
[grammar reference](/grammars/), so you can click a `function_definition` in
your own code and read the production that admitted it.

The parser is the same `.wasm` file a program would download — nothing here is
a reimplementation for the web.

<link rel="stylesheet" href="/grammar.css">
<div class="playground"><p class="dim">Loading…</p></div>
<script type="module" src="/playground.mjs"></script>

## Using the same file

Each grammar is one WebAssembly module that imports only WASI — six calls, all
file-descriptor stubs the parse path never reaches. There is no
web-tree-sitter and no emscripten glue, so a binding is about twenty lines in
any language with a WASI runtime.

```sh
curl -O https://treebank.dev/packs/treebank-python.wasm
```

The module answers for itself: `tb_provenance()` returns which grammar, which
vocabulary and what the last sweep measured, and `tb_roles()` returns the
facet manifest a query needs. Both come out of the file, so a copy found on
disk years from now still says what it is.

Files are content-addressed. The link under the panes names the exact parser
you are using, and that URL will not change.
