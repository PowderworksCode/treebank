# Syntax-only Python validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The reference parser is CPython's own, driven through `compile(src, path,
# "exec")` — the exact call Python makes to turn a module's text into code.
# It never imports, never executes and never resolves a name, so a missing
# dependency is not an error and a file is judged entirely on its own text,
# the same property that makes ts.createSourceFile usable for TypeScript and
# JavacTask.parse() for Java.
#
# Why compile() and not ast.parse(). ast.parse stops after the parser, and
# CPython enforces a set of rules *after* it that are still SyntaxErrors and
# still make the file unusable as a module: `return` or `await` outside a
# function, `_ = *[42]` ("can't use starred expression here"), a bare
# `except:` that is not last, duplicate parameter names. ast.parse accepts
# all of those. Measured on the first top-500 sweep: 11 of 30 files the
# sweep called grammar gaps were files CPython would refuse to run —
# fragments and deliberately-invalid fixtures shipped by ruff, black,
# pylint and parso as test data. compile() records them as corpus noise,
# which is what they are.
#
# The cost, stated plainly: a file that is compile-invalid for one reason
# AND has a real grammar gap for another is now noise rather than a gap, so
# gap_files can under-report. That is the same direction C's indeterminate
# collapse chose and it is the safe one — the sweep can miss a grammar bug,
# it can never invent one.
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
import sys
import warnings


def parses(path: str) -> bool:
    try:
        with open(path, "rb") as f:
            src = f.read()
    except OSError:
        return False
    try:
        # Bytes input lets CPython honour a PEP 263 coding declaration and
        # strip a BOM itself, which decoding to str here would not.
        #
        # dont_inherit keeps this process's own __future__ flags out of the
        # judgement, and warnings are silenced because corpus code raises
        # plenty of SyntaxWarnings ("is not" with a literal, invalid escape
        # sequences) that are not errors and would otherwise pour into the
        # sweep's stderr.
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            compile(src, path, "exec", dont_inherit=True)
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
