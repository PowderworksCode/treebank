// GENERATED from jsish.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Jsish;

program
    : statement* EOF
    ;

statement
    :     'function' name=ID '(' (parameters+=param (',' parameters+=param)*)? ')' body=block  # function
    |     'var' name=ID '=' value=expression ';'  # var
    |     'let' name=ID '=' value=expression ';'  # let
    |     target=ID '=' value=expression ';'  # assign
    |     'console.log' '(' value=expression ')' ';'  # print
    |     'return' value=expression ';'  # return
    |     'if' '(' condition=expression ')' consequence=block (alternative=else_clause)?  # if
    |     'while' '(' condition=expression ')' body=block  # while
    |     expression ';'  # expr
    |     block  # inj_stmt_1
    ;

block
    : '{' statement* '}'
    ;

else_clause
    : 'else' body=block
    ;

param
    : name=ID
    ;

expression
    :     function=expression '(' (arguments+=expression (',' arguments+=expression)*)? ')'  # call
    |     '-' operand=expression  # neg
    |     left=expression '*' right=expression  # mul
    |     left=expression '+' right=expression  # add
    |     left=expression '-' right=expression  # sub
    |     left=expression '<' right=expression  # lt
    |     ID  # inj_exp_2
    |     INT  # exp_int
    |     '(' expression ')'  # exp_bracket
    ;

ID : [a-zA-Z_$] ([a-zA-Z0-9_$])* ;
INT : ([0-9])+ ;
WS1 : [ \t\n\r]+ -> channel(HIDDEN) ;
COMMENT2 : '//' (~[\n\r])* -> channel(HIDDEN) ;
