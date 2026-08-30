/**
 * treebank-json: a from-scratch grammar for RFC 8259 JSON, carrying the
 * treebank vocabulary (DESIGN.md §3) in its parse table.
 *
 * This is the smallest grammar in the repository and it is meant to be.
 * JSON has no identifiers, no operators, no statements and no scopes: a
 * document is ONE value, a value is one of seven shapes, and five of those
 * seven are single tokens. Anything much longer than this file would be a
 * grammar for something other than JSON.
 *
 * STRICT RFC 8259, and that is the decision the rest of the crate is built
 * around; ledger.toml argues it at length. The short form: JSONC, JSON5 and
 * NDJSON are not later JSONs, they are separate languages that CONTAIN
 * JSON, each defined by its own tool, with no successor relation to appeal
 * to — so DESIGN.md §4.2's "latest version wins" has nothing to order.
 * What JSON does have is a version union, and it is one line long: RFC 4627
 * required the top level to be an object or an array, RFC 7159 and 8259
 * allow any value there, and the later reading is the one `document` takes.
 *
 * Three token decisions are load-bearing, and every one of them is settled
 * by a file in test/negative/ that would otherwise be accepted:
 *
 * - `extras` is `/[ \t\n\r]/` and NOT `/\s/`. JSON's whitespace is exactly
 *   those four characters (RFC 8259 §2); `\s` additionally matches form
 *   feed, vertical tab and the Unicode spaces. Measured by swapping it and
 *   re-running the suite: `/\s/` accepts n_structure_whitespace_formfeed.
 * - `number` is the RFC's production spelled out, not a permissive numeric
 *   token. `.2e-3`, `+1`, `-01`, `2.e3`, `0x1`, `NaN` and `Infinity` are
 *   each a must-reject file, and jq 1.7 accepts twenty of them — which is
 *   most of the oracle argument in ledger.toml.
 * - `string_content` excludes U+0000-U+001F. An unescaped control character
 *   in a string is a must-reject (RFC 8259 §7), and a content rule written
 *   as "anything but a quote or a backslash" takes it.
 *
 * Where the vocabulary does and does not reach is in ledger.toml's
 * roles_note. The short version: two of twenty-two table-tier terms, and
 * the twenty absences are facts about JSON rather than gaps.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank/vocabulary/supertypes.js');

module.exports = grammar({
  name: 'json',

  // RFC 8259 §2: ws = *( %x20 / %x09 / %x0A / %x0D ). Written out rather
  // than as `/\s/`, which is wider than JSON in exactly the directions
  // that matter — form feed, vertical tab, U+00A0, U+2028 and the rest of
  // the Unicode space property are not JSON whitespace.
  extras: _ => [/[ \t\n\r]/],

  // None. JSON's tokenizer has no state at all: no nesting to count, no
  // delimiter to remember, no keyword that is sometimes an identifier,
  // because there are no identifiers. Every token here is a regular
  // language over bytes, which is the property that keeps the whole
  // grammar to one screen.
  externals: _ => [],

  supertypes: $ => tb.assertTableTerms([
    '_expression',
    '_literal',
  ]).map((name) => $[name]),

  conflicts: _ => [],

  rules: {
    // A JSON text is a single value. RFC 4627 restricted that value to an
    // object or an array; RFC 7159 lifted the restriction and 8259 keeps
    // it lifted, so `3`, `"x"` and `null` are each a whole document. That
    // is the entirety of JSON's version union (DESIGN.md §4.2), and the
    // later reading wins as it does everywhere else in this repository.
    //
    // Exactly one value: whatever follows it is an error rather than a
    // second document. That is what separates JSON from NDJSON, and it is
    // a rejection with two must-reject files behind it (`[][]` and
    // `{"a": true} "x"`) rather than a stylistic preference.
    document: $ => $._expression,

    // Every JSON value denotes a value and computes nothing, so there is
    // one expression tier and `_literal` nests inside it rather than
    // sitting beside it. `object` and `array` are deliberately NOT
    // `_literal`, even though JSON is the one language here where they
    // would satisfy its definition; ledger.toml's roles_note says why.
    _expression: $ => choice(
      $.object,
      $.array,
      $._literal,
    ),

    _literal: $ => choice(
      $.string,
      $.number,
      $.true,
      $.false,
      $.null,
    ),

    object: $ => seq('{', optional(commaSep1($.pair)), '}'),

    // The key is a `string` and nothing else. It is deliberately not
    // threaded as `_name`: JSON has no identifiers, and `_name` is the
    // role for a name in a naming position rather than for a string that
    // happens to be used as one — typescript already spells a
    // string-keyed property this way. So `(_string)` finds keys and values
    // alike and `(_name)` finds nothing, which is the truth about JSON.
    // What the key position DOES get is the `key` field on a `_clause` —
    // roles.json threads `pair` there, the way yaml threads its mapping
    // pair — which is what colours a key as a key rather than as a string.
    pair: $ => seq(
      field('key', $.string),
      ':',
      field('value', $._expression),
    ),

    array: $ => seq('[', optional(commaSep1($._expression)), ']'),

    // The closing quote is `token.immediate` and the opening one is not,
    // which is a same-text token split and the only structural smell in
    // this grammar. It is here because `extras` does not stop at a rule
    // boundary: a string's body is ordinary grammar, so the lexer is free
    // to skip whitespace inside it, and a plain `'"'` close lets it. The
    // file that proves it is n_string_unescaped_tab — a string whose whole
    // body is a literal tab, which RFC 8259 §7 forbids. With a skippable
    // close the tab is booked as `extras`, the string parses as EMPTY, and
    // the grammar reports a clean tree for a file the standard rejects.
    // An immediate close forbids the skip, so the tab has nowhere to go.
    string: $ => seq(
      '"',
      repeat(choice($.string_content, $.escape_sequence)),
      token.immediate('"'),
    ),

    // RFC 8259 §7: any codepoint except `"`, `\` and U+0000-U+001F. The
    // control-character exclusion is the whole of this rule's strictness
    // and it is what rejects a literal tab or newline inside a string.
    //
    // `token.immediate` is load-bearing rather than decorative, and the
    // cost of dropping it was measured rather than assumed. A string's
    // body is ordinary grammar, so `extras` is skippable inside it: with a
    // plain token here, `"a<LF>b"` parses cleanly as two content runs with
    // the newline BOOKED AS WHITESPACE and gone from the tree. RFC 8259 §7
    // forbids that newline, and n_string_unescaped_newline is the file —
    // one must-reject accepted, and a character silently lost from every
    // string that survives.
    string_content: _ => token.immediate(prec(1, /[^"\\\u0000-\u001f]+/)),

    escape_sequence: _ => token.immediate(/\\(["\\\/bfnrt]|u[0-9a-fA-F]{4})/),

    // RFC 8259 §6, exactly: an optional minus, an integer part with no
    // leading zeros, an optional fraction that must have a digit on both
    // sides of the point, and an optional exponent that must carry at
    // least one digit. No `+` sign, no bare `.5`, no trailing `1.`, no
    // hex, no `Infinity` and no `NaN` — every one of which is a file in
    // test/negative/.
    number: _ => token(seq(
      optional('-'),
      choice('0', /[1-9]\d*/),
      optional(seq('.', /\d+/)),
      optional(seq(/[eE]/, optional(/[-+]/), /\d+/)),
    )),

    // Named so a query can match them. They carry no text beyond the
    // keyword, which is what makes them `_literal` in the vocabulary's
    // sense: the value is fully determined by the text, for every instance
    // of the rule.
    true: _ => 'true',
    false: _ => 'false',
    null: _ => 'null',
  },
});

/**
 * One or more `rule`, separated by commas, with NO trailing comma. The
 * absent `optional(',')` at the end is the JSONC decision made concrete:
 * typescript's `commaSep1` carries one and JSON's does not.
 *
 * @param {RuleOrLiteral} rule
 * @returns {SeqRule}
 */
function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}
