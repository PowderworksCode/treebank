# treebank-scala local patches

Upstream: [tree-sitter/tree-sitter-scala](https://github.com/tree-sitter/tree-sitter-scala)
at `b931fcc` (v0.26.2), the grammar's official home in the `tree-sitter` org.

Patches apply in order onto that commit; `ledger.json` carries the evidence.

## 0001 — treebank redistribution notice (packaging)

Prepends the standard warning to `README.md` so anyone who meets a
materialized or published copy knows it is a patched redistribution and where
to report problems. Touches no grammar code.

## 0002 — treebank crate identity (packaging)

Upstream owns `tree-sitter-scala` on crates.io, so the published crate is
renamed `treebank-grammar-scala` with our `repository`, `homepage` and
`description`, and `include` is extended so `ledger.json`, `LOCAL-PATCHES.md`
and `patches/` travel inside the tarball. `[lib] name` stays
`tree_sitter_scala`, so the crate remains a drop-in replacement.

## 0003 — Scala 2 `enum` as a term name

`enum` is a Scala 3 keyword and an ordinary Scala 2 term name, so Scala 2
sources both declare and use it:

```scala
class EnumValueSerializer[E](val enum: E)
def qualifyEnum(enum: Enum[_]): String = enum.getClass.getCanonicalName
```

Upstream already makes this allowance for import paths — `_namespace_path_segment`
carries the comment *"Scala 3 keywords that are valid identifiers in Scala 2
sources"* — but only there, so a parameter called `enum`, and every use of it,
failed to parse. This patch extends the same allowance to parameter names
(`_param_name`, inlined so it does not become a distinct symbol) and to
expression position, and declares the one conflict that creates: after `enum`,
an identifier is either the enum's name or a Scala 2 postfix application of a
value called `enum`, and only the rest of the construct settles it.

`export`, the other half of upstream's pair, is deliberately **not** included.
No corpus file uses it as a term, and admitting it in expression position costs
a second GLR conflict against `export_declaration` — symmetry is not evidence.

First seen in `org.apache.flink:flink-scala_2.12` 1.20.5. Six corpus files, all
Apache Flink; sweep 7560/29 → 7566/23.
