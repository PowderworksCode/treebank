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

**Where the table over-promises, and what closes the gap.** A cell saying
`carry` → "declared conflict + `prec.dynamic`" is a lowering, not a
guarantee, and treating it as one would reproduce the exact failure §2 of the
field guide exists to prevent. Three of the hardest facts in this repository
are *resource* and *interaction* facts that no intent can carry:

- **The fork budget is a measurement, not a declaration.** tree-sitter culls
  beyond six live versions, so a `carry` is only sound if the losing fork
  dies within a token or two. Whether it does is discovered by parsing with
  `--debug` and counting `version_count`, not by reading the grammar.
- **Precedence does not compose with conflicts, in both directions**
  (field guide §3). A declared conflict switches static precedence off in the
  cells it covers, and an associativity added elsewhere can silently resolve
  a cell so a declared conflict never forks at all. Lowering `precedence` and
  `carry` independently and hoping is how the ruby `do`-binding bug happens
  again.
- **A `prefer` needs a total order.** Backends want integers; the intent
  gives a partial order over pairs. Computing a consistent assignment, and
  failing loudly when the declared preferences are cyclic, is real compiler
  work.

So the lowering is not complete until it can *check itself*. Each backend
lowering owes a **post-condition it verifies against its own generated
artifact** — for tree-sitter: every `carry` actually forks (parse the
decision's `example` with `--debug`, assert `version_count` leaves 1), every
`carry` carries a weight, and every fork's loser dies inside the budget.
That is `treebank lint` promoted from smell detector to compiler back end,
and it is the reason the `example` field on a decision is mandatory rather
than decorative: it is the input the post-condition runs on.

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
   after it. Read SDF3's disambiguation filters first (§11): that set is the
   prior state of the art and the right thing to be measured against, in
   either direction.
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

## 11. Prior art, and what is actually new here

Nearly every idea above has been built before, most of it in one research
lineage, and the design should be read as a re-selection from that work
rather than an invention. Where a wheel exists, take the wheel.

### The direct ancestor: SDF and the Spoofax lineage

**SDF** (Syntax Definition Formalism — Heering, Hendriks, Klint and Rekers,
1989; **SDF2** in Visser's 1997 thesis; **SDF3** in Spoofax today) is the
closest thing to this proposal that has ever shipped, and it got there
thirty-odd years ago. Its central move is §3's move: **productions are
written without disambiguation, and disambiguation is declared separately**
as priorities, associativity, reject productions, follow restrictions and
preference attributes. That separation is exactly "express the decision, not
the mechanism", and SDF pairs it with scannerless GLR so the lexical and
syntactic layers are one formalism rather than two — which is a strictly
more honest answer to §7's scanner problem than an escape hatch.

Read SDF3 before writing a line of the decision vocabulary. If our closed
set of intents cannot express what SDF's disambiguation filters express, ours
is probably too small; if it needs something SDF lacks, that is worth knowing
precisely.

The same lineage answers §6 and the bindings layer too:

- **NaBL** (Name Binding Language — Konat, Kats, Wachsmuth and Visser, SLE
  2012) is a declarative DSL for name binding and scope rules, co-located
  with the syntax definition. It is the artifact `bindings.json` wants to be.
- **Scope graphs** ("A Theory of Name Resolution" — Néron, Tolmach, Visser
  and Wachsmuth, ESOP 2015) are the theory underneath, and they handle
  precisely the cases §5 of this note flags as the hard ones: imports,
  `global`/`nonlocal` reaching past enclosing scopes, shadowing, and
  visibility that is not lexical containment. **NaBL2** and **Statix** are
  the later, more expressive versions.
- **Rascal** and the older **ASF+SDF Meta-Environment** are the same group's
  general transformation systems over that base.

The honest reading of this lineage is a caution as much as an endorsement.
It is academically excellent, it solved these problems properly, and it did
not win. ANTLR and tree-sitter took the ecosystem on ergonomics, tooling and
approachability, not on expressiveness. That is the strongest argument in
this note for keeping the meta-grammar boring, keeping the tree-sitter output
first-class, and never asking a grammar author to learn a theory before
writing a rule.

### The scanner problem has a formalism

§7 treats external scanners as an escape hatch. Two bodies of work suggest
that is more pessimistic than necessary:

- **Data-dependent grammars** (Jim, Mandelbaum and Walker, POPL 2010, and
  the **Yakker** generator; later **Iguana**, Afroozeh and Izmaylova) extend
  context-free grammars with parameters, variable binding and constraints —
  enough to express length-prefixed data, heredocs and other "the parse
  depends on what was just read" constructs *declaratively*. Ruby's heredoc
  queue and bash's are the motivating shape.
- **Layout-sensitive parsing** (Erdweg, Rendel, Kästner and Ostermann, SLE
  2012; Adams, "Principled parsing for indentation-sensitive languages",
  POPL 2013) gives indentation and column-driven structure a grammar-level
  treatment rather than a scanner-level one. That is python's indent stack
  and yaml's column tracking — 1,686 lines of the 4,332 in this repository.

If either formalism covers those two, the escape hatch shrinks from "eight
of eleven grammars" to a genuine long tail, and §8's yaml bootstrap is the
place to find out.

### Grammars as data, and grammar-to-grammar conversion

The exact thing the user asks — write it once, emit ANTLR — has a literature:

- **Ralf Lämmel's grammarware programme**, especially "Semi-automatic Grammar
  Recovery" (Lämmel and Verhoef, 2001) and the **Software Language Processing
  Suite**, whose **BGF** (a BNF-like grammar format) and **XBGF** (a set of
  grammar transformation operators) are grammars-as-data with a checked
  algebra of edits over them. **Grammar Zoo** (Zaytsev) is the corpus of
  grammars extracted into that format.
- Its finding is the sobering one and matches §3: converting the *productions*
  between notations is largely mechanical, and converting the *disambiguation*
  is not, because it is where each generator's semantics live.

### Adjacent, and worth reading for one idea each

- **DMS Software Reengineering Toolkit** (Semantic Designs) — commercial, GLR,
  dozens of languages, and the one system that took **pretty-printing
  seriously as a co-declared artifact**: a grammar rule carries its
  prettyprinter box layout. That is the answer to the roundtripping gap the
  cubix-framework article names and this repository still has.
- **JastAdd** (reference attribute grammars; ExtendJ) and **Silver/Copper**
  (Van Wyk et al.) — attribute grammars are what the semantic layer is, and
  JastAdd's *reference* attributes are how name binding is done well.
- **Xtext** and **Langium** — one grammar producing parser, AST and a full
  language server. The realistic model for what layer 3 emits, and evidence
  that the "one source, many consumer artifacts" half is routine engineering.
- **Ohm** (Warth et al.) — deliberately separates the grammar from its
  semantic actions so one grammar carries many semantics. The same instinct
  as the tree contract.
- **ANTLR's multi-target backends** — the same `.g4` emits runtimes in Java,
  C#, Python, Go and more. Worth naming precisely because it is portability
  along a *different axis*: one parsing algorithm, many host languages. It is
  not evidence that one grammar can span algorithms.
- **Cubix** (Koppel, Premtoon and Solar-Lezama, OOPSLA 2018) — the incremental
  parametric syntax whose article started this thread. Its vocabulary idea is
  ours; note that it **wrapped existing per-language parsers rather than
  generating them**, which is the road this note declines to take.

### What is actually new here

Not the meta-grammar. Not declarative disambiguation, not scope graphs, not
grammars-as-data — all of it exists, most of it done well.

What does not exist in that lineage is **the evidence apparatus**: 220 MB of
locked corpora, reference-parser adjudication per language, span and field
oracles, mutation and fuzz in both directions, ratchets with reasons. SDF's
disambiguation filters are declared and checked for internal consistency;
they are not checked against what CPython does to 296,567 files. That is the
part this repository already has and the lineage does not, and it is what
would make a meta-grammar here trustworthy rather than merely elegant.

So the borrowing is asymmetric and should be deliberate: **take the
formalisms, keep the evidence.**

## 12. What this design does not yet answer

Named here rather than discovered later.

1. **Error recovery has no portable story.** `treebank recovery` measures
   blast radius, tree-sitter recovers by design, and an Earley recognizer
   does not recover at all. So the §7 reference recognizer is a differential
   oracle for accept/reject and for tree shape on *valid* input only — it
   cannot check the recovery gate, and the note should not imply it can.
2. **Incremental reparse is backend-semantic and stays that way.**
   `incremental.rs` tests a tree-sitter contract. Either the gate becomes a
   per-backend capability like the oracle ones, or it stays a tree-sitter
   gate that other backends simply do not have.
3. **The meta-language needs its own version, and its own bootstrap story.**
   `vocabulary 0.2.0` versions the role terms; nothing yet versions the
   surface syntax or the intent set. Self-hosting — parsing the meta-grammar
   with a treebank grammar written in it — is the obvious forcing function
   and is how SDF and Rascal do it.
4. **`because` fields are unchecked prose**, which is against this
   repository's ethic everywhere else. The `example` on a decision is the
   part that can be executed; §3's post-conditions are what make it load
   bearing, and the design should say that a decision without an example does
   not compile.
5. **No cost estimate.** §9 says it eats the roadmap and §10 sequences around
   that, but the spikes should return a number before the remaining eight
   grammars are committed to.

## 13. The spike: SDF3 to tree-sitter, measured

§11 recommended SDF3 and §3 named the lowering as the risk. So the lowering
was built, small, and run: `crates/treebank-sdf3` is a reader for SDF3
modules (winnow), a lowering to tree-sitter `grammar.json`, and a language
called mini — statements, blocks, functions, calls, a nine-operator
expression grammar with a four-group priority chain, comments, keywords —
written in SDF3 as Spoofax documents it (`spike/mini/mini.sdf3`). The
expectations in `spike/mini/test/corpus/mini.txt` were written from the SDF3
semantics before the parser existed, and the generated parser was held to
them.

**Result: 9 of 9 expectations hold.** Priorities nest as the chain says,
`{left}` groups associate left, the unary group outranks every binary group,
injections yield no node, separated lists expand, comments are extras, and a
template keyword cannot be a name. tree-sitter generated the grammar with
**zero conflicts** — 25 rules, 49 symbols, 124 states — and the readable
`grammar.js` the lowering also emits generates a byte-identical `parser.c`,
so the human-facing rendering is a second source rather than documentation.

Everything the lowering could not keep is in `findings.md`, and it is three
things:

| finding | what happened | kind |
|---|---|---|
| `{non-assoc}` | tree-sitter has no non-associativity; lowered to `prec.left`, so `a == b == c` parses where SDF3 rejects it | widening, ×2 |
| `{bracket}` | a hidden supertype member may have only one visible child ("Supertype symbols must always have a single visible child") and `( Exp )` has three; brackets became a named node SDF3's AST does not have | deviation, ×1 |
| `<left:Exp>` | SDF3 placeholders are positional; a label prefix is a treebank extension and lowers to a field | extension, ×30 |

One true widening, one tree-sitter constraint, one extension. Five SDF3
constructs were absorbed with nothing emitted — lexical restrictions and
`keyword -/-`, because tree-sitter's lexer is longest-match, and the LAYOUT
restriction, because extras are skipped greedily — and 18 mapped exactly.

Two things were learned about the *format* rather than the lowering. A
leading `[` is ambiguous between a square template and a character class,
and the reader resolves it by section (templates in context-free syntax,
classes in lexical); SDF3 presumably does the same and the reader should be
checked against its grammar. And SDF3's constructor names are terse because
the sort supplies context — `Stmt.If`, `Exp.Int` — where tree-sitter node
names are global, so `Exp.Int` collided with the token `INT` under
snake-case and became `exp_int`. A naming policy is a real part of the
meta-grammar's design, not a lowering detail.

What the spike does not test is the thing §3 flagged hardest: no `carry`,
no scanner, no layout-sensitive syntax, no deep priority conflict. Mini is
LR(1)-clean by construction. The next language to lower is the one that
is not.

## 14. The second spike: lexer state, from SDF3, generated

§13 ended by naming what mini could not test, and §3 named the seam where
SDF3's scannerless world and tree-sitter's lexer point in different
directions. `spike/rubyish` is that seam: the corner of Ruby where the
lexer needs the parser. `foo -1` is a command call with a negative
argument; `foo - 1` and `foo-1` subtract. The same spacing rule decides
`*` (splat against multiply), `[` (array argument against index), `(`
(parenthesised argument against call) and `/` (regex against divide).
CRuby decides these in its lexer with `EXPR_ARG` state; treebank's ruby
grammar decides them in 1,123 lines of hand-written scanner
(`notes/field_guide.md` §1, rung 1, and §4's one-owner-per-spelling).

**In SDF3 they are layout constraints.** `Exp.Neg = <-<Exp>>
{layout(1.last.col + 1 == 2.first.col)}` says the minus is adjacent to its
operand; `Exp.Command = <<ID> <Arg>> {layout(1.last.col + 1 <
2.first.col), prefer}` says the argument is separated from the method and
wins the ambiguity. That is the whole specification of the rule, on the
productions it concerns, in a formalism that has had it since 2012.

**tree-sitter's grammar cannot say it.** Its one whitespace fact is
`token.immediate` — no layout *before* this token — which cannot express
"layout required before" at all, and cannot reach into a nonterminal. So
the lowering does what treebank-ruby's author did by hand, mechanically
(`crates/treebank-sdf3/src/scanner.rs`):

1. Every constrained spelling is **split** into external tokens that share
   the spelling and differ in the layout they require: `_minus` and
   `_minus_spaced_tight`, `_lbracket_adjacent` and `_lbracket_spaced`. An
   unconstrained occurrence takes the default. A constraint between two
   nonterminals — Command's "separated" — propagates *required before* to
   the first literal of everything reachable at the start of the second
   sort, including a lexical sort opened by a literal, which is how the
   regex literal became scanner-scanned whole.
2. Each occurrence is **aliased back** to its spelling, so the tree still
   carries an anonymous `-` where SDF3 and the cubix-framework article both
   want one. `node-types.json` lists `(`, `*`, `-`, `/`, `[` as anonymous
   tokens exactly as before the split.
3. A **`scanner.c` is generated** — 103 lines — from a table of variants.
   It decides by *validity first, spacing second*: when the parser can
   accept only one variant of a spelling it emits that one whatever the
   spacing (`x = - 1` is unary because nothing else is possible), and only
   when several are valid does spacing arbitrate (`foo -1` against
   `foo - 1`). That is the `valid_symbols` discipline of §1 of the field
   guide, and `_error_sentinel` follows §8. The condition propagated from
   Command is therefore only ever consulted in the state the constraint
   exists to settle, which is what makes propagating it through productions
   also used elsewhere sound.

**Result: 12 of 12 expectations hold, written from Ruby's semantics before
a parser existed.** Zero conflicts at generate — 23 rules, 42 symbols, 54
states, 11 externals. `(a+b) -1` subtracts because after `)` no command is
possible; `x=-1` negates because after `=` nothing else is; `a / b / c`
divides twice while `a /b/` passes a regex; `foo (1)` and `foo(1)` differ
by one space and one node. The readable `grammar.js`, now carrying `alias`
and `externals`, still generates a byte-identical parser.

What `{prefer}` became is the small surprise: dynamic precedence +1 on
Command, which tree-sitter consults only inside a declared conflict — and
there is none, because the scanner split made the two readings different
tokens. The disambiguation moved from SDF3's post-parse filter to the
lexer, and the `prefer` is inert. Recorded as mapped, not absorbed, because
generate is what established it.

**What this says about §3's table.** The `lexical` intent's lowering to
tree-sitter is not "an external scanner the author writes"; for the class
of decisions that are layout facts, it is a scanner the compiler writes.
That class is smaller than ruby's scanner — heredocs, string
interpolation, `%w[]` literals and the `?a` character literal are state,
not spacing — but it is the class §1 of the field guide calls rung 1, and
it fell out of the grammar. Where the escape hatch survives, it now has a
measured boundary.

Two things about the format surfaced. Constraint indices count every
symbol of the production, literals included, and the `+ 1` arithmetic is
the shape the Spoofax documentation shows; both should be checked against
SDF3's own grammar before adoption is called settled. And a scanner-owned
lexical sort must be opened and closed by single-character literals, which
is a limit of this generator, not of the formalism.

Not tested, still: `carry` (C++'s `<`), layout-sensitive *structure*
(python's indent stack, yaml's columns) as opposed to layout-sensitive
*tokens*, and stateful scanning. The next two languages are those.

## 15. The third spike: carry, and composition

Two of the three things §14 left untested were one language: `carry`, and
extending a grammar without flattening it. `spike/cppish` is C plus the
one thing that makes C++ hard to parse. `a < b > c;` is either the
expression `(a < b) > c` or a declaration of `c` with type `a<b>`, and
nothing short of a symbol table decides it. treebank-cpp's ledger records
the same choice this spike makes: it carries the ambiguity in *type*
position as declared conflicts, and cut template arguments in *expression*
position (`f<int>(x)`) because that form "puts `a < b` and `a<b>` in
competition at every comparison in the language."

**In SDF3 it is one production and one attribute.** `Type.TemplateId =
<<ID>\<<{Type ","}+>\>> {prefer}` — added to a sort that `cish.sdf3`
defines, in a module that `imports cish` rather than copying it. SDF3
composition is additive: a sort gains productions from every module that
declares any, nothing is overridden. That is the answer to §2's 385 KB C++
`grammar.json` at the *source* level; the generated artifact is still flat,
as it has to be.

**In tree-sitter it is dynamic precedence plus a declared conflict, and
only one of those can be lowered from the grammar.** `{prefer}` became
`PREC_DYNAMIC(+1)` on `template_id`, exactly as in §14 — but in §14 the
weight was inert because the scanner split had made the readings different
tokens. Here they are the same tokens, the LR table has a genuine
shift/reduce conflict, and tree-sitter consults the weight only inside a
conflict the grammar *declares*. Which conflict is not derivable without
constructing the table. So the lowering asks the thing that constructs it:
`--generate` runs `tree-sitter generate`, reads the conflict it names
("Add a conflict for these rules: `template_id`, `_exp`"), declares it, and
tries again until generate is satisfied. The set it settles on is pinned in
`tree-sitter.conflicts.json` beside the module — **the carry's backend
data**: the lowering is reproducible from it without the CLI, `cargo test`
holds the committed grammar to it, and a diff in it means generate's view
of the ambiguity moved, which is worth a review. That is a declared-and-
total sidecar in the sense of §4, for a fact that belongs to one backend.

**Result: 8 of 8 expectations hold.** One declared conflict, 19 rules, 32
symbols, 45 states. `a < b > c;` and `a<b> c;` are the declaration; `a <
b;` is the comparison; `a < b > c > d;` is three comparisons, because the
template reading dies when the statement keeps going; `vector<vector<int>>
v;` parses with `>>` a single token in the grammar, because tree-sitter's
lexer offers only the tokens the state accepts and after a type argument
`>>` is not one of them — the C++11 rule, at no cost to the grammar. cish's own statements
arrive through the import, and its `int` cannot be a name in cppish.

**§3's post-condition ran, and it is the reason to trust the rest.** The
note said a lowering is not complete until it checks itself: every `carry`
must actually fork, and a declared conflict that never forks is dead text
(field guide §3). `verify.sh` parses `a < b > c;` with `--debug=normal` and
`version_count` peaks at 2 for eleven steps before the weight settles it;
it parses `x = a < b > c;` and `version_count` never leaves 1, because
after `=` no declaration is possible and validity decides before any fork.
Both are asserted, not observed.

The conflict names a supertype, `_exp`. That is the "early commit between
parallel tiers" shape `treebank lint` budgets per grammar and the field
guide §2 warns about, and it is not an accident of this lowering: `id` is
a member of both `_exp` and `_type` at statement start, which is the
ambiguity. A meta-grammar lint would flag it the same way, and the budget
would be one.

What the three spikes have now covered of §3's table: `precedence` and
`prefer` (mini), `lexical` for layout facts as a generated scanner
(rubyish), `carry` as a generate-discovered, pinned, post-condition-checked
conflict (cppish), and composition by import. Still untested:
layout-sensitive *structure* (python's indent stack, yaml's columns) and
stateful scanning (heredocs, interpolation) — the two places the scanner
escape hatch is expected to survive, and now the only two.

## 16. The second backend, and what the table got right and wrong

§7 said an abstraction with one implementation is not an abstraction, and
§3's capability table made claims about ANTLR that only an ANTLR lowering
could test. `crates/treebank-sdf3/src/antlr.rs` lowers the same three
modules to ANTLR4 — same reader, same names, same corpus — and
`tools/antlr_check.py` generates the Python target and holds it to the
expectations the tree-sitter parsers were held to. The mapping is the
natural one: a sort is a rule and a constructor a labeled alternative,
which is ANTLR's own supertype/subtype split; a priority chain is
alternative order in a left-recursive rule; `{prefer}` is alternative order
within its rule; `{non-assoc}` widens as before.

**Result: 23 of 29 expectations hold across the three spikes** — 8 of 9 on
mini, 8 of 12 on rubyish, 7 of 8 on cppish — and every miss is
attributable to one of three capability differences, none of them a
lowering bug:

| difference | what it costs | cases |
|---|---|---|
| the lexer cannot ask the parser what is valid | a spacing decision that tree-sitter's scanner settled by validity is a token ANTLR has no consumer for | `(a+b) -1`, `z=-1`, `foo((1))` |
| the lexer runs without parser state | `>>` is one token and closes no template, where tree-sitter's per-state lexer offered only `>` | `vector<vector<int>> v;` |
| trivia is on the hidden channel | comments are absent where tree-sitter shows extras | both comment cases |

The first row is the finding. Ruby's spacing rule lowered to ANTLR by the
*same* planner that generated tree-sitter's scanner — the same split into
`V_MINUS` and `V_MINUS_SPACED_TIGHT`, as lexer rules with lexer predicates
on the character before and after — minus the one thing the scanner had:
`valid_symbols`. Where tree-sitter emitted the only valid variant whatever
the spacing, ANTLR's lexer emits the variant the spacing says and the
parser has to take it or fail. Eight of twelve Ruby cases survive that;
the four that do not are precisely the ones §14 credited to validity.
The corpus now names the tree-sitter case that *widened* SDF3 (`y = - 1`,
a negation Ruby accepts and the module's adjacency constraint rejects),
which the ANTLR run rejects faithfully — the first time the two backends
disagreed with each other *and* one of them agreed with the source.

**What §3's table got wrong.** It said `lexical` lowers to ANTLR as "lexer
mode + predicate" and `predicate` as "semantic predicate", and the first
attempt did exactly that: layout constraints as parser predicates
comparing token offsets, placed where SDF3 states them. They rejected
instead of steering. A four-line grammar settled why: **ANTLR consults a
left-edge semantic predicate during prediction in a plain rule and not in
a left-recursive one** — the plain rule prunes the alternative, the
left-recursive one takes it and throws at parse time — and every
expression rule is left-recursive. So the `predicate` row is narrower than
written: predicates steer only in rules the left-recursion rewrite does not
touch. The corrected lowering puts every layout fact in the lexer, where
predicates are always consulted, and that is why the deviation table above
is about lexer capabilities and not parser ones.

**What it got right.** `prefer` as alternative order held on cppish: `a < b
> c;` is a declaration under ANTLR because ALL(*) takes the first viable
alternative and `Decl` precedes `ExprStmt` in cish — by source order, which
the attribute on `TemplateId` does not reach, and the finding says so. The
`carry` row was right to say "unavailable": ALL(*) does not keep both
readings, it picks one, and where `{prefer}` names the one to pick that is
enough; where nothing does, ANTLR would choose silently. And composition,
priorities, injections, brackets and keywords crossed without incident:
mini's only miss is the comment.

The corrected table rows, as measured:

| intent | tree-sitter | ANTLR |
|---|---|---|
| `lexical` (layout facts) | generated scanner, validity first | lexer token variants with lexer predicates, no validity |
| `predicate` | unavailable | steers only in non-left-recursive rules |
| `carry` | declared conflict + weight, discovered by generate | unavailable; `prefer` by order where a preference is declared |

Two backends from one source, twenty-nine shared expectations, six
divergences each with a named cause. That is what §4 meant by a backend
declaring what it cannot do, and it was cheaper to measure than to argue.

## 17. The fourth spike: indentation, and the declarative form that does exist

§5 sketched `external _indent { ... because "column-driven structure; no
declarative form covers it yet" }`, an escape hatch for the one thing the
surface could not say. That line was wrong, and the fourth spike is the
proof. Spoofax's layout-sensitive SDF3 (Erdweg, Rendel, Kästner and
Ostermann's layout constraints, 2012; the declarative forms of Amorim,
Steindorfer, Erdweg and Visser, SLE 2018) states block structure in four
constraint kinds, and `crates/treebank-sdf3/spike/pyish/pyish.sdf3` — a
Python subset with `if`/`else`, `while`, `def`, `return`, `global`,
assignment and expressions — uses exactly those: `align-list 1` on each
statement list, `indent 1 4` on the compound statements, `align 1 5` for
`else`, and `offside 1 2 3` on the simple ones. The module contains no
NEWLINE, INDENT or DEDENT. It says what the layout *means*.

**The lowering derives the mechanism.** The reader gained the declarative
constraints (and `&&`/`,` conjunctions, and `tokenize:`, which it had been
recording and misfiling as "no parser effect" — `else:` is two tokens
because of it). The planner turns the constraints into an indent plan:
every indented occurrence is wrapped `_indent .. _dedent`; every
production of an aligned sort ends in `_newline` unless it already ends in
an indented block (walking optional trailing sorts back to the block, which
is how `if .. else?` comes out right); and the literal before each indented
symbol is recorded as a block opener for backends that will need it. The
generated scanner keeps a column stack, serialized for incremental
parsing, and decides at a line break by the next token-bearing line's
column: deeper and `_indent` valid, push and open; deeper and not,
nothing at all, so the line continues — the offside rule as a *consequence*
of validity; at the open column, `_newline`; left of it, one zero-width
`_dedent` per column left, and an error token when the column matches no
open block. Comments and blank lines are looked past and left to the
parser, so they stay extras. That is tree-sitter-python's hand-written
scanner, derived from four attribute kinds.

**Result: 13 of 13 expectations hold** — ten semantic cases (blocks, nested
dedents, `else` alignment, offside continuation and separation, comment
lines inside blocks, trailing comments, `def`/call) and three errors
(dedent to no open column, `else` off its `if`, an opener with nothing
indented). The findings say where the lowering is not SDF3, and both
widenings land on Python's own behaviour rather than anywhere arbitrary:

| finding | kind | why |
|---|---|---|
| the offside rule applies to every aligned element, declared or not | widening | the scanner ends an element by the next line's column alone |
| inside brackets a line break is layout at any column | widening | no `_newline` is valid there, so the scanner is never asked: Python's implicit line joining, which `offside 1 2 3` rejects |
| the outermost list is aligned at column 0 | deviation | SDF3 aligns it at its first line's column; CPython does this |
| a tab is one column | deviation | tree-sitter's column count; CPython uses tab stops of eight |

One bug in the earlier spikes surfaced on the way: a single-constructor
sort collapsed to a rule named for the *sort*, so `Else.ElseClause` became
`else` and collided with the keyword. SDF3's AST node is the constructor;
it is `else_clause` now, on both backends.

**ANTLR: 10 of 13**, and the shape of the misses is the capability table
again. The emitter gives the lexer the same indent stack (a token queue
behind `nextToken`, as CPython's tokenizer and the grammars-v4 Python
grammar do) and the parser rules are wrapped and terminated exactly as
tree-sitter's. What the lexer lacks is `valid_symbols`: it cannot ask
whether a block may open here. So the emitter derives the **opener
literals** from the grammar — the literal immediately before each indented
symbol, `:` in pyish — and a deeper line opens a block only after one of
them, continuing the statement otherwise. Two misses are the hidden-channel
comments, as before. The third is the bracket case: ANTLR's lexer emits the
newline inside the parentheses and the parser rejects, which is what
`offside 1 2 3` says and what tree-sitter's lowering widened past. The
second time the backends disagree and the one without validity is the one
that agrees with the source. (One target quirk, recorded in the emitter:
ANTLR's Python lexer exposes no constant for a `tokens {}` declaration, so
`H_INDENT` and `H_DEDENT` are lexer rules on control characters no source
holds.)

The table row this adds:

| intent | tree-sitter | ANTLR |
|---|---|---|
| `lexical` (block structure by column) | generated stateful scanner, stack serialized; block-or-continuation by validity | lexer indent stack with a token queue; block-or-continuation by derived opener literals |

**What it changes in the design.** §5's `external _indent` block, with its
`state indent_stack: u16[32]` and its `because`, was the design admitting a
hole. The hole is filled from above: the four constraints are the source,
the `external` is generated output (`uint16_t cols[64]` in the scanner,
`_stack = [0]` in the lexer), and the reference recognizer of §4 can
implement the constraints directly, since they are constraints on columns
and nothing else. The bootstrap order of §8 — hardest first — now has
four data points instead of a sketch: lexer state (rubyish), carry
(cppish), composition (cppish), column structure (pyish), each lowered
from the same reader to two backends.

## 18. The fifth spike: bindings beside the syntax, held to symtable

§5's second motive was co-location: `binds left -> enclosing_scope` on the
rule that creates the binding, and `binds names -> module_scope` for the
one that reaches outward. SDF3 has nothing of the kind — Spoofax's name
binding is NaBL2 and then Statix, a separate language over the AST — so
this is the first extension that adds a *dimension* to the meta-grammar
rather than a label. Three attributes, on `pyish.sdf3`'s productions:

```
Program.Program = <<Stmt*>>                 {layout(align-list 1), scope(module)}
Stmt.Assign     = <<target:ID> = <value:Exp>> {layout(offside 1 2 3), binds(target -> enclosing)}
Stmt.Global     = <global <names:{ID ","}+>>  {binds(names -> module)}
Stmt.Def        = <def <name:ID>(..):  <body:Block>>
                                            {layout(indent 1 7), scope(function), binds(name -> enclosing as function)}
Param.Param     = <<name:ID>>               {binds(name -> enclosing as parameter)}
Exp             = ID                        {refers(1)}
```

`crates/treebank-sdf3/src/bindings.rs` lowers them to two things. The
data, `bindings.json`: scope node types with kinds; definitions keyed on
(node type, field) with the name token, the target scope and the kind;
reference node types; and the `_scope` and `_binding` **facet
memberships** that `roles.json` carries by hand today, derived — §6 said
the tier is a lowering decision, and here a facet is one. And the query
view, `queries/locals.scm` in treebank's own locals vocabulary
(`@local.scope`, `@local.definition.function`, `@local.reference`), which
the pinned CLI compiles and runs (17 captures on the closure program).

**The check is against an oracle we did not write.** `tools/bindings_check.py`
parses six programs — valid pyish and valid Python — with the generated
parser, applies `bindings.json` to the tree, resolves every name the way
the data says, and compares each scope's classification of each name
(parameter, local, free, global) with CPython's `symtable`. **Six of six
agree, name for name**: module names and a function's locals; `global x`
followed by `x = 2` in the same function; a closure's free variable and a
nested `def` bound in its enclosing function; a parameter shadowing a
module name; a reference before its assignment in the same function
(local, by Python's whole-scope rule); `while`/`if`/`else` bodies that
open no scope.

Two semantics live outside the attributes, and the spike names both. The
resolution rule the checker applies is fixed and stated in
`bindings.json`'s note: a definition binds in the enclosing or the module
scope, a reference resolves outward, and a scope's module-directed
binding of a name **redirects that scope's other bindings of it** — which
is what makes `global x; x = 2` come out right without a Python-specific
line anywhere. That rule is enough for Python's whole-scope binding; a
language with order-sensitive scopes (a Rust `let` shadowing the previous
`let`, a JavaScript `var` hoisting past a `let`) would need the data model
to say *when* a binding takes effect, and §12 should carry that as the
open question it is. The other is the oracle's: `symtable` records a
function's `global x` on the module table too, and the checker's first
draft read the module's `x` as "declared global" before "local", which at
module level are the same thing. One ordering fix; the data was right.

The query dialect is a backend like any other, and it declares what it
cannot do the same way. Two findings: tree-sitter's locals engine cannot
name the module scope, so `binds(names -> module)` becomes a pattern that
binds at the nearest scope with a note pointing at the data; and it files
a scope node's own name under that node, so the `def` name carries
nvim-treesitter's `#set! definition.function.scope "parent"`, which
tree-sitter's own highlighter ignores. Both are places where the JSON is
the truth and the query is a view — the split §7's `recognizer` row was
arguing for, now with a concrete consumer on each side.

## 19. The sixth spike: when a binding takes effect

§18 left one thing open: Python binds a name for its whole scope, so
nothing in the bindings model said *when* a binding takes effect, and
Rust and JavaScript needed it to. Two more modules settle how much the
model has to grow. The answer is two words.

`crates/treebank-sdf3/spike/rustish/rustish.sdf3` is the corner of Rust
where a binding is a point in time: `let x = x + 1;` reads the previous
`x` and shadows it from the next statement on; a block is a scope and an
expression; a `fn` item is visible throughout the block that holds it,
before its line. `spike/jsish/jsish.sdf3` is the corner of JavaScript
where two keywords bind differently: `var` in the enclosing *function*
whatever block it sits in, visible as `undefined` before its line; `let`
in its block, an error before its line; a function declaration throughout
its scope. The attributes:

```
Stmt.Let      = <let <pattern:ID> = <value:Exp>;>   {binds(pattern -> enclosing after)}     -- rustish
Item.Fn       = <fn <name:ID>(..) <ret:Ret?> <body:Block>>
                                                     {scope(function), binds(name -> enclosing as function)}
Stmt.Var      = <var <name:ID> = <value:Exp>;>      {binds(name -> function)}               -- jsish
Stmt.Let      = <let <name:ID> = <value:Exp>;>      {binds(name -> enclosing)}
```

The model gains an **effect** per binding — `whole`, the default, visible
throughout the scope, with several whole bindings of one name in one
scope being one slot; or `after`, from the end of the binding node
onward, each a new slot — and a target named by **scope kind**, of which
§18's `module` was already the first instance. The resolution rule,
stated in `bindings.json`'s note, is one sentence: a reference resolves to
the slot of its name with the latest start at or before it, in the
nearest scope that has one, outward. A whole slot starts at its scope's
start; an after slot at its node's end. That is all: Python, Rust and
JavaScript are the same rule with different effects and targets.

**The oracle is the toolchain.** `tools/resolve_check.py` resolves every
name from `bindings.json` alone, then evaluates the program with an
interpreter that knows integers, arithmetic, calls and prints and
*nothing about scope* — every scoping decision comes from the data — and
compares what it printed, or that it failed, with what rustc's compiled
binary, node or python3 prints for the same file. **Twelve of twelve
programs agree**:

| language | programs | what they exercise |
|---|---|---|
| Rust (rustc 1.94) | 5 | a shadowing chain across a block (`1 11 22 11`), a `fn` item called before its line, a parameter shadowed twice, blocks as expressions, an initializer reading the previous binding |
| JavaScript (node 22) | 5 | `var` hoisting through an `if` block (`undefined 2 3 2`), `let` per block, two temporal-dead-zone errors, a hoisted function closing over a parameter, `var` over a parameter |
| Python (3) | 2 | `UnboundLocalError` on a use before assignment, `global` |

The two error cases are the sharpest: JavaScript's `let y = y + 1;` inside
a block is a ReferenceError, Python's `print(x); x = 2` in a function is
an UnboundLocalError, and both fall out of the same model — a whole-scope
slot exists for `y` in the block, the reference resolves to it, and the
slot has no value yet. Rust's version of the same line resolves to the
outer `y` and runs, because the slot is `after`. The interpreter's one
language-shaped fact is that a `var` slot holds `undefined` before its
line where a `let` slot holds nothing; that is a runtime fact about
values, not a scoping fact, and it stays out of the data.

One thing the tree-sitter locals query dialect can now be measured
against: its engine resolves a reference to the nearest *preceding*
definition in scope, which is exactly `after` and not `whole`. So the
finding on every whole-scope binding says the engine will resolve a
use-before-definition outward where the data resolves it inward — Rust's
`let` is the case the engine gets right by construction, Python's names
and JavaScript's `var` are the cases it does not. The data is the truth;
the query is a view; the view's gap is named.

Small things the spike shook out: a block as a statement needs no
semicolon in Rust, which is a real LR conflict between statement and
expression that `--generate` discovered and pinned as `[_stmt, _exp]`;
and the `grammar.js` printer did not escape a quote inside a string token
until `println!("{}", ..)` made it. Both backends take the new modules as
they are: three of three tree-sitter corpus cases on each, two of three
under ANTLR with the comment on the hidden channel as the only miss.

What remains open is narrower than before. Order-sensitive *scopes* are
handled; order-sensitive *values* (an interpreter's concern) are not the
grammar's. What the model still does not say is destructuring (a pattern
binding several names) and imports (a binding whose definition is in
another file), and §12 should carry those two instead of the one it
carried.

## 20. The seventh spike: the shared vocabulary, as a lowering

You asked how the shared concepts were doing, and the measured answer was:
well per grammar, thinly across grammars, and untouched by the
meta-grammar. Every shipped grammar threads 12 to 20 of the 22 table-tier
terms and ledgers every node outside them, but the only check that the
terms *mean the same thing everywhere* is three rosetta programs in four
languages with 19 assertions, and the six spike modules named their sorts
`_stmt` and `_exp`, private to each. §6 said the tier is a lowering
decision. This spike makes it one.

**The surface is a `vocabulary` section**, a treebank extension on the
SDF3 module, binding terms to sorts, constructors, tokens or other terms:

```
vocabulary
  _statement    = Stmt
  _expression   = Exp
  _declaration  = Stmt.Def
  _body         = Block
  _parameter    = Param
  _name         = ID
  _literal      = Exp.Int
  _branch       = Stmt.If
  _loop         = Stmt.While
  _jump         = Stmt.Return
  _control_flow = _branch _loop _jump
  _clause       = Else
```

**The lowering decides the tier per term**, which is what §3.1.1 of
DESIGN.md says a grammar must do and today does by hand. A term bound to
one whole sort *renames* that sort's supertype, so `_statement` is `Stmt`'s
own derivation. A term bound to anything narrower *threads* a new
supertype: its members become the alternation and every reference to a
member is routed through it, so `_branch` sits inside `_statement` exactly
where `if` stood, `_control_flow` nests `_branch`, `_loop` and `_jump` as
the vocabulary's containments require, and `_body` wraps `block` in every
field that held it. The tree is unchanged throughout, since supertypes are
hidden, which the unchanged corpora confirm. A facet term goes to
`roles.json` as type-level membership, and three facets need no
declaration at all: `_scope` and `_binding` come from §18's binding
attributes, `_callable` from a binding of kind `function`, `_comment` from
LAYOUT. And a table term whose member another threaded term already
claims, with neither nesting the other, would give that node two
derivations: the lowering *demotes* it to a facet with the reason written
out where the vocabulary marks the term `either_tier`, and refuses it
where it does not. That is `_parameter`'s story in python and rust,
computed instead of narrated.

**The checks are the repository's own.** `examples/roles.rs` runs
`treebank::check::check`, the code behind `treebank roles` in CI, over
each spike's generated `node-types.json` and lowered `roles.json`; the
`treebank` library builds without its engine, so the spikes are held to
the very code the shipped grammars are. And `tools/rosetta_check.py` is
the rosetta gate over the spike languages: three cases, the same program
in pyish, rustish and jsish, facet queries expanded through each module's
own manifest as treebank expands them at load time.

| spike | table-tier terms as supertypes | facets | uncategorised | checker |
|---|---|---|---|---|
| pyish | 14 of 22 | 5 | 1 (`int`, a piece of `exp_int`) | passes |
| rustish | 14 of 22 | 5 | 1 | passes |
| jsish | 13 of 22 | 5 | 1 | passes |

**Rosetta: 20 of 20 role queries yield the same count in all three
languages** — declarations, parameters, loops, jumps, invocations,
callables, control flow, clauses, literals, names, comments, bindings, and
the field pattern `(_declaration body: (_body))` whose parent and child
are both supertypes. The gate paid for itself on the first run. The first
draft of rustish and jsish bound `let` into `_declaration`; Python's
`prefix = name` is not one; the counts differed, and the shipped grammars
turned out to agree with Python (treebank-rust's `_declaration` holds
functions, structs, traits, consts and statics, and not
`let_declaration`), so the modules were corrected and `let` is a
`_statement` and a `_binding`, as `x = 1` is. What the three still do not
share is `_assignment`, which Python's line carries and the other two do
not, and that stays a vocabulary question the gate can now ask in three
lines of SDF3 instead of three grammars.

Two things the run says about the vocabulary itself. The uncategorised
token in every spike is the same shape the shipped grammars ledger by the
dozen — a piece of a node that carries the role — and the lowering writes
that reason itself. And the term counts, 13 and 14 of 22, are what a
language this small can thread; the remaining eight (`_pattern`,
`_member`, `_modifier`, `_attribute`, `_directive` in two of them,
`_access`, `_interpolation`, `_type` in two) name constructs the modules
do not have, which is the vocabulary's own rule about omission.

What this changes in the design is §6's claim, now a mechanism: the sorts
*are* the table tier, the facets are mostly derived, and the one decision
a grammar author makes by hand today, which tier a term lives in, is the
lowering's to make and to explain.

