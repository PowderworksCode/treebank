# The meta-grammar — one source, many parsers

Status: proposal, and a decision already taken. This note designs the
language treebank grammars will be written in, and the compiler that lowers
one of them to tree-sitter today and to something else later. It supersedes
nothing yet; `notes/DESIGN.md` remains authoritative until the first grammar
is lowered from this source and passes its gates unchanged.

The one-sentence version: **a grammar is a description of a language plus a
set of named decisions about its ambiguities; the description is portable,
the decisions are declared as intents rather than as one machine's
primitives, and a backend that cannot implement an intent says so at compile
time instead of being quietly approximated.**

## 1. Why, and why now

The case is not portability insurance. It is that **the next thousand
grammars will be written by agents**, and the eleven written by hand cost a
month of measured incidents to get right — incidents recorded in
`notes/field_guide.md` as ten sections of rules, every one of them about
tree-sitter's specific machinery. An agent writing grammar number 400 will
rediscover §2's fork budget the expensive way, or not at all.

A meta-grammar changes what an agent is asked to produce. Instead of "write
a tree-sitter grammar, and by the way here are ten rules about GLR fork
lifetimes that you must hold in your head", the ask becomes "describe this
language and name its ambiguities". The field guide stops being advice the
author must remember and becomes **lowering strategy the compiler applies** —
§7's sequence shape, §5's reserved words, §4's one-owner-per-spelling, §3's
precedence-versus-conflict interaction. Those are not language facts. They
are what tree-sitter needs done to language facts, and a compiler can do them
every time.

The second reason is the one that shows up in this repository already. Three
artifacts per grammar are hand-maintained restatements of things a compiler
could derive: `roles.json`'s `demoted` map (which vocabulary terms had to
drop to the facet tier, and why), `lint_policy.toml`'s baselines (37
unreserved keywords, listed in a comment under a frozen integer), and the
`treebank: only-if` conditionals in `queries/locals.scm`. Each exists because
the grammar source cannot be asked. A meta-grammar can be asked.

Tree-sitter is an interface. It is a good one, its supertypes are what make
the vocabulary expressible at all (`notes/DESIGN.md` §2), and it stays the
first-class backend. It is not the point.

## 2. What is portable, measured

Splitting the per-grammar material into what encodes tree-sitter's physics
and what encodes the language:

| | bytes |
|---|---|
| `grammar.js` + `src/scanner.c` across 11 grammars | 679,149 |
| `roles.json`, `node_map.json`, `field_map.json`, the policies, `ledger.toml`, fixtures | 661,489 |

**49% of what a grammar crate carries is already backend-neutral**, before
counting 220 MB of corpus locks, the oracle crate, and a check suite that is
barely coupled at all: 10,766 lines across `crates/treebank-cli/src/` contain
roughly thirty `tree_sitter` references, nearly all of them "make a parser,
parse a file, walk nodes". `status.rs` (1,740 lines) and `lint.rs` (378) have
none. Only `incremental.rs` — which tests tree-sitter's reparse contract —
and `lint.rs` — which reads `grammar.json` — are genuinely backend-semantic.

This is the reason the project is affordable. The meta-grammar has to replace
the 679 KB. The other half, and the 220 MB, and the checks, port as they
stand.

## 3. The one hard problem

Parsing algorithms have incompatible physics, and every honest attempt at a
portable grammar language has died on it. tree-sitter is GLR with dynamic
precedence, a fork budget of six, and an external scanner that can read
parser state through `valid_symbols`. ANTLR is ALL(*): unbounded lookahead,
no forking, automatic left-recursion rewriting, semantic predicates, lexer
modes. An LR(1) generator has none of either. A PEG resolves by ordered
choice and cannot express genuine ambiguity at all.

A source expressive enough to carry tree-sitter's tuning *is* tree-sitter's
model with extra steps. A source abstract enough to target all four cannot
carry the tuning, and the tree-sitter output regresses from what is
hand-written today — which throws away the month.

