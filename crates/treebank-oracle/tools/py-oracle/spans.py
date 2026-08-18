"""Node BOUNDARIES from CPython, for the shape check.

stdin:  one file path per line
stdout: one JSON object per line, {"path":..., "spans":[[start,end,kind],...]}

The point is not to compare node NAMES across two parsers -- that needs a
correspondence table per language and is where this kind of check usually
dies. It is to compare where the boundaries fall. If CPython says something
spans bytes 15..20 and our tree has no node with exactly that span, our tree
has a different shape there, and a difference in shape is a bug in one of the
two parsers regardless of what either calls the node.

Offsets are absolute BYTES. `ast` reports (lineno, col_offset) where the
column is already a UTF-8 byte offset within its line, so only the line
starts have to be added back.
"""
import ast
import io
import json
import sys
import tokenize


def line_starts(data):
    """Byte offset of the start of each 1-indexed line."""
    starts = [0, 0]
    for i, b in enumerate(data):
        if b == 0x0A:
            starts.append(i + 1)
    return starts


def spans_of(tree, starts, size):
    out = []
    for node in ast.walk(tree):
        lineno = getattr(node, "lineno", None)
        end_lineno = getattr(node, "end_lineno", None)
        if lineno is None or end_lineno is None:
            # Contexts (Load/Store), operators and a few others carry no
            # position. They have no boundary to compare.
            continue
        if lineno >= len(starts) or end_lineno >= len(starts):
            continue
        start = starts[lineno] + node.col_offset
        end = starts[end_lineno] + node.end_col_offset
        if 0 <= start < end <= size:
            out.append([start, end, type(node).__name__])
    return out


def main():
    for line in sys.stdin:
        path = line.strip()
        if not path:
            continue
        # An unreadable file is an oracle FAILURE, never a verdict -- see
        # check.py. Exiting loudly is the whole contract.
        try:
            with open(path, "rb") as fh:
                data = fh.read()
        except OSError as exc:
            sys.stderr.write("py-oracle: cannot read %s: %s\n" % (path, exc))
            sys.exit(1)

        record = {"path": path, "spans": []}

        # `ast` reports columns as byte offsets into the source AS CPYTHON
        # DECODED IT. For a file with a PEP 263 coding declaration that is
        # not utf-8, or one carrying a BOM, that is a different byte string
        # from the file on disk, and every offset after the first difference
        # is meaningless. Say so instead of reporting offsets that do not
        # line up -- a wrong span reads as a disagreement about the code.
        try:
            encoding, _ = tokenize.detect_encoding(io.BytesIO(data).readline)
        except SyntaxError:
            encoding = "utf-8"
        if data.startswith(b"\xef\xbb\xbf") or encoding.lower().replace("_", "-") not in (
            "utf-8",
            "utf8",
        ):
            record["skipped"] = "source encoding %s: byte offsets would not line up" % encoding
            sys.stdout.write(json.dumps(record) + "\n")
            continue

        try:
            tree = ast.parse(data, filename=path)
        except (SyntaxError, ValueError, TypeError, RecursionError) as exc:
            # Only clean parses have meaningful boundaries. python2-only
            # files land here and are skipped, not counted as agreement.
            record["skipped"] = "parse: %s" % type(exc).__name__
        else:
            try:
                record["spans"] = spans_of(tree, line_starts(data), len(data))
            except RecursionError:
                record["skipped"] = "walk: RecursionError"
        sys.stdout.write(json.dumps(record) + "\n")


if __name__ == "__main__":
    main()
