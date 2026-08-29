#!/usr/bin/env node
// Derive the viewer's grammar bundles, and the page for each, from the
// crates themselves.
//
// The list is not written down here. A directory under crates/ with a
// grammar.js in it IS a grammar -- the same rule tools/wasm-pack/list-
// grammars.sh applies and the same one the CI matrix is built from. A
// hand-kept list would mean a new grammar's reference page is missing and
// nothing says so, which is the failure a derived list exists to prevent.
//
// Each bundle is the three source files joined and pruned to what the viewer
// reads, so a page makes one request instead of three and carries no field
// it will not use.

import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const HERE = path.dirname(new URL(import.meta.url).pathname);
const SITE = path.join(HERE, "..");
const ROOT = path.join(SITE, "..");

// A directory under crates/ with a grammar.js in it is a grammar.
async function discover() {
  const entries = await readdir(path.join(ROOT, "crates"), { withFileTypes: true });
  const found = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const dir = path.join(ROOT, "crates", entry.name);
    try {
      await readFile(path.join(dir, "grammar.js"));
    } catch {
      continue;
    }
    found.push({ name: entry.name.replace(/^treebank-/, ""), dir });
  }
  return found.sort((a, b) => a.name.localeCompare(b.name));
}

const readJson = async (where) => JSON.parse(await readFile(where, "utf8"));
const maybeJson = async (where) => readJson(where).catch(() => null);

// Only what the viewer reads. grammar.json carries `inline`, `extras`,
// `precedences` and more that the page never asks about; node-types.json
// carries per-node `fields` and `children` the vocabulary index does not
// use. Shipping them would roughly double the bytes for nothing.
function prune(grammar, nodeTypes, roles) {
  return {
    grammar: {
      name: grammar.name,
      rules: grammar.rules,
      supertypes: grammar.supertypes ?? [],
      externals: grammar.externals ?? [],
      word: grammar.word,
      conflicts: grammar.conflicts ?? [],
    },
    nodeTypes: nodeTypes
      .filter((n) => n.named || n.subtypes?.length)
      .map((n) => ({
        type: n.type,
        named: Boolean(n.named),
        ...(n.subtypes?.length
          ? { subtypes: n.subtypes.map((s) => ({ type: s.type })) }
          : {}),
      })),
    roles: roles?.facets ? { facets: roles.facets } : {},
  };
}

// How each language spells its own name. A grammar missing from here falls
// back to its directory name, which is a plain-looking page rather than a
// broken one -- the identifier is always correct even when the display name
// has not been filled in.
const NAMES = {
  bash: "Bash",
  c: "C",
  cpp: "C++",
  java: "Java",
  python: "Python",
  ruby: "Ruby",
  rust: "Rust",
  typescript: "TypeScript",
  zig: "Zig",
};

const PAGE = (name, productions) => `---
title: ${NAMES[name] ?? name}
description: Every production in the ${NAMES[name] ?? name} parse table, as EBNF and as a railroad diagram.
---

${productions} productions, generated from
\`crates/treebank-${name}/src/grammar.json\`. Every production the parser has
is on this page.

\`${name}\` is the identifier commands take, as in
\`treebank verify --grammar crates/treebank-${name}\`. The grammar runs in a
browser on the [playground](/playground/?g=${name}), and downloads as
[\`treebank-${name}.wasm\`](/packs/treebank-${name}.wasm).

The parse table this page is drawn from is at
[\`/grammars/${name}.json\`](/grammars/${name}.json) — read that instead if you
are not running JavaScript.

<link rel="stylesheet" href="/grammar.css">
<div class="grammar-viewer" data-grammar="${name}">
  <p class="grammar-loading">Loading the ${name} parse table…</p>
</div>
<script type="module" src="/grammar-viewer.mjs"></script>
`;

async function main() {
  const grammars = await discover();
  if (!grammars.length) throw new Error("no grammars found under crates/");

  const dataDir = path.join(SITE, "public", "grammars");
  const pageDir = path.join(SITE, "content", "grammars");
  await rm(dataDir, { recursive: true, force: true });
  await mkdir(dataDir, { recursive: true });
  await mkdir(pageDir, { recursive: true });

  // Pages are generated, so a grammar that goes away does not leave its page
  // behind. index.md is written by hand and stays.
  for (const stale of await readdir(pageDir).catch(() => [])) {
    if (stale !== "index.md") await rm(path.join(pageDir, stale));
  }

  const index = [];
  for (const { name, dir } of grammars) {
    const grammar = await readJson(path.join(dir, "src", "grammar.json"));
    const nodeTypes = await readJson(path.join(dir, "src", "node-types.json"));
    const roles = await maybeJson(path.join(dir, "roles.json"));

    const bundle = prune(grammar, nodeTypes, roles);
    const json = JSON.stringify(bundle);
    await writeFile(path.join(dataDir, `${name}.json`), json);
    const productions = Object.keys(bundle.grammar.rules).length;
    await writeFile(path.join(pageDir, `${name}.md`), PAGE(name, productions));

    index.push({ name, productions, bytes: Buffer.byteLength(json) });
  }

  await writeFile(
    path.join(dataDir, "index.json"),
    JSON.stringify(index.map(({ name, productions }) => ({ name, productions }))),
  );

  const total = index.reduce((n, g) => n + g.productions, 0);
  for (const g of index) {
    console.log(
      `  ${g.name.padEnd(11)} ${String(g.productions).padStart(4)} productions  ` +
        `${(g.bytes / 1024).toFixed(0).padStart(4)} KiB`,
    );
  }
  console.log(`grammars: ${index.length}, ${total} productions`);
}

await main();
