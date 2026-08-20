/**
 * treebank-sql: a from-scratch grammar for SQL, carrying the treebank
 * vocabulary (DESIGN.md §3) in its parse table.
 *
 * SQL is the first target here whose "versions" (DESIGN.md §4.2) are
 * DIALECTS rather than releases, and the union policy applies unchanged:
 * one grammar accepts SQLite, PostgreSQL and MySQL's shared surface plus
 * each one's cheap divergences (`REPLACE INTO`, `PRAGMA`, `USE`, `#`
 * comments, backquoted identifiers), and a construct only one of them has
 * is still accepted everywhere. What is deliberately NOT here is the
 * procedural half of the language — PL/pgSQL, PL/SQL, T-SQL blocks, stored
 * routine bodies — which is a different language wearing SQL's syntax, and
 * whose omission is why `_loop`, `_jump` and `_parameter` are not threaded.
 *
 * Threaded table-tier roles: see `supertypes` below. Omissions and the
 * reasons for them are in ledger.toml's roles_note.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank-core/vocabulary/supertypes.js');

// SQLite's operator table read upward, which is also PostgreSQL's and
// MySQL's for everything in it. Nothing here is tuned.
const PREC = {
  or: 1,
  and: 2,
  not: 3,
  compare: 4,      // = == != <> IS [NOT] LIKE GLOB REGEXP MATCH IN BETWEEN
  relational: 5,   // < <= > >=
  bitwise: 6,      // & | << >>
  additive: 7,
  multiplicative: 8,
  concat: 9,       // ||
  unary: 10,
  collate: 11,
  postfix: 12,     // IS NULL, IS NOT NULL
  primary: 13,
};

/**
 * A case-insensitive keyword token, aliased back to its canonical spelling
 * so the tree reads `SELECT` whatever the file wrote.
 *
 * The precedence is what keeps a keyword a keyword: `select` matches the
 * identifier token just as well, and at equal length the lexer takes the
 * higher precedence. It costs less than it looks like it should, because
 * tree-sitter's lexer only considers tokens VALID IN THE CURRENT STATE — so
 * a column named `key` still lexes as an identifier everywhere `KEY` is not
 * a token the parser could accept next. The words this does cost are listed
 * in ledger.toml under `deviations`.
 *
 * @param {string} word
 */
function kwToken(word) {
  const pattern = word
    .split('')
    .map((c) => (/[a-zA-Z]/.test(c) ? `[${c.toLowerCase()}${c.toUpperCase()}]` : c))
    .join('');
  return token(prec(1, new RegExp(pattern)));
}

function kw(word) {
  return alias(kwToken(word), word);
}

/** @param {RuleOrLiteral} rule */
function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}

/** @param {RuleOrLiteral} rule */
function commaSep(rule) {
  return optional(commaSep1(rule));
}

/** A parenthesised comma-separated list. @param {RuleOrLiteral} rule */
function parenList1(rule) {
  return seq('(', commaSep1(rule), ')');
}

