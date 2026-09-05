// GENERATED from pyish.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Pyish;

// H_ tokens are hidden in the tree, as tree-sitter's `_` externals are.

@lexer::members {
# Indentation, from the module's indent/align-list constraints: the
# indent stack tree-sitter's generated scanner keeps, without validity.
# The lexer cannot ask the parser whether a block may open here, so a
# deeper line opens one only after an opener literal, and continues
# the statement otherwise.
_OPENERS = (':',)
_COMMENT_OPEN = 35
def _ind(self):
    if not hasattr(self, '_stack'):
        self._stack = [0]
        self._queue = []
        self._last = None
    return self._stack
def _make(self, ttype):
    return self._factory.create(self._tokenFactorySourcePair, ttype, '',
        Token.DEFAULT_CHANNEL, self._input.index, self._input.index - 1,
        self.line, self.column)
def nextToken(self):
    stack = self._ind()
    if self._queue:
        return self._queue.pop(0)
    t = super().nextToken()
    if t.type == Token.EOF:
        if self._last is not None and self._last.type not in (self.H_NEWLINE, self.H_DEDENT):
            self._queue.append(self._make(self.H_NEWLINE))
        while len(stack) > 1:
            stack.pop()
            self._queue.append(self._make(self.H_DEDENT))
        if self._queue:
            self._queue.append(t)
            return self._queue.pop(0)
        return t
    if t.channel == Token.DEFAULT_CHANNEL:
        self._last = t
    return t
def on_newline(self):
    stack = self._ind()
    if self._last is None:
        self.skip()  # a break before the first token
        return
    nxt = self._input.LA(1)
    if nxt in (10, 13) or (self._COMMENT_OPEN and nxt == self._COMMENT_OPEN):
        self.skip()  # a blank or comment line: the next break decides
        return
    col = 0 if nxt == -1 else len(self.text.lstrip('\r\n'))
    top = stack[-1]
    if col > top:
        if self._last.text in self._OPENERS:
            stack.append(col)
            self._type = self.H_INDENT
            return
        self.skip()  # a continuation line: the offside rule
        return
    self._type = self.H_NEWLINE
    while col < stack[-1]:
        stack.pop()
        self._queue.append(self._make(self.H_DEDENT))
    if col != stack[-1]:
        # a dedent to a column no open block has: a token no rule accepts
        self._queue.append(self._make(Token.INVALID_TYPE))
}

program
    : stmt* EOF
    ;

stmt
    :     target=ID '=' value=exp H_NEWLINE  # assign
    |     exp H_NEWLINE  # expr
    |     'return' value=exp H_NEWLINE  # return
    |     'global' names+=ID (',' names+=ID)* H_NEWLINE  # global
    |     'pass' H_NEWLINE  # pass
    |     'if' condition=exp ':' H_INDENT consequence=block H_DEDENT (alternative=else_clause)?  # if
    |     'while' condition=exp ':' H_INDENT body=block H_DEDENT  # while
    |     'def' name=ID '(' (parameters+=param (',' parameters+=param)*)? ')' ':' H_INDENT body=block H_DEDENT  # def
    ;

else_clause
    : 'else' ':' H_INDENT body=block H_DEDENT
    ;

block
    : stmt+
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
    |     left=exp '<' right=exp  # lt
    |     ID  # inj_exp_1
    |     INT  # exp_int
    |     '(' exp ')'  # exp_bracket
    ;

H_NEWLINE : ( '\r'? '\n' | '\r' ) [ \t]* { self.on_newline() } ;
H_INDENT : '\u0001' ;
H_DEDENT : '\u0002' ;
ID : [a-zA-Z_] ([a-zA-Z0-9_])* ;
INT : ([0-9])+ ;
WS1 : [ \t]+ -> channel(HIDDEN) ;
COMMENT2 : '#' (~[\n\r])* -> channel(HIDDEN) ;
