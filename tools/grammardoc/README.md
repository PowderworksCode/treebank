# grammardoc

Renders a treebank grammar the way a language manual renders one: every
production as EBNF and as a railroad diagram, plus the precedence table and
the vocabulary index.

```sh
python3 tools/grammardoc/emit.py crates/treebank-python /tmp/python.html
python3 tools/grammardoc/emit.py --check crates/treebank-python   # what CI runs
```

No dependencies beyond the standard library. Roughly 70 ms per grammar.

## Why it is small

The input is **`src/grammar.json`**, not `grammar.js`. `grammar.js` is
arbitrary JavaScript and reading it means running it; `grammar.json` is what
`tree-sitter generate` normalises it into, and it is already an EBNF syntax
tree over sixteen node kinds — `SEQ`, `CHOICE`, `REPEAT`, `REPEAT1`,
`SYMBOL`, `STRING`, `PATTERN`, `BLANK`, `FIELD`, `ALIAS`, `TOKEN`,
`IMMEDIATE_TOKEN`, `RESERVED`, and the four `PREC` forms.

So rendering is a fold over those sixteen cases, not a parse. There is no
per-language code: the same fold produces Python, Rust and TypeScript.

Because it reads the generated grammar rather than the source, the page
cannot drift from the parser. If a production is on the page, the parse
table has it.

## Files

| | |
|---|---|
| `grammardoc.py` | loads the grammar; folds it to EBNF and to railroad nodes |
| `railroad.py` | the layout engine — every node reports width, and height above and below its entry line |
| `style.py` | palette, embedded faces, stylesheet |
| `emit.py` | assembles the page; `--check` is the CI smoke test |
| `preview.py` | rasterises one diagram offline, for eyeballing layout |

## Decisions worth knowing

**Hidden rules stay visible.** It is tempting to inline `_or_test` and
friends, but that chain *is* the precedence structure — the same thing the
MySQL manual shows as `expr → boolean_primary → predicate → bit_expr →
simple_expr`. Inlining it deletes the most informative part of the page.

**The list idiom is collapsed.** `seq(X, repeat(seq(',', X)))` is recognised
and drawn as one loop rather than a chain of five boxes. Without it about
half the diagrams are unreadable.

**Precedence gets its own table, and is also drawn in place.** EBNF cannot
express precedence at all, which is why every language manual prints it
separately. Levels are also drawn around the production they apply to, so
you do not have to cross-reference.

**The monospace face is embedded, and that is not decoration.** The SVG box
widths are computed here, from a fixed character advance (DejaVu Sans Mono,
0.60205 em). A font fallback in the browser would clip every label. It costs
about 1.3 MB per page; pass `--no-fonts` to drop it when the page will be
read somewhere the face is already available.

## Verifying a layout change

`railroad.py` sizes a node in its constructor and draws it later, and those
two must agree — the two bugs found while writing it were both a `choice`
whose branches were computed with one formula and drawn with another, which
reads as plausible source and as obvious nonsense in a picture. So the
offsets are computed once, in `__init__`, and `draw` reads them back.

After changing the engine, rasterise something with nesting in it:

```sh
python3 tools/grammardoc/preview.py crates/treebank-python /tmp _params_star_section
convert -density 140 /tmp/_params_star_section.svg /tmp/out.png
```
