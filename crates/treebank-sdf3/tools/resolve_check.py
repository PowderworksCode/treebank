#!/usr/bin/env python3
"""Hold a lowered bindings.json to what the real toolchain prints.

    python3 tools/resolve_check.py spike/rustish --entry main

For every program under `<dir>/programs/` -- valid source for the real
language and for the spike's module alike -- this parses it with the
spike's generated parser (through the tree-sitter CLI), resolves every
name from `<dir>/bindings.json` alone (scopes, definitions with their
target scope and effect, references), evaluates the program with a small
interpreter that knows integers, arithmetic, calls, prints and nothing
about the language's scope rules, and compares what it printed (or that
it failed) with what rustc's binary, node or python3 prints for the same
file. The interpreter is deliberately dumb: every scoping decision comes
from the data, so the data is what the comparison tests.

Writes `<dir>/resolve-results.md`.
"""

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

TOKEN = re.compile(r"(?:(\w+): )?\((\w+)(?: \"[^\"]*\")? \[(\d+), (\d+)\] - \[(\d+), (\d+)\]|(\))")


class Node:
    def __init__(self, kind, field, start, end):
        self.kind, self.field, self.start, self.end = kind, field, start, end
        self.children, self.parent = [], None

    def walk(self):
        yield self
        for c in self.children:
            yield from c.walk()

    def field_(self, name):
        return next((c for c in self.children if c.field == name), None)

    def fields(self, name):
        return [c for c in self.children if c.field == name]


def parse_tree(text):
    root, stack = None, []
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


class Unresolved(Exception):
    pass


class RuntimeFail(Exception):
    pass


class Resolver:
    """Static resolution from bindings.json: reference node -> slot."""

    def __init__(self, root, lines, b):
        self.root, self.lines, self.b = root, lines, b
        self.scope_kind = {s["node"]: s["kind"] for s in b["scopes"]}
        self.ref_nodes = {r["node"] for r in b["references"]}
        self.defs = {(d["node"], d["field"]): d for d in b["definitions"]}
        self.slots = {}          # scope node -> name -> [slot]
        self.def_slot = {}       # id(name node) -> slot
        self.claimed = set()
        self.build()

    def text(self, n):
        (r0, c0), (r1, c1) = n.start, n.end
        return self.lines[r0][c0:c1] if r0 == r1 else self.lines[r0][c0:] + "".join(self.lines[r0 + 1:r1]) + self.lines[r1][:c1]

    def enclosing(self, n):
        p = n.parent
        while p is not None and p.kind not in self.scope_kind:
            p = p.parent
        return p

    def target_scope(self, binder, d):
        here = self.enclosing(binder)
        if d["scope"] == "enclosing":
            return here
        p = here
        while p is not None and self.scope_kind[p.kind] != d["scope"]:
            p = self.enclosing(p)
        return p or self.root

    def build(self):
        declared = {}  # (scope, name) -> target scope, from kind-directed bindings
        for want in (lambda d: d["scope"] != "enclosing", lambda d: d["scope"] == "enclosing"):
            for n in self.root.walk():
                for c in n.children:
                    d = self.defs.get((n.kind, c.field))
                    if d is None or c.kind != d["name"] or not want(d):
                        continue
                    name = self.text(c)
                    self.claimed.add(id(c))
                    here = self.enclosing(n)
                    target = self.target_scope(n, d)
                    if d["scope"] != "enclosing" and target is not here:
                        declared[(here, name)] = target
                    elif (here, name) in declared:
                        target = declared[(here, name)]
                    slots = self.slots.setdefault(target, {}).setdefault(name, [])
                    if d["effect"] == "after":
                        slot = {"scope": target, "name": name, "start": n.end, "effect": "after", "kind": d["kind"], "node": n, "ordinal": len(slots)}
                        slots.append(slot)
                    else:
                        whole = next((s for s in slots if s["effect"] == "whole"), None)
                        if whole is None:
                            whole = {"scope": target, "name": name, "start": target.start, "effect": "whole", "kind": d["kind"], "node": n, "ordinal": len(slots), "decls": []}
                            slots.append(whole)
                        whole["decls"].append(n)
                        if d["kind"] == "parameter":
                            whole["kind"] = "parameter"
                        slot = whole
                    self.def_slot[id(c)] = slot

    def resolve(self, ref):
        """The slot a reference resolves to: the latest-starting slot of its
        name at or before it, in the nearest scope that has one."""
        name = self.text(ref)
        s = self.enclosing(ref)
        while s is not None:
            cands = [x for x in self.slots.get(s, {}).get(name, []) if x["start"] <= ref.start]
            if cands:
                return max(cands, key=lambda x: (x["start"], x["ordinal"]))
            s = self.enclosing(s)
        raise Unresolved(name)


