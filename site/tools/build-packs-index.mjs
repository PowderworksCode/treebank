#!/usr/bin/env node
// Content-address whatever packs are staged in public/packs, and write the
// manifest that points at the current one for each grammar.
//
// R2 has no symlinks, so "latest" is a pointer in a document rather than a
// second name for an object. That is the better shape anyway: duplicating the
// bytes under a mutable name would mean two objects that can disagree, where
// a manifest entry either resolves or does not.
//
// The key carries a hash because a pack is BYTE-REPRODUCIBLE -- the build
// asserts it, twice, from different paths -- so the same grammar always
// produces the same key, and a key that exists can never be the wrong bytes.
// That is what makes a pinned URL worth having: `?pack=<hash>` is not a
// convenience, it is the only way a bug report can name what it hit.
//
// Used locally to make `wrangler dev` behave like production. In CI the same
// shape is written from the packs the wasm gate already built and checked.

import { createHash } from "node:crypto";
import { copyFile, link, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const HERE = path.dirname(new URL(import.meta.url).pathname);
const PACKS = path.join(HERE, "..", "public", "packs");

export function keyFor(name, sha256) {
  return `treebank-${name}-${sha256.slice(0, 12)}.wasm`;
}

async function main() {
  let files;
  try {
    // Only the unhashed originals; the hashed names are this script's output.
    files = (await readdir(PACKS)).filter((f) => /^treebank-[a-z0-9]+\.wasm$/.test(f));
  } catch {
    console.log("packs: none staged; skipping the manifest");
    return;
  }
  if (!files.length) {
    console.log("packs: none staged; skipping the manifest");
    return;
  }

  const packs = {};
  for (const file of files.sort()) {
    const name = file.replace(/^treebank-/, "").replace(/\.wasm$/, "");
    const bytes = await readFile(path.join(PACKS, file));
    const sha256 = createHash("sha256").update(bytes).digest("hex");
    const key = keyFor(name, sha256);
    packs[name] = { sha256, key, bytes: bytes.length };

    // Materialise the hashed name locally, so a checkout without R2 serves
    // the same URLs production does and pinning can be exercised offline.
    // Hardlinked because these are the same bytes by construction; a copy
    // would double 14 MB for nothing.
    const target = path.join(PACKS, key);
    await rm(target, { force: true });
    await link(path.join(PACKS, file), target)
      .catch(() => copyFile(path.join(PACKS, file), target));

    console.log(
      `  ${name.padEnd(11)} ${sha256.slice(0, 12)}  ${(bytes.length / 1024).toFixed(0).padStart(5)} KiB`,
    );
  }

  // schema_version so a consumer that finds this document later can tell
  // whether it understands it, rather than guessing from the shape.
  const manifest = { schema_version: 1, packs };
  await writeFile(path.join(PACKS, "index.json"), JSON.stringify(manifest, null, 1) + "\n");
  console.log(`packs: ${Object.keys(packs).length} in the manifest`);
}

await main();
