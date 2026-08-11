// Parse a file with a treebank wasm pack. No npm install, no web-tree-sitter.
//
//   node parse.mjs dist/wasm/treebank-python.wasm somefile.py
//
// A pack imports only WASI, which Node ships in `node:wasi`, so this is the
// whole binding. web-tree-sitter is not involved and not needed.
import { readFileSync } from 'node:fs';
import { WASI } from 'node:wasi';

const NAMED = 1, IS_ERROR = 2, HAS_ERROR = 4, MISSING = 8;

class Pack {
  constructor(path) {
    const wasi = new WASI({ version: 'preview1', args: [], env: {} });
    const mod = new WebAssembly.Module(readFileSync(path));
    const inst = new WebAssembly.Instance(mod, wasi.getImportObject());
    this.e = inst.exports;
    this.e._initialize();
    this.mem = () => new Uint8Array(this.e.memory.buffer);   // re-read: memory can grow
  }
  cstr(ptr) {
    if (!ptr) return null;
    const n = this.e.tb_strlen(ptr);
    return new TextDecoder().decode(this.mem().subarray(ptr, ptr + n));
  }
  get provenance() {
    const ptr = this.e.tb_provenance(), n = this.e.tb_provenance_len();
    return JSON.parse(new TextDecoder().decode(this.mem().subarray(ptr, ptr + n)));
  }
  get language() { return this.cstr(this.e.tb_language_name()); }
  parse(bytes) {
    const ptr = this.e.tb_alloc(bytes.length);
    this.mem().set(bytes, ptr);
    const tree = this.e.tb_parse(ptr, bytes.length);
    this.e.tb_free(ptr);
    if (!tree) throw new Error('parse failed');
    return tree;
  }
  root(tree) { const n = this.e.tb_node_new(); this.e.tb_tree_root(tree, n); return n; }
  sexp(n) { const p = this.e.tb_node_sexp(n); const s = this.cstr(p); this.e.tb_cstr_free(p); return s; }
  errors(node) {
    const out = [];
    if (!(this.e.tb_node_flags(node) & HAS_ERROR)) return out;
    const stack = [node];
    while (stack.length) {
      const n = stack.pop();
      const f = this.e.tb_node_flags(n);
      if (f & (IS_ERROR | MISSING)) {
        out.push({
          line: this.e.tb_node_start_row(n) + 1,
          col: this.e.tb_node_start_column(n),
          kind: (f & MISSING) ? 'MISSING' : 'ERROR',
          type: this.cstr(this.e.tb_node_type(n)),
        });
      }
      for (let i = 0; i < this.e.tb_node_child_count(n); i++) {
        const kid = this.e.tb_node_new();
        this.e.tb_node_child(n, i, kid);
        if (this.e.tb_node_flags(kid) & (HAS_ERROR | IS_ERROR | MISSING)) stack.push(kid);
        else this.e.tb_node_free(kid);
      }
    }
    return out;
  }
}

const [packPath, ...files] = process.argv.slice(2);
const pack = new Pack(packPath);
const p = pack.provenance, up = p.upstream;
const fixes = p.patches.filter(x => x.kind === 'grammar').length;
console.log(`${p.pack}  language=${pack.language}  pack_abi=${p.pack_abi}`);
console.log(`  upstream ${up.git_url.split('/').pop()} ${up.version} @ ${up.sha.slice(0, 12)}`);
// Sweep shapes differ by grammar: each language's oracle reports what it can
// honestly measure, so treat this as opaque and print what is there.
const before = p.sweep?.upstream ?? {}, after = p.sweep?.patched ?? {};
const detail = (before.gap_files !== undefined && after.gap_files !== undefined)
  ? `sweep ${before.gap_files} -> ${after.gap_files} gap files`
  : (after.gap_files !== undefined ? `sweep ${after.gap_files} gap files remaining`
                                   : 'sweep numbers in ledger.json');
console.log(`  ${fixes} parser-fix patches; ${detail}`);
for (const f of files) {
  const tree = pack.parse(readFileSync(f));
  const root = pack.root(tree);
  const errs = pack.errors(root);
  console.log(`\n  ${f}: ${errs.length ? `${errs.length} error(s)` : 'clean'}`);
  for (const e of errs.slice(0, 5)) console.log(`    ${e.line}:${e.col}  ${e.kind} at (${e.type})`);
  if (!errs.length) console.log(`    ${pack.sexp(root).slice(0, 70)}...`);
  pack.e.tb_node_free(root); pack.e.tb_tree_free(tree);
}
