Shape fixtures for java. `treebank shape` compares our node BOUNDARIES
against javac's over every file here, with a ceiling of ZERO missed
boundaries, and CI runs it on every push — which is where it earns its
keep, because CI has no corpus to sweep.

These files differ from the python and rust fixtures in what they are.
There, every file is a mis-parse that was found on the corpus and fixed.
Java's shape baseline was already zero across the full corpus —
116,920,180 oracle nodes over 243,385 files — before this directory
existed, so nothing here is a scar. Each file is a construct family the
grammar handles today and that nothing was pinning: annotations, non-ASCII
identifiers, records, lambdas, generics, and the four switch forms. The
point is the same either way. A tree that silently regroups is invisible
to the sweep, the negative corpus and the corpus tests alike, because all
three judge accept-or-reject and a wrong tree is still an accepted one.

Each file must parse cleanly. An ERROR node is not a shape finding, it is
a different bug, and a fixture that errors is measuring nothing.

Deliberately absent, so their absence is a decision and not an oversight:

- Identifiers containing a Unicode combining mark (`Mn`/`Mc`) —
  Devanagari `चूंकि`, Tamil `ஆனால்`, Kannada `ನೀಡಿದ`. The identifier rule
  omits both categories, so 53 corpus files do not parse cleanly at all.
  That is issue #196, and its fixtures belong with its fix.
- `module-info.java`. Java 9 modules are not in the grammar; see
  `known_gaps` in ledger.toml.

Add to this directory whenever `treebank shape` finds something on the
corpus and it gets fixed, and whenever a construct family lands with
nothing pinning its boundaries.
