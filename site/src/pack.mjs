// A treebank wasm pack, in the browser.
//
// A pack is one standalone module: the tree-sitter runtime, the grammar and a
// small ABI, statically linked. It imports ONLY WASI -- and only six calls of
// it, all file-descriptor stubs that the parse path never reaches -- so the
// whole host is the object below. There is no web-tree-sitter here, no
// emscripten glue, and no dependency: `tree-sitter build --wasm` emits an
// emscripten side module that only web-tree-sitter can load, and packs exist
// precisely so a consumer does not have to be web-tree-sitter.
//
// Ported from tools/wasm-pack/examples/parse.mjs, which does the same job in
// Node against `node:wasi`. The only difference here is that the six imports
// are written out rather than borrowed from a runtime the browser lacks.

import { parseNodeTypes } from "./expand.mjs";

const ENOSYS = 52;

// The pack's whole import surface. Five of these can never be reached: a pack
// opens no files. fd_write is stubbed rather than omitted because an abort
// path would otherwise trap instead of printing, and a trap is much harder to
// read than a message.
function wasiStubs(instance) {
  const bytesWritten = (iovs, count, out) => {
    const view = new DataView(instance().exports.memory.buffer);
    let total = 0;
    for (let i = 0; i < count; i++) total += view.getUint32(iovs + i * 8 + 4, true);
    view.setUint32(out, total, true);
    return 0;
  };
  return {
    fd_close: () => ENOSYS,
    fd_fdstat_get: () => ENOSYS,
    fd_fdstat_set_flags: () => ENOSYS,
    fd_read: () => ENOSYS,
    fd_seek: () => ENOSYS,
    fd_write: (_fd, iovs, count, out) => bytesWritten(iovs, count, out),
  };
}

export const NAMED = 1, IS_ERROR = 2, HAS_ERROR = 4, MISSING = 8;

export class Pack {
  static async load(url, { signal } = {}) {
    const response = await fetch(url, { signal });
    if (!response.ok) throw new Error(`${response.status} fetching ${url}`);
    let instance;
    const imports = { wasi_snapshot_preview1: wasiStubs(() => instance) };
    // instantiateStreaming needs application/wasm; fall back rather than fail,
    // because a dev server serving octet-stream is not a broken pack.
    const type = response.headers.get("content-type") ?? "";
    const source = type.includes("application/wasm")
      ? await WebAssembly.instantiateStreaming(response, imports)
      : await WebAssembly.instantiate(await response.arrayBuffer(), imports);
    instance = source.instance;
    return new Pack(instance);
  }

  constructor(instance) {
    this.e = instance.exports;
    this.e._initialize();
    this.decoder = new TextDecoder();
  }

  // Linear memory can grow during a parse, which detaches any view held
  // across it. Every read takes a fresh one.
  get mem() {
    return new Uint8Array(this.e.memory.buffer);
  }

  cstr(ptr) {
    if (!ptr) return null;
    return this.decoder.decode(this.mem.subarray(ptr, ptr + this.e.tb_strlen(ptr)));
  }

  json(ptr, len) {
    return JSON.parse(this.decoder.decode(this.mem.subarray(ptr, ptr + len)));
  }

  // Provenance travels INSIDE the module rather than beside it: which
  // grammar, which vocabulary, which CLI, and what the sweep measured. A
  // pack found on disk two years from now still answers for itself.
  get provenance() {
    return this.json(this.e.tb_provenance(), this.e.tb_provenance_len());
  }

  get roles() {
    return this.json(this.e.tb_roles(), this.e.tb_roles_len());
  }

  get language() {
    return this.cstr(this.e.tb_language_name());
  }

  // node-types.json, which ships inside the module beside roles. Parsed on
  // first use and kept: it is 40-70 KB, and only a facet query with a field
  // constraint ever reads it, so parsing it on load would cost more than the
  // load. `null` on a pack built before the export existed -- expansion then
  // happens unfiltered, which is what the crate does in the same case.
  get nodeTypes() {
    if (this._nodeTypes === undefined) {
      try {
        this._nodeTypes = parseNodeTypes(
          this.json(this.e.tb_node_types(), this.e.tb_node_types_len()),
        );
      } catch {
        this._nodeTypes = null;
      }
    }
    return this._nodeTypes;
  }

