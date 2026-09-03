/// <reference types="tree-sitter-cli/dsl" />
// @ts-nocheck

// treebank YAML — YAML 1.1 and 1.2 in one grammar.
//
// YAML is a lexer's language. Where a block collection begins and ends, how
// far a plain scalar runs, and whether a `:` is a mapping indicator or one
// character of text are all questions about COLUMNS and about what is
// already open — none of them expressible in a context-free rule. So this
// grammar is deliberately thin and `src/scanner.c` is thick: the scanner
// owns every indicator whose meaning depends on context and every scalar,
// and the grammar assembles what it hands back. FIELD_GUIDE.md §1 calls the
// lexer the highest rung of the ambiguity ladder; for YAML it is very nearly
// the only rung, and the parse table has no declared conflicts as a result.
//
// Two structural ideas are worth stating before the rules.
//
// There is NO "a block mapping starts here" token. A block mapping is
// `pair+` and a pair is `node ':' value?`, so the parser reads a key as an
// ordinary node and the `:` that follows is what turns it into a mapping.
// That is left-factoring (FIELD_GUIDE.md §1, rung 2) rather than a
// prediction the scanner would have to make by reading the line ahead.
//
// And there is ONE node rule for every position — key, value, document
// root, flow entry. Two would mean two productions with the same right-hand
// side, which is a permanent reduce-reduce conflict that static precedence
// can only settle by killing one of them for every lookahead
// (FIELD_GUIDE.md §3). Reading a key and reading a value are the same act
// here; what differs is only what may follow.

const { assertStructuralTerms } = require('../treebank/vocabulary/terms.js');

