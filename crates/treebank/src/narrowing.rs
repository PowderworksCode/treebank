//! The `narrowing.json` manifest a family crate ships (notes/DESIGN.md §4.2,
//! "Narrowing a shared table").
//!
//! A rung-1 row shares its family's parse table and accepts less than that
//! table does. The manifest is how it says so: one key per row, each listing
//! the **out-of-row occurrences** — the constructs the shared grammar parses
//! that this row does not claim.
//!
//! Three properties, and each is load-bearing:
//!
//! - **An entry names a construct, never a position.** The matcher is a
//!   tree-sitter query, so `(print_statement)` matches wherever a py2 print
//!   appears and nowhere else, and text cannot be mistaken for a construct
//!   the way a substring match would mistake `print` inside a docstring.
//!   `fuzz_policy.toml`'s `node_kind` reached the same conclusion from the
//!   same failure, and a query is that idea with the two cases a bare kind
//!   cannot reach: an anonymous token (`(except_clause ",")`, which is the
//!   only thing separating py2's `except E, e:` from py3's `except E as e:`
//!   — the two build identical trees) and a text predicate (`0777` and
//!   `777` are both a bare `integer`, so only the digits tell them apart).
//!
//! - **An entry answers for an OCCURRENCE, never for a FILE.** Declaring
//!   python's five py2-union kinds to `fuzz` moved 31 findings out of
//!   undeclared and six should not have moved, because a py2-only construct
//!   nested inside a py3-only one is valid in neither version. A py2-only
//!   node kind in a file therefore does not make the file py2; which row a
//!   file belongs to stays the oracle's question (§4.3).
//!
//! - **What a manifest cannot do is declared, not discovered.** Narrowing
//!   runs downstream of the parse, so it can refuse a construct and can
//!   never change a reading. Where the shared table hands a row the wrong
//!   reading outright, `residue` records it, and that list is the priced
//!   case for giving the row a parse table of its own.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrowingManifest {
    /// The vocabulary version this manifest was written against.
    pub vocabulary: String,
    /// The grammar whose parse table every row here shares — the crate
    /// directory name without the `treebank-` prefix.
    pub grammar: String,
    /// Row name -> what that row narrows away.
    pub rows: BTreeMap<String, Row>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// What this row claims to parse, in one line.
    pub covers: String,
    /// The constructs the shared table admits and this row does not.
    #[serde(default)]
    pub out_of_row: Vec<Entry>,
    /// Where the shared table hands this row a reading it cannot accept.
    /// A manifest cannot repair these; they are the case for a second table.
    #[serde(default)]
    pub residue: Vec<Residue>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The construct, named the way a person would say it.
    pub construct: String,
    /// The matcher: a tree-sitter query against the shared grammar. It must
    /// compile, and it must match the fixture below.
    pub pattern: String,
    /// Where this construct IS valid, which is what makes it out-of-row here.
    pub valid_in: String,
    /// The version bound this entry narrows away, when the construct is a
    /// later addition rather than a removal (`match` arrives in 3.10). What
    /// `version_of()` reports as a floor.
    #[serde(default)]
    pub since: Option<String>,
    /// A file, relative to the crate root, that this pattern must match.
    /// Liveness: a narrowing nobody can trip fails the gate the way a role
    /// nobody threads does.
    pub fixture: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Residue {
    pub construct: String,
    /// The reading the shared table gives, and what this row needed instead.
    pub why: String,
}

impl NarrowingManifest {
    pub fn parse(json: &str) -> Result<NarrowingManifest> {
        serde_json::from_str(json).context("parse narrowing.json")
    }

    pub fn load(path: &Path) -> Result<NarrowingManifest> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Self::parse(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_parses_with_its_entries_and_residue() {
        let m = NarrowingManifest::parse(
            r#"{
              "vocabulary": "0.1.0",
              "grammar": "python",
              "rows": {
                "python3": {
                  "covers": "Python 3.x",
                  "out_of_row": [{
                    "construct": "py2 print statement",
                    "pattern": "(print_statement) @m",
                    "valid_in": "2.7",
                    "fixture": "test/narrowing/python3/print-statement.py"
                  }],
                  "residue": []
                }
              }
            }"#,
        )
        .unwrap();
        let row = &m.rows["python3"];
        assert_eq!(row.out_of_row.len(), 1);
        assert_eq!(row.out_of_row[0].pattern, "(print_statement) @m");
        assert!(row.out_of_row[0].since.is_none());
    }

    /// An unknown key is a typo that would otherwise narrow nothing and say
    /// nothing, so the manifest refuses it.
    #[test]
    fn an_unknown_key_is_refused() {
        let err = NarrowingManifest::parse(
            r#"{"vocabulary":"0.1.0","grammar":"python","rows":{},"extra":1}"#,
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("parse narrowing.json"));
    }
}