  // Whether this pack can run queries at all. Adding exports does not break a
  // binding, so an older pack is missing them rather than broken -- asking is
  // the difference between a page that explains itself and one that throws.
  get canQuery() {
    return typeof this.e.tb_query_new === "function";
  }

  parse(text) {
    const bytes = new TextEncoder().encode(text);
    const ptr = this.e.tb_alloc(bytes.length);
    if (!ptr) throw new Error("out of memory in the pack");
    this.mem.set(bytes, ptr);
    const tree = this.e.tb_parse(ptr, bytes.length);
    this.e.tb_free(ptr);
    if (!tree) throw new Error("parse failed");
    return new Tree(this, tree, bytes.length);
  }
}

// tree-sitter's TSQueryError, by position in the C enum, worded as
// crates/treebank/src/pack.rs words them so the page and the crate do not
// describe the same failure differently.
const QUERY_ERROR = [
  "the query is valid",
  "the query is not valid s-expression syntax",
  "the query names a node type this grammar does not have",
  "the query names a field this grammar does not have",
  "the query captures something that cannot be captured",
  "the query asks for a shape this grammar cannot produce, usually a field on a node type that does not have it",
  "the query names a language that is not this one",
];

// A compiled query. Compiling is the expensive half and is worth keeping
// across runs; the cursor is per-run and is not.
//
// Queries arrived with pack_abi 3. An older pack parses perfectly well and
// simply cannot do this, which is a fact about the pack rather than a fault
// in the page -- so `Pack.canQuery` is asked first and the box says so.
export class Query {
  constructor(pack, source) {
    const e = pack.e;
    if (!pack.canQuery) {
      throw new Error(
        `this ${pack.language} pack cannot run queries: it is pack_abi ` +
          `${pack.provenance.pack_abi}, and queries need 3. Fetch a current pack.`,
      );
    }
    const bytes = new TextEncoder().encode(source);
    const src = e.tb_alloc(bytes.length || 1);
    if (!src) throw new Error("out of memory in the pack");
    pack.mem.set(bytes, src);
    // Two u32 out-params: the byte offset the compiler stopped at, and which
    // kind of error it is. The offset is the whole message for a person who
    // just typed the query.
    const err = e.tb_alloc(8);
    const handle = e.tb_query_new(src, bytes.length, err, err + 4);
    let offset = 0, kind = 0;
    if (!handle) {
      const view = new DataView(e.memory.buffer);
      offset = view.getUint32(err, true);
      kind = view.getUint32(err + 4, true);
    }
    e.tb_free(src);
    e.tb_free(err);
    if (!handle) {
      const what = QUERY_ERROR[kind] ?? "the query is not valid";
      const error = new Error(`${what}, at byte ${offset}`);
      error.offset = offset;
      error.kind = what;
      throw error;
    }
    this.pack = pack;
    this.handle = handle;
  }

  get patternCount() {
    return this.pack.e.tb_query_pattern_count(this.handle);
  }

  // Captures come out one at a time because wasm cannot return a struct --
  // the same reason nodes are handles. `limit` stops a pattern that matches
  // everything from building a list nothing will read.
  run(node, { limit = Infinity } = {}) {
    const e = this.pack.e;
    const cursor = e.tb_query_exec(this.handle, node);
    if (!cursor) throw new Error("could not start the query");
    const slot = e.tb_node_new();
    const out = e.tb_alloc(8);
    const found = [];
    let truncated = false;
    try {
      while (e.tb_query_next_capture(cursor, this.handle, slot, out, out + 4)) {
        if (found.length >= limit) {
          truncated = true;
          break;
        }
        const view = new DataView(e.memory.buffer);
        found.push({
          name: this.pack.cstr(e.tb_query_capture_name(this.handle, view.getUint32(out + 4, true))),
          pattern: view.getUint32(out, true),
          type: this.pack.cstr(e.tb_node_type(slot)),
          startByte: e.tb_node_start_byte(slot),
          endByte: e.tb_node_end_byte(slot),
          startRow: e.tb_node_start_row(slot),
          startColumn: e.tb_node_start_column(slot),
        });
      }
    } finally {
      e.tb_node_free(slot);
      e.tb_free(out);
      e.tb_query_cursor_delete(cursor);
    }
    return { captures: found, truncated };
  }

