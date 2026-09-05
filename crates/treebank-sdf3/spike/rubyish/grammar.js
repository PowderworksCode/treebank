// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "rubyish",
  extras: $ => [/[ \t]/, $.comment],
  supertypes: $ => [$._stmt, $._exp, $._arg],
  externals: $ => [$.regex, $._minus_spaced_tight, $._star_spaced_tight, $._lbracket_adjacent, $._lbracket_spaced, $._lparen_adjacent, $._lparen_spaced, $._minus, $._slash, $._star, $._error_sentinel],
  rules: {
    program: $ => seq(
      optional($.nl),
      repeat($._stmt)
    ),

    assign: $ => seq(
      field("target", $.id),
      "=",
      field("value", $._exp),
      $.nl
    ),

    expr: $ => seq(
      $._exp,
      $.nl
    ),

    _stmt: $ => choice(
      $.assign,
      $.expr
    ),

    exp_int: $ => $.int,

    exp_regex: $ => $.regex,

    array: $ => seq(
      alias($._lbracket_spaced, "["),
      field("elements", optional(seq(
        $._exp,
        repeat(seq(
          ",",
          $._exp
        ))
      ))),
      "]"
    ),

    exp_bracket: $ => seq(
      alias($._lparen_spaced, "("),
      $._exp,
      ")"
    ),

    index: $ => prec(5, seq(
      field("receiver", $._exp),
      alias($._lbracket_adjacent, "["),
      field("index", $._exp),
      "]"
    )),

    call: $ => prec(5, seq(
      field("method", $.id),
      alias($._lparen_adjacent, "("),
      field("arguments", optional(seq(
        $._exp,
        repeat(seq(
          ",",
          $._exp
        ))
      ))),
      ")"
    )),

    command: $ => prec.dynamic(1, prec(1, seq(
      field("method", $.id),
      field("argument", $._arg)
    ))),

    neg: $ => prec(4, seq(
      alias($._minus_spaced_tight, "-"),
      field("operand", $._exp)
    )),

    mul: $ => prec.left(3, seq(
      field("left", $._exp),
      alias($._star, "*"),
      field("right", $._exp)
    )),

    div: $ => prec.left(3, seq(
      field("left", $._exp),
      alias($._slash, "/"),
      field("right", $._exp)
    )),

    add: $ => prec.left(2, seq(
      field("left", $._exp),
      "+",
      field("right", $._exp)
    )),

    sub: $ => prec.left(2, seq(
      field("left", $._exp),
      alias($._minus, "-"),
      field("right", $._exp)
    )),

    _exp: $ => choice(
      $.id,
      $.exp_int,
      $.exp_regex,
      $.array,
      $.exp_bracket,
      $.index,
      $.call,
      $.command,
      $.neg,
      $.mul,
      $.div,
      $.add,
      $.sub
    ),

    splat: $ => seq(
      alias($._star_spaced_tight, "*"),
      field("operand", $._exp)
    ),

    _arg: $ => choice(
      $._exp,
      $.splat
    ),

    id: $ => /[a-z_](?:[a-zA-Z0-9_])*/,

    int: $ => /(?:[0-9])+/,

    comment: $ => /#(?:[^\n])*/,

    nl: $ => /[\n](?:(?:[ \t\n]|#(?:[^\n])*))*/,

  },
});
