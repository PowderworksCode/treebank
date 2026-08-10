# Syntax-only Python validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The reference parser is CPython's own. `ast.parse` runs the parser and
# stops: it never imports, never executes, and never resolves a name, so a
# file is judged entirely on its own text — the same property that makes
# ts.createSourceFile usable for TypeScript, JavacTask.parse() for Java and
# Roslyn's ParseText for C#. Missing imports are not errors here.
#
# `compile(..., PyCF_ONLY_AST)` is what ast.parse calls; going through
# ast.parse keeps the feature-version knob below in one place.
#
# The language version is whatever CPython is running this, and that is a
# real knob rather than an incidental: `match` is 3.10+, parenthesised
# context managers are 3.9+ (3.10 in practice), and the walrus is 3.8+. A
# file that needs syntax newer than this interpreter is not valid Python
# *here*, and recording it as corpus noise is the honest answer. ledger.json
# records the version the sweep numbers were produced with, exactly as
# generate_cli records the CLI.
#
# Deliberately NOT tolerant of Python 2. `print "x"` is a syntax error to
# every supported CPython, and calling it valid would turn the grammar's
# correct rejection of Python 2 into a reported grammar gap — the same trap
# as pointing the JavaScript oracle at the TypeScript parser.
import ast
import sys


def parses(path: str) -> bool:
    try:
        with open(path, "rb") as f:
            src = f.read()
    except OSError:
        return False
    try:
        # Bytes input lets CPython honour a PEP 263 coding declaration and
        # strip a BOM itself, which decoding to str here would not.
        ast.parse(src, filename=path)
        return True
    except (SyntaxError, ValueError, MemoryError, RecursionError):
        # ValueError covers embedded NULs; RecursionError covers pathological
        # nesting, which is a real thing in generated corpus files.
        return False


def main() -> None:
    out = sys.stdout
    for line in sys.stdin:
        path = line.strip()
        if not path:
            continue
        out.write(f"{path}\t{'valid' if parses(path) else 'invalid'}\n")
    out.flush()


if __name__ == "__main__":
    main()
