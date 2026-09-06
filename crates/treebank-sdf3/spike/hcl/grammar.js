// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "hcl",
  word: $ => $.identifier,
  extras: $ => [/[ \t]/, /(?:[\r])?[\n]/, $.comment, $.block_comment],
  supertypes: $ => [$._declaration, $._name, $._expression, $._literal, $._argument, $._body, $._branch, $._loop, $._invocation, $._access, $._interpolation, $._control_flow],
  externals: $ => [$.quote, $.escape_sequence, $._qchunk, $._hchunk, $.heredoc_end, $._dir_else, $._dir_endif, $._dir_endfor, $._nl, $.heredoc_start, $._error_sentinel],
  inline: $ => [$._un_op, $._bin_op_mul, $._bin_op_add, $._bin_op_cmp, $._bin_op_eq, $._bin_op_and, $._bin_op_or, $._obj_assign, $._interp_open, $._interp_close, $._dir_open, $._dir_close],
  rules: {
    config_file: $ => seq(
      repeat(choice(
        seq(
          $._declaration,
          $._nl
        )
      )),
      optional($._declaration)
    ),

    _declaration: $ => choice(
      $.attribute,
      $.block
    ),

    attribute: $ => seq(
      field("name", $._name),
      "=",
      field("value", $._expression)
    ),

    block: $ => seq(
      field("type", $._name),
      field("label", repeat($._label)),
      field("body", $._body)
    ),

    _label: $ => choice(
      $._name,
      $.string_lit
    ),

    body: $ => choice(
      seq(
        "{",
        $._nl,
        repeat(choice(
          seq(
            $._declaration,
            $._nl
          )
        )),
        "}"
      ),
      seq(
        "{",
        optional($.attribute),
        "}"
      )
    ),

    _body: $ => choice(
      $.body
    ),

    _name: $ => choice(
      $.identifier
    ),

    function_call: $ => seq(
      field("function", $.function_name),
      "(",
      optional($.arguments),
      ")"
    ),

    _invocation: $ => choice(
      $.function_call
    ),

    parenthesized_expression: $ => seq(
      "(",
      $._expression,
      ")"
    ),

    unary_expression: $ => prec(8, seq(
      field("operator", $._un_op),
      field("operand", $._expression)
    )),

    binary_expression: $ => choice(
      prec.left(7, seq(
        field("left", $._expression),
        field("operator", $._bin_op_mul),
        field("right", $._expression)
      )),
      prec.left(6, seq(
        field("left", $._expression),
        field("operator", $._bin_op_add),
        field("right", $._expression)
      )),
      prec.left(5, seq(
        field("left", $._expression),
        field("operator", $._bin_op_cmp),
        field("right", $._expression)
      )),
      prec.left(4, seq(
        field("left", $._expression),
        field("operator", $._bin_op_eq),
        field("right", $._expression)
      )),
      prec.left(3, seq(
        field("left", $._expression),
        field("operator", $._bin_op_and),
        field("right", $._expression)
      )),
      prec.left(2, seq(
        field("left", $._expression),
        field("operator", $._bin_op_or),
        field("right", $._expression)
      ))
    ),

    conditional: $ => prec.right(1, seq(
      field("condition", $._expression),
      "?",
      field("consequence", $._expression),
      ":",
      field("alternative", $._expression)
    )),

    _branch: $ => choice(
      $.conditional
    ),

    get_attr: $ => prec(9, seq(
      field("operand", $._expression),
      ".",
      field("name", $._name)
    )),

    index: $ => prec(9, seq(
      field("operand", $._expression),
      "[",
      field("key", $._expression),
      "]"
    )),

    legacy_index: $ => prec(9, seq(
      field("operand", $._expression),
      field("key", token.immediate(/\.(?:[0-9])+/))
    )),

    attr_splat: $ => prec.right(9, seq(
      field("operand", $._expression),
      ".",
      "*",
      repeat($._splat_name)
    )),

    full_splat: $ => prec.right(9, seq(
      field("operand", $._expression),
      "[",
      "*",
      "]",
      repeat($._splat_suffix)
    )),

    _access: $ => choice(
      $.get_attr,
      $.index,
      $.legacy_index,
      $.attr_splat,
      $.full_splat
    ),

    _expression: $ => choice(
      $._literal,
      $.identifier,
      $.quoted_template,
      $.heredoc_template,
      $.tuple,
      $.object,
      $._control_flow,
      $._invocation,
      $.parenthesized_expression,
      $.unary_expression,
      $.binary_expression,
      $._access
    ),

    _un_op: $ => choice(
      "-",
      "!"
    ),

    _bin_op_mul: $ => choice(
      "*",
      "/",
      "%"
    ),

    _bin_op_add: $ => choice(
      "+",
      "-"
    ),

    _bin_op_cmp: $ => choice(
      ">",
      ">=",
      "<",
      "<="
    ),

    _bin_op_eq: $ => choice(
      "==",
      "!="
    ),

    _bin_op_and: $ => "&&",

    _bin_op_or: $ => "||",

    _splat_name: $ => seq(
      ".",
      field("name", $._name)
    ),

    _splat_suffix: $ => choice(
      seq(
        ".",
        field("name", $._name)
      ),
      seq(
        "[",
        field("key", $._expression),
        "]"
      )
    ),

    true: $ => "true",

    false: $ => "false",

    null: $ => "null",

    _literal: $ => choice(
      $.integer,
      $.float,
      $.true,
      $.false,
      $.null
    ),

    function_name: $ => seq(
      $._name,
      repeat(seq(
        "::",
        $._name
      ))
    ),

    arguments: $ => seq(
      seq(
        $._argument,
        repeat(seq(
          ",",
          $._argument
        ))
      ),
      optional(choice(
        ",",
        $.ellipsis
      ))
    ),

    _argument: $ => choice(
      $._expression
    ),

    ellipsis: $ => "...",

    tuple: $ => seq(
      "[",
      optional(choice(
        seq(
          seq(
            $._expression,
            repeat(seq(
              ",",
              $._expression
            ))
          ),
          optional(",")
        )
      )),
      "]"
    ),

    object: $ => seq(
      "{",
      optional($._nl),
      optional($._obj_elems),
      "}"
    ),

    _obj_elems: $ => seq(
      $.object_elem,
      repeat(choice(
        seq(
          $._obj_sep,
          $.object_elem
        )
      )),
      optional($._obj_sep)
    ),

    _obj_sep: $ => choice(
      seq(
        ",",
        optional($._nl)
      ),
      $._nl
    ),

    object_elem: $ => seq(
      field("key", $._expression),
      $._obj_assign,
      field("value", $._expression)
    ),

    _obj_assign: $ => choice(
      "=",
      ":"
    ),

    for_tuple_expr: $ => seq(
      "[",
      $._for_intro,
      field("result", $._expression),
      field("condition", optional($.for_cond)),
      "]"
    ),

    for_object_expr: $ => seq(
      "{",
      optional($._nl),
      $._for_intro,
      field("key", $._expression),
      "=>",
      field("value", $._expression),
      field("grouping", optional($.ellipsis)),
      field("condition", optional($.for_cond)),
      "}"
    ),

    _loop: $ => choice(
      $.for_tuple_expr,
      $.for_object_expr
    ),

    _control_flow: $ => choice(
      $._branch,
      $._loop
    ),

    _for_intro: $ => seq(
      "for",
      field("binding", $._name),
      optional($._for_second),
      "in",
      field("collection", $._expression),
      ":"
    ),

    _for_second: $ => seq(
      ",",
      field("binding", $._name)
    ),

    for_cond: $ => seq(
      "if",
      field("condition", $._expression)
    ),

    template_interpolation: $ => seq(
      $._interp_open,
      field("expression", $._expression),
      $._interp_close
    ),

    _interpolation: $ => choice(
      $.template_interpolation
    ),

    _interp_open: $ => choice(
      "${~",
      "${"
    ),

    _interp_close: $ => choice(
      "~}",
      "}"
    ),

    _dir_open: $ => choice(
      "%{~",
      "%{"
    ),

    _dir_close: $ => choice(
      "~}",
      "}"
    ),

    _dir_if: $ => seq(
      $._dir_open,
      "if",
      field("condition", $._expression),
      $._dir_close
    ),

    _dir_for: $ => seq(
      $._dir_open,
      "for",
      field("binding", $._name),
      optional($._for_second),
      "in",
      field("collection", $._expression),
      $._dir_close
    ),

    _q_part: $ => choice(
      $.template_literal,
      $._interpolation,
      $.template_if,
      $.template_for
    ),

    _h_part: $ => choice(
      alias($.h_lit_template_literal, $.template_literal),
      $._interpolation,
      alias($.h_if_template_if, $.template_if),
      alias($.h_for_template_for, $.template_for)
    ),

    quoted_template: $ => seq(
      $.quote,
      repeat($._q_part),
      $.quote
    ),

    template_literal: $ => prec.right(0, repeat1(choice(
      $._qchunk,
      $.escape_sequence
    ))),

    template_if: $ => seq(
      $._dir_if,
      field("consequence", optional($.template_body)),
      optional($.else_clause),
      $._dir_endif
    ),

    else_clause: $ => seq(
      $._dir_else,
      field("alternative", optional($.template_body))
    ),

    template_for: $ => seq(
      $._dir_for,
      field("body", optional($.template_body)),
      $._dir_endfor
    ),

    template_body: $ => repeat1($._q_part),

    heredoc_template: $ => seq(
      $.heredoc_start,
      repeat($._h_part),
      $.heredoc_end
    ),

    h_lit_template_literal: $ => prec.right(0, repeat1($._hchunk)),

    h_if_template_if: $ => seq(
      $._dir_if,
      field("consequence", optional(alias($.h_body_template_body, $.template_body))),
      optional(alias($.h_else_else_clause, $.else_clause)),
      $._dir_endif
    ),

    h_else_else_clause: $ => seq(
      $._dir_else,
      field("alternative", optional(alias($.h_body_template_body, $.template_body)))
    ),

    h_for_template_for: $ => seq(
      $._dir_for,
      field("body", optional(alias($.h_body_template_body, $.template_body))),
      $._dir_endfor
    ),

    h_body_template_body: $ => repeat1($._h_part),

    float: $ => /(?:(?:[0-9])+\.(?:[0-9])+(?:(?:[eE](?:[\-+])?(?:[0-9])+))?|(?:[0-9])+[eE](?:[\-+])?(?:[0-9])+)/,

    identifier: $ => /[a-zA-Z_](?:[a-zA-Z0-9_\-])*/,

    integer: $ => /(?:[0-9])+/,

    comment: $ => /(?:#|\/\/)(?:[^\n\r])*/,

    block_comment: $ => /\/\*(?:(?:[^*]|(?:[*])+[^*\/]))*(?:[*])+\//,

    string_lit: $ => /"(?:(?:[^"\\\r\n]|\\[nrt"\\]|\\u(?:[0-9a-fA-F])(?:[0-9a-fA-F])(?:[0-9a-fA-F])(?:[0-9a-fA-F])|\\U(?:[0-9a-fA-F])(?:[0-9a-fA-F])(?:[0-9a-fA-F])(?:[0-9a-fA-F])(?:[0-9a-fA-F])(?:[0-9a-fA-F])(?:[0-9a-fA-F])(?:[0-9a-fA-F])))*"/,

    _legacy_key: $ => /\.(?:[0-9])+/,

  },
});
