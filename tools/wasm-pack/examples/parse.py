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
    def roles(self):
        """The facet manifest, straight out of the module.

        Table-tier roles (_declaration, _loop, _invocation, ...) are real
        supertypes: query them directly, the parser knows them. Facets
        (_callable, _binding, _scope, _clause) cross-cut derivations and
        cannot be supertypes, so they are expanded against this manifest
        before the query runs. Without it a consumer cannot write
        (_callable) at all -- which is why it ships INSIDE the pack.
        """
        ptr = self._call("tb_roles")
        n = self._call("tb_roles_len")
        return json.loads(self.mem.read(self.store, ptr, ptr + n))

    def expand_facets(self, query):
        """Rewrite facet patterns into the concrete alternation they mean.

        `(_callable)` -> `[(function_definition) (lambda)]`. Mirrors
        treebank_core::expand; string literals and ; comments are left
        alone so a facet name inside them is never rewritten.
        """
        facets = self.roles.get("facets", {})
        out, i = [], 0
        while i < len(query):
            ch = query[i]
            if ch == '"':
                j = i + 1
                while j < len(query) and query[j] != '"':
                    j += 2 if query[j] == "\\" else 1
                out.append(query[i : j + 1])
                i = j + 1
            elif ch == ";":
                j = query.find("\n", i)
                j = len(query) if j < 0 else j
                out.append(query[i:j])
                i = j
            elif ch == "(":
                j = i + 1
                while j < len(query) and (query[j].isalnum() or query[j] == "_"):
                    j += 1
                name = query[i + 1 : j]
                members = facets.get(name)
                if members:
                    depth, k = 0, i
                    while k < len(query):
                        if query[k] == "(":
                            depth += 1
                        elif query[k] == ")":
                            depth -= 1
                            if depth == 0:
                                break
                        k += 1
                    body = self.expand_facets(query[j:k])
                    out.append("[" + " ".join(f"({m}{body})" for m in members) + "]")
                    i = k + 1
                else:
                    out.append("(")
                    i += 1
            else:
                out.append(ch)
                i += 1
        return "".join(out)

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
    print(f"treebank-{p['language']}  language={p['grammar_name']}  pack_abi={p['pack_abi']}")
    print(f"  {p['versions']}")
    print(f"  vocabulary {p['vocabulary']}  cli {p['generate_cli']}  runtime {p['runtime']}")
    # The grammar is treebank's own, so provenance is a SOURCE HASH rather
    # than an upstream sha and a patch series: there is no upstream to name.
    print(f"  grammar.js {p['sources']['grammar.js'][:12]}")
    # Sweep shapes differ by language: each oracle reports what it can
    # honestly measure, so treat this as opaque and print what is there.
    for name, sw in (p.get("sweeps") or {}).items():
        print(f"  {name}: {sw.get('pass_rate', '?')} of {sw['files']} files, {sw['gap_files']} gap files")
    facets = " ".join(f"{k}({len(v)})" for k, v in pack.roles.get("facets", {}).items())
    print(f"  facets: {facets}")
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
