// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "mysql_8_0",
  word: $ => $.name,
  extras: $ => [/[ \t\n\r]/, $.comment, $.comment_2],
  supertypes: $ => [$._statement, $._type, $._name, $._expression, $._literal, $._invocation, $._declaration, $._assignment, $._modifier],
  reserved: { global: $ => ["AND", "AS", "ASC", "BY", "CREATE", "DELETE", "DESC", "DROP", "DUPLICATE", "FROM", "IGNORE", "INSERT", "INT", "INTO", "KEY", "LIKE", "LIMIT", "NOT", "NULL", "OFFSET", "ON", "OR", "ORDER", "OVER", "PARTITION", "REPLACE", "SELECT", "SET", "SQL_NO_CACHE", "TABLE", "TEXT", "UPDATE", "VALUES", "VARCHAR", "WHERE", "WITH"] },
  rules: {
    script: $ => repeat($._statement),

    stmt_select: $ => seq(
      field("with", optional($.with)),
      $.select,
      ";"
    ),

    insert: $ => seq(
      "INSERT",
      field("hints", repeat($._modifier)),
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
      field("upsert", optional($.on_duplicate_key)),
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
      ";"
    ),

    delete: $ => seq(
      "DELETE",
      "FROM",
      field("table", $._name),
      field("where", optional($.where)),
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

    replace: $ => seq(
      "REPLACE",
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
      ");"
    ),

    _statement: $ => choice(
      $.stmt_select,
      $.insert,
      $.update,
      $.delete,
      $._declaration,
      $.drop_table,
      $.replace
    ),

    select: $ => seq(
      "SELECT",
      field("hints", repeat($._modifier)),
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

    quoted: $ => $.backtick,

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

    call: $ => prec(12, seq(
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

    neg: $ => prec(11, seq(
      "-",
      $._expression
    )),

    mul: $ => prec.left(10, seq(
      field("left", $._expression),
      "*",
      field("right", $._expression)
    )),

    add: $ => prec.left(9, seq(
      field("left", $._expression),
      "+",
      field("right", $._expression)
    )),

    sub: $ => prec.left(9, seq(
      field("left", $._expression),
      "-",
      field("right", $._expression)
    )),

    eq: $ => prec.left(8, seq(
      field("left", $._expression),
      "=",
      field("right", $._expression)
    )),

    lt: $ => prec.left(8, seq(
      field("left", $._expression),
      "<",
      field("right", $._expression)
    )),

    gt: $ => prec.left(8, seq(
      field("left", $._expression),
      ">",
      field("right", $._expression)
    )),

    like: $ => prec.left(8, seq(
      field("left", $._expression),
      "LIKE",
      field("right", $._expression)
    )),

    not: $ => prec(7, seq(
      "NOT",
      $._expression
    )),

    and: $ => prec.left(6, seq(
      field("left", $._expression),
      "AND",
      field("right", $._expression)
    )),

    or: $ => prec.left(5, seq(
      field("left", $._expression),
      "OR",
      field("right", $._expression)
    )),

    exp_bracket: $ => seq(
      "(",
      $._expression,
      ")"
    ),

    arrow: $ => prec.left(4, seq(
      field("left", $._expression),
      "->",
      field("right", $._expression)
    )),

    arrow_text: $ => prec.left(4, seq(
      field("left", $._expression),
      "->>",
      field("right", $._expression)
    )),

    over: $ => prec(2, seq(
      $._expression,
      "OVER",
      "(",
      field("partition", optional($.partition)),
      field("order", optional($.order_by)),
      ")"
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
      $.arrow,
      $.arrow_text,
      $.over
    ),

    limit: $ => seq(
      "LIMIT",
      field("count", $.int)
    ),

    offset: $ => seq(
      "OFFSET",
      field("start", $.int)
    ),

    ignore: $ => "IGNORE",

    on_duplicate_key: $ => seq(
      "ON",
      "DUPLICATE",
      "KEY",
      "UPDATE",
      seq(
        $._assignment,
        repeat(seq(
          ",",
          $._assignment
        ))
      )
    ),

    no_cache: $ => "SQL_NO_CACHE",

    _modifier: $ => choice(
      $.asc,
      $.desc,
      $.no_cache,
      $.ignore
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

    backtick: $ => /`(?:[^`])*`/,

    dqstring: $ => /"(?:[^"])*"/,

    int: $ => /(?:[0-9])+/,

    comment: $ => /--(?:[^\n\r])*/,

    comment_2: $ => /#(?:[^\n\r])*/,

    name: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    string: $ => /(?:'(?:(?:''|[^']))*'|(?:"(?:[^"])*"))/,

  },
});
