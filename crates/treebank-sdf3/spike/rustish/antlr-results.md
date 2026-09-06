# ANTLR results for Rustish

2 of 3 corpus expectations hold under the ANTLR lowering.

## PASS: a let is a statement with a pattern and a value; a block has statements and a tail

## PASS: a fn item inside a block is a statement; a block is an expression; a return type

## FAIL: a block as a statement needs no semicolon; a comment is an extra

expected:

```
(program (fn name: (id) body: (block (comment) (block (let pattern: (id) value: (exp_int (int)))))))
```

got:

```
(program (fn name: (id) body: (block (block (let pattern: (id) value: (exp_int (int)))))))
```

