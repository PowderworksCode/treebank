/**
 * treebank-hcl: a from-scratch grammar for the HCL2 native syntax,
 * carrying the treebank vocabulary (DESIGN.md §3) in its parse table.
 *
 * THE LANGUAGE IS HCL; TERRAFORM IS A DIALECT OF IT. `.tf` and `.tfvars`
 * are Terraform's file names for HCL and nothing else: a Terraform file
 * adds a block schema, a function table and a variable namespace on top of
 * this syntax, and every one of those is semantics. There is no `.tf`
 * production below, because there is no `.tf` syntax. See ledger.toml.
 *
 * The decision that shapes everything here is that HCL IS NEWLINE
 * SENSITIVE, and only in some of its brackets. A newline terminates an
 * attribute and separates the items of a body; it is invisible inside
 * `(…)`, `[…]`, a call's arguments and an index; and inside an OBJECT it
 * is a real token again, separating elements. hclsyntax expresses this
 * with a recursive-descent parser carrying a newline-inclusion stack,
 * which an LR table cannot copy — there is one expression grammar here,
 * not one per newline mode.
 *
 * So the decision is pushed to the highest rung of the ambiguity ladder
 * (FIELD_GUIDE.md §1): `_newline` is an EXTERNAL token, and the scanner
 * emits it only where `valid_symbols` says the parse table wants one.
 * Everywhere else the newline falls through to `extras` and is trivia.
 * The parser never sees a decision, because the lexer already made it, and
 * the whole of "newlines are invisible inside brackets" costs zero rules:
 * a tuple simply never admits `_newline`, so the scanner never offers one.
 * What that CANNOT express is a newline INSIDE an expression that is not
 * finished: after a trailing operator at body level, and at every position
 * inside an object but the separator. Both are widenings on input HCL
 * rejects, both are declared in ledger.toml, and both need the
 * newline-inclusion mode hclsyntax has — which is a second copy of the
 * expression grammar, not a rule.
 *
 * The second decision is that HCL HAS NO RESERVED WORDS, which inverts
 * FIELD_GUIDE.md §5. `for = 1`, `x = for`, `true {}` and
 * `[for for in y : for]` are all valid, because a keyword's role is
 * contextual: `for` is a keyword only immediately after the `[` or `{`
 * that could open a for-expression, `in` only after the loop variables.
 * tree-sitter's keyword extraction is exactly that rule — the word token
 * is re-lexed as a keyword only in states where the keyword is valid — so
 * declaring `word: $ => $.identifier` and NO reserved set is not the
 * omission §5 warns about here; it is the language's own rule. The
 * negative corpus carries the cases that prove it, in both directions.
 *
 * The third is the template sub-language, which is why there is a
 * scanner at all. A quoted template and a heredoc are the same grammar
 * over different literal text — a heredoc runs to a delimiter line and
 * does not process backslash escapes, a quoted template runs to `"` and
 * does — and either can nest inside the other through an interpolation
 * (`"${<<EOT\n…\nEOT\n}"` is valid HCL). The scanner therefore carries a
 * MODE STACK rather than a heredoc flag, and owns the quote characters so
 * the stack stays in step with the parse.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank/vocabulary/terms.js');

// HCL's own precedence table (hclsyntax/spec.md, "Operations"), lowest
// binding first. Levels 1-6 are the spec's; the conditional sits below
// them all and the suffix operators above the unary ones.
const PREC = {
  conditional: 1,
  or: 2,
  and: 3,
  equality: 4,
  comparison: 5,
  additive: 6,
  multiplicative: 7,
  unary: 8,
  suffix: 9,
};

/**
 * `%{ endif }` and its two siblings, as ONE token each, with the strip
 * markers and the whitespace HCL admits between them.
 *
 * @param {string} keyword
 */
function directiveCloser(keyword) {
  return token(seq(
    '%{',
    optional('~'),
    /[ \t\r\n]*/,
    keyword,
    /[ \t\r\n]*/,
    optional('~'),
    '}',
  ));
}

