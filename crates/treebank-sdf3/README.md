# treebank-sdf3

The spike behind `notes/metagrammar.md` §11: can SDF3, as Spoofax documents
it, be adopted as treebank's meta-grammar and lowered to tree-sitter without
losing what it says?

Three pieces:

- **`src/parse.rs`** — a reader for SDF3 modules, written with winnow.
  Sections, both production forms (productive and template), `{Elem Sep}*`
  lists, character classes with SDF's escaping, attributes, priority chains
  with associativity groups, restrictions, template options. What it does
  not understand it refuses, loudly.
- **`src/lower.rs`** — the lowering to a tree-sitter `grammar.json`, and a
  `Finding` for every place the lowering is not exact. Sorts become
  supertypes; constructors become named nodes; injections become supertype
  members with no node; priority chains become `prec.left` levels;
  `template options` become `word` plus `reserved`; LAYOUT becomes `extras`.
- **`src/scanner.rs`** — the planner for layout constraints tree-sitter's
  grammar cannot express, and the scanner it generates for them.
- **`spike/mini/`** — a small imperative language in `mini.sdf3`, the
  generated `grammar.json`, a readable `grammar.js`, `findings.md`, the
  generated parser under `src/`, and `test/corpus/mini.txt` — expectations
  written from the SDF3 semantics, which `tree-sitter test` then holds the
  generated parser to.

## Running it

```sh
# lower mini.sdf3 -> grammar.json, grammar.js, findings.md (committed)
cargo run -p treebank-sdf3 --example lower -- crates/treebank-sdf3/spike/mini/mini.sdf3

# the committed output is what the reader and lowering produce
cargo test -p treebank-sdf3

# generate the parser and hold it to the expectations (needs tree-sitter 0.26.12)
crates/treebank-sdf3/spike/mini/verify.sh
crates/treebank-sdf3/spike/rubyish/verify.sh
crates/treebank-sdf3/spike/cppish/verify.sh   # also asks generate for the carry's conflicts
crates/treebank-sdf3/spike/pyish/verify.sh    # regenerates the indent-stack scanner; checks bindings against symtable and python3
crates/treebank-sdf3/spike/rustish/verify.sh  # checks bindings against rustc
crates/treebank-sdf3/spike/jsish/verify.sh    # checks bindings against node
python3 crates/treebank-sdf3/tools/rosetta_check.py crates/treebank-sdf3/spike/rosetta   # the rosetta gate over the spike languages
```

## What it found

Everything in `findings.md`, in short: the SDF3 semantics survive the trip
except in three named places. Non-associativity has no tree-sitter form and
lowers to `prec.left` (a widening). A `{bracket}` production cannot be a
hidden supertype member, because tree-sitter requires such a member to have
exactly one visible child and `( Exp )` has three, so brackets become a named
node SDF3's AST does not have (a deviation). And SDF3 has no field labels, so
the reader accepts `<left:Exp>` as a treebank extension.

One treebank extension, one tree-sitter constraint, one true widening. The
rest lowered exactly, and `notes/metagrammar.md` §13 records the numbers.

## The second language: the corner of Ruby where the lexer needs the parser

`spike/rubyish/` is the test mini could not be. The same characters lex
differently by spacing and by what the parser could accept: `foo -1` is a
command call with a negative argument, `foo - 1` and `foo-1` subtract; `foo
*a` splats, `foo * a` multiplies; `foo [1]` passes an array, `foo[1]`
indexes; `foo(1)` calls, `foo (1)` passes a parenthesised argument; `a /b/`
passes a regex, `a / b` divides. CRuby decides these in its lexer with
EXPR_ARG state; treebank's ruby grammar decides them in a hand-written
scanner (`notes/field_guide.md` §1, rung 1).

In SDF3 they are layout constraints on productions — `{layout(1.last.col +
1 == 2.first.col)}` for adjacency, `<` for separation — plus `{prefer}`. A
tree-sitter grammar has no way to say "layout required before this token",
so `src/scanner.rs` **splits** every constrained spelling into scanner-owned
variants (`_minus` and `_minus_spaced_tight`), aliases each back to its
spelling so the tree still shows `-`, and **generates `src/scanner.c`**,
which decides between variants by what the parser could accept first and by
the actual spacing second. That is the shape of treebank-ruby's scanner,
derived from the grammar instead of written.

Twelve expectations written from Ruby's semantics; twelve hold, with zero
conflicts. `verify.sh` there regenerates grammar and scanner together.

## The third language: the ambiguity C++ keeps

`spike/cppish/` is the `carry` intent. `a < b > c;` is either the expression
`(a < b) > c` or a declaration of `c` with type `a<b>`, and nothing short of
a symbol table decides it. SDF3 parses both and `{prefer}` on the template
reading picks it when both survive the statement. treebank-cpp carries the
same ambiguity as declared conflicts and cut template arguments in
*expression* position to keep its table converging; this module keeps them
in type position only, as that grammar does.

