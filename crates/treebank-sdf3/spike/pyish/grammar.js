// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "pyish",
  word: $ => $.id,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._statement, $._expression, $._declaration, $._body, $._parameter, $._name, $._literal, $._directive, $._assignment, $._invocation, $._branch, $._loop, $._jump, $._control_flow],
  externals: $ => [$._newline, $._indent, $._dedent, $._error_sentinel],
  reserved: { global: $ => ["def", "else", "global", "if", "pass", "print", "return", "while"] },
  rules: {
    program: $ => repeat($._statement),

    assign: $ => seq(
      field("target", $._name),
      "=",
      field("value", $._expression),
      $._newline
    ),

    _assignment: $ => choice(
      $.assign
    ),

    expr: $ => seq(
      $._expression,
      $._newline
    ),

    return: $ => seq(
      "return",
      field("value", $._expression),
      $._newline
    ),

    _jump: $ => choice(
      $.return
    ),

    global: $ => seq(
      "global",
      field("names", seq(
        $._name,
        repeat(seq(
          ",",
          $._name
        ))
      )),
      $._newline
    ),

    _directive: $ => choice(
      $.global
    ),

    pass: $ => seq(
      "pass",
      $._newline
    ),

    print: $ => seq(
      "print",
      "(",
      field("value", $._expression),
      ")",
      $._newline
    ),

    if: $ => seq(
      "if",
      field("condition", $._expression),
      ":",
      $._indent,
      field("consequence", $._body),
      $._dedent,
      field("alternative", optional($.else_clause))
    ),

    _branch: $ => choice(
      $.if
    ),

    while: $ => seq(
      "while",
      field("condition", $._expression),
      ":",
      $._indent,
      field("body", $._body),
      $._dedent
    ),

    _loop: $ => choice(
      $.while
    ),

    _control_flow: $ => choice(
      $._branch,
      $._loop,
      $._jump
    ),

    def: $ => seq(
      "def",
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
      ":",
      $._indent,
      field("body", $._body),
      $._dedent
    ),

    _declaration: $ => choice(
      $.def
    ),

    _statement: $ => choice(
      $._assignment,
      $.expr,
      $._control_flow,
      $._directive,
      $.pass,
      $.print,
      $._declaration
    ),

    else_clause: $ => seq(
      "else",
      ":",
      $._indent,
      field("body", $._body),
      $._dedent
    ),

    block: $ => repeat1($._statement),

    _body: $ => choice(
      $.block
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

    id: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    _name: $ => choice(
      $.id
    ),

    int: $ => /(?:[0-9])+/,

    comment: $ => /#(?:[^\n\r])*/,

  },
});