module.exports = grammar({
  name: 'hcl',

  word: $ => $.identifier,

  // A newline is NOT trivia everywhere, so it is listed separately from
  // the horizontal whitespace: where the grammar wants one the scanner has
  // already taken it as `_newline` and this rule never sees it. A lone
  // carriage return is deliberately absent — HCL admits `\r` only as part
  // of a `\r\n` pair, and a file with bare-CR line endings is a parse
  // error there too.
  extras: $ => [
    /[ \t]/,
    /\r?\n/,
    $.comment,
    $.block_comment,
  ],

  externals: $ => [
    // The line terminator, emitted only where the table admits one.
    $._newline,
    // The quote characters of a quoted template. External because the
    // scanner's mode stack has to know when one opens and closes: the
    // literal text of a heredoc and of a quoted template are the same
    // token here, and only the stack says which rule scans it.
    $._quote_open,
    $._quote_close,
    // One line, or one run up to the next escape or introduction, of
    // literal template text, in whichever mode is on top.
    $._template_chunk,
    // `<<EOT` / `<<-EOT` through the newline, and the delimiter line that
    // closes it. The closing newline is left for `_newline`, so a heredoc
    // that ends at EOF without one is rejected exactly as HCL rejects it.
    $._heredoc_open,
    $._heredoc_close,
    // Never produced. Every symbol is marked valid during error recovery,
    // so a scanner that cannot tell it is in recovery will emit tokens it
    // has no business emitting (FIELD_GUIDE.md §8).
    $._error_sentinel,
  ],

  supertypes: $ => tb.assertStructuralTerms([
    '_expression',
    '_declaration',
    '_name',
    '_literal',
    '_argument',
    '_body',
    '_control_flow',
    '_branch',
    '_loop',
    '_invocation',
    '_access',
    '_interpolation',
  ]).map((name) => $[name]),

  // None. Every ambiguity HCL contains is settled a rung higher: the
  // newline decisions in the scanner, the keyword decisions by keyword
  // extraction, the operator ladder by static precedence. A declared
  // conflict here would be a fork carrying a reading the language never
  // has (FIELD_GUIDE.md §2).
  conflicts: _ => [],

  rules: {
    // A file IS a body: the same attributes and blocks a `{ … }` holds,
    // with no wrapper of its own. `.tfvars` is not a second production —
    // it is a body that happens to contain only attributes, which is a
    // fact about Terraform's schema and not about this syntax.
    //
    // Every item carries a terminator EXCEPT the last, and only at end of
    // file: `blk { a = 1 }` with no trailing newline is valid HCL, while
    // the same file's `a = 1 }` inside a multi-line block is not. So the
    // allowance is here, at the top level, and nowhere else.
    config_file: $ => seq(
      repeat(seq($._declaration, $._newline)),
      optional($._declaration),
    ),

    // Both of a body's item kinds are declarations, and nothing else is
    // one: an attribute introduces a named value, a block introduces a
    // named entity with a body. HCL declares no `_statement` at all —
    // nothing here is executed for effect as an element of a sequence,
    // which is what "declarative" means when it is a fact about syntax
    // rather than a slogan.
    _declaration: $ => choice($.attribute, $.block),

    // `name = expr`. A `_declaration` rather than an `_assignment`: HCL
    // has no mutation and no places to store into, so an attribute is the
    // introduction of a named entity and nothing more.
    attribute: $ => seq(
      field('name', $._name),
      '=',
      field('value', $._expression),
    ),

    block: $ => seq(
      field('type', $._name),
      repeat(field('label', $._block_label)),
      field('body', $._body),
    ),

    // A label is a naked identifier or a QUOTED LITERAL — never a
    // template. `blk "${a}" {}` is rejected by hclsyntax, and the
    // `string_lit` token below is where that is (nearly) enforced; see
    // ledger.toml for the part of it a single token cannot say.
    _block_label: $ => choice($._name, $.string_lit),

    // The two forms are the spec's `Block` and `OneLineBlock`, and they
    // are genuinely different rules rather than one rule with optional
    // newlines. Once a body takes the newline after `{` it must have one
    // before `}` as well — `blk {\n a = 1 }` is a parse error — and a
    // one-line body admits exactly ONE attribute and no nested block.
    _body: $ => choice($.body),

    body: $ => choice(
      seq('{', $._newline, repeat(seq($._declaration, $._newline)), '}'),
      seq('{', optional($.attribute), '}'),
    ),

    // ---------------------------------------------------------------
    // Expressions
    // ---------------------------------------------------------------

    _expression: $ => choice(
      $._expr_term,
      $.unary_expression,
      $.binary_expression,
      $._control_flow,
    ),

    // The expression sub-language's control flow, and only its own.
    //
    // The template `%{ if }` and `%{ for }` directives are the same two
    // ideas in the template sub-language and are NOT members here, which
    // was measured rather than assumed: a supertype is one alternation, so
    // putting both sub-languages' forms in `_branch` makes
    // `_control_flow` reachable at a template-part position AND as the
    // start of a conditional's condition at that same position, and
    // `tree-sitter generate` reports it as an unresolved conflict —
    // `(_expression _control_flow)` against `(_template_part
    // _control_flow)` — rather than as a widening a corpus might later
    // reveal. It is a real ambiguity in the table, not a lexical one the
    // scanner could close. So the term goes where it buys the most: over
    // real Terraform the for-EXPRESSION is the construct a `(_loop)` query
    // is asking about, by a wide margin over the template directive, and
    // the directives are recorded in terms.json's `uncategorised` with
    // this as the reason.
    _control_flow: $ => choice($._branch, $._loop),
    _branch: $ => choice($.conditional),
    _loop: $ => choice($.for_tuple_expr, $.for_object_expr),

    _expr_term: $ => choice(
      $._literal,
      $.quoted_template,
      $.heredoc_template,
      $.tuple,
      $.object,
      // Through the supertype rather than the node: a supertype nothing
      // REFERENCES is an unused rule, and generate drops it — silently, and
      // the loss shows up two steps later as a query that will not compile
      // against a term the grammar declares.
      $._invocation,
      $.identifier,
      $.parenthesized_expression,
      $._access,
    ),

    _literal: $ => choice(
      $.integer,
      $.float,
      $.true,
      $.false,
      $.null,
    ),

    // `NumericLit = decimal+ ("." decimal+)? (expmark decimal+)?`. Split
    // into two node types because a query wants the distinction and HCL's
    // single token does not give it: HCL has one arbitrary-precision
    // number type, so `1` and `1.0` are the same VALUE and different
    // SYNTAX, and the syntax is what a grammar is for. Neither form admits
    // a sign (that is the unary operator), a leading or trailing dot, an
    // underscore or a hex prefix — `0x10` and `1_000` are parse errors in
    // HCL and are parse errors here.
    integer: _ => token(/[0-9]+/),
    float: _ => token(choice(
      /[0-9]+\.[0-9]+([eE][-+]?[0-9]+)?/,
      /[0-9]+[eE][-+]?[0-9]+/,
    )),

    true: _ => 'true',
    false: _ => 'false',
    null: _ => 'null',

    // `ID_Start (ID_Continue | '-')*`, with the dash HCL adds so a block
    // type or attribute name may be written `foo-bar`. The classes are the
    // Unicode general categories UAX#31 is defined over; what they leave
    // out is `Other_ID_Start` and `Other_ID_Continue`, two small legacy
    // lists the property adds on top, and that narrowing is declared in
    // ledger.toml.
    identifier: _ => token(/[\p{L}\p{Nl}_][\p{L}\p{Nl}\p{Mn}\p{Mc}\p{Nd}\p{Pc}_-]*/u),

    _name: $ => choice($.identifier),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    unary_expression: $ => prec(PREC.unary, seq(
      field('operator', choice('-', '!')),
      field('operand', $._expression),
    )),

    binary_expression: $ => {
      const table = [
        [PREC.multiplicative, choice('*', '/', '%')],
        [PREC.additive, choice('+', '-')],
        [PREC.comparison, choice('>', '>=', '<', '<=')],
        [PREC.equality, choice('==', '!=')],
        [PREC.and, '&&'],
        [PREC.or, '||'],
      ];
      return choice(...table.map(([precedence, operator]) => prec.left(
        // @ts-ignore
        precedence,
        seq(
          field('left', $._expression),
          // @ts-ignore
          field('operator', operator),
          field('right', $._expression),
        ),
      )));
    },

    // `a ? b : c`. Right associative so `a ? b : c ? d : e` nests to the
    // right, which is how HCL reads it.
    conditional: $ => prec.right(PREC.conditional, seq(
      field('condition', $._expression),
      '?',
      field('consequence', $._expression),
      ':',
      field('alternative', $._expression),
    )),

    // ---------------------------------------------------------------
    // Collections
    // ---------------------------------------------------------------

    // A tuple's elements are separated by COMMAS only. Newlines inside the
    // brackets are trivia — `[1\n2]` is a missing-comma error in HCL, not
    // two elements — and that falls out of `_newline` never being valid
    // here, so the scanner never offers one.
    tuple: $ => seq(
      '[',
      optional(seq(
        $._expression,
        repeat(seq(',', $._expression)),
        optional(','),
      )),
      ']',
    ),

    // An object's elements are separated by a comma OR a newline, which is
    // the one place inside brackets where HCL keeps newlines significant.
    // The two are not interchangeable in both directions: `{a = 1,\nb = 2}`
    // is valid and `{a = 1\n, b = 2}` is not, because a newline after a
    // comma is trivia and a comma after a newline is a second separator.
    // That asymmetry is why the separator is a rule of its own rather than
    // a `choice(',', $._newline)` — the `optional($._newline)` inside it is
    // what makes the first form parse and the second one fail.
    object: $ => seq(
      '{',
      optional($._newline),
      optional(seq(
        $.object_elem,
        repeat(seq($._object_separator, $.object_elem)),
        optional($._object_separator),
      )),
      '}',
    ),

    _object_separator: $ => choice(
      seq(',', optional($._newline)),
      $._newline,
    ),

    // `(Identifier | Expression) ("=" | ":") Expression`. The two are one
    // rule here because an identifier IS an expression: `{foo = 1}` and
    // `{(foo) = 1}` differ in the tree by the parentheses, which is
    // exactly the distinction hclsyntax draws between a literal attribute
    // name and a computed one.
    object_elem: $ => seq(
      field('key', $._expression),
      choice('=', ':'),
      field('value', $._expression),
    ),

    // ---------------------------------------------------------------
    // For expressions
    // ---------------------------------------------------------------

    for_tuple_expr: $ => seq(
      '[',
      $._for_intro,
      field('result', $._expression),
      optional(field('condition', $.for_cond)),
      ']',
    ),

    // The leading `optional($._newline)` is not decoration. A `{` may open
    // an object or a for-object, so at that one position the table admits
    // an object's leading newline and the scanner therefore offers one —
    // and a for-object that did not admit it would fail on every
    // `{\nfor v in y : …}`. Inside the for-object nothing else admits a
    // newline, so every other line break in it is trivia, which is what
    // HCL does with them.
    for_object_expr: $ => seq(
      '{',
      optional($._newline),
      $._for_intro,
      field('key', $._expression),
      '=>',
      field('value', $._expression),
      optional(field('grouping', $.ellipsis)),
      optional(field('condition', $.for_cond)),
      '}',
    ),

    _for_intro: $ => seq(
      'for',
      field('binding', $._name),
      optional(seq(',', field('binding', $._name))),
      'in',
      field('collection', $._expression),
      ':',
    ),

    for_cond: $ => seq('if', field('condition', $._expression)),

    ellipsis: _ => '...',

    // ---------------------------------------------------------------
    // Calls and access
    // ---------------------------------------------------------------

    // `FunctionCall = Identifier "(" arguments ")"`, and the identifier is
    // load-bearing: HCL has no callable expressions, so `a.b(1)` and
    // `f(a)(b)` are parse errors and are not admitted here. What the name
    // MAY carry is `::` separators, which is how a Terraform provider
    // function is spelled (`provider::aws::arn_parse(…)`); `a::b` outside
    // a call position is a parse error, so the separator lives in this
    // rule rather than in the identifier token.
    _invocation: $ => choice($.function_call),

    function_call: $ => seq(
      field('function', $.function_name),
      '(',
      optional($.arguments),
      ')',
    ),

    function_name: $ => seq($._name, repeat(seq('::', $._name))),

    arguments: $ => seq(
      $._argument,
      repeat(seq(',', $._argument)),
      optional(choice(',', $.ellipsis)),
    ),

    _argument: $ => choice($._expression),

    // The operand of every suffix operator. A for-expression is an
    // ExprTerm in HCL's own grammar and so may be indexed, accessed and
    // splatted — `[for k, h in var.hosts : h if h.ip != ""][0]` is how a
    // module picks the first match, and it appeared in real configuration
    // before it appeared in a fixture. It cannot simply JOIN `_expr_term`,
    // because that would put it under both `_expr_term` and
    // `_control_flow` at the one `_expression` position and generate
    // rejects that outright (DESIGN.md §2, fact 3). Naming it a second
    // time HERE is the way round: this is a different position, so
    // `(_loop)` still has exactly one derivation wherever it matches.
    //
    // The conditional is deliberately NOT in this list. HCL's
    // `Conditional` is not an ExprTerm, so `a ? b : c[0]` indexes `c` and
    // indexing the whole conditional needs parentheses — which is also
    // what this grammar does.
    _operand: $ => choice($._expr_term, $._loop),

    _access: $ => choice(
      $.get_attr,
      $.index,
      $.legacy_index,
      $.attr_splat,
      $.full_splat,
    ),

    get_attr: $ => prec(PREC.suffix, seq(
      field('operand', $._operand),
      '.',
      field('name', $._name),
    )),

    index: $ => prec(PREC.suffix, seq(
      field('operand', $._operand),
      '[',
      field('key', $._expression),
      ']',
    )),

    // `.0`, kept only for compatibility with HCL's precursor language and
    // required of a conforming parser. It does not chain — `foo.0.0` is a
    // number literal in the middle and a parse error — so the index digits
    // are one token and no rule admits a second.
    legacy_index: $ => prec(PREC.suffix, seq(
      field('operand', $._operand),
      field('key', token.immediate(/\.[0-9]+/)),
    )),

    // Greedy, which is the spec's reading: the attribute accesses after
    // `.*` belong to the splat (`attrSplat = "." "*" GetAttr*`), so where
    // `a.*.b` could reduce the splat and then read `.b` as an ordinary
    // access, it shifts instead. A trailing INDEX is a different matter and
    // is deliberately left outside — `a.*.b[0]` indexes the splat's result,
    // which is the whole point of the attribute-only form.
    attr_splat: $ => prec.right(PREC.suffix, seq(
      field('operand', $._operand),
      '.',
      '*',
      repeat(seq('.', field('name', $._name))),
    )),

    full_splat: $ => prec.right(PREC.suffix, seq(
      field('operand', $._operand),
      '[',
      '*',
      ']',
      repeat(choice(
        seq('.', field('name', $._name)),
        seq('[', field('key', $._expression), ']'),
      )),
    )),

    // ---------------------------------------------------------------
    // Templates
    // ---------------------------------------------------------------

    // A quoted template is not a `_literal`, and the reason is the term's
    // own per-RULE test (DESIGN.md §3.2): this rule can carry an
    // interpolation, so no instance of it qualifies, exactly as python's
    // `string` cannot. `_string` is the nominal term that answers "find every
    // string" over both.
    // The delimiters are NODES rather than hidden tokens, and that is not
    // decoration. They have to be external -- the scanner's mode stack
    // opens and closes on them -- and an external token is either named or
    // hidden, with no anonymous option; hidden would put a lexeme the
    // language spells nowhere in the tree. `treebank shape`'s lexical layer
    // is what made the difference visible: hclsyntax lexes `"` and `<<EOT`
    // as tokens of their own, and every one of them was a boundary we could
    // not show, across twelve different parent node kinds.
    //
    // `alias` rather than a `quote: choice($._quote_open, $._quote_close)`
    // rule, and the difference is not cosmetic. A shared rule makes BOTH
    // symbols valid wherever either is -- so at the OPENING quote the
    // scanner sees `_quote_close` in `valid_symbols`, pops the template
    // mode it was about to push, and every heredoc containing a nested
    // quoted template stops parsing. 32 corpus files said so. Aliasing
    // keeps the two symbols distinct in the table and gives them one node
    // name in the tree, which is what was wanted in the first place.
    quoted_template: $ => seq(
      alias($._quote_open, $.quote),
      repeat($._template_part),
      alias($._quote_close, $.quote),
    ),

    // Two names rather than one, because unlike the quote these are not the
    // same lexeme at both ends: the opener carries the `<<`, the indent
    // marker and the delimiter word, and the closer is the word.
    heredoc_template: $ => seq(
      alias($._heredoc_open, $.heredoc_start),
      repeat($._template_part),
      alias($._heredoc_close, $.heredoc_end),
    ),

    _template_part: $ => choice(
      $.template_literal,
      $._interpolation,
      $.template_if,
      $.template_for,
    ),

    // One node per RUN of literal text, not per chunk, and the escapes are
    // inside it. hclsyntax lexes a heredoc line by line and a quoted
    // literal in pieces around its escapes, then coalesces the pieces into
    // one `LiteralValueExpr`; this shape agrees with both halves of that,
    // which is what `treebank shape` checks. `prec.right` is what makes the
    // run greedy -- without it, "continue this literal" and "start a new
    // one" are the same decision on the same token.
    template_literal: $ => prec.right(repeat1(choice(
      $._template_chunk,
      $.escape_sequence,
    ))),

    _interpolation: $ => choice($.template_interpolation),

    // `${ … }`, with the optional `~` strip markers. The markers are part
    // of the delimiter's spelling rather than separate nodes, because a
    // `~` is only a marker when it is immediately inside the braces and is
    // not an operator anywhere in HCL.
    template_interpolation: $ => seq(
      choice('${', '${~'),
      field('expression', $._expression),
      choice('}', '~}'),
    ),

    // The CLOSING directives are single tokens, and that is the whole
    // reason this grammar has no declared conflicts.
    //
    // `template_body` is a named rule, so ending one is a REDUCE, and with
    // `%{` spelling both "a nested directive begins" and "this body is
    // over" the reduce and the shift compete on the same lookahead — an
    // LR(1) table cannot see the `if` or the `endif` that follows.
    // `tree-sitter generate` reports exactly that. The alternatives were a
    // declared conflict with a dynamic weight (a fork that dies one token
    // later, so cheap — but a fork at every `%{` in the language) or
    // flattening `template_body` and `else_clause` away, which buys the
    // table what it wants by throwing out the structure a consumer asked
    // for: with them gone, `else` is an anonymous token and "the
    // alternative branch" stops being a thing a query can name.
    //
    // Making the three closers one token each moves the decision to the
    // lexer, where longest-match settles it for free (FIELD_GUIDE.md §1),
    // and keeps both nodes. What it costs is a COMMENT inside a closing
    // directive — `%{ /* c */ endif }`, which hclsyntax accepts — and that
    // is a gap the sweep counts against us rather than a widening it
    // cannot see. Newlines and horizontal space are admitted, because HCL
    // admits them and heredoc templates use them.
    template_if: $ => seq(
      $._directive_start,
      'if',
      field('condition', $._expression),
      $._directive_end,
      optional(field('consequence', $.template_body)),
      optional($.else_clause),
      $._directive_endif,
    ),

    else_clause: $ => seq(
      $._directive_else,
      optional(field('alternative', $.template_body)),
    ),

    template_for: $ => seq(
      $._directive_start,
      'for',
      field('binding', $._name),
      optional(seq(',', field('binding', $._name))),
      'in',
      field('collection', $._expression),
      $._directive_end,
      optional(field('body', $.template_body)),
      $._directive_endfor,
    ),

    template_body: $ => repeat1($._template_part),

    _directive_start: _ => choice('%{', '%{~'),
    _directive_end: _ => choice('}', '~}'),

    _directive_else: _ => directiveCloser('else'),
    _directive_endif: _ => directiveCloser('endif'),
    _directive_endfor: _ => directiveCloser('endfor'),

    // Only a quoted template processes these; a heredoc's backslash is
    // ordinary text, which is why the scanner stops a quoted chunk at `\`
    // and a heredoc chunk does not.
    escape_sequence: _ => token(seq(
      '\\',
      choice(
        /[nrt"\\]/,
        /u[0-9a-fA-F]{4}/,
        /U[0-9a-fA-F]{8}/,
      ),
    )),

    // A block label: a quoted string that may NOT interpolate, so unlike
    // `quoted_template` every instance is determined by its own text and
    // the rule is a `_literal`. One token rather than a rule because the
    // scanner's mode stack must not open on it.
    string_lit: _ => token(seq(
      '"',
      repeat(choice(
        /[^"\\\r\n]/,
        /\\[nrt"\\]/,
        /\\u[0-9a-fA-F]{4}/,
        /\\U[0-9a-fA-F]{8}/,
      )),
      '"',
    )),

    // ---------------------------------------------------------------
    // Trivia
    // ---------------------------------------------------------------

    // `#` and `//` are one node because they are one construct: HCL's own
    // formatter rewrites `//` to `#` and treats the two identically. The
    // inline form is separate because it is not equivalent to a newline
    // and the line form is.
    comment: _ => token(seq(choice('#', '//'), /[^\r\n]*/)),
    block_comment: _ => token(seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')),
  },
});
