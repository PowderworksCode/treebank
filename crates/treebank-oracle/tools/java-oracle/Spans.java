// Node BOUNDARIES from javac, for `treebank shape`.
//
// stdin:  one file path per line
// stdout: one JSON object per line:
//         {"path":..., "spans":[[start,end,"KIND"],...], "error":off, "skipped":...}
//
// Positions come from Trees.getSourcePositions() over JavacTask.parse() —
// parse only, no analyze, so no synthetic members exist and every node is
// something the file spells. javac reports offsets in UTF-16 CODE UNITS
// into the decoded source; tree-sitter counts BYTES, so the conversion
// happens here, where the decoded string is still in hand.
//
// Files that are not clean UTF-8 (or carry a BOM) are skipped by name:
// their byte offsets would not line up with the chars javac counted, and a
// wrong span reads as a disagreement about the code. Same rule as the
// python oracle's.
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.Tree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.SourcePositions;
import com.sun.source.util.TreeScanner;
import com.sun.source.util.Trees;

import javax.tools.Diagnostic;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class Spans {
    public static void main(String[] args) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        if (compiler == null) {
            System.err.println("java-oracle: no system compiler — a JDK is required, not a JRE");
            System.exit(1);
        }
        List<String> paths = new ArrayList<>();
        try (BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            String line;
            while ((line = in.readLine()) != null) {
                if (!line.isBlank()) paths.add(line.trim());
            }
        }
        StringBuilder out = new StringBuilder();
        try (StandardJavaFileManager fm =
                 compiler.getStandardFileManager(null, null, StandardCharsets.UTF_8)) {
            for (String p : paths) {
                out.setLength(0);
                one(compiler, fm, p, out);
                System.out.println(out);
            }
        }
        System.out.flush();
    }

    static void one(JavaCompiler compiler, StandardJavaFileManager fm, String path, StringBuilder out) {
        out.append("{\"path\":").append(quote(path));
        byte[] bytes;
        try {
            bytes = Files.readAllBytes(Path.of(path));
        } catch (Exception e) {
            // An unreadable file is an oracle FAILURE, never a verdict.
            System.err.println("java-oracle: cannot read " + path + ": " + e);
            System.exit(1);
            return;
        }
        if (bytes.length >= 3 && (bytes[0] & 0xFF) == 0xEF && (bytes[1] & 0xFF) == 0xBB && (bytes[2] & 0xFF) == 0xBF) {
            out.append(",\"spans\":[],\"skipped\":\"BOM: byte offsets would not line up\"}");
            return;
        }
        String src;
        try {
            src = StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(java.nio.ByteBuffer.wrap(bytes)).toString();
        } catch (Exception e) {
            out.append(",\"spans\":[],\"skipped\":\"source encoding: not UTF-8\"}");
            return;
        }
        // Byte offset at each UTF-16 code-unit index, so javac's positions
        // convert in O(1) each. A surrogate PAIR is one 4-byte codepoint:
        // both units map to its start, and the unit after it lands past it.
        int[] byteAt = new int[src.length() + 1];
        int b = 0;
        for (int i = 0; i < src.length(); ) {
            int cp = src.codePointAt(i);
            int units = Character.charCount(cp);
            int len = cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
            for (int u = 0; u < units; u++) byteAt[i + u] = b;
            i += units;
            b += len;
        }
        byteAt[src.length()] = b;

        final long[] firstError = {-1};
        List<CompilationUnitTree> units = new ArrayList<>();
        Trees trees;
        try {
            Iterable<? extends JavaFileObject> objs = fm.getJavaFileObjects(path);
            JavacTask task = (JavacTask) compiler.getTask(
                null, fm,
                d -> {
                    if (d.getKind() == Diagnostic.Kind.ERROR && firstError[0] < 0
                        && d.getPosition() != Diagnostic.NOPOS) {
                        firstError[0] = d.getPosition();
                    }
                },
                List.of("--release", "21",
                    // javac's parser folds `"a" + "b"` into ONE folded
                    // STRING_LITERAL spanning both literals and the `+`,
                    // a span our tree rightly has no node for. The hidden
                    // switch turns the folding off; positions stay honest.
                    "-XDallowStringFolding=false"), null, objs);
            task.parse().forEach(units::add);
            trees = Trees.instance(task);
        } catch (Throwable t) {
            out.append(",\"spans\":[],\"skipped\":").append(quote("javac threw: " + t.getClass().getSimpleName())).append('}');
            return;
        }
        if (firstError[0] >= 0 || units.isEmpty()) {
            // Only clean parses have meaningful boundaries — but WHERE it
            // failed is worth reporting for the error-position check.
            out.append(",\"spans\":[],\"skipped\":\"parse\"");
            if (firstError[0] >= 0 && firstError[0] <= src.length()) {
                out.append(",\"error\":").append(byteAt[(int) firstError[0]]);
            }
            out.append('}');
            return;
        }

        out.append(",\"spans\":[");
        SourcePositions pos = trees.getSourcePositions();
        final boolean[] first = {true};
        for (CompilationUnitTree cu : units) {
            new TreeScanner<Void, Void>() {
                @Override
                public Void scan(Tree node, Void unused) {
                    if (node == null) return null;
                    long s = pos.getStartPosition(cu, node);
                    long e = pos.getEndPosition(cu, node);
                    // NOPOS, empty spans (bare MODIFIERS), and the unit
                    // itself (a wrapper with nothing of its own to compare)
                    // have no boundary to report.
                    if (s >= 0 && e > s && e <= src.length()
                        && node.getKind() != Tree.Kind.COMPILATION_UNIT) {
                        if (!first[0]) out.append(',');
                        first[0] = false;
                        out.append('[').append(byteAt[(int) s]).append(',')
                           .append(byteAt[(int) e]).append(',')
                           .append(quote(node.getKind().name())).append(']');
                    }
                    return super.scan(node, unused);
                }
            }.scan(cu, null);
        }
        out.append("],\"has_edges\":false}");
    }

    static String quote(String s) {
        StringBuilder q = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"' -> q.append("\\\"");
                case '\\' -> q.append("\\\\");
                case '\n' -> q.append("\\n");
                case '\r' -> q.append("\\r");
                case '\t' -> q.append("\\t");
                default -> {
                    if (c < 0x20) q.append(String.format("\\u%04x", (int) c));
                    else q.append(c);
                }
            }
        }
        return q.append('"').toString();
    }
}
