#!/usr/bin/env python3
"""Hold an ANTLR lowering to the same corpus the tree-sitter one is held to.

    python3 tools/antlr_check.py spike/mini

Generates the Python3 target from `<dir>/<name>.g4` with the ANTLR tool,
parses every case in `<dir>/test/corpus/*.txt` (tree-sitter's corpus format,
fed the same way tree-sitter feeds it), prints each tree in the tree-sitter
S-expression vocabulary -- labeled alternatives as node names, element
labels as fields, named tokens as leaves, literals omitted, injections
elided -- and writes `<dir>/antlr-results.md` with a verdict per case.

The ANTLR jar is fetched to ~/.cache/treebank-sdf3 on first use. The Python
runtime must be installed: pip install antlr4-python3-runtime==4.13.2
"""

import importlib
import json
import os
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

ANTLR_VERSION = "4.13.2"


def jar_path() -> Path:
    cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "treebank-sdf3"
    cache.mkdir(parents=True, exist_ok=True)
    jar = cache / f"antlr4-{ANTLR_VERSION}-complete.jar"
    if not jar.exists():
        url = f"https://repo1.maven.org/maven2/org/antlr/antlr4/{ANTLR_VERSION}/antlr4-{ANTLR_VERSION}-complete.jar"
        print(f"fetching {url}", file=sys.stderr)
        urllib.request.urlretrieve(url, jar)
    return jar


def read_corpus(path: Path):
    """tree-sitter's corpus format. The input is everything between the header
    block and the `---` line, which is why it starts with a newline."""
    text = path.read_text()
    cases = []
    lines = text.split("\n")
    i = 0
    while i < len(lines):
        if lines[i].startswith("=" * 20):
            j = i + 1
            name_lines = []
            while j < len(lines) and not lines[j].startswith("=" * 20):
                name_lines.append(lines[j])
                j += 1
            attrs = [l for l in name_lines if l.startswith(":")]
            name = " ".join(l for l in name_lines if not l.startswith(":")).strip()
            k = j + 1
            body = []
            while k < len(lines) and not lines[k].startswith("-" * 20):
                body.append(lines[k])
                k += 1
            inp = "\n".join(body) + "\n"
            m = k + 1
            exp = []
            while m < len(lines) and not lines[m].startswith("=" * 20):
                exp.append(lines[m])
                m += 1
            cases.append({
                "name": name,
                "error": ":error" in attrs,
                "input": inp,
                "expected": " ".join("\n".join(exp).split()),
            })
            i = m
        else:
            i += 1
    return cases


def snake_of_context(cls_name: str, rule: str) -> str:
    base = cls_name[: -len("Context")]
    if base.lower() == rule.replace("_", "").lower() or base.lower() == rule.lower():
        return rule
    return base[0].lower() + base[1:]


def main(spike: Path) -> int:
    from antlr4 import CommonTokenStream, InputStream
    from antlr4.error.ErrorListener import ErrorListener
    from antlr4.tree.Tree import TerminalNode

    g4 = next(spike.glob("*.g4"))
    gname = g4.stem
    gen = spike / "antlr-gen"
    gen.mkdir(exist_ok=True)
    r = subprocess.run(
        ["java", "-jar", str(jar_path()), "-Dlanguage=Python3", "-o", str(gen), "-Xexact-output-dir", str(g4)],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        print(r.stderr, file=sys.stderr)
        return 2
    tool_warnings = [
        l for l in r.stderr.splitlines() if l.strip() and not l.startswith("Picked up JAVA_TOOL_OPTIONS")
    ]
    sys.path.insert(0, str(gen))
    lexer_mod = importlib.import_module(f"{gname}Lexer")
    parser_mod = importlib.import_module(f"{gname}Parser")
    Lexer = getattr(lexer_mod, f"{gname}Lexer")
    Parser = getattr(parser_mod, f"{gname}Parser")

    class Collect(ErrorListener):
        def __init__(self):
            self.errors = []

        def syntaxError(self, recognizer, offendingSymbol, line, column, msg, e):
            self.errors.append(f"{line}:{column} {msg}")

    def sexp(node, parser):
        if isinstance(node, TerminalNode):
            t = node.symbol
            if t.type == -1:
                return None
            name = parser.symbolicNames[t.type] if t.type < len(parser.symbolicNames) else None
            if not name or name == "<INVALID>" or name.startswith("V_"):
                return None
            return f"({name.lower()})"
        rule = parser.ruleNames[node.getRuleIndex()]
        label = snake_of_context(type(node).__name__, rule)
        fields = {}
        for k, v in vars(node).items():
            if k.startswith("_") or k in ("parser", "parentCtx", "invokingState", "children", "start", "stop", "exception"):
                continue
            if isinstance(v, list):
                for item in v:
                    fields[id(item)] = k
            elif v is not None and not isinstance(v, (int, str)):
                fields[id(v)] = k
        parts = []
        for ch in node.children or []:
            s = sexp(ch, parser)
            if s is None:
                continue
            # A labeled token is stored as the Token; the child is the
            # TerminalNode wrapping it. A label that collided with a rule
            # name carries a trailing underscore the emitter added.
            f = fields.get(id(ch))
            if f is None and isinstance(ch, TerminalNode):
                f = fields.get(id(ch.symbol))
            if f:
                f = f.rstrip("_")
            parts.append(f"{f}: {s}" if f else s)
        if label.startswith("inj_"):
            return " ".join(parts) if parts else None
        inner = (" " + " ".join(parts)) if parts else ""
        return f"({label}{inner})"

    cases = read_corpus(next((spike / "test" / "corpus").glob("*.txt")))
    start = None
    with open(g4) as f:
        for line in f:
            m = re.match(r"^([a-z_][a-z0-9_]*)\s*$", line)
            if m and start is None:
                start = m.group(1)
                break
    results = []
    passed = 0
    for case in cases:
        lexer = Lexer(InputStream(case["input"]))
        errs = Collect()
        lexer.removeErrorListeners()
        lexer.addErrorListener(errs)
        parser = Parser(CommonTokenStream(lexer))
        parser.removeErrorListeners()
        parser.addErrorListener(errs)
        tree = getattr(parser, start)()
        actual = " ".join(sexp(tree, parser).split()) if not errs.errors else None
        if case["error"]:
            ok = bool(errs.errors)
            actual_text = "; ".join(errs.errors) if errs.errors else actual
        else:
            ok = not errs.errors and actual == case["expected"]
            actual_text = "; ".join(errs.errors) if errs.errors else actual
        passed += ok
        results.append((case, ok, actual_text))

    out = [f"# ANTLR results for {gname}", "", f"{passed} of {len(cases)} corpus expectations hold under the ANTLR lowering.", ""]
    if tool_warnings:
        out += ["ANTLR tool output:", "", "```"] + tool_warnings + ["```", ""]
    for case, ok, actual in results:
        out.append(f"## {'PASS' if ok else 'FAIL'}: {case['name']}")
        out.append("")
        if not ok:
            out.append("expected:")
            out.append("")
            out.append("```")
            out.append(case["expected"] if not case["error"] else "(a syntax error)")
            out.append("```")
            out.append("")
            out.append("got:")
            out.append("")
            out.append("```")
            out.append(str(actual))
            out.append("```")
            out.append("")
    (spike / "antlr-results.md").write_text("\n".join(out) + "\n")
    print(f"{gname}: {passed}/{len(cases)} -> {spike / 'antlr-results.md'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]).resolve()))
