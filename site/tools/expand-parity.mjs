// Differential driver for src/expand.mjs.
//
// Reads {grammars: [{name, facets, nodeTypes}], cases: [{grammar, query,
// filtered}]} on stdin and writes a JSON array of {ok: true, value} |
// {ok: false, error}. Driven by crates/treebank/tests/expand_parity.rs, which
// runs the same cases through the Rust and fails on any difference.
//
// node-types is sent once per grammar rather than per case: it is 40-70 KB and
// there are hundreds of cases. One process for the whole batch, so the check
// stays cheap enough that nobody is tempted to skip it.

import { expandQuery, parseNodeTypes } from "../src/expand.mjs";

const input = await new Promise((resolve, reject) => {
  let buf = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (d) => (buf += d));
  process.stdin.on("end", () => resolve(buf));
  process.stdin.on("error", reject);
});

const { grammars, cases } = JSON.parse(input);
const prepared = grammars.map((g) => ({
  facets: g.facets,
  nodeTypes: g.nodeTypes ? parseNodeTypes(g.nodeTypes) : null,
}));

const out = cases.map(({ grammar, query, filtered }) => {
  const g = prepared[grammar];
  try {
    return { ok: true, value: expandQuery(query, g.facets, filtered ? g.nodeTypes : null) };
  } catch (e) {
    return { ok: false, error: String(e.message ?? e) };
  }
});

process.stdout.write(JSON.stringify(out));
