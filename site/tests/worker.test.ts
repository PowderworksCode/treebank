// What the Worker may and may not reach in the bucket.
//
// The interesting half is the NEGATIVE one. A preview deployment reads
// `previews/<sha>/` so that a branch adding a grammar can load its pack
// before it merges; production must never construct such a key, whatever a
// request or a manifest says, because the objects under that prefix are
// built from branch code that nobody has reviewed yet. "Production ignores
// it" is the security property this file exists to hold down.
import { beforeEach, describe, expect, test } from "bun:test";

// The Worker caches the preview marker in module scope, deliberately: assets
// are immutable for the life of a deployment. That makes each case here a
// different deployment, so each one gets a fresh module — a query string is
// the only thing that makes an ES module specifier distinct.
let nonce = 0;
async function loadWorker() {
  nonce += 1;
  const mod = await import(`../worker.ts?case=${nonce}`);
  return mod.default as {
    fetch(request: Request, env: unknown): Promise<Response>;
  };
}

const PYTHON_KEY = "treebank-python-aaaaaaaaaaaa.wasm";
const HCL_MAIN_KEY = "treebank-hcl-111111111111.wasm";
const HCL_BRANCH_KEY = "treebank-hcl-222222222222.wasm";
const SHA = "abc1234def5678abc1234def5678abc1234def56";
const PREFIX = `previews/${SHA}/`;

function bucketOf(objects: Record<string, string>) {
  const reads: string[] = [];
  return {
    reads,
    bucket: {
      async get(key: string) {
        reads.push(key);
        const value = objects[key];
        if (value === undefined) return null;
        return {
          body: new Response(value).body,
          httpEtag: `"${key}"`,
          writeHttpMetadata() {},
        };
      },
    },
  };
}

function envOf(objects: Record<string, string>, marker: string | null) {
  const { bucket, reads } = bucketOf(objects);
  return {
    reads,
    env: {
      PACKS: bucket,
      ASSETS: {
        async fetch(input: RequestInfo | URL) {
          const url = new URL(
            typeof input === "string"
              ? input
              : input instanceof Request
                ? input.url
                : input.toString(),
          );
          if (url.pathname === "/packs/preview.json" && marker !== null) {
            return new Response(marker, { status: 200 });
          }
          return new Response("not found", { status: 404 });
        },
      },
    },
  };
}

// main published python and hcl; a branch rebuilt hcl and nothing else.
const OBJECTS = {
  "index.json": JSON.stringify({
    packs: {
      python: { sha256: "aaaa", key: PYTHON_KEY },
      hcl: { sha256: "1111", key: HCL_MAIN_KEY },
    },
  }),
  [PYTHON_KEY]: "python-bytes",
  [HCL_MAIN_KEY]: "hcl-main-bytes",
  [`${PREFIX}index.json`]: JSON.stringify({
    packs: {
      python: { sha256: "aaaa", key: PYTHON_KEY },
      hcl: { sha256: "2222", key: HCL_BRANCH_KEY },
    },
  }),
  [`${PREFIX}${HCL_BRANCH_KEY}`]: "hcl-branch-bytes",
};

const MARKER = JSON.stringify({ schema_version: 1, prefix: PREFIX });

async function get(path: string, marker: string | null) {
  const worker = await loadWorker();
  const { env, reads } = envOf(OBJECTS, marker);
  const response = await worker.fetch(
    new Request(`https://example.test${path}`),
    env,
  );
  return { response, reads };
}

describe("production", () => {
  test("serves main's manifest", async () => {
    const { response } = await get("/packs/index.json", null);
    const body = (await response.json()) as { packs: Record<string, unknown> };
    expect(Object.keys(body.packs).sort()).toEqual(["hcl", "python"]);
    expect((body.packs.hcl as { key: string }).key).toBe(HCL_MAIN_KEY);
  });

  test("resolves a pointer through main's manifest", async () => {
    const { response } = await get("/packs/treebank-hcl.wasm", null);
    expect(await response.text()).toBe("hcl-main-bytes");
  });

  test("never reads a previews/ key", async () => {
    for (const path of [
      "/packs/index.json",
      "/packs/treebank-hcl.wasm",
      `/packs/${HCL_BRANCH_KEY}`,
    ]) {
      const { reads } = await get(path, null);
      expect(reads.some((key) => key.startsWith("previews/"))).toBe(false);
    }
  });

  test("a preview object is unreachable even when its hash is asked for", async () => {
    // The bytes exist in the bucket under the prefix; the bare key does not.
    const { response } = await get(`/packs/${HCL_BRANCH_KEY}`, null);
    expect(response.status).toBe(404);
  });
});

describe("preview", () => {
  test("serves the branch's manifest", async () => {
    const { response } = await get("/packs/index.json", MARKER);
    const body = (await response.json()) as { packs: Record<string, unknown> };
    expect((body.packs.hcl as { key: string }).key).toBe(HCL_BRANCH_KEY);
  });

  test("resolves a rebuilt grammar to the branch's object", async () => {
    const { response } = await get("/packs/treebank-hcl.wasm", MARKER);
    expect(await response.text()).toBe("hcl-branch-bytes");
  });

  test("falls back to main for a grammar the branch did not touch", async () => {
    // The whole reason a preview publishes only what changed: python's bytes
    // are main's, so there is no second copy of them under the prefix.
    const { response, reads } = await get(
      "/packs/treebank-python.wasm",
      MARKER,
    );
    expect(await response.text()).toBe("python-bytes");
    expect(reads).toContain(`${PREFIX}${PYTHON_KEY}`);
    expect(reads).toContain(PYTHON_KEY);
  });

  test("a hashed request reaches the prefixed object", async () => {
    const { response } = await get(`/packs/${HCL_BRANCH_KEY}`, MARKER);
    expect(await response.text()).toBe("hcl-branch-bytes");
  });
});

describe("a marker that is not one", () => {
  beforeEach(() => {
    nonce += 1;
  });

  for (const [name, marker] of [
    ["not json", "{"],
    ["no prefix", JSON.stringify({ schema_version: 1 })],
    ["a prefix outside the namespace", JSON.stringify({ prefix: "../" })],
    [
      "a prefix that is not a sha",
      JSON.stringify({ prefix: "previews/not-a-sha/" }),
    ],
    ["an absolute key", JSON.stringify({ prefix: "/" })],
  ] as const) {
    test(`${name} is production`, async () => {
      const { response, reads } = await get("/packs/treebank-hcl.wasm", marker);
      expect(await response.text()).toBe("hcl-main-bytes");
      expect(reads.some((key) => key.startsWith("previews/"))).toBe(false);
    });
  }
});

describe("a manifest that names something it should not", () => {
  test("a key outside the pack namespace is refused", async () => {
    const worker = await loadWorker();
    const { env } = envOf(
      {
        "index.json": JSON.stringify({
          packs: { hcl: { key: "../secrets/id_rsa" } },
        }),
      },
      null,
    );
    const response = await worker.fetch(
      new Request("https://example.test/packs/treebank-hcl.wasm"),
      env,
    );
    expect(response.status).toBe(404);
  });
});