Two things are new in the lowering. `cppish.sdf3` **imports** `cish.sdf3`
rather than copying it — SDF3 composition is additive, so `Type` gains one
production and every other rule is cish's; the loader (`load_module`)
merges sections, and a finding says so. And `{prefer}` lowers to dynamic
precedence, which tree-sitter only consults inside a **declared conflict**
— which the lowering cannot compute. So `--generate` asks `tree-sitter
generate`, declares the conflict it names, and pins the set in
`tree-sitter.conflicts.json` beside the module: the carry's backend data,
reproducible without the CLI, diffable when generate's view moves.

Eight expectations from C++'s semantics; eight hold. One declared conflict,
`[template_id, _exp]` — it names a supertype, the early-commit shape the
field guide budgets for. `verify.sh` there also runs the post-condition
`notes/metagrammar.md` §3 asks for: `a < b > c;` must actually fork
(`version_count` peaks at 2) and `x = a < b > c;` must not (it stays at 1,
because after `=` no declaration is possible). `vector<vector<int>> v;`
parses with `>>` a single token in the grammar, because tree-sitter's lexer
only offers the tokens the state accepts.

## The fourth language: structure by column

`spike/pyish/` is the one the design note's §5 said had no declarative form
yet: blocks by indentation. It does have one. Spoofax's layout-sensitive
SDF3 states it in four constraint kinds, and `pyish.sdf3` uses them and
nothing else: `align-list 1` on a statement list (every element starts a
line at one column), `indent 1 4` on `Stmt.If` (the block is deeper than
the `if`), `align 1 5` (`else` sits at the `if`'s column), and `offside 1 2
3` on a simple statement (a deeper next line continues it). No NEWLINE, no
INDENT, no DEDENT: the module says what the layout means and the lowering
derives the mechanism. `src/scanner.rs` turns the constraints into an
**indent-stack scanner** (160 lines of generated C, with the stack
serialized so incremental parsing works): a wrapped `_indent .. _dedent`
around every indented occurrence, a `_newline` terminator on every
production of an aligned sort that does not already end in a block, and a
scanner that emits `_indent` when the next line is deeper and the parser
can open a block, nothing when it is deeper and cannot (the offside rule),
`_newline` at the open column, and one zero-width `_dedent` per column the
next line has left, refusing a column no open block has. That is the shape
of tree-sitter-python's hand-written scanner, derived.

Thirteen expectations from the SDF3 semantics, three of them errors;
thirteen hold. The findings name two places tree-sitter widens SDF3 and
lands on Python's own behaviour: the offside rule applies to every aligned
element whether or not its production declared it, and inside brackets a
line break is layout at any column (Python's implicit line joining, which
`offside` rejects). Under ANTLR the same module gives 10 of 13: the lexer
keeps the same stack but cannot ask the parser whether a block may open, so
the emitter derives the **opener literals** (the literal before each
indented symbol, `:` here) and a deeper line opens a block only after one.
The bracket case is the miss worth having: ANTLR rejects it, as SDF3 does,
and tree-sitter accepted it.

## Bindings, next to the rules that create them

SDF3 has no binding attributes; Spoofax keeps name binding in a separate
language (NaBL2, then Statix). The design note's §5 wants a binding beside
the rule that creates it, so pyish carries three attributes that are a
treebank extension: `scope(function)` on `Stmt.Def` and `scope(module)` on
`Program`; `binds(target -> enclosing)` on `Stmt.Assign`, `binds(name ->
enclosing as function)` on `Stmt.Def`, `binds(name -> enclosing as
parameter)` on `Param.Param`, `binds(names -> module)` on `Stmt.Global`
(the note's own example: a binding that reaches past every enclosing
scope); and `refers(1)` on the injection `Exp = ID`. `src/bindings.rs`
lowers them to **`bindings.json`** — scopes, definitions keyed on (node,
field), references, and the `_scope`/`_binding` facet memberships
`roles.json` would carry, derived — and to **`queries/locals.scm`** in
treebank's locals vocabulary, with a finding at each of the two places the
query dialect cannot say what the data says (it cannot name the module
scope, and it files a scope node's own name under that node).

The check is against an oracle that is not ours: `tools/bindings_check.py`
parses each program under `spike/pyish/bindings/` with the generated
parser, applies `bindings.json` to the tree, resolves every name the way
the data says, and compares the per-scope classification (parameter,
local, free, global) with CPython's `symtable`. Six programs — module
names, `global`, a closure's free variable, shadowing, a forward
reference, control flow — and six agree, name for name.

## When a binding takes effect: Rust and JavaScript

Python binds a name for its whole scope, so pyish never had to say *when*
a binding takes effect. Rust's `let x = x + 1;` reads the previous `x` and
shadows it from the next statement on; JavaScript's `var` binds in the
enclosing function whatever block it sits in, while `let` binds in its
block. `spike/rustish/` and `spike/jsish/` add exactly two words to the
model: a binding's **effect**, `whole` (the default: visible throughout
the scope, and several such bindings of one name in one scope are one
slot) or `after` (from the end of the binding node onward, each a new
slot), and a target named by **scope kind**, `binds(name -> function)`.
The resolution rule in `bindings.json` is one sentence: a reference
resolves to the slot of its name with the latest start at or before it,
in the nearest scope that has one, outward.

The oracle is the real toolchain. `tools/resolve_check.py` resolves every
name from `bindings.json` alone, evaluates the program with an
interpreter that knows integers, arithmetic, calls and prints and nothing
about scope, and compares what it printed with what rustc's binary, node
or python3 prints for the same file. Five Rust programs (shadowing chains,
a `fn` item called before its line, a parameter shadowed twice, blocks as
expressions, an initializer reading the previous binding), five
JavaScript programs (`var` hoisting through a block, `let` per block, two
temporal-dead-zone errors, a hoisted function, `var` over a parameter)
and two Python programs (an `UnboundLocalError`, `global`) all print, or
fail, as the toolchain does: twelve of twelve.

## The shared vocabulary, from the module

treebank's grammars share a closed vocabulary (`notes/DESIGN.md` §3): 22
table-tier terms that are real supertypes (`_statement`, `_branch`,
`_body`, …) and 7 facets shipped as `roles.json`, checked by `treebank
roles` in CI and by the rosetta suite across languages. A `vocabulary`
section, a treebank extension, binds terms to a module's sorts and
constructors:

```
vocabulary
  _statement    = Stmt
  _branch       = Stmt.If
  _control_flow = _branch _loop _jump
  _body         = Block
  _clause       = Else
```

`src/vocab.rs` decides the tier per term, as §3.1.1 says a grammar must. A
term bound to a whole sort **renames** that sort's supertype; a term bound
to constructors, single-constructor sorts, tokens or other terms
**threads** a new supertype through every reference to a member, so
`_branch` nests inside `_statement` where `if` stood and `_body` wraps
`block` in every field that held it, with the tree unchanged; a facet goes
to `roles.json`, and three facets are derived from what the module already
says (`_scope` and `_binding` from the binding attributes, `_callable`
from a function binding, `_comment` from LAYOUT); a term that would give a
node two derivations is **demoted** with the reason written for it, where
the vocabulary allows, and refused where it does not. Every node left
uncovered is ledgered as uncategorised with a reason that says which
covered node it is a piece of.

Three checks, none of them written for the spikes. `examples/roles.rs`
runs `treebank::check::check`, the code behind `treebank roles`, over each
spike's generated `node-types.json` and lowered `roles.json`: pyish and
rustish carry 14 of the 22 table-tier terms as supertypes, jsish 13, each
with five facets and one uncategorised token. `tools/rosetta_check.py` is
the rosetta gate over the spike languages: three cases under
`spike/rosetta/` with the same program in pyish, rustish and jsish, and
20 of 20 role queries yield the same count in all three. The gate earned
its keep on the first run: the modules had put `let` in `_declaration`,
Python's `prefix = name` is not one, and the shipped grammars agree with
Python, so the modules were corrected.

## The second backend: ANTLR

`src/antlr.rs` lowers the same modules to ANTLR4 grammars (`<Name>.g4`,
Python3 target), with the node names the tree-sitter lowering chose so one
corpus serves both, and `tools/antlr_check.py` generates the parser and holds
it to that corpus, writing `antlr-results.md` beside each spike. Sorts become
rules and constructors labeled alternatives — ANTLR's own supertype/subtype
split — priority chains become alternative order in left-recursive rules,
and layout constraints become lexer token variants with lexer predicates,
from the same plan as the generated scanner.

Across the three spikes, 23 of 29 expectations hold under ANTLR, and every
miss is one of three capability differences the design's table predicted
and the runs now measure: the ANTLR lexer cannot ask the parser what is
valid, so spacing decisions that tree-sitter's scanner settled by validity
are rejected (`(a+b) -1`, `z=-1`, `foo((1))`); it lexes without parser
state, so `>>` is one token and closes no template; and trivia goes to the
hidden channel, so comments are absent where tree-sitter shows extras. One
fact was established by a four-line experiment on the way and is recorded:
ANTLR consults a left-edge semantic predicate during prediction in a plain
rule and not in a left-recursive one, which is why parser predicates could
not carry Ruby's spacing rule.

```sh
pip install antlr4-python3-runtime==4.13.2   # the tool jar is fetched on first use
python3 crates/treebank-sdf3/tools/antlr_check.py crates/treebank-sdf3/spike/rubyish
```

## What it is not

Not a grammar crate. There is deliberately no `grammar.js` at this crate's
root, because a `grammar.js` under `crates/treebank-*/` is the repository's
definition of a shipped grammar (`tools/wasm-pack/list-grammars.sh`,
`treebank status`, the site build). The generated parsers live under
`spike/mini/`, `spike/rubyish/`, `spike/cppish/` and `spike/pyish/`, and
nothing gates on them but their `verify.sh` scripts.
