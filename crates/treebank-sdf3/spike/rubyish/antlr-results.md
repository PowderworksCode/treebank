# ANTLR results for Rubyish

8 of 12 corpus expectations hold under the ANTLR lowering.

## PASS: a spaced, tight minus is a command argument

## PASS: a minus with space on both sides is subtraction

## PASS: a minus with no space before it is subtraction

## FAIL: WIDENING (tree-sitter): where only a unary minus is possible, spacing does not matter; the module's adjacency constraint rejects `y = - 1`, Ruby accepts it

expected:

```
(program (nl) (assign target: (id) value: (neg operand: (exp_int (int))) (nl)) (assign target: (id) value: (neg operand: (exp_int (int))) (nl)) (assign target: (id) value: (neg operand: (exp_int (int))) (nl)))
```

got:

```
3:4 extraneous input '-' expecting {V_MINUS_SPACED_TIGHT, V_LBRACKET_SPACED, V_LPAREN_SPACED, ID, INT, REGEX}; 4:2 extraneous input '-' expecting {V_MINUS_SPACED_TIGHT, V_LBRACKET_SPACED, V_LPAREN_SPACED, ID, INT, REGEX}
```

## FAIL: after a closing paren no command is possible, so a spaced tight minus is binary

expected:

```
(program (nl) (expr (sub left: (exp_bracket (add left: (id) right: (id))) right: (exp_int (int))) (nl)))
```

got:

```
2:6 missing NL at '-'
```

## PASS: star: splat when spaced and tight, multiplication otherwise

## PASS: bracket: an array argument when spaced, an index when adjacent

## FAIL: paren: a call when adjacent, a parenthesised argument when spaced

expected:

```
(program (nl) (expr (call method: (id) arguments: (exp_int (int)) arguments: (exp_int (int))) (nl)) (expr (command method: (id) argument: (exp_bracket (exp_int (int)))) (nl)) (assign target: (id) value: (exp_bracket (exp_int (int))) (nl)) (expr (call method: (id) arguments: (exp_bracket (exp_int (int)))) (nl)))
```

got:

```
5:4 extraneous input '(' expecting {')', V_MINUS_SPACED_TIGHT, V_LBRACKET_SPACED, V_LPAREN_SPACED, ID, INT, REGEX}; 5:7 extraneous input ')' expecting NL
```

## PASS: slash: a regex argument when spaced and tight, division otherwise

## PASS: a command argument is a whole expression

## FAIL: line breaks absorb blank lines and comment lines; a trailing comment is an extra

expected:

```
(program (nl) (assign target: (id) value: (exp_int (int)) (comment) (nl)) (assign target: (id) value: (exp_int (int)) (nl)))
```

got:

```
(program (nl) (assign target: (id) value: (exp_int (int)) (nl)) (assign target: (id) value: (exp_int (int)) (nl)))
```

## PASS: a splat where no command is possible is an error, as in ruby

