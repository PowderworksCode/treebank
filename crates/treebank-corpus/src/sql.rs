use std::path::Path;

use anyhow::Result;

use crate::rank::RankedCrate;
use crate::{debian, Ecosystem};
use treebank_lang::LangName;

pub struct Sql;

/// **SQL is a guest language, and more completely so than shell.** No
/// package is written in SQL. It arrives as schema, migrations, seed data,
/// stored views and test fixtures inside software written in something
/// else, so the corpus question is the one bash's Debian path answers —
/// "which installed packages carry SQL" — and not the registry question
/// python, rust and typescript answer.
///
/// That makes Debian the right source and, for this language, very nearly
/// the only honest one. There is no SQL registry to rank. GitHub's
/// `language:SQL` selects repositories that are *mostly* SQL, which for a
/// guest language is a small and unrepresentative tail: dialect tutorials,
/// interview-question repos and single-file sample databases, not the
/// migration directories where SQL actually lives. popcon ranks the
/// package, so what this measures is "SQL that ships inside software people
/// install" — that is the honest reading of it, and it is not the same as
/// "popular SQL".
///
/// The population it selects, stated plainly: database engines and their
/// test suites (postgresql-18 alone carries 130,141 lines of it), ORM and
/// driver packages, web applications with a migration history, and the
/// long tail of packages with one `schema.sql`. Dialect is therefore
/// skewed toward whatever the shipping engine speaks, and Debian ships
/// postgres, mysql and sqlite in that order of bulk.
fn is_sql(s: &debian::Sloc) -> bool {
    s.lines("sql") >= SQL_MIN
}

/// The same floor bash uses, for the same reason: enough of the language to
/// be worth a download, wherever in the package it sits. It is deliberately
/// not tuned — a floor picked to make a number look better is a floor that
/// has stopped measuring anything.
const SQL_MIN: i64 = 500;

/// Extensions this grammar's `tree-sitter.json` claims. `.ddl` and `.dml`
/// are the two conventional splits of a schema dump; `.psql` and `.mysql`
/// are dialect-tagged scripts, and they are in the corpus for the same
/// reason they are in the grammar — the dialects are a union, not a choice.
const SQL_EXTENSIONS: [&str; 5] = ["sql", "ddl", "dml", "psql", "mysql"];

impl Ecosystem for Sql {
    fn name(&self) -> LangName {
        LangName::Sql
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        debian::rank(LangName::Sql, db, k, "sql-carrying", &is_sql)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        debian::resolve(LangName::Sql, pkg)
    }

    /// Extension only, and no second-stage filter. Shell needed one because
    /// its files have no extension; SQL has the opposite problem — a `.sql`
    /// file is unambiguously SQL, and the ambiguity is about WHICH DIALECT,
    /// which no rule over the path can settle and which this grammar
    /// deliberately does not have to.
    ///
    /// What is excluded, and why it is excluded here rather than adjudicated
    /// later: `.sql.in`, `.sql.tmpl` and friends are templates that RENDER to
    /// SQL — autoconf substitutions and Jinja alike — and they are not SQL.
    /// They fail `extension() == "sql"` already, since the extension is the
    /// last one. `.sql` files that are still templates (a `${schema}` inside
    /// an otherwise ordinary script) are real and are left in: they are a
    /// declared blind spot in ledger.toml rather than a filter, because no
    /// cheap rule separates them from SQL that merely mentions a brace.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        SQL_EXTENSIONS
            .contains(&rel.extension()?.to_str()?.to_ascii_lowercase().as_str())
            .then_some(None)
    }

    /// **No cap, and the contrast with bash is the whole reason.**
    ///
    /// Bash caps artifacts at 250 MB because its giants are incidental: the
    /// top-500 Debian sources come to 11.5 GB of which 7.6 GB is eight
    /// packages -- texlive, chromium, libreoffice -- that carry shell as
    /// build glue and are 1.6% of the corpus. Two-thirds of the bytes for
    /// one part in sixty, so capping costs almost nothing.
    ///
    /// For SQL the giants ARE the population. The same cap was tried here
    /// first and skipped five sources carrying 190,189 lines of SQL between
    /// them -- `mysql-9.7` alone has 87,855, second only to postgres in the
    /// entire ranking, and it is the only first-party MySQL in a corpus for
    /// a grammar whose whole claim is a dialect UNION. chromium and
    /// qt6-webengine are next, at 50,041 and 44,944, because a browser
    /// vendors sqlite and its test suite with it.
    ///
    /// So the artifact size is uncorrelated with how much SQL a package
    /// carries, and capping on it is sampling by download convenience. The
    /// cost is real and is stated rather than hidden: the corpus is ~3.3 GB
    /// larger to fetch. That is a price paid once per refresh, against a
    /// blind spot that would have been permanent.
    fn max_artifact_bytes(&self) -> Option<u64> {
        None
    }
}
