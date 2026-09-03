//! The treebank vocabulary (notes/DESIGN.md §3), as code.
//!
//! Every term is delivered one of two ways, and which one is dictated by
//! what tree-sitter's supertype mechanism can express:
//!
//! - **Structural** — a real supertype rule threaded through a grammar's
//!   productions. Membership is decided by structure: the parse went
//!   through it *here*. Enforced at generate time, natively queryable.
//! - **Nominal** — a term that cross-cuts derivations and therefore cannot
//!   be a supertype. Shipped as a `terms.json` manifest per grammar crate,
//!   where membership is decided by name, and expanded into concrete
//!   alternations at query-load time by [`expand`].
//!
//! Which way is a property of the *grammar*, not of the vocabulary: terms
//! listed in `demotable` may be delivered either way, and each grammar
//! picks. Structural is stronger and is the default; a grammar demotes a
//! term to nominal only when its language partitions the position (Python
//! orders parameters, so `_parameter` cannot be one alternation without the
//! grammar accepting `def f(a=1, b)`). Demotion is sound precisely when
//! every member of the term is a concrete node type that occurs nowhere
//! else, since then structural and nominal membership select the same
//! nodes; see notes/DESIGN.md §3.4.
//!
//! The vocabulary itself lives in `vocabulary/vocabulary.json`, embedded
//! here and re-exported to JavaScript by `vocabulary/terms.js`, so the
//! grammars and this crate can never disagree about what the vocabulary is.

pub mod check;
pub mod expand;
#[cfg(feature = "fetch-bytes")]
pub mod fetch;
pub mod node_types;
#[cfg(feature = "pack")]
pub mod pack;
pub mod terms;

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
/// actually protects a stale `terms.json` is the structural checking in
/// [`check`], not this string: a removed or renamed term fails rule 1 or
/// rule 5, and a term that moved tier fails the demotion rules. Every
/// breaking change is caught by what the manifest SAYS, not by what it
/// claims to target. Start versioning for real when someone outside this
/// repository depends on it.
#[derive(Debug, Deserialize)]
pub struct Vocabulary {
    pub version: String,
    pub structural: Vec<Term>,
    pub nominal: Vec<Term>,
    /// Structural terms a grammar may deliver nominally instead, when its
    /// language partitions the position. Demotion must be declared and
    /// justified in the grammar's `terms.json` (`demoted`).
    #[serde(default)]
    pub demotable: Vec<String>,
    /// Required containments, as (inner, outer): every grammar that
    /// declares both must nest inner inside outer.
    pub containments: Vec<(String, String)>,
}

impl Vocabulary {
    pub fn structural_terms(&self) -> impl Iterator<Item = &str> {
        self.structural.iter().map(|t| t.name.as_str())
    }

    pub fn nominal_terms(&self) -> impl Iterator<Item = &str> {
        self.nominal.iter().map(|t| t.name.as_str())
    }

    pub fn is_structural_term(&self, name: &str) -> bool {
        self.structural.iter().any(|t| t.name == name)
    }

    pub fn is_nominal_term(&self, name: &str) -> bool {
        self.nominal.iter().any(|t| t.name == name)
    }

    /// Whether a grammar may choose for itself how to deliver this term.
    pub fn is_demotable(&self, name: &str) -> bool {
        self.demotable.iter().any(|t| t == name)
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
        assert_eq!(v.structural.len(), 22);
        assert_eq!(v.nominal.len(), 7);
        for t in v.structural.iter().chain(v.nominal.iter()) {
            assert!(t.name.starts_with('_'), "{} must be underscored", t.name);
            assert!(!t.definition.is_empty());
        }
        // No name is both structural and nominal.
        for f in &v.nominal {
            assert!(
                !v.is_structural_term(&f.name),
                "{} is both structural and nominal",
                f.name
            );
        }
        // Demotable terms are structural terms a grammar may deliver
        // nominally instead (§3.1.1). They must not be pre-declared
        // nominal, and each must be a real structural term.
        for name in &v.demotable {
            assert!(
                v.is_structural_term(name),
                "{name} is not a structural term"
            );
            assert!(!v.is_nominal_term(name), "{name} is already a nominal term");
        }
        // Containments reference structural terms only.
        for (inner, outer) in &v.containments {
            assert!(v.is_structural_term(inner), "{inner} not a structural term");
            assert!(v.is_structural_term(outer), "{outer} not a structural term");
        }
    }
}
