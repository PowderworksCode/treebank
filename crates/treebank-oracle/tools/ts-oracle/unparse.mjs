// Re-render each file from the TypeScript compiler's own AST.
//
// stdin:  one file path per line
// stdout: one JSON object per line, {"path":..., "source":...} or {"path":..., "skipped":...}
//
// `ts.createPrinter` prints the tree back in ONE canonical spelling.
// Parsing that with our grammar asks a question the corpus cannot: whether we
// handle each construct in the form the language's own tools emit, rather
// than only in the form its authors happened to write.
import fs from "node:fs";
import ts from "typescript";

function scriptKind(path) {
  if (path.endsWith(".tsx")) return ts.ScriptKind.TSX;
  return ts.ScriptKind.TS;
}

const printer = ts.createPrinter({ newLine: ts.NewLineKind.LineFeed });
const input = fs.readFileSync(0, "utf8");
for (const line of input.split("\n")) {
  const path = line.trim();
  if (!path) continue;
  let src;
  try {
    src = fs.readFileSync(path, "utf8");
  } catch (e) {
    process.stderr.write(`ts-oracle: cannot read ${path}: ${e.message}\n`);
    process.exit(1);
  }
  const out = { path };
  try {
    const sf = ts.createSourceFile(path, src, ts.ScriptTarget.Latest, true, scriptKind(path));
    if ((sf.parseDiagnostics ?? []).length > 0) {
      out.skipped = "parse";
    } else {
      out.source = printer.printFile(sf);
    }
  } catch (e) {
    out.skipped = `print: ${e.message}`;
  }
  process.stdout.write(JSON.stringify(out) + "\n");
}
