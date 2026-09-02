// The playground's grammar list is DERIVED and its samples are not, so this
// is where the two are made to agree.
//
// `build-grammars.mjs` builds the dropdown from the crates — a directory with
// a grammar.js in it is a grammar — which is the right shape and has one
// consequence nobody sees until they look: a language that arrives without a
// sample is in the menu, loads its parser, and shows an empty editor. Both
// `yaml` and `hcl` shipped exactly that way, on production, and nothing said
// so. This is what says so.
import { describe, expect, test } from "bun:test";
import { readdirSync, existsSync } from "node:fs";
import { join } from "node:path";

import { SAMPLES } from "../src/samples.mjs";

const CRATES = join(import.meta.dir, "..", "..", "crates");

// The same rule build-grammars.mjs and tools/wasm-pack/list-grammars.sh use.
const grammars = readdirSync(CRATES)
  .filter((name) => existsSync(join(CRATES, name, "grammar.js")))
  .map((name) => name.replace(/^treebank-/, ""))
  .sort();

describe("playground samples", () => {
  test("there is at least one grammar to check", () => {
    expect(grammars.length).toBeGreaterThan(0);
  });

  for (const name of grammars) {
    test(`${name} has a sample`, () => {
      const sample = (SAMPLES as Record<string, string | undefined>)[name];
      expect(sample, `no playground sample for ${name}`).toBeDefined();
      // A blank one would pass the key check and fail the reader.
      expect((sample ?? "").trim().length).toBeGreaterThan(0);
    });
  }

  // The other direction: a sample for a grammar that no longer exists is
  // dead weight nobody would think to remove.
  test("every sample names a grammar", () => {
    expect(Object.keys(SAMPLES).sort()).toEqual(grammars);
  });
});
