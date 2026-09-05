## DEVIATION -- the tree differs in shape from SDF3's AST (2)

- Item.Fn: `binds(name -> enclosing)` on a scope node: tree-sitter's locals engine files a definition under the innermost scope containing it, which is this node; the query carries nvim-treesitter's `#set! ..scope "parent"`, which tree-sitter's own highlighter ignores
- a whole-scope binding (`effect: whole`) is visible before its definition; tree-sitter's locals engine resolves a reference to the nearest definition that precedes it, so a use before the definition resolves outward there. `after` bindings match the engine exactly

## EXTENSION -- a treebank addition outside SDF3 was used (7)

- Program.Program: `scope(module)` (not SDF3): `program` delimits a lexical scope
- Item.Fn: `scope(function)` (not SDF3): `fn` delimits a lexical scope
- Item.Fn: `binds(name -> enclosing as function whole)` (not SDF3): the `id` under `name` of `fn` is bound in the enclosing scope, for the whole scope
- Param.Param: `binds(name -> enclosing as parameter whole)` (not SDF3): the `id` under `name` of `param` is bound in the enclosing scope, for the whole scope
- Block.Block: `scope(block)` (not SDF3): `block` delimits a lexical scope
- Stmt.Let: `binds(pattern -> enclosing as var after)` (not SDF3): the `id` under `pattern` of `let` is bound in the enclosing scope, from the end of the node onward
- Exp: `refers(1)` (not SDF3): every `id` not claimed by a definition is a reference; the production is an injection, so the reference is the token itself

## MAPPED -- lowered exactly (1)

- facets from the attributes: _scope = [block, fn, program], _binding = [fn, let, param]

