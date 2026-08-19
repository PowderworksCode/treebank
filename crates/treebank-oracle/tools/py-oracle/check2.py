# -*- coding: utf-8 -*-
# Syntax-only Python 2 validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The python2 half of the union oracle (DESIGN.md 4.3): the rust driver
# runs check.py under python3 first and this under python2 for whatever
# python3 rejected — a file is valid if ANY version family accepts it.
# This script must run under CPython 2.7, so it is written in the common
# subset. Same rules as check.py: compile() not ast.parse (post-parse
# SyntaxErrors count), bytes in so coding declarations are honoured, an
# unreadable file is an oracle failure and never a verdict.
import sys
import warnings


def parses(path):
    try:
        f = open(path, "rb")
        try:
            src = f.read()
        finally:
            f.close()
    except (IOError, OSError):
        e = sys.exc_info()[1]
        sys.stderr.write(
            "py2-oracle: cannot read %s: %s\n"
            "py2-oracle: this is an oracle failure, not a verdict; "
            "check the corpus root\n" % (path, e)
        )
        sys.exit(1)
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            compile(src, path, "exec", 0, 1)  # flags=0, dont_inherit=1
        return True
    except (SyntaxError, ValueError, TypeError, MemoryError, RuntimeError):
        # RuntimeError: py2 raises it (maximum recursion depth) where py3
        # has RecursionError. NUL bytes in the source are a VERDICT, not an
        # oracle failure — the bytes were read fine, they are just not
        # valid python — but py2 reports them as TypeError where py3 uses
        # ValueError, so both must be caught or the oracle dies on a file
        # it should simply reject. Found by widening the corpus to the
        # top-1000 packages; the py3 leg had this right already.
        return False


def main():
    out = sys.stdout
    # `iter(readline, '')` rather than `for line in sys.stdin`: the file
    # iterator reads ahead by a block, so a persistent oracle blocks until
    # its caller sends enough data or closes the pipe -- which is exactly
    # what a sentinel protocol must not require. python2's read-ahead is
    # the larger, but neither is safe here.
    for line in iter(sys.stdin.readline, ''):
        path = line.strip()
        if path == "\x00--end--":
            out.write("\x00--end--\n")
            out.flush()
            continue
        if not path:
            continue
        out.write("%s\t%s\n" % (path, "valid" if parses(path) else "invalid"))
    out.flush()


if __name__ == "__main__":
    main()
