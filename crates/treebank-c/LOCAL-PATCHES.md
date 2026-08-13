# treebank-c

Upstream [tree-sitter/tree-sitter-c](https://github.com/tree-sitter/tree-sitter-c)
pinned at **0.24.2** (`b780e47fc780ddc8da13afa35a3f4ed5c157823d`, the commit
tagged v0.24.2) as the `upstream/` git submodule; `scripts/materialize.sh`
applies the patch series below and generates the parser into `build/`
(gitignored). One grammar, no npm deps for generation (`generate_deps` is
null). Contract, reconstruction invariant, CLI pin rationale, and workflow:
see [GRAMMARS.md](../../GRAMMARS.md) at the repo root.

**Read [ORACLE.md](ORACLE.md) before reading any number here.** C is the first
language in treebank whose validity oracle cannot answer every question, and
the numbers below are only meaningful alongside what it declines to decide.

## The corpus is a distribution, not a registry

C has no registry, so popularity is borrowed. The choice is **Debian sid**,
ranked by **popcon** (`popularity-contest`) installs, which Debian already
aggregates per *source* package — so unlike Java there is no binary→source
mapping to invent. popcon counts machines, which is closer in kind to
crates.io downloads than Java's dependent-repos proxy.

The bias is explicit, and it is the point: **this corpus is the C that ships
in a distribution** — system libraries, daemons, autotools trees, GNU
extensions, decades-old code that still runs everything. It is not "trending
C on GitHub". Those are different corpora and they will give different gap
numbers; a claim from this one should say Debian.

Two filters stand between popcon and the corpus:

- **Is it C at all?** popcon ranks everything Debian ships. Without a filter
  the top of the list spends its bandwidth on LibreOffice (4.4M lines of C++
  to 34k of C) and gcc-16 (no C at all). `sources.debian.org` publishes
  per-language SLOC, so `rank()` keeps a package when `ansic >= 2000` and
  `ansic >= cpp`, one small request per candidate.
- **Is this header C or C++?** `.h` belongs to both languages — the routing
  problem `DESIGN.md` flags as unresolved — and only content tells them
  apart. See below.

Upstream's own `.orig.tar.*` is fetched, never Debian's `.debian.tar.*`, so
the corpus is upstream source rather than the distro's patched tree. All
three compressions are real: measured across sid main, 60,674 `.orig.tar.gz`,
22,708 `.orig.tar.xz`, 2,384 `.orig.tar.bz2` — and all three occur inside the
top 25 C sources, so `fetch.rs` learned xz and bzip2 for this grammar.

### Keeping the corpus fresh

`daily.sh` re-ranks every day, and two things about that were wrong at first.

The Debian `Sources` index was cached forever. It carries every package's
*version*, so a permanent cache **froze the corpus**: `resolve()` would return
the same tarballs indefinitely, the sweep cache would skip every one of them,
and the "new version of a top-K package" event the whole loop is built around
could never fire for C. It is now refreshed on any run where the cached copy
is over 12 hours old, `TREEBANK_REFRESH_SOURCES=1` forcing it.

The SLOC filter made one request per candidate, every day — about 1,250 of
them at the default `TREEBANK_RANK_K=1000`, for facts that change only when a
package does. Verdicts now persist in `corpus/c/db/sloc.json`, keyed by name
and stamped with the version measured, so a daily run queries exactly the
packages whose version moved. Measured at k=200: 227 lookups in 9s cold
(~25/sec, 8 at a time), then 2 lookups and 1.9s warm.

Fixing the first exposed something worse. **sources.debian.org lags the
archive**: on any day Debian has just accepted an upload, the new version is
in the index but has no SLOC record yet. The first refreshed run hit exactly
that on glibc 2.43-3 and mesa 26.1.6-1 — and dropped both, silently removing
the two largest C sources from the corpus. A failed lookup now falls back to
the newest version sources.debian.org actually holds, stamped with what was
really measured so the next run re-queries once the archive catches up.

Lookups are batched (64) and resolved concurrently, but each batch is consumed
in popcon order, so the list is identical to a sequential walk and does not
depend on which request finished first. Verified: the k=20 list is a prefix of
the k=100 list, and both agree with the pre-batching implementation's skip
counts.

### The `.h` filter, and why it is content-based

A C++ header does **not** come back cleanly `invalid` from the oracle; it
comes back `indeterminate` (measured: `namespace` + `template class` →
1 Parse Issue + 1 Semantic Issue). Feeding headers in raw would therefore
inflate the one bucket whose size decides whether C is sweepable at all,
which is why the filter exists rather than leaving it to the oracle.

Two false-positive classes had to be fixed before it was usable, both found
by checking what it dropped rather than by reasoning about it:

- **Comment prose.** glibc's `elf/elf.h` was dropped over the words "class
  declaration." ending a block comment, and `malloc/obstack.h` over
  "namespace with `<stddef.h>`'s symbols" on a GNU comment *continuation*
  line, which carries no `*` prefix to skip on. Comments and string literals
  are now blanked before scanning.
- **Dual C/C++ headers.** glibc's `math.h` contains `extern "C++" { template
  <class __T> … }` — inside `#ifdef __cplusplus`. It is a C header. Anything
  inside a conditional mentioning `__cplusplus` is now skipped, both
  branches, so only *unguarded* C++ counts.

