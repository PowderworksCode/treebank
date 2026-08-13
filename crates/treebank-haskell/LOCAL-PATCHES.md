# Local patches — tree-sitter-haskell

Upstream: [tree-sitter/tree-sitter-haskell](https://github.com/tree-sitter/tree-sitter-haskell)
pinned at `0975ef72`. `ledger.json` is the machine-readable version of this
file, with the sweep numbers.

Two of the five patches are packaging (`0001`, `0005`), one is a
generation artifact of the pinned CLI (`0002`), and **two are parser
fixes** (`0003`, `0004`) — both of them the same underlying defect.

## 0001 — treebank redistribution notice (packaging)

The standard warning at the top of upstream's `README.md`, so that anyone
who meets a materialized or published copy knows it is a patched
redistribution and where to report problems. Touches no grammar code.

## 0002 — error-recovery tree for the pinned CLI (generation)

Upstream generated `src/` with tree-sitter-cli 0.24.x; this repo pins
0.25.10, and under the pin exactly one of upstream's 725 corpus tests fails:
`varsym: error: carrow`, whose expected tree records how the parser
*recovers* from `f = a => a`, which is not valid Haskell either way.

```
0.24:  (match (apply (variable) (ERROR (UNEXPECTED '>')) (variable)))
0.25:  (ERROR (match (variable))) (match (ERROR (UNEXPECTED '>')) (variable))
```

The file is rejected before and after — both trees carry `ERROR` nodes and
`tree-sitter parse` exits 1 on it — so this is the CLI's error recovery
moving, not the grammar's language changing. The CLI pin is not ours to move
for a test-tree mismatch (0.26.x ships Unicode identifier tables that wrongly
drop XID_Start characters, which is why every grammar here pins 0.25.10), so
the expected tree moves instead and this entry is the record of why.

## 0003 — tab is whitespace when skipping layout (parser fix)

```haskell
module M (
	eval,
) where
```

That file — one tab, one identifier — parses to `(ERROR (UNEXPECTED '\t'))`.
Replace `eval` with `abcd` and it parses. Replace the tab with eight spaces
and it parses.

`scanner.c` skips layout whitespace with `is_space_char()`, which is not the
scanner's own function: it comes from the **generated Unicode bitmap** in
`src/unicode.h`, whose `bitmap_space_min_codepoint` is 32. Tab is codepoint
9, so `is_space_char('\t')` is false and `skip_space()` stops dead at a tab.
`take_space_from()`'s own doc comment already states the intent the predicate
does not implement — *"until the following character is neither space nor
tab"*.

The six letters are `w i t e d m`, and they are not a coincidence: `lex()`
dispatches on the first character to try the layout keywords (`where`, `in`,
`then`, `else`, `deriving`, `module`), and only that path sets
`newline.unsafe`, which is what routes through the `newline_post()` skip
where the bug lives. `case`, `foreign` and `newtype` start with letters the
switch does not name, so they never reach it.

Diagnosed with a `TREE_SITTER_DEBUG` build: the tab-indented run finishes its
newline token at `newline_post@0` where the space-indented run finishes at
`newline_post@8` — the lexer never moves past the tab, so the parser meets it
as an unexpected character.

The fix adds `is_whitespace_char()` (tab, or `is_space_char`) and uses it in
the three places whose intent is whitespace. **+360 files**, gap files 556 →
283.

## 0004 — a tab counts as one column in layout, not eight (parser fix)

```haskell
f = do
	r <- a
	let c = g r
	pure c
```

`newline_lookahead()` counted a tab as **eight** columns of indent, following
the report's tab stops. Every other column in this scanner comes from
`column()`, i.e. tree-sitter's `get_column()`, which counts a tab as **one**
character like any other. Layout is nothing but comparing those two numbers,
so a context opened on a tab-indented line recorded an indent that no
interior column could ever match, and a `let` inside a tab-indented `do`
could not be placed.

Eight is the report's answer and one is tree-sitter's, and the scanner cannot
have both: `get_column()` is the lexer's own counter and takes no tab-width
argument, so honouring the report everywhere would mean re-implementing
column tracking across the scanner. Within a file indented one way — which is
what real code is — either convention orders the lines identically, and
consistency is what layout actually needs.

**The residual divergence, stated rather than hidden:** a file that mixes
tabs and spaces *at the same nesting level* is exactly where the two
conventions disagree, since GHC puts a tab and eight spaces at the same
column and this does not. The direction that could cost a file is a
space-indented line followed by a tab-indented one GHC considers equally
deep; measured, it parses anyway, because tree-sitter ends a layout on a
strictly smaller indent and the mixed cases in the corpus land deeper rather
than shallower. Both probes parse clean and the oracle calls them valid.

**+147 files**, gap files 283 → 194. With 0003, **556 → 194, a 65%
reduction**, and both patches are one defect: the scanner had one notion of a
tab in `newline_lookahead()` and another everywhere else.

## 0005 — treebank crate identity (packaging)

Upstream owns `tree-sitter-haskell` on crates.io, so publishing needs our own
name, `repository`, `homepage` and `description`, and an `include` list that
carries `ledger.json`, `LOCAL-PATCHES.md` and `patches/` inside the tarball.
`[lib] name` is pinned to upstream's `tree_sitter_haskell` so the crate stays
a drop-in replacement.

One addition beyond the usual identity patch: upstream's `include` list names
`grammar.js` but not `grammar/*.js`, and this grammar's `grammar.js` is a
loader that `require`s fifteen modules out of `grammar/`. The published
tarball therefore advertised a grammar source it did not contain. The include
list now carries them.

## Not patched, and why

**The fork.** `tree-sitter-grammars/tree-sitter-haskell` (which
nvim-treesitter's `main` branch pins) is nine commits ahead of the pinned
canonical repo, but its grammar divergence is three lines: a `type_synomym` →
`type_synonym` node rename, and a scanner change that assigns `PEEK` to a
local before `array_push` (labelled *"fix aarch64-linux malloc"*). Neither is
adopted: the rename is a breaking change for query consumers with no parsing
effect, and the `array_push` macro is comma-sequenced, so on this platform
the assignment is a no-op. Neither fixes anything the corpus shows.

**Unterminated block comments.** `{- open` with no `-}` is accepted by the
grammar (it runs to EOF) and rejected by GHC, so it is an accepts-invalid
divergence — the direction that matters most. It is *not* in
`test/negative/`, because a negative test may only contain syntax the grammar
can actually reject, and expressing "this token must be terminated before
EOF" is not something this grammar's comment scanner does today. Recorded
here rather than quietly dropped.

**The third tab bug.** A `let` bound to a `\case` whose alternatives are
indented by three tabs, inside a tab-indented `do`, still fails
(`git-annex`'s `Annex/ChangedRefs.hs`). It survives both patches above, so it
is a different defect rather than more of the same, and it is left for a
diagnosis of its own rather than guessed at.
