# A field guide to writing the parsers

DESIGN.md says what a treebank grammar must *be* — the vocabulary it
carries, the evidence it ships, the gates it passes. This document says
how to *write* one that survives contact with a real corpus. Every rule
here was paid for: each cites the incident that taught it, mostly from
the ruby bring-up (the most lexically ambiguous language in the set) with
the earlier grammars' lessons alongside. When a rule and your intuition
disagree, re-read the incident before trusting the intuition.

The one-sentence version: **a parser is good in proportion to how early
its decisions die.** Every construct that two readings survive is a debt,
and the interest compounds in ways none of your tests will localise.

## 1. The ambiguity ladder

When two readings of the same text exist, there are four places the
decision can be made. They are not interchangeable. Use the highest one
that can express the decision, and treat each step down as a cost you
must justify:

1. **The lexer (external scanner).** The decision dies before the parse
   table ever sees it: the scanner looks at spacing, one character of
   lookahead, or its own state, and emits ONE token. `a * b` against
   `foo *args`, `a / b` against `foo /x/`, `:decl=>1` against `:decl=`
   are all decided here in ruby, exactly where CRuby's own lexer decides
   them (its `EXPR_BEG`/`EXPR_ARG` states are this ladder rung wearing
   yacc clothes). tree-sitter hands the scanner `valid_symbols` — the set
   of tokens the parser could accept right now — which is a window into
   parser state; combined with `has_leading_whitespace`-style tracking,
   it can express nearly every "the lexer must know what the parser
   wants" rule a language manual contains.

