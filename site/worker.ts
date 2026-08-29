// Two jobs, both of them about serving something the static build cannot.
//
// 1. The markdown twin of a docs page, when a client explicitly asks for
//    text/markdown. powderworks-docs writes each page's source as index.md
//    beside its rendered index.html, so negotiation is a path rewrite. Only
//    requests naming text/markdown negotiate; browsers never do, so they keep
//    the HTML untouched.
//
// 2. The wasm packs, out of R2. They are build artifacts -- 14 MB across nine
//    grammars, rebuilt whenever a grammar changes -- so they are neither
//    committed nor part of the site build. R2 is what makes this the boring
//    option: egress is free, the packs are two thousandths of the free
//    storage tier, and serving them through the Worker keeps them SAME-ORIGIN.
//    That last part is not a nicety. GitHub release assets send no
//    access-control-allow-origin on either the github.com redirect or the
//    final object, so a browser on this domain cannot fetch them at all.

interface Env {
  ASSETS: { fetch(request: RequestInfo | URL): Promise<Response> };
  // Bound in wrangler.toml. Optional so `wrangler dev` works against packs
  // staged in public/ without an R2 binding present.
  PACKS?: R2Bucket;
}

const MARKDOWN_TYPE = "text/markdown; charset=utf-8";
const PACK_PREFIX = "/packs/";

// A pack is immutable for the bytes it has -- the build asserts byte
// reproducibility -- but the name is not versioned, so a grammar change
// replaces the object behind the same URL. ETag revalidation is therefore the
// honest cache: hold it briefly, then ask, and let R2 answer 304 almost
// always. Long-lived immutable caching would need the URL to carry the hash.
const PACK_CACHE = "public, max-age=300, stale-while-revalidate=86400";

async function servePack(request: Request, env: Env, key: string): Promise<Response | null> {
  if (!env.PACKS) return null;

  // Let the client's cached copy settle without shipping a megabyte.
  const etagIn = request.headers.get("If-None-Match") ?? undefined;
  const object = await env.PACKS.get(key, {
    onlyIf: etagIn ? { etagDoesNotMatch: etagIn.replace(/^W\//, "").replace(/"/g, "") } : undefined,
  });
  if (!object) return null;

  const headers = new Headers();
  object.writeHttpMetadata(headers);
  headers.set("Content-Type", "application/wasm");
  headers.set("Cache-Control", PACK_CACHE);
  headers.set("ETag", object.httpEtag);

  // `get` with onlyIf returns a body-less object when the condition fails,
  // which is R2's way of saying "unchanged".
  if (!("body" in object) || object.body === null) {
    return new Response(null, { status: 304, headers });
  }
  if (request.method === "HEAD") return new Response(null, { headers });
  return new Response(object.body, { headers });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    // Packs first: R2 where it is bound, otherwise fall through to whatever
    // the static build has, so a local checkout with packs staged in public/
    // behaves the same as production.
    if (url.pathname.startsWith(PACK_PREFIX) && url.pathname.endsWith(".wasm")) {
      const key = url.pathname.slice(PACK_PREFIX.length);
      // No traversal, no nesting: a pack key is one flat filename.
      if (/^[a-z0-9][a-z0-9._-]*\.wasm$/.test(key)) {
        const served = await servePack(request, env, key);
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
