## WIDENING -- tree-sitter accepts more than SDF3 here (1)

- without validity, an unconstrained occurrence takes only the default variant and a constrained one only its own: `(a+b) -1` has no token that subtraction accepts, `z=-1` has no token that negation accepts, and both are rejected where SDF3 accepts them; `y = - 1` is rejected as SDF3 rejects it, where tree-sitter's scanner widened

## DEVIATION -- the tree differs in shape from SDF3's AST (5)

- no parser predicate carries a layout constraint: ANTLR consults a left-edge predicate during prediction in a plain rule and not in a left-recursive one (measured), and every expression rule is left-recursive
- injection into Exp is a context node in ANTLR's tree (`inj_exp_1`); the driver elides it when printing
- injection into Arg is a context node in ANTLR's tree (`inj_arg_2`); the driver elides it when printing
- LAYOUT goes to the hidden channel: comments are absent from ANTLR's tree, where tree-sitter shows them as extras
- lexical sort REGEX is reached by a separation constraint: the lexer checks the character before it with a predicate, since it cannot ask the parser what is valid; `x =/b/` is rejected where tree-sitter's validity-first scanner accepts it

## MAPPED -- lowered exactly (2)

- layout constraints became lexer token variants with lexer predicates on the character before and after, from the same plan as the tree-sitter scanner; the parser has no say in which variant the lexer emits, which is the validity tree-sitter's scanner had and ANTLR's lexer does not
- Exp.Command: `{prefer}` became alternative order within `exp`; ALL(*) takes the first viable alternative, so an ambiguity decided in an ancestor rule follows that rule's source order, which this attribute does not reach

