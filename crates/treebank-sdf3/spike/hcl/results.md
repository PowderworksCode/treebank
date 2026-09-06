# hcl: the shipped grammar, rewritten in SDF3, held to everything it is held to

The module is `hcl.sdf3` (275 lines); the reference is `crates/treebank-hcl`
(grammar.js, 64 rules, and a 479-line hand-written scanner). Every gate
below runs the same code the shipped crate's CI runs, over the same
fixtures and the same locked corpus (`corpus-locks/hcl.json`, 499 packages,
19,219 files). `verify.sh` reproduces the lot.

## The tree-sitter lowering, against the reference

| gate | reference (`crates/treebank-hcl`) | this module (`spike/hcl`) |
|---|---|---|
| `tree-sitter test`, the crate's own corpus | 17 / 17 | 17 / 17 |
| `treebank negative`, the crate's negative corpus | 28 / 28 rejected | 27 / 28 rejected |
| `treebank roles` | 12 table terms as supertypes, 6 facets, 44 named nodes, 11 uncategorised | 12, 6, 44, 11 -- the same lists |
| `treebank lint`: declared conflicts / dynamic weights / same-text tokens | 0 / 0 / 0 | 0 / 0 / 0 |
| `treebank lint`: unreserved keyword-shaped tokens | 9 | 6 (`else`, `endif`, `endfor` are inside the scanner-owned closing directives) |
| `treebank lint`: parse states | 474 | 477 |
| `treebank shape`, the crate's shape fixtures | 0 missed | 0 missed |
| `treebank sweep`, the locked corpus | 19,219 / 19,219 parse | 19,219 / 19,219 parse |
| `treebank shape`, the locked corpus | 2,227,614 oracle nodes, 0 missed, 0 lexical disagreements, 0 field mismatches | 2,227,614 oracle nodes, 0 missed, 0 lexical disagreements, 0 field mismatches |
| `grammar.json` rules / externals / generated scanner | 64 / 7 / 479 lines, by hand | 81 / 11 / 1,040 lines, generated |

The one negative the module accepts is `string-raw-newline.tf`, a line
break inside a quoted template. tree-sitter's lexer skips the break as an
extra before the scanner is consulted again, and the scanner cannot see
what was skipped; the reference rejects it by an accident of lex-state
merging (its internal lexer happens to find `identifier` after the
skipped break, where this grammar's finds nothing and the scanner is asked
again). The lowering says so as its one WIDENING finding, and names every
scanner-owned token the effect can reach.

The three extra parse states are the hidden rules the `_`-prefixed sorts
became (`_obj_elems`, `_for_intro`, `_label`) where grammar.js writes the
same shapes inline; `lint_policy.toml` carries them as the baseline with
the reference's numbers beside each.

## What the module needed that SDF3 does not have

Three extensions beyond those the earlier spikes established, each named in
the findings ledger:

- **Kernel syntax as written.** SDF3's `syntax` section, where no layout
  is admitted between symbols unless `LAYOUT?-CF` is written, holds the
  template sub-language. That is SDF3, not an extension; what is new is
  the lowering: every lexical sort kernel syntax reaches where no layout
  may precede it is scanned by the generated scanner, by simulating the
  sort's automaton (Thompson's construction, no backtracking, `mark_end` at
  every accepting position), and the scanner is consulted before extras.
  The reference scanner's mode stack is what this replaces.
- **A lexical sort whose text is LAYOUT** (`_NL`). The generated scanner
  emits it only where the parse admits it and the same text is layout
  everywhere else: tree-sitter-hcl's `_newline`, derived from the overlap
  between the sort and `LAYOUT` rather than written.
- **`delimiter(1, 3)`**, the one thing SDF3 cannot say: the heredoc's
  closing delimiter is the word its opener chose. The scanner keeps a
  stack of the captured words; the closer matches only at the start of a
  line, only with the word on top, and pops it.

And, from the earlier spikes' set: `_`-prefixed hidden sorts (SDF3 has no
constructor-less production of more than one symbol), placeholder labels,
priority members written as whole productions (SDF3's own form, needed
since `Exp.BinaryExpression` has six productions at six levels), the
`vocabulary` section, `scope`/`binds`/`refers`, and `IDENTIFIER = keyword
{prefer}`, which lowers to `word` without a reserved set -- tree-sitter's
keyword extraction, which is HCL's own rule.

## The findings ledger

| backend | findings | unsupported | widening | deviation | extension | absorbed | mapped |
|---|---|---|---|---|---|---|---|
| tree-sitter (`findings.md`) | 217 | 0 | 1 | 1 | 88 | 11 | 116 |
| bindings (`bindings-findings.md`) | 12 | 0 | 0 | 1 | 10 | 0 | 1 |
| winnow (`winnow-findings.md`) | 6 | 0 | 0 | 0 | 1 | 0 | 5 |
| ANTLR (`antlr-findings.md`) | 65 | 1 | 1 | 60 | 0 | 0 | 3 |

The one tree-sitter deviation is the bracket production
(`Exp.ParenthesizedExpression`), as in every spike. The 88 extensions are
the labels, the hidden sorts and the binding attributes, counted per
production. The ANTLR deviations are its injection context nodes, elided
by the driver.

## The other two backends

| case | tree-sitter | winnow | ANTLR |
|---|---|---|---|
| the crate's corpus, 17 cases | 17 | 17 | 3 |

winnow holds every case: kernel syntax is parsed as written (no layout
skipped where none is admitted), `_NL` is matched before layout is
skipped, the delimiter is a dynamic guard every list loop consults, and
the sort's follow restrictions travel with it into the sorts that use it
(`_QSIGIL -/- [\{]` is what ends a chunk at `${`). ANTLR holds the three
cases with no quoted string in them and fails the rest: kernel syntax
needs lexer modes, which the lowering does not derive, so the template
tokens are declared unmatchable and the finding says so. `_NL` did lower
there -- a token, with `H_NL*` at every position layout is admitted -- and
so did `IDENTIFIER = keyword {prefer}`, as a hidden rule admitting the
keywords at every identifier position. `confer-results.md` has the case
table: 3 agree, 14 differ, all 14 on ANTLR's templates.
