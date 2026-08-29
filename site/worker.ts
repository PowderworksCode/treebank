// Two jobs, both about serving something the static build cannot.
//
// 1. The markdown twin of a docs page, when a client explicitly asks for
//    text/markdown. powderworks-docs writes each page's source as index.md
//    beside its rendered index.html, so negotiation is a path rewrite. Only
//    requests naming text/markdown negotiate; browsers never do, so they keep
//    the HTML untouched.
//
// 2. The wasm packs, out of R2, CONTENT-ADDRESSED.
//
//    A pack is byte-reproducible, so its sha256 is a name it can never
//    outgrow: `treebank-python-<hash>.wasm` either is those bytes or does not
//    exist. Those objects are immutable and cached forever. `treebank-
//    python.wasm` is the moving pointer, resolved through a manifest rather
//    than duplicated as a second object, because R2 has no symlinks and two
//    copies under two names is two things that can disagree.
//
//    Serving them through the Worker keeps the packs SAME-ORIGIN, which is
//    not a nicety: GitHub release assets carry no access-control-allow-origin
//    on either the github.com redirect or the final object, so a browser on
//    this domain cannot fetch them at all. R2 makes the rest unremarkable --
//    egress is free, including via the Workers API.

interface R2ObjectLike {
  body: ReadableStream | null;
  httpEtag: string;
  writeHttpMetadata(headers: Headers): void;
}
interface R2BucketLike {
  get(
    key: string,
    options?: { onlyIf?: { etagDoesNotMatch?: string } },
  ): Promise<R2ObjectLike | null>;
}

interface Env {
  ASSETS: { fetch(request: RequestInfo | URL): Promise<Response> };
  // Bound in wrangler.toml. Optional so `wrangler dev` and a local checkout
  // with packs staged in public/ work with no bucket at all.
  PACKS?: R2BucketLike;
}

const MARKDOWN_TYPE = "text/markdown; charset=utf-8";
const PACK_PREFIX = "/packs/";
const MANIFEST_KEY = "index.json";

// A hashed key is those bytes or nothing, so it can be cached for a year.
const IMMUTABLE = "public, max-age=31536000, immutable";
// The pointer moves when a grammar does; revalidation keeps a repeat visit at
// 304 and no bytes without pinning anyone to a stale parser.
const POINTER = "public, max-age=300, stale-while-revalidate=86400";
// The manifest moves most often of the three and is the smallest.
const MANIFEST = "public, max-age=60, stale-while-revalidate=600";

const HASHED = /^treebank-[a-z0-9]+-[0-9a-f]{12}\.wasm$/;
const POINTED = /^treebank-([a-z0-9]+)\.wasm$/;

async function fromBucket(
  request: Request,
  bucket: R2BucketLike,
  key: string,
  cacheControl: string,
  contentType: string,
): Promise<Response | null> {
  const inbound = request.headers.get("If-None-Match") ?? undefined;
  const object = await bucket.get(key, {
    onlyIf: inbound
      ? { etagDoesNotMatch: inbound.replace(/^W\//, "").replace(/"/g, "") }
      : undefined,
  });
  if (!object) return null;

  const headers = new Headers();
  object.writeHttpMetadata(headers);
  headers.set("Content-Type", contentType);
  headers.set("Cache-Control", cacheControl);
  headers.set("ETag", object.httpEtag);

  // R2 answers a failed onlyIf with a body-less object: "unchanged".
  if (object.body === null) return new Response(null, { status: 304, headers });
  if (request.method === "HEAD") return new Response(null, { headers });
  return new Response(object.body, { headers });
}

async function resolvePointer(bucket: R2BucketLike, name: string): Promise<string | null> {
  const manifest = await bucket.get(MANIFEST_KEY);
  if (!manifest || manifest.body === null) return null;
  try {
    const parsed = await new Response(manifest.body).json() as {
      packs?: Record<string, { key?: string }>;
    };
    const key = parsed.packs?.[name]?.key;
    // Never let a manifest name an object outside the pack namespace.
    return key && HASHED.test(key) ? key : null;
  } catch {
    return null;
  }
}

async function servePack(request: Request, env: Env, file: string): Promise<Response | null> {
  if (!env.PACKS) return null;

  if (file === MANIFEST_KEY) {
    return fromBucket(request, env.PACKS, MANIFEST_KEY, MANIFEST, "application/json");
  }
  if (HASHED.test(file)) {
    return fromBucket(request, env.PACKS, file, IMMUTABLE, "application/wasm");
  }
  const pointed = POINTED.exec(file);
  if (pointed) {
    const key = await resolvePointer(env.PACKS, pointed[1]);
    if (!key) return null;
    return fromBucket(request, env.PACKS, key, POINTER, "application/wasm");
  }
  return null;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    // Packs first: R2 where it is bound, otherwise fall through to whatever
    // the static build carries, so a local checkout with packs staged in
    // public/ behaves exactly like production.
    if (url.pathname.startsWith(PACK_PREFIX)) {
      const file = url.pathname.slice(PACK_PREFIX.length);
      if (/^[a-z0-9][a-z0-9.-]*$/.test(file)) {
        const served = await servePack(request, env, file);
        if (served) return served;
      }
    }

    const asset = await env.ASSETS.fetch(request);
    if (!(request.headers.get("Accept") ?? "").includes("text/markdown")) {
      return asset;
    }

    const path = url.pathname !== "/" && url.pathname.endsWith("/")
      ? url.pathname.slice(0, -1)
      : url.pathname;
    const twin = await env.ASSETS.fetch(new URL(`${path}/index.md`, url.origin));
    if (twin.status === 404) return asset;

    const response = new Response(twin.body, twin);
    response.headers.set("Content-Type", MARKDOWN_TYPE);
    response.headers.set("Vary", "Accept");
    return response;
  },
};