2. **Left-factoring.** Restructure rules so both readings share a prefix
   and the parser carries them in ONE state, deterministically, until a
   token settles it. An identifier at expression start might be a value
   read or a call's function; if both rules reach it through the same
   derivation, no decision is ever made — the `(` that arrives (or
   doesn't) makes it. The measured contrast: upstream tree-sitter-ruby
   parses with **zero** declared conflicts this way, and its parse table
   is two-thirds the size of a conflict-heavy one (5,989 states against
   9,154), because forks inflate tables faster than factoring does.

3. **Static precedence.** Where a genuine shift/reduce remains, a
   `prec`/`prec.left`/`prec.right` annotation resolves it at GENERATE
   time, for free at runtime. This is the right tool for operator
   ladders and for keyword-versus-continuation decisions (§6).

4. **GLR with dynamic precedence — the floor, not the default.** A
   declared conflict makes the parser fork and carry both readings.
   Reach for it only when the same text truly parses two ways and only
   later context tells (`A::B` as constant path or method call), and
   ALWAYS attach a `prec.dynamic` weight so the tie is decided by you
   and not by the machinery (§2, §3). An unweighted declared conflict is
   a coin you've asked the runtime to flip.

The failure mode this ladder prevents is treating step 4 as a
convenience. `tree-sitter generate` helpfully suggests `Add a conflict
for these rules:` at every impasse, and accepting each suggestion feels
like progress — the grammar generates, the tests pass. The ruby grammar
reached 60+ declared conflicts that way, and what that bought is §2.

## 2. The fork budget, and the failure signature nothing else explains

tree-sitter's GLR caps concurrently live parse versions at **six**
(`MAX_VERSION_COUNT`, runtime `parser.c`). Beyond the cap it culls,
ranking by error cost, then by accumulated dynamic precedence. Mid-file,
the correct version usually ties the junk versions on both — no errors
yet, no weights — and a tie is culled by internal ordering, i.e.
arbitrarily. A culled version is gone; if it was the right one, the
survivors parse into a wall and error recovery shreds the file.

This produces a failure signature unlike any grammar bug, and learning
to recognise it saves days:

- **Non-compositional.** Every construct from the failing file parses
  clean in isolation. Only the density fails — module wrapping class
  wrapping def wrapping case wrapping block is what overlaps enough
  fork lifetimes to hit the cap.
- **Everything is load-bearing.** Delta-minimising a failing file
  converges on a reproduction where deleting ANY line fixes it —
  including comments. A comment can only be load-bearing when the
  failure is resource pressure, not rules: removing it shifts which
  forks are alive at the cull instant.
- **Fixes act at a distance.** A weight added to one rule fixes files
  whose errors are nowhere near that rule, because it changed cull
  rankings, not acceptance.

Confirm the diagnosis with `tree-sitter parse --debug=normal` and count
`version_count:` — sustained values above six in the failing region are
the proof. Then fix it on the ladder: the fork that lived longest (the
debug log names its reduces) either gets factored out of existence or
gets a `prec.dynamic` weight so it never ties again. In ruby, a `do`
after a loop iterable could attach as a block to the iterable's trailing
call and stay viable for fifteen rows; one weight on do-block attachment
recovered the whole cluster.

Budget rule of thumb: count your declared conflicts, then ask of each
"how many tokens does the wrong fork survive?" Conflicts whose losers
die within a token or two are cheap. Conflicts whose losers can consume
statements are the ones that stack — kill those first.

## 3. The trap: a declared conflict switches static precedence OFF

In any table cell covered by a declared conflict, tree-sitter ignores
static precedence entirely and forks. The two mechanisms do not
compose; declaring a conflict does not "keep precedence as a
tiebreak" — it replaces it.

This has a nasty consequence: an annotation that provably works in a
small grammar silently stops working when a later, unrelated conflict
declaration overlaps its state. The ruby `do`-binding fix is the
canonical case — `prec(PREC.do_block, …)` was correct and *did nothing*,
because the identifier-versus-callee conflicts covered the same cells;
only a dynamic weight worked. The python grammar's comment archive says
the same thing ("a declared conflict also switches static precedence
off") about its comma-expression handling.

Corollaries:

- After ADDING a conflict, re-test every precedence decision whose rules
  it touches. Nothing will warn you.
- When a static precedence "mysteriously doesn't work", grep the
  conflicts list before doubting the annotation.
- Every declared conflict should carry a dynamic weight on the reading
  you'd pick, even when today's tests pass without it: the weight is
  inert until the day a tie reaches the cull, and that day is exactly
  when you won't be looking (§2).

The trap runs the other way too: **static resolution switches your
declared conflict off.** The generator resolves a cell by precedence
first, then by associativity, and only a cell that survives both is
checked against the conflicts list. Equal precedence plus an
associativity is a resolution — so a `prec.left` added for one reason
(ruby's was on `_argument`, for jump-keyword modifiers) silently decides
*every other* shift/reduce tie that rule appears in, and the conflict
you declared for those cells never forks at all. The ruby symptom:
`[$._argument, $.pair]` was declared, yet `x = y.merge "a" => b` parsed
with `version_count` pinned at 1 the whole way — the pair fork never
spawned, the reduce won by left-associativity, and a one-line pattern
match stole the rocket. The diagnosis is mechanical: parse a file that
should be ambiguous with `--debug` and watch `version_count`; if it
never leaves 1, the conflict is dead text. The fix is to decide, not to
fork harder: when one reading is always right in that cell (parse.y
makes the pair reading unconditional there, because a match value is an
`arg` and a command is not), give the winning side an explicitly higher
precedence and let the cell be deterministic.

## 4. One text, one token

Two DISTINCT token definitions with the same spelling — the classic is
`token.immediate('(')` in one rule and plain `'('` in another — are two
different tokens to the lexer, and at each position it produces exactly
one of them per parse state. If two GLR forks need the two different
tokens at the same position, one fork starves at the lexer, before the
parse table gets a vote, and the death is silent.

The incident: ruby's `argument_list` opened with `token.immediate('(')`
(to tell `foo(x)` from `foo (x)`) while parameter lists opened with
plain `'('`. Result: **every** `def f(a)` failed — the call fork owned
the `(` and the definition fork never saw it. The fix was to give the
parameter list the immediate token too; the spaced `def f (a)` form
(which CRuby itself warns about) falls back to a ledgered
approximation.

Rules:

- Before introducing `token.immediate` for a spelling that already
  exists as a plain token, find every state where both could be valid
  and decide who wins — or make them the same token and disambiguate a
  level up.
- The same hazard applies to a scanner external that duplicates an
  internal token's text. Prefer the upstream-ruby arrangement: if the
  scanner arbitrates a spelling (their `/`), the scanner owns it
  EVERYWHERE — one owner per spelling.

## 5. Reserve your keywords

Keyword tokens beat identifier tokens only where the keyword is VALID.
Everywhere else the identifier regex matches the same text and wins —
so a stray `end` at statement level lexes as a variable named `end` and
the file parses. That is a widening no corpus of valid code will ever
show you; ruby's was caught by a two-line negative-corpus file.

tree-sitter's `reserved: { global: [...] }` (0.25+) is the fix: listed
words cannot lex as the `word` token in states where the keyword isn't
expected, while positions that genuinely admit keywords as names
(`x.class`, `def if`) list them explicitly and are untouched. The
python grammar predates the feature and its ledger records the exact
disease (`return yield x` parsing as a variable read) with this as the
wished-for cure — new grammars should start with the reserved list, not
retrofit it.

Declaring `word:` is a precondition, and it wants a SIMPLE token; if
your identifier is a rule (ruby's carries a scanner-attached `?`/`!`
suffix), point `word:` at the inner bare token.

## 6. Let the follow set do your lookahead

Some "needs one word of lookahead" problems don't need a scanner at
all, because LR's reduce actions only exist for lookaheads in the
FOLLOW set — the table has already done the case analysis for you.

The incident: after `break`, an `if` must be a statement modifier, not
the start of an if-expression value (`break if done?`). A zero-width
scanner guard was tried and died of exactly this: to decline correctly
it would have had to re-derive the follow set by hand. The working fix
was `prec.left` on the jump rule — because the ONLY tokens that can
both continue a value and follow a bare `break` are the five modifier
keywords, the shift/reduce conflict exists precisely there, and left
associativity resolves each toward reduce. `break 1` never conflicts
(no reduce action on `1`), so values still parse.

Generalisation: when a decision seems to need lookahead, first write
down the intersection of "tokens that continue reading A" and "tokens
in FOLLOW of reading B". If the intersection is exactly the cases you
want to steer, an associativity annotation is the whole fix. Only when
the intersection is wrong do you need the scanner.

## 7. Sequences must not commit to what hasn't arrived

Shape repetition so that consuming a separator never promises another
element. The largest single loss in the ruby bring-up — 141 of the
first thousand files — was a statement sequence shaped
`repeat(term) statement …`: at a blank line the parser consumed the
terminator INSIDE the "another statement is coming" branch, and a file
(or class body) that ended in comments and blank lines had promised a
statement that never came. The fix is to make the terminator an item of
its own (`choice(seq(stmt, term), term)`), so any run of blank lines
and comments is a complete parse at every prefix.

The same principle catches list rules (`a, b,` trailing commas), bodies
that may be empty, and anything else where "separator" and "there is
more" are different facts. If deleting trailing trivia from a valid
file changes whether it parses, a sequence somewhere is committing
early.

## 8. Scanner discipline

The external scanner is the most powerful tool in the box (§1) and the
easiest place to create unfalsifiable bugs. The rules that kept ruby's
honest:

- **Emit nothing you cannot justify from your own state.** During error
  recovery every symbol is marked valid; detect it (an `_error_sentinel`
  external that is never produced — if it's "valid", you're in
  recovery) and do almost nothing: reset literal stacks, decline
  everything but the safest token. A zero-width token emitted in
  recovery loops the parser forever — bash learned this with a
  zero-width CONCAT that hung a whole sweep.
- **`mark_end` early, look ahead freely.** Advancing past `mark_end`
  and returning true costs nothing; it's how one-character-lookahead
  decisions (`:decl=` vs `:decl=>`) and line-start peeks are written.
  Returning false discards everything. The only unforgivable state is
  consuming into the NEXT token's text with `mark_end` already moved.
- **Whitespace is owned by whoever runs first.** The scanner is
  consulted before extras are skipped, so every scanner-owned token
  must skip its own leading trivia — and a scanner that skips a newline
  it shouldn't (a terminator the grammar needed) merges two statements
  silently. Ruby's `_line_break` handles the newline decision in ONE
  place for this reason, including the look-past-blank-lines check for
  leading-`.` method chains.
- **Serialize the whole truth.** Anything the scanner knows that
  affects future tokens — open-literal stacks, heredoc queues,
  at-line-start bits — must round-trip through serialize/deserialize,
  or incremental reparsing will diverge from fresh parsing in ways only
  `treebank incremental` will ever catch. Design the state so it FITS
  (the buffer is 1 KiB): fixed-size stacks with declining-gracefully
  overflow, not heap structures.
- **Decline is a feature.** Returning false hands the text to the
  internal lexer. The cleanest scanner branches end in a reasoned
  `return false` — `:` followed by `:` is a scope operator, not ours;
  `%` before a space is modulo, not a literal. Write the decline cases
  first; they're the spec.

## 9. Measure like the sweep is watching, because it is

Grammar work runs ahead of intuition only when every hypothesis is
cheap to test. The kit that made ruby's bring-up converge:

- **Cluster failures by the first-error source line.** One `sort |
  uniq -c` over "the line where each failing file's first ERROR starts"
  turns 328 failures into eight causes. Fix by cluster size, not by
  which failure you saw first.
- **Minimise with the oracle in the predicate.** Delta-debugging with
  "our parser errors" as the predicate converges on truncation
  artifacts (any prefix of a valid file errors). The predicate must be
  *"the reference parser accepts AND ours rejects"* — the sweep's own
  definition of a gap. With that, ddmin hands you a valid, minimal,
  actionable reproduction every time.
- **Isolated retest before every fix.** A failing line from a cluster
  that parses fine alone is telling you it's §2, not a rule bug — the
  fix you were about to write would have done nothing.
- **Two corpora, different biases.** Registry code is modern and
  machine-formatted; the language's own standard library is old, dense
  and exercises corners gems avoid (ruby: 97.8% vs 97.0% — the gap IS
  the bias). A number from one population is not a number from the
  other; the ledger says which was measured.
- **The negative corpus is the only mirror.** Sweeps measure
  rejects-valid and are structurally blind to accepts-invalid; every
  deliberate rejection (a modifier in argument position, a stray `end`)
  earns a two-line file in `test/negative/`. Both ruby reserved-word
  widenings were caught there, by files written BEFORE the fix existed.
- **`treebank lint` is the smell detector.** It reads the generated
  artifacts and flags this guide's mechanical smells — conflict count
  and supertype-overlap, unweighted conflicts, same-text token splits,
  unreserved keywords, scanner/externals drift, state-count growth —
  against a per-grammar policy with ratchets. Run it before trusting a
  green sweep; a grammar can pass every behavioural gate while
  accumulating the debt that fails next month's file.

## 10. The checklist

Before calling a grammar done, walk this list top to bottom:

1. Every declared conflict: can it be factored away? If not, does the
   preferred reading carry a `prec.dynamic` weight? Can the losing fork
   survive more than ~2 tokens? And does it actually FORK — parse an
   ambiguous file with `--debug` and check `version_count`, then prune
   the declarations generation no longer needs (§1, §2, §3)
2. Any spelling defined as two distinct tokens? (§4)
3. `word:` declared, hard keywords in `reserved.global`, and a negative
   file proving a stray keyword is rejected? (§5)
4. Every "needs lookahead" decision checked against the follow set
   before reaching for the scanner? (§6)
5. Do all sequence rules parse cleanly with trailing separators,
   comments, and blank lines at every boundary? (§7)
6. Scanner: recovery-safe, fully serialized, decline-first? (§8)
7. Failures clustered, minimised with the oracle predicate, and
   retested in isolation before each fix? (§9)
8. `treebank lint` clean against the grammar's policy, and the policy's
   exceptions each carry a reason? (§9)
