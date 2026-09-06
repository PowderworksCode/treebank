// GENERATED from rubyish.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Rubyish;

@lexer::members {
def gap_before(self):
    return self._input.LA(-1) in (-1, 10, 13, 32, 9)
def gap_after(self):
    return self._input.LA(1) in (-1, 10, 13, 32, 9)
}

program
    : NL?  stmt* EOF
    ;

stmt
    :     target=ID  '='  value=exp  NL  # assign
    |     exp  NL  # expr
    ;

exp
    :     method=ID  argument=arg  # command
    |     receiver=exp  V_LBRACKET_ADJACENT  index=exp  ']'  # index
    |     method=ID  V_LPAREN_ADJACENT  (arguments+=exp (',' arguments+=exp)*)?  ')'  # call
    |     V_MINUS_SPACED_TIGHT  operand=exp  # neg
    |     left=exp  V_STAR  right=exp  # mul
    |     left=exp  V_SLASH  right=exp  # div
    |     left=exp  '+'  right=exp  # add
    |     left=exp  V_MINUS  right=exp  # sub
    |     ID  # inj_exp_1
    |     INT  # exp_int
    |     REGEX  # exp_regex
    |     V_LBRACKET_SPACED  (elements+=exp (',' elements+=exp)*)?  ']'  # array
    |     V_LPAREN_SPACED  exp  ')'  # exp_bracket
    ;

arg
    :     exp  # inj_arg_2
    |     V_STAR_SPACED_TIGHT  operand=exp  # splat
    ;

V_MINUS_SPACED_TIGHT : {self.gap_before()}? '-' {not self.gap_after()}? ;
V_STAR_SPACED_TIGHT : {self.gap_before()}? '*' {not self.gap_after()}? ;
V_LBRACKET_ADJACENT : {not self.gap_before()}? '[' ;
V_LBRACKET_SPACED : {self.gap_before()}? '[' ;
V_LPAREN_ADJACENT : {not self.gap_before()}? '(' ;
V_LPAREN_SPACED : {self.gap_before()}? '(' ;
V_MINUS : '-' ;
V_SLASH : '/' ;
V_STAR : '*' ;
ID : [a-z_] ([a-zA-Z0-9_])* ;
INT : ([0-9])+ ;
WS1 : [ \t]+ -> channel(HIDDEN) ;
COMMENT2 : '#' (~[\n])* -> channel(HIDDEN) ;
NL : [\n] (([ \t\n] | '#' (~[\n])*))* ;
REGEX : {self.gap_before()}? '/' ~[/\n \t] (~[/\n])* '/' ;
