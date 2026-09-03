+++
language = 'ruby'
vocabulary = '0.1.0'
generate_cli = '0.26.12'

# Which measurement essays this ledger carries. NAMED, not inferred from
# which keys happen to exist -- see notes/ledger-format.md sec 2.2.
checks = ['shape']

[corpus]
source = 'locked top-120 RubyGems by downloads (fetched 2026-08-20), ruby-platform releases; the CRuby 3.3.6 standard-library measurements are retained separately as historical context'
files = 6487
packages = 120

[[oracles]]
id = 'cruby'
family = 'ruby'
tool = 'RubyVM::AbstractSyntaxTree.parse_file, via tools/rb-oracle/check.rb'
version = 'CRuby 3.3.6'

[[known_gaps]]
id = 'char-literal-ternary'
construct = '`?a` character literals adjacent to ternaries'

[[known_gaps]]
id = 'percent-equals-literal'
construct = '`%=…=` percent literals'

[[known_gaps]]
id = 'spaced-paren-parameter-list'
construct = '`def f (a)` with a space before the parenthesis'

[[known_gaps]]
id = 'symbol-operator-lookahead'
construct = '`:sym=~x` operator families'

[[known_gaps]]
id = 'do-block-subscript'
construct = 'Subscripting a `do`-block call directly'

[[known_widenings]]
id = 'parenless-oneline-body'
construct = 'A paren-less definition accepts a body on the same line'
found_by = 'corpus sweep'

[[known_widenings]]
id = 'parameter-ordering'
construct = 'The parameter list accepts orderings CRuby rejects'
found_by = 'corpus sweep'

[[known_widenings]]
id = 'command-do-in-argument-list'
construct = "A paren'd argument list accepts a command with a `do`-block"
found_by = 'corpus sweep'

[[deviations]]
id = 'extras-heredoc'
what = '`heredoc_body` sits in `extras`'

[[deviations]]
id = 'resolved-lexer-rules'
what = "Bare lowercase heredocs and the spaced global-scope `::` follow CRuby's lexer"
+++

# ruby — grammar ledger

What this grammar covers, what it is measured against, and every place it
knowingly differs from CRuby. The numbers live in `evidence.json`, which
`treebank sweep` owns; nothing here is generated.

## Versions

Ruby 3.x, plus the older spellings that cost nothing to keep (hash rockets, `for` loops, `=begin` comments, BEGIN/END blocks). The 3.x additions are in: endless methods and one-line pattern matching (3.0), hash shorthand `{x:}` and anonymous block/splat forwarding (3.1/3.2), case/in patterns with find/alternative/pin forms. `it` as an implicit block parameter (3.4) is not special-cased — it parses as the ordinary identifier it also is.

## Vocabulary

