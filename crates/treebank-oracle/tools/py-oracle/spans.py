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

# Token types that mark no text: layout the parser synthesises, and the
# stream's own bookends. Everything else has a real extent to compare.
LAYOUT = frozenset(
    getattr(tokenize, n)
    for n in ("ENCODING", "ENDMARKER", "NEWLINE", "NL", "INDENT", "DEDENT")
    if hasattr(tokenize, n)
)


def line_starts(data):
    """Byte offset of the start of each 1-indexed line."""
    starts = [0, 0]
    for i, b in enumerate(data):
        if b == 0x0A:
            starts.append(i + 1)
    return starts


def edge_span(node, starts, size):
    """Absolute byte span of an AST node, or None when it carries no position."""
    lineno = getattr(node, "lineno", None)
    end_lineno = getattr(node, "end_lineno", None)
    if lineno is None or end_lineno is None:
        return None
    if lineno >= len(starts) or end_lineno >= len(starts):
        return None
    start = starts[lineno] + node.col_offset
    end = starts[end_lineno] + node.end_col_offset
    if not (0 <= start < end <= size):
        return None
    return start, end


# Four `ast` classes carry no position at all: `comprehension`, `withitem`,
# `match_case` and `arguments`. That is not an oversight in CPython -- they
# are joint nodes whose extent is implied by their children rather than
# recorded. We build a node for three of them, so a boundary check that
# skips them is blind to exactly the joins.
#
# The extent can be recovered as the HULL of the positioned children, but
# only the first of these is honest on its own:
#
#   withitem      `a as b`         -- the `with` belongs to the STATEMENT,
#                                     so the hull is already exact
#   match_case    `case P: body`   -- hull starts at the pattern, one `case`
#                                     keyword short
#   comprehension `for x in y`     -- hull starts at the target, one `for`
#                                     (or `async for`) short
#
# `arguments` is not recoverable: `def f():` has an empty one with no
# children to take a hull of, and our `parameters` includes the parens the
# hull can never see. It stays skipped.
# Per kind: the leading keyword the hull cannot see, and which fields to
# take the hull OVER. `comprehension` needs the second: CPython hangs the
# conditions off the same node (`ifs`), while we build a separate
# `if_clause` beside the `for_in_clause`. Including them would compare our
# node against a span that is deliberately a different node, so the hull is
# taken over the clause HEAD -- which is exactly what our node is.
HULL_KINDS = {
    "withitem": ((), None),
    "match_case": ((b"case",), None),
    "comprehension": ((b"for",), ("target", "iter")),
}


def extend_left(data, start, words):
    """Move `start` back over whitespace to swallow a leading keyword.

    Returns None rather than guessing: a hull we cannot complete is not a
    boundary claim we are entitled to make.
    """
    i = start
    while i > 0 and data[i - 1 : i] in (b" ", b"\t", b"\n", b"\r", b"\\"):
        i -= 1
    for w in words:
        if not data[:i].endswith(w):
            continue
        j = i - len(w)
        # A word boundary, or `for` matches the tail of `endfor`.
        if j > 0:
            prev = data[j - 1 : j]
            if prev.isalnum() or prev == b"_":
                return None
        # `async for` -- the comprehension owns the `async` too.
        k = j
        while k > 0 and data[k - 1 : k] in (b" ", b"\t", b"\n", b"\r", b"\\"):
            k -= 1
        if data[:k].endswith(b"async"):
            a = k - 5
            if a == 0 or not (data[a - 1 : a].isalnum() or data[a - 1 : a] == b"_"):
                return a
        return j
    return None


def bracket_pairs(data, starts, text_lines):
    """`{open_offset: close_offset}` and its inverse, from the TOKEN stream.

    A hull cannot see enclosing brackets, because CPython records no
    position for them -- `with (yield from pool) as conn` gives a
    `context_expr` spanning `yield from pool`, so the hull stops one byte
    inside the paren on each side and the boundary never matches ours.

    Balancing fixes it, but only if the brackets are real: a `)` inside a
    string literal is not one. The tokenizer already tells us which is
    which, so take the pairs from there rather than scanning bytes.
    """
    def at(row, col):
        line = text_lines[row] if row < len(text_lines) else ""
        return starts[row] + len(line[:col].encode("utf-8"))

    opens, closes, stack = {}, {}, []
    try:
        for t in tokenize.tokenize(io.BytesIO(data).readline):
            if t.type != tokenize.OP or t.string not in "()[]{}":
                continue
            if t.start[0] >= len(starts):
                continue
            off = at(t.start[0], t.start[1])
            if t.string in "([{":
                stack.append((t.string, off))
            elif stack and stack[-1][0] == "([{"[")]}".index(t.string)]:
                _, o = stack.pop()
                opens[o] = off
                closes[off] = o
    except (tokenize.TokenError, SyntaxError, IndentationError, ValueError):
        return {}, {}
    return opens, closes


def balance(data, lo, hi, opens, closes):
    """Widen a hull over brackets that open before it or close after it."""
    for _ in range(64):
        depth, unmatched_close, unmatched_open = 0, None, None
        i = lo
        while i < hi:
            if i in opens:
                if opens[i] >= hi:
                    unmatched_open = i
                    break
                i = opens[i]
                continue
            if i in closes and closes[i] < lo:
                unmatched_close = i
                break
            i += 1
        if unmatched_close is not None:
            lo = closes[unmatched_close]
        elif unmatched_open is not None:
            hi = opens[unmatched_open] + 1
        else:
            return lo, hi
    return lo, hi