  free() {
    this.pack.e.tb_query_delete(this.handle);
    this.handle = 0;
  }
}

export class Tree {
  constructor(pack, handle, byteLength) {
    this.pack = pack;
    this.handle = handle;
    this.byteLength = byteLength;
  }

  root() {
    const n = this.pack.e.tb_node_new();
    this.pack.e.tb_tree_root(this.handle, n);
    return n;
  }

  free() {
    this.pack.e.tb_tree_free(this.handle);
    this.handle = 0;
  }
}

// Walking is done here rather than in the UI so the two callers -- the tree
// view and the error list -- cannot disagree about what a node is. Nodes are
// handles that must be freed, and forgetting one leaks the pack's heap for as
// long as the page is open.
export function walk(pack, node, visit, { namedOnly = true, budget = Infinity } = {}) {
  const e = pack.e;
  let seen = 0;

  const describe = (n, field, depth) => ({
    type: pack.cstr(e.tb_node_type(n)),
    field,
    depth,
    flags: e.tb_node_flags(n),
    startByte: e.tb_node_start_byte(n),
    endByte: e.tb_node_end_byte(n),
    startRow: e.tb_node_start_row(n),
    startColumn: e.tb_node_start_column(n),
    childCount: namedOnly ? e.tb_node_named_child_count(n) : e.tb_node_child_count(n),
  });

  const recurse = (n, field, depth) => {
    if (seen >= budget) return;
    seen++;
    visit(describe(n, field, depth));
    const count = namedOnly ? e.tb_node_named_child_count(n) : e.tb_node_child_count(n);
    for (let i = 0; i < count && seen < budget; i++) {
      const kid = e.tb_node_new();
      if (namedOnly) e.tb_node_named_child(n, i, kid);
      else e.tb_node_child(n, i, kid);
      // Field names are the edge labels a query uses, and they belong to the
      // PARENT's view of the child -- which is why they are read here rather
      // than from the child itself.
      const name = namedOnly ? null : pack.cstr(e.tb_node_field_name_for_child(n, i));
      recurse(kid, name, depth + 1);
      e.tb_node_free(kid);
    }
  };

  recurse(node, null, 0);
  return seen;
}

// Errors are walked over ALL children, not just named ones: a MISSING node is
// often anonymous, and skipping anonymous children hides exactly the thing
// this is looking for.
export function errorsIn(pack, node) {
  const e = pack.e;
  const found = [];
  if (!(e.tb_node_flags(node) & (HAS_ERROR | IS_ERROR | MISSING))) return found;

  const recurse = (n) => {
    const flags = e.tb_node_flags(n);
    if (flags & (IS_ERROR | MISSING)) {
      found.push({
        kind: flags & MISSING ? "MISSING" : "ERROR",
        type: pack.cstr(e.tb_node_type(n)),
        row: e.tb_node_start_row(n),
        column: e.tb_node_start_column(n),
        startByte: e.tb_node_start_byte(n),
        endByte: e.tb_node_end_byte(n),
      });
    }
    const count = e.tb_node_child_count(n);
    for (let i = 0; i < count; i++) {
      const kid = e.tb_node_new();
      e.tb_node_child(n, i, kid);
      if (e.tb_node_flags(kid) & (HAS_ERROR | IS_ERROR | MISSING)) recurse(kid);
      e.tb_node_free(kid);
    }
  };

  recurse(node);
  return found;
}
