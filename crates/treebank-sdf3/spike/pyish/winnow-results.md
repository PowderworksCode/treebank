# winnow results for pyish

13 of 13 corpus expectations hold under the winnow lowering, 1 of them by rejecting what the source rejects (SOURCE).

## PASS: a statement ends at the line break; the next line at its column is the next statement

## PASS: a block is the lines indented past the line that opened it

## PASS: one line closes every block it has left

## PASS: else sits at its if's column

## PASS: offside: a deeper next line continues the statement

## PASS: offside: a next line at the statement's column is a new statement, so a minus there is unary

## PASS: blank lines and comment lines neither end nor continue a block

## PASS: a trailing comment is an extra before the line break that ends the statement

## PASS: def: parameters, a body, a return; a call with arguments

## SOURCE: WIDENING (tree-sitter): inside brackets a line break is layout at any column, as in Python; the offside constraint rejects the `2`

expected:

```
(program (assign target: (id) value: (exp_bracket (add left: (exp_int (int)) right: (exp_int (int))))) (assign target: (id) value: (exp_int (int))))
```

got:

```
ERROR at 2:8
```

## PASS: a dedent to a column no open block has is an error

## PASS: else off its if's column is an error

## PASS: a block opener with nothing indented after it is an error

