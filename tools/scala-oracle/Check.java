// Syntax-only Scala validity check for the treebank oracle.
//
// stdin:  one "<dialect>\t<path>" per line
// stdout: "<path>\t<valid|invalid>\t<dialect>" per line
//
// Run with the JDK's single-file source launcher, no build step:
//   java -cp "$(cat tools/scala-oracle/classpath)" tools/scala-oracle/Check.java
// where the classpath comes from tools/scala-oracle/fetch-jars.sh.
//
// The reference parser is scalameta's, the parser Metals and scalafmt are
// built on. It parses without typing, so unresolved imports and a missing
// classpath are not errors and a file is judged on its own, exactly like
// JavacTask.parse() for Java. `Parsed.Error` is the verdict "invalid"; a
// `Parsed.Success` is "valid".
//
// THE DIALECT IS AN INPUT, NOT A GUESS. Scala 2 and Scala 3 are different
// languages sharing the `.scala` extension, and scalameta requires the
// dialect per file. Nothing in a file's path settles it, so the caller
// (crates/treebank-cli/src/lang/scala.rs) derives it from the Maven
// coordinate the file came from — `cats-core_3` is Scala 3,
// `spark-core_2.11` is Scala 2.11 — and passes it in per file. This oracle
// never picks a dialect, never falls back to another one, and never takes
// the union of two: measured over 3,508 corpus files, trying every dialect
// and keeping any success makes 100% of files valid by construction, while
// the declared dialect misclassified 0 and any single fixed dialect
// misclassified between 1.7% and 8.6%. Taking the union would not be a
// shortcut, it would be a lie.
//
// PARSING IS SERIAL, ON PURPOSE. scalameta's
// `internal.tokenizers.PlatformTokenizerCache.megaCache` is a
// ConcurrentHashMap of Dialect -> *non-concurrent* mutable.Map, so two
// threads parsing under the same dialect race and throw
// ConcurrentModificationException. Measured on this corpus: at 4/8/16
// threads, 1/8/15 valid files flipped to invalid, and a different set each
// run. A false `invalid` records a real grammar gap as corpus noise, so the
// oracle would quietly agree with us -- exactly the failure GRAMMARS.md
// forbids. There is one worker thread and it exists only for its stack size.
import scala.meta.Dialect;
import scala.meta.Source;
import scala.meta.inputs.Input;
import scala.meta.parsers.Parse;
import scala.meta.parsers.Parsed;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class Check {
    // Deep expression nesting is real in generated Scala, and a stack
    // overflow is a limit of ours rather than a fact about the file. Parsing
    // on one thread with a large stack keeps it from being one; if it
    // happens anyway, fail() below refuses to call it a verdict.
    private static final long STACK_BYTES = 512L * 1024 * 1024;

    private static Object dialectsModule;
    private static final Map<String, Dialect> DIALECTS = new HashMap<>();

    public static void main(String[] args) throws Exception {
        Class<?> pkg = Class.forName("scala.meta.dialects.package$");
        dialectsModule = pkg.getField("MODULE$").get(null);

        List<String[]> work = new ArrayList<>();
        try (BufferedReader in = new BufferedReader(
                new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            for (String line = in.readLine(); line != null; line = in.readLine()) {
                if (line.isBlank()) {
                    continue;
                }
                // Split on the FIRST tab only: a corpus path may contain one.
                int tab = line.indexOf('\t');
                if (tab < 0) {
                    fail("input line has no dialect: " + line,
                         "every line must be \"<dialect>\\t<path>\"");
                }
                String dialect = line.substring(0, tab);
                String path = line.substring(tab + 1);
                dialect(pkg, dialect, path);   // resolve now, so a bad name dies before any verdict
                work.add(new String[]{dialect, path});
            }
        }

        PrintStream out = new PrintStream(System.out, false, StandardCharsets.UTF_8);
        Thread worker = new Thread(null, () -> run(work, out), "scala-oracle", STACK_BYTES);
        worker.start();
        worker.join();
        out.flush();
    }

    private static void run(List<String[]> work, PrintStream out) {
        Parse<Source> parse = Parse.parseSource();
        for (String[] w : work) {
            String dialect = w[0], path = w[1];
            String text = read(path);
            Parsed<Source> parsed;
            try {
                parsed = parse.apply(new Input.VirtualFile(path, text), dialect(null, dialect, path));
            } catch (Throwable t) {
                // scalameta reports a syntax error by RETURNING Parsed.Error.
                // Anything thrown out of it is the parser breaking, not the
                // file being invalid, and guessing "invalid" here would file a
                // grammar gap as corpus noise. Measured: nothing in a 3,508
                // file corpus reaches this serially.
                fail("scalameta threw on " + path + " (dialect " + dialect + "): " + t,
                     "a thrown exception is not a verdict");
                throw new AssertionError("unreachable");
            }
            out.printf("%s\t%s\t%s%n", path, parsed.toOption().isDefined() ? "valid" : "invalid", dialect);
        }
    }

    private static Dialect dialect(Class<?> pkg, String name, String forPath) {
        Dialect known = DIALECTS.get(name);
        if (known != null) {
            return known;
        }
        Dialect d;
        try {
            d = (Dialect) (pkg != null ? pkg : dialectsModule.getClass())
                    .getMethod(name).invoke(dialectsModule);
        } catch (ReflectiveOperationException | ClassCastException e) {
            // An unroutable file is not an invalid file, for the same reason
            // an unreadable one is not: picking a dialect here would be this
            // oracle inventing the answer it exists to look up.
            fail("no scalameta dialect named \"" + name + "\" (for " + forPath + ")",
                 "the caller must name a dialect scalameta knows, e.g. Scala213 or Scala3");
            throw new AssertionError("unreachable");
        }
        DIALECTS.put(name, d);
        return d;
    }

    // An unreadable file is NOT an invalid file. validate() only ever runs on
    // files the grammar already failed, and an `invalid` verdict records the
    // file as corpus NOISE -- so a mistyped corpus root would turn every
    // grammar failure into noise, drive gap_files to zero, and report a
    // flawless grammar. A broken oracle must fail loudly, never quietly agree
    // with us.
    private static String read(String path) {
        if (!Files.isReadable(Path.of(path))) {
            fail("cannot read " + path, "check the corpus root");
        }
        try {
            return new String(Files.readAllBytes(Path.of(path)), StandardCharsets.UTF_8);
        } catch (IOException e) {
            fail("cannot read " + path + ": " + e, "check the corpus root");
            throw new AssertionError("unreachable");
        }
    }

    private static void fail(String what, String why) {
        System.err.println("scala-oracle: " + what);
        System.err.println("scala-oracle: this is an oracle failure, not a verdict; " + why);
        System.err.flush();
        Runtime.getRuntime().halt(1);   // halt, not exit: no shutdown hook may flush a partial verdict
    }
}
