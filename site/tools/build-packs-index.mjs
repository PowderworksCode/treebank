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
import {
  copyFile,
  link,
  mkdir,
  readdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";

const HERE = path.dirname(new URL(import.meta.url).pathname);
const PACKS = path.join(HERE, "..", "public", "packs");

// The branch Cloudflare treats as production. Everything else is a preview
// and reads one prefix further into the bucket.
const PRODUCTION_BRANCH = process.env.PACKS_PRODUCTION_BRANCH ?? "main";

// Tell the Worker which prefix this deployment reads, or that it reads none.
//
// A branch that adds a grammar has no pack for it in the bucket, because
// packs are published from main -- so the playground on that branch's preview
// URL cannot load the thing the branch exists to add. CI publishes those
// under `previews/<sha>/`; this writes the sha where the Worker can find it.
//
// It is an ASSET rather than a var because that is what makes it safe: it is
// fixed for the life of a deployment and no request can ask to be served from
// somewhere else. Production writes no marker and the file is removed if one
// is lying around, so a stale local preview cannot survive into a real build.
//
// `WORKERS_CI_BRANCH` and `WORKERS_CI_COMMIT_SHA` are injected by Cloudflare
// Workers Builds. Outside it -- a laptop, `wrangler dev` -- neither is set,
// nothing is written, and the packs staged in public/ are what gets served.
async function writePreviewMarker() {
  const marker = path.join(PACKS, "preview.json");
  const branch = process.env.WORKERS_CI_BRANCH ?? "";
  const sha = process.env.WORKERS_CI_COMMIT_SHA ?? "";
  if (
    !branch ||
    branch === PRODUCTION_BRANCH ||
    !/^[0-9a-f]{7,40}$/.test(sha)
  ) {
    await rm(marker, { force: true });
    return;
  }
  await mkdir(PACKS, { recursive: true });
  await writeFile(
    marker,
    JSON.stringify({ schema_version: 1, prefix: `previews/${sha}/`, branch }) +
      "\n",
  );
  console.log(`packs: preview build on ${branch}, reading previews/${sha}/`);
}

export function keyFor(name, sha256) {
  return `treebank-${name}-${sha256.slice(0, 12)}.wasm`;
}

async function main() {
  await writePreviewMarker();

  let files;
  try {
    // Only the unhashed originals; the hashed names are this script's output.
    files = (await readdir(PACKS)).filter((f) =>
      /^treebank-[a-z0-9]+\.wasm$/.test(f),
    );
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
    await link(path.join(PACKS, file), target).catch(() =>
      copyFile(path.join(PACKS, file), target),
    );

    console.log(
      `  ${name.padEnd(11)} ${sha256.slice(0, 12)}  ${(bytes.length / 1024).toFixed(0).padStart(5)} KiB`,
    );
  }

  // schema_version so a consumer that finds this document later can tell
  // whether it understands it, rather than guessing from the shape.
  const manifest = { schema_version: 1, packs };
  await writeFile(
    path.join(PACKS, "index.json"),
    JSON.stringify(manifest, null, 1) + "\n",
  );
  console.log(`packs: ${Object.keys(packs).length} in the manifest`);
}

await main();
