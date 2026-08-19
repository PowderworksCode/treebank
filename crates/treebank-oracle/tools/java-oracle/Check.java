// Syntax-only Java validity check for the treebank oracle.
//
// stdin:  one file path per line
// stdout: "<path>\tvalid|invalid" per line
//
// Run with the JDK's single-file source launcher, no build step:
//   java tools/java-oracle/Check.java
//
// The reference parser is javac's own. JavacTask.parse() runs the parser
// and stops — it never attributes, so unresolved imports and missing
// classpath entries are not errors and a file can be judged on its own,
// exactly like ts.createSourceFile for TypeScript. Only ERROR diagnostics
// count; warnings (deprecation, unchecked) do not.
//
// The source level is the JDK's own latest. A file javac rejects at that
// level is not valid modern Java — `enum`, `assert` or `_` used as an
// identifier is 1.4-era code, and recording it as corpus noise is correct.

import com.sun.source.util.JavacTask;

import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

public class Check {
    public static void main(String[] args) throws IOException {
        // Two modes on one protocol. Reading to EOF answers a single batch
        // and exits, which is what the sweep wants. A SENTINEL line ends a
        // batch WITHOUT ending the process, which is what `fuzz` wants:
        // starting a JVM costs 0.57s and parsing a file costs 1.2ms, so a
        // fuzz run that launches per question spends its life in startup.
        final String SENTINEL = "\u0000--end--";
        BufferedReader in = new BufferedReader(
                new InputStreamReader(System.in, StandardCharsets.UTF_8));
        PrintStream out = new PrintStream(System.out, false, StandardCharsets.UTF_8);
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        if (compiler == null) {
            // No compiler means no oracle. Say so instead of calling
            // everything valid, which would make the sweep numbers lie.
            System.err.println("java-oracle: no system Java compiler (needs a JDK, not a JRE)");
            System.exit(2);
        }

        // Parse one file per task: files in a shared task share a diagnostic
        // stream, and one file that makes javac give up would take its
        // neighbours' verdicts with it.
        try (StandardJavaFileManager fm =
                     compiler.getStandardFileManager(null, null, StandardCharsets.UTF_8)) {
            List<String> batch = new ArrayList<>();
            for (String line = in.readLine(); line != null; line = in.readLine()) {
                if (line.equals(SENTINEL)) {
                    answer(compiler, fm, batch, out);
                    batch.clear();
                    out.println(SENTINEL);
                    out.flush();
                    continue;
                }
                String p = line.trim();
                if (!p.isEmpty()) {
                    batch.add(p);
                }
            }
            // EOF with work outstanding: the single-batch mode.
            answer(compiler, fm, batch, out);
        }
        out.flush();
    }

    private static void answer(JavaCompiler compiler, StandardJavaFileManager fm,
                               List<String> paths, PrintStream out) {
        for (String path : paths) {
            out.printf("%s\t%s%n", path, parses(compiler, fm, path) ? "valid" : "invalid");
        }
    }

    // An unreadable file is NOT an invalid file. Returning false there looks
    // harmless and is not: validate() is only ever called on files the
    // grammar already failed, and an invalid verdict records the file as
    // corpus NOISE. So a mistyped corpus root would make every path
    // unreadable, every grammar failure noise, gap_files zero -- and the
    // sweep would report a flawless grammar. A broken oracle must fail
    // loudly, never quietly agree with us (the reasoning is spelled out in
    // crates/treebank-cli/src/lang/exec_oracle.rs).
    //
    // A RuntimeException out of javac stays a verdict: that is the parser
    // failing on the file's own content, which is what invalid means.
    private static boolean parses(JavaCompiler compiler, StandardJavaFileManager fm, String path) {
        if (!Files.isReadable(Path.of(path))) {
            System.err.println("java-oracle: cannot read " + path);
            System.err.println("java-oracle: this is an oracle failure, not a verdict; "
                    + "check the corpus root");
            System.exit(1);
        }
        try {
            DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
            Iterable<? extends JavaFileObject> units = fm.getJavaFileObjects(Path.of(path));
            // -proc:none keeps annotation processors out; they would need a
            // classpath we do not have and are not part of the syntax.
            List<String> options = Arrays.asList("-proc:none", "-XDshould-stop.ifError=PARSE");
            JavacTask task = (JavacTask) compiler.getTask(Writer.nullWriter(), fm, diagnostics, options, null, units);
            task.parse();
            for (Diagnostic<? extends JavaFileObject> d : diagnostics.getDiagnostics()) {
                if (d.getKind() == Diagnostic.Kind.ERROR) {
                    return false;
                }
            }
            return true;
        } catch (IOException e) {
            System.err.println("java-oracle: cannot read " + path + ": " + e);
            System.err.println("java-oracle: this is an oracle failure, not a verdict; "
                    + "check the corpus root");
            System.exit(1);
            throw new AssertionError("unreachable");
        } catch (RuntimeException e) {
            return false;
        }
    }
}
