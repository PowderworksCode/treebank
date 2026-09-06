## EXTENSION -- a treebank addition outside SDF3 was used (16)

- Stmt.Decl: placeholder label `type` became a field (not SDF3)
- Stmt.Decl: placeholder label `name` became a field (not SDF3)
- Stmt.Assign: placeholder label `target` became a field (not SDF3)
- Stmt.Assign: placeholder label `value` became a field (not SDF3)
- Type.TemplateId: placeholder label `name` became a field (not SDF3)
- Type.TemplateId: placeholder label `arguments` became a field (not SDF3)
- Exp.Call: placeholder label `function` became a field (not SDF3)
- Exp.Call: placeholder label `arguments` became a field (not SDF3)
- Exp.Shr: placeholder label `left` became a field (not SDF3)
- Exp.Shr: placeholder label `right` became a field (not SDF3)
- Exp.Add: placeholder label `left` became a field (not SDF3)
- Exp.Add: placeholder label `right` became a field (not SDF3)
- Exp.Lt: placeholder label `left` became a field (not SDF3)
- Exp.Lt: placeholder label `right` became a field (not SDF3)
- Exp.Gt: placeholder label `left` became a field (not SDF3)
- Exp.Gt: placeholder label `right` became a field (not SDF3)

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (3)

- `keyword -/- [class]`: tree-sitter's keyword extraction already refuses to lex a keyword that is a prefix of a longer word
- lexical restriction on ID: longest-match tokenisation gives the same effect
- lexical restriction on NUM: longest-match tokenisation gives the same effect

## MAPPED -- lowered exactly (17)

- imports [cish] merged additively by the loader: an imported sort gains this module's productions, nothing is overridden -- where tree-sitter's `extends` would flatten and override
- sort Program has the single constructor Program; collapsed to the named rule `program`
- injection into Type became a supertype member with no node of its own
- a `{Elem Sep}+` list expanded to seq/repeat; the expansion has no name in grammar.json
- Type.TemplateId: `{prefer}` became dynamic precedence +1; it decides only where a conflict is declared
- injection into Exp became a supertype member with no node of its own
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- Exp.Shr at priority level 2 became PREC_LEFT at that level
- Exp.Add at priority level 3 became PREC_LEFT at that level
- Exp.Lt at priority level 1 became PREC_LEFT at that level
- Exp.Gt at priority level 1 became PREC_LEFT at that level
- lexical sort ID became the token `id` /[a-zA-Z_](?:[a-zA-Z0-9_])*/
- LAYOUT class became an extras pattern /[ \t\n\r]/
- LAYOUT production became the named extra `comment` /\/\/(?:[^\n\r])*/
- lexical sort NUM became the token `num` /(?:[0-9])+/
- `ID = keyword {reject}` became `word: id` plus reserved.global = [char, int]
- declared conflict [template_id, _exp]: a carry, named by tree-sitter generate and pinned in tree-sitter.conflicts.json; it names a supertype, the early-commit shape notes/field_guide.md §2 budgets for

