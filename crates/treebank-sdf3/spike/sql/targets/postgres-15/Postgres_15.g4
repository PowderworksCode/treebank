// GENERATED from postgres/15.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Postgres_15;

options { caseInsensitive = true; }

script
    : statement* EOF
    ;

statement
    :     (with_=with)?  select  ';'  # stmt_select
    |     'INSERT'  'INTO'  table=name  '('  columns+=name (',' columns+=name)*  ')'  'VALUES'  '('  values+=expression (',' values+=expression)*  ')'  (upsert_=upsert)?  (returning_=returning)?  ';'  # insert
    |     'UPDATE'  table=name  'SET'  assign (',' assign)*  (where_=where)?  (returning_=returning)?  ';'  # update
    |     'DELETE'  'FROM'  table=name  (where_=where)?  (returning_=returning)?  ';'  # delete
    |     'CREATE'  'TABLE'  table=name  '('  col_def (',' col_def)*  ')'  (tail=without_oids)?  ';'  # create_table
    |     'DROP'  'TABLE'  table=name  ';'  # drop_table
    |     'MERGE'  'INTO'  target=name  'USING'  source=name  'ON'  expression  when+  ';'  # merge
    ;

select
    : 'SELECT'  items+=item (',' items+=item)*  (from_=from)?  (where_=where)?  (order_=order_by)?  (limit_=limit)?  (offset_=offset)?
    ;

item
    : expression  (alias_=alias)?
    ;

alias
    :     'AS'  name  # as
    |     name  # bare
    ;

from
    : 'FROM'  table=name
    ;

where
    : 'WHERE'  expression
    ;

order_by
    : 'ORDER'  'BY'  order (',' order)*
    ;

order
    : expression  (dir_=dir)?
    ;

dir
    :     'ASC'  # asc
    |     'DESC'  # desc
    ;

cte
    : name_=name  'AS'  '('  select  ')'
    ;

assign
    : column=name  '='  value=expression
    ;

col_def
    : name_=name  type
    ;

type
    :     'INT'  # type_int
    |     'VARCHAR'  '('  INT  ')'  # varchar
    |     'TEXT'  # text
    ;

name
    :     NAME  # ident_name
    |     DQUOTED  # quoted
    ;

expression
    :     function=NAME  '('  (arguments+=expression (',' arguments+=expression)*)?  ')'  # call
    |     '-'  expression  # neg
    |     left=expression  '*'  right=expression  # mul
    |     left=expression  '+'  right=expression  # add
    |     left=expression  '-'  right=expression  # sub
    |     left=expression  '='  right=expression  # eq
    |     left=expression  '<'  right=expression  # lt
    |     left=expression  '>'  right=expression  # gt
    |     left=expression  'LIKE'  right=expression  # like
    |     'NOT'  expression  # not
    |     left=expression  'AND'  right=expression  # and
    |     left=expression  'OR'  right=expression  # or
    |     expression  'OVER'  '('  (partition_=partition)?  (order_=order_by)?  ')'  # over
    |     left=expression  '->'  right=expression  # arrow
    |     left=expression  '->>'  right=expression  # arrow_text
    |     expression  '::'  type  # cast
    |     left=expression  'ILIKE'  right=expression  # i_like
    |     name  # inj_exp_1
    |     table=name  '.'  column=name  # column
    |     '*'  # star
    |     INT  # exp_int
    |     STRING  # str
    |     'NULL'  # null
    |     '('  expression  ')'  # exp_bracket
    ;

limit
    : 'LIMIT'  count=INT
    ;

offset
    : 'OFFSET'  start=INT
    ;

with
    : 'WITH'  cte (',' cte)*
    ;

partition
    : 'PARTITION'  'BY'  expression (',' expression)*
    ;

returning
    : 'RETURNING'  item (',' item)*
    ;

without_oids
    : 'WITHOUT'  'OIDS'
    ;

upsert
    :     'ON'  'CONFLICT'  'DO'  'NOTHING'  # nothing
    |     'ON'  'CONFLICT'  '('  name (',' name)*  ')'  'DO'  'UPDATE'  'SET'  assign (',' assign)*  # upsert_update
    ;

when
    :     'WHEN'  'MATCHED'  'THEN'  'UPDATE'  'SET'  assign (',' assign)*  # matched
    |     'WHEN'  'NOT'  'MATCHED'  'THEN'  'INSERT'  '('  columns+=name (',' columns+=name)*  ')'  'VALUES'  '('  values+=expression (',' values+=expression)*  ')'  # not_matched
    ;

fragment DOLLAR : '$$' (~[$])* '$$' ;
DQUOTED : '"' (~["])* '"' ;
INT : ([0-9])+ ;
WS1 : [ \t\n\r]+ -> channel(HIDDEN) ;
COMMENT2 : '--' (~[\n\r])* -> channel(HIDDEN) ;
NAME : [a-zA-Z_] ([a-zA-Z0-9_])* ;
STRING : ( '\'' (('\'\'' | ~[']))* '\'' | DOLLAR ) ;
