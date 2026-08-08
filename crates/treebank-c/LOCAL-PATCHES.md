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

## Pilot sweep — measured

20 packages, 39,928 files, tree-sitter-c 0.24.2 unpatched, no grammar patches:

```
21,991 passed (55.1%) / 17,937 failed (44.9%), 924 clusters
  of the failures:  5,629 gap  |  302 noise  |  12,006 indeterminate
```

556 of the 924 clusters contain at least one known-valid file. The six
largest gap classes, and what they actually are:

| valid | files | signature | class |
|------:|------:|-----------|-------|
| 926 | 3455 | `expression_statement > MISSING ;` | statement macro then a block: `list_for_each(li, &q->ifaces) { … }` |
| 763 |  912 | `preproc_ifdef > MISSING #endif` | `extern "C"` brace asymmetry — preprocessor-inherent, not a grammar bug |
| 456 | 1383 | `function_definition > ERROR(identifier)` | macro in declaration position: `_INLINE_ void __list_add(…)` |
| 449 | 1557 | `declaration > MISSING ;` | same family |
| 290 | 1406 | `declaration > ERROR(identifier)` | `THREAD_LOCAL int adjustment = 0;` |
| 168 | 1782 | `argument_list > ERROR(identifier)` | type as macro argument: `list_entry(a, struct file_element, file_list)` |

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

The 28% has a partial fix that needs no new machinery and was tested:
`-idirafter` with every package header dir, which cannot shadow system headers
because it is searched *after* them. On 60 glibc indeterminate files it cuts
unresolved includes from **339 to 28 (-92%)** — and changes **no verdicts at
all**, because those files stay indeterminate on their remaining
parse-plus-semantic mix. Resolution and adjudication are not the same lever.
It is not landed for that reason; it would pair with config-header stubs,
which the 26% row makes the obvious first move.

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

## Patches

### 0001 — treebank redistribution notice

Packaging only, no grammar code. Prepends the standard warning to upstream's
`README.md` so that anyone encountering a materialized or published copy of
this tree knows it is a patched redistribution and where to report problems.
Applies first, per the contract in `GRAMMARS.md`.

**No grammar patches yet.** The pilot sweep's gap clusters are recorded above
and in `corpus/c/reports/REPORT.md`; none has been turned into a patch, so
this grammar is currently upstream 0.24.2 plus a README notice.
