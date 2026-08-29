// Differential driver for src/expand.mjs.
//
// Reads a JSON array of {query, facets} on stdin and writes a JSON array of
// {ok: true, value} | {ok: false, error}. Driven by
// crates/treebank/tests/expand_parity.rs, which runs the same cases through
// the Rust and fails on any difference.
//
// One process for the whole batch: the point is to make the check cheap
// enough that nobody is tempted to skip it.

import { expandQuery } from "../src/expand.mjs";

const input = await new Promise((resolve, reject) => {
  let buf = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (d) => (buf += d));
  process.stdin.on("end", () => resolve(buf));
  process.stdin.on("error", reject);
});

const out = JSON.parse(input).map(({ query, facets }) => {
  try {
    return { ok: true, value: expandQuery(query, facets) };
  } catch (e) {
    return { ok: false, error: String(e.message ?? e) };
  }
});

process.stdout.write(JSON.stringify(out));
