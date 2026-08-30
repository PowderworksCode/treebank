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

// A batch ends at EOF or at the sentinel; see py-oracle/check.py for why.
// Read line by line rather than to EOF, because a persistent oracle must
// answer a batch while its caller still holds the pipe open.
const SENTINEL = "\u0000--end--";

function verdict(path) {
  // An unreadable file is NOT an invalid file -- see read() above.
  const src = read(path);
  try {
    const sf = ts.createSourceFile(path, src, ts.ScriptTarget.Latest, false, scriptKind(path));
    // parseDiagnostics is internal to the compiler and absent from its
    // public types, but it is the only purely syntactic verdict it exposes.
    return (/** @type {any} */ (sf).parseDiagnostics ?? []).length === 0;
  } catch {
    return false;
  }
}

let pending = "";
process.stdin.on("data", (chunk) => {
  pending += chunk;
  let nl;
  while ((nl = pending.indexOf("\n")) >= 0) {
    const path = pending.slice(0, nl).trim();
    pending = pending.slice(nl + 1);
    if (path === SENTINEL) {
      process.stdout.write(SENTINEL + "\n");
      continue;
    }
    if (!path) continue;
    process.stdout.write(`${path}\t${verdict(path) ? "valid" : "invalid"}\n`);
  }
});
process.stdin.on("end", () => {
  const path = pending.trim();
  if (path && path !== SENTINEL) {
    process.stdout.write(`${path}\t${verdict(path) ? "valid" : "invalid"}\n`);
  }
});
