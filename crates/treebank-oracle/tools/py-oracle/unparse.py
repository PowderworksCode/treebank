"""Re-render each file from CPython's own AST, for the round-trip check.

stdin:  one file path per line
stdout: one JSON object per line, {"path":..., "source":...} or {"path":..., "skipped":...}

`ast.unparse` prints the tree back as source in ONE canonical spelling: no
comments, normalised quotes and spacing, parentheses only where the tree
needs them. Parsing that with our grammar asks a question the corpus cannot:
whether we handle each construct in the form the language's own tools emit,
rather than only in the form its authors happened to write.
"""
import ast
import json
import sys


def main():
    for line in sys.stdin:
        path = line.strip()
        if not path:
            continue
        record = {"path": path}
        try:
            with open(path, "rb") as fh:
                data = fh.read()
        except OSError as exc:
            sys.stderr.write("py-oracle: cannot read %s: %s\n" % (path, exc))
            sys.exit(1)
        try:
            tree = ast.parse(data, filename=path)
        except (SyntaxError, ValueError, RecursionError):
            # Not ours to round-trip; the sweep already judges these.
            record["skipped"] = "parse"
        else:
            try:
                record["source"] = ast.unparse(tree)
            except (RecursionError, ValueError, AttributeError) as exc:
                # `ast.unparse` gives up on some deeply nested trees. Its
                # limitation, not the grammar's.
                record["skipped"] = "unparse: %s" % type(exc).__name__
        sys.stdout.write(json.dumps(record) + "\n")


if __name__ == "__main__":
    main()
