## WIDENING -- tree-sitter accepts more than SDF3 here (5)

- 2 independent priority chains; tree-sitter precedence is one global order, so their levels are numbered together and may interact
- Exp.Eq is non-assoc; tree-sitter has no non-associativity, lowered to PREC_LEFT so `a == b == c` parses where SDF3 rejects it
- Exp.Lt is non-assoc; tree-sitter has no non-associativity, lowered to PREC_LEFT so `a == b == c` parses where SDF3 rejects it
- Exp.Gt is non-assoc; tree-sitter has no non-associativity, lowered to PREC_LEFT so `a == b == c` parses where SDF3 rejects it
- Exp.Like is non-assoc; tree-sitter has no non-associativity, lowered to PREC_LEFT so `a == b == c` parses where SDF3 rejects it

## DEVIATION -- the tree differs in shape from SDF3's AST (2)

- bracket production of Exp became the named node `exp_bracket`; SDF3's AST has no node for brackets, but a hidden supertype member may have only one visible child and `( Exp )` has three
- 9 named node(s) outside the vocabulary, ledgered as uncategorised: [backtick, col_def, int, item, name, order, script, select, string]

## EXTENSION -- a treebank addition outside SDF3 was used (57)

- Stmt.Insert: placeholder label `hints` became a field (not SDF3)
- Stmt.Insert: placeholder label `table` became a field (not SDF3)
- Stmt.Insert: placeholder label `columns` became a field (not SDF3)
- Stmt.Insert: placeholder label `values` became a field (not SDF3)
- Stmt.Insert: placeholder label `upsert` became a field (not SDF3)
- Stmt.Update: placeholder label `table` became a field (not SDF3)
- Stmt.Update: placeholder label `where` became a field (not SDF3)
- Stmt.Delete: placeholder label `table` became a field (not SDF3)
- Stmt.Delete: placeholder label `where` became a field (not SDF3)
- Stmt.CreateTable: placeholder label `table` became a field (not SDF3)
- Stmt.DropTable: placeholder label `table` became a field (not SDF3)
- Stmt.Replace: placeholder label `table` became a field (not SDF3)
- Stmt.Replace: placeholder label `columns` became a field (not SDF3)
- Stmt.Replace: placeholder label `values` became a field (not SDF3)
- Query.Select: placeholder label `hints` became a field (not SDF3)
- Query.Select: placeholder label `items` became a field (not SDF3)
- Query.Select: placeholder label `from` became a field (not SDF3)
- Query.Select: placeholder label `where` became a field (not SDF3)
- Query.Select: placeholder label `order` became a field (not SDF3)
- Query.Select: placeholder label `limit` became a field (not SDF3)
- Query.Select: placeholder label `offset` became a field (not SDF3)
- Item.Item: placeholder label `alias` became a field (not SDF3)
- From.From: placeholder label `table` became a field (not SDF3)
- OrderItem.Order: placeholder label `dir` became a field (not SDF3)
- Cte.Cte: placeholder label `name` became a field (not SDF3)
- Assign.Assign: placeholder label `column` became a field (not SDF3)
- Assign.Assign: placeholder label `value` became a field (not SDF3)
- ColDef.ColDef: placeholder label `name` became a field (not SDF3)
- Exp.Column: placeholder label `table` became a field (not SDF3)
- Exp.Column: placeholder label `column` became a field (not SDF3)
- Exp.Call: placeholder label `function` became a field (not SDF3)
- Exp.Call: placeholder label `arguments` became a field (not SDF3)
- Exp.Mul: placeholder label `left` became a field (not SDF3)
- Exp.Mul: placeholder label `right` became a field (not SDF3)
- Exp.Add: placeholder label `left` became a field (not SDF3)
- Exp.Add: placeholder label `right` became a field (not SDF3)
- Exp.Sub: placeholder label `left` became a field (not SDF3)
- Exp.Sub: placeholder label `right` became a field (not SDF3)
- Exp.Eq: placeholder label `left` became a field (not SDF3)
- Exp.Eq: placeholder label `right` became a field (not SDF3)
- Exp.Lt: placeholder label `left` became a field (not SDF3)
- Exp.Lt: placeholder label `right` became a field (not SDF3)
- Exp.Gt: placeholder label `left` became a field (not SDF3)
- Exp.Gt: placeholder label `right` became a field (not SDF3)
- Exp.Like: placeholder label `left` became a field (not SDF3)
- Exp.Like: placeholder label `right` became a field (not SDF3)
- Exp.And: placeholder label `left` became a field (not SDF3)
- Exp.And: placeholder label `right` became a field (not SDF3)
- Exp.Or: placeholder label `left` became a field (not SDF3)
- Exp.Or: placeholder label `right` became a field (not SDF3)
- Exp.Arrow: placeholder label `left` became a field (not SDF3)
- Exp.Arrow: placeholder label `right` became a field (not SDF3)
- Exp.ArrowText: placeholder label `left` became a field (not SDF3)
- Exp.ArrowText: placeholder label `right` became a field (not SDF3)
- Limit.Limit: placeholder label `count` became a field (not SDF3)
- Offset.Offset: placeholder label `start` became a field (not SDF3)
- `vocabulary` section (not SDF3): 9 terms of treebank's vocabulary 0.1.0 bound to this module's sorts and constructors

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (6)

- `keyword -/- [class]`: tree-sitter's keyword extraction already refuses to lex a keyword that is a prefix of a longer word
- lexical restriction on NAME: longest-match tokenisation gives the same effect
- lexical restriction on INT: longest-match tokenisation gives the same effect
- context-free restriction on LAYOUT?: extras are skipped greedily

