#!/usr/bin/env python3
"""The rosetta gate over the spike languages (notes/DESIGN.md §5.4).

    python3 tools/rosetta_check.py spike/rosetta

Every case directory holds the same program in pyish (.py), rustish (.rs)
and jsish (.js) and an expected.json of role queries with the count each
must yield in every language. Facet queries are expanded through each
spike's lowered roles.json, as treebank expands them at load time, and
table-tier queries run as written, since the vocabulary's supertypes are
real supertypes of the generated grammar. Captures are counted with the
pinned tree-sitter CLI. Writes <dir>/rosetta-results.md.
"""

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

SPIKES = {".py": "pyish", ".rs": "rustish", ".js": "jsish"}


def expand(query: str, roles: dict) -> str:
    """`(_callable) @fn` -> `[(def)] @fn` through the facet manifest; a
    table term is left alone."""
    facets = roles.get("facets", {})

    def sub(m):
        term = m.group(1)
        if term in facets:
            return "[" + " ".join(f"({n})" for n in facets[term]) + "]"
        return m.group(0)

    return re.sub(r"\((_[a-z_]+)\)", sub, query)


def count(spike_dir: Path, query: str, program: Path) -> int:
    with tempfile.NamedTemporaryFile("w", suffix=".scm", delete=False) as f:
        f.write(query + "\n")
        scm = f.name
    r = subprocess.run(["tree-sitter", "query", scm, str(program)], cwd=spike_dir, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"{query!r} on {program.name}: {r.stderr.strip()}")
    return sum(1 for l in r.stdout.splitlines() if l.strip().startswith("capture:"))


def main(root: Path) -> int:
    spikes = root.parent
    out = ["# Rosetta results for the spike languages", ""]
    total = passed = 0
    for case in sorted(p for p in root.iterdir() if p.is_dir()):
        expected = json.loads((case / "expected.json").read_text())
        programs = sorted(p for p in case.iterdir() if p.suffix in SPIKES)
        out += [f"## {case.name}", "", expected.get("note", ""), "", "| query | " + " | ".join(SPIKES[p.suffix] for p in programs) + " | expected |", "|---|" + "---|" * (len(programs) + 1)]
        for query, want in expected["queries"].items():
            row = []
            ok = True
            for prog in programs:
                spike = spikes / SPIKES[prog.suffix]
                roles = json.loads((spike / "roles.json").read_text())
                try:
                    got = count(spike, expand(query, roles), prog)
                except RuntimeError as e:
                    got = f"error: {e}"
                if got != want:
                    ok = False
                row.append(str(got))
            total += 1
            passed += ok
            out.append(f"| `{query}` | " + " | ".join(row) + f" | {want}{'' if ok else ' **differs**'} |")
        out.append("")
    out.insert(2, f"{passed} of {total} role queries yield the expected count in every spike language.")
    out.insert(3, "")
    (root / "rosetta-results.md").write_text("\n".join(out) + "\n")
    print(f"rosetta: {passed}/{total} -> {root / 'rosetta-results.md'}")
    return 0 if passed == total else 1


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]).resolve()))
