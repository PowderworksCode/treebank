---
title: Playground
description: Parse code with a real Treebank grammar, in your browser.
order: 7
---

Pick a grammar, paste some code, and watch it parse. Node types link into the
[grammar reference](/grammars/), so you can click a `function_definition` in
your own code and read the production that admitted it.

The box under the panes runs queries against what you just parsed. Write
`(_callable) @c` and it matches whatever that language calls a callable — a
`function_definition` in Python, an `arrow_function` in TypeScript, a
`closure_expression` in Rust as well as an `fn`. Switch the grammar and the
same query keeps working, which is the whole point of a shared vocabulary.

The parser is the same `.wasm` file a program would download — nothing here is
a reimplementation for the web.

<link rel="stylesheet" href="/grammar.css">
<div class="playground"><p class="dim">Loading…</p></div>
<script type="module" src="/playground.mjs"></script>

## Queries, and what expansion is for

A **supertype** like `_declaration` is a real rule in the parse table, so
tree-sitter matches it natively. A **nominal** term like `_callable` is a list
in the grammar's `terms.json` — it cross-cuts derivations and cannot be a
rule — so
it is rewritten into an alternation before the query runs:

```
(_callable) @c   ->   [(function_definition) (lambda)] @c
```

Expand the line above the results to see what your query became. The rewrite
happens here in the browser, and the crate does exactly the same thing in
`Pack::query`; a differential test runs both over every grammar's terms and
fails on any difference, because a query that means two things is worse than
one that fails.

A pattern with a field constraint needs one more step. tree-sitter rejects a
whole alternation if any one branch is impossible, so `(_callable name: (_)
@n)` would die in Python on account of `lambda`, which has no `name`. Members
that cannot take the field are therefore dropped before the query runs — the
expansion shows `[(function_definition name: (_) @n)]` and no `lambda`. Every
pack carries `node-types.json` inside it, so the browser knows which fields
each node type declares without downloading anything else.

## Using the same file

From Rust, the same grammar is two lines and no download to manage:

```rust
let pack = Pack::fetch("python")?;
let tree = pack.parse(source)?;
for capture in pack.query(&tree, "(_callable) @c")? {
    println!("{} {:?}", capture.kind, capture.range);
}
```

Each grammar is one WebAssembly module importing only WASI — six calls, all
file-descriptor stubs the parse path never reaches. There is no
web-tree-sitter and no emscripten glue, so a binding is about twenty lines in
any language with a WASI runtime. [Using a grammar](/integrate/) has both, and
the module URLs.

The module answers for itself: `tb_provenance()` returns which grammar, which
vocabulary and what the last sweep measured, and `tb_terms()` returns the
nominal manifest a query needs. Both come out of the file, so a copy found on
disk years from now still says what it is.

Files are content-addressed. The link under the panes names the exact parser
you are using, and that URL will not change.
