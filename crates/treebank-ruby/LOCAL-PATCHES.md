# Local patches — treebank-ruby

Upstream: [tree-sitter/tree-sitter-ruby](https://github.com/tree-sitter/tree-sitter-ruby)
pinned at `ad907a69da0c8a4f7a943a7fe012712208da6dee`, master head, four
commits past tag `v0.23.1`.

The pin is past the tag on purpose. `ad907a6` is *scanner: fix heredoc
serialization buffer overflows* — a correctness fix in the external scanner,
in a language where heredocs are everywhere. The other three commits touch
the Rust/wasm bindings and the LICENSE include, not the parser.

Twelve patches: two packaging, ten grammar. On the 1000-gem, 44,292-file
corpus they take the sweep from 44,210 passing with 28 grammar gaps to
**44,238 passing with 0**. The 54 files still failing are ERB templates that
CRuby also rejects. `noise_files` was 54 before the first patch and 54 after
every one of the ten — measured at each patch level — so no patch bought a
passing file by accepting something the reference parser rejects.

The corpus was doubled from 500 gems to 1000 after the first ten patches had
taken the 500-gem sweep to zero gaps: at that point the corpus, not the
grammar, was the limit. The second 500 gems produced exactly two new gaps,
0011 and 0012, both pre-existing upstream — each was checked by materializing
the pin with only the two packaging patches applied.

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md` so anyone who encounters a
materialized or published copy knows it is a generated redistribution and
where to report problems. Touches no grammar code, applies first.

## 0002 — treebank crate identity

Upstream owns `tree-sitter-ruby` on crates.io, so the redistribution
publishes as `treebank-grammar-ruby`, with treebank's repository, homepage
and description. `[lib] name` is pinned to `tree_sitter_ruby` so the crate
stays a drop-in replacement, and `include` gains `LOCAL-PATCHES.md`,
`ledger.json` and `patches/*` so provenance travels inside the published
tarball. `Cargo.lock` gets the matching rename and nothing else.

The published version string is deliberately absent: `publish.sh` derives it
from crates.io at publish time. See `PUBLISHING.md`.

## 0003 — endless method with a command call body

7 files. Ruby 3.1 allows a command call — a call whose arguments are not
parenthesised — as the body of an endless method:

```ruby
def compile(**options) = raise NotImplementedError, 'subclass responsibility'
def insert_minmax(idx, min, max) = minmaxes.insert idx, [min, max]
def visit_any(_) = yield "Any"
```

parse.y calls this `endless_command` and its productions are `command`,
`endless_command rescue arg` and `not endless_command` — deliberately not the
whole of `expr`. Scoped to match, and checked in both directions against
CRuby 3.2.3: `def a = b and c` parses as `(def a = b) and c`, and
`def a = not b` is a syntax error, so neither was admitted.

The cost is four precedence declarations rather than one, and that is
inherent: once the body may be a command, the end of the body coincides with
every point where the command could still be extended — a `.` continuing a
chained call, a `|` continuing an alternative pattern, a `...` continuing a
range pattern. Ruby keeps extending at all of them, so `_body_expr` takes
`prec(-1)` and `_chained_command_call`, `alternative_pattern` and the
two-sided `_pattern_range` each take `prec.left(1)`. Each was added only
after tree-sitter reported the specific conflict it resolves.

## 0004 — block argument with a space, or on its own line

5 files. Two shapes, one cause:

```ruby
new(args).execute(& block)      # space between & and the block
target(key: value,
       &                        # anonymous block forwarding, Ruby 3.1
      )
```

The external `_block_ampersand` fires only when `&` is followed immediately
by a non-space. Relaxing that test in the scanner was tried first and
**rejected**: it turned `a & b` into a call with a block argument, which the
grammar's own `bitwise and` corpus test caught. The scanner runs before the
parser state is consulted and cannot tell an argument-start `&` from a binary
one — after `(` or `,` there is no left operand, so only a block-pass is
possible, and that is a fact about the parser state. So the plain `&` token
is offered as a second alternative inside `block_argument`, at `prec(-1)` so
binary wins wherever both readings exist.

## 0005 — self and super as method names

3 files. Ruby's `fname` admits every reserved word, so `def super` and
`def self` define ordinary methods (pry and the parser gem both do it).
Measured across all 40 reserved words against CRuby 3.2.3: `self` and `super`
were the only two the grammar rejected, because each has its own rule the
lexer prefers over `identifier`. `prec(-1)` keeps `def self.foo` a
`singleton_method`.

## 0006 — symbols for punctuation globals and non-ASCII names

3 files. Two scanner bugs.

`is_iden_char` took a `char`, truncating the codepoint to its low byte, so
any character whose low byte collided with one of `NON_IDENTIFIER_CHARS` was
wrongly rejected. U+2620 SKULL AND CROSSBONES ends in `0x20`, the byte for a
space, so sidekiq's `alias_method :☠, :exit` would not lex. It now takes the
codepoint and treats everything above U+007F as an identifier character,
which is what Ruby and grammar.js's own `IDENTIFIER_CHARS` both do.

Separately, of Ruby's one-character global variables only those that are also
operators (`$!`, `$&`, `$~`) reached `scan_operator`; `$:`, `$;` and `$'` did
not, so `:$:` and `:$'` failed to lex at all. parse.y's set is now checked
explicitly, before the operator path — which also stops `scan_operator`
reading `:$<` as `<<` or `<=`.

## 0007 — character literal with an escaped control character

1 file. The character escaped by `\C-`, `\c-` or `\M-` may itself be an
escape: `?\C-\]` is control-] and appears in ruby_parser's lexer tests. The
old `-\S` consumed only the backslash and left the `]` to close whatever
bracket was open around it.

## 0008 — defined? as a hash key

1 file. `defined?` is the only reserved word ending in `?`, so it is the only
one `identifier_suffix` cannot supply as a hash key: at equal length the
literal keyword token outranks the identifier regex. Every other reserved
word already worked. rubocop-ast keys a node-type map with it.

## 0009 — embedded document ends at a line starting with =end

1 file, and one accepts-invalid fix in the other direction.

The token scanned for `=end` *anywhere*, so an embedded document ended at the
first mention of the string wherever it sat. rdoc's `block_parser.rb` has

```ruby
subtree = parse_subtree(["=begin\n", "<<< #{basename}\n", "=end\n"])
```

inside an embedded document, and the rest of the file was lost to it.
Rewritten line by line to CRuby's rule: the terminator is a line that
*begins* with `=end` and has whitespace or a line ending after it.

The old `[\s*]*=end` also let an **indented** `=end` close a document. CRuby
does not — that file is an unterminated embedded document and a syntax error.
The grammar now rejects it too, and both directions are in `test/negative`.

## 0010 — call with a line break before the dot arguments

1 file. `argument_list` opens with `token.immediate('(')` so that `foo (1)`
stays a command call whose single argument happens to be parenthesised. After
a call operator with no method name — the `foo.(args)` sugar for
`foo.call(args)` — there is no such ambiguity, and Ruby allows a line break.
representable splits exactly there:

```ruby
Collect[*bin.default_render_fragment_functions].
  (represented, {doc: doc, fragment: represented})
```

That position gets an argument list without the immediacy constraint. The new
corpus test asserts `foo (1)` is still a command call.

## 0011 — more than one call chained onto a do block

1 file. `_chained_command_call` took only a `command_call_with_block` as its
receiver, so exactly one call could follow a `do` block:

```ruby
should.raise Foo do
  bar
end.message.should.match(/x/)   # end.message parsed; .should did not
```

Made left-recursive. This is the same rule patch 0003 put a precedence on, so
it was worth ruling out as fallout: the packaging-only baseline rejects it
too, and always did.

## 0012 — element reference on a string with a leading space

1 file. The scanner emits `_element_reference_bracket` for a `[` with
whitespace before it only when an expression cannot start at that position.
After a string literal one still can — adjacent string literals concatenate,
so `STRING_START` stays valid — but a string is not a method and cannot take
command arguments, so `[` is the only possible reading. The scanner cannot
see that; the grammar can, so the case is expressed there. fog writes:

```ruby
tests('Compute::VcloudDirector | media' ['attributes']) do
```

`'a' 'b'` still chains and `puts ['a']` still takes an array argument, both
asserted in the corpus test.

## 0013 — rescue clauses are ordered

Found by measuring accepts-invalid, not by the sweep — the sweep is exhausted
for this grammar (0 gaps over 44,292 files) and can only ever find
rejects-valid.

`_body_statement` was `repeat(choice(rescue, else, ensure))`, which says
nothing about order or count, so four shapes CRuby rejects parsed clean:
`else` with no `rescue` (in a `begin` and in a method body alike), `ensure`
before `rescue`, and `else` before `rescue`. Ruby's `bodystmt` is
`stmts? rescue* (else stmts)? ensure?`, and "else without rescue is useless"
is a SyntaxError rather than a warning.

All four are now in `test/negative`. The sweep is unchanged at 44,238 with 0
gaps, so the tightening costs no real-world file.

It also required editing an **upstream** corpus test. `begin with else`
asserted that `begin / foo / else / bar / end` parses — which CRuby rejects —
so the test encoded the bug; it now uses the form that includes a `rescue`.
