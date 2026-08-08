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
        List<String> paths = new ArrayList<>();
        try (BufferedReader in = new BufferedReader(
                new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            for (String line = in.readLine(); line != null; line = in.readLine()) {
                String p = line.trim();
                if (!p.isEmpty()) {
                    paths.add(p);
                }
            }
        }
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
            for (String path : paths) {
                out.printf("%s\t%s%n", path, parses(compiler, fm, path) ? "valid" : "invalid");
            }
        }
        out.flush();
    }

    private static boolean parses(JavaCompiler compiler, StandardJavaFileManager fm, String path) {
        try {
            if (!Files.isReadable(Path.of(path))) {
                return false;
            }
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
        } catch (IOException | RuntimeException e) {
            return false;
        }
    }
}
