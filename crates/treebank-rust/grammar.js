/**
 * treebank-rust: a from-scratch grammar for Rust across editions 2015–2024,
 * carrying the treebank vocabulary (DESIGN.md §3) in its parse table.
 *
 * Rust is expression-oriented, which changes where the vocabulary threads:
 * `_control_flow` (and `_branch`/`_loop`/`_jump`) nest inside `_expression`
 * rather than `_statement`, `_member` is threadable (impl and trait bodies
 * hold declarations), and `_modifier` gets its first members
 * (visibility_modifier, mutable_specifier). 21 of 22 table-tier terms
 * thread; only `_clause` cannot (see ledger roles_note).
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank-core/vocabulary/supertypes.js');

const PREC = {
  closure: -2,
  assign: 1,
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
  cast: 12,
  unary: 13,
  try: 14,
  call: 15,
  field: 16,
};

module.exports = grammar({
  name: 'rust',

  word: $ => $.identifier,

  extras: $ => [
    /\s/,
    $.line_comment,
    $.block_comment,
  ],

  externals: $ => [
    $.float,
    $.raw_string,
    $.block_comment,
  ],

  supertypes: $ => tb.assertTableTerms([
    '_statement',
    '_expression',
    '_declaration',
    '_pattern',
    '_type',
    '_name',
    '_literal',
    // `_parameter` is demoted to the facet tier here; see roles.json.
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
    '_attribute',
    // `_modifier` is demoted to the facet tier here: rust never lets
    // visibility and `mut` occupy the same slot, so one alternation across
    // both is exactly the rule that accepted `mut use x;`. See roles.json.
    ...tb.assertDemotable([]),
  ]).map((name) => $[name]),

  conflicts: $ => [
    [$._brace_token_tree, $.token_tree],
    [$._control_flow, $._callee],
    [$._no_struct_expression, $._callee],
    [$._expression_with_block, $._callee],
    [$._callee, $.tuple_struct_pattern],
    // Whether a macro statement owns its semicolon. Both readings are
    // complete; the negative dynamic precedence on the no-semicolon
    // alternative of `expression_statement` is what decides it.
    [$._invocation, $.expression_statement],
    [$.scoped_identifier, $.generic_function, $._path_ref],
    [$.scoped_identifier, $._path_ref],
    [$._soft_keyword, $.box_pattern],
    [$.generic_type, $.generic_function],
    [$.generic_type, $.generic_function, $._path_ref],
    [$.generic_type, $._name],
    [$._type, $.generic_type],
    [$.rest_pattern, $.range_pattern],
    [$.rest_pattern, $.range_expression],
    [$.rest_pattern, $.range_pattern, $.range_expression],
    [$.bounded_type],
    [$.type_binding, $.generic_type],
    [$.visibility_modifier],
    [$.tuple_type, $.parenthesized_expression, $.tuple_pattern],
    [$.trait_bounds, $.bounded_type],
    [$._type, $._invocation, $._pattern],
    [$.tuple_type, $.parenthesized_expression],
    [$._type, $._invocation],
    [$.function_type, $._name, $.tuple_struct_pattern],
    [$.shorthand_field_initializer, $.field_pattern],
    [$.field_initializer_list, $.struct_pattern],
    [$.closure_parameter, $.or_pattern],
    [$.range_pattern],
    [$.array_expression, $.slice_pattern],
    [$.parenthesized_expression, $.tuple_pattern],
    [$.closure_parameters],
    [$._no_struct_expression, $.tuple_struct_pattern],
    [$._no_struct_expression, $._range_pattern_end],
    [$._no_struct_expression, $._pattern],
    [$._access, $._range_pattern_end],
    [$._invocation, $._pattern],
    [$._name, $.tuple_struct_pattern],
    [$.function_definition, $.const_definition],
    [$.fn_type_parameters, $.tuple_struct_pattern],
    [$._type, $.function_type],
    [$._type, $._name],
    [$.generic_function, $._path_ref],
    [$.scoped_type_identifier, $.scoped_identifier],
    [$.tuple_type, $.tuple_pattern],
    [$._type, $._pattern],
    [$.capture_pattern],
    [$.higher_ranked_bound, $.higher_ranked_prefix],
    [$.trait_bounds],
    [$.tuple_pattern, $.parenthesized_pattern],
    [$._no_struct_expression, $.struct_expression],
    [$.struct_expression, $._name],
    [$.impl_block, $.never_type],
    [$._no_struct_expression, $._expression_with_block],
    [$._expression_with_block, $._invocation],
    [$._expression_with_block, $._control_flow],
    [$.range_expression],
    [$.function_definition, $.extern_block],
  ],

  rules: {
    // ITEMS, not statements. `repeat($._statement)` accepted `let x = 1;`,
    // `x + 1;` and `if true {}` at the top level of a file, none of which is
    // valid Rust -- and it is also what made a stray `;` an item (`use x;;`)
    // and let a paren-delimited macro stand as an item without its semicolon.
    // Three catalogued widenings, one cause.
    source_file: $ => repeat($._item),

    // What may appear where items are expected: a file's top level, a
    // module body, an `extern` block. A macro invocation is one too, but
    // only brace-delimited without a semicolon -- `cfg_if! { ... }` needs
    // none and `criterion_main!(benches)` does.
    _item: $ => choice(
      $._declaration,
      $._directive,
      $.impl_block,
      $.extern_block,
      $.inner_attribute_item,
      alias($._macro_item, $.expression_statement),
    ),

    _macro_item: $ => seq(
      repeat($._attribute),
      choice(
        // Brace-delimited needs no semicolon: `cfg_if! { ... }`.
        alias($._brace_macro_invocation, $.macro_invocation),
        // Paren- and bracket-delimited require one, which is why
        // `criterion_main!(benches)` alone is not an item.
        seq($.macro_invocation, ';'),
      ),
    ),

    _brace_macro_invocation: $ => seq(
      field('macro', $._path_ref),
      '!',
      alias($._brace_token_tree, $.token_tree),
    ),

    _brace_token_tree: $ => seq('{', repeat($._token), '}'),

    // ── statements ───────────────────────────────────────────────────
    _statement: $ => choice(
      $._declaration,
      $._directive,
      $.impl_block,
      $.extern_block,
      $.let_declaration,
      $.expression_statement,
      $.inner_attribute_item,
      $.empty_statement,
    ),

    empty_statement: _ => ';',

    expression_statement: $ => choice(
      seq(repeat($._attribute), $._expression, ';'),
      // Above every binary operator, because in STATEMENT position a
      // block-like expression ends the statement. rustc's rule, and the
      // reason `if c { g(); }` followed by a line starting with `*`, `-` or
      // `&` is two statements rather than one:
      //
      //     if c { *rem = 0; }
      //     *buf = Some(slice);
      //
      // At prec 1 -- below `mul` (11) -- the operator won the shift and the
      // two statements became one `binary_expression` whose left operand was
      // the `if`. No error, so no sweep could see it. In EXPRESSION position
      // nothing changes: this alternative is not reachable there, and
      // `({ 1 }) * 3` still means what it says.
      // `prec.dynamic` as well as the static precedence: this is a declared
      // GLR conflict, and a declared conflict ignores static precedence
      // entirely. The two readings differ in what they contain -- splitting
      // gives TWO expression_statements, merging gives one -- so weighting
      // this alternative is what decides it.
      prec.dynamic(1, prec(PREC.field + 1, seq(repeat($._attribute), choice(
        $._branch, $._loop, $.async_block, $.const_block, $.unsafe_block,
        $.labeled_block, $._body,
      )))),
      // A macro invocation is block-LIKE (`cfg_if! { ... }` needs no
      // semicolon) but it is not a block: `println!("x");` is ONE statement
      // and the semicolon is part of it, which is what syn says too
      // (`Stmt::Macro` carries the semi). So it gets its own alternative
      // with a NEGATIVE dynamic precedence -- available when there is no
      // semicolon, and losing to the first alternative whenever there is.
      // Without the negative weight the two readings tied and the semicolon
      // was orphaned into an `empty_statement` after every macro call in the
      // language.
      prec.dynamic(-1, seq(repeat($._attribute), $.macro_invocation)),
    ),

    let_declaration: $ => seq(
      repeat($._attribute),
      'let',
      field('pattern', $._pattern),
      optional(seq(':', field('type', $._type))),
      optional(seq(
        '=',
        field('value', $._expression),
        optional(seq('else', field('alternative', $.block))),   // let-else
      )),
      ';',
    ),

    // ── declarations ─────────────────────────────────────────────────
    _declaration: $ => choice(
      $.function_definition,
      $.struct_definition,
      $.enum_definition,
      $.union_definition,
      $.trait_definition,
      $.module_definition,
      $.module_declaration,
      $.const_definition,
      $.static_definition,
      $.type_alias,
      $.macro_definition,
    ),

    visibility_modifier: $ => seq(
      'pub',
      optional(seq(
        '(',
        choice('crate', 'super', 'self', seq('in', $._path_ref)),
        ')',
      )),
    ),

    mutable_specifier: _ => 'mut',

    function_definition: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      repeat(choice('default', 'const', 'async', 'unsafe', 'safe',
        seq('extern', optional(alias($._abi, $.string))))),
      'fn',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      field('parameters', $.parameters),
      optional(seq('->', field('return_type', $._type))),
      optional($.where_clause),
      choice(field('body', $._body), ';'),
    ),

    // Rust's parameter list is ordered: a `self` receiver is only ever the
    // FIRST parameter, and the C-variadic `...` is only ever the last. One
    // `_parameter` alternation repeated by commas says neither, and accepts
    // `fn f(a: i32, self)`. So the list is spelled out as "what may still
    // follow", which is also why `_parameter` is a facet rather than a
    // supertype here -- see roles.json's `demoted` and DESIGN.md 3.4.
    parameters: $ => seq('(', optional($._parameter_list), ')'),

    _parameter_list: $ => choice(
      seq($.self_parameter, optional($._parameter_tail)),
      seq($.parameter, optional($._parameter_tail)),
      seq($.variadic_parameter, optional(',')),
    ),

    // `self` may not reappear; `...` closes the list.
    _parameter_rest: $ => choice(
      seq($.parameter, optional($._parameter_tail)),
      seq($.variadic_parameter, optional(',')),
    ),

    _parameter_tail: $ => seq(',', optional($._parameter_rest)),

    parameter: $ => seq(
      repeat($._attribute),
      choice(
        seq(field('pattern', $._pattern), ':', field('type', choice($._type, $.variadic_parameter))),
        field('type', $._type),               // fn pointers / trait signatures
      ),
    ),

    self_parameter: $ => seq(
      repeat($._attribute),
      optional(seq('&', optional($.lifetime))),
      optional($.mutable_specifier),
      'self',
      optional(seq(':', field('type', $._type))),
    ),

    variadic_parameter: _ => '...',

    struct_definition: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      'struct',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      choice(
        seq(optional($.where_clause), field('body', $.field_declaration_list)),
        seq(field('body', $.ordered_field_declaration_list), optional($.where_clause), ';'),
        seq($.where_clause, ';'),
        ';',
      ),
    ),

    field_declaration_list: $ => seq(
      '{',
      optional(seq(commaSep1($.field_declaration), optional(','))),
      '}',
    ),

    field_declaration: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      field('name', $._name),
      ':',
      field('type', $._type),
    ),

    ordered_field_declaration_list: $ => seq(
      '(',
      optional(seq(
        commaSep1(seq(repeat($._attribute), optional($.visibility_modifier), field('type', $._type))),
        optional(','),
      )),
      ')',
    ),

    enum_definition: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      'enum',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      optional($.where_clause),
      field('body', $.enum_variant_list),
    ),

    enum_variant_list: $ => seq(
      '{',
      optional(seq(commaSep1($.enum_variant), optional(','))),
      '}',
    ),

    enum_variant: $ => seq(
      repeat($._attribute),
      field('name', $._name),
      optional(field('body', choice(
        $.field_declaration_list,
        $.ordered_field_declaration_list,
      ))),
      optional(seq('=', field('value', $._expression))),
    ),

    union_definition: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      'union',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      optional($.where_clause),
      field('body', $.field_declaration_list),
    ),

    trait_definition: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      optional('unsafe'),
      optional('auto'),
      'trait',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      optional(seq(':', $.trait_bounds)),
      optional($.where_clause),
      field('body', $.declaration_list),
    ),

    // impl and trait bodies hold declarations, threaded through `_member`
    // so `(_member)` matches exactly the items that are members.
    // No bare `;`: rustc calls it `non-item in item list` in every list
    // this rule serves — extern blocks, impls, traits and modules alike.
    declaration_list: $ => seq('{', repeat($._member), '}'),
    _member: $ => choice(
      $._declaration,
      $.inner_attribute_item,
      alias($.member_macro_invocation, $.macro_invocation),
    ),

    // A macro in an item list carries its own terminator, which is why
    // `declaration_list` no longer offers a bare `;` to lend it. The
    // delimiter decides: `m! { … }` is complete, `m!( … )` and `m![ … ]`
    // need the semicolon and `m! { … };` does not take one.
    member_macro_invocation: $ => seq(
      repeat($._attribute),
      field('macro', $._path_ref),
      '!',
      choice(
        alias($._brace_token_tree, $.token_tree),
        seq(alias($._delimited_token_tree, $.token_tree), ';'),
      ),
    ),

    _delimited_token_tree: $ => choice(
      seq('(', repeat($._token), ')'),
      seq('[', repeat($._token), ']'),
    ),

    impl_block: $ => seq(
      repeat($._attribute),
      optional('unsafe'),
      'impl',
      field('type_parameters', optional($.type_parameters)),
      optional(seq(
        optional('!'),
        field('trait', $._type),
        'for',
      )),
      field('type', $._type),
      optional($.where_clause),
      // `impl X;` is `expected {}, found ;`.
      field('body', $.declaration_list),
    ),

    module_definition: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      optional('unsafe'),
      'mod',
      field('name', $._name),
      field('body', alias($.mod_block, $.block)),
    ),
    // Items, like a file's top level -- a module body is not a block.
    mod_block: $ => seq('{', repeat($._item), '}'),

    module_declaration: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      optional('unsafe'),
      'mod',
      field('name', $._name),
      ';',
    ),

    const_definition: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      'const',
      field('name', $._name),
      ':',
      field('type', $._type),
      optional(seq('=', field('value', $._expression))),
      ';',
    ),

    static_definition: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      'static',
      optional('safe'),
      optional($.mutable_specifier),
      field('name', $._name),
      ':',
      field('type', $._type),
      optional(seq('=', field('value', $._expression))),
      ';',
    ),

    type_alias: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      optional('default'),
      'type',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      optional(seq(':', $.trait_bounds)),
      optional($.where_clause),
      optional(seq('=', field('value', $._type))),
      optional($.where_clause),          // GATs: type X<'a> = T<'a> where …
      ';',
    ),

    // Same delimiter rule as a macro INVOCATION at item level: a
    // brace-delimited body needs no semicolon, a paren- or
    // bracket-delimited one requires it. `macro_rules! m ( ... );` used to
    // parse only because a stray `;` was itself an item, which is what let
    // `use x;;` through.
    macro_definition: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      'macro_rules',
      token.immediate('!'),
      field('name', $._name),
      choice(
        field('body', alias($._brace_token_tree, $.token_tree)),
        seq(field('body', $.token_tree), ';'),
      ),
    ),

    extern_block: $ => seq(
      repeat($._attribute),
      optional('unsafe'),
      'extern',
      optional(alias($._abi, $.string)),
      field('body', $.declaration_list),
    ),

    extern_crate_declaration: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      'extern',
      'crate',
      field('name', $._name),
      optional(seq('as', field('alias', $._name))),
      ';',
    ),

    // ── directives ───────────────────────────────────────────────────
    _directive: $ => choice(
      $.use_declaration,
      $.extern_crate_declaration,
    ),

    use_declaration: $ => seq(
      repeat($._attribute),
      optional($.visibility_modifier),
      'use',
      $._use_clause,
      ';',
    ),

    _use_clause: $ => choice(
      $._path_ref,
      seq('::', $._path_ref),
      $.use_as_clause,
      $.use_list,
      $.scoped_use_list,
      seq('::', $.use_list),
      $.use_wildcard,
    ),

    use_as_clause: $ => seq(field('path', $._path_ref), 'as', field('alias', $._name)),
    use_list: $ => seq('{', optional(seq(commaSep1($._use_clause), optional(','))), '}'),
    scoped_use_list: $ => seq(field('path', $._path_ref), '::', $.use_list),
    use_wildcard: $ => seq(optional(choice(seq($._path_ref, '::'), '::')), '*'),

    // ── attributes ───────────────────────────────────────────────────
    _attribute: $ => choice($.attribute_item),

    attribute_item: $ => seq('#', '[', $.attribute, ']'),
    inner_attribute_item: $ => seq('#', '!', '[', $.attribute, ']'),

    attribute: $ => seq(
      $._path_ref,
      optional(choice(
        seq('=', $._expression),
        $.token_tree,
      )),
    ),

    // ── generics ─────────────────────────────────────────────────────
    type_parameters: $ => seq(
      '<',
      commaSep1(choice(
        $.lifetime_parameter,
        $.type_parameter,
        $.const_parameter,
      )),
      optional(','),
      '>',
    ),

    lifetime_parameter: $ => seq($.lifetime, optional(seq(':', $.trait_bounds))),

    type_parameter: $ => seq(
      repeat($._attribute),
      field('name', $._name),
      optional(seq(':', $.trait_bounds)),
      optional(seq('=', field('value', $._type))),
    ),

    const_parameter: $ => seq(
      'const',
      field('name', $._name),
      ':',
      field('type', $._type),
      optional(seq('=', field('value', choice($.block, $.identifier, $._literal, $.negative_literal)))),
    ),

    trait_bounds: $ => prec.right(seq(sep1(choice(
      $._type,
      $.lifetime,
      seq('?', $._type),
      $.higher_ranked_bound,
      $.use_bound,          // impl Trait + use<'a, T> (precise capture, 1.82)
    ), '+'), optional('+'))),

    use_bound: $ => seq(
      'use',
      '<',
      optional(seq(commaSep1(choice($.lifetime, $.identifier, $.self)), optional(','))),
      '>',
    ),

    higher_ranked_bound: $ => seq('for', $.type_parameters, $._type),

    where_clause: $ => prec.right(seq(
      'where',
      optional(seq(commaSep1($.where_predicate), optional(','))),
    )),

    where_predicate: $ => seq(
      choice($._type, $.lifetime, $.higher_ranked_bound),
      ':',
      optional($.trait_bounds),
    ),

    type_arguments: $ => seq(
      '<',
      optional(commaSep1(choice(
        $._type,
        $.lifetime,
        $._literal,
        $.negative_literal,
        $.block,
        $.type_binding,
      ))),
      optional(','),
      '>',
    ),

    type_binding: $ => seq(
      field('name', $._name),
      field('type_arguments', optional($.type_arguments)),
      choice(
        seq('=', field('type', $._type)),
        seq(':', $.trait_bounds),
      ),
    ),

    // ── types ────────────────────────────────────────────────────────
    _type: $ => choice(
      alias($.identifier, $.type_identifier),
      $.bounded_type,
      $.scoped_type_identifier,
      $.generic_type,
      $.reference_type,
      $.pointer_type,
      $.tuple_type,
      $.array_type,
      $.function_type,
      $.dyn_type,
      $.impl_type,
      $.never_type,
      $.qualified_type,
      $.macro_invocation,
    ),

    scoped_type_identifier: $ => seq(
      field('path', optional(choice($._path_ref, $.generic_type, $.qualified_type))),
      '::',
      field('name', alias($.identifier, $.type_identifier)),
    ),

    generic_type: $ => seq(
      field('type', choice(alias($.identifier, $.type_identifier), $.scoped_type_identifier)),
      optional('::'),
      field('type_arguments', $.type_arguments),
    ),

    reference_type: $ => seq(
      '&',
      optional($.lifetime),
      optional($.mutable_specifier),
      field('type', $._type),
    ),

    pointer_type: $ => seq(
      '*',
      choice('const', $.mutable_specifier),
      field('type', $._type),
    ),

    tuple_type: $ => seq('(', optional(seq(commaSep1($._type), optional(','))), ')'),

    array_type: $ => seq(
      '[',
      field('element', $._type),
      optional(seq(';', field('length', $._expression))),
      ']',
    ),

    function_type: $ => prec.right(seq(
      optional($.higher_ranked_prefix),
      choice(
        // Fn / FnMut / FnOnce parenthesized sugar: Fn(i32) -> i32
        field('trait', choice(alias($.identifier, $.type_identifier), $.scoped_type_identifier)),
        seq(
          repeat(choice('unsafe', seq('extern', optional(alias($._abi, $.string))))),
          'fn',
        ),
      ),
      field('parameters', alias($.fn_type_parameters, $.parameters)),
      optional(seq('->', field('return_type', $._type))),
    )),
    higher_ranked_prefix: $ => seq('for', $.type_parameters),
    fn_type_parameters: $ => seq(
      '(',
      optional(seq(commaSep1(choice(
        seq(optional(seq(field('name', $.identifier), ':')), $._type),
        $.variadic_parameter,
      )), optional(','))),
      ')',
    ),

    bounded_type: $ => prec.left(-1, seq(
      choice($._type, $.lifetime, seq('?', $._type), $.use_bound),
      repeat1(seq('+', choice($._type, $.lifetime, seq('?', $._type), $.use_bound))),
    )),

    dyn_type: $ => prec.right(seq('dyn', $.trait_bounds)),
    impl_type: $ => prec.right(seq('impl', $.trait_bounds)),
    never_type: _ => '!',

    qualified_type: $ => seq(
      '<',
      field('type', $._type),
      optional(seq('as', field('alias', $._type))),
      '>',
    ),

    lifetime: $ => prec(1, seq('\'', token.immediate(/(r#)?[_\p{XID_Start}][_\p{XID_Continue}]*/))),

    // ── expressions ──────────────────────────────────────────────────
    _expression: $ => choice(
      $._no_struct_expression,
      $.struct_expression,
    ),

    // Conditions and iterables: rustc bans bare struct literals there, so
    // the restriction is structural, not a precedence trick.
    _no_struct_expression: $ => choice(
      $._control_flow,
      $._invocation,
      $._access,
      $._assignment,
      $._literal,
      $.negative_literal,
      $._name,
      $.scoped_identifier,
      $.generic_function,
      $.self,
      $.metavariable,
      $.binary_expression,
      $.unary_expression,
      $.reference_expression,
      $.try_expression,
      $.await_expression,
      $.cast_expression,
      $.range_expression,
      $.closure_expression,
      $.tuple_expression,
      $.array_expression,
      $.parenthesized_expression,
      $.async_block,
      $.const_block,
      $.unsafe_block,
      $.labeled_block,
      $._body,
    ),

    // Everything that ends with `}` and can stand as a statement without a
    // semicolon.
    _expression_with_block: $ => choice(
      $._branch,
      $._loop,
      $.async_block,
      $.const_block,
      $.unsafe_block,
      $.macro_invocation,
      $.labeled_block,
      $._body,
    ),

    _control_flow: $ => choice($._branch, $._loop, $._jump),

    _branch: $ => choice($.if_expression, $.match_expression),

    if_expression: $ => prec.right(seq(
      'if',
      field('condition', choice($._no_struct_expression, $.let_condition, $.let_chain)),
      field('consequence', $.block),
      optional(field('alternative', $.else_clause)),
    )),

    // `else` is a node, not an inline tail. Same construct as python's and
    // typescript's `else`, so it carries the same name (DESIGN.md §4.1):
    // `alternative:` points at an else_clause in all three, and `(_clause)`
    // has a member here that it was silently missing. What rust admits
    // AFTER the keyword differs — a block or a chained if, where python
    // takes a suite and typescript any statement — but that is the
    // clause's contents, not its shape.
    else_clause: $ => seq('else', choice($.block, $.if_expression)),

    let_condition: $ => seq(
      'let',
      field('pattern', $._match_pattern),
      '=',
      prec.left(PREC.and + 1, field('value', $._no_struct_expression)),
    ),

    let_chain: $ => prec.left(PREC.and, seq(
      field('left', choice($.let_condition, $.let_chain, $._no_struct_expression)),
      '&&',
      field('right', choice($.let_condition, $._no_struct_expression)),
    )),

    match_expression: $ => seq(
      'match',
      field('subject', $._no_struct_expression),
      field('body', $.match_block),
    ),

    match_block: $ => seq(
      '{',
      repeat($.inner_attribute_item),
      optional(seq(
        repeat($.match_arm),
        alias($.last_match_arm, $.match_arm),
      )),
      '}',
    ),

    match_arm: $ => prec.right(seq(
      repeat($._attribute),
      field('pattern', $._match_pattern),
      optional(seq('if', field('guard', $._expression))),
      '=>',
      choice(
        seq(field('value', $._expression), ','),
        seq(field('value', prec(1, $._expression_with_block)), optional(',')),
      ),
    )),

    last_match_arm: $ => seq(
      repeat($._attribute),
      field('pattern', $._match_pattern),
      optional(seq('if', field('guard', $._expression))),
      '=>',
      field('value', $._expression),
      optional(','),
    ),

    _match_pattern: $ => choice($._pattern, $.or_pattern),

    _loop: $ => choice($.while_expression, $.loop_expression, $.for_expression),

    while_expression: $ => seq(
      optional(seq($.loop_label, ':')),
      'while',
      field('condition', choice($._no_struct_expression, $.let_condition, $.let_chain)),
      field('body', $.block),
    ),

    loop_expression: $ => seq(
      optional(seq($.loop_label, ':')),
      'loop',
      field('body', $.block),
    ),

    for_expression: $ => seq(
      optional(seq($.loop_label, ':')),
      'for',
      field('left', $._pattern),
      'in',
      field('right', $._no_struct_expression),
      field('body', $.block),
    ),

    loop_label: $ => $.lifetime,

    _jump: $ => choice(
      $.return_expression,
      $.break_expression,
      $.continue_expression,
      $.yield_expression,
    ),

    return_expression: $ => choice(
      prec.left(seq('return', $._expression)),
      prec(-1, 'return'),
    ),

    break_expression: $ => prec.left(seq(
      'break',
      optional($.loop_label),
      optional($._expression),
    )),

    continue_expression: $ => prec.left(seq('continue', optional($.loop_label))),

    yield_expression: $ => choice(
      prec.left(seq('yield', $._expression)),
      prec(-1, 'yield'),
    ),

    _assignment: $ => choice($.assignment_expression, $.compound_assignment),

    assignment_expression: $ => prec.left(PREC.assign, seq(
      field('left', $._expression),
      '=',
      field('right', $._expression),
    )),

    compound_assignment: $ => prec.left(PREC.assign, seq(
      field('left', $._expression),
      field('operator', choice('+=', '-=', '*=', '/=', '%=', '&=', '|=', '^=', '<<=', '>>=')),
      field('right', $._expression),
    )),

    binary_expression: $ => {
      const table = [
        ['||', PREC.or], ['&&', PREC.and],
        ['==', PREC.compare], ['!=', PREC.compare], ['<', PREC.compare],
        ['<=', PREC.compare], ['>', PREC.compare], ['>=', PREC.compare],
        ['|', PREC.bitor], ['^', PREC.bitxor], ['&', PREC.bitand],
        ['<<', PREC.shift], ['>>', PREC.shift],
        ['+', PREC.add], ['-', PREC.add],
        ['*', PREC.mul], ['/', PREC.mul], ['%', PREC.mul],
      ];
      return choice(...table.map(([op, p]) => prec.left(p, seq(
        field('left', $._expression),
        field('operator', op),
        field('right', $._expression),
      ))));
    },

    unary_expression: $ => prec(PREC.unary, seq(
      field('operator', choice('-', '!', '*')),
      field('operand', $._expression),
    )),

    reference_expression: $ => prec(PREC.unary, seq(
      '&',
      optional(choice(
        $.mutable_specifier,
        seq('raw', choice('const', $.mutable_specifier)),   // &raw const x (2024)
      )),
      field('operand', $._expression),
    )),

    try_expression: $ => prec(PREC.try, seq($._expression, '?')),

    await_expression: $ => prec(PREC.field, seq($._expression, '.', 'await')),

    cast_expression: $ => prec.left(PREC.cast, seq(
      field('value', $._expression),
      'as',
      field('type', $._type),
    )),

    range_expression: $ => choice(
      prec.left(PREC.range, seq($._expression, choice('..', '..='), $._expression)),
      prec.left(PREC.range, seq($._expression, '..')),
      prec.left(PREC.range - 1, seq(choice('..', '..='), $._expression)),
      prec(PREC.range - 1, '..'),
    ),

    _invocation: $ => choice($.call_expression, $.macro_invocation),

    // The callee is a restricted tier, not `$._expression`. Rust has no
    // callable jump or range, so `break (None, None)` is a break carrying a
    // tuple and `p[(a + b)..(a + c)]` is a range between two parenthesised
    // sums -- but with the full expression on the left we read both as CALLS,
    // of `break` and of the open range respectively. Well-formed either way,
    // so no sweep saw it; the node mapping did, because the kinds disagree
    // while the bytes do not.
    call_expression: $ => prec(PREC.call, seq(
      field('function', $._callee),
      field('arguments', $.arguments),
    )),

    _callee: $ => choice(
      $._invocation,
      $._access,
      $._literal,
      $._name,
      $.scoped_identifier,
      $.generic_function,
      $.self,
      $.metavariable,
      $.parenthesized_expression,
      $.try_expression,
      $.await_expression,
      $.unsafe_block,
      $.const_block,
      $.async_block,
      $._body,
      $._branch,
      $._loop,
      // `|| -> T { ... }()` -- an immediately-invoked closure. ONLY the form
      // with an explicit return type, which forces a block body and so has a
      // definite end. A bodyless-typed closure is greedy over its body, so
      // admitting the general form made `|x| f(x)` a CALL of the closure
      // `|x| f` -- 6,143 corpus files, measured.
      alias($._returning_closure, $.closure_expression),

    ),

    arguments: $ => seq(
      '(',
      optional(seq(commaSep1(seq(repeat($._attribute), $._argument)), optional(','))),
      ')',
    ),

    _argument: $ => choice($._expression),

    macro_invocation: $ => seq(
      field('macro', $._path_ref),
      '!',
      $.token_tree,
    ),

    token_tree: $ => choice(
      seq('(', repeat($._token), ')'),
      seq('[', repeat($._token), ']'),
      seq('{', repeat($._token), '}'),
    ),

    _token: $ => choice(
      $.token_tree,
      $.string,
      $.raw_string,
      $.char,
      $._token_soup,
    ),

    // The soup never crosses a '/': a comment can begin there, and a soup
    // token that started earlier would otherwise munch through `//` and
    // trip over brackets quoted inside the comment text.
    // Never crosses whitespace either: a soup run that started at an
    // identifier would otherwise munch straight through ` r#"` and eat a
    // raw string's prefix before the scanner ever saw it.
    // '@' is fenced too: insta/jiff-style `@r##"…"##` snapshots need the
    // scanner to see the `r` at a token boundary.
    _token_soup: _ => token(prec(-1, choice(/[^()\[\]{}"'\/@\s]+/, "'", '/', '@'))),

    _access: $ => choice($.member_expression, $.subscript_expression),

    member_expression: $ => prec(PREC.field, seq(
      field('object', $._expression),
      '.',
      field('property', choice($._name, $.integer)),
      optional(seq('::', field('type_arguments', $.type_arguments))),   // .sum::<i32>()
    )),

    subscript_expression: $ => prec(PREC.call, seq(
      field('object', $._expression),
      '[',
      field('subscript', $._expression),
      ']',
    )),

    _returning_closure: $ => prec(PREC.closure, seq(
      optional('static'),
      optional('async'),
      optional('move'),
      field('parameters', $.closure_parameters),
      '->',
      field('return_type', $._type),
      field('body', $.block),
    )),

    closure_expression: $ => prec(PREC.closure, seq(
      optional('static'),
      optional('async'),
      optional('move'),
      field('parameters', $.closure_parameters),
      choice(
        seq('->', field('return_type', $._type), field('body', $.block)),
        field('body', $._expression),
      ),
    )),

    closure_parameters: $ => seq(
      '|',
      optional(seq(commaSep1(alias($.closure_parameter, $.parameter)), optional(','))),
      '|',
    ),

    closure_parameter: $ => seq(
      field('pattern', $._pattern),
      optional(seq(':', field('type', $._type))),
    ),

    tuple_expression: $ => seq(
      '(',
      seq(repeat($._attribute), $._expression, ','),
      optional(seq(commaSep1(seq(repeat($._attribute), $._expression)), optional(','))),
      ')',
    ),

    array_expression: $ => seq(
      '[',
      optional(choice(
        seq(field('element', $._expression), ';', field('length', $._expression)),
        seq(commaSep1(seq(repeat($._attribute), $._expression)), optional(',')),
      )),
      ']',
    ),

    parenthesized_expression: $ => seq('(', optional($._expression), ')'),

    struct_expression: $ => seq(
      field('name', choice($.identifier, $.scoped_identifier, $.generic_function)),
      field('body', $.field_initializer_list),
    ),

    field_initializer_list: $ => seq(
      '{',
      optional(seq(commaSep1(choice(
        $.field_initializer,
        $.shorthand_field_initializer,
        $.base_field_initializer,
      )), optional(','))),
      '}',
    ),

    field_initializer: $ => seq(
      repeat($._attribute),
      field('name', choice($._name, $.integer)),
      ':',
      field('value', $._expression),
    ),

    shorthand_field_initializer: $ => seq(repeat($._attribute), $._name),

    base_field_initializer: $ => seq('..', $._expression),

    labeled_block: $ => seq($.loop_label, ':', $.block),

    async_block: $ => seq('async', optional('move'), $.block),
    const_block: $ => seq('const', $.block),
    unsafe_block: $ => seq('unsafe', $.block),

    _body: $ => choice($.block),

    block: $ => seq(
      '{',
      repeat($._statement),
      optional(seq(repeat($._attribute), $._expression)),
      '}',
    ),

    // ── paths ────────────────────────────────────────────────────────
    _name: $ => choice(
      $.identifier,
      alias($._soft_keyword, $.identifier),
    ),
    _soft_keyword: _ => choice('raw', 'default', 'auto', 'union', 'macro_rules', 'safe', 'box'),

    identifier: _ => /r#?[_\p{XID_Start}][_\p{XID_Continue}]*|[_\p{XID_Start}][_\p{XID_Continue}]*/,

    self: _ => 'self',
    metavariable: _ => /\$[a-zA-Z_][a-zA-Z0-9_]*/,

    scoped_identifier: $ => seq(
      field('path', optional(choice(
        $._path_ref,
        $.generic_function,
        $.qualified_type,
      ))),
      '::',
      field('name', choice($.identifier, 'super', $.self)),
    ),

    generic_function: $ => seq(
      field('function', choice($.identifier, $.scoped_identifier)),
      '::',
      field('type_arguments', $.type_arguments),
    ),

    _path_ref: $ => choice(
      $.identifier,
      alias($._soft_keyword, $.identifier),
      $.scoped_identifier,
      $.self,
      alias(choice('crate', 'super'), $.identifier),
      $.metavariable,
    ),

    // ── patterns ─────────────────────────────────────────────────────
    _pattern: $ => choice(
      $._name,
      $.generic_function,       // None::<T> in pattern position
      $._literal,
      $.negative_literal,
      $.scoped_identifier,
      $.tuple_pattern,
      $.tuple_struct_pattern,
      $.struct_pattern,
      $.reference_pattern,
      $.mut_pattern,
      $.ref_pattern,
      $.box_pattern,
      $.range_pattern,
      $.slice_pattern,
      $.capture_pattern,
      $.rest_pattern,
      $.parenthesized_pattern,
      $.macro_invocation,
    ),

    or_pattern: $ => prec.left(seq(
      optional('|'),
      $._pattern,
      repeat1(seq('|', $._pattern)),
    )),

    tuple_pattern: $ => seq(
      '(',
      optional(seq(commaSep1($._match_pattern), optional(','))),
      ')',
    ),

    tuple_struct_pattern: $ => seq(
      field('type', choice($.identifier, $.scoped_identifier, $.generic_function)),
      '(',
      optional(seq(commaSep1($._match_pattern), optional(','))),
      ')',
    ),

    struct_pattern: $ => seq(
      field('type', choice($.identifier, $.scoped_identifier, $.generic_function)),
      '{',
      optional(seq(commaSep1(choice(
        $.field_pattern,
        $.rest_pattern,
      )), optional(','))),
      '}',
    ),

    field_pattern: $ => seq(
      repeat($._attribute),
      optional('ref'),
      optional($.mutable_specifier),
      field('name', choice($._name, $.integer)),
      optional(seq(':', field('pattern', $._match_pattern))),
    ),

    reference_pattern: $ => seq(
      choice('&', '&&'),
      optional($.mutable_specifier),
      $._pattern,
    ),

    mut_pattern: $ => prec(-1, seq($.mutable_specifier, $._pattern)),
    box_pattern: $ => seq('box', $._pattern),
    ref_pattern: $ => seq('ref', $._pattern),

    range_pattern: $ => choice(
      seq(
        $._range_pattern_end,
        choice('..=', '...', '..'),
        optional($._range_pattern_end),
      ),
      seq(choice('..=', '...', '..'), $._range_pattern_end),
    ),
    _range_pattern_end: $ => choice(
      $._literal, $.negative_literal, $.scoped_identifier, $._name,
      $.member_expression,
    ),

    slice_pattern: $ => seq(
      '[',
      optional(seq(commaSep1($._match_pattern), optional(','))),
      ']',
    ),

    capture_pattern: $ => seq(
      optional('ref'),
      optional($.mutable_specifier),
      field('name', $._name),
      '@',
      field('pattern', $._pattern),
    ),

    rest_pattern: $ => prec(PREC.range - 1, '..'),
    parenthesized_pattern: $ => seq('(', $._match_pattern, ')'),

    // ── literals ─────────────────────────────────────────────────────
    _literal: $ => choice(
      $.integer,
      $.float,
      $.string,
      $.raw_string,
      $.char,
      $.true,
      $.false,
    ),

    negative_literal: $ => prec(1, seq('-', choice($.integer, $.float))),

    integer: _ => token(choice(
      /[0-9][0-9_]*([ui](8|16|32|64|128|size)|f32|f64)?/,
      /0x[0-9a-fA-F_]+([ui](8|16|32|64|128|size))?/,
      /0o[0-7_]+([ui](8|16|32|64|128|size))?/,
      /0b[01_]+([ui](8|16|32|64|128|size))?/,
    )),

    // One token, deliberately: the external block_comment is an extra, so
    // the scanner is consulted at every token boundary — a multi-part
    // string would let a "/*" inside the text start a hundred-line
    // phantom comment. A single token has no interior boundaries.
    // An ABI is a PLAIN string: `extern b"C"` is `non-string ABI literal`
    // to rustc, and the `[bc]?` in `string` let it through. A separate token
    // rather than a check, because the two are only ever valid in disjoint
    // states, so the lexer can tell them apart from context. Aliased to
    // `string` so node-types.json does not gain a type for it.
    _abi: _ => token(seq(
      '"',
      repeat(choice(/[^"\\]+/, /\\(.|\r?\n)/)),
      '"',
    )),

    string: _ => token(seq(
      /[bc]?"/,
      repeat(choice(/[^"\\]+/, /\\(.|\r?\n)/)),
      '"',
    )),

    char: _ => token(seq(
      optional('b'),
      '\'',
      choice(
        /[^'\\]/,
        /\\(x[0-9a-fA-F]{1,2}|u\{[0-9a-fA-F_]+\}|.)/,
      ),
      '\'',
    )),

    true: _ => 'true',
    false: _ => 'false',

    line_comment: _ => token(seq('//', /.*/)),
  },
});

function commaSep1(rule) {
  return sep1(rule, ',');
}

function sep1(rule, separator) {
  return seq(rule, repeat(seq(separator, rule)));
}
