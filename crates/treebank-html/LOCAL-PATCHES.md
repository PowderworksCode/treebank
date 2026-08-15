# treebank-html local patches

Upstream: [tree-sitter/tree-sitter-html](https://github.com/tree-sitter/tree-sitter-html)
pinned at `73a3947324f6efddf9e17c0ea58d454843590cc0`.

Seven patches: two packaging, and five grammar fixes that between them take
the gap queue from **9,707 files to 119** and the corpus from 91.8% parsing
to **99.16%**.

The first two are the same shape — a character HTML5 treats as ordinary text
that the grammar had no token for. The last three are the structural ones:
what happens at end of document, what happens to an end tag that closes
nothing, and where a tag name stops.

## 0001 — treebank redistribution notice

The standard first patch of every grammar: a warning at the top of upstream's
`README.md` saying that this tree is an automatically generated, patched
redistribution maintained by [treebank](https://treebank.dev), so anyone who
meets a materialized or published copy knows what it is and where to report
problems. Touches no grammar code.

## 0002 — treebank crate identity

The standard last patch. Upstream owns `tree-sitter-html` on crates.io, so the
redistribution publishes under `treebank-grammar-html` with its own
`repository`, `homepage` and `description`, and extends `include` so
`ledger.json`, `LOCAL-PATCHES.md` and `patches/` travel inside the published
tarball. `[lib] name` stays `tree_sitter_html`, so the crate is a drop-in
replacement.

Nothing here corrects upstream's `include` list, unlike lua's equivalent
patch: `LICENSE` reached it in commit `cbb91a0` (PR #117), which is one of the
five commits between the `v0.23.2` tag and the pinned sha — and is part of why
the pin is HEAD rather than the tag.

## 0003 — a bare ampersand in text

`<p>Privacy & Terms</p>` did not parse. HTML5 requires no escaping of `&` —
a lone ampersand is character data, and it is only the "ambiguous ampersand"
parse error when it *looks* like a named reference and is not one. The grammar
had no token that could match it: `text` cannot start on `&`, and `entity`
needs a `#` or a letter next. The asymmetry shows in the small — `R&D` parses
because `entity` matches `&D`, while `R & D` does not.

Adding `'&'` as a second alternative of `text` fixes it. `entity` still wins
wherever it applies, because tree-sitter prefers the longest match.

**5,713 files**, 59% of the whole gap queue and its largest cluster by a wide
margin. No upstream issue covers this; #45/#50/#10 are about parsing entities
and #51/#108 about entities in *attributes*. Worth offering upstream.

## 0004 — a bare greater-than in text

`<p>a > b</p>` did not parse either, and this one is not even a parse error in
HTML5 — only `<` and `&` begin anything, so a lone `>` is ordinary character
data. The grammar excluded it from `text` alongside them.

**1,183 files**, measured with 0003 already applied, almost all of them pages
that display code: `WHERE o.created_at > NOW()`, `(job) => createJob(...)`
inside `<pre>`, `<code>` or a paragraph, where the author had no reason to
write `&gt;`.

## 0005 — end of document closes every open element

This was the 955-file class the previous ledger recorded as deliberately not
attempted, on the grounds that it would need a rewrite of the scanner's
end-tag logic. It did not. `scan_implicit_end_tag` closed an element at EOF
only when the innermost open one was `html`, `head` or `body`:

```c
((parent->type == HTML || parent->type == HEAD || parent->type == BODY) && lexer->eof(lexer))
```

The spec's "stop parsing" step pops the *whole* stack, so `<div><p>hello` with
no closing tags — which every browser renders — was an ERROR. Dropping the
three-way type test to a bare `lexer->eof(lexer)` is the entire change.

**1,826 files.** Gap queue 2,823 → 1,052.

## 0006 — a stray end tag the scanner never ran for

`<p>x</p></head>` was an ERROR while the same stray tag mid-document parsed.

The grammar has an `erroneous_end_tag` rule for exactly this, and
`scan_end_tag_name` already emits `ERRONEOUS_END_TAG_NAME` when the name does
not match the top of the stack. But the scanner's dispatch gated that branch on
`valid_symbols[START_TAG_NAME] || valid_symbols[END_TAG_NAME]` and never asked
about `ERRONEOUS_END_TAG_NAME` — so in the one situation the rule exists for
(document level, empty tag stack, where `END_TAG_NAME` is not valid because
there is nothing to close) the scanner declined to run at all.

**895 files.** Gap queue 1,052 → 158. One line, and the largest fix per
character in the series.

## 0007 — tag names end only at whitespace, a slash or `>`

`scan_tag_name` accepted alnum, `-` and `:`. The spec's tag-name state ends on
exactly three characters, and everything else — `=`, `.`, non-ASCII, even `<` —
is part of the name. The *start* is separately restricted to ASCII alpha by the
tag-open state, which is what keeps `<?xml ?>`, `<3` and `< div` from being
tags, so that half is kept explicitly.

**39 files** — the smallest yield here and the only patch that cost nothing:
`noise_files` is unchanged. It also fixes two things that are simply correct
rather than tolerant: `<dØdd>` (a non-ASCII element name) and the wpt fixture
asserting that `<x<>` creates an element named `x<`.

## What is left, and what to be careful of

**119 gap files, and no head to the queue any more** — the largest cluster is
67 occurrences across 61 files and the rest are single digits. Much of the tail
is web-platform-tests fixtures written to exercise parser edge cases rather
than pages anyone serves. Expect diminishing returns.

The other direction has its own queue, and it is the one the sweep on this
language cannot find: **six classes where the oracle says the markup is
malformed and the grammar accepts it anyway**, each with a repro, in
`ledger.json` under `accepts_invalid_markup`.

Whichever direction the next patch goes, check the other one. Four of the five
fixes here moved files from noise into passing — 12, 38, 55, 1 and zero for
0007 — 106 files cumulatively, which is the honest cost of a more forgiving
grammar; a strictness patch pays the same toll in
reverse, and on a recovery-by-spec language overshooting turns valid pages into
gaps. `tools/consumer-test/fixtures/patched.html` and `test/negative/` are the
two guards, and both were re-run rather than assumed after each patch.

## Why the pin is HEAD and not the tag

`73a3947` is five commits past `v0.23.2`, and all five are CI bumps, a
`FUNDING.yml` and the `LICENSE` fix — **no grammar change**. It is also the
commit that **both nvim-treesitter and Helix pin**, so the pin follows the
editors rather than the tag. Zed is the odd one out at `bfa075d`, an older
commit.