Together those recovered 82 of 447 dropped headers (18%), including
`elf.h`, `math.h`, `string.h`, `stdlib.h`, `obstack.h` and `ldsodefs.h`. What
stays dropped is real C++: `ncurses/c++/`, krb5's Windows MFC classes,
glibc's `template<>` test fixtures.

## Reference parser

`tools/c-oracle` is **libclang** (pinned 20.1.2), parse-only, with the verdict
computed from clang's own diagnostic categories and no message text matched
anywhere. It is **three-valued** — valid / invalid / indeterminate — because C
is not parseable without semantic information and pretending otherwise would
make the numbers lie. [ORACLE.md](ORACLE.md) is the full argument, including
why `gcc -fsyntax-only` cannot do this job and why classifying gcc's
diagnostics cannot either.

`Lang::validate` is two-valued, so **indeterminate collapses to "not a gap"**:
no fix agent is dispatched at a file whose validity we cannot vouch for. That
makes `gap_files` a **floor**, and mixes indeterminates into `noise_files`.
The full split is printed by every sweep and written to
`corpus/c/oracle-verdicts.json`. **A C gap number quoted without its
indeterminate count is not a claim this crate makes.**

### Include resolution is the whole ballgame

The oracle judges a file in whatever include environment it is given, and for
C that environment does most of the work. No build system is run — no
`./configure`, no `cmake` — so a generated `config.h` is absent and its
absence shows as indeterminate rather than as a fabricated verdict.

Getting the flags right mattered more than any other single decision, and the
intuitive choice was measurably wrong. Full 20-package pilot, 17,868 failing
files adjudicated:

| include flags                      | valid | invalid | indeterminate |
|------------------------------------|-------|---------|---------------|
| conventional dirs, `-I`            |  4578 |     146 |         13144 |
| every header-bearing dir, `-I`     |  4060 |     144 |         13664 |
| every header-bearing dir, `-iquote`|  5651 |     300 |         11917 |

Widening `-I` made it **worse**. `-I` is searched for `#include <...>` as well
as `"..."`, so a package's private replacements for system headers start
answering system includes — glibc's `string/string.h` for `<string.h>`,
mesa's `util/` for `<util/…>` — and those copies only stand up inside their
own build. `-iquote` applies to the quoted form only, which is how
package-internal headers are actually included.

