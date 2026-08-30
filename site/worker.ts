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
//
//    A PREVIEW deployment reads one prefix further in. Packs are published
//    from main, so a branch that adds a grammar has no pack for it and the
//    playground on that branch's preview URL cannot load the thing the
//    branch exists to add. CI publishes those under `previews/<sha>/`, and a
//    preview build writes the sha into `/packs/preview.json` as a static
//    asset -- fixed per deployment, so a request cannot ask to be served
//    from somewhere else. Production carries no such asset, so it never
//    constructs a `previews/` key at all.

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
const PREVIEW_MARKER = "/packs/preview.json";
// `previews/<40-hex sha>/`. Matched rather than trusted: the marker is an
// asset of this deployment and not user input, but a key built from a
// document is still a key, and this is what keeps it inside the namespace.
const PREVIEW_PREFIX = /^previews\/[0-9a-f]{7,40}\/$/;

// A hashed key is those bytes or nothing, so it can be cached for a year.
const IMMUTABLE = "public, max-age=31536000, immutable";
// The pointer moves when a grammar does; revalidation keeps a repeat visit at
// 304 and no bytes without pinning anyone to a stale parser.
const POINTER = "public, max-age=300, stale-while-revalidate=86400";
// The manifest moves most often of the three and is the smallest.
const MANIFEST = "public, max-age=60, stale-while-revalidate=600";

const HASHED = /^treebank-[a-z0-9]+-[0-9a-f]{12}\.wasm$/;
const POINTED = /^treebank-([a-z0-9]+)\.wasm$/;

// Read once per isolate: assets are immutable for the life of a deployment,
// so the answer cannot change under us. `undefined` means "not asked yet",
// `null` means "asked, and this is production".
let previewPrefix: string | null | undefined;

async function previewFor(env: Env, origin: string): Promise<string | null> {
  if (previewPrefix !== undefined) return previewPrefix;
  try {
    const marker = await env.ASSETS.fetch(new URL(PREVIEW_MARKER, origin));
    if (!marker.ok) {
      // No marker is production's answer and it cannot change under this
      // deployment, so it is worth remembering.
      previewPrefix = null;
      return previewPrefix;
    }
    const parsed = (await marker.json()) as { prefix?: unknown };
    previewPrefix =
      typeof parsed.prefix === "string" && PREVIEW_PREFIX.test(parsed.prefix)
        ? parsed.prefix
        : null;
    return previewPrefix;
  } catch {
    // A THROWN error is not an answer: a transient asset-store failure
    // cached as `null` would leave this isolate serving a preview as though
    // it were production for the rest of its life. Fall back for this
    // request and ask again on the next one.
    return null;
  }
}

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

// The preview manifest first, then main's. Two gets on a preview and one in
// production, where `prefix` is null and the first is skipped entirely.
async function manifestBody(
  bucket: R2BucketLike,
  prefix: string | null,
): Promise<R2ObjectLike | null> {
  if (prefix) {
    const preview = await bucket.get(prefix + MANIFEST_KEY);
    if (preview) return preview;
  }
  return bucket.get(MANIFEST_KEY);
}

async function resolvePointer(
  bucket: R2BucketLike,
  prefix: string | null,
  name: string,
): Promise<string | null> {
  const manifest = await manifestBody(bucket, prefix);
  if (!manifest || manifest.body === null) return null;
  try {
    const parsed = (await new Response(manifest.body).json()) as {
      packs?: Record<string, { key?: string }>;
    };
    const key = parsed.packs?.[name]?.key;
    // Never let a manifest name an object outside the pack namespace. The
    // preview manifest names BARE keys like main's; the prefix is this
    // Worker's to add, so a document can never reach across into another
    // deployment's objects or out of `previews/` altogether.
    return key && HASHED.test(key) ? key : null;
  } catch {
    return null;
  }
}

// A preview publishes only what main does not already have -- packs are
// byte-reproducible, so an unchanged grammar has main's hash and main's
// object -- which is why every read falls back rather than choosing.
async function fromPacks(
  request: Request,
  bucket: R2BucketLike,
  prefix: string | null,
  key: string,
  cacheControl: string,
  contentType: string,
): Promise<Response | null> {
  if (prefix) {
    const preview = await fromBucket(
      request,
      bucket,
      prefix + key,
      cacheControl,
      contentType,
    );
    if (preview) return preview;
  }
  return fromBucket(request, bucket, key, cacheControl, contentType);
}

async function servePack(
  request: Request,
  env: Env,
  file: string,
  prefix: string | null,
): Promise<Response | null> {
  if (!env.PACKS) return null;

  if (file === MANIFEST_KEY) {
    return fromPacks(
      request,
      env.PACKS,
      prefix,
      MANIFEST_KEY,
      MANIFEST,
      "application/json",
    );
  }
  if (HASHED.test(file)) {
    return fromPacks(
      request,
      env.PACKS,
      prefix,
      file,
      IMMUTABLE,
      "application/wasm",
    );
  }
  // The group is checked rather than asserted: a matched pattern always has
  // it, but noUncheckedIndexedAccess is right that the type does not say so.
  const pointed = POINTED.exec(file);
  if (pointed?.[1]) {
    const key = await resolvePointer(env.PACKS, prefix, pointed[1]);
    if (!key) return null;
    return fromPacks(
      request,
      env.PACKS,
      prefix,
      key,
      POINTER,
      "application/wasm",
    );
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
        const prefix = await previewFor(env, url.origin);
        const served = await servePack(request, env, file, prefix);
        if (served) return served;
      }
    }

    const asset = await env.ASSETS.fetch(request);
    if (!(request.headers.get("Accept") ?? "").includes("text/markdown")) {
      return asset;
    }

    const path =
      url.pathname !== "/" && url.pathname.endsWith("/")
        ? url.pathname.slice(0, -1)
        : url.pathname;
    const twin = await env.ASSETS.fetch(
      new URL(`${path}/index.md`, url.origin),
    );
    if (twin.status === 404) return asset;

    const response = new Response(twin.body, twin);
    response.headers.set("Content-Type", MARKDOWN_TYPE);
    response.headers.set("Vary", "Accept");
    return response;
  },
};