18 of 22 structural terms are threaded. `_type` is omitted because ruby has no type syntax; `_modifier` and `_attribute` because ruby has neither keyword modifiers nor annotations — visibility (`private`) and macros (`attr_reader`) are ordinary method calls and parse as the calls they are. `_control_flow` is omitted for python's reason with ruby's evidence: branches and loops are values here (`x = if c then 1 end`), but jumps are not (`x = break` is "void value expression", a SyntaxError from CRuby's parser), so an umbrella containing `_jump` would either accept those or violate the vocabulary's containment rule. `_branch`, `_loop` and `_jump` thread individually.

Statement modifiers (`x if y`, `retry while flaky`) are NOT under `_branch`/`_loop`, and the reason is the same one python's terms.json records for its match shapes: a supertype's members enter every position that references it, `_branch` is reachable from argument position (`x = if c then a end` is legal), and a modifier there — `foo(1 if c)` — is a SyntaxError. The modifier nodes are their own statement-tier types instead.

## Corpus

RubyGems publishes real per-gem download counts but no top-N endpoint; ecosyste.ms indexes the registry and sorts by them, the same traffic metric crates.io and npm rank by. The `.gem` format needs the nested-archive hooks: a gem is a plain tar whose source lives entirely inside its `data.tar.gz` member, and neither layer has a wrapper directory to strip. Platform gems (precompiled extensions) are refused in favour of the `ruby`-platform release, for the reason python refuses wheels. One acquisition deviation is on record: this corpus was downloaded by a curl-driven script applying treebank-corpus's ruby rules verbatim (ranking source, platform rule, nesting, the classify lists, vendor/ exclusion), because the treebank binary's TLS stack does not trust the sandbox's egress proxy; the sweep itself ran through `treebank sweep` against the manifest unchanged.

## Sweep history

SECOND PASS, after the notes/field_guide.md round. The first pass shipped at 97.8% with 140 gaps; stdlib had moved 67.2% -> 97.0% during bring-up (the largest single fix, 141 files, was a statement-sequence shape that committed to a statement after a run of blank lines that never came) and stands at 99.5% now (8 of 1,650 files). Zero noise both passes: every failure is a file CRuby accepts, adjudicated per file by the oracle.

What the second round found, in the order the corpus taught it:

- The largest recovery was structural, not a weight: block-only calls hang their block on a callee tier that excludes completed invocations, so `File.open(path) do ... end` stopped nesting as a call-of-a-call, and the graded dynamic weights that had been compensating came OUT — measured first: a weight on a construct correct parses contain re-ranks whole versions at the cull (one such weight took the stdlib sweep from single digits to 49 failures).
- `yield unless done?` — prec.left on yield, the jump-keyword follow-set trick (notes/field_guide.md §6); prec.right had been eating the modifier as an argument, and the swallowed body ate the enclosing `end`.
- Definitions and blocks stopped owning their own trailing newline (one owner per newline, §7): a def/class body's first terminator forked per construct, and the forks stacked with nesting depth.
- `x = y.merge "a" => b.value` lost its pair to a one-line pattern match with version_count pinned at 1 the whole way: the declared [_argument, pair] conflict was DEAD — _argument's prec.left resolved the cell reduce-first before the conflicts list was ever consulted (§3's trap, running the other way). The rocket now outranks the reduce statically, because parse.y makes the pair unconditional there: a match value is an arg, and a command is not an arg. That one resolution also collapsed the parse table from 9,868 states to 7,285.
- Bare `module?` — parse.y's tokenizer consumes ?/! into the identifier BEFORE keyword lookup, so keyword-plus-suffix spellings are literal tokens and maximal munch picks them over the keyword; no fork, no reserved-word rejection.
- Symbols of the special globals (`:$/`, `:$;`, `:$-w`) — the scanner's symbol path now mirrors the global_variable token's own character set.
- `class Attribute < Struct.new :relation, :name` — superclass is parse.y's expr_value, a tier that includes paren-less commands.
- `def ~@` and `def !@`; `def (o = self.content).content` — the at-suffixed operator names, and the parenthesised singleton receiver.
- Four declared conflicts generation no longer needed were pruned (the member_block family and _statements/function_definition), each removal re-validated against both corpora.

One measurement CORRECTION from between the passes is on record: an interim claim that the stdlib gap count matched upstream tree-sitter-ruby's exactly (three files) was an artifact — the tree-sitter CLI caches compiled grammars BY NAME (~/.cache/tree-sitter/lib/ruby.so), and the comparison had loaded the upstream parser under this grammar's name. The honest number at that point was 49; every number in this ledger postdates that discovery and was measured cold-cache, and the measurement script now clears the cache unconditionally.

The residue (8 stdlib files of 1,650, 7 gem files of 6,485) is the first pass's named cluster, much smaller: GLR version-count pressure in deeply nested module-wrapped code. Minimized reproductions run 50-140 lines of ordinary nesting — defs inside ifs inside blocks — and pass the moment any line is removed; --debug shows version_count riding tree-sitter's cull ceiling of six at the failing depth. The fix direction stays structural (fewer ambient forks), NOT weights: the first pass's advice to spread dynamic-precedence weights is retracted by the measurement above.

THIRD PASS, the scanner round, driven by the shape gate rather than the sweep (its findings parse cleanly, so no acceptance number moves): the `::` token split by CRuby's own rule (glued to a value it chains, otherwise it opens the global scope — valid_symbols is the value-context test, so `A::B::C.m args do … end` extends deterministically and never breaks into a command call); range operators moved into the scanner for their one-token memory (after `..` a glued minus is unary, so `1..-x.size - 1` is a range with a negated bound, not an endless range being subtracted from); `do` reattached by structure and score (an unweighted command route against value routes that each carry a dynamic -1, after an extraction experiment showed mid-rule aliases of the do-family silently vanish from the generated table — three generate-and-probe rounds of evidence, kept as canary probes); shorthand interpolation (`#@ivar`, identifier-shaped only — `#$%` stays text); bare lowercase heredocs by the whitespace rule; and regex character classes with a depth counter that lives in the literal stack so `%r{[{}/]}` survives its own delimiters. Acceptance held exactly through all of it: the round was measured by the boundary check instead.

## Oracle: CRuby {#cruby}

