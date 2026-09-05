#!/usr/bin/env python3
"""Hold the template-derived printer to the language's own formatter.

    python3 tools/format_check.py spike/rustish

For every program of the spike (programs/*, and the rosetta programs in
its language), three checks:

- round trip: parse -> implode -> print -> parse -> implode gives the
  same term (comments and blank lines aside), so printing loses nothing;
- idempotence: printing the printed text gives the same text;
- the oracle: the printed text equals what rustfmt, black or prettier
  produces for the same source, since the module's templates are written
  in that formatter's style.

Writes <dir>/format-results.md.
"""

import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FORMATTER = {
    ".rs": lambda p: subprocess.run(["rustfmt", "--emit", "stdout", "--edition", "2021", str(p)], capture_output=True, text=True).stdout.split("\n", 2)[2] if True else "",
    ".py": lambda p: subprocess.run(["python3", "-m", "black", "-q", "-"], input=p.read_text(), capture_output=True, text=True).stdout,
    ".js": lambda p: subprocess.run(["prettier", str(p)], capture_output=True, text=True).stdout,
}
TOOL = {".rs": "rustfmt", ".py": "black", ".js": "prettier"}


def fmt(spike: Path, prog: Path, term=False) -> str:
    args = ["cargo", "run", "-q", "-p", "treebank-sdf3", "--example", "format", "--", str(spike), str(prog)]
    if term:
        args.append("--term")
    r = subprocess.run(args, capture_output=True, text=True, cwd=ROOT)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.strip())
    return r.stdout


def main(spike: Path) -> int:
    ext = {"rustish": ".rs", "pyish": ".py", "jsish": ".js"}[spike.name]
    programs = sorted((spike / "programs").glob("*" + ext))
    programs += sorted((spike.parent / "rosetta").glob("*/program" + ext))
    results = []
    passed = 0
    for prog in programs:
        row = {"name": str(prog.relative_to(spike.parent)), "ok": True, "notes": []}
        try:
            t1 = fmt(spike, prog, term=True)
            out = fmt(spike, prog)
            with tempfile.NamedTemporaryFile("w", suffix=ext, delete=False) as f:
                f.write(out)
                tmp = Path(f.name)
            t2 = fmt(spike, tmp, term=True)
            out2 = fmt(spike, tmp)
            if t1 != t2:
                row["ok"] = False
                row["notes"].append("round trip changed the term")
            if out2 != out:
                row["ok"] = False
                row["notes"].append("printing is not idempotent")
            real = FORMATTER[ext](prog)
            if real.strip("\n") != out.strip("\n"):
                row["ok"] = False
                row["notes"].append(f"differs from {TOOL[ext]}")
                row["ours"], row["real"] = out, real
            row["printed"] = out
        except RuntimeError as e:
            row["ok"] = False
            row["notes"].append(f"error: {e.splitlines()[0] if e else e}")
        passed += row["ok"]
        results.append(row)
    lines = [f"# Format results for {spike.name}", "", f"{passed} of {len(programs)} programs round-trip, print idempotently, and print exactly what {TOOL[ext]} prints.", ""]
    for r in results:
        lines.append(f"## {'PASS' if r['ok'] else 'FAIL'}: {r['name']}")
        lines.append("")
        if r["notes"]:
            lines += ["- " + n for n in r["notes"]] + [""]
        if "printed" in r:
            lines += ["```" + ext[1:], r["printed"].rstrip("\n"), "```", ""]
        if "real" in r:
            lines += [f"{TOOL[ext]}:", "", "```" + ext[1:], r["real"].rstrip("\n"), "```", ""]
    (spike / "format-results.md").write_text("\n".join(lines) + "\n")
    print(f"{spike.name}: {passed}/{len(programs)} -> {spike / 'format-results.md'}")
    return 0 if passed == len(programs) else 1


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]).resolve()))
