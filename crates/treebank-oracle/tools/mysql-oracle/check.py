# MySQL validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The reference parser is MySQL's own -- the server's, because MySQL ships
# no standalone parser -- reached through `PREPARE stmt FROM '…'`, which
# parses and prepares a statement and does not run it. That is the same
# shape of check `EXPLAIN` gives for SQLite and `compile()` gives for
# CPython: the file is judged on its own text.
#
# **Why this exists.** The SQLite oracle is one dialect adjudicating a
# dialect-union grammar, and it cannot contradict MySQL-only syntax:
# `ON DUPLICATE KEY UPDATE`, `REPLACE INTO`, backquoted identifiers,
# `group_concat(x SEPARATOR ',')`, `SET @v = 1`. Every one of those was
# either an unadjudicable failure or a gap taken on documentation rather
# than evidence. This oracle is what turns them into measurements.
#
# **The verdict is an ERROR CODE, not a message.** MySQL numbers its
# errors, so the syntax/semantics line needs no string matching: 1064
# (ER_PARSE_ERROR) and 1149 (ER_SYNTAX_ERROR) are the parser's, and
# everything else -- 1046 no database selected, 1049 unknown database,
# 1146 no such table, 1054 unknown column -- is about a schema this oracle
# deliberately does not have. 1295 (ER_UNSUPPORTED_PS) is the interesting
# one: it means the statement PARSED and the prepared-statement protocol
# will not carry it, which is a fact about the protocol, not the text.
#
# **What it cannot see.** It is MySQL 8.0, and the corpus carries mysql
# 9.7's own test suite, so syntax newer than 8.0 is judged invalid here --
# the same knob the python oracle documents about its interpreter, and the
# same honest answer: a file that needs syntax this server does not have is
# not valid MySQL *here*. Recorded in ledger.toml next to the version.
import os
import re
import subprocess
import sys
import tempfile

# The parser's own errors, and nothing else.
SYNTAX_ERRNOS = {1064, 1149}

# A statement bigger than this is not put to the server: max_allowed_packet
# would refuse it, and a refusal is not a verdict. The corpus has one such
# file -- a 67 MB mysql-test dump whose statements run to a megabyte -- and
# it is booked as unadjudicable rather than guessed at.
MAX_STATEMENT = 4 * 1024 * 1024

SENTINEL = "\x00--end--"


def split(text):
    """Split a MySQL script into statements.

    MySQL's own lexical rules, because nothing else will do: `'`, `"` and
    backquote all quote, backslash escapes inside them, `--` comments only
    when followed by whitespace, `#` to end of line, `/* */` blocks, and
    `/*!…*/` version comments whose CONTENTS are SQL the server executes.
    `DELIMITER` changes the terminator, which is how every dump that
    defines a trigger or a routine is written.

    Yields `(statement, terminated)`. The flag carries the lesson the
    SQLite oracle learned the hard way: a trailing fragment that no
    terminator closed may be one unterminated statement or several the
    splitter failed to see, and the caller must not treat the two alike.
    """
    delim = ";"
    out = []
    buf = []
    at_start = True          # buf holds nothing but whitespace so far
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        # DELIMITER is a client directive, not SQL: it is recognised only at
        # the start of a statement and never reaches the server.
        #
        # `at_start` is TRACKED rather than recomputed, and that is not a
        # micro-optimisation. Joining the buffer to test it made this
        # function quadratic in statement length: the corpus's 67 MB dump is
        # 158 lines of about a megabyte each, so the join ran a million
        # times over a million characters and the oracle sat at 100% CPU for
        # twenty minutes while the server it was meant to be driving idled
        # at 1%. Same shape as the string-escape bug in the ledger, found
        # the same way -- by looking at which process was actually busy.
        if at_start:
            m = re.match(r"[ \t]*delimiter[ \t]+(\S+)[ \t]*(\r?\n|$)", text[i:], re.I)
            if m:
                delim = m.group(1)
                i += m.end()
                buf = []
                continue
        if c in ("'", '"', "`"):
            at_start = False
            quote = c
            buf.append(c)
            i += 1
            while i < n:
                if text[i] == "\\" and quote != "`" and i + 1 < n:
                    buf.append(text[i : i + 2])
                    i += 2
                    continue
                if text[i] == quote:
                    # A doubled quote is an escaped one, in all three.
                    if i + 1 < n and text[i + 1] == quote:
                        buf.append(text[i : i + 2])
                        i += 2
                        continue
                    buf.append(quote)
                    i += 1
                    break
                buf.append(text[i])
                i += 1
            continue
        if text.startswith("--", i) and (i + 2 >= n or text[i + 2] in " \t\r\n"):
            j = text.find("\n", i)
            i = n if j < 0 else j + 1
            buf.append(" ")
            continue
        if c == "#":
            j = text.find("\n", i)
            i = n if j < 0 else j + 1
            buf.append(" ")
            continue
        if text.startswith("/*", i):
            j = text.find("*/", i + 2)
            end = n if j < 0 else j + 2
            # `/*!40101 … */` is a version comment: the server reads what is
            # inside it, so it is kept rather than stripped.
            if text.startswith("/*!", i):
                at_start = False
                buf.append(text[i:end])
            else:
                buf.append(" ")
            i = end
            continue
        if text.startswith(delim, i):
            out.append(("".join(buf), True))
            buf = []
            at_start = True
            i += len(delim)
            continue
        buf.append(c)
        if at_start and not c.isspace():
            at_start = False
        i += 1
    if "".join(buf).strip():
        out.append(("".join(buf), False))
    return out


