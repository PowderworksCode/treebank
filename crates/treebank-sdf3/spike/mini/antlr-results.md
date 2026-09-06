# ANTLR results for Mini

8 of 9 corpus expectations hold under the ANTLR lowering.

## PASS: priorities nest the way the SDF3 chain says: * above +

## PASS: {left} groups associate to the left

## PASS: the unary group outranks every binary group

## PASS: DEVIATION: a {bracket} production is a named node here; SDF3's AST has none

## PASS: an injection yields no node: Exp = ID is just the id

## PASS: comparison sits below arithmetic

## FAIL: statements, separated lists, blocks, and a comment as an extra

expected:

```
(program (comment) body: (fun name: (id) parameters: (id) parameters: (id) body: (block (return value: (sub left: (id) right: (id))))) body: (fun name: (id) body: (block)) body: (if condition: (lt left: (id) right: (exp_int (int))) consequence: (block (assign target: (id) value: (call function: (id) arguments: (id) arguments: (exp_int (int))))) alternative: (block (while condition: (eq left: (id) right: (exp_int (int))) body: (block)))))
```

got:

```
(program body: (fun name: (id) parameters: (id) parameters: (id) body: (block (return value: (sub left: (id) right: (id))))) body: (fun name: (id) body: (block)) body: (if condition: (lt left: (id) right: (exp_int (int))) consequence: (block (assign target: (id) value: (call function: (id) arguments: (id) arguments: (exp_int (int))))) alternative: (block (while condition: (eq left: (id) right: (exp_int (int))) body: (block)))))
```

## PASS: WIDENING: non-assoc lowered to prec.left, so a == b == c parses (SDF3 rejects it)

## PASS: a template keyword is reserved: `let` cannot be a name