class Frame:
    def __init__(self, scope, parent):
        self.scope, self.parent, self.values = scope, parent, {}


class Function:
    def __init__(self, node, frame):
        self.node, self.frame = node, frame


class Return(Exception):
    def __init__(self, value):
        self.value = value


UNDEF = object()


class Evaluator:
    """Integers, arithmetic, calls, prints. Every scope decision is the
    resolver's; the only language-shaped facts here are what `var` and
    `let` do at run time before their line."""

    def __init__(self, res):
        self.r = res
        self.out = []

    def frame_for(self, frame, scope):
        f = frame
        while f is not None and f.scope is not scope:
            f = f.parent
        if f is None:
            raise RuntimeFail(f"no frame for scope {scope.kind}")
        return f

    def run(self, entry=None):
        root = self.r.root
        frame = Frame(root, None)
        self.hoist(root, frame)
        self.stmts(root.children, frame)
        if entry is not None:
            # A language whose program is items, entered at a named one.
            slots = self.r.slots.get(root, {}).get(entry, [])
            if not slots:
                raise RuntimeFail(f"no {entry}")
            fn = frame.values[(entry, slots[0]["ordinal"])]
            f = Frame(fn.node, fn.frame)
            self.hoist(fn.node, f)
            try:
                self.block(fn.node.field_("body"), f)
            except Return:
                pass
        return self.out

    def hoist(self, scope, frame):
        """Whole-scope function bindings are callable before their line;
        whole-scope `var` slots exist as undefined."""
        for name, slots in self.r.slots.get(scope, {}).items():
            for s in slots:
                if s["effect"] != "whole":
                    continue
                key = (name, s["ordinal"])
                if s["kind"] == "function":
                    frame.values[key] = Function(s["node"], frame)
                elif s["kind"] == "var" and any(d.kind == "var" for d in s.get("decls", [])):
                    frame.values[key] = UNDEF

    def stmts(self, nodes, frame):
        for n in nodes:
            self.stmt(n, frame)

    def bind_here(self, name_node, value, frame):
        slot = self.r.def_slot[id(name_node)]
        f = self.frame_for(frame, slot["scope"])
        f.values[(slot["name"], slot["ordinal"])] = value

    def stmt(self, n, frame):
        k = n.kind
        if k in ("let", "var", "assign"):
            target = n.field_("pattern") or n.field_("name") or n.field_("target")
            value = self.exp(n.field_("value"), frame)
            if id(target) in self.r.def_slot:
                self.bind_here(target, value, frame)
            else:
                slot = self.r.resolve(target)
                self.frame_for(frame, slot["scope"]).values[(slot["name"], slot["ordinal"])] = value
        elif k == "print":
            v = self.exp(n.field_("value"), frame)
            self.out.append("undefined" if v is UNDEF else str(v))
        elif k == "expr":
            self.exp(n.children[0], frame)
        elif k == "return":
            raise Return(self.exp(n.field_("value"), frame))
        elif k == "if":
            if self.exp(n.field_("condition"), frame):
                self.block(n.field_("consequence"), frame)
        elif k == "block":
            self.block(n, frame)
        elif k == "comment":
            pass
        elif k in self.r.scope_kind and self.r.scope_kind[k] == "function":
            pass  # hoisted already
        elif k == "pass":
            pass
        elif k == "global":
            pass
        else:
            raise RuntimeFail(f"statement {k}")

    def block(self, n, frame):
        f = Frame(n, frame)
        self.hoist(n, f)
        body = [c for c in n.children if c.field != "tail"]
        self.stmts(body, f)
        tail = n.field_("tail")
        return self.exp(tail, f) if tail is not None else None

    def exp(self, n, frame):
        k = n.kind
        if k == "id":
            slot = self.r.resolve(n)
            f = self.frame_for(frame, slot["scope"])
            key = (slot["name"], slot["ordinal"])
            if key not in f.values:
                raise RuntimeFail(f"{slot['name']} used before its binding")
            return f.values[key]
        if k == "exp_int" or k == "int":
            return int(self.r.text(n if k == "int" else n.children[0]))
        if k == "exp_bracket":
            return self.exp(n.children[0], frame)
        if k == "block":
            return self.block(n, frame)
        if k in ("add", "sub", "mul", "lt"):
            a, b = self.exp(n.field_("left"), frame), self.exp(n.field_("right"), frame)
            if a is UNDEF or b is UNDEF:
                raise RuntimeFail("arithmetic on undefined")
            return {"add": a + b, "sub": a - b, "mul": a * b, "lt": int(a < b)}[k]
        if k == "neg":
            return -self.exp(n.field_("operand"), frame)
        if k == "call":
            fn = self.exp(n.field_("function"), frame)
            args = [self.exp(a, frame) for a in n.fields("arguments")]
            if not isinstance(fn, Function):
                raise RuntimeFail("call of a non-function")
            f = Frame(fn.node, fn.frame)
            params = fn.node.fields("parameters")
            for p, a in zip(params, args):
                self.bind_here(p.field_("name"), a, f)
            self.hoist(fn.node, f)
            body = fn.node.field_("body")
            try:
                # The body block is its own scope in rustish; in jsish and
                # pyish the function node is the scope and the body a block.
                if body.kind in self.r.scope_kind:
                    return self.block(body, f)
                self.stmts(body.children, f)
            except Return as r:
                return r.value
            return None
        raise RuntimeFail(f"expression {k}")