def literal(sql):
    """Escape a statement into a single-quoted MySQL string literal, on one
    line -- the line number is how a batch error is matched back to the
    statement that caused it, so no real newline may survive."""
    return (
        sql.replace("\\", "\\\\")
        .replace("'", "\\'")
        .replace("\r", "\\r")
        .replace("\n", "\\n")
        .replace("\x1a", "\\Z")
    )


def server():
    """A socket for a running mysqld, starting one if needed.

    `TREEBANK_MYSQL_SOCKET` points at an existing server; otherwise a
    throwaway one is initialised under the cache dir and left running for
    reuse, the way the node oracles leave `node_modules` in place. It is
    started with `--skip-networking`, so nothing outside this machine can
    reach it, and its datadir holds no corpus data -- statements are
    prepared, never executed.
    """
    sock = os.environ.get("TREEBANK_MYSQL_SOCKET")
    if sock and os.path.exists(sock):
        return sock
    cache = os.path.join(
        os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache")),
        "treebank",
        "mysql-oracle",
    )
    datadir = os.path.join(cache, "data")
    sock = os.path.join(cache, "mysqld.sock")
    if os.path.exists(sock):
        return sock
    os.makedirs(cache, exist_ok=True)
    files = os.path.join(cache, "files")
    tmp = os.path.join(cache, "tmp")
    for d in (files, tmp):
        os.makedirs(d, exist_ok=True)
    if not os.path.isdir(datadir):
        sys.stderr.write("mysql-oracle: initialising a server under %s\n" % cache)
        r = subprocess.run(
            ["mysqld", "--initialize-insecure", "--user=root",
             "--datadir=" + datadir, "--tmpdir=" + tmp],
            capture_output=True, text=True,
        )
        if r.returncode != 0:
            sys.stderr.write(r.stderr[-2000:] + "\n")
            sys.stderr.write(
                "mysql-oracle: could not initialise mysqld. This is an oracle "
                "failure, not a verdict.\n"
            )
            sys.exit(1)
    subprocess.Popen(
        ["mysqld", "--user=root", "--datadir=" + datadir, "--tmpdir=" + tmp,
         "--socket=" + sock, "--skip-networking", "--skip-grant-tables",
         "--secure-file-priv=" + files,
         "--pid-file=" + os.path.join(cache, "mysqld.pid")],
        stdout=subprocess.DEVNULL, stderr=open(os.path.join(cache, "mysqld.log"), "w"),
    )
    for _ in range(600):
        if os.path.exists(sock):
            return sock
        subprocess.run(["sleep", "0.1"])
    sys.stderr.write(
        "mysql-oracle: mysqld did not come up; see %s/mysqld.log. This is an "
        "oracle failure, not a verdict.\n" % cache
    )
    sys.exit(1)


