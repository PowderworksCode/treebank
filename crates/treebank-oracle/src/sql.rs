use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Sql;

impl Oracle for Sql {
    fn name(&self) -> LangName {
        LangName::Sql
    }

    /// SQLite's own parser, through the standard library's `sqlite3`
    /// module and `tools/sql-oracle/check.py`. Every statement is prepared
    /// under `EXPLAIN`, which compiles it and returns the VDBE listing
    /// instead of running it, so no corpus file ever executes.
    ///
    /// **This is a ONE-DIALECT oracle for a dialect-union grammar, and
    /// that asymmetry is the ceiling on what a SQL sweep can mean.**
    /// PostgreSQL's parser lives inside the server (or libpg_query, a C
    /// library to build) and MySQL's inside mysqld; SQLite's is the only
    /// one reachable with nothing installed. So postgres-only and
    /// mysql-only syntax is never contradicted here, and a file that fails
    /// both the grammar and SQLite is booked as corpus noise rather than as
    /// the gap it may be. That is the safe direction — the sweep can miss a
    /// grammar bug, it can never invent one — and ledger.toml names a
    /// second oracle as the first thing to add rather than claiming a
    /// union number this cannot support.
    ///
    /// The syntax/semantics line is drawn on the error message rather than
    /// on the error class, because preparing resolves names: `SELECT * FROM
    /// t` against an empty database fails with "no such table", and a file
    /// that is perfectly good SQL must not be booked as noise for it.
    /// SQLite's parser produces a short closed set of messages and
    /// everything else it can say is about a schema this oracle
    /// deliberately does not have.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::persistent(
            "sql",
            "python3",
            &[crate::tool("sql-oracle/check.py")
                .to_string_lossy()
                .as_ref()],
            "python3 tools/sql-oracle/check.py — is python3 installed?",
            srcroot,
            paths,
        )
    }

    /// The same map, and not the default `None`: this oracle judges syntax
    /// and nothing else BY CONSTRUCTION. It never executes a statement, and
    /// the one post-parse judgement preparing would otherwise make — name
    /// resolution against a schema it does not have — is exactly what the
    /// message classification throws away.
    fn validate_syntax_only(
        &self,
        srcroot: &Path,
        paths: &[String],
    ) -> Result<Option<HashMap<String, bool>>> {
        self.validate(srcroot, paths).map(Some)
    }
}
