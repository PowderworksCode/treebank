//! HTML: the first **recovery-by-spec** language in this repo, and the whole
//! job is the oracle.
//!
//! Every other grammar here has a reference parser that says no. HTML's does
//! not, and cannot: the HTML5 parsing algorithm defines a recovery for every
//! byte sequence, so `html5ever` and `parse5` return a document tree for
//! random bytes. A `validate()` built the obvious way returns `true` for
//! everything, every grammar failure becomes a gap, and the gap number stops
//! meaning anything — which is precisely the failure `GRAMMARS.md` forbids.
//!
//! The escape is that the spec separates **parse errors** from parse
//! *failure*. A conforming parser recovers *and* is required to say an error
//! occurred, so there is something to read. Two candidates were stood up and
//! measured on 6,015 real files before anything was vendored (470 GitHub
//! `language:HTML` repositories, ≤30 files each, deduped by content sha1;
//! tree-sitter-html @ 73a3947 fails 1,010 of them, 16.8%):
//!
//! | oracle                                     | rejects | rej│grammar-FAIL | rej│grammar-PASS |
//! |--------------------------------------------|---------|------------------|------------------|
//! | html5ever 0.35, any parse error             |  57.5%  |      76.8%       |      53.5%       |
//! | vnu 26.8.6 (Nu Html Checker), any error     |  80.3%  |      93.8%       |      77.6%       |
//! | **this oracle** (markup-syntax errors only) |   8.6%  |      43.6%       |       1.5%       |
//!
//! **Neither candidate failed for lack of rejection power. Both failed for
//! having far too much of it**, which is the same disaster from the other
//! side: naive vnu calls 947 of the 1,010 grammar failures invalid and leaves
//! a gap queue of 63, and it does that because it rejects 77.6% of the files
//! the grammar parses *perfectly*. A verdict that lands on four fifths of a
//! clean corpus is not measuring parseability.
//!
//! The two disagree exactly as conformance-vs-parseability predicts. vnu's
//! rejects are a strict superset of html5ever's — 3,456 both, 1,373 vnu-only,
//! **zero html5ever-only** — and the 1,373 vnu-only files are pure
//! conformance: a missing `alt`, an obsolete doctype, a skipped heading
//! level, a CSS property typo. None of that is a claim about whether the
//! bytes are markup.
//!
//! **What treebank asks here is parseability, not conformance**, so this
//! oracle is built on the line inside the spec that separates the two: see
//! [`markup_syntax_error`]. An `invalid` verdict from it means *"this is not
//! well-formed markup"*, never *"this is non-conforming HTML"* — a file with
//! no `<!DOCTYPE>`, no `alt`, a stray `</div>` and three obsolete elements is
//! **valid** to this oracle, because a grammar should parse all of it.
//!
//! The residual risk runs the honest way. This oracle under-rejects by
//! construction, and an under-rejection surfaces as a gap file somebody has
//! to look at; an over-rejection would hide a grammar bug in the noise
//! column where nobody would ever see it. Everything ambiguous is therefore
//! resolved towards `valid`. That is also why `test/negative/` carries the
//! weight on this language rather than the sweep.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use html5ever::driver::ParseOpts;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::RcDom;
use rayon::prelude::*;

