// Node BOUNDARIES from the TypeScript compiler, for the shape check.
//
// stdin:  one file path per line
// stdout: one JSON object per line, {"path":..., "spans":[[start,end,kind],...]}
//
// The point is not to compare node NAMES across two parsers -- that needs a
// correspondence table per language and is where this kind of check usually
// dies. It is to compare where the boundaries fall. If tsc says something
// spans bytes 15..20 and our tree has no node with exactly that span, our
// tree has a different shape there, and a difference in shape is a bug in
// one of the two parsers regardless of what either calls the node.
//
// Offsets are BYTES, not UTF-16 code units. tsc counts in UTF-16 and
// tree-sitter counts in bytes; every non-ASCII character before a node
// shifts one against the other, so the conversion happens here where the
// string is already decoded.
import fs from "node:fs";
import ts from "typescript";

function scriptKind(path) {
  if (path.endsWith(".tsx")) return ts.ScriptKind.TSX;
  return ts.ScriptKind.TS;
}

// See check.mjs: an unreadable file is an oracle FAILURE, never a verdict.
function read(path) {
  try {
    return fs.readFileSync(path, "utf8");
  } catch (e) {
    process.stderr.write(`ts-oracle: cannot read ${path}: ${e.message}\n`);
    process.exit(1);
  }
}

// utf16 index -> byte offset, built in one pass.
function byteMap(src) {
  const map = new Int32Array(src.length + 1);
  let b = 0;
  let i = 0;
  while (i < src.length) {
    map[i] = b;
    const c = src.charCodeAt(i);
    if (c < 0x80) { b += 1; i += 1; }
    else if (c < 0x800) { b += 2; i += 1; }
    else if (c >= 0xd800 && c <= 0xdbff && i + 1 < src.length) {
      map[i + 1] = b;   // a position inside a surrogate pair maps to its start
      b += 4; i += 2;
    } else { b += 3; i += 1; }
  }
  map[src.length] = b;
  return map;
}

// `ts.SyntaxKind[n]` returns the FIRST name with that numeric value, and the
// enum is full of range markers aliased onto real kinds -- SyntaxKind.
// FirstStatement is VariableStatement, FirstNode is QualifiedName. Reading
// those back gives a mapping table full of names that describe nothing.
// Build the reverse table once, preferring any name that is not a marker.
const KIND_NAME = (() => {
  const names = {};
  for (const [name, value] of Object.entries(ts.SyntaxKind)) {
    if (typeof value !== "number") continue;
    const marker = /^(First|Last)/.test(name);
    if (names[value] === undefined || (marker === false && /^(First|Last)/.test(names[value]))) {
      names[value] = name;
    }
  }
  return names;
})();

const input = fs.readFileSync(0, "utf8");
for (const line of input.split("\n")) {
  const path = line.trim();
  if (!path) continue;
  const src = read(path);
  let out = { path, spans: [] };
  try {
    const sf = ts.createSourceFile(path, src, ts.ScriptTarget.Latest, true, scriptKind(path));
    // Internal to the compiler, and absent from its public types; see the
    // header on why a syntax-only verdict has to come from here.
    const diags = /** @type {any} */ (sf).parseDiagnostics ?? [];
    if (diags.length > 0) {
      // Only clean parses have meaningful boundaries.
      out.skipped = "parse errors";
      // ...but WHERE it failed is worth reporting. Rejecting the right files
      // at the wrong offset makes error recovery useless to an editor, and
      // nothing has ever checked it.
      const first = diags.reduce((a, b) => (b.start < a.start ? b : a));
      if (typeof first.start === "number") {
        out.error = byteMap(src)[first.start] ?? first.start;
      }
    } else {
      const m = byteMap(src);
      const spans = [];
      const edges = [];
      // Labelled parent -> child edges. Spans say what is there; edges say
      // how it is connected, and two trees can agree on every node while
      // attaching the children under different names -- which is the
      // difference a consumer actually reads.
      //
      // tsc has no field-name API, but a node's named children ARE its own
      // enumerable properties. `parent` has to go (it points back up and
      // would make the walk infinite), and so does everything that is not a
      // node: `pos`, `end`, `kind` and the flag caches are numbers.
      const isNode = (v) =>
        v !== null && typeof v === "object" && typeof v.kind === "number" &&
        typeof v.pos === "number" && typeof v.end === "number";
      const walk = (node) => {
        const s = node.getStart(sf);
        const e = node.getEnd();
        // Zero-width nodes have no boundary to compare.
        if (e > s) {
          spans.push([m[s], m[e], KIND_NAME[node.kind] ?? String(node.kind)]);
          const pk = KIND_NAME[node.kind] ?? String(node.kind);
          for (const key of Object.keys(node)) {
            if (key === "parent" || key === "original" || key === "symbol") continue;
            const value = node[key];
            const children = Array.isArray(value) ? value : [value];
            for (const child of children) {
              if (!isNode(child)) continue;
              const cs = child.getStart(sf);
              const ce = child.getEnd();
              if (ce > cs) edges.push([m[s], m[e], pk, key, m[cs], m[ce]]);
            }
          }
        }
        ts.forEachChild(node, walk);
      };
      ts.forEachChild(sf, walk);
      out.spans = spans;
      out.edges = edges;
    }
  } catch (e) {
    out.skipped = `oracle threw: ${e.message}`;
  }
  process.stdout.write(JSON.stringify(out) + "\n");
}