module.exports = grammar({
  name: 'sql',

  word: $ => $.identifier,

  extras: $ => [$.comment, /\s/],

  // The words that may never be an identifier, and the smallest set that
  // does the job. tree-sitter's lexer only offers a token where the parse
  // state could accept it, which is usually a gift here — a column called
  // `key` or `text` lexes as an identifier everywhere those are not tokens
  // the parser could take next, so SQL's enormous keyword list costs almost
  // nothing. The exception is the position where BOTH readings are live:
  // after `SELECT`, an identifier is expected, so `SELECT FROM users`
  // lexed `FROM` as a column and `users` as its alias, and the grammar
  // accepted a statement no SQL engine will. Reserved words are what fix
  // that, and each one below is reserved in SQLite, PostgreSQL and MySQL
  // alike — no dialect lets a bare column be called `select` or `where`.
  //
  // `ON` is deliberately NOT here even though all three reserve it:
  // `PRAGMA foreign_keys = ON` puts it in value position, which is a real
  // form in real files, and losing it costs more than the widening
  // (`SELECT on FROM t` parses) is worth. Every word left off this list is
  // a word a column may be named after, which is the safe direction.
  reserved: {
    global: _ => [
      kwToken('SELECT'), kwToken('FROM'), kwToken('WHERE'), kwToken('GROUP'),
      kwToken('HAVING'), kwToken('ORDER'), kwToken('JOIN'),
      kwToken('UNION'), kwToken('INTERSECT'), kwToken('EXCEPT'),
      kwToken('INSERT'), kwToken('UPDATE'), kwToken('DELETE'),
      kwToken('CREATE'), kwToken('DROP'), kwToken('ALTER'), kwToken('TABLE'),
      kwToken('INTO'), kwToken('VALUES'), kwToken('SET'), kwToken('AND'),
      kwToken('OR'), kwToken('NOT'), kwToken('NULL'), kwToken('CASE'),
      kwToken('WHEN'), kwToken('THEN'), kwToken('ELSE'), kwToken('END'),
    ],
  },

  supertypes: $ => tb.assertTableTerms([
    '_statement',
    '_expression',
    '_declaration',
    '_type',
    '_name',
    '_literal',
    '_argument',
    '_member',
    // `_modifier` is demoted to the facet tier in this grammar. SQL's
    // modifiers are position-bound to the point of being a different
    // vocabulary per construct — `DISTINCT` may only follow SELECT,
    // `TEMPORARY` only CREATE, `ASC` only an ordering term, `NOT NULL` only
    // a column definition — so one alternation reachable from all four
    // positions accepts `SELECT TEMPORARY`, `CREATE DISTINCT TABLE` and
    // `ORDER BY x PRIMARY KEY`. See roles.json and DESIGN.md §3.1.1.
    ...tb.assertDemotable([]),
    '_directive',
    '_body',
    '_control_flow',
    '_branch',
    '_assignment',
    '_invocation',
    '_access',
  ]).map((name) => $[name]),

  conflicts: $ => [
    // `CREATE TABLE t (a, b)` — a body of two typeless columns, which
    // SQLite allows, or the column-name list of a `CREATE TABLE … AS
    // SELECT`. Nothing before the closing paren tells them apart; the `AS`
    // after it does.
    //
    // Suspected once of making the parser fork at every `INSERT INTO t (a,
    // b)` and measured innocent: removing it left a 5.5 MB mysql script at
    // exactly the same 45+ seconds. The cost was error recovery over a
    // string the grammar could not lex (see `string` below), not this.
    [$.column_name_list, $.column_definition],
  ],

  rules: {
    // A script, not a statement: the unit a file holds is a sequence of
    // statements separated by `;`, and a trailing separator is optional
    // because half the corpus writes it and half does not.
    program: $ => seq(
      optional($._statement),
      repeat(seq($._separator, optional($._statement))),
    ),

    // `;` everywhere, plus T-SQL's `GO`. `GO` is not SQL -- it is the batch
    // separator sqlcmd and SSMS understand, and no server ever sees it --
    // but it is what mssql `.sql` files are written with, and a grammar
    // that already takes `[bracketed]` identifiers should read the files
    // those identifiers come in.
    _separator: $ => choice(';', $.batch_separator),

    batch_separator: $ => kw('GO'),

    // ── statements ───────────────────────────────────────────────────
    _statement: $ => choice(
      $._declaration,
      $._directive,
      $.select_statement,
      $.values_clause,
      $.insert_statement,
      $.update_statement,
      $.delete_statement,
      $.drop_statement,
      $.alter_table_statement,
      $.truncate_statement,
      $.explain_statement,
      $.call_statement,
      $.begin_statement,
      $.commit_statement,
      $.rollback_statement,
      $.savepoint_statement,
      $.release_statement,
    ),

    // Introducing a named entity — and in SQL that is exactly the `CREATE`
    // family, all of which occur only at statement position. A column
    // definition is NOT here: it introduces a name too, but it occurs only
    // inside a table body, and one alternation reachable from both
    // positions would make `a INTEGER` a statement.
    _declaration: $ => choice(
      $.create_table_statement,
      $.create_view_statement,
      $.create_index_statement,
      $.create_trigger_statement,
      $.create_schema_statement,
    ),

    // Affects the environment rather than computing in it. Transaction
    // control is deliberately not here: `COMMIT` acts on the session's
    // work, not on the environment the rest of the script is read in.
    _directive: $ => choice(
      $.pragma_statement,
      $.set_statement,
      $.use_statement,
      $.show_statement,
    ),

    // ── select ───────────────────────────────────────────────────────
    // A bare `VALUES (1), (2)` is a query in its own right, but it is NOT
    // an alternative here: `INSERT INTO t VALUES (1)` would then read
    // equally well as the insert's own row list and as a one-clause
    // select_statement, and no precedence settles a reduce both readings
    // want. It is a statement of its own instead, which is also what it
    // looks like in a file.
    select_statement: $ => seq(
      optional($.with_clause),
      $._select_core,
      repeat($.compound_select),
      optional($.order_by_clause),
      optional($.limit_clause),
      optional($.offset_clause),
      optional($.locking_clause),
    ),

    _select_core: $ => seq(
      $.select_clause,
      optional($.from_clause),
      optional($.where_clause),
      optional($.group_by_clause),
      optional($.having_clause),
      optional($.window_clause),
    ),

    compound_select: $ => seq(
      field('operator', $.compound_operator),
      choice($._select_core, $.values_clause),
    ),

    compound_operator: $ => choice(
      seq(kw('UNION'), optional(choice($.all_modifier, $.distinct_modifier))),
      kw('INTERSECT'),
      kw('EXCEPT'),
    ),

    select_clause: $ => seq(
      kw('SELECT'),
      optional(choice($.distinct_modifier, $.all_modifier)),
      commaSep1($.result_column),
    ),

    result_column: $ => choice(
      $.star,
      $.qualified_star,
      seq(field('value', $._expression), optional($.alias)),
    ),

    star: _ => '*',

    qualified_star: $ => seq(field('qualifier', $._name), '.', $.star),

    // `AS x` and bare `x` are one node: the keyword is optional in every
    // dialect here, and a consumer asking "what is this called" should not
    // have to know which form was written.
    // The column-name list belongs to the ALIAS rather than to the
    // relation: `FROM t (a, b)` without one is indistinguishable from a
    // table-valued function call, and `FROM t AS x (a, b)` is the form
    // every dialect that has it actually writes.
    alias: $ => seq(
      optional(kw('AS')),
      field('name', $._name),
      optional($.column_name_list),
    ),

    from_clause: $ => seq(kw('FROM'), commaSep1($._relation)),

    // A comma in FROM is a join, so the list above is the cross-join form
    // and this is everything else.
    _relation: $ => choice($.relation, $.join_clause),

    relation: $ => seq(
      field(
        'name',
        choice(
          $._name,
          $.qualified_name,
          $.subquery,
          $.parenthesized_relation,
          // A table-valued function: `FROM generate_series(1, 10)`.
          $._invocation,
        ),
      ),
      optional($.alias),
    ),

    parenthesized_relation: $ => seq('(', $._relation, ')'),

    // `prec.right`, and the reason is the trailing condition. With
    // `prec.left` the reduce of the join won over shifting the `ON`, which
    // is invisible in a plain SELECT -- the `ON` has nowhere else to go, so
    // recovery lands on the right tree anyway -- and fatal inside `INSERT
    // INTO t (a, b) SELECT … JOIN u ON …`, where the insert's own `ON
    // CONFLICT` gives the reduce somewhere to go and the join's condition
    // is then left stranded outside the statement. Right nesting is not
    // what this buys: `a JOIN b ON p JOIN c ON q` still comes out as
    // `((a JOIN b ON p) JOIN c ON q)`, because the recursion direction is
    // decided by the fields, not by this.
    join_clause: $ => prec.right(seq(
      field('left', $._relation),
      optional($.join_type),
      kw('JOIN'),
      field('right', $._relation),
      optional(choice($.on_clause, $.using_clause)),
    )),

    // `NATURAL JOIN` on its own is a join type, so the qualifier after
    // NATURAL has to be optional rather than required.
    join_type: $ => choice(
      seq(kw('NATURAL'), optional($._join_kind)),
      $._join_kind,
    ),

    _join_kind: $ => choice(
      kw('CROSS'),
      seq(choice(kw('LEFT'), kw('RIGHT'), kw('FULL')), optional(kw('OUTER'))),
      kw('INNER'),
    ),

    on_clause: $ => seq(kw('ON'), field('condition', $._expression)),
    using_clause: $ => seq(kw('USING'), parenList1($._name)),

    where_clause: $ => seq(kw('WHERE'), field('condition', $._expression)),

    group_by_clause: $ => seq(kw('GROUP'), kw('BY'), commaSep1($._expression)),
    having_clause: $ => seq(kw('HAVING'), field('condition', $._expression)),

    window_clause: $ => seq(
      kw('WINDOW'),
      commaSep1(seq(field('name', $._name), kw('AS'), $.window_definition)),
    ),

    order_by_clause: $ => seq(kw('ORDER'), kw('BY'), commaSep1($.ordering_term)),

    ordering_term: $ => seq(
      field('value', $._expression),
      optional($.direction_modifier),
      optional($.nulls_modifier),
    ),

    // `LIMIT a, b` is MySQL's and SQLite's spelling of `LIMIT b OFFSET a`,
    // with the arguments the other way round. It is one alternative rather
    // than a rewrite: the tree says what the file said.
    limit_clause: $ => seq(
      kw('LIMIT'),
      field('value', $._expression),
      optional(seq(',', field('value', $._expression))),
    ),

    offset_clause: $ => seq(kw('OFFSET'), field('value', $._expression)),

    locking_clause: $ => seq(
      kw('FOR'),
      choice(kw('UPDATE'), kw('SHARE')),
      optional(seq(kw('OF'), commaSep1(field('name', choice($._name, $.qualified_name))))),
      optional(choice(kw('NOWAIT'), seq(kw('SKIP'), kw('LOCKED')))),
    ),

    // ── common table expressions ─────────────────────────────────────
    with_clause: $ => seq(
      kw('WITH'),
      optional($.recursive_modifier),
      commaSep1($.common_table_expression),
    ),

    common_table_expression: $ => seq(
      field('name', $._name),
      optional($.column_name_list),
      kw('AS'),
      optional($.materialized_modifier),
      '(',
      field('body', $._cte_body),
      ')',
    ),

    _cte_body: $ => choice(
      $.select_statement,
      $.insert_statement,
      $.update_statement,
      $.delete_statement,
      // `xaxis(x) AS (VALUES(-2.0) UNION ALL SELECT …)` -- a recursive CTE
      // seeded by a VALUES list, which is the idiomatic sqlite spelling.
      seq($.values_clause, repeat($.compound_select)),
    ),

    column_name_list: $ => parenList1($._name),

    // ── insert ───────────────────────────────────────────────────────
    insert_statement: $ => seq(
      optional($.with_clause),
      choice(
        seq(kw('INSERT'), optional(choice($.or_modifier, $.ignore_modifier))),
        kw('REPLACE'),
      ),
      kw('INTO'),
      field('name', choice($._name, $.qualified_name)),
      // No alias here, deliberately. `INSERT INTO t x (a, b)` cannot be
      // told from `INSERT INTO t AS x` followed by the insert's own column
      // list, and the column list is the form that matters — postgres's
      // `AS x` on an insert target exists only to be named by ON CONFLICT.
      optional($.column_name_list),
      choice(
        $.values_clause,
        $.select_statement,
        $.default_values_clause,
      ),
      optional(choice($.on_conflict_clause, $.on_duplicate_key_clause)),
      optional($.returning_clause),
    ),

    values_clause: $ => seq(
      choice(kw('VALUES'), kw('VALUE')),
      commaSep1($.value_row),
    ),

    value_row: $ => seq('(', commaSep($._expression), ')'),

    default_values_clause: $ => seq(kw('DEFAULT'), kw('VALUES')),

    on_conflict_clause: $ => seq(
      kw('ON'),
      kw('CONFLICT'),
      optional($.column_name_list),
      optional($.where_clause),
      kw('DO'),
      choice(
        kw('NOTHING'),
        seq(kw('UPDATE'), $.set_clause, optional($.where_clause)),
      ),
    ),

    // MySQL's spelling of the same thing.
    on_duplicate_key_clause: $ => seq(
      kw('ON'),
      kw('DUPLICATE'),
      kw('KEY'),
      kw('UPDATE'),
      commaSep1($._assignment),
    ),

    returning_clause: $ => seq(kw('RETURNING'), commaSep1($.result_column)),

    // ── update / delete ──────────────────────────────────────────────
    update_statement: $ => seq(
      optional($.with_clause),
      kw('UPDATE'),
      optional($.or_modifier),
      field('name', choice($._name, $.qualified_name)),
      optional($.alias),
      $.set_clause,
      optional($.from_clause),
      optional($.where_clause),
      optional($.returning_clause),
    ),

    set_clause: $ => seq(kw('SET'), commaSep1($._assignment)),

    delete_statement: $ => seq(
      optional($.with_clause),
      kw('DELETE'),
      kw('FROM'),
      field('name', choice($._name, $.qualified_name)),
      optional($.alias),
      optional($.delete_using_clause),
      optional($.where_clause),
      optional($.returning_clause),
    ),

    // Postgres's join-in-a-delete. Separate from `using_clause`, which is
    // the join's column list and a different construct wearing the same
    // keyword.
    delete_using_clause: $ => seq(kw('USING'), commaSep1($._relation)),

    // The one place SQL stores into a place, and the same node serves the
    // `SET` directive: `SET search_path = public` and `SET x = 1` in an
    // UPDATE are the same syntax doing the same thing to different places.
    _assignment: $ => $.assignment,

    // Two forms rather than one because the right sides differ: a single
    // column takes an expression, and the row form `SET (a, b) = (1, 2)`
    // takes a row. Spelling the row as `expression_list` only where a row
    // is the only legal thing keeps it out of expression position, where
    // `(1)` would be both a row of one and a parenthesised expression.
    assignment: $ => choice(
      seq(
        field('left', $._name),
        field('operator', choice('=', kw('TO'))),
        field('right', choice($._expression, $.default_modifier)),
      ),
      seq(
        field('left', $.column_name_list),
        field('operator', '='),
        field('right', choice($.expression_list, $.subquery)),
      ),
    ),

    // ── create ───────────────────────────────────────────────────────
    create_table_statement: $ => seq(
      kw('CREATE'),
      optional(choice($.temporary_modifier, $.virtual_modifier)),
      kw('TABLE'),
      optional($.if_not_exists_modifier),
      field('name', choice($._name, $.qualified_name)),
      choice(
        // The MySQL options go on this branch only. After `AS SELECT` a
        // trailing identifier is already a column alias, and an option list
        // there is the same tokens meaning something else.
        seq(field('body', $._body), optional($.table_options)),
        seq(optional($.column_name_list), kw('AS'), field('value', $.select_statement)),
        $.module_clause,
      ),
    ),

    // SQLite's virtual tables: `CREATE VIRTUAL TABLE t USING fts5(a, b,
    // tokenize = 'porter')`. The arguments are the MODULE's language, not
    // SQL's -- sqlite hands them to the module verbatim -- so they are taken
    // as expressions, which is what the common forms (a bare column name, a
    // `key = 'value'` pair) already are.
    module_clause: $ => seq(
      kw('USING'),
      field('name', $._name),
      optional(seq('(', commaSep($._expression), ')')),
    ),

    // MySQL's `) ENGINE = InnoDB DEFAULT CHARSET = utf8`.
    //
    // The `=` is REQUIRED even though MySQL allows `ENGINE InnoDB`, and the
    // corpus is why: without it `name value` matches any two words, so a
    // `CREATE TABLE …)` followed by an unterminated `alter table foo add`
    // — which is how the mssql scripts in openldap are written, with `GO`
    // rather than `;` as the separator — had its next statement eaten as an
    // option. Six files got worse for one form that nobody writes.
    table_options: $ => repeat1($.table_option),

    table_option: $ => choice(
      seq(field('name', $._name), '=', field('value', choice($._literal, $._name))),
      // SQLite's two, which take no value at all and so need no `=` to tell
      // them from a following statement. `WITHOUT ROWID` was the largest
      // gap cluster in the sweep at 56 files.
      seq(kw('WITHOUT'), kw('ROWID')),
      kw('STRICT'),
      // T-SQL's filegroup placement: `) ON [PRIMARY] TEXTIMAGE_ON
      // [PRIMARY]`. A keyword rather than an `=` separates these, which is
      // why they are alternatives here rather than ordinary options.
      seq(choice(kw('ON'), kw('TEXTIMAGE_ON')), field('value', choice($._name, $.qualified_name))),
    ),

    // The only body position SQL has. A view's defining query is a
    // `select_statement` reached through the `value:` field rather than
    // through `_body`, because a supertype covering both would make
    // `CREATE VIEW v (a INTEGER)` parse.
    _body: $ => $.table_body,

    // The comma is REQUIRED here even though SQLite makes it optional --
    // its grammar spells the separator as a rule that may be empty, and
    // chromium's generated schemas use that: `…, PRIMARY KEY(a, b) FOREIGN
    // KEY(c) REFERENCES t(d))`. Allowing it costs an ambiguity in the
    // commonest position in the language: with no comma to end a column
    // definition, `a CONSTRAINT …` is both a column with a named constraint
    // and a column followed by a table constraint, at every column of every
    // CREATE TABLE. Ten occurrences of generated schema is not worth that;
    // ledger.toml's gaps records it.
    // The comma is REQUIRED even though SQLite makes it optional, and
    // chromium's generated schemas use the optional form (`PRIMARY KEY(a,
    // b) FOREIGN KEY(c)`, 10 files). Tried twice and rejected both times,
    // the second time on better grounds than the first: it is not the fork
    // that costs -- the mysql escape showed forking was never the expense
    // -- it is that the ambiguity is GENUINE. With no comma to end a column
    // definition, `a INT CONSTRAINT c PRIMARY KEY` is both a column
    // carrying a named constraint and a column followed by a table
    // constraint, and nothing in the text decides. A tree that varies on
    // every named column constraint in the corpus is worse than ten
    // generated files that do not parse.
    table_body: $ => seq('(', commaSep1($._member), ')'),

    _member: $ => choice($.column_definition, $.table_constraint),

    column_definition: $ => seq(
      field('name', $._name),
      optional(field('type', $._type)),
      repeat($.column_constraint),
    ),

    column_constraint: $ => seq(
      optional(seq(kw('CONSTRAINT'), field('name', $._name))),
      choice(
        seq(
          kw('PRIMARY'),
          kw('KEY'),
          optional(choice(kw('ASC'), kw('DESC'))),
          optional($.conflict_clause),
          optional(kw('AUTOINCREMENT')),
        ),
        seq(kw('NOT'), kw('NULL'), optional($.conflict_clause)),
        kw('NULL'),
        seq(kw('UNIQUE'), optional($.conflict_clause)),
        kw('AUTO_INCREMENT'),
        seq(kw('CHECK'), '(', field('condition', $._expression), ')'),
        // NOT `_expression`. `argument TEXT DEFAULT '' NOT NULL` is the
        // shape half the corpus writes, and with a full expression here the
        // parser reads `'' NOT LIKE …` and never gets to the NOT NULL that
        // follows. This is SQLite's own rule for the position — a literal, a
        // signed number, a call, or a parenthesised expression — and it is
        // the standard's too.
        seq(kw('DEFAULT'), field('value', $._default_value)),
        seq(kw('COLLATE'), field('value', $._name)),
        seq(
          optional(seq(kw('GENERATED'), kw('ALWAYS'))),
          kw('AS'),
          '(',
          field('value', $._expression),
          ')',
          optional(choice(kw('STORED'), kw('VIRTUAL'))),
        ),
        $.references_clause,
      ),
    ),

    _default_value: $ => choice(
      $._literal,
      $._name,
      $._invocation,
      $.parenthesized_expression,
      $.signed_number,
    ),

    signed_number: $ => seq(choice('-', '+'), $.number),

    // SQLite's `… PRIMARY KEY ON CONFLICT REPLACE`: what to do when this
    // constraint is the one that fails. Not the INSERT clause of the same
    // name, which is a different construct with the same two keywords.
    conflict_clause: $ => seq(
      kw('ON'),
      kw('CONFLICT'),
      choice(kw('ROLLBACK'), kw('ABORT'), kw('FAIL'), kw('IGNORE'), kw('REPLACE')),
    ),

    table_constraint: $ => seq(
      optional(seq(kw('CONSTRAINT'), field('name', $._name))),
      choice(
        seq(kw('PRIMARY'), kw('KEY'), parenList1($.indexed_column)),
        seq(kw('UNIQUE'), parenList1($.indexed_column)),
        seq(kw('CHECK'), '(', field('condition', $._expression), ')'),
        seq(
          kw('FOREIGN'),
          kw('KEY'),
          parenList1($._name),
          $.references_clause,
        ),
      ),
    ),

    references_clause: $ => seq(
      kw('REFERENCES'),
      field('name', choice($._name, $.qualified_name)),
      optional(parenList1($._name)),
      repeat($.reference_action),
    ),

    reference_action: $ => seq(
      kw('ON'),
      choice(kw('DELETE'), kw('UPDATE')),
      choice(
        seq(kw('SET'), choice(kw('NULL'), kw('DEFAULT'))),
        kw('CASCADE'),
        kw('RESTRICT'),
        seq(kw('NO'), kw('ACTION')),
      ),
    ),

    indexed_column: $ => seq(
      field('value', $._expression),
      optional($.direction_modifier),
    ),

    create_view_statement: $ => seq(
      kw('CREATE'),
      optional($.or_replace_modifier),
      optional($.temporary_modifier),
      // MySQL's view attributes, and the largest cluster the MySQL oracle
      // made visible at 96 files -- the whole of mysql's own sys_schema is
      // written with them. They were invisible before because SQLite
      // rejects the files too, so every one of them booked as noise.
      repeat($.view_attribute),
      kw('VIEW'),
      optional($.if_not_exists_modifier),
      field('name', choice($._name, $.qualified_name)),
      optional($.column_name_list),
      kw('AS'),
      field('value', $.select_statement),
      optional($.check_option_clause),
    ),

    view_attribute: $ => choice(
      seq(kw('ALGORITHM'), '=', field('value', choice(kw('UNDEFINED'), kw('MERGE'), kw('TEMPTABLE')))),
      seq(kw('DEFINER'), '=', field('value', $.user_name)),
      seq(kw('SQL'), kw('SECURITY'), field('value', choice(kw('DEFINER'), kw('INVOKER')))),
    ),

    // `'root'@'localhost'`, `root@localhost`, or `CURRENT_USER`. The `@` is
    // a plain token here and does not collide with a bind parameter:
    // `bind_parameter` requires a letter straight after the `@`, so
    // `@'localhost'` cannot match it.
    user_name: $ => choice(
      seq(
        field('name', choice($.string, $._name)),
        optional(seq('@', field('host', choice($.string, $._name)))),
      ),
      kw('CURRENT_USER'),
    ),

    check_option_clause: $ => seq(
      kw('WITH'),
      optional(choice(kw('CASCADED'), kw('LOCAL'))),
      kw('CHECK'),
      kw('OPTION'),
    ),

    create_index_statement: $ => seq(
      kw('CREATE'),
      optional($.unique_modifier),
      kw('INDEX'),
      optional($.if_not_exists_modifier),
      field('name', choice($._name, $.qualified_name)),
      kw('ON'),
      field('value', choice($._name, $.qualified_name)),
      parenList1($.indexed_column),
      optional($.where_clause),
      // T-SQL again: `CREATE INDEX i ON [dbo].[t]([c]) ON [PRIMARY]`. The
      // same filegroup placement a table takes, reusing the same node.
      optional($.table_options),
    ),

    // A trigger whose body is a plain statement list, which is SQLite's,
    // MySQL's simple form and the standard's. This is NOT the procedural
    // gap ledger.toml describes: there is no control flow, no variable and
    // no declaration inside -- just INSERT/UPDATE/DELETE/SELECT, each
    // terminated by `;`. PL/pgSQL's `DO $$ … $$` and a routine body remain
    // out of scope and are still pinned by test/negative.
    create_trigger_statement: $ => seq(
      kw('CREATE'),
      optional($.temporary_modifier),
      kw('TRIGGER'),
      optional($.if_not_exists_modifier),
      field('name', choice($._name, $.qualified_name)),
      optional(choice(kw('BEFORE'), kw('AFTER'), seq(kw('INSTEAD'), kw('OF')))),
      field('event', $.trigger_event),
      kw('ON'),
      field('value', choice($._name, $.qualified_name)),
      optional(seq(kw('FOR'), kw('EACH'), kw('ROW'))),
      optional($.when_condition_clause),
      field('body', $.trigger_body),
    ),

    trigger_event: $ => choice(
      kw('DELETE'),
      kw('INSERT'),
      seq(kw('UPDATE'), optional(seq(kw('OF'), commaSep1($._name)))),
    ),

    // `WHEN <expr>` on a trigger. A different construct from CASE's
    // `when_clause`, which carries a THEN, so it gets its own node rather
    // than a looser shared one.
    when_condition_clause: $ => seq(kw('WHEN'), field('condition', $._expression)),

    // The body's statements are a RESTRICTED set, and that is what keeps
    // `END` unambiguous: `commit_statement` is spelled `COMMIT` or `END`,
    // so admitting every statement here would make the `END` that closes
    // the body indistinguishable from a transaction statement inside it.
    trigger_body: $ => seq(
      kw('BEGIN'),
      repeat1(seq($._trigger_statement, ';')),
      kw('END'),
    ),

    _trigger_statement: $ => choice(
      $.select_statement,
      $.insert_statement,
      $.update_statement,
      $.delete_statement,
      $.values_clause,
    ),

    create_schema_statement: $ => seq(
      kw('CREATE'),
      choice(kw('SCHEMA'), kw('DATABASE')),
      optional($.if_not_exists_modifier),
      field('name', choice($._name, $.qualified_name)),
    ),

    // ── drop / alter / truncate ──────────────────────────────────────
    drop_statement: $ => seq(
      kw('DROP'),
      choice(kw('TABLE'), kw('VIEW'), kw('INDEX'), kw('TRIGGER'), kw('SCHEMA'), kw('DATABASE')),
      optional($.if_exists_modifier),
      commaSep1(field('name', choice($._name, $.qualified_name))),
      optional(choice(kw('CASCADE'), kw('RESTRICT'))),
    ),

    alter_table_statement: $ => seq(
      kw('ALTER'),
      kw('TABLE'),
      optional($.if_exists_modifier),
      field('name', choice($._name, $.qualified_name)),
      // T-SQL's `ALTER TABLE t WITH NOCHECK ADD CONSTRAINT …`: whether to
      // validate existing rows against the constraint being added.
      optional($.check_modifier),
      commaSep1(choice($.add_clause, $.drop_column_clause, $.rename_clause)),
    ),

    // `ADD` takes a `_member`, not a column definition: `ALTER TABLE t ADD
    // CONSTRAINT pk PRIMARY KEY (a, b)` adds an element to the table's body
    // exactly as `CREATE TABLE` does, and it is the same alternation in both
    // places rather than two lists that can drift apart.
    add_clause: $ => seq(
      kw('ADD'),
      optional(kw('COLUMN')),
      optional($.if_not_exists_modifier),
      $._member,
    ),

    drop_column_clause: $ => seq(
      kw('DROP'),
      optional(kw('COLUMN')),
      optional($.if_exists_modifier),
      field('name', $._name),
    ),

    rename_clause: $ => seq(
      kw('RENAME'),
      choice(
        seq(kw('TO'), field('name', $._name)),
        seq(
          optional(kw('COLUMN')),
          field('value', $._name),
          kw('TO'),
          field('name', $._name),
        ),
      ),
    ),

    truncate_statement: $ => seq(
      kw('TRUNCATE'),
      optional(kw('TABLE')),
      commaSep1(field('name', choice($._name, $.qualified_name))),
    ),

    // ── directives and transactions ──────────────────────────────────
    pragma_statement: $ => seq(
      kw('PRAGMA'),
      field('name', choice($._name, $.qualified_name)),
      optional(choice(
        seq('=', field('value', $._expression)),
        seq('(', field('value', $._expression), ')'),
      )),
    ),

    set_statement: $ => seq(
      kw('SET'),
      optional(choice(kw('SESSION'), kw('LOCAL'), kw('GLOBAL'))),
      commaSep1($._assignment),
    ),

    use_statement: $ => seq(kw('USE'), field('name', $._name)),

    // `SHOW block_size`, `SHOW ALL`, `SHOW TIME ZONE`. Postgres's and
    // MySQL's way of reading a setting, and the largest single gap cluster
    // in the sweep at 72 occurrences -- which is the evidence the earlier
    // ledger entry said it was waiting for before picking members out of an
    // open-ended list of utility statements.
    show_statement: $ => seq(kw('SHOW'), field('name', repeat1(choice($._name, $.qualified_name)))),

    begin_statement: $ => seq(
      choice(kw('BEGIN'), seq(kw('START'), kw('TRANSACTION'))),
      optional(choice(kw('DEFERRED'), kw('IMMEDIATE'), kw('EXCLUSIVE'))),
      optional(choice(kw('TRANSACTION'), kw('WORK'))),
    ),

    commit_statement: $ => seq(
      choice(kw('COMMIT'), kw('END')),
      optional(choice(kw('TRANSACTION'), kw('WORK'))),
    ),

    rollback_statement: $ => seq(
      kw('ROLLBACK'),
      optional(choice(kw('TRANSACTION'), kw('WORK'))),
      optional(seq(kw('TO'), optional(kw('SAVEPOINT')), field('name', $._name))),
    ),

    savepoint_statement: $ => seq(kw('SAVEPOINT'), field('name', $._name)),

    release_statement: $ => seq(
      kw('RELEASE'),
      optional(kw('SAVEPOINT')),
      field('name', $._name),
    ),

    explain_statement: $ => seq(
      kw('EXPLAIN'),
      optional(choice(seq(kw('QUERY'), kw('PLAN')), kw('ANALYZE'))),
      $._statement,
    ),

    call_statement: $ => seq(kw('CALL'), $._invocation),

    // ── expressions ──────────────────────────────────────────────────
    _expression: $ => choice(
      $._literal,
      $._name,
      $._access,
      $._invocation,
      $._control_flow,
      $.bind_parameter,
      $.parenthesized_expression,
      $.subquery,
      $.binary_expression,
      $.unary_expression,
      $.between_expression,
      $.in_expression,
      $.is_expression,
      $.null_test,
      $.exists_expression,
      $.cast_expression,
      $.collate_expression,
      $.extract_expression,
      $.array_constructor,
      $.quantified_subquery,
    ),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    // A `(SELECT …)` — scalar, row-valued or table-valued depending on
    // where it sits, which is a question for the layer above the grammar.
    subquery: $ => seq('(', $.select_statement, ')'),

    binary_expression: $ => {
      const table = [
        [PREC.or, choice(kw('OR'), '||')],
        [PREC.and, kw('AND')],
        [PREC.compare, choice('=', '==', '!=', '<>', kw('LIKE'), kw('GLOB'), kw('REGEXP'), kw('MATCH'), seq(kw('NOT'), kw('LIKE')))],
        [PREC.relational, choice('<', '<=', '>', '>=')],
        [PREC.bitwise, choice('&', '|', '<<', '>>')],
        [PREC.additive, choice('+', '-')],
        [PREC.multiplicative, choice('*', '/', '%')],
      ];
      return choice(...table.map(([precedence, operator]) => prec.left(
        Number(precedence),
        seq(
          field('left', $._expression),
          field('operator', operator),
          field('right', $._expression),
        ),
      )));
    },

    unary_expression: $ => prec(PREC.unary, seq(
      field('operator', choice('-', '+', '~', kw('NOT'))),
      field('value', $._expression),
    )),

    between_expression: $ => prec.left(PREC.compare, seq(
      field('value', $._expression),
      optional(kw('NOT')),
      kw('BETWEEN'),
      field('left', $._expression),
      kw('AND'),
      field('right', $._expression),
    )),

    in_expression: $ => prec.left(PREC.compare, seq(
      field('value', $._expression),
      optional(kw('NOT')),
      kw('IN'),
      field('right', choice(
        $.subquery,
        $.expression_list,
        $._name,
        $.qualified_name,
      )),
    )),

    expression_list: $ => seq('(', commaSep($._expression), ')'),

    // `a = ANY (SELECT …)`. The quantifier belongs to the subquery rather
    // than to the operator, which is how the standard reads it and what
    // keeps the operator ladder a ladder.
    // `ALL` is deliberately not a quantifier here: it is already the
    // modifier after SELECT, and `SELECT ALL (…)` is then two readings of
    // the same bytes with nothing to settle them. `= ALL (subquery)` is in
    // ledger.toml's gaps; `= ANY (subquery)`, which is the form that
    // appears, is not affected.
    quantified_subquery: $ => seq(
      choice(kw('ANY'), kw('SOME')),
      field('value', $.subquery),
    ),

    // Postgres's `ARRAY[1, 2, 3]`.
    array_constructor: $ => seq(kw('ARRAY'), '[', commaSep($._expression), ']'),

    // `IS`, `IS NOT`, and `IS [NOT] DISTINCT FROM` in one rule: they are
    // one operator whose right side may be a keyword rather than a value.
    is_expression: $ => prec.left(PREC.postfix, seq(
      field('left', $._expression),
      kw('IS'),
      optional(kw('NOT')),
      optional(seq(kw('DISTINCT'), kw('FROM'))),
      field('right', $._expression),
    )),

    // `NOT EXISTS` is not spelled here: `NOT` is the unary operator it
    // already is everywhere else, and giving this rule its own optional one
    // made every `NOT EXISTS (…)` two readings of the same bytes.
    // SQLite's postfix null predicates: `WHERE keyword NOT NULL` means `IS
    // NOT NULL`, and `ISNULL`/`NOTNULL` are the one-word spellings of the
    // same thing. Postfix rather than an `is_expression` because there is
    // no right operand to give it.
    // `NOT NULL` is ONE token here, whitespace included. As two it is
    // ambiguous with everything else `NOT` starts in expression position --
    // the unary operator, `NOT LIKE`, `NOT IN`, `NOT BETWEEN` -- and the
    // generator says so. As one token the lexer settles it, and the
    // column-constraint spelling is unaffected because the combined token
    // is not valid in that state.
    null_test: $ => prec.left(PREC.postfix, seq(
      field('value', $._expression),
      choice($._not_null, kw('ISNULL')),
    )),

    // prec 2, above the 1 every keyword token carries: tree-sitter's lexer
    // compares precedence before length, so at default precedence the
    // three-character `NOT` beat the eight-character `NOT NULL` and this
    // rule never fired.
    _not_null: _ => token(prec(2, seq(/[Nn][Oo][Tt]/, /[ \t\r\n]+/, /[Nn][Uu][Ll][Ll]/))),

    // `NOTNULL`, the one-word spelling, is NOT here. It resisted the same
    // token lift that made `NOT NULL` and `ISNULL` work, and at 12
    // occurrences in the corpus and zero gap files it did not earn more
    // digging. Shipping a rule that cannot be demonstrated is worse than
    // omitting one.

    exists_expression: $ => seq(
      kw('EXISTS'),
      field('value', $.subquery),
    ),

    // Two spellings of one node: the standard's `CAST(x AS t)` and
    // postgres's `x::t`, which is the form most real postgres writes.
    cast_expression: $ => choice(
      seq(
        kw('CAST'),
        '(',
        field('value', $._expression),
        kw('AS'),
        field('type', $._type),
        ')',
      ),
      prec.left(PREC.collate, seq(
        field('value', $._expression),
        '::',
        field('type', $._type),
      )),
    ),

    // `EXTRACT(YEAR FROM ts)` is the standard's, and the one function-like
    // form whose arguments are not a comma-separated list. `SUBSTRING(x
    // FROM 1 FOR 2)`, `TRIM(BOTH ' ' FROM x)` and `POSITION(x IN y)` are the
    // same shape and are NOT here; they are in ledger.toml's gaps.
    extract_expression: $ => seq(
      kw('EXTRACT'),
      '(',
      field('name', $._name),
      kw('FROM'),
      field('value', $._expression),
      ')',
    ),

    collate_expression: $ => prec(PREC.collate, seq(
      field('value', $._expression),
      kw('COLLATE'),
      field('name', $._name),
    )),

    // ── control flow ─────────────────────────────────────────────────
    // SQL's one conditional. It is an EXPRESSION, so `_control_flow` nests
    // inside `_expression` here the way it nests inside `_statement` in
    // python — which is the whole point of the vocabulary.
    _control_flow: $ => $._branch,
    _branch: $ => $.case_expression,

    case_expression: $ => prec.right(seq(
      kw('CASE'),
      optional(field('value', $._expression)),
      repeat1($.when_clause),
      optional($.else_clause),
      kw('END'),
    )),

    when_clause: $ => seq(
      kw('WHEN'),
      field('condition', $._expression),
      kw('THEN'),
      field('value', $._expression),
    ),

    else_clause: $ => seq(kw('ELSE'), field('value', $._expression)),

    // ── invocation and access ────────────────────────────────────────
    _invocation: $ => $.function_call,

    function_call: $ => prec(PREC.primary, seq(
      field('name', choice($._name, $.qualified_name)),
      '(',
      optional(choice(kw('DISTINCT'), kw('ALL'))),
      field('arguments', commaSep($._argument)),
      optional($.order_by_clause),
      ')',
      optional($.filter_clause),
      optional($.over_clause),
    )),

    // A positional argument is a bare expression and threads through, the
    // way DESIGN.md §3.2 says it does; `*` is the one argument form SQL
    // gives a syntax of its own.
    _argument: $ => choice($._expression, $.star),

    filter_clause: $ => seq(kw('FILTER'), '(', $.where_clause, ')'),

    over_clause: $ => seq(
      kw('OVER'),
      choice(field('name', $._name), $.window_definition),
    ),

    window_definition: $ => seq(
      '(',
      optional(field('name', $._name)),
      optional($.partition_by_clause),
      optional($.order_by_clause),
      optional($.frame_clause),
      ')',
    ),

    partition_by_clause: $ => seq(kw('PARTITION'), kw('BY'), commaSep1($._expression)),

    frame_clause: $ => seq(
      choice(kw('RANGE'), kw('ROWS'), kw('GROUPS')),
      choice(
        $._frame_bound,
        seq(kw('BETWEEN'), $._frame_bound, kw('AND'), $._frame_bound),
      ),
      optional(seq(
        kw('EXCLUDE'),
        choice(
          seq(kw('NO'), kw('OTHERS')),
          seq(kw('CURRENT'), kw('ROW')),
          kw('GROUP'),
          kw('TIES'),
        ),
      )),
    ),

    _frame_bound: $ => choice(
      seq(kw('UNBOUNDED'), choice(kw('PRECEDING'), kw('FOLLOWING'))),
      seq(kw('CURRENT'), kw('ROW')),
      seq($._expression, choice(kw('PRECEDING'), kw('FOLLOWING'))),
    ),

    // Reading a place: `t.c`, `s.t.c`, `s.t`. One node for every dotted
    // form, in every position, because `schema.table` in FROM and
    // `table.column` in WHERE are the same syntax and telling them apart is
    // name resolution's job, not the parser's.
    _access: $ => choice($.qualified_name, $.subscript),

    // Postgres's array element, and the only INDEX read SQL has. Its
    // brackets are not the T-SQL identifier quoting `[name]`, which this
    // grammar does not accept for exactly this reason: one of the two has
    // to lose, and an array read is the one that appears inside
    // expressions.
    subscript: $ => prec(PREC.primary, seq(
      field('value', $._expression),
      '[',
      field('index', $._expression),
      ']',
    )),

    qualified_name: $ => prec.left(PREC.primary, seq(
      field('value', choice($._name, $.qualified_name)),
      '.',
      field('name', $._name),
    )),

    // ── names, types and literals ────────────────────────────────────
    _name: $ => choice($.identifier, $.quoted_identifier),

    identifier: _ => token(/[a-zA-Z_][a-zA-Z_0-9$]*/),

    // Double quotes are the standard's identifier quoting, backquotes are
    // MySQL's and brackets are T-SQL's. Doubling the quote escapes it in the
    // first two.
    //
    // The brackets were left out at first because `[a]` and the array
    // subscript `a[1]` are the same two characters, and one of them had to
    // lose. Neither does: tree-sitter's lexer only offers a token the parse
    // state can accept, and after an expression the state wants `[` for a
    // subscript while at the head of a name it wants an identifier -- so the
    // two never compete. The corpus settled the priority anyway (7 files with
    // `[dbo].[t]`, none with an array subscript).
    quoted_identifier: _ => token(choice(
      seq('"', repeat(choice(/[^"]/, '""')), '"'),
      seq('`', repeat(choice(/[^`]/, '``')), '`'),
      seq('[', repeat(/[^\]]/), ']'),
    )),

    // A type is a run of words plus optional arguments: `INTEGER`,
    // `VARCHAR(255)`, `DOUBLE PRECISION`, `TIMESTAMP WITH TIME ZONE`,
    // `NUMERIC(10, 2)`, `TEXT[]`. Spelling it as a word run rather than a
    // list of type keywords is what keeps `INTEGER` and `TEXT` out of the
    // reserved set, so a column may still be called `text`.
    _type: $ => $.type_name,

    type_name: $ => prec.right(seq(
      field('name', repeat1($._name)),
      optional(seq('(', commaSep1($._expression), ')')),
      // `timestamp(2) without time zone` — the standard's one type suffix
      // that comes AFTER the precision, so the word run above cannot pick it
      // up. Spelled out rather than allowed as a trailing run of names,
      // which would also accept `INT b` as a type.
      optional(seq(choice(kw('WITH'), kw('WITHOUT')), kw('TIME'), kw('ZONE'))),
      repeat(seq('[', optional($.number), ']')),
    )),

    _literal: $ => choice(
      $.number,
      $.string,
      $.typed_literal,
      $.blob,
      $.boolean_literal,
      $.null_literal,
    ),

    // `DATE '2024-01-01'`, `INTERVAL '1 day'`, `TIMESTAMP '…'`. A literal
    // and not a cast: the value is still fully determined by its own text,
    // which is the `_literal` test in DESIGN.md §3.2.
    // The type is one word rather than `_type`: a full type at expression
    // position collides with the array subscript (`a[1]` against `a[]`) and
    // with a parenthesised call (`t(1)` against `t(1) '…'`). Every typed
    // literal that appears in practice is one word; `TIMESTAMP WITH TIME
    // ZONE '…'` is in ledger.toml's gaps.
    typed_literal: $ => seq(field('type', $._name), field('value', $.string)),

    number: _ => token(choice(
      /[0-9]+(\.[0-9]*)?([eE][+-]?[0-9]+)?/,
      /\.[0-9]+([eE][+-]?[0-9]+)?/,
      /0[xX][0-9a-fA-F]+/,
    )),

    // Single quotes. Two escapes: the standard's doubled `''`, and MySQL's
    // backslash form, which PostgreSQL also accepts with
    // standard_conforming_strings off.
    //
    // The backslash was not here at first, and the cost of leaving it out
    // was not a rejected file -- it was TIME. `UNIT["Clarke\'s link"]`
    // inside mysql's own `mysql_system_tables_data_fix.sql` closed the
    // string early, and the rest of the 5.5 MB file went through error
    // recovery: over 45 seconds for 5,237 statements, against 10 seconds
    // for a 67 MB dump with no apostrophe in it. Error recovery is what a
    // wrong lexer costs, and it does not look like a lexer bug from the
    // outside.
    //
    // What it widens, stated because it is a real dialect divergence:
    // backslash is NOT an escape in the standard, in SQLite, or in modern
    // postgres, so `'C:\'` is a complete string there and an unterminated
    // one here. Measured on the corpus, that trade is worth taking; the
    // reverse costs every mysql dump carrying an apostrophe.
    //
    // Interpolation is still absent either way, so every instance of this
    // rule is fully determined by its own text and the `_literal` test in
    // DESIGN.md §3.2 is satisfied for all of them.
    string: _ => token(seq("'", repeat(choice(/[^'\\]/, "''", /\\./)), "'")),

    blob: _ => token(seq(/[xX]/, "'", repeat(/[^']/), "'")),

    boolean_literal: $ => choice(kw('TRUE'), kw('FALSE')),
    null_literal: $ => kw('NULL'),

    // `?`, `?1`, `:name`, `@name`, `$1` — every dialect's placeholder. A
    // value the statement does not carry, which is why it is an expression
    // leaf and not a `_literal`.
    bind_parameter: _ => token(choice(
      /\?[0-9]*/,
      /[:@$][a-zA-Z_][a-zA-Z_0-9]*/,
      /\$[0-9]+/,
    )),

    // ── modifiers (facet tier — see roles.json) ──────────────────────
    distinct_modifier: $ => kw('DISTINCT'),
    all_modifier: $ => kw('ALL'),
    unique_modifier: $ => kw('UNIQUE'),
    virtual_modifier: $ => kw('VIRTUAL'),
    check_modifier: $ => seq(kw('WITH'), choice(kw('CHECK'), kw('NOCHECK'))),
    temporary_modifier: $ => choice(kw('TEMPORARY'), kw('TEMP')),
    recursive_modifier: $ => kw('RECURSIVE'),
    materialized_modifier: $ => seq(optional(kw('NOT')), kw('MATERIALIZED')),
    if_not_exists_modifier: $ => seq(kw('IF'), kw('NOT'), kw('EXISTS')),
    if_exists_modifier: $ => seq(kw('IF'), kw('EXISTS')),
    or_replace_modifier: $ => seq(kw('OR'), kw('REPLACE')),
    or_modifier: $ => seq(kw('OR'), choice(kw('ROLLBACK'), kw('ABORT'), kw('FAIL'), kw('IGNORE'), kw('REPLACE'))),
    ignore_modifier: $ => kw('IGNORE'),
    default_modifier: $ => kw('DEFAULT'),
    direction_modifier: $ => choice(kw('ASC'), kw('DESC')),
    nulls_modifier: $ => seq(kw('NULLS'), choice(kw('FIRST'), kw('LAST'))),

    // ── comments ─────────────────────────────────────────────────────
    // `--` is the standard's, `/* */` is every dialect's, `#` is MySQL's.
    comment: _ => token(choice(
      seq('--', /[^\r\n]*/),
      seq('#', /[^\r\n]*/),
      seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'),
    )),
  },
});
