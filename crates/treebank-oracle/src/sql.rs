use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Sql;

impl Oracle for Sql {
    fn name(&self) -> LangName {
        LangName::Sql
    }

    /// The UNION oracle for a union grammar (DESIGN.md §4.3): a file is
    /// valid SQL if ANY dialect accepts it. SQLite judges every file first,
    /// through `tools/sql-oracle/check.py`; whatever it rejects is
    /// re-judged by MySQL through `tools/mysql-oracle/check.py`.
    ///
    /// MySQL is REQUIRED, not optional, for the same reason python2 is
    /// required for python: a union grammar swept with half its oracle
    /// books every MySQL-only file as noise, which silently hides that
    /// half's gaps. `ON DUPLICATE KEY UPDATE`, `SET @v = 1`, backquoted
    /// identifiers and `group_concat(x SEPARATOR ',')` are all invisible to
    /// SQLite. If mysqld cannot be started the oracle errors loudly rather
    /// than degrading to a number that looks like evidence and is not.
    ///
    /// SQLite's own parser, through the standard library's `sqlite3`
    /// module and `tools/sql-oracle/check.py`. Every statement is prepared
    /// under `EXPLAIN`, which compiles it and returns the VDBE listing
    /// instead of running it, so no corpus file ever executes.
    ///
    /// MySQL's own parser is the server's, because MySQL ships no
    /// standalone one: `tools/mysql-oracle/check.py` starts a mysqld with
    /// `--skip-networking` on a throwaway datadir and puts each statement
    /// to it as `PREPARE stmt FROM '…'`, which parses and prepares without
    /// running. Its verdict is an ERROR CODE rather than a message — 1064
    /// and 1149 are the parser's, and 1046/1049/1146/1054 are all about a
    /// schema it does not have — which is a cleaner line than SQLite's,
    /// where the classification has to match strings.
    ///
    /// **What is still missing is PostgreSQL,** whose parser lives inside
    /// the server or inside libpg_query, a C library to build. So a
    /// postgres-only construct neither SQLite nor MySQL accepts is still
    /// booked as corpus noise rather than as the gap it may be. That is the
    /// safe direction — the sweep can miss a grammar bug, it can never
    /// invent one — and it is now the LAST such hole rather than two thirds
    /// of the union.
    ///
    /// The syntax/semantics line is drawn on the error message rather than
    /// on the error class, because preparing resolves names: `SELECT * FROM
    /// t` against an empty database fails with "no such table", and a file
    /// that is perfectly good SQL must not be booked as noise for it.
    /// SQLite's parser produces a short closed set of messages and
    /// everything else it can say is about a schema this oracle
    /// deliberately does not have.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let mut verdicts = stdin_oracle::persistent(
            "sql",
            "python3",
            &[crate::tool("sql-oracle/check.py")
                .to_string_lossy()
                .as_ref()],
            "python3 tools/sql-oracle/check.py — is python3 installed?",
            srcroot,
            paths,
        )?;

        let sqlite_rejected: Vec<String> = paths
            .iter()
            .filter(|p| verdicts.get(*p).copied() == Some(false))
            .cloned()
            .collect();
        if sqlite_rejected.is_empty() {
            return Ok(verdicts);
        }
        let mysql = stdin_oracle::persistent(
            "mysql",
            "python3",
            &[crate::tool("mysql-oracle/check.py")
                .to_string_lossy()
                .as_ref()],
            "python3 tools/mysql-oracle/check.py — is mysqld installed?",
            srcroot,
            &sqlite_rejected,
        )?;
        for (path, ok) in mysql {
            if ok {
                verdicts.insert(path, true);
            }
        }
        Ok(verdicts)
    }

    /// The same map, and not the default `None`: both halves judge syntax
    /// and nothing else BY CONSTRUCTION. Neither executes a statement —
    /// SQLite prepares under `EXPLAIN`, MySQL under `PREPARE` — and the one
    /// post-parse judgement preparing would otherwise make, name resolution
    /// against a schema neither has, is exactly what the message
    /// classification and the error-code classification throw away.
    fn validate_syntax_only(
        &self,
        srcroot: &Path,
        paths: &[String],
    ) -> Result<Option<HashMap<String, bool>>> {
        self.validate(srcroot, paths).map(Some)
    }
}
