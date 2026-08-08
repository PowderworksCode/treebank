# treebank-java

Upstream [tree-sitter/tree-sitter-java](https://github.com/tree-sitter/tree-sitter-java)
pinned at **0.23.5** (`94703d5a6bed02b98e438d7cad1136c01a60ba2c`) as the
`upstream/` git submodule; `scripts/materialize.sh` applies the patch series
below and generates the parser into `build/` (gitignored). One grammar, no
npm deps for generation (`generate_deps` is null). Contract, CLI pin
rationale, and workflow: see [GRAMMARS.md](../../GRAMMARS.md) at the repo
root.

## Corpus

Maven Central is the registry, but it is not shaped like crates.io or npm.

- **No download counts.** Central publishes none, so the ranking comes from
  ecosyste.ms' index of `repo1.maven.org`, ordered by how many public
  repositories depend on each artifact. That is a dependency-graph proxy for
  popularity, not traffic; the ledger records the difference.
- **Source lives in a separate artifact.** The main jar holds `.class`
  files. Java's convention is a parallel `-sources.jar`, present for 89 of
  the top 100; the other 11 are pom-only aggregators (the
  `spring-boot-starter-*` family) with no source of their own, and the fetch
  driver skips them.
- **Sources jars are zips, not tarballs**, and their entries are already
  root-relative — there is no wrapper directory to strip, so stripping one
  would eat the leading `com/` of every path. `fetch::extract` now picks the
  archive shape from the file's magic bytes and only strips a root component
  for tarballs.
- **Versions come from Central, not the index.** ecosyste.ms'
  `latest_release_number` is stale for some artifacts (it reported guava
  16.0.1 against Central's 33.6.0-jre), so `resolve()` reads
  `maven-metadata.xml` and takes `<release>`.

21,049 `.java` files from 89 artifacts.

## Reference parser

`tools/java-oracle/Check.java` is javac's own parser, reached through
`com.sun.source.util.JavacTask.parse()` and run by the JDK's single-file
source launcher — no build step, no jar. `parse()` stops before attribution,
so unresolved imports and a missing classpath are not errors and each file
is judged on its own, which is the same property that makes
`ts.createSourceFile` usable for TypeScript. Only `ERROR` diagnostics count.

Each file gets its own task: files sharing a task share a diagnostic stream,
and one file that makes javac give up would take its neighbours' verdicts
with it. With no JDK present the oracle exits nonzero rather than reporting
everything valid, which would make the sweep numbers lie.

The source level is the JDK's own latest (21). A file javac rejects there is
not valid modern Java — `enum` or `_` as an identifier is 1.4-era code — and
recording it as corpus noise is the right answer.

## Patches

1. **Treebank redistribution notice** (`0001`) — prepends a warning to
   upstream's `README.md` stating that this tree is an automatically
   generated, patched redistribution maintained by
   [treebank](https://treebank.dev), so the notice travels with every
   materialized/published copy. Applied first; touches no grammar code.

2. **Type annotation before the varargs ellipsis** (`0002`) —
   `void f(String @Nullable ... args)`. JLS 8.4.1 puts the annotations
   between the element type and the ellipsis, where they annotate the array
   type the ellipsis creates. Upstream had `repeat($._annotation)` *after*
   the `'...'`, so it got both directions wrong at once: it rejected the
   legal form and accepted `String ... @Nullable args`, which javac rejects.
   Upstream's own corpus test asserted the illegal form while its title read
   "Annotations before a spread parameter's ellipsis"; the patch corrects
   the test source to match its title. Found by the Maven top-100 sweep in
   spring-webmvc 7.0.8 and 54 other files — 9 of the 10 sweep clusters.

3. **`when` is a contextual keyword** (`0003`) —
   `for (SimpleWhen when = this.when; …)`. JLS 3.9 lists `when` as
   contextual: it introduces a switch guard but stays a legal identifier
   everywhere else. It joins the grammar's existing `_reserved_identifier`
   list next to `record`, `sealed`, `with` and `yield`. That makes
   `case x instanceof T when …` genuinely ambiguous — bind `when` as the
   instanceof pattern's name, or start the guard — so `instanceof_expression`
   is declared as a conflict and GLR keeps both readings alive. Switch
   guards still parse; the corpus test exercises guards and identifier uses
   in one file. Found in h2 2.4.240 (1 file).

## Negative corpus

`test/negative/` holds 7 files javac rejects and this grammar must keep
rejecting, including the two that guard the patches above:
`VarargsAnnotationAfterEllipsis.java` (the form patch 1 stopped accepting)
and `GuardWithoutExpression.java` (`case Integer i when ->`, so patch 2
cannot degrade `when` into a plain identifier everywhere).

Two further invalid files were tried and left out because the grammar
accepts them and no context-free rule would catch them: a duplicated
modifier (`public public void f()`) and `enum` used as an identifier, which
is a Java-5-onwards restriction rather than a syntactic one.
