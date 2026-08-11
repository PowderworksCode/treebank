#!/usr/bin/env python3
"""Parse a file with a treebank wasm pack. No Rust, no C, no tree-sitter install.

    pip install wasmtime
    python parse.py dist/wasm/treebank-python.wasm somefile.py

Everything below is the whole binding. There is no native code and no
emscripten glue: a pack imports only WASI, so this is instantiate-and-call.
"""
import json
import sys

from wasmtime import Engine, FuncType, Linker, Module, Store, ValType, WasiConfig

NAMED, IS_ERROR, HAS_ERROR, MISSING, EXTRA = 1, 2, 4, 8, 16


class Pack:
    def __init__(self, path):
        engine = Engine()
        self.store = Store(engine)
        self.store.set_wasi(WasiConfig())
        linker = Linker(engine)
        linker.define_wasi()
        inst = linker.instantiate(self.store, Module.from_file(engine, path))
        self.e = inst.exports(self.store)
        self.mem = self.e["memory"]
        self.e["_initialize"](self.store)

    def _call(self, name, *args):
        return self.e[name](self.store, *args)

    def _cstr(self, ptr):
        if not ptr:
            return None
        n = self._call("tb_strlen", ptr)
        return self.mem.read(self.store, ptr, ptr + n).decode("utf-8", "replace")

    @property
    def provenance(self):
        ptr = self._call("tb_provenance")
        n = self._call("tb_provenance_len")
        return json.loads(self.mem.read(self.store, ptr, ptr + n))

    @property
    def language(self):
        return self._cstr(self._call("tb_language_name"))

    def parse(self, src: bytes):
        ptr = self._call("tb_alloc", len(src))
        self.mem.write(self.store, src, ptr)
        tree = self._call("tb_parse", ptr, len(src))
        self._call("tb_free", ptr)
        if not tree:
            raise RuntimeError("parse failed")
        return tree

    # --- node access: a node is a caller-owned slot of module memory --------
    def root(self, tree):
        n = self._call("tb_node_new")
        self._call("tb_tree_root", tree, n)
        return n

    def flags(self, n):
        return self._call("tb_node_flags", n)

    def type(self, n):
        return self._cstr(self._call("tb_node_type", n))

    def start(self, n):
        return self._call("tb_node_start_row", n), self._call("tb_node_start_column", n)

    def child_count(self, n):
        return self._call("tb_node_child_count", n)

    def child(self, n, i, out):
        return self._call("tb_node_child", n, i, out)

    def sexp(self, n):
        p = self._call("tb_node_sexp", n)
        s = self._cstr(p)
        self._call("tb_cstr_free", p)
        return s

    def errors(self, node):
        """Every ERROR / MISSING node, as (line, col, type). Descends only into
        subtrees that HAS_ERROR marks, which is why this is cheap."""
        out = []
        if not self.flags(node) & HAS_ERROR:
            return out
        stack = [node]
        while stack:
            n = stack.pop()
            f = self.flags(n)
            if f & (IS_ERROR | MISSING):
                row, col = self.start(n)
                out.append((row + 1, col, "MISSING" if f & MISSING else "ERROR", self.type(n)))
            for i in range(self.child_count(n)):
                kid = self._call("tb_node_new")
                self.child(n, i, kid)
                if self.flags(kid) & (HAS_ERROR | IS_ERROR | MISSING):
                    stack.append(kid)
                else:
                    self._call("tb_node_free", kid)
        return out


def main():
    pack_path, *files = sys.argv[1:]
    pack = Pack(pack_path)
    p = pack.provenance
    up = p["upstream"]
    fixes = sum(1 for x in p["patches"] if x["kind"] == "grammar")
    print(f"{p['pack']}  language={pack.language}  pack_abi={p['pack_abi']}")
    print(f"  upstream {up['git_url'].split('/')[-1]} {up['version']} @ {up['sha'][:12]}")
    # Sweep shapes differ by grammar: each language's oracle reports what it can
    # honestly measure, so treat this as opaque and print what is there.
    sweep = p.get("sweep") or {}
    before, after = (sweep.get("upstream") or {}), (sweep.get("patched") or {})
    if "gap_files" in before and "gap_files" in after:
        detail = f"sweep {before['gap_files']} -> {after['gap_files']} gap files"
    elif "gap_files" in after:
        detail = f"sweep {after['gap_files']} gap files remaining"
    else:
        detail = "sweep numbers in ledger.json"
    print(f"  {fixes} parser-fix patches; {detail}")
    for f in files:
        src = open(f, "rb").read()
        tree = pack.parse(src)
        root = pack.root(tree)
        errs = pack.errors(root)
        status = "clean" if not errs else f"{len(errs)} error(s)"
        print(f"\n  {f}: {status}")
        for line, col, kind, typ in errs[:5]:
            print(f"    {line}:{col}  {kind} at ({typ})")
        if not errs:
            print(f"    {pack.sexp(root)[:70]}...")
        pack._call("tb_node_free", root)
        pack._call("tb_tree_free", tree)


if __name__ == "__main__":
    main()
