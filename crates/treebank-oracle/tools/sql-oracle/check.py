# Syntax-only SQL validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The reference parser is SQLite's own, reached through the standard
# library's `sqlite3` module. It is the only SQL parser that can be run with
# nothing installed and no server: PostgreSQL's is inside the server (or
# inside libpg_query, a C library to build), and MySQL's is inside mysqld.
#
# **Never executes.** Every statement is prepared under `EXPLAIN`, which
# compiles it to a VDBE program and returns that program's listing instead
# of running it. So a corpus file that drops a table does not drop one, and
# a file that reads from the network does not read from it. Statement
# splitting is `sqlite3.complete_statement`, which is sqlite3_complete() and
# therefore knows that the `;` inside a trigger's `BEGIN … END` does not end
# the statement -- splitting on `;` does not.
#
# **The syntax/semantics line, and where it is drawn.** Preparing resolves
# names, so `SELECT * FROM t` fails on an empty database with "no such
# table: t" -- a file that is perfectly good SQL. That verdict would be
# catastrophic here: `validate` is only ever called on files the grammar
# already failed, and an `invalid` answer books the file as corpus NOISE, so
# an oracle that answers invalid for schema it does not have would convert
# every grammar failure into noise and report a flawless grammar. So the
# verdict is taken from the ERROR MESSAGE: sqlite's parser produces a short,
# closed set of messages (near "X": syntax error / unrecognized token /
# incomplete input / parser stack overflow), and everything else it can say
# is about names, types or context -- which is to say about the schema this
# oracle deliberately does not have. Only the parser's own messages are
# `invalid`.
#
# **What this cannot see, which is the ceiling on a SQL sweep.** SQLite is
# one dialect. PostgreSQL-only and MySQL-only syntax that the grammar
# accepts is not contradicted here, and a file that fails BOTH the grammar
# and SQLite -- a postgres file using a construct neither knows -- is booked
# as noise rather than as the gap it may be. That direction is the safe one
# (the sweep can miss a grammar bug, it can never invent one), and it is why
# ledger.toml names a second oracle as the first thing to add rather than
# claiming a dialect-union number this cannot support.
import sqlite3
import sys

# SQLite's parser errors, and nothing else. Everything sqlite can say about
# a statement that PARSED -- no such table, no such column, no such
# function, ambiguous column name, misuse of aggregate -- is a fact about
# the schema this oracle does not have, not about the file's syntax.
SYNTAX_MARKERS = (
    "syntax error",
    "unrecognized token",
    "incomplete input",
    "parser stack overflow",
    'near "',
)


def statements(text: str):
    """Split a script the way sqlite3_complete() does.

    Accumulates lines until the buffer is a complete statement, so a `;`
    inside a trigger body or a quoted string does not split it. A trailing
    fragment that never completes is yielded as-is: it is either an
    incomplete statement (which the parser will call invalid, correctly) or
    a comment tail (which is empty after stripping and is skipped).
    """
    buf = ""
    for line in text.splitlines(keepends=True):
        buf += line
        if sqlite3.complete_statement(buf):
            yield buf
            buf = ""
    if buf.strip():
        yield buf


def parses(conn: sqlite3.Connection, sql: str) -> bool:
    stripped = sql.strip().rstrip(";").strip()
    if not stripped:
        return True
    # `EXPLAIN EXPLAIN …` is itself a syntax error, and so is `EXPLAIN` in
    # front of the handful of statements sqlite refuses to explain. Both are
    # cheap to detect and neither says anything about the file.
    prefix = "" if stripped[:7].upper() == "EXPLAIN" else "EXPLAIN "
    try:
        conn.execute(prefix + stripped)
        return True
    except sqlite3.Warning:
        # "You can only execute one statement at a time" -- the splitter
        # handed over something it should not have. Not a verdict.
        return True
    except sqlite3.Error as e:
        message = str(e).lower()
        return not any(m in message for m in SYNTAX_MARKERS)


def valid(path: str) -> bool:
    # An unreadable file is NOT an invalid file: an `invalid` verdict books
    # the file as corpus noise, so an oracle that answers it for files it
    # could not read reports a flawless grammar. Fail loudly instead.
    try:
        with open(path, "rb") as f:
            raw = f.read()
    except OSError as e:
        sys.stderr.write(
            f"sql-oracle: cannot read {path}: {e}\n"
            "sql-oracle: this is an oracle failure, not a verdict; "
            "check the corpus root\n"
        )
        sys.exit(1)
    # Corpus SQL is full of latin-1 dumps and stray bytes inside string
    # literals. Replacing undecodable bytes keeps the parse honest about the
    # syntax while refusing to turn an encoding accident into a verdict.
    text = raw.decode("utf-8", errors="replace")
    # A NUL in the text is not a read failure and not a verdict this oracle
    # is guessing at: SQLite's own C API takes a NUL-terminated string, so a
    # statement containing one cannot be put to it at all, and
    # complete_statement raises rather than answering. In a `.sql` corpus
    # this is almost always a UTF-16 export or a binary blob that happens to
    # carry the extension, so `invalid` -- which books the file as corpus
    # NOISE -- is the right classification as well as the only available
    # one. Measured on the Debian corpus: 1 file of 3,494.
    if "\x00" in text:
        return False
    conn = sqlite3.connect(":memory:")
    try:
        return all(parses(conn, s) for s in statements(text))
    finally:
        conn.close()


# A batch ends at EOF (the sweep, one launch over the whole corpus) or at
# this line (fuzz, which asks about one program at a time and again at every
# shrink step), matching the other oracles here.
SENTINEL = "\x00--end--"


def main() -> None:
    out = sys.stdout
    for line in iter(sys.stdin.readline, ""):
        path = line.strip()
        if path == SENTINEL:
            out.write(SENTINEL + "\n")
            out.flush()
            continue
        if not path:
            continue
        out.write(f"{path}\t{'valid' if valid(path) else 'invalid'}\n")
    out.flush()


if __name__ == "__main__":
    main()
