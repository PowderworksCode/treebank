# ANTLR results for Pyish

10 of 13 corpus expectations hold under the ANTLR lowering.

## PASS: a statement ends at the line break; the next line at its column is the next statement

## PASS: a block is the lines indented past the line that opened it

## PASS: one line closes every block it has left

## PASS: else sits at its if's column

## PASS: offside: a deeper next line continues the statement

## PASS: offside: a next line at the statement's column is a new statement, so a minus there is unary

## FAIL: blank lines and comment lines neither end nor continue a block

expected:

```
(program (if condition: (id) consequence: (block (pass) (comment) (comment) (pass))) (assign target: (id) value: (exp_int (int))))
```

got:

```
(program (if condition: (id) consequence: (block (pass) (pass))) (assign target: (id) value: (exp_int (int))))
```

## FAIL: a trailing comment is an extra before the line break that ends the statement

expected:

```
(program (assign target: (id) value: (exp_int (int)) (comment)) (if condition: (id) (comment) consequence: (block (pass))))
```

got:

```
(program (assign target: (id) value: (exp_int (int))) (if condition: (id) consequence: (block (pass))))
```

## PASS: def: parameters, a body, a return; a call with arguments

## FAIL: WIDENING (tree-sitter): inside brackets a line break is layout at any column, as in Python; the offside constraint rejects the `2`

expected:

```
(program (assign target: (id) value: (exp_bracket (add left: (exp_int (int)) right: (exp_int (int))))) (assign target: (id) value: (exp_int (int))))
```

got:

```
2:8 extraneous input '\n' expecting {'(', '-', ID, INT}
```

## PASS: a dedent to a column no open block has is an error

## PASS: else off its if's column is an error

## PASS: a block opener with nothing indented after it is an error

