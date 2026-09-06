# winnow results for mini

9 of 9 corpus expectations hold under the winnow lowering, 1 of them by rejecting what the source rejects (SOURCE).

## PASS: priorities nest the way the SDF3 chain says: * above +

## PASS: {left} groups associate to the left

## PASS: the unary group outranks every binary group

## PASS: DEVIATION: a {bracket} production is a named node here; SDF3's AST has none

## PASS: an injection yields no node: Exp = ID is just the id

## PASS: comparison sits below arithmetic

## PASS: statements, separated lists, blocks, and a comment as an extra

## SOURCE: WIDENING: non-assoc lowered to prec.left, so a == b == c parses (SDF3 rejects it)

expected:

```
(program body: (assign target: (id) value: (eq left: (eq left: (exp_int (int)) right: (exp_int (int))) right: (exp_int (int)))))
```

got:

```
ERROR at 2:11
```

## PASS: a template keyword is reserved: `let` cannot be a name

