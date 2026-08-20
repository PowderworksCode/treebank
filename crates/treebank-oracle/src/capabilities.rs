//! What each language's reference toolchain can do BEYOND a verdict.
//!
//! Three of the checks in this repository need more from a reference
//! implementation than "is this file valid": [`crate::spans`] wants node
//! boundaries, [`crate::reformat`] wants a text-to-text formatter,
//! [`crate::unparse`] wants a printer that renders from a tree. No
//! toolchain has all three, several have none, and which ones a language
//! has is not derivable from anything — it is a fact about the tools that
//! exist.
//!
//! It used to be three separate exhaustive `match`es in three files, which
//! meant a new language was three edits and, in practice, three chances to
//! answer one of them by copying the neighbour's answer. It is one `match`
//! now. The exhaustiveness is kept on purpose — the compiler asking a new
//! language for all three answers is the point, and `None` is a real
//! answer as long as it comes with the sentence saying why, because the
//! alternative is a check that silently compares against nothing.
//!
//! What is *not* here is the validity oracle itself: [`crate::get`] is
//! total, because a language with no way to adjudicate its own corpus
//! cannot be swept at all.

use crate::{
    reformat::{self, Reformatter},
    rust_spans,
    spans::{self, SpanOracle},
    unparse::{self, Unparser},
    LangName,
};

/// The optional capabilities of one language's reference toolchain.
pub(crate) struct Capabilities {
    /// Node boundaries, for `treebank shape`.
    pub spans: Option<&'static dyn SpanOracle>,
    /// A formatter, for `treebank reformat`. Whether the tool is installed
    /// is a separate question, asked by [`Reformatter::available`].
    pub reformat: Option<&'static dyn Reformatter>,
    /// A tree printer, for `treebank roundtrip --unparse`.
    pub unparse: Option<&'static dyn Unparser>,
}

static TS_SPANS: spans::TypeScriptSpans = spans::TypeScriptSpans;
static PY_SPANS: spans::PythonSpans = spans::PythonSpans;
static RS_SPANS: rust_spans::RustSpans = rust_spans::RustSpans;
static JAVA_SPANS: spans::JavaSpans = spans::JavaSpans;
static BASH_SPANS: spans::BashSpans = spans::BashSpans;

static RUSTFMT: reformat::RustFmt = reformat::RustFmt;
static BLACK: reformat::BlackFmt = reformat::BlackFmt;

static PY_PRINT: unparse::PythonUnparser = unparse::PythonUnparser;
static TS_PRINT: unparse::TypeScriptUnparser = unparse::TypeScriptUnparser;
static RS_PRINT: unparse::RustUnparser = unparse::RustUnparser;

pub(crate) fn get(name: LangName) -> Capabilities {
    match name {
        LangName::Python => Capabilities {
            spans: Some(&PY_SPANS),
            reformat: Some(&BLACK),
            unparse: Some(&PY_PRINT),
        },

        LangName::Rust => Capabilities {
            spans: Some(&RS_SPANS),
            reformat: Some(&RUSTFMT),
            unparse: Some(&RS_PRINT),
        },

        // One toolchain answers for both: tsc parses `.js` as a dialect,
        // which is the same union the grammar carries.
        LangName::Typescript | LangName::Javascript => Capabilities {
            spans: Some(&TS_SPANS),
            // tsc exposes formatting only through the language service,
            // and prettier is not vendored. Stated rather than faked.
            reformat: None,
            unparse: Some(&TS_PRINT),
        },

        LangName::Java => Capabilities {
            spans: Some(&JAVA_SPANS),
            // google-java-format is the obvious candidate and is not
            // installed; the JDK ships no formatter of its own.
            reformat: None,
            // javac's `Pretty` printer is an internal API behind
            // --add-exports, and it is lossy in ways that would read as
            // our failures. Left out until it is worth the argument.
            unparse: None,
        },

        LangName::Bash => Capabilities {
            spans: Some(&BASH_SPANS),
            // shfmt is the candidate and is not installed.
            reformat: None,
            // bash has no printer; nothing renders a script back from a
            // tree.
            unparse: None,
        },

        LangName::Ruby => Capabilities {
            // CRuby can give one: RubyVM::AbstractSyntaxTree nodes carry
            // first/last line and column, so a span oracle is reachable
            // the same way python's was. Not built yet, and saying so
            // beats a `shape` run that silently compares against nothing.
            spans: None,
            // rubocop and standardrb are gems, not part of the
            // interpreter, and neither is installed. Stated rather than
            // faked.
            reformat: None,
            // CRuby ships no unparser: RubyVM::AbstractSyntaxTree has no
            // printer, and prism's is not in the 3.3 stdlib.
            unparse: None,
        },

        LangName::Zig => Capabilities {
            // `zig fmt` reports a verdict and a diagnostic, not a tree.
            // The compiler can dump one (`std.zig.Ast`, from a small
            // program of our own, the way the java and bash oracles were
            // built), and that is not built yet. Saying so beats a `shape`
            // run that compares against nothing.
            spans: None,
            // `zig fmt` IS a real reformatter, shipped with the compiler.
            // Not wired yet: the reformat gate wants the tool pinned
            // alongside the oracle, and the oracle's own version pin is
            // the thing to settle first.
            reformat: None,
            // `std.zig.render` is what `zig fmt` is built on, so a printer
            // is reachable through the same small program. Not built yet.
            unparse: None,
        },
    }
}