def hull_span(node, data, starts, size, opens, closes):
    """Span of a positionless joint node, from its positioned children."""
    spec = HULL_KINDS.get(type(node).__name__)
    if spec is None:
        return None
    words, fields = spec
    if fields is None:
        roots = [node]
    else:
        roots = [getattr(node, f, None) for f in fields]
    lo, hi = size + 1, -1
    for root in roots:
        if not isinstance(root, ast.AST):
            continue
        for child in ast.walk(root):
            span = edge_span(child, starts, size)
            if span is None:
                continue
            lo, hi = min(lo, span[0]), max(hi, span[1])
    if hi < 0:
        return None
    lo, hi = balance(data, lo, hi, opens, closes)
    if words:
        lo2 = extend_left(data, lo, words)
        if lo2 is None:
            return None
        lo = lo2
    return (lo, hi) if 0 <= lo < hi <= size else None


def edges_of(tree, starts, size):
    """Labelled parent -> child edges: [pstart, pend, pkind, field, cstart, cend].

    Spans say what is there; edges say how it is connected. Two trees can
    agree on every node and still attach the children under different names,
    and the names are what a consumer reads -- `orelse` versus `body` is the
    difference between a program and its opposite.
    """
    out = []
    for node in ast.walk(tree):
        parent = edge_span(node, starts, size)
        if parent is None:
            continue
        pkind = type(node).__name__
        for field in node._fields:
            value = getattr(node, field, None)
            children = value if isinstance(value, list) else [value]
            for child in children:
                if not isinstance(child, ast.AST):
                    continue
                span = edge_span(child, starts, size)
                if span is not None:
                    out.append([parent[0], parent[1], pkind, field, span[0], span[1]])
    return out


def tokens_of(data, starts, size, text_lines):
    """Token extents from CPython's own tokenizer, as byte spans.

    A second oracle one level below `ast`, and the only one we have for the
    lexer. Two parsers can build identical trees over a token stream they
    disagree about -- a numeric literal form, an operator glued together, a
    string prefix -- and nothing above this level would notice.
    """
    # `tokenize` and `ast` do NOT agree on what a column is. `ast.col_offset`
    # is documented as a UTF-8 BYTE offset; a token's start and end are
    # CHARACTER indices into the decoded line. Reading the second as the first
    # puts every token after a non-ASCII character on the wrong byte, which is
    # exactly what the first run of this check reported -- boundaries landing
    # inside identifiers, in files containing box-drawing characters.
    def at(row, col):
        line = text_lines[row] if row < len(text_lines) else ""
        return starts[row] + len(line[:col].encode("utf-8"))

    out = []
    try:
        for t in tokenize.tokenize(io.BytesIO(data).readline):
            if t.type in LAYOUT:
                continue
            if t.start[0] >= len(starts) or t.end[0] >= len(starts):
                continue
            start = at(t.start[0], t.start[1])
            end = at(t.end[0], t.end[1])
            if 0 <= start < end <= size:
                out.append([start, end])
    except (tokenize.TokenError, SyntaxError, IndentationError, ValueError):
        # The tokenizer gave up part way. What it produced before that is
        # not a complete account of the file, so report none of it.
        return None
    return out


def spans_of(tree, starts, size, data, opens, closes):
    out = []
    for node in ast.walk(tree):
        lineno = getattr(node, "lineno", None)
        end_lineno = getattr(node, "end_lineno", None)
        if lineno is None or end_lineno is None:
            # Contexts (Load/Store), operators and a few others carry no
            # position. They have no boundary to compare -- except the
            # joint nodes, whose extent their children imply.
            hull = hull_span(node, data, starts, size, opens, closes)
            if hull is not None:
                out.append([hull[0], hull[1], type(node).__name__])
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

        record = {"path": path, "spans": [], "edges": []}

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
            # ...but WHERE it failed is worth reporting. Rejecting the right
            # files at the wrong offset makes error recovery useless to an
            # editor and misleads every gap investigation, and nothing has
            # ever checked it.
            lineno = getattr(exc, "lineno", None)
            offset = getattr(exc, "offset", None)
            if lineno is not None and offset is not None:
                starts = line_starts(data)
                if 0 < lineno < len(starts):
                    lines = [""] + data.decode("utf-8", "replace").split("\n")
                    line = lines[lineno] if lineno < len(lines) else ""
                    # `offset` is a 1-based CHARACTER column.
                    col = max(0, offset - 1)
                    record["error"] = starts[lineno] + len(line[:col].encode("utf-8"))
        else:
            try:
                starts = line_starts(data)
                text_lines0 = [""] + data.decode("utf-8", "replace").split("\n")
                opens, closes = bracket_pairs(data, starts, text_lines0)
                record["spans"] = spans_of(tree, starts, len(data), data, opens, closes)
                record["edges"] = edges_of(tree, starts, len(data))
                # Split on "\n" ONLY, and 1-index to match `starts`.
                # `splitlines()` also breaks on \r\n, \x0c and \u2028, while
                # `starts` counts \n -- so on any file containing one of
                # those the two lists desync and every column after it lands
                # on the wrong byte. Which is what the second run of this
                # check reported: boundaries inside identifiers, in files
                # with a LINE SEPARATOR in a string literal.
                text_lines = [""] + data.decode("utf-8", "replace").split("\n")
                toks = tokens_of(data, starts, len(data), text_lines)
                if toks is not None:
                    record["tokens"] = toks
            except RecursionError:
                record["skipped"] = "walk: RecursionError"
        sys.stdout.write(json.dumps(record) + "\n")


if __name__ == "__main__":
    main()
