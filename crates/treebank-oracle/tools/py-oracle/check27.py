# Syntax-only Python 2.7 validity check, running under CPython 3.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The reference parser is CPython 2.7's OWN parser. `typed_ast.ast27` is
# that parser — Python/graminit.c and the 2.7 grammar, vendored as a C
# extension for python3 — so this is not a reimplementation or an
# approximation of Python 2, it is Python 2's parser reached from a python3
# process.
#
# Why not just run python2. Because CI has no python2 binary, and the
# alternative that keeps one is a container: an EOL interpreter, pulled per
# job, for a yes/no answer. This gets the same parser from a pinned wheel.
# A real CPython 2.7 stays useful as the ADJUDICATOR for anything this and
# the grammar disagree about, which is exactly how rust uses `syn`
# in-process and `rustc -Zparse-only` for the arguments (DESIGN.md §4.3).
#
# Two differences from check.py, both of which change what the numbers
# mean and so are stated rather than discovered:
#
# 1. This is `parse`, not `compile`. check.py judges with compile() so that
#    post-parse SyntaxErrors — `return` outside a function, a bare `except:`
#    that is not last — count as invalid, which keeps deliberately-broken
#    fixtures out of the gap count. typed_ast exposes the parser only, so
#    this oracle is the `ast.parse` equivalent and is LAXER. The direction
#    matters: a laxer oracle calls more files valid, so it can only ever
#    over-report gaps, never hide them. python3's ledger records the
#    opposite bias for compile() and calls it out as understating gaps;
#    this one errs the safe way for a grammar under construction.
#
# 2. PEP 263 has to be handled here rather than by the parser. typed_ast
#    decodes the bytes to unicode before parsing, and CPython 2's parser
#    then refuses a source with a coding declaration in it ("encoding
#    declaration in Unicode string"). Coding declarations are ordinary in
#    python 2 code, so left alone this rejects a large fraction of the
#    corpus and calls it invalid — measured at 77% of files in py2-era
#    sdists before it was found. The declaration is neutralised below, in
#    the only two lines PEP 263 lets it appear on, preserving byte LENGTH so
#    that any offset into the source is still the offset on disk.
import re
import sys

try:
    from typed_ast import ast27
except ImportError:
    sys.stderr.write(
        "py2-oracle: typed_ast is not installed.\n"
        "py2-oracle: it carries CPython 2.7's own parser; install the "
        "pinned version with `pip install typed_ast==1.5.5`.\n"
    )
    sys.exit(1)

# PEP 263: the declaration may only appear on line 1 or 2, in a comment.
CODING = re.compile(rb"^([ \t\f]*#.*?)coding([:=])", re.M)


def neutralise_coding(raw: bytes) -> bytes:
    """Blank the PEP 263 declaration without moving any other byte.

    `coding` becomes `codinX`: same length, no longer matches the regex
    CPython 2 uses to find it. Only the first two lines are touched,
    because that is the only place the declaration is meaningful — a later
    line saying `coding:` is ordinary text and must stay ordinary text.
    """
    first, sep, rest = raw.partition(b"\n")
    second, sep2, tail = rest.partition(b"\n")
    head = CODING.sub(rb"\1codinX\2", first + sep + second + sep2)
    return head + tail


def parses(path: str) -> bool:
    # An unreadable file is NOT an invalid file. Same rule as check.py: an
    # invalid verdict records the file as corpus NOISE, so a mistyped
    # corpus root would silently turn the whole sweep green.
    try:
        with open(path, "rb") as f:
            raw = f.read()
    except OSError as e:
        sys.stderr.write(
            "py2-oracle: cannot read %s: %s\n"
            "py2-oracle: this is an oracle failure, not a verdict; "
            "check the corpus root\n" % (path, e)
        )
        sys.exit(1)
    try:
        ast27.parse(neutralise_coding(raw))
        return True
    except (SyntaxError, ValueError, TypeError, MemoryError, RecursionError):
        # A NUL byte or a bad coding declaration is a VERDICT — python 2
        # would refuse the file too — not an oracle failure.
        return False


# The persistent-oracle protocol, identical to check.py's: a sentinel line
# in each direction, so the caller can write a batch and read its answers
# without closing the pipe. Writing a fresh main() here instead of copying
# that one is what made the first run die -- the sentinel begins with a NUL,
# `open()` refuses a path containing one, and the oracle exited mid-batch.
SENTINEL = "\x00--end--"


def main() -> None:
    out = sys.stdout
    # `iter(readline, '')` rather than `for line in sys.stdin`: the file
    # iterator reads ahead by a block, so a persistent oracle blocks until
    # its caller sends enough data or closes the pipe -- exactly what a
    # sentinel protocol must not require.
    for line in iter(sys.stdin.readline, ""):
        path = line.strip()
        if path == SENTINEL:
            out.write(SENTINEL + "\n")
            out.flush()
            continue
        if not path:
            continue
        out.write("%s\t%s\n" % (path, "valid" if parses(path) else "invalid"))
    out.flush()


if __name__ == "__main__":
    main()
