// The renderer is total over grammar.json's node kinds, or it is not
// documentation.
//
// `toEbnf` and `toRr` throw on a node kind they do not know, so running them
// over every rule of every grammar is the whole check: the day a grammar
// starts using a DSL construct the fold has never seen, this fails instead of
// quietly dropping part of a production from its page. A missing case is
// invisible in the output -- a rule renders three quarters of itself and
// looks fine -- which is why it has to be caught here.
//
// The grammar list is discovered, never written down. A hand-kept list would
// mean a new grammar's reference is unrendered and nothing says so.

import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import {
  Grammar,
  groupsOf,
  precedences,
  termIndex,
  toEbnf,
  toRr,
} from "../src/grammar.mjs";
import { diagram } from "../src/railroad.mjs";

const DATA = join(import.meta.dir, "..", "public", "grammars");

function bundles() {
  let files: string[];
  try {
    files = readdirSync(DATA);
  } catch {
    throw new Error(`no ${DATA}; run \`bun run grammars\` first`);
  }
  const found = files.filter((f) => f.endsWith(".json") && f !== "index.json");
  if (!found.length) throw new Error(`no grammar bundles in ${DATA}`);
  return found.map((f) => [f.replace(/\.json$/, ""), join(DATA, f)] as const);
}

describe("every grammar renders", () => {
  for (const [name, where] of bundles()) {
    test(`${name}`, () => {
      const g = new Grammar(JSON.parse(readFileSync(where, "utf8")));
      const rules = Object.entries(g.rules);
      expect(rules.length).toBeGreaterThan(0);

      for (const [rule, body] of rules) {
        // Throws on an unknown node kind. That is the point.
        expect(() => toEbnf(body, g)).not.toThrow();
        const node = toRr(body, g);

        // Geometry a browser cannot draw: NaN, negative, or runaway. A node
        // sized by one formula and drawn by another shows up here as a width
        // that is not a number long before it shows up as a broken picture.
        for (const dim of ["width", "up", "down"] as const) {
          const v = node[dim];
          expect(Number.isFinite(v), `${rule}: ${dim}=${v}`).toBe(true);
          expect(v >= 0 && v < 1e5, `${rule}: ${dim}=${v}`).toBe(true);
        }
      }
    });
  }
});

describe("every grammar assembles a page", () => {
  for (const [name, where] of bundles()) {
    test(`${name}`, () => {
      const g = new Grammar(JSON.parse(readFileSync(where, "utf8")));
      const { structural: table } = termIndex(g);
      const groups = groupsOf(g, table);

      // Every rule appears exactly once across the groups: the index rail and
      // the production list are built from the same grouping, so a rule that
      // is listed twice is shown twice, and one that is listed never is
      // documentation that silently omits a production.
      const listed = groups.flatMap(([, members]) => members);
      expect(new Set(listed).size).toBe(listed.length);
      expect(new Set(listed)).toEqual(new Set(Object.keys(g.rules)));

      expect(precedences(g)).toBeInstanceOf(Map);

      // Drawing is the half `toRr` does not exercise: sizes are computed in
      // the constructor and read back by draw, and the two must agree.
      for (const [, body] of Object.entries(g.rules)) {
        const svg = diagram(toRr(body, g), "t");
        expect(svg.startsWith("<svg")).toBe(true);
        expect(svg).not.toContain("NaN");
      }
    });
  }
});
