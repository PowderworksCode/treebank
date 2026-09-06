// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "jsish",
  word: $ => $.id,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._statement, $._expression, $._declaration, $._body, $._parameter, $._name, $._literal, $._assignment, $._invocation, $._branch, $._loop, $._jump, $._control_flow],
  reserved: { global: $ => ["else", "function", "if", "let", "return", "var", "while"] },
  rules: {
    program: $ => repeat($._statement),

    function: $ => seq(
      "function",
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
      field("body", $._body)
    ),

    _declaration: $ => choice(
      $.function
    ),

    var: $ => seq(
      "var",
      field("name", $._name),
      "=",
      field("value", $._expression),
      ";"
    ),

    let: $ => seq(
      "let",
      field("name", $._name),
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

    print: $ => seq(
      "console.log",
      "(",
      field("value", $._expression),
      ")",
      ";"
    ),

    return: $ => seq(
      "return",
      field("value", $._expression),
      ";"
    ),

    _jump: $ => choice(
      $.return
    ),

    if: $ => seq(
      "if",
      "(",
      field("condition", $._expression),
      ")",
      field("consequence", $._body),
      field("alternative", optional($.else_clause))
    ),

    _branch: $ => choice(
      $.if
    ),

    while: $ => seq(
      "while",
      "(",
      field("condition", $._expression),
      ")",
      field("body", $._body)
    ),

    _loop: $ => choice(
      $.while
    ),

    _control_flow: $ => choice(
      $._branch,
      $._loop,
      $._jump
    ),

    expr: $ => seq(
      $._expression,
      ";"
    ),

    _statement: $ => choice(
      $._declaration,
      $.var,
      $.let,
      $._assignment,
      $.print,
      $._control_flow,
      $.expr,
      $._body
    ),

    block: $ => seq(
      "{",
      repeat($._statement),
      "}"
    ),

    _body: $ => choice(
      $.block
    ),

    else_clause: $ => seq(
      "else",
      field("body", $._body)
    ),

    param: $ => field("name", $._name),

    _parameter: $ => choice(
      $.param
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
      $._invocation,
      $.neg,
      $.mul,
      $.add,
      $.sub,
      $.lt,
      $.exp_bracket
    ),

    id: $ => /[a-zA-Z_$](?:[a-zA-Z0-9_$])*/,

    _name: $ => choice(
      $.id
    ),

    int: $ => /(?:[0-9])+/,

    comment: $ => /\/\/(?:[^\n\r])*/,

  },
});
