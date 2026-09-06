// GENERATED from mini.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Mini;

program
    : (body+=stmt)* EOF
    ;

stmt
    :     'let'  name=ID  '='  value=exp  ';'  # let
    |     target=ID  '='  value=exp  ';'  # assign
    |     'if'  '('  condition=exp  ')'  consequence=block  'else'  alternative=block  # if
    |     'while'  '('  condition=exp  ')'  body=block  # while
    |     'fun'  name=ID  '('  (parameters+=ID (',' parameters+=ID)*)?  ')'  body=block  # fun
    |     'return'  value=exp  ';'  # return
    |     exp  ';'  # expr
    ;

block
    : '{'  stmt*  '}'
    ;

exp
    :     '-'  operand=exp  # neg
    |     '!'  operand=exp  # not
    |     left=exp  '*'  right=exp  # mul
    |     left=exp  '/'  right=exp  # div
    |     left=exp  '+'  right=exp  # add
    |     left=exp  '-'  right=exp  # sub
    |     left=exp  '=='  right=exp  # eq
    |     left=exp  '<'  right=exp  # lt
    |     ID  # inj_exp_1
    |     INT  # exp_int
    |     function=ID  '('  (arguments+=exp (',' arguments+=exp)*)?  ')'  # call
    |     '('  exp  ')'  # exp_bracket
    ;

ID : [a-zA-Z_] ([a-zA-Z0-9_])* ;
INT : ([0-9])+ ;
WS1 : [ \t\n\r]+ -> channel(HIDDEN) ;
COMMENT2 : '//' (~[\n\r])* -> channel(HIDDEN) ;