module.exports = grammar({
  name: 'yaml',

  externals: $ => [
    // Zero-width. Closes the innermost open block collection; the scanner
    // emits one per call, so a run of closes is a run of calls.
    $._block_end,
    // Zero-width, and the reason a value on a FOLLOWING line can be told
    // from the next key at the same column: the scanner offers it only
    // where the next line opens a node the enclosing collection contains.
    $._indented,
    // Zero-width, its opposite: there is no node in this position at all.
    // A token is needed rather than nothing, because the parse table's
    // choice between "shift this scalar as the value" and "reduce an empty
    // value and read it as the next key" is made on the lookahead, and
    // declining to lex leaves the parser with no lookahead to decide on.
    $._empty_node,
    $._block_seq_bullet,
    $._block_map_colon,
    // The same spelling as `_block_map_colon`, emitted where the `:` is the
    // first content on its line. An implicit key and its colon share a
    // line, so a colon that BEGINS one cannot belong to the key above it —
    // it can only be the value indicator of an explicit `? key` pair or of
    // a pair with no key. The parse table offers both readings and the
    // lexer starves the wrong one.
    $._own_line_colon,
    $._block_map_question,
    $._document_start,
    $._document_end,
    $._anchor_sigil,
    $._alias_sigil,
    $.anchor_name,
    $._flow_seq_start,
    $._flow_seq_end,
    $._flow_map_start,
    $._flow_map_end,
    $.tag,
    $.plain_scalar,
    $.single_quote_scalar,
    $.double_quote_scalar,
    $.block_scalar,
    // Never produced. If it is "valid" the parser is in error recovery,
    // where every symbol is offered and nothing the scanner emits can be
    // justified from its own state (FIELD_GUIDE.md §8).
    $._error_sentinel,
  ],

  extras: $ => [$.comment, /[ \t]/, /\r?\n/],

  // Substituted at its use sites rather than existing as a rule of its own;
  // the reason is written out at `_implicit_key`.
  inline: $ => [$._implicit_key],

  supertypes: $ => assertStructuralTerms([
    '_expression',
    '_literal',
    '_name',
    '_type',
    '_directive',
  ]).map(name => $[name]),

  rules: {
    // A YAML stream is a sequence of documents. Nothing above the document
    // level exists in the language, so this is the whole of it.
    //
    // A `...` with no document in front of it closes nothing and produces
    // nothing; it is a marker in the stream rather than a document, and
    // wrapping one in a `document` would put an empty node in the tree that
    // the file does not contain.
    stream: $ => repeat(choice($.document, $._document_end)),

    // Right-associative: after a `---` the node that follows belongs to
    // THAT document. The alternative reading — end this document, start a
    // new bare one — is not YAML, and deciding it at generate time keeps
    // the stream level fork-free.
    document: $ => prec.right(choice(
      seq(repeat1($._directive), $._document_start, optional($._expression), optional($._document_end)),
      seq($._document_start, optional($._expression), optional($._document_end)),
      seq($._expression, optional($._document_end)),
    )),

    _directive: $ => $.directive,

    // `%YAML 1.2`, `%TAG !e! tag:example.com,2000:app/`, and whatever a
    // future revision reserves. One node type for all of them, because the
    // SYNTAX is one production — a name and its arguments — and which
    // directive a name selects is the processor's business rather than the
    // grammar's.
    directive: $ => seq(
      field('name', $.directive_name),
      repeat(field('parameter', $.directive_parameter)),
    ),
    directive_name: $ => token(seq('%', /[^\s]+/)),
    directive_parameter: $ => token(/[^\s#][^\s]*/),

    // ── nodes ────────────────────────────────────────────────────────────

    // Everything a YAML node can denote, which in a data language is
    // everything there is. The block collections sit here alongside the
    // flow ones rather than in a tier of their own: what keeps a block
    // mapping out of `[ … ]` is the LEXER — the scanner emits no bullet, no
    // `_block_end` and no block scalar while its flow depth is non-zero —
    // so the branch is dead there rather than wrong, and every node
    // occurrence in the language answers `(_expression)`.
    _expression: $ => choice(
      $.block_mapping,
      $.block_sequence,
      $.annotated_node,
      // Outside `_inline` on purpose: an alias is the one node that can
      // carry no properties of its own. `&b *a` is not YAML, because the
      // alias IS the node and the anchor would have nothing to attach to,
      // and keeping `alias` out of what `annotated_node` may contain is the
      // whole of that rule.
      $.alias,
      $._inline,
    ),

    // What a node may be when it shares a line with whatever introduced it.
    // A block collection cannot: it needs a line of its own.
    _inline: $ => choice(
      $.flow_mapping,
      $.flow_sequence,
      $._literal,
    ),

    // The implicit key of a pair. Written out rather than named, because a
    // rule of its own would have the same right-hand side as `_expression`'s
    // and two productions spelled identically are a reduce-reduce conflict
    // on every lookahead (FIELD_GUIDE.md §3). Inline, the parser reads the
    // node once and the `:` behind it decides whether to shift into a pair
    // or reduce to a value.
    //
    // Block collections are absent because an implicit key is one line and
    // a block collection is not; `?` is how YAML writes a collection key,
    // and it is a separate branch below.
    _implicit_key: $ => choice($.annotated_node, $.alias, $._inline),

    // A node carrying an anchor, a tag, or both. It is a node in its own
    // right — `!!str a` denotes a value the way `a` does — which is why it
    // WRAPS rather than sitting beside its content: the tag belongs to that
    // node, and a query for the value should get the whole of it.
    //
    // The split between the two branches is the reason this rule exists.
    // `&a foo: 1` and `&a` / newline / `  foo: 1` are the same tokens in
    // the same order, and YAML tells them apart by a line break: the first
    // anchors the KEY, the second anchors the mapping. Requiring
    // `_indented` in front of a block collection is that line break made
    // into a token, and it is what keeps the two readings from being a
    // permanent two-way fork that no dynamic precedence would settle before
    // the end of the mapping (FIELD_GUIDE.md §2).
    annotated_node: $ => prec.right(2, choice(
      seq($._properties, optional(choice($._inline, $._empty_node))),
      seq($._properties, $._indented, $._expression),
    )),

    _properties: $ => prec.right(choice(
      seq($.anchor, optional($._type)),
      seq($._type, optional($.anchor)),
    )),

    // A tag is the node's TYPE — `!!str`, `!!int`, `!MyClass` — which is
    // the one place the treebank `_type` term lands in a data language.
    _type: $ => $.tag,

    anchor: $ => seq($._anchor_sigil, field('name', $._name)),
    alias: $ => seq($._alias_sigil, field('name', $._name)),
    // One node type for both, because an alias's name IS an anchor name.
    _name: $ => $.anchor_name,

    // Every one of these is fully determined by its own text. Whether `no`
    // is a boolean and `0o14` an integer is RESOLUTION, decided by a schema
    // this grammar has no opinion about.
    _literal: $ => choice(
      $.plain_scalar,
      $.single_quote_scalar,
      $.double_quote_scalar,
      $.block_scalar,
    ),

    // ── block collections ────────────────────────────────────────────────

    block_mapping: $ => seq(repeat1($.block_mapping_pair), $._block_end),

    // Ranked above `_block_value`: a node with a `:` behind it is a KEY,
    // not a finished value. This is the left-factored form of "is this line
    // a mapping" — the parser reads the node either way and the colon
    // settles it — and it is what keeps the cell deterministic instead of
    // forking at every scalar in the language. Right-associative for the
    // same reason `document` is: the greedy reading of a pair's optional
    // halves is the only YAML one.
    block_mapping_pair: $ => prec.right(1, choice(
      seq(
        field('key', $._implicit_key),
        $._block_map_colon,
        optional(choice(field('value', $._block_value), $._empty_node)),
      ),
      // The explicit-key form. `?` introduces a key that may be a
      // collection or span lines, which the implicit form forbids.
      seq(
        $._block_map_question,
        optional(choice(field('key', $._block_value), $._empty_node)),
        optional(seq(
          choice($._block_map_colon, $._own_line_colon),
          optional(choice(field('value', $._block_value), $._empty_node)),
        )),
      ),
      // A pair with no key at all: `: value`, and `:` alone. Ranked BELOW
      // the explicit form, so the `:` line of a `? key` / `: value` pair
      // joins the key it belongs to instead of starting a keyless pair of
      // its own.
      prec(-1, seq(
        $._own_line_colon,
        optional(choice(field('value', $._block_value), $._empty_node)),
      )),
    )),

    block_sequence: $ => seq(repeat1($.block_sequence_item), $._block_end),

    // Right-associative rather than a declared conflict: a second `-` with
    // no `_block_end` between it and the first can only be a nested compact
    // sequence, because a sibling entry at the same column always arrives
    // behind the close of whatever the first entry opened. The decision is
    // made at generate time and no fork is created for it
    // (FIELD_GUIDE.md §1, rung 3).
    block_sequence_item: $ => prec.right(seq(
      $._block_seq_bullet,
      optional(choice($._block_value, $._empty_node)),
    )),

    // A value is either on the same line as its indicator, or on a
    // following line that `_indented` has vouched for.
    _block_value: $ => choice(
      $._expression,
      seq($._indented, $._expression),
    ),

    // ── flow collections ─────────────────────────────────────────────────

    flow_sequence: $ => seq(
      $._flow_seq_start,
      optional(seq(sepBy1(',', $._flow_seq_entry), optional(','))),
      $._flow_seq_end,
    ),
    _flow_seq_entry: $ => choice($.flow_pair, $._expression),

    flow_mapping: $ => seq(
      $._flow_map_start,
      optional(seq(sepBy1(',', $._flow_map_entry), optional(','))),
      $._flow_map_end,
    ),
    _flow_map_entry: $ => choice($.flow_pair, $._expression),

    flow_pair: $ => prec.right(2, choice(
      seq(
        field('key', $._implicit_key),
        $._flow_colon,
        optional(field('value', $._expression)),
      ),
      seq($._flow_colon, optional(field('value', $._expression))),
      seq(
        $._block_map_question,
        optional(field('key', $._implicit_key)),
        optional(seq($._flow_colon, optional(field('value', $._expression)))),
      ),
    )),

    // Inside a flow collection the commas and brackets delimit, so both
    // spellings of `:` are safe there and the line a colon sits on says
    // nothing: `{ "foo"` / `  :bar }` is one pair across two lines.
    //
    // Ranked above the block pair rules, and that ranking is the price of
    // `_expression` carrying the block collections. A flow entry is
    // GRAMMATICALLY allowed to be a block mapping; what keeps one out of
    // `[ … ]` is the lexer, which generation cannot see, so inside a flow
    // collection an explicit `? … :` reaches both pair rules and something
    // has to decide. Deciding it here costs nothing at runtime, where a
    // declared conflict would have forked over a cell whose lookahead — a
    // block sequence bullet at non-zero flow depth — the scanner will never
    // produce. One line, and it buys `(_expression)` matching every node in
    // the language, the entries of a flow collection included.
    _flow_colon: $ => prec(3, choice($._block_map_colon, $._own_line_colon)),

    comment: $ => token(seq('#', /[^\n\r]*/)),
    comment: $ => token(seq('#', /[^\n\r]*/)),
  },
});

function sepBy1(sep, rule) {
  return seq(rule, repeat(seq(sep, rule)));
}
