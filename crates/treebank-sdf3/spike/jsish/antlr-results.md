# ANTLR results for Jsish

2 of 3 corpus expectations hold under the ANTLR lowering.

## PASS: var and let are different statements; a block is a statement

## PASS: a function declaration with parameters, a return, a call, a print

## FAIL: if with a block; a comment is an extra

expected:

```
(program (comment) (if condition: (exp_int (int)) consequence: (block (var name: (id) value: (exp_int (int))))))
```

got:

```
(program (if condition: (exp_int (int)) consequence: (block (var name: (id) value: (exp_int (int))))))
```

