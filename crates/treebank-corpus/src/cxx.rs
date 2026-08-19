use std::path::Path;

use anyhow::Result;
use crate::rank::RankedCrate;
use crate::{debian, Ecosystem};
use treebank_lang::LangName;

pub struct Cxx;

/// The mirror image of [`crate::c`]'s filter, and deliberately written as
/// one: **more C++ than C**, so that a package lands in the C corpus or the
/// C++ one and never in both. Without that second half, glibc (135k lines
/// of C, a few hundred of C++) would enter here on its fringe exactly as
/// LibreOffice would enter the C corpus on its.
fn is_cxx(s: &debian::Sloc) -> bool {
    s.lines("cpp") >= 2000 && s.lines("cpp") > s.lines("ansic")
}

/// Extensions that are C++ and nothing else. `.h` is deliberately absent:
/// it belongs to both languages and is decided by content, below.
const CXX_EXTENSIONS: [&str; 11] = [
    "cc", "cpp", "cxx", "c++", "hpp", "hh", "hxx", "h++", "ipp", "tcc", "inl",
];

/// Markers that make an ambiguous `.h` a C++ header. Only **unguarded**
/// C++ counts: a great many C headers carry C++ sections behind `#ifdef
/// __cplusplus` — glibc's `math.h` has `extern "C++" { template <class __T>
/// … }` — and those are C headers.
const CXX_MARKERS: [&str; 9] = [
    "namespace ",
    "template<",
    "template <",
    "class ",
    "public:",
    "private:",
    "protected:",
    "using namespace ",
    "extern \"C++\"",
];

impl Ecosystem for Cxx {
    fn name(&self) -> LangName {
        LangName::Cpp
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        debian::rank(LangName::Cpp, db, k, "C++", &is_cxx)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        debian::resolve(LangName::Cpp, pkg)
    }

    /// The unambiguous C++ extensions, plus `.h` as a candidate that
    /// `admit` decides on its content. The split is forced by the trait:
    /// `classify` sees only a path, and for a `.h` the answer is not in the
    /// path.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        let ext = rel.extension()?.to_str()?;
        (CXX_EXTENSIONS.contains(&ext) || ext == "h").then_some(None)
    }

    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        if rel.extension().and_then(|e| e.to_str()) != Some("h") {
            return true;
        }
        header_is_cxx(rel, content)
    }

    fn max_artifact_bytes(&self) -> Option<u64> {
        Some(250_000_000)
    }
}

/// Is this `.h` a C++ header rather than a C one?
///
/// Deliberately blunt: a corpus boundary, not a language detector. It is
/// shared by both ecosystems so that the two corpora partition the `.h`
/// files between them instead of each answering the question its own way.
pub fn header_is_cxx(rel: &Path, content: &[u8]) -> bool {
    // Directory naming is the cheapest signal and the most reliable one.
    let dir = rel
        .parent()
        .map(|p| p.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if dir
        .split('/')
        .any(|c| matches!(c, "c++" | "cxx" | "cpp" | "include-c++"))
    {
        return true;
    }
    let raw = String::from_utf8_lossy(&content[..content.len().min(200_000)]);
    let text = strip_comments_and_strings(&raw);
    // one entry per open conditional: does it mention __cplusplus?
    let mut guards: Vec<bool> = Vec::new();
    for line in text.lines() {
        let l = line.trim_start();
        if let Some(directive) = l.strip_prefix('#') {
            let d = directive.trim_start();
            match d.split_whitespace().next().unwrap_or("") {
                "if" | "ifdef" | "ifndef" => guards.push(d.contains("__cplusplus")),
                "else" | "elif" => {
                    if let Some(top) = guards.last_mut() {
                        *top = *top || d.contains("__cplusplus");
                    }
                }
                "endif" => {
                    guards.pop();
                }
                _ => {}
            }
            continue;
        }
        if guards.iter().any(|g| *g) {
            continue;
        }
        if CXX_MARKERS.iter().any(|m| l.starts_with(m)) {
            return true;
        }
    }
    false
}

/// Comments and string literals blanked, newlines preserved so that line
/// starts still mean something. Both exclusions were measured needs rather
/// than hygiene: a first version scanned raw text and called
/// `glibc/elf/elf.h` C++ over the words "class declaration." at the end of
/// a block comment, and `malloc/obstack.h` over "namespace with
/// <stddef.h>'s symbols" on a GNU comment continuation line, which carries
/// no `*` prefix to skip on.
fn strip_comments_and_strings(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"/*") {
            let end = text[i + 2..]
                .find("*/")
                .map(|j| i + 2 + j + 2)
                .unwrap_or(b.len());
            out.extend(text[i..end].chars().filter(|c| *c == '\n'));
            i = end;
        } else if b[i..].starts_with(b"//") {
            i = text[i..].find('\n').map(|j| i + j).unwrap_or(b.len());
        } else if b[i] == b'"' || b[i] == b'\'' {
            let quote = b[i];
            out.push(' ');
            i += 1;
            while i < b.len() && b[i] != quote {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c::is_c;

    #[test]
    fn a_cplusplus_guarded_section_does_not_make_a_header_cxx() {
        let src = br#"
#ifdef __cplusplus
extern "C" {
#endif
int f(void);
#ifdef __cplusplus
}
#endif
"#;
        assert!(!header_is_cxx(Path::new("foo/bar.h"), src));
    }

    #[test]
    fn unguarded_cxx_makes_a_header_cxx() {
        assert!(header_is_cxx(
            Path::new("a/b.h"),
            b"namespace ns {\nclass X {};\n}\n"
        ));
        assert!(header_is_cxx(Path::new("ncurses/c++/cursesw.h"), b"int x;\n"));
    }

    #[test]
    fn prose_about_cxx_does_not_count() {
        // Both of these are real glibc headers that a raw text scan misread.
        assert!(!header_is_cxx(
            Path::new("elf/elf.h"),
            b"/* A class declaration. */\nint x;\n"
        ));
        assert!(!header_is_cxx(
            Path::new("malloc/obstack.h"),
            b"/* pollutes the\n   namespace with <stddef.h>'s symbols.  */\nint x;\n"
        ));
    }

    #[test]
    fn the_two_filters_partition_a_package() {
        // A package cannot satisfy both: one wants ansic >= cpp, the other
        // wants cpp > ansic.
        for (ansic, cpp) in [(5000i64, 100i64), (100, 5000), (3000, 3000)] {
            let s = debian::Sloc {
                version: "1".into(),
                langs: [("ansic".to_string(), ansic), ("cpp".to_string(), cpp)]
                    .into_iter()
                    .collect(),
            };
            assert!(!(is_c(&s) && is_cxx(&s)), "ansic={ansic} cpp={cpp}");
        }
    }
}
