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

// An unreadable file is NOT an invalid file. Returning "invalid" for one
// looks harmless and is not: validate() is only ever called on files the
// grammar already failed, and an invalid verdict records the file as corpus
// NOISE. So a mistyped corpus root would make every path unreadable, every
// grammar failure noise, gap_files zero -- and the sweep would report a
// flawless grammar. A broken oracle must fail loudly, never quietly agree
// with us; the reasoning is spelled out in
// crates/treebank-cli/src/lang/exec_oracle.rs.
function read(path) {
  try {
    return fs.readFileSync(path, "utf8");
  } catch (e) {
    process.stderr.write(`ts-oracle: cannot read ${path}: ${e.message}\n`);
    process.stderr.write("ts-oracle: this is an oracle failure, not a verdict; " +
      "check the corpus root\n");
    process.exit(1);
  }
}

const input = fs.readFileSync(0, "utf8");
for (const line of input.split("\n")) {
  const path = line.trim();
  if (!path) continue;
  // An unreadable file is NOT an invalid file -- see read() above.
  const src = read(path);
  let ok = false;
  try {
    const sf = ts.createSourceFile(path, src, ts.ScriptTarget.Latest, false, scriptKind(path));
    ok = (sf.parseDiagnostics ?? []).length === 0;
  } catch {
    ok = false;
  }
  process.stdout.write(`${path}\t${ok ? "valid" : "invalid"}\n`);
}
