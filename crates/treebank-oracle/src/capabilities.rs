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

/// Configuration-only capability flags. Unlike `reformatter_for`, this does
/// not probe whether an optional executable such as Black is installed on the
/// current machine; it answers what the registry says the language supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityFlags {
    pub spans: bool,
    pub formatter: bool,
    pub printer: bool,
}

pub fn configured(name: LangName) -> CapabilityFlags {
    let capabilities = get(name);
    CapabilityFlags {
        spans: capabilities.spans.is_some(),
        formatter: capabilities.reformat.is_some(),
        printer: capabilities.unparse.is_some(),
    }
}

static TS_SPANS: spans::TypeScriptSpans = spans::TypeScriptSpans;
static PY_SPANS: spans::PythonSpans = spans::PythonSpans;
static RS_SPANS: rust_spans::RustSpans = rust_spans::RustSpans;
static JAVA_SPANS: spans::JavaSpans = spans::JavaSpans;
static BASH_SPANS: spans::BashSpans = spans::BashSpans;
static RB_SPANS: spans::RubySpans = spans::RubySpans;
static C_SPANS: spans::CSpans = spans::CSpans;
static CPP_SPANS: spans::CppSpans = spans::CppSpans;
static YAML_SPANS: spans::YamlSpans = spans::YamlSpans;

static RUSTFMT: reformat::RustFmt = reformat::RustFmt;
static BLACK: reformat::BlackFmt = reformat::BlackFmt;
static TS_FORMAT: reformat::TypeScriptFmt = reformat::TypeScriptFmt;
static ZIG_FORMAT: reformat::ZigFmt = reformat::ZigFmt;

static PY_PRINT: unparse::PythonUnparser = unparse::PythonUnparser;
static TS_PRINT: unparse::TypeScriptUnparser = unparse::TypeScriptUnparser;
static RS_PRINT: unparse::RustUnparser = unparse::RustUnparser;

pub(crate) fn get(name: LangName) -> Capabilities {
    // `NONE` is recorded explicitly in the central registry. It means the
    // reference toolchain has no implementation wired for that capability;
    // adding one is a single registry-field change once its adapter exists.
    macro_rules! capability {
        (NONE) => {
            None
        };
        ($value:ident) => {
            Some(&$value)
        };
    }
    macro_rules! capability_match {
        ($( $variant:ident => $name:literal, $exts:tt, $grammar:ident, $rosetta:literal,
             $ecosystem:path, $oracle:path, ($spans:ident, $reformat:ident, $unparse:ident); )+) => {
            match name {
                $(LangName::$variant => Capabilities {
                    spans: capability!($spans),
                    reformat: capability!($reformat),
                    unparse: capability!($unparse),
                },)+
            }
        };
    }
    treebank_lang::for_each_language!(capability_match)
}

#[cfg(test)]
mod tests {
    use super::{configured, get, LangName};

    #[test]
    fn configured_capabilities_do_not_depend_on_local_executables() {
        let python = configured(LangName::Python);
        assert!(python.spans);
        assert!(python.formatter);
        assert!(python.printer);
    }
    #[test]
    fn formatter_capabilities_are_registered_for_supported_toolchains() {
        for lang in [
            LangName::Python,
            LangName::Rust,
            LangName::Typescript,
            LangName::Javascript,
            LangName::Zig,
        ] {
            assert!(get(lang).reformat.is_some(), "missing formatter for {lang}");
        }
    }

    #[test]
    fn c_family_span_capabilities_are_registered() {
        assert!(get(LangName::C).spans.is_some());
        assert!(get(LangName::Cpp).spans.is_some());
    }
}
