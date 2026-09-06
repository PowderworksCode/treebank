// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "postgres_9_4",
  word: $ => $.name,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._statement, $._type, $._name, $._expression, $._literal, $._invocation, $._declaration, $._assignment, $._modifier],
  reserved: { global: $ => ["AND", "AS", "ASC", "BY", "CREATE", "DELETE", "DESC", "DROP", "FROM", "ILIKE", "INSERT", "INT", "INTO", "LIKE", "LIMIT", "NOT", "NULL", "OFFSET", "OIDS", "OR", "ORDER", "OVER", "PARTITION", "RETURNING", "SELECT", "SET", "TABLE", "TEXT", "UPDATE", "VALUES", "VARCHAR", "WHERE", "WITH", "WITHOUT"] },
  rules: {
    script: $ => repeat($._statement),

    stmt_select: $ => seq(
      field("with", optional($.with)),
      $.select,
      ";"
    ),

    insert: $ => seq(
      "INSERT",
      "INTO",
      field("table", $._name),
      "(",
      field("columns", seq(
        $._name,
        repeat(seq(
          ",",
          $._name
        ))
      )),
      ")",
      "VALUES",
      "(",
      field("values", seq(
        $._expression,
        repeat(seq(
          ",",
          $._expression
        ))
      )),
      ")",
      field("returning", optional($.returning)),
      ";"
    ),

    update: $ => seq(
      "UPDATE",
      field("table", $._name),
      "SET",
      seq(
        $._assignment,
        repeat(seq(
          ",",
          $._assignment
        ))
      ),
      field("where", optional($.where)),
      field("returning", optional($.returning)),
      ";"
    ),

    delete: $ => seq(
      "DELETE",
      "FROM",
      field("table", $._name),
      field("where", optional($.where)),
      field("returning", optional($.returning)),
      ";"
    ),

    create_table: $ => seq(
      "CREATE",
      "TABLE",
      field("table", $._name),
      "(",
      seq(
        $.col_def,
        repeat(seq(
          ",",
          $.col_def
        ))
      ),
      ")",
      field("tail", optional($._modifier)),
      ";"
    ),

    _declaration: $ => choice(
      $.create_table
    ),

    drop_table: $ => seq(
      "DROP",
      "TABLE",
      field("table", $._name),
      ";"
    ),

    _statement: $ => choice(
      $.stmt_select,
      $.insert,
      $.update,
      $.delete,
      $._declaration,
      $.drop_table
    ),

    select: $ => seq(
      "SELECT",
      field("items", seq(
        $.item,
        repeat(seq(
          ",",
          $.item
        ))
      )),
      field("from", optional($.from)),
      field("where", optional($.where)),
      field("order", optional($.order_by)),
      field("limit", optional($.limit)),
      field("offset", optional($.offset))
    ),

    item: $ => seq(
      $._expression,
      field("alias", optional($._alias))
    ),

    as: $ => seq(
      "AS",
      $._name
    ),

    bare: $ => $._name,

    _alias: $ => choice(
      $.as,
      $.bare
    ),

    from: $ => seq(
      "FROM",
      field("table", $._name)
    ),

    where: $ => seq(
      "WHERE",
      $._expression
    ),

    order_by: $ => seq(
      "ORDER",
      "BY",
      seq(
        $.order,
        repeat(seq(
          ",",
          $.order
        ))
      )
    ),

    order: $ => seq(
      $._expression,
      field("dir", optional($._modifier))
    ),

    asc: $ => "ASC",

    desc: $ => "DESC",

    cte: $ => seq(
      field("name", $._name),
      "AS",
      "(",
      $.select,
      ")"
    ),

    assign: $ => seq(
      field("column", $._name),
      "=",
      field("value", $._expression)
    ),

    _assignment: $ => choice(
      $.assign
    ),

    col_def: $ => seq(
      field("name", $._name),
      $._type
    ),

    type_int: $ => "INT",

    varchar: $ => seq(
      "VARCHAR",
      "(",
      $.int,
      ")"
    ),

    text: $ => "TEXT",

    _type: $ => choice(
      $.type_int,
      $.varchar,
      $.text
    ),

    ident_name: $ => $.name,

    quoted: $ => $.dquoted,

    _name: $ => choice(
      $.ident_name,
      $.quoted
    ),

    column: $ => seq(
      field("table", $._name),
      ".",
      field("column", $._name)
    ),

    star: $ => "*",

    exp_int: $ => $.int,

    str: $ => $.string,

    null: $ => "NULL",

    _literal: $ => choice(
      $.exp_int,
      $.str,
      $.null
    ),

    call: $ => prec(18, seq(
      field("function", $.name),
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

    neg: $ => prec(17, seq(
      "-",
      $._expression
    )),

    mul: $ => prec.left(16, seq(
      field("left", $._expression),
      "*",
      field("right", $._expression)
    )),

    add: $ => prec.left(15, seq(
      field("left", $._expression),
      "+",
      field("right", $._expression)
    )),

    sub: $ => prec.left(15, seq(
      field("left", $._expression),
      "-",
      field("right", $._expression)
    )),

    eq: $ => prec.left(14, seq(
      field("left", $._expression),
      "=",
      field("right", $._expression)
    )),

    lt: $ => prec.left(14, seq(
      field("left", $._expression),
      "<",
      field("right", $._expression)
    )),

    gt: $ => prec.left(14, seq(
      field("left", $._expression),
      ">",
      field("right", $._expression)
    )),

    like: $ => prec.left(14, seq(
      field("left", $._expression),
      "LIKE",
      field("right", $._expression)
    )),

    not: $ => prec(13, seq(
      "NOT",
      $._expression
    )),

    and: $ => prec.left(12, seq(
      field("left", $._expression),
      "AND",
      field("right", $._expression)
    )),

    or: $ => prec.left(11, seq(
      field("left", $._expression),
      "OR",
      field("right", $._expression)
    )),

    exp_bracket: $ => seq(
      "(",
      $._expression,
      ")"
    ),

    over: $ => prec(10, seq(
      $._expression,
      "OVER",
      "(",
      field("partition", optional($.partition)),
      field("order", optional($.order_by)),
      ")"
    )),

    arrow: $ => prec.left(8, seq(
      field("left", $._expression),
      "->",
      field("right", $._expression)
    )),

    arrow_text: $ => prec.left(8, seq(
      field("left", $._expression),
      "->>",
      field("right", $._expression)
    )),

    cast: $ => prec(6, seq(
      $._expression,
      "::",
      $._type
    )),

    i_like: $ => prec.left(4, seq(
      field("left", $._expression),
      "ILIKE",
      field("right", $._expression)
    )),

    _expression: $ => choice(
      $._name,
      $.column,
      $.star,
      $._literal,
      $._invocation,
      $.neg,
      $.mul,
      $.add,
      $.sub,
      $.eq,
      $.lt,
      $.gt,
      $.like,
      $.not,
      $.and,
      $.or,
      $.exp_bracket,
      $.over,
      $.arrow,
      $.arrow_text,
      $.cast,
      $.i_like
    ),

    limit: $ => seq(
      "LIMIT",
      field("count", $.int)
    ),

    offset: $ => seq(
      "OFFSET",
      field("start", $.int)
    ),

    with: $ => seq(
      "WITH",
      seq(
        $.cte,
        repeat(seq(
          ",",
          $.cte
        ))
      )
    ),

    partition: $ => seq(
      "PARTITION",
      "BY",
      seq(
        $._expression,
        repeat(seq(
          ",",
          $._expression
        ))
      )
    ),

    returning: $ => seq(
      "RETURNING",
      seq(
        $.item,
        repeat(seq(
          ",",
          $.item
        ))
      )
    ),

    with_oids: $ => seq(
      "WITH",
      "OIDS"
    ),

    without_oids: $ => seq(
      "WITHOUT",
      "OIDS"
    ),

    _modifier: $ => choice(
      $.asc,
      $.desc,
      $.with_oids,
      $.without_oids
    ),

    dollar: $ => /\$\$(?:[^$])*\$\$/,

    dquoted: $ => /"(?:[^"])*"/,

    int: $ => /(?:[0-9])+/,

    comment: $ => /--(?:[^\n\r])*/,

    name: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    string: $ => /(?:'(?:(?:''|[^']))*'|(?:\$\$(?:[^$])*\$\$))/,

  },
});
