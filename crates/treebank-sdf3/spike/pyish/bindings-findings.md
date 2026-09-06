## DEVIATION -- the tree differs in shape from SDF3's AST (3)

- a whole-scope binding (`effect: whole`) is visible before its definition; tree-sitter's locals engine resolves a reference to the nearest definition that precedes it, so a use before the definition resolves outward there. `after` bindings match the engine exactly
- Stmt.Global: `binds(names -> module)`: the locals query dialect cannot name a scope by kind, so the pattern binds at the nearest scope; bindings.json carries the target
- Stmt.Def: `binds(name -> enclosing)` on a scope node: tree-sitter's locals engine files a definition under the innermost scope containing it, which is this node; the query carries nvim-treesitter's `#set! ..scope "parent"`, which tree-sitter's own highlighter ignores

## EXTENSION -- a treebank addition outside SDF3 was used (7)

- Program.Program: `scope(module)` (not SDF3): `program` delimits a lexical scope
- Stmt.Assign: `binds(target -> enclosing as var whole)` (not SDF3): the `id` under `target` of `assign` is bound in the enclosing scope, for the whole scope
- Stmt.Global: `binds(names -> module as var whole)` (not SDF3): the `id` under `names` of `global` is bound in the module scope, for the whole scope
- Stmt.Def: `scope(function)` (not SDF3): `def` delimits a lexical scope
- Stmt.Def: `binds(name -> enclosing as function whole)` (not SDF3): the `id` under `name` of `def` is bound in the enclosing scope, for the whole scope
- Param.Param: `binds(name -> enclosing as parameter whole)` (not SDF3): the `id` under `name` of `param` is bound in the enclosing scope, for the whole scope
- Exp: `refers(1)` (not SDF3): every `id` not claimed by a definition is a reference; the production is an injection, so the reference is the token itself

## MAPPED -- lowered exactly (1)

- facets from the attributes: _scope = [def, program], _binding = [assign, def, global, param]