def real_output(prog: Path):
    if prog.suffix == ".rs":
        with tempfile.TemporaryDirectory() as d:
            exe = Path(d) / "a.out"
            r = subprocess.run(["rustc", "-o", str(exe), str(prog)], capture_output=True, text=True)
            if r.returncode != 0:
                return None, r.stderr.strip()
            r = subprocess.run([str(exe)], capture_output=True, text=True)
    elif prog.suffix == ".js":
        r = subprocess.run(["node", str(prog)], capture_output=True, text=True)
    else:
        r = subprocess.run(["python3", str(prog)], capture_output=True, text=True)
    if r.returncode != 0:
        lines = [l.strip() for l in r.stderr.splitlines() if l.strip()]
        named = [l for l in lines if re.search(r"\w*Error", l)]
        return None, (named or lines or ["exit %d" % r.returncode])[0]
    return r.stdout.split(), None


def main(spike: Path, entry=None) -> int:
    b = json.loads((spike / "bindings.json").read_text())
    programs = sorted(p for p in (spike / "programs").iterdir() if p.suffix in (".rs", ".js", ".py"))
    results, passed = [], 0
    for prog in programs:
        src = prog.read_text()
        r = subprocess.run(["tree-sitter", "parse", str(prog)], cwd=spike, capture_output=True, text=True)
        tree = parse_tree(r.stdout)
        if tree is None or "ERROR" in r.stdout or "MISSING" in r.stdout:
            results.append((prog.name, False, "parser rejected: " + r.stdout, "-"))
            continue
        res = Resolver(tree, src.split("\n"), b)
        try:
            ours = Evaluator(res).run(entry)
            ours_text = " ".join(ours)
        except (Unresolved, RuntimeFail, RecursionError) as e:
            ours_text = f"error: {e}"
        real, err = real_output(prog)
        real_text = " ".join(real) if real is not None else f"error: {err}"
        ok = (real is None and ours_text.startswith("error:")) or (real is not None and ours_text == real_text)
        passed += ok
        results.append((prog.name, ok, ours_text, real_text))
    tool = {"rustish": "rustc", "jsish": "node", "pyish": "python3"}.get(spike.name, "the toolchain")
    out = [f"# Resolution results for {spike.name}", "", f"{passed} of {len(programs)} programs print, under resolution from bindings.json alone, what {tool} prints.", ""]
    for name, ok, ours, real in results:
        out += [f"## {'PASS' if ok else 'FAIL'}: {name}", "", "```" + (spike / "programs" / name).suffix[1:], (spike / "programs" / name).read_text().rstrip("\n"), "```", "",
                "| | output |", "|---|---|", f"| bindings.json | `{ours}` |", f"| {tool} | `{real}` |", ""]
    (spike / "resolve-results.md").write_text("\n".join(out) + "\n")
    print(f"{spike.name}: {passed}/{len(programs)} -> {spike / 'resolve-results.md'}")
    return 0 if passed == len(programs) else 1


if __name__ == "__main__":
    args = sys.argv[1:]
    entry = None
    if "--entry" in args:
        i = args.index("--entry")
        entry = args[i + 1]
        del args[i:i + 2]
    sys.exit(main(Path(args[0]).resolve(), entry))
