#!/usr/bin/env python3
"""Hold the winnow lowering to the same corpus the tree-sitter one is held to.

    python3 tools/winnow_check.py spike/mini

Builds `<dir>/winnow/` (the crate the lowering emitted; the build goes to
the repository's target/winnow-spikes so every spike shares one compiled
winnow), runs every case of `<dir>/test/corpus/*.txt` through it, and
writes `<dir>/winnow-results.md` with a verdict per case, in the same
format as antlr_check.py's so tools/confer.py can read both.

A case whose title starts with WIDENING is one where tree-sitter accepts
what the SDF3 source rejects. The corpus expectation is tree-sitter's tree;
a rejection from a backend that follows the source is the other right
answer, and is reported as SOURCE rather than FAIL.
"""
import os
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from antlr_check import read_corpus  # noqa: E402


def build(crate: Path) -> Path:
    root = crate
    while not (root / "Cargo.lock").exists() and root.parent != root:
        root = root.parent
    target = Path(os.environ.get("CARGO_TARGET_DIR", root / "target" / "winnow-spikes"))
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    r = subprocess.run(["cargo", "build", "-q", "--offline"], cwd=crate, env=env, capture_output=True, text=True)
    if r.returncode != 0:
        r = subprocess.run(["cargo", "build", "-q"], cwd=crate, env=env, capture_output=True, text=True)
        if r.returncode != 0:
            print(r.stderr, file=sys.stderr)
            sys.exit(2)
    name = None
    for line in (crate / "Cargo.toml").read_text().splitlines():
        if line.startswith("name = "):
            name = line.split('"')[1]
    return target / "debug" / name


def parse(binary: Path, text: str):
    with tempfile.NamedTemporaryFile("w", suffix=".src", delete=False) as f:
        f.write(text)
        path = f.name
    try:
        r = subprocess.run([str(binary), path], capture_output=True, text=True)
    finally:
        os.unlink(path)
    out = r.stdout.strip()
    if r.returncode != 0 or out.startswith("ERROR"):
        return None, out
    return " ".join(out.split()), out


def main(spike: Path) -> int:
    binary = build(spike / "winnow")
    cases = read_corpus(next((spike / "test" / "corpus").glob("*.txt")))
    results = []
    passed = 0
    source = 0
    for case in cases:
        actual, raw = parse(binary, case["input"])
        widening = case["name"].upper().startswith("WIDENING")
        if case["error"]:
            ok = actual is None
            verdict = "PASS" if ok else "FAIL"
        elif actual == case["expected"]:
            ok = True
            verdict = "PASS"
        elif widening and actual is None:
            ok = True
            source += 1
            verdict = "SOURCE"
        else:
            ok = False
            verdict = "FAIL"
        passed += ok
        results.append((case, verdict, raw if actual is None else actual))

    name = spike.name
    out = [f"# winnow results for {name}", "", f"{passed} of {len(cases)} corpus expectations hold under the winnow lowering" + (f", {source} of them by rejecting what the source rejects (SOURCE)." if source else "."), ""]
    for case, verdict, actual in results:
        out.append(f"## {verdict}: {case['name']}")
        out.append("")
        if verdict != "PASS":
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
    (spike / "winnow-results.md").write_text("\n".join(out) + "\n")
    print(f"{name}: {passed}/{len(cases)} -> {spike / 'winnow-results.md'}")
    return 0 if passed == len(cases) else 1


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]).resolve()))
