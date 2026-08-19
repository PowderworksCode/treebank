# Syntax-ONLY validity, for measuring what the compile() oracle cannot see.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# check.py drives `compile(src, path, "exec")` on purpose, and its header
# explains why: CPython enforces rules AFTER the parser that are still
# SyntaxErrors and still make a file unusable — `return` outside a function,
# a bare `except:` that is not last, duplicate parameter names. Judging those
# files invalid is what keeps deliberately-broken test fixtures out of the
# gap count.
#
# That choice has a cost, and the same header states it: a file that is
# compile-invalid for one reason AND has a real grammar gap for another is
# recorded as noise rather than a gap. This script exists to MEASURE that
# cost rather than to replace the oracle. `ast.parse` stops at the parser,
# so a file it accepts and `compile` rejects is exactly one where the
# difference is post-parse — and if the grammar also rejects it, that is a
# gap the sweep cannot see.
import ast
import sys


def main():
    # `iter(readline, '')` rather than `for line in sys.stdin`: the file
    # iterator reads ahead by a block, so a persistent oracle blocks until
    # its caller sends enough data or closes the pipe -- which is exactly
    # what a sentinel protocol must not require. python2's read-ahead is
    # the larger, but neither is safe here.
    for line in iter(sys.stdin.readline, ''):
        path = line.strip()
        if path == "\x00--end--":
            sys.stdout.write("\x00--end--\n")
            sys.stdout.flush()
            continue
        if not path:
            continue
        try:
            with open(path, "rb") as fh:
                data = fh.read()
        except OSError as exc:
            sys.stderr.write("py-oracle: cannot read %s: %s\n" % (path, exc))
            sys.stderr.write("py-oracle: this is an oracle failure, not a verdict\n")
            sys.exit(1)
        try:
            ast.parse(data, filename=path)
            ok = True
        except (SyntaxError, ValueError, RecursionError):
            ok = False
        sys.stdout.write("%s\t%s\n" % (path, "valid" if ok else "invalid"))


if __name__ == "__main__":
    main()
