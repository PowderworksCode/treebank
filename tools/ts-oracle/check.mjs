// Syntax-only TypeScript validity check for the treebank oracle.
//
// stdin:  one file path per line
// stdout: "<path>\tvalid|invalid" per line
//
// Parses with ts.createSourceFile and reads parseDiagnostics — purely
// syntactic, so type errors don't make a file "invalid", and .d.ts files
// work (ts.transpileModule throws on them because they emit no output).
// ScriptKind routes .tsx through the JSX parser.
import fs from "node:fs";
import ts from "typescript";

function scriptKind(path) {
  if (path.endsWith(".tsx")) return ts.ScriptKind.TSX;
  return ts.ScriptKind.TS;
}

const input = fs.readFileSync(0, "utf8");
for (const line of input.split("\n")) {
  const path = line.trim();
  if (!path) continue;
  let ok = false;
  try {
    const src = fs.readFileSync(path, "utf8");
    const sf = ts.createSourceFile(path, src, ts.ScriptTarget.Latest, false, scriptKind(path));
    ok = (sf.parseDiagnostics ?? []).length === 0;
  } catch {
    ok = false;
  }
  process.stdout.write(`${path}\t${ok ? "valid" : "invalid"}\n`);
}
