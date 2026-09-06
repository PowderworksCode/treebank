#!/usr/bin/env python3
"""Hold a lowered bindings.json to CPython's symtable.

    python3 tools/bindings_check.py spike/pyish

For every program under `<dir>/bindings/*.py` -- valid Python and valid
pyish alike -- this parses it with the spike's generated parser (through
the tree-sitter CLI's parse output), applies `<dir>/bindings.json` to the
tree to find scopes, definitions and references, resolves every name the
way the data says (a definition binds in the enclosing or module scope; a
reference resolves outward; a scope's module-directed binding of a name
redirects that scope's other bindings of it), and compares the per-scope
classification -- parameter, local, free, global -- with what
`symtable.symtable` reports for the same source. Writes
`<dir>/bindings-results.md`.
"""

import json
import re
import subprocess
import symtable
import sys
from pathlib import Path


class Node:
    def __init__(self, kind, field, start, end):
        self.kind, self.field, self.start, self.end = kind, field, start, end
        self.children = []
        self.parent = None

    def walk(self):
        yield self
        for c in self.children:
            yield from c.walk()


TOKEN = re.compile(r"(?:(\w+): )?\((\w+)(?: \"[^\"]*\")? \[(\d+), (\d+)\] - \[(\d+), (\d+)\]|(\))")


def parse_tree(text):
    """The CLI's S-expression with positions, into a tree."""
    root = None
    stack = []
    for m in TOKEN.finditer(text):
        if m.group(7):
            stack.pop()
            continue
        n = Node(m.group(2), m.group(1), (int(m.group(3)), int(m.group(4))), (int(m.group(5)), int(m.group(6))))
        if stack:
            n.parent = stack[-1]
            stack[-1].children.append(n)
        else:
            root = n
        stack.append(n)
    return root


def text_of(lines, n):
    (r0, c0), (r1, c1) = n.start, n.end
    if r0 == r1:
        return lines[r0][c0:c1]
    return lines[r0][c0:] + "".join(lines[r0 + 1 : r1]) + lines[r1][:c1]


def classify_from_bindings(root, lines, b):
    scope_kinds = {s["node"]: s["kind"] for s in b["scopes"]}
    ref_nodes = {r["node"] for r in b["references"]}
    defs = {(d["node"], d["field"]): d for d in b["definitions"]}

    def enclosing_scope(n):
        p = n.parent
        while p is not None and p.kind not in scope_kinds:
            p = p.parent
        return p

    scope_names = {}
    for n in root.walk():
        if n.kind in scope_kinds:
            if n.parent is None:
                scope_names[n] = "top"
            else:
                name = next((text_of(lines, c) for c in n.children if c.field == "name"), None)
                scope_names[n] = name or n.kind
    bound = {s: {} for s in scope_names}      # scope -> name -> kind
    declared = {s: {} for s in scope_names}   # scope -> name -> target scope
    refs = {s: set() for s in scope_names}
    claimed = set()
    # Module-directed bindings first: they redirect the rest.
    passes = [lambda d: d["scope"] == "module", lambda d: d["scope"] != "module"]
    for want in passes:
        for n in root.walk():
            for c in n.children:
                d = defs.get((n.kind, c.field))
                if d is None or c.kind != d["name"] or not want(d):
                    continue
                name = text_of(lines, c)
                claimed.add(id(c))
                here = enclosing_scope(n)
                if d["scope"] == "module":
                    target = root
                    if here is not root:
                        declared[here][name] = root
                else:
                    target = declared[here].get(name, here)
                kind = d["kind"] if target is here else "var"
                if name not in bound[target] or kind == "parameter":
                    bound[target][name] = kind
    for n in root.walk():
        if n.kind in ref_nodes and id(n) not in claimed:
            refs[enclosing_scope(n)].add(text_of(lines, n))

    out = {}
    for s, sname in scope_names.items():
        table = {}
        for name in set(bound[s]) | set(refs[s]) | set(declared[s]):
            if name in declared[s]:
                table[name] = "global"
            elif name in bound[s]:
                table[name] = "parameter" if bound[s][name] == "parameter" else "local"
            else:
                p = enclosing_scope(s)
                found = None
                while p is not None:
                    if name in bound[p] and p is not root:
                        found = p
                        break
                    p = enclosing_scope(p)
                table[name] = "free" if found is not None else "global"
        out[sname] = table
    return out


def classify_from_symtable(src):
    out = {}

    def visit(t):
        table = {}
        for sym in t.get_symbols():
            # A `global x` in a function is recorded on the module table too;
            # there, an assigned name is a module local whatever else it is.
            if sym.is_parameter():
                k = "parameter"
            elif t.get_type() == "module" and sym.is_local():
                k = "local"
            elif sym.is_declared_global():
                k = "global"
            elif sym.is_free():
                k = "free"
            elif sym.is_local():
                k = "local"
            elif sym.is_global():
                k = "global"
            else:
                k = "?"
            table[sym.get_name()] = k
        out[t.get_name()] = table
        for c in t.get_children():
            visit(c)

    visit(symtable.symtable(src, "<pyish>", "exec"))
    return out


def main(spike: Path) -> int:
    b = json.loads((spike / "bindings.json").read_text())
    results = []
    passed = 0
    programs = sorted((spike / "bindings").glob("*.py"))
    for prog in programs:
        src = prog.read_text()
        lines = src.split("\n")
        r = subprocess.run(["tree-sitter", "parse", str(prog)], cwd=spike, capture_output=True, text=True)
        tree = parse_tree(r.stdout)
        if tree is None or "ERROR" in r.stdout or "MISSING" in r.stdout:
            results.append((prog.name, False, "the generated parser rejected the program:\n" + r.stdout, None))
            continue
        ours = classify_from_bindings(tree, lines, b)
        oracle = classify_from_symtable(src)
        ok = ours == oracle
        passed += ok
        results.append((prog.name, ok, ours, oracle))

    out = [f"# Bindings results for {spike.name}", "", f"{passed} of {len(programs)} programs classify every name in every scope as CPython's symtable does.", ""]
    for name, ok, ours, oracle in results:
        out.append(f"## {'PASS' if ok else 'FAIL'}: {name}")
        out.append("")
        out.append("```python")
        out.append((spike / "bindings" / name).read_text().rstrip("\n"))
        out.append("```")
        out.append("")
        if oracle is None:
            out.append("```")
            out.append(str(ours))
            out.append("```")
            out.append("")
            continue
        out.append("| scope | name | bindings.json | symtable |")
        out.append("|---|---|---|---|")
        for scope in sorted(set(ours) | set(oracle), key=lambda s: (s != "top", s)):
            names = sorted(set(ours.get(scope, {})) | set(oracle.get(scope, {})))
            for n in names:
                a = ours.get(scope, {}).get(n, "-")
                o = oracle.get(scope, {}).get(n, "-")
                mark = "" if a == o else " **differs**"
                out.append(f"| {scope} | `{n}` | {a} | {o}{mark} |")
        out.append("")
    (spike / "bindings-results.md").write_text("\n".join(out) + "\n")
    print(f"{spike.name}: {passed}/{len(programs)} -> {spike / 'bindings-results.md'}")
    return 0 if passed == len(programs) else 1


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]).resolve()))
