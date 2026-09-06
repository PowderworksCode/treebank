// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "rust_2021",
  word: $ => $.id,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._statement, $._expression, $._declaration, $._type, $._body, $._parameter, $._name, $._literal, $._assignment, $._invocation, $._branch, $._loop, $._jump, $._control_flow],
  conflicts: $ => [[$._statement, $._expression]],
  reserved: { global: $ => ["async", "await", "dyn", "else", "fn", "i64", "if", "let", "mut", "return", "try", "while"] },
  rules: {
    program: $ => seq(
      repeat($._declaration),
      optional($._reserved_word)
    ),

    fn: $ => seq(
      "fn",
      field("name", $._name),
      "(",
      field("parameters", optional(seq(
        $._parameter,
        repeat(seq(
          ",",
          $._parameter
        ))
      ))),
      ")",
      field("ret", optional($._type)),
      field("body", $._body)
    ),

    _declaration: $ => choice(
      $.fn
    ),

    ret: $ => seq(
      "->",
      "i64"
    ),

    _type: $ => choice(
      $.ret
    ),

    param: $ => seq(
      field("name", $._name),
      ":",
      "i64"
    ),

    _parameter: $ => choice(
      $.param
    ),

    block: $ => seq(
      "{",
      repeat($._statement),
      field("tail", optional($._expression)),
      "}"
    ),

    _body: $ => choice(
      $.block
    ),

    let: $ => seq(
      "let",
      field("pattern", $._name),
      "=",
      field("value", $._expression),
      ";"
    ),

    let_mut: $ => seq(
      "let",
      "mut",
      field("pattern", $._name),
      "=",
      field("value", $._expression),
      ";"
    ),

    assign: $ => seq(
      field("target", $._name),
      "=",
      field("value", $._expression),
      ";"
    ),

    _assignment: $ => choice(
      $.assign
    ),

    if: $ => seq(
      "if",
      field("condition", $._expression),
      field("consequence", $._body),
      field("alternative", optional($.else_clause))
    ),

    _branch: $ => choice(
      $.if
    ),

    while: $ => seq(
      "while",
      field("condition", $._expression),
      field("body", $._body)
    ),

    _loop: $ => choice(
      $.while
    ),

    return: $ => seq(
      "return",
      field("value", $._expression),
      ";"
    ),

    _jump: $ => choice(
      $.return
    ),

    _control_flow: $ => choice(
      $._branch,
      $._loop,
      $._jump
    ),

    print: $ => seq(
      "println!",
      "(",
      "\"",
      "{",
      "}",
      "\"",
      ",",
      field("value", $._expression),
      ")",
      ";"
    ),

    expr: $ => seq(
      $._expression,
      ";"
    ),

    _statement: $ => choice(
      $.let,
      $.let_mut,
      $._assignment,
      $._control_flow,
      $.print,
      $.expr,
      $._declaration,
      $._body
    ),

    else_clause: $ => seq(
      "else",
      field("body", $._body)
    ),

    exp_int: $ => $.int,

    _literal: $ => choice(
      $.exp_int
    ),

    call: $ => prec(5, seq(
      field("function", $._expression),
      "(",
      field("arguments", optional(seq(
        $._expression,
        repeat(seq(
          ",",
          $._expression
        ))
      ))),
      ")"
    )),

    _invocation: $ => choice(
      $.call
    ),

    neg: $ => prec(4, seq(
      "-",
      field("operand", $._expression)
    )),

    mul: $ => prec.left(3, seq(
      field("left", $._expression),
      "*",
      field("right", $._expression)
    )),

    add: $ => prec.left(2, seq(
      field("left", $._expression),
      "+",
      field("right", $._expression)
    )),

    sub: $ => prec.left(2, seq(
      field("left", $._expression),
      "-",
      field("right", $._expression)
    )),

    lt: $ => prec.left(1, seq(
      field("left", $._expression),
      "<",
      field("right", $._expression)
    )),

    exp_bracket: $ => seq(
      "(",
      $._expression,
      ")"
    ),

    _expression: $ => choice(
      $._name,
      $._literal,
      $._body,
      $._invocation,
      $.neg,
      $.mul,
      $.add,
      $.sub,
      $.lt,
      $.exp_bracket
    ),

    id: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    _name: $ => choice(
      $.id
    ),

    int: $ => /(?:[0-9])+/,

    comment: $ => /\/\/(?:[^\n\r])*/,

    _reserved_word: $ => seq(
      /[^\s\S]/,
      choice(
        "async",
        "await",
        "dyn",
        "try"
      )
    ),

  },
});