## MAPPED -- lowered exactly (63)

- imports [mysql/5.6, sql/json-arrow] merged additively by the loader: an imported sort gains this module's productions, nothing is overridden -- where tree-sitter's `extends` would flatten and override
- sort CreateTail has no production in this composition (a dialect point this target leaves empty); its optional occurrence was removed from [Stmt.CreateTable]
- sort Returning has no production in this composition (a dialect point this target leaves empty); its optional occurrence was removed from [Stmt.Insert, Stmt.Update, Stmt.Delete]
- sort With has no production in this composition (a dialect point this target leaves empty); its optional occurrence was removed from [Stmt.Select]
- sort Script has the single constructor Script; collapsed to the named rule `script`
- a `{Elem Sep}+` list expanded to seq/repeat; the expansion has no name in grammar.json
- sort Query has the single constructor Select; collapsed to the named rule `select`
- sort Item has the single constructor Item; collapsed to the named rule `item`
- sort From has the single constructor From; collapsed to the named rule `from`
- sort Where has the single constructor Where; collapsed to the named rule `where`
- a `{Elem Sep}+` list expanded to seq/repeat; the expansion has no name in grammar.json
- sort OrderBy has the single constructor OrderBy; collapsed to the named rule `order_by`
- sort OrderItem has the single constructor Order; collapsed to the named rule `order`
- sort Cte has the single constructor Cte; collapsed to the named rule `cte`
- sort Assign has the single constructor Assign; collapsed to the named rule `assign`
- sort ColDef has the single constructor ColDef; collapsed to the named rule `col_def`
- injection into Exp became a supertype member with no node of its own
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- Exp.Call at priority level 10 became PREC at that level
- Exp.Neg at priority level 9 became PREC at that level
- Exp.Mul at priority level 8 became PREC_LEFT at that level
- Exp.Add at priority level 7 became PREC_LEFT at that level
- Exp.Sub at priority level 7 became PREC_LEFT at that level
- Exp.Eq at priority level 6 became PREC_LEFT at that level
- Exp.Lt at priority level 6 became PREC_LEFT at that level
- Exp.Gt at priority level 6 became PREC_LEFT at that level
- Exp.Like at priority level 6 became PREC_LEFT at that level
- Exp.Not at priority level 5 became PREC at that level
- Exp.And at priority level 4 became PREC_LEFT at that level
- Exp.Or at priority level 3 became PREC_LEFT at that level
- Exp.Arrow at priority level 2 became PREC_LEFT at that level
- Exp.ArrowText at priority level 2 became PREC_LEFT at that level
- sort Limit has the single constructor Limit; collapsed to the named rule `limit`
- sort Offset has the single constructor Offset; collapsed to the named rule `offset`
- sort InsertHint has the single constructor Ignore; collapsed to the named rule `ignore`
- a `{Elem Sep}+` list expanded to seq/repeat; the expansion has no name in grammar.json
- sort Upsert has the single constructor OnDuplicateKey; collapsed to the named rule `on_duplicate_key`
- lexical sort BACKTICK became the token `backtick` /`(?:[^`])*`/
- lexical sort DQSTRING became the token `dqstring` /"(?:[^"])*"/
- lexical sort INT became the token `int` /(?:[0-9])+/
- LAYOUT class became an extras pattern /[ \t\n\r]/
- LAYOUT production became the named extra `comment` /--(?:[^\n\r])*/
- LAYOUT production became the named extra `comment_2` /#(?:[^\n\r])*/
- lexical sort NAME became the token `name` /[a-zA-Z_](?:[a-zA-Z0-9_])*/
- lexical sort STRING became the token `string` /(?:'(?:(?:''|[^']))*'|(?:"(?:[^"])*"))/
- `NAME = keyword {reject}` became `word: name` plus reserved.global = [AND, AS, ASC, BY, CREATE, DELETE, DESC, DROP, DUPLICATE, FROM, IGNORE, INSERT, INT, INTO, KEY, LIKE, LIMIT, NOT, NULL, OFFSET, ON, OR, ORDER, REPLACE, SELECT, SET, SQL_CACHE, SQL_NO_CACHE, TABLE, TEXT, UPDATE, VALUES, VARCHAR, WHERE]
- `tokenize: "(),;*."`: the reader split template literal runs at these characters, so each is its own token
- `_statement` is the sort Stmt: its supertype `_stmt` is named `_statement`
- `_expression` is the sort Exp: its supertype `_exp` is named `_expression`
- `_name` is the sort Ident: its supertype `_ident` is named `_name`
- `_literal` threaded as a supertype over [exp_int, str, null]; every reference to a member now goes through it, and the tree is unchanged
- `_invocation` threaded as a supertype over [call]; every reference to a member now goes through it, and the tree is unchanged
- `_declaration` threaded as a supertype over [create_table]; every reference to a member now goes through it, and the tree is unchanged
- `_assignment` threaded as a supertype over [assign]; every reference to a member now goes through it, and the tree is unchanged
- `_modifier` threaded as a supertype over [_dir, _select_hint, ignore]; every reference to a member now goes through it, and the tree is unchanged; [_dir, _select_hint] are sorts, not terms, and were flattened into it
- `_clause` is a facet: [as, bare, from, limit, offset, on_duplicate_key, order_by, where] listed in roles.json; [_alias] left the supertypes list (hidden rules still), since a supertype must be a table-tier term
- roles.json: 9 of 22 table-tier terms are supertypes, 2 facet(s), 57 named node(s), 9 uncategorised (vocabulary 0.1.0)

