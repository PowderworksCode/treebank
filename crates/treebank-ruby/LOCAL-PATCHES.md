# Local patches — treebank-ruby

Upstream: [tree-sitter/tree-sitter-ruby](https://github.com/tree-sitter/tree-sitter-ruby)
pinned at `ad907a69da0c8a4f7a943a7fe012712208da6dee`, master head, four
commits past tag `v0.23.1`.

The pin is past the tag on purpose. `ad907a6` is *scanner: fix heredoc
serialization buffer overflows* — a correctness fix in the external scanner,
in a language where heredocs are everywhere. The other three commits touch
the Rust/wasm bindings and the LICENSE include, not the parser.

Ten patches: two packaging, eight grammar. On the 500-gem, 25,604-file
corpus they take the sweep from 25,542 passing with 22 grammar gaps to
**25,564 passing with 0**. The 40 files still failing are ERB templates that
CRuby also rejects. `noise_files` was 40 before the first patch and 40 after
every one of the eight — measured at each patch level — so no patch bought a
passing file by accepting something the reference parser rejects.

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
