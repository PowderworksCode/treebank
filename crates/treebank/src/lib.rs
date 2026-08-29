//! The treebank vocabulary (DESIGN.md §3), as code.
//!
//! Two tiers with different physics, dictated by what tree-sitter's
//! supertype mechanism can express:
//!
//! - **Table tier** — real supertype rules threaded through a grammar's
//!   productions. Occurrence-level semantics, enforced at generate time,
//!   natively queryable.
//! - **Facet tier** — roles that cross-cut derivations and therefore cannot
//!   be supertypes. Shipped as a `roles.json` manifest per grammar crate
//!   (type-level membership) and expanded into concrete alternations at
//!   query-load time by [`expand`].
//!
//! A term's tier is a property of the *grammar*, not of the vocabulary:
//! terms listed in `either_tier` may be delivered by either mechanism, and
//! each grammar picks. The table tier is stronger and is the default; a
//! grammar demotes a term to a facet only when its language partitions the
//! position (Python orders parameters, so `_parameter` cannot be one
//! alternation without the grammar accepting `def f(a=1, b)`). Demotion is
//! sound precisely when every member of the term is a concrete node type
//! that occurs nowhere else, since then type-level and occurrence-level
//! membership select the same nodes; see DESIGN.md §3.4.
//!
//! The vocabulary itself lives in `vocabulary/vocabulary.json`, embedded
//! here and re-exported to JavaScript by `vocabulary/supertypes.js`, so the
//! grammars and this crate can never disagree about what the vocabulary is.

pub mod check;
pub mod expand;
pub mod node_types;
#[cfg(feature = "pack")]
pub mod pack;
pub mod roles;

#[cfg(feature = "pack")]
pub use pack::Pack;

use std::sync::OnceLock;

use serde::Deserialize;

/// One vocabulary term: a name (always underscore-prefixed) and its
/// one-line definition.
#[derive(Debug, Clone, Deserialize)]
pub struct Term {
    pub name: String,
    pub definition: String,
}

/// The closed vocabulary. A grammar may omit terms its language lacks; it
/// may not invent terms.
///
/// `version` is an identity, not a compatibility promise. It is deliberately
/// NOT bumped per change while the vocabulary is still being worked out --
/// a number climbing through 0.4 in a fortnight claims a stability nothing
/// here has yet, and there is no external consumer to claim it to. What
/// actually protects a stale `roles.json` is the structural checking in
/// [`check`], not this string: a removed or renamed term fails rule 1 or
/// rule 5, and a term that moved tier fails the demotion rules. Every
/// breaking change is caught by what the manifest SAYS, not by what it
/// claims to target. Start versioning for real when someone outside this
/// repository depends on it.
#[derive(Debug, Deserialize)]
pub struct Vocabulary {
    pub version: String,
    pub table: Vec<Term>,
    pub facets: Vec<Term>,
    /// Table-tier terms a grammar may deliver as a facet instead, when its
    /// language partitions the position. Demotion must be declared and
    /// justified in the grammar's `roles.json` (`demoted`).
    #[serde(default)]
    pub either_tier: Vec<String>,
    /// Required containments, as (inner, outer): every grammar that
    /// declares both must nest inner inside outer.
    pub containments: Vec<(String, String)>,
}

impl Vocabulary {
    pub fn table_terms(&self) -> impl Iterator<Item = &str> {
        self.table.iter().map(|t| t.name.as_str())
    }

    pub fn facet_terms(&self) -> impl Iterator<Item = &str> {
        self.facets.iter().map(|t| t.name.as_str())
    }

    pub fn is_table_term(&self, name: &str) -> bool {
        self.table.iter().any(|t| t.name == name)
    }

    pub fn is_facet_term(&self, name: &str) -> bool {
        self.facets.iter().any(|t| t.name == name)
    }

    /// Whether a grammar may choose this term's tier for itself.
    pub fn is_either_tier(&self, name: &str) -> bool {
        self.either_tier.iter().any(|t| t == name)
    }
}

/// The vocabulary this build of treebank carries. Parsing the embedded
/// JSON cannot fail for a released crate; the unit tests parse it too, so a
/// malformed edit fails `cargo test` before it can fail a consumer.
pub fn vocabulary() -> &'static Vocabulary {
    static VOCAB: OnceLock<Vocabulary> = OnceLock::new();
    VOCAB.get_or_init(|| {
        serde_json::from_str(include_str!("../vocabulary/vocabulary.json"))
            .expect("embedded vocabulary.json is malformed")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_vocabulary_parses_and_is_closed_and_underscored() {
        let v = vocabulary();
        assert_eq!(v.version, "0.1.0");
        assert_eq!(v.table.len(), 22);
        assert_eq!(v.facets.len(), 7);
        for t in v.table.iter().chain(v.facets.iter()) {
            assert!(t.name.starts_with('_'), "{} must be underscored", t.name);
            assert!(!t.definition.is_empty());
        }
        // No name appears in both tiers.
        for f in &v.facets {
            assert!(!v.is_table_term(&f.name), "{} is in both tiers", f.name);
        }
        // Demotable terms are table-tier terms a grammar may deliver as a
        // facet instead (§3.1.1). They must not be pre-declared facets,
        // and each must be a real table term.
        for name in &v.either_tier {
            assert!(v.is_table_term(name), "{name} is not a table term");
            assert!(!v.is_facet_term(name), "{name} is already a facet term");
        }
        // Containments reference table-tier terms only.
        for (inner, outer) in &v.containments {
            assert!(v.is_table_term(inner), "{inner} not a table term");
            assert!(v.is_table_term(outer), "{outer} not a table term");
        }
    }
}