def judge(sock, checks):
    """Run one batch of PREPAREs and return the set of check indexes that
    failed with a PARSER error.

    One check per line, so mysql's `at line N` names the check. `--force`
    is what makes a batch possible at all: without it the client stops at
    the first error and every later statement goes unjudged.
    """
    if not checks:
        return set()
    with tempfile.NamedTemporaryFile("w", suffix=".sql", delete=False) as f:
        for sql in checks:
            f.write("PREPARE p FROM '%s';\n" % literal(sql))
        path = f.name
    try:
        with open(path) as fh:
            r = subprocess.run(
                ["mysql", "--socket=" + sock, "-u", "root", "--batch",
                 "--skip-column-names", "--force"],
                stdin=fh, capture_output=True, text=True,
            )
    finally:
        os.unlink(path)
    bad = set()
    for line in r.stderr.splitlines():
        m = re.match(r"ERROR (\d+) \([^)]*\) at line (\d+)", line)
        if not m:
            continue
        errno, lineno = int(m.group(1)), int(m.group(2))
        if errno in SYNTAX_ERRNOS:
            bad.add(lineno - 1)
    return bad


def judge_paths(sock, paths):
    """Verdicts for one batch of paths."""
    checks = []          # every statement of every file, flattened
    owner = []           # checks[i] belongs to paths[owner[i]]
    verdict = [True] * len(paths)
    for idx, path in enumerate(paths):
        # An unreadable file is NOT an invalid file: `invalid` books it as
        # corpus noise, so an oracle that answers it for files it could not
        # read reports a flawless grammar. Fail loudly instead.
        try:
            with open(path, "rb") as fh:
                raw = fh.read()
        except OSError as e:
            sys.stderr.write(
                "mysql-oracle: cannot read %s: %s\nmysql-oracle: this is an "
                "oracle failure, not a verdict; check the corpus root\n" % (path, e)
            )
            sys.exit(1)
        text = raw.decode("utf-8", errors="replace")
        if "\x00" in text:
            verdict[idx] = False
            continue
        for sql, terminated in split(text):
            body = sql.strip()
            if not body:
                continue
            if len(body) > MAX_STATEMENT or (not terminated and len(body) > 65536):
                # Too big to put to the server, or an unterminated tail long
                # enough that it is probably several statements the splitter
                # did not see. Neither is a verdict; book it as noise.
                verdict[idx] = False
                continue
            checks.append(body)
            owner.append(idx)

    # In batches, so one temp file never holds every statement in the
    # corpus. The line number mysql reports is relative to the batch, so the
    # offset is added back before it is used to name a check.
    BATCH = 4000
    for start in range(0, len(checks), BATCH):
        for i in judge(sock, checks[start : start + BATCH]):
            j = start + i
            if 0 <= j < len(owner):
                verdict[owner[j]] = False
    return verdict


def main():
    """One batch per SENTINEL, and the process STAYS ALIVE between them.

    That is the contract `stdin_oracle::persistent` expects -- it writes
    paths, writes the sentinel, and reads verdicts until the sentinel comes
    back -- and getting it wrong presents as `oracle exited mid-batch`
    rather than as anything about SQL. Keeping the process up is also what
    makes the mysqld worth starting: it is initialised once and reused for
    every batch of the sweep.
    """
    out = sys.stdout
    sock = None
    paths = []
    for line in iter(sys.stdin.readline, ""):
        path = line.strip()
        if path == SENTINEL:
            if paths:
                if sock is None:
                    sock = server()
                for p, ok in zip(paths, judge_paths(sock, paths)):
                    out.write("%s\t%s\n" % (p, "valid" if ok else "invalid"))
                paths = []
            out.write(SENTINEL + "\n")
            out.flush()
            continue
        if path:
            paths.append(path)
    if paths:
        if sock is None:
            sock = server()
        for p, ok in zip(paths, judge_paths(sock, paths)):
            out.write("%s\t%s\n" % (p, "valid" if ok else "invalid"))
    out.flush()


if __name__ == "__main__":
    main()
