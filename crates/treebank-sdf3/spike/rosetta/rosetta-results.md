# Rosetta results for the spike languages

20 of 20 role queries yield the expected count in every spike language.

## branching

if/else in three spike languages, the shape of test/rosetta/branching. `(_declaration body: (_body))` is the field pattern with a supertype child, the writable form; `(else_clause)` is asserted by concrete name because all three modules name the constructor ElseClause. `_control_flow` is 3: the branch and the two jumps it contains, since the containment nests `_jump` inside it.

| query | jsish | pyish | rustish | expected |
|---|---|---|---|---|
| `(_declaration) @d` | 1 | 1 | 1 | 1 |
| `(_declaration body: (_body)) @db` | 1 | 1 | 1 | 1 |
| `(_branch) @b` | 1 | 1 | 1 | 1 |
| `(_control_flow) @cf` | 3 | 3 | 3 | 3 |
| `(_jump) @j` | 2 | 2 | 2 | 2 |
| `(else_clause) @e` | 1 | 1 | 1 | 1 |
| `(_clause) @c` | 1 | 1 | 1 | 1 |
| `(_literal) @lit` | 3 | 3 | 3 | 3 |

## comments

the shape of test/rosetta/strings-and-comments without the string, since none of the spike modules has one yet. The local variable is the interesting line: the first draft of rustish and jsish put `let` in `_declaration`, and this gate caught that Python's `prefix = name` is not one. The shipped grammars agree with Python -- treebank-rust's `_declaration` holds functions, structs, traits, consts and statics and not `let_declaration` -- so the modules were corrected, and `let` is a `_statement` and a `_binding`, as `x = 1` is. What the three still do not share is `_assignment`, which Python's line carries and the other two do not; that stays a vocabulary question.

| query | jsish | pyish | rustish | expected |
|---|---|---|---|---|
| `(_comment) @c` | 2 | 2 | 2 | 2 |
| `(_declaration) @d` | 1 | 1 | 1 | 1 |
| `(_name) @n` | 6 | 6 | 6 | 6 |
| `(_binding) @b` | 3 | 3 | 3 | 3 |

## hello-roles

the shape of test/rosetta/hello-roles over what the three spike modules share: two functions, three parameters, a loop, three returns, one call. `_scope` and `_binding` are not asserted here: Rust and JavaScript blocks are scopes and Python's are not, and the counts differ for that reason alone.

| query | jsish | pyish | rustish | expected |
|---|---|---|---|---|
| `(_declaration) @d` | 2 | 2 | 2 | 2 |
| `(_parameter) @p` | 3 | 3 | 3 | 3 |
| `(_loop) @l` | 1 | 1 | 1 | 1 |
| `(_jump) @j` | 3 | 3 | 3 | 3 |
| `(_invocation) @i` | 1 | 1 | 1 | 1 |
| `(_callable) @fn` | 2 | 2 | 2 | 2 |
| `(_control_flow) @cf` | 4 | 4 | 4 | 4 |
| `(_declaration name: (_name) @n)` | 2 | 2 | 2 | 2 |

