// Every content page must carry title and description frontmatter: the
// generator turns them into <title>, the meta description, and the entry on
// its section index. A page without them is a blank line in the index.
import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const CONTENT_DIR = join(import.meta.dir, "..", "content");

function markdownFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...markdownFiles(full));
    else if (entry.name.endsWith(".md")) out.push(full);
  }
  return out;
}

describe("content frontmatter", () => {
  for (const file of markdownFiles(CONTENT_DIR)) {
    const rel = file.slice(CONTENT_DIR.length + 1);
    test(`${rel} declares title and description`, () => {
      const text = readFileSync(file, "utf8");
      const match = /^---\n([\s\S]*?)\n---/.exec(text);
      expect(match).not.toBeNull();
      expect(match![1]).toMatch(/^title: \S/m);
      expect(match![1]).toMatch(/^description: \S/m);
    });
  }
});

describe("every grammar has a page", () => {
  // The pages are generated from crates/, so this asserts the generator ran
  // and covered what it discovered -- a grammar whose page is missing is a
  // grammar nobody can read.
  test("one page per bundle", () => {
    const data = join(import.meta.dir, "..", "public", "grammars");
    const bundles = readdirSync(data)
      .filter((f) => f.endsWith(".json") && f !== "index.json")
      .map((f) => f.replace(/\.json$/, ""))
      .sort();
    const pages = readdirSync(join(CONTENT_DIR, "grammars"))
      .filter((f) => f.endsWith(".md") && f !== "index.md")
      .map((f) => f.replace(/\.md$/, ""))
      .sort();
    expect(bundles.length).toBeGreaterThan(0);
    expect(pages).toEqual(bundles);
  });
});