**The resolution: do not express the mechanism. Express the decision.**

`notes/field_guide.md` §1 already ranks decisions rather than mechanisms —
the lexer decides, or factoring dissolves it, or static precedence resolves
it, or the parser carries both readings. That ladder is not tree-sitter
vocabulary; it is a taxonomy of *what kind of ambiguity this is*, and every
backend has a way to implement each rung, or a documented inability:

| declared intent | tree-sitter | ANTLR | LR(1) | PEG |
|---|---|---|---|---|
| `precedence` / `assoc` | `prec`, `prec.left/right` | precedence climbing | `%left` / `%right` | ordered choice |
| `prefer` (one reading always wins here) | higher explicit `prec` | rule order | explicit precedence | ordered choice |
| `lexical` (the lexer decides, given parser state) | external scanner | lexer mode + predicate | lexer hack | semantic predicate |
| `carry` (genuinely ambiguous until later context) | declared conflict + `prec.dynamic` | **unavailable** | **unavailable** | **unavailable** |
| `predicate` (user code decides) | **unavailable** | semantic predicate | action | semantic predicate |

Each backend declares which intents it implements. A grammar declares which
backends it targets. Using `carry` in a grammar that targets ANTLR is a
**compile error naming the decision, the backend and the missing capability**
— not a silent approximation, and not a global ban on `carry` for everyone.
C++'s `<` stays a `carry` decision, tree-sitter keeps lowering it to a
declared conflict with a weight, and the ANTLR target for C++ simply does not
exist until somebody writes a `predicate` alternative for that one decision.

This is `crates/treebank-oracle/src/capabilities.rs` applied to backends
rather than to reference toolchains, and that file already argues the ethic:
"`None` is a real answer as long as it comes with the sentence saying why,
because the alternative is a check that silently compares against nothing."

## 4. The shape of the language

Five layers, each with a different portability story.

```
lexical      tokens, keywords, reserved sets, trivia, scanner contracts
syntax       productions, fields, ambiguity decisions
tree         node kinds, roles, aliases — the contract consumers see
semantic     bindings, scopes, references
evidence     oracles, corpora, fixtures, policies, ledger
```

Three properties hold throughout.

**Data with reified abstraction, not a program.** The article at
cubix-framework.com is right that a grammar written in a general-purpose
language loses its structure at generate time: python's twelve-row operator
table becomes 4,664 bytes of JSON holding thirteen near-identical
alternations with no trace of the table. The fix is not to ban abstraction —
C's `preprocessor(word)` factory and `preprocIf(suffix, content, prec)` are
what make a 1,775-line grammar writable. The fix is to make abstraction a
**declared, named, non-Turing-complete construct that survives into the
compiled artifact**. A `commaSep1` site compiles to its expansion *and*
records `{"expand": "commaSep1", "args": [...]}`. Backends read the
expansion; tooling reads the reference. If the meta-grammar ever needs
arbitrary computation, the design has failed and should be cut back.

**Everything about a rule lives in the rule.** Productions, fields, roles,
binding semantics, oracle mapping, examples, and the note explaining the
shape are one block. The sidecars (`roles.json`, `node_map.json`,
`field_map.json`, `bindings.json`) become *compiler output*, still checked
against the oracles exactly as today, but no longer separately authored and
separately drifting.

**Nothing is inferred that could be declared, and nothing is declared that
could be derived.** The tier split is the worked example — see §6.

## 5. Surface

A sketch, not a specification. The canonical form is JSON; this is the face
humans and agents write.

```
grammar python {
  versions   "2.7 ∪ 3.x"
  vocabulary 0.2.0
  targets    tree-sitter, recognizer
  word       identifier
  trivia     comment, line_continuation
}

rule for_statement : _statement, _control_flow, _loop {
  syntax   'for' left:_pattern 'in' right:_expression_list ':' body:_body
           else:else_clause?
  binds    left -> enclosing_scope
  oracle   cpython.ast = For, AsyncFor
  example  "for x in y:\n    pass\n"
}
```

