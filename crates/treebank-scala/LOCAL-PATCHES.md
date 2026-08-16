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

## 0004 — Scala 2 names that Scala 3 made keywords

Patch 0003's problem in the two positions it did not reach:

```scala
val enum = enumOf(obj)                                    // chill, Flink
@JsonSerialize(using = classOf[ExecutorMetricsJsonSerializer])   // Spark, Delta
```

`enum` also gets *bound*, not just declared and used, which goes through
`_definition_pattern`. And `using` is the same story with a different keyword:
the Scala 3 `(using expr)` clause claims the token, so the `=` of a named
argument has nowhere to go. That half needs a declared conflict, because
`(using x)` and `(using = x)` are told apart by a token an arbitrary
expression away.

`extension` as a pattern name (`case extension: CatalogExtension =>`) is the
same family and is deliberately **not** here: it is already a scanner-lexed
soft identifier, so aliasing does not apply, and the conflict it needs against
`extension_definition` cascaded.

Five files; sweep 17608/57 → 17613/52.

## 0005 — A newline ends a bare Scala 2 declaration (scanner)

The largest cluster in the corpus, and not what it looks like. Scala 2
procedure syntax is **not** unsupported — `def f(t: T) {}` parses, and so does
a lone `def f(t: T)` as a template's last member. What failed was a bare
declaration with *any* member after it:

```scala
trait A {
  def completeExceptionally(t: Throwable)
  val x = 1                                   // <- did not parse
}
```

Scala lets a `def` continue onto the next line in three places — before
another parameter list, before the result type, before the body — and a bare
declaration *ends* at that same newline. Both readings were live at one token;
the grammar preferred continuing, so the ending reading was unreachable.

The parser cannot decide this, because what follows may be an arbitrary
distance away. Every grammar-level attempt failed: declaring the conflict is
reported unnecessary (precedence already resolved it), swapping `prec.right`
for `prec.dynamic` moves the failure to `def this` and cascades, and deleting
the repeat's optional semicolon breaks upstream's own tests 54 and 55 — which
is how its load-bearing role was confirmed.

The **lexer** can decide it, because the answer is the very next character:
only `(`, `[`, `:`, `=` or `{` continues a def. So the continuation gets its
own external token, `DEF_CONTINUATION`, and the ending newline stays an
ordinary `AUTOMATIC_SEMICOLON`. The two can no longer be confused, and the
grammar needs no conflict at all. `valid_symbols` keeps the blast radius to
def signatures and bodies; four new negative-corpus files probe the change
from the other side.

Eleven files; sweep 17613/52 → 17624/41.