Primary and only. It runs the same parser `ruby` runs and stops at the AST: no require, no execution, no constant resolution, so a missing gem is not an error and a file is judged entirely on its own text. The interpreter's version decides what counts as valid Ruby and is a real knob — `it` blocks are 3.4+, `{x:}` shorthand is 3.1+, endless methods 3.0+ — so a file needing newer syntax than the oracle's interpreter is recorded as noise, an answer about the toolchain rather than the grammar. Verified rather than assumed: `File.write(...); raise "ran"` parses valid and writes nothing, and neither `BEGIN {}` nor `at_exit {}` fires.

## Shape check

`treebank shape` compares our node BOUNDARIES against CRuby's own AST (RubyVM::AbstractSyntaxTree) and our token boundaries against Ripper, CRuby's lexer — the checks that see a file parse CLEANLY AND WRONGLY, which the sweep is structurally blind to. Building it found three live mis-parses on the first 150 corpus files, each a clean parse with the wrong tree: `defined?(x) && y` swallowed the conjunction into the operand (fixed — the paren form is a parse.y PRIMARY and binds like a call), `create_table :a, id: pkt do … end` hung the block on the pair's VALUE (fixed structurally: `do` attaches to commands through an unweighted route while every value route carries a dynamic -1, so ruby's outermost-command rule falls out of the cull scores), and a three-constant scope chain broke into a command call under GLR pressure (open: the fix is the `::` spacing split CRuby's own lexer makes, queued with the scanner work, and the shape report holds it visible at 22 misses). Most of CRuby's disagreements are its PACKAGING nodes — argument LISTs that exclude the parens, chained WHEN/elsif/kwarg spans, the invisible ERRINFO of a rescue capture — each reproduced on a minimal file and declared in shape_policy.toml with its reasoning. The scanner round then took the queue to near-zero on the triage sample: after the `::` split, the range-op memory, and the squiggly-dedent declarations, the first 150 corpus files show 2 missed boundaries of 21,252 and no token disagreements — the two survivors are one version-pressure adjacency, the same cluster the sweeps name. The first FULL-corpus run stands at 3,259 missed of 3,788,448 (99.91% boundary agreement over 6,471 files; 14 skipped for encodings), and its sixty clusters are the next queue — most look like the same packaging families at spellings the sample never hit (subscript and regex interiors, splatted arrays), with one `BLOCK <- block` mass to triage before anything is declared.

## Known gaps

Constructs known missing or approximated, each ledgered when found on the
corpus.

### `?a` character literals adjacent to ternaries {#char-literal-ternary}

`a ?b : c` mis-lexes.

### `%=…=` percent literals {#percent-equals-literal}

Refused, so that `a %= b` stays an operator.

### `def f (a)` with a space before the parenthesis {#spaced-paren-parameter-list}

The parentheses read as a destructuring parameter. CRuby itself warns on the form.

### `:sym=~x` operator families {#symbol-operator-lookahead}

`a!=b` written without spaces lexes as `a != b` correctly, but the `:sym=~x` families rely on one-character lookahead in the scanner.

### Subscripting a `do`-block call directly {#do-block-subscript}

`list.map do … end[0]` is a gap, because the chain family carries members and calls but not subscripts — its cross-recursion multiplied table generation past use.

## Known widenings

Places the grammar accepts more than CRuby does, taken deliberately.

### A paren-less definition accepts a body on the same line {#parenless-oneline-body}

`def f(a) x end` is accepted; CRuby rejects it. Taken so that a definition's own newline never competes with its body's — one owner per newline.

### The parameter list accepts orderings CRuby rejects {#parameter-ordering}

`def f(&b, a)` is accepted. The ordering chain python's `parameterRules` spells out is the known fix, and until then `_parameter` stays honestly structural.

### A paren'd argument list accepts a command with a `do`-block {#command-do-in-argument-list}

`f(foo bar do end)` is accepted and CRuby rejects it: the single-command slot does not distinguish the spellings.

## Deviations

### `heredoc_body` sits in `extras` {#extras-heredoc}

`heredoc_body` sits in `extras`, an exception to notes/DESIGN.md §4.1's comments-and-whitespace rule, declared here: a heredoc's body physically lies between the newline that ends its operator's line and the next line — between tokens of unrelated constructs — and an extra is the only placement the parse table offers for that. The scanner only produces one while a heredoc operator is actually pending.

### Bare lowercase heredocs and the spaced global-scope `::` follow CRuby's lexer {#resolved-lexer-rules}

A bare lowercase heredoc after a value (`a <<b`) is read as CRuby reads it — a heredoc when whitespace precedes the `<<`, the shift when glued — and the spaced global-scope `::` follows CRuby's lexer the same way (glued to a value it chains, spaced or at a beginning it opens the global scope), warnings and all. Recorded because both spellings look like gaps to a reader who meets them in a diff.
