# winnow results for rubyish

12 of 12 corpus expectations hold under the winnow lowering, 1 of them by rejecting what the source rejects (SOURCE).

## PASS: a spaced, tight minus is a command argument

## PASS: a minus with space on both sides is subtraction

## PASS: a minus with no space before it is subtraction

## SOURCE: WIDENING (tree-sitter): where only a unary minus is possible, spacing does not matter; the module's adjacency constraint rejects `y = - 1`, Ruby accepts it

expected:

```
(program (nl) (assign target: (id) value: (neg operand: (exp_int (int))) (nl)) (assign target: (id) value: (neg operand: (exp_int (int))) (nl)) (assign target: (id) value: (neg operand: (exp_int (int))) (nl)))
```

got:

```
ERROR at 3:7
```

## PASS: after a closing paren no command is possible, so a spaced tight minus is binary

## PASS: star: splat when spaced and tight, multiplication otherwise

## PASS: bracket: an array argument when spaced, an index when adjacent

## PASS: paren: a call when adjacent, a parenthesised argument when spaced

## PASS: slash: a regex argument when spaced and tight, division otherwise

## PASS: a command argument is a whole expression

## PASS: line breaks absorb blank lines and comment lines; a trailing comment is an extra

## PASS: a splat where no command is possible is an error, as in ruby

