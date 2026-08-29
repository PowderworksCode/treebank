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