Operator families are data, which is the article's complaint answered
directly — and it reads as what it is, a precedence ladder:

```
operators binary_expression {
  operand  _primary_expression
  field    operator
  bitor    left  '|'
  bitxor   left  '^'
  bitand   left  '&'
  shift    left  '<<' '>>'
  plus     left  '+' '-'
  times    left  '*' '/' '//' '%' '@'
  power    right '**'
}
```

Ambiguity decisions are named, exampled, and carry the reason — which is
today's best commentary promoted from a comment to a checked construct:

```
decision pair_vs_command_argument {
  intent   prefer pair
  over     _argument
  example  "x = y.merge \"a\" => b"
  because  "parse.y makes the pair reading unconditional here: a match value
            is an arg and a command is not"
}

decision template_argument_list {
  intent   carry                       # needs a symbol table; no lexing decides
  between  template_instantiation, comparison_chain
  example  "a < b > c"
  because  "whether `a` names a template is not a syntactic fact"
}
```

Bindings sit next to the rule that creates them (§5 of this note's motive),
and scope rules are declared, not assumed — because a binding's scope is
routinely *not* its nearest enclosing scope:

```
rule global_statement : _statement, _directive {
  syntax  'global' names:identifier+
  binds   names -> module_scope        # reaches outward, past every enclosing scope
}
```

Externals declare a contract even though the implementation is native code:

```
external _indent {
  intent    lexical
  owns      indentation
  zero_width
  recovery  decline
  state     indent_stack: u16[32]
  impl      tree-sitter "src/scanner.c#INDENT"
  because   "column-driven structure; no declarative form covers it yet"
}
```

## 6. Roles: the tier becomes a lowering decision

Today a grammar hand-declares which vocabulary terms it had to demote to the
facet tier, with a hand-written reason, and `treebank roles` checks the
declaration is *permitted*. `crates/treebank-python/roles.json` carries one
such entry for `_parameter`, arguing from python's ordered parameter list.

In the meta-grammar, role membership is declared once per rule as plain
type-level fact (`rule parameter : _parameter, _binding`). The **compiler**
determines the tier, because it can: `DESIGN.md` §2's four facts are
mechanical. Fact 3 — overlapping membership at one position is a hard error —
is a computation over the productions. Fact 1 — an unreferenced supertype is
silently pruned — is a reachability check. The demotion condition from §3.1.1
(every member is a concrete type occurring nowhere else) is decidable from the
same data.

So the compiler emits the table-tier supertypes it can prove safe, demotes
the rest, and **writes the reason itself**, from the partition it found. That
is one class of hand-maintained argument removed from every future grammar,
and it is the strongest single piece of evidence that the meta-grammar is not
just portability insurance: it makes today's work smaller.

The same mechanism subsumes `queries/locals.scm`'s `treebank: only-if _parameter`
conditionals — the compiler knows which terms a grammar carries.

## 7. Backends

**tree-sitter** stays first-class and is the acceptance target (§8). It
implements `precedence`, `prefer`, `lexical`, `carry`; not `predicate`.

**A reference recognizer** is the second backend and should be built early
and deliberately cheap: an Earley or GLL recognizer in Rust, slow, correct,
no tuning knobs, implementing every intent trivially because it carries all
readings anyway. Its value is not production use. It is that **an abstraction
with one implementation is not an abstraction** — the recognizer is what
proves a meta-grammar means something independent of tree-sitter, and it
doubles as a differential oracle: where the recognizer and the tree-sitter
parser disagree about a file, one of the two lowerings is wrong, and that is
a check no current gate can perform.

**ANTLR, LALRPOP, others** come later and on demand, and each arrives as a
lowering plus a capability declaration. A grammar using an intent the backend
lacks fails loudly for that backend alone.

What does *not* port, and should not be pretended: external scanners. Eight
of the eleven grammars have one, totalling 4,332 lines of C, and ruby's 35
externals and yaml's 22 are state machines coupled to parser state. The
contract ports (§5); the implementation is written per backend or the backend
does not support that language. Note the shape of the long tail here — c,
java and zig declare **zero** externals, rust three, typescript two. The
escape hatch is for the hard languages, not the common case.

## 8. Bootstrap, and the acceptance test that already exists

The meta-grammar is done when it reproduces the eleven, gate for gate. Not
approximately: the same sweep numbers, the same `shape` results, the same
negative corpus verdicts, the same kind budgets, and a parse-table state
count inside the existing `lint_policy.toml` ratchet. That is an unusually
strong forcing function and it costs nothing to adopt, because every one of
those gates is already written and already green.

**Order the bootstrap hardest-first**, against every instinct:

1. **ruby** — 35 externals, 1,123 lines of scanner, 60 declared conflicts,
   and the source of most of the field guide. If the decision vocabulary
   cannot express ruby's `/`, its `do`-binding weight and its `:decl=` versus
   `:decl=>` lexing, the design is wrong and it is worth knowing in week one.
2. **yaml** — structure decided by columns, 22 externals and **zero declared
   conflicts**: every decision in that language is made in the scanner, which
   makes it the sharpest test of whether the external contract is expressive
   enough to be worth having. It also carries §8's hardest lesson — a scanner
   that tracks position must own every token that can begin a line.
3. **c++** — grammar inheritance and 71 declared conflicts, the highest in
   the repository, most of them the genuine `carry` of `a < b > c`. Tests
   whether the meta-grammar can express "this grammar extends that one"
   without the 385 KB flattening.

python, rust and typescript are the easy ones and prove nothing; do them
fourth, fifth and sixth as regression insurance. A design validated on python
first will be wrong.

## 9. Risks, and what each is bought off with

**Lowest-common-denominator decay** — the meta-grammar drifts toward what
every backend supports, and the tree-sitter output gets worse. Bought off by
per-backend capability declaration: intents are never removed for everyone
because one backend lacks them, and the tree-sitter lowering is measured
against today's tables by the §8 ratchet.

**The meta-language becomes a worse JavaScript** — it grows conditionals,
then variables, then functions. Bought off by the reified-abstraction rule in
§4: parameterized rules are declared constructs with names, and there is no
general computation. If a grammar needs an `if`, the answer is a new declared
construct, reviewed once, not an escape into a host language.

**The abstraction is never paid for because the second backend never
arrives.** Bought off by building the reference recognizer first and cheaply
(§7). It is a week or two, not a quarter, and it earns its keep as a
differential oracle regardless.

**Regression against a month of hand-tuning.** Bought off by §8 — the eleven
grammars and their gates are the acceptance suite, and a lowering that cannot
reproduce them is not finished.

**It eats the roadmap.** This is the real one. Bought off only by sequencing:
the recognizer and the ruby lowering are the two spikes that answer whether
the design works, and neither requires committing the other grammars. If ruby
cannot be expressed, the note is filed and the eleven continue as they are.

## 10. Order of work

1. **The decision vocabulary**, written against the incidents already
   recorded — walk `notes/field_guide.md` §§1–8 and
   `crates/*/lint_policy.toml` and classify every real decision in the eleven
   grammars into intents. If the closed set does not cover them, it is the
   wrong set. This is a reading exercise, costs days, and de-risks everything
   after it.
2. **The reference recognizer**, so the abstraction has two implementations
   from the start.
3. **ruby**, end to end, against its existing gates.
4. **yaml** and **c++**, for the external contract and inheritance.
5. The compiler's derived artifacts — role tiers with computed reasons,
   `bindings.json`, the externals contract — replacing the hand-maintained
   sidecars one at a time, each landing on its own.
6. The remaining eight grammars.
7. A second real backend, chosen by whoever asks for one first.

Nothing here is a big bang. Step 5 is useful with no backend but tree-sitter;
step 2 is useful with no meta-grammar at all.