use super::{github, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Html;

/// Tokenizer states in which the parser is reading **markup syntax** — a tag,
/// a comment or a doctype — rather than text.
///
/// This list is the oracle. html5ever's `bad_char_error` reports the state it
/// was in when it hit a character that state does not allow, so "the
/// tokenizer was inside a tag and met something that cannot appear in a tag"
/// is directly readable, and it is exactly the class a tree-sitter grammar
/// should also refuse. Everything outside this list — every tree-construction
/// error, every character-reference complaint, every character in `Data` — is
/// the spec recovering from *well-formed markup in the wrong shape*, which a
/// grammar must parse rather than reject.
///
/// Measured consequence of drawing the line here rather than at "any parse
/// error": rejection falls from 57.5% of the sample to 8.6%, and the share of
/// files the grammar parses cleanly that get called invalid falls from 53.5%
/// to **1.5%**.
///
/// It is an allowlist and not a denylist on purpose. A future html5ever that
/// adds a state this list has never heard of will under-reject, and an
/// under-rejection becomes a visible gap file; a denylist would over-reject
/// on the same change and quietly bury a grammar bug as noise. The version
/// that produced the ledger's numbers is pinned in `oracle.version`, so the
/// list and the parser cannot drift apart silently.
const MARKUP_STATES: &[&str] = &[
    // tags
    "TagOpen",
    "EndTagOpen",
    "TagName",
    "BeforeAttributeName",
    "AttributeName",
    "AfterAttributeName",
    "BeforeAttributeValue",
    "AfterAttributeValueQuoted",
    "SelfClosingStartTag",
    // comments
    "MarkupDeclarationOpen",
    "CommentStart",
    "CommentStartDash",
    "CommentEnd",
    "CommentEndDash",
    "CommentEndBang",
    "CommentLessThanSignBangDashDash",
    // doctypes
    "Doctype",
    "BeforeDoctypeName",
    "DoctypeName",
    "AfterDoctypeName",
    "AfterDoctypeKeyword(Public)",
    "AfterDoctypeKeyword(System)",
    "BeforeDoctypeIdentifier(Public)",
    "BeforeDoctypeIdentifier(System)",
    "BetweenDoctypePublicAndSystemIdentifiers",
];

/// Errors that are malformed markup regardless of state, named literally
/// because html5ever emits them as fixed strings.
///
/// `Duplicate attribute` is deliberately **not** here, and it is the closest
/// call in the file: `<a href=x href=y>` is non-conforming, but it is
/// perfectly well-formed markup and a grammar has no business rejecting it.
/// Same reasoning excludes `Bad DOCTYPE` (the HTML4 doctypes, 354 files in
/// the sample, well-formed and legacy), `Unacknowledged self-closing tag`
/// (`<div/>`, well-formed), and every character-reference error (`&amp` with
/// no semicolon is text, not markup).
const MALFORMED: &[&str] = &[
    // `</div class="x">` — an end tag cannot carry attributes.
    "Attributes on an end tag",
    // `</div/>`
    "Self-closing end tag",
    // Not UTF-8. The bytes are not text in the document's own encoding, so
    // there is nothing for any grammar to parse.
    "invalid byte sequence",
];

/// Does this parse error say the *markup syntax itself* is malformed?
///
/// `ParseOpts::default()` leaves `exact_errors` off, which is what makes this
/// readable: with it off html5ever formats bad-character and EOF errors as
/// `Saw {c} in state {state:?}` / `Saw EOF in state {state:?}` and the state
/// is the discriminator. Turning `exact_errors` on collapses both to the
/// bare strings "Bad character" / "Unexpected EOF" — losing the state — and
/// additionally emits a "Bad character U+XXXX" for every control code point
/// anywhere in the document, which is a text-level conformance complaint
/// this oracle must not act on. So the *less* exact option is the one that
/// carries the information, and that is worth stating because it looks
/// backwards.
fn markup_syntax_error(msg: &str) -> bool {
    if MALFORMED.contains(&msg) {
        return true;
    }
    // End of file part-way through something: `<span class="a` and nothing
    // more, an unclosed `<!--`, an unclosed `<script>`. Every one of those is
    // truncated markup, so the test is not "which construct" but "was the
    // tokenizer inside one at all".
    //
    // This is the one place the allowlist polarity flips, and deliberately.
    // `Data` and `Plaintext` are the only two states in which reaching the
    // end of the document is *normal*; every other state means an unfinished
    // construct, including any state a future html5ever might add. So here a
    // two-name denylist is the complete and stable rule where an allowlist
    // would need extending forever. Checked against the grammar rather than
    // assumed: tree-sitter-html rejects an unterminated comment, `<script>`,
    // `<style>`, CDATA section and doctype alike, so an oracle that called
    // those valid would report five classes of phantom gap.
    if let Some(state) = msg.strip_prefix("Saw EOF in state ") {
        return !matches!(state, "Data" | "Plaintext");
    }
    let Some(rest) = msg.strip_prefix("Saw ") else {
        return false;
    };
    let Some(i) = rest.find(" in state ") else {
        return false;
    };
    MARKUP_STATES.contains(&&rest[i + " in state ".len()..])
}

/// Template markers that mean a file is a *template that renders to HTML*
/// rather than HTML.
///
/// This is the same problem `treebank-bash` solved for Jinja-in-shell and it
/// is worse here, because HTML's templating ecosystem is enormous and every
/// engine reuses the `.html` extension: Jinja, Django, Twig, Liquid,
/// Handlebars, Mustache, Go templates, ERB, EJS, Blade, Svelte. Measured on
/// the 6,015-file sample, **1,219 files (20.3%) carry one of these**, and
/// tree-sitter-html fails 28.6% of them against 13.8% of everything else —
/// so left in, templating would be a third of the gap queue and none of it
/// would be a grammar bug.
///
/// Detection is content-based and not path-based, for the reason
/// `treebank-go` gives for build tags: nothing in `templates/index.html`
/// distinguishes it from `docs/index.html`, and plenty of Jinja lives
/// outside a directory called `templates`.
///
/// The delimiters are matched **without** a keyword check, which is where
/// this deliberately diverges from bash's rule. Bash needs `{% if %}` rather
/// than bare `{%` because `printf '{%s%}'` is real shell; in HTML a `{%` or
/// `{{` in document text is not idiomatic and the cost of the loose rule is
/// paid in the safe direction — a wrongly excluded file is one file missing
/// from the corpus, where a wrongly included one is a permanent phantom gap.
const TEMPLATE_MARKERS: &[&[u8]] = &[
    b"{%",     // Jinja, Django, Twig, Liquid, Nunjucks
    b"{{",     // Handlebars, Mustache, Go text/template, Vue, Angular
    b"<%",     // ERB, EJS, ASP, JSP
    b"<?php",  // PHP
    b"<?=",    // PHP short echo
    b"{#if",   // Svelte
    b"{#each", // Svelte
    b"@section", // Blade
    b"@extends", // Blade
    b"@yield",   // Blade
];

impl Lang for Html {
    fn name(&self) -> LangName {
        LangName::Html
    }

    /// GitHub repositories that GitHub classifies as HTML, by stars — the
    /// artifact-corpus path `treebank-bash` built, used here for the same
    /// reason: HTML has no package registry, so there is no download count
    /// to rank by and no release to pin.
    ///
    /// **Why repository HTML and not the live web.** Both populations were
    /// available (there is a Common Crawl store on the build machine) and
    /// they are not the same language: web HTML is overwhelmingly
    /// machine-generated, minified tag soup, while repository HTML is closer
    /// to what a developer edits. treebank is a tool about source code, and a
    /// gap queue built from minified crawl output would be a queue of
    /// minifier artefacts. That is the choice; what it biases toward is
    /// stated in `lang::github` and in `ledger.json`'s `corpus`.
    ///
    /// One bias worth checking rather than assuming, because it is the
    /// obvious objection to this source: `language:HTML` selects repositories
    /// that are *mostly* HTML, which sounds like it would select generated
    /// documentation trees. Measured on the sample, only 5.2% of files carry
    /// a `<meta name="generator">` (gitbook, doxygen, docusaurus, jsdoc), and
    /// the grammar fails **14.9%** of those against 16.9% of the rest — so
    /// generated output is neither a large share nor a distorting one, and no
    /// generator filter is applied.
    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        github::rank(LangName::Html, "HTML", k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        github::resolve(LangName::Html, pkg)
    }

    /// `.html` only — the single extension `tree-sitter-html`'s own
    /// `tree-sitter.json` claims under `file-types`, following the rule ruby,
    /// lua, python and javascript all follow: `classify()` matches what the
    /// grammar advertises, and widening it is a deliberate change with its
    /// own sweep evidence rather than a silent one. `.htm`, `.xhtml` and the
    /// server-page extensions Helix lists (`.jsp`, `.asp`, `.aspx`, `.rhtml`,
    /// `.cshtml`) are therefore out; the last five are template languages in
    /// any case, which the next filter would drop.
    ///
    /// `node_modules/` and `vendor/` are excluded for the reason python
    /// excludes `_vendor/` and ruby excludes `vendor/`: a repository that
    /// vendors a dependency ships somebody else's source, so a failure there
    /// is attributed to the wrong project.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        if rel
            .components()
            .any(|c| matches!(c.as_os_str().to_str(), Some("node_modules" | "vendor")))
        {
            return None;
        }
        (rel.extension()?.to_str()? == "html").then_some(None)
    }

    /// A template that renders to HTML is not HTML. See [`TEMPLATE_MARKERS`]
    /// for the measurement that makes this the largest single correction to
    /// the corpus.
    fn admit(&self, _rel: &Path, content: &[u8]) -> bool {
        !TEMPLATE_MARKERS
            .iter()
            .any(|m| content.windows(m.len()).any(|w| w == *m))
    }

    /// 250 MB, the same cap and the same reasoning as bash: an artifact from
    /// a repository has no size discipline, and the largest `language:HTML`
    /// repositories are course-note and textbook dumps whose bulk is PDFs and
    /// images rather than markup. Measured while building the oracle sample,
    /// 112 of the top 600 exceed 150 MB and the biggest single download ran
    /// past 400 MB for a handful of `.html` files. Every skip is logged by
    /// the fetch driver and the cap is recorded in `ledger.json` next to the
    /// repository count it produced.
    fn max_artifact_bytes(&self) -> Option<u64> {
        Some(250_000_000)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// html5ever's spec parse errors, filtered to the ones that say the
    /// markup syntax itself is malformed. See the module header for the
    /// measurement behind that filter and for what an `invalid` verdict here
    /// does and does not mean.
    ///
    /// In-process rather than a script under `tools/`, like rust's `syn`
    /// oracle and for the same reason: the reference parser is a Rust
    /// library, so there is no interpreter to start and no line protocol to
    /// speak. 6,015 files in 13.8 s single-threaded, and this runs them in
    /// parallel.
    ///
    /// **An unreadable file is not an invalid file.** This errors out rather
    /// than returning a verdict, so a mistyped corpus root fails the sweep
    /// instead of scoring every grammar failure as noise and reporting a
    /// flawless grammar.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        paths
            .par_iter()
            .map(|p| {
                let full = srcroot.join(p);
                let bytes = std::fs::read(&full)
                    .with_context(|| format!("html oracle: read {}", full.display()))?;
                Ok((p.clone(), is_well_formed_markup(&bytes)))
            })
            .collect()
    }
}

