#!/usr/bin/env node
// Snapshot `treebank status` for the site.
//
// The site is built by Cloudflare Workers Build, which has no Rust toolchain,
// so the inventory cannot be produced at deploy time. It is generated here
// from the real command -- never reimplemented in JavaScript, because a second
// implementation of what counts as stale evidence is a second answer waiting
// to disagree with the first -- and committed.
//
// A committed snapshot can go stale, so CI regenerates it and fails on a
// difference, the same way the corpus ledgers are guarded. That is the trade:
// the numbers on the site are as fresh as the last commit, and cannot be
// quietly older than that.
//
//     bun run status      # regenerate, needs ./target/debug/treebank
//
// Pruned to what the pages show. The full inventory carries ledger prose --
// several thousand words per grammar of measurement notes -- that belongs in
// the repository rather than in every page load.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { writeFile } from "node:fs/promises";
import path from "node:path";

const HERE = path.dirname(new URL(import.meta.url).pathname);
const SITE = path.join(HERE, "..");
const ROOT = path.join(SITE, "..");
const BINARY = path.join(ROOT, "target", "debug", "treebank");

function inventory() {
  if (!existsSync(BINARY)) {
    throw new Error(
      `no ${BINARY}\n` +
        "  cargo build -p treebank-cli   # then re-run `bun run status`",
    );
  }
  // --check is deliberately NOT passed: a repository with a real
  // configuration problem should still publish its inventory, saying so,
  // rather than failing to build the page that would have shown it.
  return JSON.parse(
    execFileSync(BINARY, ["status", "--format", "json", "--root", ROOT], {
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    }),
  );
}

// A grammar's lint runs against its lint_policy.toml ratchets where one
// exists and is advisory where none does. That distinction is not in the
// inventory, and it is exactly the one a reader wants, so it is taken from
// the checkout by the same rule CI applies: the file is there, or it is not.
const ratchet = (name) =>
  existsSync(path.join(ROOT, "crates", `treebank-${name}`, "lint_policy.toml"));

const corpus = (c) => ({
  language: c.language,
  files: c.files,
  passed: c.passed,
  failed: c.failed,
  grammar_gaps: c.grammar_gaps,
  noise: c.noise,
  pass_rate: c.pass_rate,
  freshness: c.freshness,
  freshness_reasons: c.freshness_reasons ?? [],
  grammar_revision: c.grammar_revision,
});

// The gaps, widenings and deviations are the honest half of the ledger: where
// the grammar is losing, where it accepts more than the language does, and
// where it knowingly differs from the reference parser. They are the reason to
// publish an inventory rather than a pass rate.
//
// The inventory calls the field `summary`; the ledger's longer prose note for
// each one stays in the repository, where a reader who wants the reasoning can
// find it next to the grammar it is about.
const gap = (g) => ({ summary: g.summary ?? null, files: g.files ?? null });

async function main() {
  const raw = inventory();
  const grammars = {};
  for (const g of raw.grammars) {
    grammars[g.grammar] = {
      versions: g.versions ?? null,
      generate_cli: g.generate_cli ?? null,
      vocabulary: g.vocabulary ?? null,
      languages: (g.languages ?? []).map((l) => l.name),
      capabilities: g.capabilities ?? {},
      roles: g.roles ?? {},
      tests: g.tests ?? {},
      known_deviations: g.known_deviations ?? {},
      distribution: g.distribution ?? {},
      corpus_lock: Boolean(g.corpus_lock),
      corpus_canary: Boolean(g.corpus_canary),
      external_scanner: Boolean(g.external_scanner),
      evidence_freshness: g.evidence_freshness ?? null,
      lint_ratchet: ratchet(g.grammar),
      corpora: (g.evidence?.corpora ?? []).map(corpus),
      known_gaps: (g.evidence?.known_gaps ?? []).map(gap),
      known_widenings: (g.evidence?.known_widenings ?? []).map(gap),
      deviations: (g.evidence?.deviations ?? []).map(gap),
    };
  }

  // `revision` is deliberately dropped. It is the repository's HEAD, so
  // committing this snapshot changes it and instantly invalidates the file
  // that was just written -- a freshness check against it can never pass.
  // The revision that actually matters is per corpus: `grammar_revision`
  // says which grammar the evidence was measured against, changes only when
  // that grammar does, and is what `freshness` is computed from.
  const out = {
    schema_version: raw.schema_version ?? null,
    summary: raw.summary ?? {},
    warnings: raw.warnings ?? [],
    errors: raw.errors ?? [],
    grammars,
  };

  const where = path.join(SITE, "public", "status.json");
  await writeFile(where, JSON.stringify(out, null, 1) + "\n");

  const bytes = Buffer.byteLength(JSON.stringify(out));
  console.log(
    `status: ${Object.keys(grammars).length} grammars, ` +
      `${out.warnings.length} warning(s), ${out.errors.length} error(s), ` +
      `${(bytes / 1024).toFixed(0)} KiB`,
  );
  for (const [name, g] of Object.entries(grammars)) {
    const c = g.corpora[0];
    console.log(
      `  ${name.padEnd(11)} ${(c?.pass_rate ?? "—").padStart(7)}  ` +
        `gaps ${String(c?.grammar_gaps ?? "—").padStart(6)}  ` +
        `evidence ${g.evidence_freshness}  ` +
        `lint ${g.lint_ratchet ? "ratchet" : "advisory"}`,
    );
  }
}

await main();
