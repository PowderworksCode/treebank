// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "jsish",
  word: $ => $.id,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._stmt, $._exp],
  reserved: { global: $ => ["function", "if", "let", "return", "var"] },
  rules: {
    program: $ => repeat($._stmt),

    function: $ => seq(
      "function",
      field("name", $.id),
      "(",
      field("parameters", optional(seq(
        $.param,
        repeat(seq(
          ",",
          $.param
        ))
      ))),
      ")",
      field("body", $.block)
    ),

    var: $ => seq(
      "var",
      field("name", $.id),
      "=",
      field("value", $._exp),
      ";"
    ),

    let: $ => seq(
      "let",
      field("name", $.id),
      "=",
      field("value", $._exp),
      ";"
    ),

    assign: $ => seq(
      field("target", $.id),
      "=",
      field("value", $._exp),
      ";"
    ),

    print: $ => seq(
      "console.log",
      "(",
      field("value", $._exp),
      ")",
      ";"
    ),

    return: $ => seq(
      "return",
      field("value", $._exp),
      ";"
    ),

    if: $ => seq(
      "if",
      "(",
      field("condition", $._exp),
      ")",
      field("consequence", $.block)
    ),

    expr: $ => seq(
      $._exp,
      ";"
    ),

    _stmt: $ => choice(
      $.function,
      $.var,
      $.let,
      $.assign,
      $.print,
      $.return,
      $.if,
      $.expr,
      $.block
    ),

    block: $ => seq(
      "{",
      repeat($._stmt),
      "}"
    ),

    param: $ => field("name", $.id),

    exp_int: $ => $.int,

    call: $ => prec(4, seq(
      field("function", $._exp),
      "(",
      field("arguments", optional(seq(
        $._exp,
        repeat(seq(
          ",",
          $._exp
        ))
      ))),
      ")"
    )),

    neg: $ => prec(3, seq(
      "-",
      field("operand", $._exp)
    )),

    mul: $ => prec.left(2, seq(
      field("left", $._exp),
      "*",
      field("right", $._exp)
    )),

    add: $ => prec.left(1, seq(
      field("left", $._exp),
      "+",
      field("right", $._exp)
    )),

    sub: $ => prec.left(1, seq(
      field("left", $._exp),
      "-",
      field("right", $._exp)
    )),

    exp_bracket: $ => seq(
      "(",
      $._exp,
      ")"
    ),

    _exp: $ => choice(
      $.id,
      $.exp_int,
      $.call,
      $.neg,
      $.mul,
      $.add,
      $.sub,
      $.exp_bracket
    ),

    id: $ => /[a-zA-Z_$](?:[a-zA-Z0-9_$])*/,

    int: $ => /(?:[0-9])+/,

    comment: $ => /\/\/(?:[^\n\r])*/,

  },
});
