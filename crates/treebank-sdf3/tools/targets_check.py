#!/usr/bin/env python3
"""Hold a family's per-target parsers to its corpus, and the corpus to the
real thing.

    targets_check.py <family dir> [--require-oracles]

<family dir>/targets.json names the targets (each a module lowered to
<family dir>/targets/<target with / as ->/), the corpus directory, and the
oracle. Every corpus file starts with a `targets:` line naming the targets
that accept it; every other target must reject it. For each file and each
target the check is: the generated parser accepts iff the header says so.
Where an oracle exists for a target -- a server of exactly that version,
or rustc with that edition -- the header's claim is itself checked against
the oracle, so the matrix is not this spike's word for it.

Two oracle kinds:

  sql    servers, one per target, each an environment variable holding a
         client command line (`psql -h ... -d ...`, `mariadb -S ...`).
         Each file runs against a fresh copy of schema.sql. A syntax error
         (psql: `syntax error at or near`; mariadb: ERROR 1064) is a
         rejection; success is acceptance; any other error means the corpus
         file is semantically wrong and the run fails, since the oracle
         cannot then say anything about syntax.
  rustc  `rustc --edition <target's last path segment>` on each file;
         acceptance is a clean exit.

Writes <family dir>/targets-results.md and exits non-zero on any mismatch.
"""
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path


def sh(cmd, cwd=None, inp=None):
    r = subprocess.run(cmd, cwd=cwd, input=inp, capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def target_dir(root, t):
    return root / "targets" / t.replace("/", "-")


def parse_with(root, t, f):
    """(accepted, tree with positions stripped)."""
    code, out = sh(["tree-sitter", "parse", str(f)], cwd=target_dir(root, t))
    tree = re.sub(r" \[\d+, \d+\] - \[\d+, \d+\]", "", out)
    # Keep the S-expression only: the CLI prints a config warning around it.
    tree = "\n".join(l for l in tree.splitlines() if l.startswith("(") or l.startswith(" "))
    ok = code == 0 and "(ERROR" not in out and "MISSING" not in out and "(UNEXPECTED" not in out
    return ok, tree.strip()


def oracle_sql(kind_cfg, root, t, f):
    var = kind_cfg["servers"].get(t)
    if not var:
        return None
    cmd = os.environ.get(var)
    if not cmd:
        return "unavailable"
    argv = shlex.split(cmd)
    schema = (root / kind_cfg["schema"]).read_text()
    body = f.read_text()
    if argv[0].endswith("psql"):
        script = "BEGIN;\n" + schema + body + "\nROLLBACK;\n"
        code, out = sh(argv + ["-X", "-v", "ON_ERROR_STOP=1", "-q"], inp=script)
        if "syntax error at or near" in out:
            return False
        if "ERROR:" in out or code != 0:
            return ("inconclusive", out.strip())
        return True
    # mariadb / mysql: DDL commits, so each file gets a fresh database.
    db = "treebank_targets"
    sh(argv + ["-e", f"DROP DATABASE IF EXISTS {db}; CREATE DATABASE {db};"])
    code, out = sh(argv + [db], inp=schema + body)
    if "ERROR 1064" in out:
        return False
    if "ERROR" in out or code != 0:
        return ("inconclusive", out.strip())
    return True


def oracle_rustc(kind_cfg, root, t, f):
    edition = t.rsplit("/", 1)[-1]
    with tempfile.TemporaryDirectory() as d:
        code, out = sh(["rustc", "--edition", edition, "--crate-type", "bin", "--emit=metadata", "-o", f"{d}/x", str(f)])
    return code == 0


def main():
    root = Path(sys.argv[1]).resolve()
    require = "--require-oracles" in sys.argv
    cfg = json.loads((root / "targets.json").read_text())
    targets = cfg["targets"]
    files = sorted((root / cfg["corpus"]).glob("*" + cfg["suffix"]))
    oracle = cfg.get("oracle", {})
    run_oracle = {"sql": oracle_sql, "rustc": oracle_rustc}.get(oracle.get("kind"))
    comment = cfg.get("comment", "--")

    failures = []
    unavailable = set()
    rows = []
    trees = {}
    for f in files:
        head = f.read_text().splitlines()[0]
        m = re.match(re.escape(comment) + r"\s*targets:\s*(.*)", head)
        if not m:
            failures.append(f"{f.name}: no `{comment} targets:` header")
            continue
        expect = set(m.group(1).split())
        unknown = expect - set(targets)
        if unknown:
            failures.append(f"{f.name}: unknown targets {sorted(unknown)}")
        cells = []
        by_tree = {}
        for t in targets:
            want = t in expect
            got, tree = parse_with(root, t, f)
            if got:
                by_tree.setdefault(tree, []).append(t)
            mark = "✓" if got else "✗"
            if got != want:
                failures.append(f"{f.name} on {t}: parser {'accepted' if got else 'rejected'}, header says {'accept' if want else 'reject'}")
                mark += "!"
            if run_oracle:
                o = run_oracle(oracle, root, t, f)
                if o == "unavailable":
                    unavailable.add(t)
                elif isinstance(o, tuple):
                    failures.append(f"{f.name} on {t}: oracle inconclusive: {o[1]}")
                    mark += "?"
                elif o is not None:
                    if o != want:
                        failures.append(f"{f.name} on {t}: oracle {'accepted' if o else 'rejected'}, header says {'accept' if want else 'reject'}")
                        mark += "!"
                    else:
                        mark += "•"
            cells.append(mark)
        rows.append((f.name, cells))
        if len(by_tree) > 1:
            trees[f.name] = by_tree

    out = []
    out.append(f"# {root.name}: one parser per target, held to the corpus\n")
    out.append("GENERATED by tools/targets_check.py. ✓ accepted, ✗ rejected; `•` the oracle for that exact version agrees with the header; `!` a mismatch; `?` the oracle could not judge.\n")
    out.append("| file | " + " | ".join(targets) + " |")
    out.append("|---|" + "---|" * len(targets))
    for name, cells in rows:
        out.append(f"| {name} | " + " | ".join(cells) + " |")
    checked = [t for t in targets if run_oracle and oracle.get("kind") == "rustc" or t in oracle.get("servers", {})]
    checked = [t for t in checked if t not in unavailable]
    out.append("")
    out.append(f"Oracle-checked targets: {', '.join(checked) or 'none'}." + (f" Unavailable: {', '.join(sorted(unavailable))}." if unavailable else ""))
    out.append("Every other column is the header's claim, cited to the release notes in the module that adds or hides the feature, held to the parser only.")
    if trees:
        out.append("\n## Same text, different trees\n")
        out.append("Files more than one target accepts with different trees. The constructor names come from the same modules, so where the targets agree the trees are identical; where they differ, the difference is the dialect.\n")
        for name, by_tree in trees.items():
            out.append(f"### {name}\n")
            for tree, ts in by_tree.items():
                out.append(f"{', '.join(ts)}:\n\n```\n{tree}\n```\n")
    (root / "targets-results.md").write_text("\n".join(out) + "\n")

    if unavailable and require:
        failures.append(f"oracles unavailable for {sorted(unavailable)} (set the environment variables targets.json names)")
    for m in failures:
        print("FAIL", m)
    n = len(files) * len(targets)
    print(f"{root.name}: {len(files)} files x {len(targets)} targets = {n} cells, {len(failures)} failures; oracle-checked {checked}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
