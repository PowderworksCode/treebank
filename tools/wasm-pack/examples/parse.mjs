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
  // The facet manifest, straight out of the module. Table-tier roles are
  // real supertypes and queryable from the parser; facets cross-cut
  // derivations, cannot be supertypes, and are expanded against this
  // before a query runs — which is why it ships INSIDE the pack.
  get roles() {
    const p = this.e.tb_roles();
    const n = this.e.tb_roles_len();
    return JSON.parse(new TextDecoder().decode(this.mem().subarray(p, p + n)));
  }

  // `(_callable)` -> `[(function_definition) (lambda)]`. Mirrors
  // treebank_core::expand; strings and ; comments are left alone.
  expandFacets(query) {
    const facets = this.roles.facets ?? {};
    let out = '', i = 0;
    while (i < query.length) {
      const ch = query[i];
      if (ch === '"') {
        let j = i + 1;
        while (j < query.length && query[j] !== '"') j += query[j] === '\\' ? 2 : 1;
        out += query.slice(i, j + 1); i = j + 1;
      } else if (ch === ';') {
        let j = query.indexOf('\n', i); if (j < 0) j = query.length;
        out += query.slice(i, j); i = j;
      } else if (ch === '(') {
        let j = i + 1;
        while (j < query.length && /[A-Za-z0-9_]/.test(query[j])) j++;
        const members = facets[query.slice(i + 1, j)];
        if (members?.length) {
          let depth = 0, k = i;
          for (; k < query.length; k++) {
            if (query[k] === '(') depth++;
            else if (query[k] === ')' && --depth === 0) break;
          }
          const body = this.expandFacets(query.slice(j, k));
          out += '[' + members.map((m) => `(${m}${body})`).join(' ') + ']';
          i = k + 1;
        } else { out += '('; i++; }
      } else { out += ch; i++; }
    }
    return out;
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
const p = pack.provenance;
console.log(`treebank-${p.language}  language=${p.grammar_name}  pack_abi=${p.pack_abi}`);
console.log(`  ${p.versions}`);
console.log(`  vocabulary ${p.vocabulary}  cli ${p.generate_cli}  runtime ${p.runtime}`);
// The grammar is treebank's own, so provenance is a SOURCE HASH rather than
// an upstream sha and a patch series: there is no upstream to point at.
console.log(`  grammar.js ${p.sources['grammar.js'].slice(0, 12)}`);
// Sweep shapes differ by language: each oracle reports what it can honestly
// measure, so treat this as opaque and print what is there.
for (const [name, sw] of Object.entries(p.sweeps ?? {})) {
  console.log(`  ${name}: ${sw.pass_rate ?? '?'} of ${sw.files} files, ${sw.gap_files} gap files`);
}
const facets = Object.entries(pack.roles.facets ?? {})
  .map(([k, v]) => `${k}(${v.length})`).join(' ');
console.log(`  facets: ${facets}`);
for (const f of files) {
  const tree = pack.parse(readFileSync(f));
  const root = pack.root(tree);
  const errs = pack.errors(root);
  console.log(`\n  ${f}: ${errs.length ? `${errs.length} error(s)` : 'clean'}`);
  for (const e of errs.slice(0, 5)) console.log(`    ${e.line}:${e.col}  ${e.kind} at (${e.type})`);
  if (!errs.length) console.log(`    ${pack.sexp(root).slice(0, 70)}...`);
  pack.e.tb_node_free(root); pack.e.tb_tree_free(tree);
}
