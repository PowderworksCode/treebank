/**
 * treebank-zig: a from-scratch grammar for Zig 0.11–0.15, carrying the
 * treebank vocabulary (DESIGN.md §3) in its parse table.
 *
 * Zig is the first language here whose TYPES ARE VALUES, and that decides
 * where the vocabulary lands. `u32` is an identifier, `[]const u8` is an
 * expression, `struct { … }` is an expression, and `const Foo = struct {}`
 * is a variable declaration and nothing more. So `_type` cannot be a
 * sibling of `_expression` the way it is in rust — every type occurrence
 * would then be reachable as both at one position, which generate rejects
 * outright (DESIGN.md §2, fact 3). It is a NESTED partition instead:
 * `_expression → _type → pointer_type | slice_type | …`, covering the
 * syntax that CONSTRUCTS a type. `(_type)` therefore answers "where is
 * type syntax written", which is the question the term exists for, and it
 * answers it without claiming Zig has a type grammar it does not have.
 *
 * The second Zig-shaped decision is `_type_operand`. `T{…}` is an
 * initializer, so a type position that admits a full `_expression` eats the
 * initializer that belongs to its own parent: `[_]u8{1, 2}` becomes an
 * array of `u8{1, 2}`, and `fn f() void {}` becomes a function with a
 * `void{}` return type and no body. Zig's own grammar already says what the
 * fix is — a type position holds a `TypeExpr`, which stops below the binary
 * operators and below the initializer — so every type position here takes
 * `_type_operand` and the precedences on it are what enforce the stop.
 *
 * Omissions and the reasons for them are in ledger.toml's roles_note.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank-core/vocabulary/supertypes.js');

// Zig's own precedence table (language reference, "Precedence"), lowest
// binding first. `x{…}` sits BELOW the prefix operators there, and `a!b`
// above them; both are copied rather than rationalised.
const PREC = {
  assign: 1,
  orelse: 2,
  range: 2,
  or: 3,
  and: 4,
  compare: 5,
  bitor: 6,
  bitxor: 7,
  bitand: 8,
  shift: 9,
  add: 10,
  mul: 11,
  init: 12,
  prefix: 13,
  error_union: 14,
  suffix: 15,
};

module.exports = grammar({
  name: 'zig',

  word: $ => $.identifier,

  extras: $ => [
    /\s/,
    $.line_comment,
    $.doc_comment,
    $.container_doc_comment,
  ],

  // None. Zig's tokenizer is a context-free DFA over a fixed token set:
  // no indentation, no raw-string delimiter to carry, no regex-vs-division
  // decision, no template nesting. The one construct that looks like it
  // needs state — the multiline string — is a per-LINE token that the
  // parse table repeats, so a regular token expresses it exactly.
  externals: _ => [],

  supertypes: $ => tb.assertTableTerms([
    '_statement',
    '_expression',
    '_declaration',
    '_type',
    '_name',
    '_literal',
    // `_parameter` is demoted to the facet tier here; see roles.json. Zig
    // puts the C-variadic `...` last and nowhere else, and one alternation
    // repeated by commas cannot say that.
    ...tb.assertDemotable([]),
    '_argument',
    '_member',
    '_directive',
    '_body',
    '_control_flow',
    '_branch',
    '_loop',
    '_jump',
    '_assignment',
    '_invocation',
    '_access',
    // `_modifier` is demoted to the facet tier here, for rust's reason:
    // Zig orders its modifiers (`pub` before `export`/`extern` before
    // `threadlocal`) and one alternation across all of them accepts
    // `threadlocal pub extern const x`. See roles.json.
    ...tb.assertDemotable([]),
  ]).map((name) => $[name]),

  conflicts: $ => [
    // `inline` before a loop belongs to the loop; `inline` before a switch
    // prong belongs to the prong. In `switch (x) { inline while …` both
    // readings are live, because a prong's value is an expression and a
    // `while` is one. The prong reading is the one that survives, and it
    // survives by completing.
    [$.while_expression],
    [$.for_expression],
  ],

  rules: {
    // A Zig file IS a struct body: the same members, in the same order
    // rules, which is why `@This()` at the top level is a type.
    //
    // A `.zon` file is the other thing a file can be — one anonymous
    // initializer and nothing else, which is what every `build.zig.zon`
    // is. It is admitted here rather than given a second grammar because
    // it is the same tokenizer and the same expression rules; restricting
    // it to the `.{ … }` form keeps it unambiguous against a container
    // body, since no member may begin with a `.`.
    source_file: $ => choice(
      containerMembers($),
      field('value', $.anonymous_initializer_expression),
    ),

    // ── members ──────────────────────────────────────────────────────
    _member: $ => choice(
      $._declaration,
      $._directive,
      $.container_field,
      $.test_declaration,
      // Container-level `comptime { … }`. It is the same NODE as the
      // statement-level one — same text, same meaning — but the rule takes
      // a block and nothing else. Zig's own grammar makes `comptime Expr`
      // a primary, so a member that could be any comptime expression is
      // ambiguous with a comptime FIELD from `comptime` all the way to the
      // `:`, and every suffix (`!`, `(`, `.`) reopened it.
      alias($._comptime_block, $.comptime_expression),
    ),

    // The block here is UNLABELLED. `comptime` may also be a container
    // field's modifier, so `comptime x` is a field until proven otherwise,
    // and a labelled block would make the two indistinguishable until the
    // token after the `:` — a label on a container-level comptime block
    // can name nothing anyway, since nothing there can break to it.
    _comptime_block: $ => seq('comptime', field('operand', alias($._unlabelled_block, $.block))),

    _unlabelled_block: $ => seq('{', repeat($._statement), '}'),

    // The comma is REQUIRED after a field and absent from the last one, so
    // the last field is a rule of its own rather than an `optional(',')`
    // that would also accept `struct { a: u32 b: u32 }`.
    container_field: $ => seq($._container_field_body, ','),

    _container_field_body: $ => seq(
      optional(field('modifier', $.parameter_modifier)),
      choice(
        seq(
          field('name', $._name),
          ':',
          field('type', $._type_operand),
          optional($.align_qualifier),
          optional(seq('=', field('value', $._expression))),
        ),
        // The enum-member form. A bare identifier is read HERE and not as
        // a tuple field, and that falls out of the narrowing below rather
        // than needing a declared conflict: the tuple form admits type
        // syntax, `a.b` and `f(x)`, and none of those is a lone name.
        seq(
          field('name', $._name),
          optional(seq('=', field('value', $._expression))),
        ),
        // The tuple form: `struct { []const u8, std.ArrayList(u8) }` —
        // fields that are types with no name at all. Deliberately NOT the
        // whole expression category: a bare identifier there is the
        // enum-member reading above, and admitting every expression also
        // made `comptime { … }` at container level indistinguishable from
        // a comptime field whose type is a block.
        field('type', choice($._type, $.field_expression, $.call_expression)),
      ),
    ),

    // ── declarations ─────────────────────────────────────────────────
    _declaration: $ => choice(
      $.function_declaration,
      $.variable_declaration,
    ),

    // The name is REQUIRED, and that is what keeps `fn` unambiguous: with
    // a name it is a declaration, with `(` straight after `fn` it is the
    // function TYPE `fn (u32) void`, and one token of lookahead separates
    // them. An `optional(name)` here costs a GLR conflict for nothing.
    function_declaration: $ => seq(
      optional(field('modifier', $.visibility_modifier)),
      optional(field('modifier', choice($.linkage_modifier, $.inline_modifier))),
      'fn',
      field('name', $._name),
      field('parameters', $.parameters),
      repeat(field('modifier', $._fn_qualifier)),
      field('return_type', choice(
        $._type_operand,
        alias($._inferred_error_union, $.error_union_type),
      )),
      choice(field('body', $._body), ';'),
    ),

    // `fn f() !void` — the error set is inferred, so there is no left
    // operand. Aliased to `error_union_type` so `(_type)` sees it: the
    // node cannot be reachable from `_expression` under its own name,
    // because `!x` there is boolean negation and the two would collide.
    _inferred_error_union: $ => seq('!', field('payload', $._type_operand)),

    variable_declaration: $ => seq(
      optional(field('modifier', $.visibility_modifier)),
      optional(field('modifier', $.linkage_modifier)),
      optional(field('modifier', $.threadlocal_modifier)),
      field('kind', choice('const', 'var')),
      field('name', $._name),
      optional(seq(':', field('type', $._type_operand))),
      optional(field('modifier', $.align_qualifier)),
      optional(field('modifier', $.addrspace_qualifier)),
      optional(field('modifier', $.link_section)),
      optional(seq('=', field('value', $._expression))),
      ';',
    ),

    test_declaration: $ => seq(
      'test',
      optional(field('name', choice($.string, $._name))),
      field('body', $._body),
    ),

    // ── directives ───────────────────────────────────────────────────
    // Zig has no import statement — `@import` is an ordinary builtin call
    // whose result is a value — so `usingnamespace` is the language's only
    // construct that acts on the compilation unit rather than in it.
    _directive: $ => $.usingnamespace_declaration,

    usingnamespace_declaration: $ => seq(
      optional(field('modifier', $.visibility_modifier)),
      'usingnamespace',
      field('namespace', $._expression),
      ';',
    ),

    // ── modifiers ────────────────────────────────────────────────────
    visibility_modifier: _ => 'pub',
    linkage_modifier: $ => choice('export', seq('extern', optional($.string))),
    inline_modifier: _ => choice('inline', 'noinline'),
    threadlocal_modifier: _ => 'threadlocal',
    parameter_modifier: _ => choice('comptime', 'noalias'),
    pointer_qualifier: _ => choice('const', 'volatile', 'allowzero'),

    _fn_qualifier: $ => choice(
      $.align_qualifier,
      $.addrspace_qualifier,
      $.link_section,
      $.calling_convention,
    ),

    align_qualifier: $ => seq('align', '(', $._expression, optional(seq(':', $._expression, ':', $._expression)), ')'),
    addrspace_qualifier: $ => seq('addrspace', '(', $._expression, ')'),
    link_section: $ => seq('linksection', '(', $._expression, ')'),
    calling_convention: $ => seq('callconv', '(', $._expression, ')'),

    // ── parameters ───────────────────────────────────────────────────
    // `...` is only ever last, which is why `_parameter` is a facet here
    // and not a supertype: one alternation repeated by commas would accept
    // `fn (…, a: u32)`, and extern C declarations are exactly where that
    // would go unnoticed.
    parameters: $ => seq('(', optional($._parameter_list), ')'),

    _parameter_list: $ => choice(
      seq($.variadic_parameter, optional(',')),
      seq($.parameter, optional(seq(',', optional($._parameter_list)))),
    ),

    parameter: $ => seq(
      optional(field('modifier', $.parameter_modifier)),
      optional(seq(field('name', $._name), ':')),
      // `anytype` reaches here through `_type`, not as an extra
      // alternative: listed twice it is two ways to parse one token.
      field('type', $._type_operand),
    ),

    variadic_parameter: _ => '...',

    inferred_type: _ => 'anytype',

    // ── statements ───────────────────────────────────────────────────
    _statement: $ => choice(
      $._declaration,
      $._directive,
      $.expression_statement,
      $.defer_statement,
      $.errdefer_statement,
      $.suspend_statement,
    ),

    expression_statement: $ => choice(
      seq(choice($._expression, $._assignment), ';'),
      // A statement that ENDS in `}` takes no semicolon, and this has to
      // outrank every binary operator or the next line's leading `*`, `-`
      // or `&` joins the two statements into one silently — rust learned
      // that one the expensive way and the note is in its grammar.
      // Inlined rather than routed through a named rule on purpose: the
      // precedence has to sit on the REDUCTION that ends the statement,
      // and a one-symbol `_block_statement -> _body` production in between
      // carries none of it.
      prec(PREC.suffix + 1, choice(
        $._body,
        $.if_expression,
        $.while_expression,
        $.for_expression,
        $.switch_expression,
        $.comptime_expression,
        $.nosuspend_expression,
      )),
    ),

    defer_statement: $ => seq('defer', field('body', $._statement)),
    errdefer_statement: $ => seq('errdefer', optional(field('capture', $.payload)), field('body', $._statement)),
    suspend_statement: $ => seq('suspend', choice(field('body', $._body), ';')),

    _assignment: $ => choice($.assignment_expression, $.augmented_assignment_expression),

    assignment_expression: $ => prec.left(PREC.assign, seq(
      field('left', $._expression),
      '=',
      field('right', $._expression),
    )),

    augmented_assignment_expression: $ => prec.left(PREC.assign, seq(
      field('left', $._expression),
      field('operator', choice(
        '+=', '-=', '*=', '/=', '%=', '<<=', '>>=', '&=', '^=', '|=',
        '+%=', '-%=', '*%=', '+|=', '-|=', '*|=', '<<|=',
      )),
      field('right', $._expression),
    )),

    // ── bodies and blocks ────────────────────────────────────────────
    _body: $ => choice($.block),

    block: $ => seq(
      optional(field('label', $.block_label)),
      '{',
      repeat($._statement),
      '}',
    ),

    // Below everything, so that a `:` following a name is read as the
    // label ONLY where nothing else wants it. `[N:0]u8` is the case that
    // forces it: inside the brackets the size is an expression and the `:`
    // introduces the sentinel, and a label would swallow both.
    block_label: $ => prec(-1, seq($._name, ':')),

    // ── expressions ──────────────────────────────────────────────────
    _expression: $ => choice(
      $.initializer_expression,
      // The primary-and-suffix half is reached THROUGH `_suffix_expression`
      // rather than repeated here. Listing `_invocation` in both made `f()`
      // reachable two ways at once, which is an ambiguity at every `(` in
      // the language rather than a duplication.
      $._suffix_expression,
      $._type,
      $._control_flow,
      $._body,
      $.binary_expression,
      $.unary_expression,
      $.address_of_expression,
      $.try_expression,
      $.await_expression,
      $.async_expression,
      $.resume_expression,
      $.nosuspend_expression,
      $.comptime_expression,
      $.catch_expression,
      $.orelse_expression,
      $.range_expression,
    ),

    // Zig's `PrimaryTypeExpr` with its suffixes: what may sit to the LEFT
    // of a `(`, a `.`, a `[` or a `.*`. Narrower than the expression
    // category on purpose — `f()`, `a.b` and `a[i]` bind to a primary, not
    // to a prefix expression, so `comptime { … }` is a comptime block and
    // never the callee of the `(` on the next line.
    _suffix_expression: $ => choice(
      $._name,
      $._literal,
      $._invocation,
      $._access,
      $.unwrap_expression,
      $.deref_expression,
      $.grouped_expression,
      $.enum_literal,
      $.error_value,
      $.anonymous_initializer_expression,
      // `initializer_expression` is deliberately NOT here. With it, an
      // initializer is reachable from inside every type operand (through
      // `a.b` and `f()`), and `*[3]u8{ … }` becomes a choice between a
      // pointer to an initialised array and an initialised pointer type —
      // at every `{` in the language. `(Foo{}).bar` needs its parentheses
      // here; that is the price, and it is recorded in the ledger.
      $.function_type,
      $.asm_expression,
    ),

    // What may carry an initializer list: Zig's `CurlySuffixExpr <-
    // TypeExpr InitList?`. The two halves are kept DISJOINT — the
    // container and error-set declarations live in `_type` and nowhere
    // else — because a node reachable through both would be an ambiguity
    // at every `{` in the language.
    // Above the bare expression reduction and below `_type_operand`, which
    // is the whole ordering the `{` ambiguity needs: an initializer beats
    // "the expression ended here", and a prefix type op's operand beats
    // the initializer.
    _init_type: $ => choice(
      prec(PREC.init, $._type),
      prec(PREC.init, $._suffix_expression),
    ),

    // Zig's `TypeExpr`, and what EVERY type position here takes: a
    // function's return type, a variable's annotation, a parameter's type,
    // a field's type, and the operand of a prefix type op.
    //
    // The same content as `_init_type`, and the precedence is the whole
    // point of the second copy. In a type position the `{` that follows
    // belongs to the enclosing construct and never to the type, so the
    // reduction that ENDS the type has to beat the shift that would start
    // an initializer: above `PREC.init`, which is what makes `[_]u8{ 1, 2 }`
    // an initialised `[_]u8` rather than an array of `u8{ 1, 2 }`, and
    // `fn f() []const u8 {` a function with a body rather than one whose
    // return type ate it.
    //
    // Below `PREC.suffix`, so a `.` or a `(` still binds INTO the type:
    // `fn f() !std.ArrayList(u8)` returns the list, not `std`.
    _type_operand: $ => choice(
      prec(PREC.prefix, $._suffix_expression),
      prec(PREC.prefix, $._type),
    ),

    // ── types ────────────────────────────────────────────────────────
    // Nested inside `_expression`, because in Zig a type IS an expression.
    // The members are the syntax that CONSTRUCTS a type; `u32` is an
    // `identifier` and reaches `_name` like any other.
    _type: $ => choice(
      $.pointer_type,
      $.slice_type,
      $.array_type,
      $.optional_type,
      $.error_union_type,
      $.anyframe_type,
      $.container_declaration,
      $.error_set_declaration,
      $.inferred_type,
    ),

    pointer_type: $ => prec.right(PREC.prefix, seq(
      field('kind', choice(
        '*',
        '**',
        token(seq('[', '*', ']')),
        token(seq('[', '*', 'c', ']')),
        seq(token(seq('[', '*')), ':', field('sentinel', $._expression), ']'),
      )),
      repeat(field('modifier', $._pointer_modifier)),
      field('type', $._type_operand),
    )),

    slice_type: $ => prec.right(PREC.prefix, seq(
      '[',
      optional(seq(':', field('sentinel', $._expression))),
      ']',
      repeat(field('modifier', $._pointer_modifier)),
      field('type', $._type_operand),
    )),

    array_type: $ => prec.right(PREC.prefix, seq(
      '[',
      field('size', $._expression),
      optional(seq(':', field('sentinel', $._expression))),
      ']',
      field('type', $._type_operand),
    )),

    _pointer_modifier: $ => choice(
      $.pointer_qualifier,
      $.align_qualifier,
      $.addrspace_qualifier,
    ),

    optional_type: $ => prec.right(PREC.prefix, seq('?', field('type', $._type_operand))),

    // The left operand is an ERROR SET, not an arbitrary expression:
    // `anyerror`, `error{…}`, `Foo.Error`, `Allocator.Error`. Zig's own
    // grammar says the same (`ErrorUnionExpr <- SuffixExpr (! TypeExpr)?`)
    // and the narrowing is what keeps `!` apart from prefix `!` — with a
    // full expression on the left, `comptime x` and `!x` both reached this
    // rule and the container body could not be parsed at all.
    error_union_type: $ => prec.left(PREC.error_union, seq(
      field('error_set', choice(
        $._name,
        $.field_expression,
        $.call_expression,
        $.error_set_declaration,
      )),
      '!',
      field('payload', $._type_operand),
    )),

    anyframe_type: $ => prec.right(seq(
      'anyframe',
      optional(seq('->', field('type', $._type_operand))),
    )),

    container_declaration: $ => seq(
      optional(field('layout', $.container_layout)),
      field('kind', choice('struct', 'enum', 'union', 'opaque')),
      // `union(enum)` and `union(enum(u8))`: the tag type is INFERRED from
      // the union's own fields, so the `enum` there declares nothing and
      // is not a container declaration — it is a word in a slot.
      optional(seq('(', optional(field('tag', choice($._expression, $.inferred_enum_tag))), ')')),
      field('body', $.container_body),
    ),

    container_layout: _ => choice('extern', 'packed'),

    inferred_enum_tag: $ => seq('enum', optional(seq('(', $._expression, ')'))),

    container_body: $ => seq('{', containerMembers($), '}'),

    error_set_declaration: $ => seq(
      'error',
      '{',
      optional(seq(commaSep1(field('name', $._name)), optional(','))),
      '}',
    ),

    // ── control flow ─────────────────────────────────────────────────
    // Nested inside `_expression` rather than `_statement`: Zig's `if`,
    // `while`, `for` and `switch` all produce values, and the statement
    // forms are the same nodes in a position that discards the value.
    _control_flow: $ => choice($._branch, $._loop, $._jump),

    _branch: $ => choice($.if_expression, $.switch_expression),

    if_expression: $ => prec.right(seq(
      'if',
      '(', field('condition', $._expression), ')',
      optional(field('capture', $.payload)),
      field('consequence', $._expression),
      optional(field('alternative', $.else_clause)),
    )),

    else_clause: $ => seq(
      'else',
      optional(field('capture', $.payload)),
      field('body', $._expression),
    ),

    switch_expression: $ => seq(
      'switch',
      '(', field('value', $._expression), ')',
      '{',
      optional(seq(commaSep1($.switch_case), optional(','))),
      '}',
    ),

    switch_case: $ => seq(
      optional($.inline_modifier),
      choice(
        seq(commaSep1(field('value', $._switch_item)), optional(',')),
        'else',
      ),
      '=>',
      optional(field('capture', $.payload)),
      field('body', choice($._expression, $._assignment)),
    ),

    _switch_item: $ => choice($._expression, $.switch_range),

    switch_range: $ => seq(field('start', $._expression), '...', field('end', $._expression)),

    _loop: $ => choice($.while_expression, $.for_expression),

    while_expression: $ => prec.right(seq(
      optional(field('label', $.block_label)),
      optional(field('modifier', $.inline_modifier)),
      'while',
      '(', field('condition', $._expression), ')',
      optional(field('capture', $.payload)),
      optional(seq(':', '(', field('continuation', choice($._expression, $._assignment)), ')')),
      field('body', $._expression),
      optional(field('alternative', $.else_clause)),
    )),

    for_expression: $ => prec.right(seq(
      optional(field('label', $.block_label)),
      optional(field('modifier', $.inline_modifier)),
      'for',
      '(', commaSep1(field('subject', $._expression)), optional(','), ')',
      field('capture', $.payload),
      field('body', $._expression),
      optional(field('alternative', $.else_clause)),
    )),

    // `|x|`, `|x, i|`, `|*item|` — the capture list every Zig control
    // construct can carry.
    payload: $ => seq(
      '|',
      optional('*'),
      field('name', $._name),
      optional(seq(',', field('index', $._name))),
      '|',
    ),

    _jump: $ => choice(
      $.return_expression,
      $.break_expression,
      $.continue_expression,
      $.unreachable_expression,
    ),

    return_expression: $ => prec.right(seq('return', optional(field('value', $._expression)))),

    break_expression: $ => prec.right(seq(
      'break',
      optional(field('label', $.loop_label)),
      optional(field('value', $._expression)),
    )),

    continue_expression: $ => prec.right(seq('continue', optional(field('label', $.loop_label)))),

    loop_label: $ => seq(':', $._name),

    // `unreachable` is a `_jump` and not a literal: it transfers control
    // out of the function (to a panic in a safe build, to nothing at all
    // in a released one) and never yields a value. It is Zig's `raise`.
    unreachable_expression: _ => 'unreachable',

    // ── invocation and access ────────────────────────────────────────
    _invocation: $ => choice($.call_expression, $.builtin_call),

    call_expression: $ => prec(PREC.suffix, seq(
      field('function', $._suffix_expression),
      field('arguments', $.arguments),
    )),

    // `@import`, `@This`, `@intCast`. A builtin is not a callable value in
    // Zig — it cannot be stored or passed — but the call syntax is a call
    // and `(_invocation)` should see it.
    builtin_call: $ => seq(
      field('function', $.builtin_identifier),
      field('arguments', $.arguments),
    ),

    arguments: $ => seq(
      '(',
      optional(seq(commaSep1($._argument), optional(','))),
      ')',
    ),

    _argument: $ => choice($._expression),

    _access: $ => choice($.field_expression, $.subscript_expression),

    field_expression: $ => prec(PREC.suffix, seq(
      field('object', $._suffix_expression),
      '.',
      field('field', $._name),
    )),

    subscript_expression: $ => prec(PREC.suffix, seq(
      field('object', $._suffix_expression),
      '[',
      field('index', $._expression),
      optional(seq(':', field('sentinel', $._expression))),
      ']',
    )),

    unwrap_expression: $ => prec(PREC.suffix, seq(field('value', $._suffix_expression), '.', '?')),
    deref_expression: $ => prec(PREC.suffix, seq(field('value', $._suffix_expression), '.', '*')),

    // ── operators ────────────────────────────────────────────────────
    binary_expression: $ => {
      const table = [
        [PREC.or, 'or'],
        [PREC.and, 'and'],
        [PREC.compare, choice('==', '!=', '<', '>', '<=', '>=')],
        [PREC.bitor, choice('|', '||')],
        [PREC.bitxor, '^'],
        [PREC.bitand, '&'],
        [PREC.shift, choice('<<', '>>', '<<|')],
        [PREC.add, choice('+', '-', '++', '+%', '-%', '+|', '-|')],
        [PREC.mul, choice('*', '/', '%', '**', '*%', '*|')],
      ];
      return choice(...table.map(([precedence, operator]) => prec.left(
        /** @type {number} */ (precedence),
        seq(
          field('left', $._expression),
          field('operator', /** @type {any} */ (operator)),
          field('right', $._expression),
        ),
      )));
    },

    unary_expression: $ => prec(PREC.prefix, seq(
      field('operator', choice('-', '-%', '~', '!')),
      field('operand', $._expression),
    )),

    address_of_expression: $ => prec(PREC.prefix, seq('&', field('operand', $._expression))),

    try_expression: $ => prec.right(PREC.prefix, seq('try', field('operand', $._expression))),
    await_expression: $ => prec.right(PREC.prefix, seq('await', field('operand', $._expression))),
    async_expression: $ => prec.right(PREC.prefix, seq('async', field('operand', $._expression))),
    resume_expression: $ => prec.right(PREC.prefix, seq('resume', field('operand', $._expression))),
    nosuspend_expression: $ => prec.right(PREC.prefix, seq('nosuspend', field('operand', $._expression))),
    comptime_expression: $ => prec.right(PREC.prefix, seq('comptime', field('operand', $._expression))),

    catch_expression: $ => prec.left(PREC.orelse, seq(
      field('value', $._expression),
      'catch',
      optional(field('capture', $.payload)),
      field('fallback', $._expression),
    )),

    orelse_expression: $ => prec.left(PREC.orelse, seq(
      field('value', $._expression),
      'orelse',
      field('fallback', $._expression),
    )),

    // `0..10` in a for, `a[i..j]` in a slice. One rule for both, as rust
    // does: the two positions differ in nothing a query would ask about.
    range_expression: $ => prec.left(PREC.range, seq(
      optional(field('start', $._expression)),
      '..',
      optional(field('end', $._expression)),
    )),

    grouped_expression: $ => seq('(', $._expression, ')'),

    // ── initializers ─────────────────────────────────────────────────
    initializer_expression: $ => prec(PREC.init, seq(
      field('type', $._init_type),
      field('value', $.initializer_list),
    )),

    // `.{ .a = 1 }` and `.{ 1, 2 }` — the type is inferred from context.
    anonymous_initializer_expression: $ => seq('.', field('value', $.initializer_list)),

    initializer_list: $ => seq(
      '{',
      optional(choice(
        seq(commaSep1($.field_initializer), optional(',')),
        seq(commaSep1($._expression), optional(',')),
      )),
      '}',
    ),

    field_initializer: $ => seq('.', field('name', $._name), '=', field('value', $._expression)),

    // `.ok`, `.not_found` — an enum literal whose type comes from context.
    enum_literal: $ => seq('.', field('name', $._name)),

    error_value: $ => seq('error', '.', field('name', $._name)),

    // ── inline assembly ──────────────────────────────────────────────
    asm_expression: $ => seq(
      'asm',
      optional(field('modifier', $.asm_volatile)),
      '(',
      field('template', $._expression),
      repeat(seq(':', optional(seq(commaSep1(choice($.asm_operand, $.string)), optional(','))))),
      ')',
    ),

    asm_volatile: _ => 'volatile',

    asm_operand: $ => seq(
      '[', field('name', $._name), ']',
      field('constraint', $.string),
      '(',
      choice(seq('->', field('type', $._expression)), field('value', $._expression)),
      ')',
    ),

    // ── functions as types ───────────────────────────────────────────
    function_type: $ => prec.right(seq(
      'fn',
      field('parameters', $.parameters),
      repeat(field('modifier', $._fn_qualifier)),
      field('return_type', choice(
        $._type_operand,
        alias($._inferred_error_union, $.error_union_type),
      )),
    )),

    // ── names ────────────────────────────────────────────────────────
    _name: $ => choice($.identifier),

    // `@"any string at all"` is an identifier too, which is how Zig names
    // things its keyword list has taken.
    identifier: _ => token(choice(
      /[A-Za-z_][A-Za-z0-9_]*/,
      seq('@"', /[^"\n]*/, '"'),
    )),

    // `@` followed by a LETTER, which is what keeps it apart from the
    // quoted identifier above: one token cannot be both.
    builtin_identifier: _ => token(seq('@', /[A-Za-z_][A-Za-z0-9_]*/)),

    // ── literals ─────────────────────────────────────────────────────
    // Every one of these is fully determined by its own text: Zig strings
    // do not interpolate, so `_literal` admits them without the
    // per-instance caveat python's `string` rule forces.
    _literal: $ => choice(
      $.integer,
      $.float,
      $.string,
      $.multiline_string,
      $.character,
      $.boolean,
      $.null,
      $.undefined,
    ),

    integer: _ => token(choice(
      /0[xX][0-9a-fA-F][0-9a-fA-F_]*/,
      /0[oO][0-7][0-7_]*/,
      /0[bB][01][01_]*/,
      /[0-9][0-9_]*/,
    )),

    float: _ => token(choice(
      /[0-9][0-9_]*\.[0-9][0-9_]*([eE][-+]?[0-9_]+)?/,
      /[0-9][0-9_]*[eE][-+]?[0-9_]+/,
      /0[xX][0-9a-fA-F][0-9a-fA-F_]*\.[0-9a-fA-F][0-9a-fA-F_]*([pP][-+]?[0-9_]+)?/,
      /0[xX][0-9a-fA-F][0-9a-fA-F_]*[pP][-+]?[0-9_]+/,
    )),

    string: $ => seq(
      '"',
      repeat(choice(
        $.escape_sequence,
        token.immediate(prec(1, /[^"\\\n]+/)),
      )),
      '"',
    ),

    // A multiline string is a run of LINES, each `\\` to the end of it.
    // That is why this grammar needs no external scanner: the construct
    // that looks like it carries state is a regular token repeated.
    multiline_string: _ => prec.right(repeat1(token(seq('\\\\', /[^\n]*/)))),

    character: _ => token(seq(
      "'",
      choice(
        /[^'\\\n]/,
        seq('\\', /u\{[0-9a-fA-F]+\}/),
        seq('\\', /x[0-9a-fA-F][0-9a-fA-F]/),
        seq('\\', /[^\n]/),
      ),
      "'",
    )),

    escape_sequence: _ => token.immediate(seq(
      '\\',
      choice(
        /u\{[0-9a-fA-F]+\}/,
        /x[0-9a-fA-F][0-9a-fA-F]/,
        /[^\n]/,
      ),
    )),

    boolean: _ => choice('true', 'false'),
    null: _ => 'null',
    undefined: _ => 'undefined',

    // ── comments ─────────────────────────────────────────────────────
    // Three kinds, and the vocabulary's `_comment` facet is exactly why
    // that is worth spelling out: a consumer that hand-maintains "what is
    // a comment here" gets rust's two wrong and Zig's three wrong.
    // `////` is an ordinary comment in Zig, not a doc comment, which is
    // what the negated first character in `doc_comment` is for.
    // Spelled out as regexes rather than separated by token precedence,
    // because tree-sitter's lexer weighs PRECEDENCE BEFORE LENGTH: a
    // `line_comment` at prec(-1) loses `//// four slashes` to a
    // `doc_comment` that matched three characters of it, and the rest of
    // the line becomes an ERROR. Each rule states its own shape instead,
    // and the longest match then decides.
    line_comment: _ => token(choice(
      /\/\/([^!\/\n][^\n]*)?/,
      /\/\/\/\/[^\n]*/,
    )),
    doc_comment: _ => token(/\/\/\/([^\/\n][^\n]*)?/),
    container_doc_comment: _ => token(seq('//!', /[^\n]*/)),
  },
});

/**
 * A container body: members, then optionally one last field with no comma
 * after it. Shared by `source_file` and `container_body` because a Zig
 * file is a struct body.
 *
 * @param {any} $
 */
function containerMembers($) {
  return seq(
    repeat($._member),
    optional(alias($._container_field_body, $.container_field)),
  );
}

/**
 * @param {any} rule
 */
function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}
