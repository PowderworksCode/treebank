// Syntax-only JavaScript validity check for the treebank oracle.
//
// stdin:  one file path per line
// stdout: "<path>\tvalid|invalid" per line
//
// The reference parser is V8 itself, driven the way Node drives it:
//
//   - CommonJS leg: vm.compileFunction with Node's module wrapper argument
//     list, which is what `node --check` does for CJS. Top-level `return`
//     is therefore valid, as it is in a real CJS module.
//   - ESM leg: new vm.SourceTextModule (needs --experimental-vm-modules).
//     Construction parses only; nothing is linked or evaluated, so corpus
//     code never runs.
//   - Mode follows Node's own rule: .mjs is ESM, .cjs is CJS, .js/.jsx take
//     the nearest package.json "type" and, failing that, retry the other
//     mode (Node's detect-module behaviour).
//
// JSX is not JavaScript — V8 rejects it — but tree-sitter-javascript parses
// it, and npm ships it in .jsx and .js files. So a file V8 rejects gets one
// more chance from @babel/parser with ONLY the jsx plugin enabled. That
// plugin set accepts JSX and sloppy-mode JS while still rejecting every
// TypeScript construct, which matters: a TS-aware oracle would mark
// `const x: number = 1` valid JavaScript and turn the JS grammar's correct
// rejection of it into a reported "grammar gap".
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { parse as babelParse } from "@babel/parser";

const WRAPPER_ARGS = ["exports", "require", "module", "__filename", "__dirname"];

/** Nearest package.json "type", walking up from the file. */
const typeCache = new Map();
function packageType(file) {
  let dir = path.dirname(path.resolve(file));
  const seen = [];
  for (;;) {
    if (typeCache.has(dir)) break;
    seen.push(dir);
    const pkg = path.join(dir, "package.json");
    if (fs.existsSync(pkg)) {
      let type = "commonjs";
      try {
        type = JSON.parse(fs.readFileSync(pkg, "utf8")).type === "module" ? "module" : "commonjs";
      } catch { /* unparsable package.json: treat as commonjs, like Node */ }
      typeCache.set(dir, type);
      break;
    }
    const parent = path.dirname(dir);
    if (parent === dir) {
      typeCache.set(dir, "commonjs");
      break;
    }
    dir = parent;
  }
  const resolved = typeCache.get(dir);
  for (const d of seen) typeCache.set(d, resolved);
  return resolved;
}

function asScript(src, filename) {
  try {
    vm.compileFunction(src, WRAPPER_ARGS, { filename });
    return true;
  } catch {
    return false;
  }
}

function asModule(src, identifier) {
  try {
    new vm.SourceTextModule(src, { identifier });
    return true;
  } catch {
    return false;
  }
}

function asJsx(src, sourceType) {
  try {
    babelParse(src, { sourceType, plugins: ["jsx"], errorRecovery: false });
    return true;
  } catch {
    return false;
  }
}

// An unreadable file is NOT an invalid file. Returning false here looks
// harmless and is not: validate() is only ever called on files the grammar
// already failed, and an invalid verdict records the file as corpus NOISE.
// So a mistyped corpus root would make every path unreadable, every grammar
// failure noise, gap_files zero -- and the sweep would report a flawless
// grammar. A broken oracle must fail loudly, never quietly agree with us;
// the reasoning is spelled out in
// crates/treebank-cli/src/lang/exec_oracle.rs.
//
// The asScript/asModule/asJsx catches below are untouched: those are the
// parser rejecting the file's own content, which is what invalid means.
function check(file) {
  let src;
  try {
    src = fs.readFileSync(file, "utf8");
  } catch (e) {
    process.stderr.write(`js-oracle: cannot read ${file}: ${e.message}\n`);
    process.stderr.write("js-oracle: this is an oracle failure, not a verdict; " +
      "check the corpus root\n");
    process.exit(1);
  }
  if (src.charCodeAt(0) === 0xfeff) src = src.slice(1);
  // Node strips a shebang before compiling; vm.compileFunction does not.
  if (src.startsWith("#!")) src = "//" + src.slice(2);

  const ext = path.extname(file);
  const mode = ext === ".mjs" ? "module" : ext === ".cjs" ? "commonjs" : packageType(file);

  if (mode === "module") {
    if (asModule(src, file)) return true;
    if (ext !== ".mjs" && asScript(src, file)) return true;
  } else {
    if (asScript(src, file)) return true;
    if (ext !== ".cjs" && asModule(src, file)) return true;
  }
  return asJsx(src, mode === "module" ? "module" : "unambiguous");
}

const input = fs.readFileSync(0, "utf8");
for (const line of input.split("\n")) {
  const file = line.trim();
  if (!file) continue;
  process.stdout.write(`${file}\t${check(file) ? "valid" : "invalid"}\n`);
}
