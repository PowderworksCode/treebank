## DEVIATION -- the tree differs in shape from SDF3's AST (1)

- a whole-scope binding (`effect: whole`) is visible before its definition; tree-sitter's locals engine resolves a reference to the nearest definition that precedes it, so a use before the definition resolves outward there. `after` bindings match the engine exactly

## EXTENSION -- a treebank addition outside SDF3 was used (10)

- ConfigFile.ConfigFile: `scope(module)` (not SDF3): `config_file` delimits a lexical scope
- Attribute.Attribute: `binds(name -> enclosing as attribute whole)` (not SDF3): the `_name` under `name` of `attribute` is bound in the enclosing scope, for the whole scope
- Block.Block: `binds(type -> enclosing as block whole)` (not SDF3): the `_name` under `type` of `block` is bound in the enclosing scope, for the whole scope
- Body.Body: `scope(block)` (not SDF3): `body` delimits a lexical scope
- Exp: `refers(1)` (not SDF3): every `identifier` not claimed by a definition is a reference; the production is an injection, so the reference is the token itself
- ForTupleExpr.ForTupleExpr: `scope(for)` (not SDF3): `for_tuple_expr` delimits a lexical scope
- ForObjectExpr.ForObjectExpr: `scope(for)` (not SDF3): `for_object_expr` delimits a lexical scope
- QFor.TemplateFor: `scope(for)` (not SDF3): `template_for` delimits a lexical scope
- HFor.TemplateFor: `scope(for)` (not SDF3): `template_for` delimits a lexical scope

## MAPPED -- lowered exactly (1)

- facets from the attributes: _scope = [body, config_file, for_object_expr, for_tuple_expr, template_for], _binding = [attribute, block]

