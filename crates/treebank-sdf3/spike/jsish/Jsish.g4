// GENERATED from jsish.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Jsish;

program
    : stmt* EOF
    ;

stmt
    :     'function' name=ID '(' (parameters+=param (',' parameters+=param)*)? ')' body=block  # function
    |     'var' name=ID '=' value=exp ';'  # var
    |     'let' name=ID '=' value=exp ';'  # let
    |     target=ID '=' value=exp ';'  # assign
    |     'console.log' '(' value=exp ')' ';'  # print
    |     'return' value=exp ';'  # return
    |     'if' '(' condition=exp ')' consequence=block  # if
    |     exp ';'  # expr
    |     block  # inj_stmt_1
    ;

block
    : '{' stmt* '}'
    ;

param
    : name=ID
    ;

exp
    :     function=exp '(' (arguments+=exp (',' arguments+=exp)*)? ')'  # call
    |     '-' operand=exp  # neg
    |     left=exp '*' right=exp  # mul
    |     left=exp '+' right=exp  # add
    |     left=exp '-' right=exp  # sub
    |     ID  # inj_exp_2
    |     INT  # exp_int
    |     '(' exp ')'  # exp_bracket
    ;

ID : [a-zA-Z_$] ([a-zA-Z0-9_$])* ;
INT : ([0-9])+ ;
WS1 : [ \t\n\r]+ -> channel(HIDDEN) ;
COMMENT2 : '//' (~[\n\r])* -> channel(HIDDEN) ;
