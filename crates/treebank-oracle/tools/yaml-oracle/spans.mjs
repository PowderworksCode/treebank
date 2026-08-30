// Node BOUNDARIES from the `yaml` package, for the shape check.
//
// stdin:  one file path per line
// stdout: one JSON object per line, {"path":..., "spans":[[start,end,kind],...]}
//
// The point is not to compare node NAMES across two parsers — that needs a
// correspondence table per language and is where this kind of check usually
// dies. It is to compare where the BOUNDARIES fall. If the reference parser
// says something spans bytes 15..20 and our tree has no node with exactly
// that span, our tree has a different shape there, whatever either of us
// calls the node.
//
// This is the capability the YAML ledger promised and did not yet have. A
// yes/no oracle cannot see a file that parses cleanly into the wrong tree,
// and YAML has more ways to do that than most languages: every one of them
// is a column, and a column is exactly what a boundary records.
//
// Offsets are BYTES. The `yaml` package counts UTF-16 code units and
// tree-sitter counts bytes, so every non-ASCII character before a node
// shifts one against the other; the conversion happens here where the
// string is already decoded.
import fs from "node:fs";
import YAML from "yaml";

// See check.mjs: an unreadable file is an oracle FAILURE, never a verdict.
function read(path) {
  try {
    return fs.readFileSync(path, "utf8");
  } catch (e) {
    process.stderr.write(`yaml-oracle: cannot read ${path}: ${e.message}\n`);
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
      map[i + 1] = b; // a position inside a surrogate pair maps to its start
      b += 4; i += 2;
    } else { b += 3; i += 1; }
  }
  map[src.length] = b;
  return map;
}

// A node's `range` is `[start, valueEnd, nodeEnd]`: the third includes the
// comment and whitespace that trail the node, which belong to no node in
// our tree. `valueEnd` is the boundary worth comparing.
function kindOf(node, src) {
  if (YAML.isMap(node)) return node.flow ? "flow_mapping" : "block_mapping";
  if (YAML.isSeq(node)) return node.flow ? "flow_sequence" : "block_sequence";
  if (YAML.isAlias(node)) return "alias";
  if (YAML.isScalar(node)) {
    switch (node.type) {
      case "QUOTE_SINGLE": return "single_quote_scalar";
      case "QUOTE_DOUBLE": return "double_quote_scalar";
      case "BLOCK_LITERAL":
      case "BLOCK_FOLDED": return "block_scalar";
      default: return "plain_scalar";
    }
  }
  return "node";
}

// Where a node's CONTENT ends.
//
// `range[1]` is the node's "value end", and for a collection that turns out
// to include the comment trailing its last entry — the `yaml` package hangs
// a comment off the node it follows. A comment is not part of the sequence
// it sits after, and our tree does not put it there, so comparing against
// `range[1]` would report a boundary difference on every commented
// collection in the corpus and hide whatever real ones are behind it.
//
// The content end is the last entry's content end, recursively. Scalars and
// aliases are their own answer.
function contentEnd(node) {
  if (!node) return undefined;
  if (YAML.isPair(node)) return contentEnd(node.value) ?? contentEnd(node.key);
  if (YAML.isCollection(node)) {
    for (let i = node.items.length - 1; i >= 0; i--) {
      const end = contentEnd(node.items[i]);
      if (end !== undefined) return end;
    }
    return node.range ? node.range[1] : undefined;
  }
  return node.range ? node.range[1] : undefined;
}

// A document's directives belong to it — `l-directive-document` in the
// specification is `l-directive+ l-explicit-document` — and our tree puts
// them inside the `document` node. The `yaml` package starts the document
// at its `---` and hands the directives to a separate object with no range
// of its own, so the boundary is walked back over any run of `%` lines
// immediately above the marker. Declaring the difference instead would have
// meant an ignore entry reading `document <- document`, which is every
// document disagreement there could ever be.
function documentStart(src, start) {
  let at = start;
  for (;;) {
    // Step back over the line break above `at`, then to that line's start.
    let end = at;
    if (end > 0 && src[end - 1] === "\n") end--;
    if (end > 0 && src[end - 1] === "\r") end--;
    if (end === at) return at;
    let begin = end;
    while (begin > 0 && src[begin - 1] !== "\n") begin--;
    if (src[begin] !== "%") return at;
    at = begin;
  }
}

function spansOf(src) {
  const out = [];
  const map = byteMap(src);
  const docs = YAML.parseAllDocuments(src, { prettyErrors: false });
  for (const doc of docs) {
    if (doc.errors.length > 0) return { skipped: doc.errors[0].code || "error" };
  }
  const push = (start, end, kind) => {
    if (typeof start !== "number" || typeof end !== "number") return;
    if (end <= start) return; // an empty node has no boundary to compare
    out.push([map[start], map[end], kind]);
  };
  const pushNode = (node, kind) => {
    if (!node || !node.range) return;
    push(node.range[0], contentEnd(node), kind);
  };
  for (const doc of docs) {
    if (doc.range) {
      push(documentStart(src, doc.range[0]), contentEnd(doc.contents) ?? doc.range[1], "document");
    }
    YAML.visit(doc, {
      Map: (_k, node) => pushNode(node, kindOf(node, src)),
      Seq: (_k, node) => pushNode(node, kindOf(node, src)),
      Scalar: (_k, node) => pushNode(node, kindOf(node, src)),
      Alias: (_k, node) => pushNode(node, kindOf(node, src)),
      // A Pair carries no range of its own; it spans from its key to the
      // end of its value. A pair with either half empty has no boundary
      // that both parsers could agree on, so it contributes none.
      Pair: (_k, pair) => {
        const k = pair.key && pair.key.range;
        if (!k || !pair.value) return;
        push(k[0], contentEnd(pair.value), "pair");
      },
    });
  }
  return { spans: out };
}

let buffer = "";
process.stdin.setEncoding("utf8");
function answer(path) {
  const src = read(path);
  let result;
  try {
    result = spansOf(src);
  } catch (e) {
    result = { skipped: e.message || "threw" };
  }
  // `has_edges: false` — the `yaml` package names a Pair's halves `key` and
  // `value`, which are the only two field names in the language, and both
  // are already implied by the pair boundary. There is no second field
  // vocabulary here for an edge check to disagree about.
  process.stdout.write(
    JSON.stringify({ path, has_edges: false, ...result }) + "\n",
  );
}
process.stdin.on("data", (chunk) => {
  buffer += chunk;
  let nl;
  while ((nl = buffer.indexOf("\n")) >= 0) {
    const path = buffer.slice(0, nl);
    buffer = buffer.slice(nl + 1);
    if (path.length > 0) answer(path);
  }
});
process.stdin.on("end", () => {
  if (buffer.length > 0) answer(buffer);
});