/// Parse `bytes` as a document and report whether html5ever saw any
/// markup-syntax error. The tree is thrown away; only the error list matters.
fn is_well_formed_markup(bytes: &[u8]) -> bool {
    let dom = html5ever::parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .one(bytes);
    let errors = dom.errors.borrow();
    !errors.iter().any(|e| markup_syntax_error(e.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The battery `GRAMMARS.md` demands of any oracle that is not simply
    /// "the language's own compiler said no": agreement on clean files is
    /// worth nothing, only files that *should* be rejected test it. Each case
    /// below is one class of malformed markup, and each was verified to be
    /// the class it claims by running html5ever on it.
    #[test]
    fn malformed_markup_is_rejected() {
        for src in [
            // a `<` that opens nothing, mid-text
            &b"<p>a < b</p>"[..],
            // junk after `</`
            b"<p>x</ 3>",
            // an attribute value butted against the next attribute
            b"<input type=\"text\" placeholder=\"Username\"require>",
            // `=` where an attribute name must start
            b"<div ==\"x\">y</div>",
            // a quote inside an attribute NAME
            b"<a hre\"f=\"/x\">y</a>",
            // `/` not followed by `>`
            b"<img src=\"a.png\" / class=\"b\">",
            // attributes on an end tag
            b"<div>x</div class=\"y\">",
            // a self-closing end tag
            b"<div>x</div/>",
            // truncated inside a tag
            b"<p>hello<span class=\"a",
            // `<!` followed by junk that is not a comment or doctype
            b"<! doctype html>",
            // not UTF-8
            b"<p>\xff\xfe\xfa</p>",
        ] {
            assert!(
                !is_well_formed_markup(src),
                "should be rejected: {}",
                String::from_utf8_lossy(src)
            );
        }
    }

    /// The other half, and the half that decides whether the oracle is
    /// usable: everything the HTML5 spec recovers from is **valid** here.
    /// Each of these is a spec parse error that a naive oracle would reject,
    /// and every one of them is well-formed markup a grammar must parse.
    #[test]
    fn spec_recovery_is_not_invalidity() {
        for src in [
            // a bare fragment — no doctype, no <html>, no <head>. This is the
            // single largest class in real repositories: 2,280 files of the
            // 6,015-file sample, and calling it invalid was most of why a
            // naive html5ever oracle rejects 57.5% of a clean corpus.
            &b"<p>just a fragment</p>"[..],
            // mis-nested tags — the textbook parse error
            b"<p>a<div>b</p></div>",
            // a stray end tag with nothing to close
            b"<div>x</div></span>",
            // an unclosed element at EOF
            b"<div><p>x",
            // an obsolete but well-formed HTML4 doctype
            b"<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\"><html><body>x</body></html>",
            // duplicate attributes
            b"<a href=\"/x\" href=\"/y\">z</a>",
            // `<div/>`, which HTML does not have but which parses
            b"<div/>text",
            // an unquoted attribute value
            b"<div class=box>x</div>",
            // a character reference with no semicolon
            b"<p>Tom &amp Jerry &copy 2026</p>",
            // an unknown element
            b"<my-widget data-x=\"1\">hi</my-widget>",
            // text before <html>, and content after </html>
            b"lead<html><body>x</body></html>trail",
            // a completely empty file
            b"",
        ] {
            assert!(
                is_well_formed_markup(src),
                "should be valid: {}",
                String::from_utf8_lossy(src)
            );
        }
    }

    #[test]
    fn templates_that_render_to_html_are_not_html() {
        let h = Html;
        let p = Path::new("index.html");
        assert!(!h.admit(p, b"<html>{% for x in y %}<p>{{ x }}</p>{% endfor %}</html>"));
        assert!(!h.admit(p, b"<ul>{{#each items}}<li>{{this}}</li>{{/each}}</ul>"));
        assert!(!h.admit(p, b"<p><%= @user.name %></p>"));
        assert!(!h.admit(p, b"<p><?php echo $x; ?></p>"));
        assert!(!h.admit(p, b"@extends('layout')\n@section('body')<p>x</p>"));
        assert!(!h.admit(p, b"{#if ok}<p>yes</p>{/if}"));
        assert!(h.admit(p, b"<!DOCTYPE html><html><body><p>plain</p></body></html>"));
    }

    #[test]
    fn classify_takes_the_extension_the_grammar_claims() {
        let h = Html;
        assert!(h.classify(Path::new("index.html")).is_some());
        assert!(h.classify(Path::new("docs/a/b.html")).is_some());
        // not claimed by tree-sitter.json
        assert!(h.classify(Path::new("index.htm")).is_none());
        assert!(h.classify(Path::new("a.xhtml")).is_none());
        assert!(h.classify(Path::new("style.css")).is_none());
        // somebody else's source
        assert!(h.classify(Path::new("node_modules/x/index.html")).is_none());
        assert!(h.classify(Path::new("vendor/y/index.html")).is_none());
    }
}