Final flags use **all three search paths**, each for a distinct job:
`-iquote` over every header-bearing dir (the package's own quoted includes),
`-I` over the package's *public* dirs only (its own API, angle-bracketed),
and `-idirafter` over every header-bearing dir. The last one exists because
packages angle-bracket their own internals too — glibc's `#include
<sigsetops.h>` — and `-idirafter` is searched *after* the system directories,
so it supplies only headers the system lacks and cannot shadow `<string.h>`.
On 1,500 failing files, `-idirafter` is worth **372 → 453 valid** and
**1117 → 1010 indeterminate**.

## Pilot sweep — measured

20 packages, 39,928 files, tree-sitter-c 0.24.2 unpatched, no grammar patches:

```
21,991 passed (55.1%) / 17,937 failed (44.9%), 924 clusters
  of the failures:  4,542 gap  |  960 config-inherent  |  12,435 noise
  oracle split:     5,502 valid | 452 invalid | 11,983 indeterminate
```

**config-inherent** is a third verdict, added once C arrived: valid files the
grammar rejects that **no grammar patch can fix**, because a preprocessor
conditional splits a construct the parser must see whole. They parse cleanly
once the branches a compiler would have dropped are removed. 909 of the 5,502
files the oracle calls valid are this class — see
[treebank-preprocessing](../treebank-preprocessing/src/lib.rs). `REPORT.md`
names them in their own section and keeps them out of the fix instructions, so
the agent does not spend its attempts on the one cluster it cannot win.

Of the 4,593 gaps, **1,102 (24.0%) parse cleanly once the package's macros are
expanded** — the grammar was meeting an unexpanded macro, not unfamiliar
syntax. That does not excuse them: unlike the `extern "C"` case, `THREAD_LOCAL
int x;` is something a grammar *could* parse. So they stay gaps, and each
cluster is annotated with the macros responsible, which is what writing a
minimal rule — and checking it does not over-accept — actually requires.

The six largest *gap* classes, and what they actually are:

| valid | files | signature | class |
|------:|------:|-----------|-------|
| 950 | 3455 | `expression_statement > MISSING ;` | statement macro then a block: `list_for_each(li, &q->ifaces) { … }` |
| 422 | 1383 | `function_definition > ERROR(identifier)` | macro in declaration position: `_INLINE_ void __list_add(…)` |
| 413 | 1557 | `declaration > MISSING ;` | same family |
| 279 | 1406 | `declaration > ERROR(identifier)` | `THREAD_LOCAL int adjustment = 0;` |
| 164 | 1782 | `argument_list > ERROR(identifier)` | type as macro argument: `list_entry(a, struct file_element, file_list)` |

Read `gap_files` as a floor: see [ORACLE.md](ORACLE.md#measured-result-on-the-20-package-pilot).

## Preprocessor observations (out of scope, recorded)

Per the brief, no preprocessor mechanism is built here; C# already has this
measured in [treebank-csharp](../treebank-csharp/LOCAL-PATCHES.md) and the
general design belongs to a separate session. What this corpus shows:

- **`#ifdef __cplusplus` brace asymmetry** is the largest single *gap*
  cluster: `preproc_ifdef > MISSING #endif`, first seen at
  `util-linux/libuuid/src/uuid.h:127`. The canonical C header opens `extern
  "C" {` inside one `#ifdef __cplusplus` and closes `}` inside another. clang
  (with `__cplusplus` undefined) never sees either brace and the file is
  valid; tree-sitter parses all branches and finds an unbalanced one. Same
  shape as the C# asymmetry, and like it, not a fixable grammar bug.
- **Undefined macros in declaration position** drive nearly every
  indeterminate verdict, and several gap clusters too: `THREAD_LOCAL int x;`,
  `_INLINE_ void f(...)`, `void *p(size_t n _unused_)`.
- **Types as macro arguments** — `list_entry(a, struct file_element,
  file_list)` — are a distinct class: valid after expansion, unparseable as a
  call whose arguments must be expressions.
- **Statement macros followed by a block** — `list_for_each(li, &q->ifaces)
  { … }` — likewise.

The last three are the classes a grammar fix would target, and each is a
place where a careless fix would start accepting invalid C. The negative
corpus guards all three deliberately.

### How much a macro system would actually buy — measured

Two independent ceilings, both sampled from this corpus. They are recorded
here as input to whoever designs the preprocessor mechanism; nothing below is
built in this crate.

**Grammar side.** Random 299 of the 5,629 known-valid gap files, classified by
what is at the real first-error line (from `tree-sitter parse`):

| share | at the error site |
|------:|-------------------|
| 62.9% | a macro `#define`d **in the same package**, on the error line |
|  7.4% | same, on the line above |
| 20.7% | a preprocessor conditional — `#ifdef`/`__cplusplus` |
|  9.0% | no in-package macro nearby |

So **~70% of the gap queue is macro-shaped and the definitions are already in
the corpus** — same package, no external knowledge needed. But the next 21% is
*not* macro expansion: it is the `extern "C"` brace asymmetry, which needs
**configuration selection** (parse one branch, as Roslyn does for C#), a
different mechanism. Together, 91% of C's gap queue is preprocessor-shaped in
one form or the other.

Worth noting for whoever picks this up: the three fixable classes do not
strictly *need* macro definitions in order to parse. A grammar rule for
`IDENT(args) compound_statement` would parse `list_for_each(…) { … }` with no
macro knowledge at all. What the macro knowledge buys is the right to do that
without over-accepting — which is precisely what `test/negative/` exists to
police.

**Oracle side.** Random 300 of the 12,006 indeterminate files, classified by
first unresolved `#include`:

| share | what is missing |
|------:|-----------------|
| 28.0% | a header that **is in the package**, at a path we do not search |
| 26.3% | a **generated** config header (`config.h`, `include/config.h`) |
| 24.3% | **another package's** header (`glib-object.h`, …) |
| 21.3% | nothing — indeterminate for non-include reasons |

The 28% is fixed, by `-idirafter` over every package header dir — see the
include-flags section above. It is now part of the standard flags.

**A correction belongs here, because it changed a conclusion.** When first
tested, `-idirafter` appeared to cut unresolved includes on glibc by 92%
(339 → 28) while flipping *no verdicts at all*, and it was written up as
evidence that "resolution and adjudication are not the same lever". That was
an artifact of a bug in `c-oracle`: a fixed 128-flag cap per request silently
dropped everything past field 128, and with glibc at 498 header-bearing dirs
the `-idirafter` flags were precisely the ones discarded. The direct-clang
test bypassed the oracle, which is why the two disagreed. With the cap
removed, `-idirafter` is worth +81 valid files per 1,500 sampled.

The lesson still worth keeping is the one that caught it: when two
measurements of the same thing disagree, the disagreement is the finding.

The 26% (generated config headers) is **not addressable at all**, which took
a further round of measuring to establish. Stubbing an empty `config.h`
resolves the include, but `valid` requires `parse == 0` and an empty stub
supplies none of the macros whose absence caused the parse error — so it
cannot promote anything, and its only possible effect is to turn
indeterminate into a confident, wrong `invalid`. Measured: 0 verdict changes
in 500 files. The guard this paragraph used to propose would not rescue the
idea; there is nothing to rescue. See
[ORACLE.md](ORACLE.md#stubbing-configh-cannot-work-and-the-reason-is-structural).

The 24.3% is tractable *for this corpus specifically*: Debian declares each
source package's `Build-Depends`, and the Sources index already parsed by
`rank()` carries that field.

## Negative corpus

`test/negative/` holds nine files: every one is **rejected by the oracle**
(purely `Parse Issue`, no missing-context evidence) and must stay rejected by
the grammar.

Two are real, taken from the pilot sweep's oracle-invalid set:
`util-linux/include/pt-mbr-partnames.h`, an initializer-list fragment meant
to be `#include`d mid-declaration, and one of mesa's `glcpp` GLSL
preprocessor test fixtures — a `.c` file that is not C at all, exactly the
"other-dialect files" `DESIGN.md` predicts as corpus noise.

The rest are minimal, and three of them exist specifically to guard the gap
classes above: a fix for macros-in-declaration-position must not start
accepting juxtaposed expressions, and a fix for statement-macros-with-blocks
must not accept an arbitrary call with a block glued on. Three earlier drafts
were rewritten after the oracle called them `indeterminate` rather than
`invalid` — a negative file that the reference parser will not positively
reject does not belong in the corpus.

## The corpus since the pilot, and what the patches moved

The pilot above is 20 packages. The corpus is now **100 Debian sid source
packages, 84,455 files**, and `corpus/c/reports/REPORT.md` is a sweep of that:

```
upstream 0.24.2:  43,089 passed / 41,366 failed
                  13,826 gap | 1,883 config-inherent | 25,657 noise, 1,035 clusters
+ patches 3-15:   44,021 passed / 40,434 failed
                  13,199 gap | 1,889 config-inherent | 25,346 noise
```

**+932 files**, and the shape of what is left has not changed: the gap queue
is still dominated by undefined macros in declaration and statement position,
which no grammar rule can absorb without accepting arbitrary juxtaposed
identifiers. The report's own sample puts ~70% of the gap queue in that class
and another ~21% in the `#ifdef __cplusplus` brace asymmetry, which is
configuration selection rather than parsing. The patches below are the
remainder — the part that is language, not preprocessor state.

## Adversarial review of the sweep fixes

The thirteen grammar fixes were checked by generating invalid C aimed at each
new rule, confirming the oracle rejects it, and then asking whether the grammar
does too. Eight of ten probes were genuinely invalid; the grammar rejected
seven of those eight.

The eighth is a deliberate, inherent trade, now recorded in the ledger against
patch 0013: parsing `va_arg(ap, char *)` requires accepting a type with an
abstract declarator as a call argument, which necessarily also accepts
`g(char *)` — invalid C when `g` is a function. Nothing in the token stream
distinguishes the two. The guard that *does* hold is that a bare type name
stays an error.

The review's substantive finding was procedural: thirteen permissive rules
arrived with **no new negative-corpus files**, which is precisely the direction
`GRAMMARS.md` says sweeps cannot catch. Six were added, one per new accept
surface, each verified oracle-invalid and grammar-rejected.

## Patches

### 0001 — treebank redistribution notice

Packaging only, no grammar code. Prepends the standard warning to upstream's
`README.md` so that anyone encountering a materialized or published copy of
this tree knows it is a patched redistribution and where to report problems.
Applies first, per the contract in `GRAMMARS.md`.

### 0002 — treebank crate identity

Packaging only, no grammar code, and the last *packaging* patch in the series
per `GRAMMARS.md`. Upstream owns `tree-sitter-c` on crates.io, so the
redistribution publishes as `treebank-grammar-c` with its own `repository`,
`homepage` and `description`, and `include` grows to carry `ledger.json`,
`LOCAL-PATCHES.md` and `patches/*` inside the published tarball so provenance
travels with it.

`[lib] name` is pinned to `tree_sitter_c`. Renaming the package would
otherwise rename the lib to `treebank_grammar_c` and break every
`tree_sitter_c::LANGUAGE` call site; pinning it keeps the crate a drop-in
replacement. `tools/consumer-test` asserts exactly that against
`fixtures/patched.c`.

The published version is deliberately absent — `publish.sh` derives it from
crates.io at publish time. See [PUBLISHING.md](../../PUBLISHING.md).

## Grammar patches

Thirteen, all from one sweep of `corpus/c` (100 Debian sid source packages,
84,455 files). Every one is a construct the C standard or the GNU dialect
defines, or a place a preprocessor directive is allowed to stand — no macro
knowledge is involved in any of them, which is the line this grammar holds.
The report's largest clusters are undefined macros in declaration or
statement position (`_INLINE_ void f(...)`, `list_for_each(li, &q->ifaces) {`,
`THREAD_LOCAL int x;`) and none of them is fixable without either expanding
macros or accepting arbitrary juxtaposed identifiers, which is exactly what
`test/negative/` exists to prevent. What follows is what a parser *can* fix.

### 0003 — anonymous bit-fields

C11 6.7.2.1 writes struct-declarator as `declarator` or `declarator_opt :
constant-expression`, so a bit-field may be unnamed: `unsigned int :3;` is
padding, `int :0;` forces alignment to the next storage unit.
`_field_declaration_declarator` required a declarator before the
`bitfield_clause`, so every such member came out as `MISSING
field_identifier` — the whole `field_declaration > MISSING field_identifier`
cluster, first seen at `glibc/include/struct___timespec64.h:19`
(`__int32_t :32;`) and all over the kernel UAPI headers systemd vendors. The
rule is now the two-way choice the standard writes.

### 0004 — GNU attributes in declaration positions

Three positions GCC accepts and the grammar did not:

- after the declarator of an object declaration —
  `int x __attribute__((unused)) = 0;`, `int a, b __attribute__((aligned(4)));`
- after the `*` of a pointer declarator —
  `static inline void * __attribute__((nonnull (1))) l_memcpy(...)`
- on either side of the type in a typedef —
  `typedef unsigned long int __attribute__ ((__may_alias__)) op_t;`

The first is a new rule, `gnu_attributed_declarator`, aliased back to
`attributed_declarator` so the node name consumers query is unchanged, and
deliberately limited to the object declarators (identifier, pointer, array,
parenthesized). Every other declarator position already absorbs a trailing
attribute — function declarators through `_function_declaration_declarator`,
parameters through `parameter_declaration`, members through
`field_declaration`, typedefs through `type_definition` — and letting the
general `attributed_declarator` take `__attribute__` made all four ambiguous:
generation failed outright on three of them, and once the conflict was
declared, `void f(int x __attribute__((unused)))` re-parsed into a different
tree and broke upstream's own `Attributes` and `Type qualifiers` corpus
tests. Keeping the new rule out of those positions leaves every existing
parse untouched.

The typedef half is the same asymmetry from the other side: an ordinary
declaration already admits an attribute on either side of the type specifier
through `_declaration_modifiers`, and a typedef is that declaration with
`typedef` in the storage-class slot, so `_type_definition_type` now takes the
same choice.

### 0005 — named variadic macro parameters

`#define check(FMT...) do { … } while (0)` is the GNU spelling of a variadic
macro: the parameter name stands for the rest of the argument list, where ISO
C writes `...` and `__VA_ARGS__`. `preproc_params` took `identifier` or
`...` but not an identifier carrying one, so the `#define` itself failed —
and with it every use of the macro below it.

### 0006 — complex types

`_Complex` and `_Imaginary` (C99 6.7.2) appear *beside* float/double rather
than instead of them — `_Complex float`, `double _Complex`, `long double
_Complex` — which is why they belong with signed/unsigned/long/short in
`sized_type_specifier` and not in `primitive_type`. `__complex__` is GCC's
spelling, accepted in every dialect. Without them `_Complex float cf;` read
as two juxtaposed type names and failed.

### 0007 — case ranges

`case 'a' ... 'z':` and `case 0 ... 2:` — a GNU extension used by every
character-classification switch in the corpus (glibc's locale reader, grub's
script executor, mesa, iptables). The range end is a new `end` field on
`case_statement`, so a plain `case` is unchanged.

### 0008 — alternate asm qualifier keywords

GCC's alternate keywords come in two spellings, `__volatile__` and
`__volatile`, and glibc's per-architecture headers use the short one:
`__asm __volatile ("flush %0" : : "r"(reloc_addr));`. `gnu_asm_qualifier`
knew only the doubled form. `__inline`/`__inline__` are added for the same
reason.

### 0009 — `__has_include` in preprocessor conditionals

`#if __has_include(<stdcountof.h>)` is C23, and a GNU/clang extension long
before that. Its operand is a header name rather than an expression, so like
`defined` it cannot go through `preproc_call_expression`: the `<…>` form is a
`system_lib_string`, which inside an `#if` would otherwise lex as a less-than
operator. New `preproc_has_include` node, mirroring `preproc_defined`.

### 0010 — typeof

C23's `typeof`, which GCC has spelled `__typeof`/`__typeof__` for decades.
`typeof(x)` on a bare name already parsed, by accident: the operand looked
like a type descriptor, so it came out as a `macro_type_specifier`. Anything
else did not — `(__typeof (cmsg->cmsg_len)) SIZE_MAX` has a field access for
an operand, and no type descriptor holds one. The new `typeof_specifier`
takes either a type or an expression, and now carries the bare-name case too.

### 0011 — `_Atomic(T)`

C11 6.7.2.4 gives `_Atomic` two jobs: the qualifier `_Atomic int x;`, which
the grammar had, and the atomic type specifier `_Atomic(int) x;`, which it
did not. mimalloc, vendored inside CPython, is written in the second form
throughout.

### 0012 — a label at the end of a compound statement

C23 6.8.1 allows a label with no statement after it at the end of a block —
the `out:` immediately before the closing brace that the `goto out;` idiom
produces — and GCC and clang accept it in every dialect. `labeled_statement`
required a statement, so the block ended with a `MISSING ;`. The new
`trailing_label` is admitted in that one position only, so a label with no
statement anywhere else stays an error.

### 0013 — a type as a call argument, when it carries an abstract declarator

`va_arg(ap, char *)` is not a call: `va_arg` is a macro and its second
argument is a type. `argument_list` already admits a `compound_statement` for
exactly this reason — upstream's comment says "macros taking statements as
arguments" — and this extends it to a type descriptor, but only one carrying
an abstract declarator (`char *`, `int (*)(void)`, `long []`), none of which
can be read as an expression.

Two things it deliberately does not do. A bare type name is not accepted, so
`test/negative/type-in-plain-call-argument.c` (`g(struct s)`) stays rejected —
that negative file is the guard on this exact fix. And a *leading* qualifier
is not accepted: making `const` valid at the start of an argument makes the
lexer prefer the keyword inside `__attribute__((const))`, which breaks
upstream's `Attributes` test, so `va_arg(ap, const char *)` still fails while
`va_arg(ap, char const *)` parses. The alias carries `prec.dynamic(-2)` so
`macro_type_specifier` keeps winning wherever upstream's tests say it should.

### 0014 — preprocessor directives between enumerators

`enumerator_list` already admitted `#if`/`#ifdef` and a bare `#directive`,
but not `#define` — and the kernel UAPI headers systemd vendors wholesale put
one after every enumerator, so each name can be tested with `#ifdef`:

```c
enum fsconfig_command {
	FSCONFIG_SET_FLAG	= 0,
#define FSCONFIG_SET_FLAG FSCONFIG_SET_FLAG
	FSCONFIG_SET_STRING	= 1,
#define FSCONFIG_SET_STRING FSCONFIG_SET_STRING
};
```

Unlike an enumerator the directive is not followed by a comma, and it can
also follow the *last* enumerator, so it needed a slot both in the repeat and
in the comma-less tail.

### 0015 — preprocessor conditionals and directives in an initializer list

A braced initializer whose elements are guarded by `#ifdef` is one of the
commonest shapes in this corpus — tables of syscalls, ioctls, partition
types, colour names — and `initializer_list` was a plain `commaSep` with no
preprocessor slot at all, so the whole declaration failed. It is now shaped
like `enumerator_list`: element-then-comma, or a conditional, or a directive,
with the same comma-less tail, and `preprocIf` gets two more instantiations,
`_in_initializer_list` and `_in_initializer_list_no_comma` — exactly how
upstream already handles enumerators and struct members.

The `#include` slot is for the list-fragment header (util-linux's
`pt-mbr-partnames.h`, `#include`d in the middle of a `{ … }`). The fragment
itself stays invalid on its own, which `test/negative/` asserts and still
does. The one new conflict, `[expression, concatenated_string]`, is the
identifier-then-string ambiguity `concatenated_string` has always carried; it
becomes reachable now that an `#ifdef` name can sit directly before an
element.
