# treebank-html local patches

Upstream: [tree-sitter/tree-sitter-html](https://github.com/tree-sitter/tree-sitter-html)
pinned at `73a3947324f6efddf9e17c0ea58d454843590cc0`.

Four patches: two packaging, and two grammar fixes that between them take the
gap queue from 9,707 files to 2,823 and the corpus from 91.8% parsing to
**97.1%**. Both grammar fixes are the same shape — a character that HTML5
treats as ordinary text and the grammar had no token for.

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

## What is left, and what to be careful of

The head of the remaining queue is one class: **an element left unclosed at end
of document**, 955 files. The spec closes them at EOF and every browser renders
them; tree-sitter-html closes a tag implicitly only through its external
`_implicit_end_tag`. It is a real gap and it is deliberately not attempted
here, because making EOF close arbitrary open elements touches the scanner's
end-tag logic and therefore every file in the corpus.

The other direction has its own queue, and it is the one the sweep on this
language cannot find: **six classes where the oracle says the markup is
malformed and the grammar accepts it anyway**, each with a repro, in
`ledger.json` under `accepts_invalid_markup`.

Whichever direction the next patch goes, check the other one. Both fixes here
moved a handful of files from noise into passing (12 and 38), which is the
honest cost of a looser text rule; a strictness patch pays the same toll in
reverse, and on a recovery-by-spec language overshooting turns valid pages into
gaps. `tools/consumer-test/fixtures/patched.html` and `test/negative/` are the
two guards, and both were re-run rather than assumed after each patch.

## Why the pin is HEAD and not the tag

`73a3947` is five commits past `v0.23.2`, and all five are CI bumps, a
`FUNDING.yml` and the `LICENSE` fix — **no grammar change**. It is also the
commit that **both nvim-treesitter and Helix pin**, so the pin follows the
editors rather than the tag. Zed is the odd one out at `bfa075d`, an older
commit.
