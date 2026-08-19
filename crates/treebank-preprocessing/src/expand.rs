//! Macro expansion — for **diagnosis**, not for excusing a gap.
//!
//! The distinction matters and it is the opposite of the one [`crate::reduce`]
//! makes. A brace split across `#ifdef`s has no well-formed tree at all, so
//! those failures are not grammar bugs. But
//!
//! ```c
//! THREAD_LOCAL int adjustment = 0;
//! list_for_each(li, &q->ifaces) { ... }
//! ```
//!
//! *could* be parsed by a grammar — editors want exactly that — so these
//! remain real gaps. What expansion adds is the answer to "which macro, and
//! what does it expand to", which is precisely what someone writing a minimal
//! grammar rule needs, and what they need in order to judge whether the rule
//! over-accepts.
//!
//! So nothing here changes a verdict. It annotates.
//!
//! # What is modelled, and what is refused
//!
//! Object-like and function-like macros, with argument substitution,
//! rescanning, and the "blue paint" rule that a macro never expands inside
//! its own expansion. Directive lines are never expanded, and neither are
//! comments or string literals.
//!
//! Everything else is **refused by name rather than guessed**: stringify
//! (`#`), token pasting (`##`), variadic macros, argument-count mismatches,
//! and anything nested deeper than [`MAX_DEPTH`]. A refused macro is left in
//! the source exactly as written and its name is reported. That direction is
//! deliberate: expansion *fabricates* text, so a wrong expansion invents a
//! parse success, which is the one failure mode that would make these numbers
//! lie.

use std::collections::{BTreeSet, HashMap, HashSet};

/// Deep enough for real macro chains, shallow enough that a pathological or
/// mutually-recursive definition is refused rather than run forever.
pub const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone)]
pub struct Macro {
    pub name: String,
    /// `None` for object-like macros.
    pub params: Option<Vec<String>>,
    pub body: String,
}

/// Names that must never be expanded even when a package defines them.
///
/// Real packages ship compatibility shims — `#define const`, `#define void`,
/// `#define __attribute__(x)` — guarded by conditionals for compilers we are
/// not. A whole-package macro census cannot tell those are dead, and expanding
/// one *deletes a keyword from live code*, which is how an expander invents a
/// parse success. Found by inspecting what the expander claimed to have
/// expanded: `void`, `const` and `__attribute__` appeared as "macros" at
/// hundreds of error sites.
const NEVER_EXPAND: &[&str] = &[
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "_Bool",
    "_Complex",
    "_Noreturn",
    // GNU spellings the grammar already models itself
    "__attribute__",
    "__extension__",
    "__inline",
    "__inline__",
    "__const",
    "__signed",
    "__volatile",
    "__volatile__",
    "__restrict",
    "__restrict__",
    "__asm",
    "__asm__",
    "__typeof",
    "__typeof__",
];

impl Macro {
    /// Constructs this expander does not model. Checked once at definition
    /// time so a refusal is reported against the macro, not the call site.
    fn unsupported(&self) -> Option<&'static str> {
        if NEVER_EXPAND.contains(&self.name.as_str()) {
            return Some("shadows a keyword the grammar models itself");
        }
        if self.body.contains("##") {
            return Some("token pasting");
        }
        if self.params.is_some() && has_stringify(&self.body) {
            return Some("stringify");
        }
        if self
            .params
            .as_ref()
            .is_some_and(|p| p.iter().any(|x| x == "..."))
        {
            return Some("variadic");
        }
        None
    }
}

/// `#` used as the stringify operator, as opposed to appearing in a string.
fn has_stringify(body: &str) -> bool {
    let b = body.as_bytes();
    (0..b.len()).any(|i| b[i] == b'#' && (i + 1 >= b.len() || b[i + 1] != b'#'))
}

/// Every macro definition in scope, collected from source text.
#[derive(Debug, Default, Clone)]
pub struct Macros {
    map: HashMap<String, Macro>,
}

impl Macros {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&Macro> {
        self.map.get(name)
    }

    /// Add every `#define` in a source file. Later definitions win, which
    /// matches a compiler seeing a redefinition, and `#undef` removes.
    ///
    /// This is a whole-package census, not a per-file view: it deliberately
    /// ignores which header actually reached which file, because the question
    /// being answered is "what does this identifier mean in this package",
    /// not "what would the compiler have seen".
    pub fn add_source(&mut self, source: &str) {
        for line in logical_lines(source) {
            let t = line.trim_start();
            let Some(rest) = t.strip_prefix('#') else {
                continue;
            };
            let rest = rest.trim_start();
            if let Some(def) = rest.strip_prefix("define") {
                if let Some(m) = parse_define(def) {
                    self.map.insert(m.name.clone(), m);
                }
            } else if let Some(undef) = rest.strip_prefix("undef") {
                if let Some(name) = undef.split_whitespace().next() {
                    self.map.remove(name);
                }
            }
        }
    }
}

/// Physical lines joined across backslash continuations.
fn logical_lines(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in source.split('\n') {
        let trimmed = line.strip_suffix('\r').unwrap_or(line);
        if let Some(head) = trimmed.strip_suffix('\\') {
            current.push_str(head);
            current.push(' ');
        } else {
            current.push_str(trimmed);
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn parse_define(def: &str) -> Option<Macro> {
    let def = def.strip_prefix(|c: char| c.is_whitespace())?;
    let def = def.trim_start();
    let name_end = def.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let (name, rest) = def.split_at(name_end);
    if name.is_empty() {
        return None;
    }
    // A parameter list only counts when the `(` is flush against the name;
    // `#define FOO (1+2)` is object-like.
    if let Some(inner) = rest.strip_prefix('(') {
        let close = inner.find(')')?;
        let params: Vec<String> = inner[..close]
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        return Some(Macro {
            name: name.to_string(),
            params: Some(params),
            body: inner[close + 1..].trim().to_string(),
        });
    }
    Some(Macro {
        name: name.to_string(),
        params: None,
        body: rest.trim().to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ident,
    Space,
    Other,
}

#[derive(Debug, Clone)]
struct Tok {
    text: String,
    kind: Kind,
    /// 1-based line in the ORIGINAL source. Body tokens inherit the line of
    /// the invocation that introduced them, so an expansion can always be
    /// attributed back to where the programmer wrote it.
    line: usize,
    /// Macros already expanded to produce this token; none of them may expand
    /// again here. This is what stops `#define foo foo` looping.
    hide: HashSet<String>,
}

/// The result of expanding one file.
#[derive(Debug, Clone)]
pub struct Expansion {
    pub text: String,
    /// `(macro name, line in the original source)`, in expansion order.
    pub expanded: Vec<(String, usize)>,
    /// Macros left unexpanded, with the reason.
    pub refused: BTreeSet<(String, &'static str)>,
}

impl Expansion {
    pub fn changed(&self) -> bool {
        !self.expanded.is_empty()
    }

    /// Macros expanded on or immediately above a given line — the ones that
    /// could plausibly explain a parse error reported there.
    pub fn near(&self, line: usize) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .expanded
            .iter()
            .filter(|(_, l)| *l == line || (*l + 1 == line) || (line + 1 == *l))
            .map(|(n, _)| n.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

/// Expand every macro invocation this expander models, leaving the rest
/// exactly as written.
///
/// Line numbering is **not** preserved — expansion changes token counts by
/// nature — so the result is suitable for asking "does this parse now?" and
/// not for reporting positions. [`Expansion::expanded`] carries the original
/// lines instead.
pub fn expand(source: &str, macros: &Macros) -> Expansion {
    let mut out = String::with_capacity(source.len());
    let mut expanded = Vec::new();
    let mut refused = BTreeSet::new();

    for segment in segments(source) {
        match segment {
            Segment::Directive(text) | Segment::Verbatim(text) => out.push_str(text),
            Segment::Code(text, start_line) => {
                let toks = tokenize(text, start_line);
                let done = expand_tokens(toks, macros, 0, &mut expanded, &mut refused);
                for t in done {
                    out.push_str(&t.text);
                }
            }
        }
    }
    Expansion {
        text: out,
        expanded,
        refused,
    }
}

enum Segment<'a> {
    /// A preprocessor directive line: never expanded.
    Directive(&'a str),
    /// A comment or string literal: never expanded.
    Verbatim(&'a str),
    Code(&'a str, usize),
}

/// Split source into directive lines, comments/strings, and ordinary code.
fn segments(source: &str) -> Vec<Segment<'_>> {
    let b = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1usize;
    let mut code_start = 0usize;
    let mut code_line = 1usize;
    let mut at_line_start = true;

    fn flush<'s>(out: &mut Vec<Segment<'s>>, from: usize, to: usize, l: usize, src: &'s str) {
        if to > from {
            out.push(Segment::Code(&src[from..to], l));
        }
    }

    while i < b.len() {
        let c = b[i];
        if at_line_start && (c == b'#' || (c as char).is_whitespace() && c != b'\n') {
            // Look ahead: is this a directive line?
            let mut j = i;
            while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
                j += 1;
            }
            if j < b.len() && b[j] == b'#' {
                flush(&mut out, code_start, i, code_line, source);
                let start = i;
                // consume the directive, honouring backslash continuations
                loop {
                    while j < b.len() && b[j] != b'\n' {
                        j += 1;
                    }
                    let cont = j > 0 && b[j - 1] == b'\\';
                    if j < b.len() {
                        j += 1;
                        line += 1;
                    }
                    if !cont || j >= b.len() {
                        break;
                    }
                }
                out.push(Segment::Directive(&source[start..j]));
                i = j;
                code_start = i;
                code_line = line;
                at_line_start = true;
                continue;
            }
        }
        if c == b'/' && i + 1 < b.len() && (b[i + 1] == b'/' || b[i + 1] == b'*') {
            flush(&mut out, code_start, i, code_line, source);
            let start = i;
            if b[i + 1] == b'/' {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            } else {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    if b[i] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                }
                i = (i + 2).min(b.len());
            }
            out.push(Segment::Verbatim(&source[start..i]));
            code_start = i;
            code_line = line;
            at_line_start = false;
            continue;
        }
        if c == b'"' || c == b'\'' {
            flush(&mut out, code_start, i, code_line, source);
            let quote = c;
            let start = i;
            i += 1;
            while i < b.len() && b[i] != quote {
                if b[i] == b'\\' {
                    i += 1;
                }
                if i < b.len() && b[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            out.push(Segment::Verbatim(&source[start..i]));
            code_start = i;
            code_line = line;
            at_line_start = false;
            continue;
        }
        if c == b'\n' {
            line += 1;
            at_line_start = true;
        } else if !(c as char).is_whitespace() {
            at_line_start = false;
        }
        i += 1;
    }
    flush(&mut out, code_start, b.len(), code_line, source);
    out
}

fn tokenize(text: &str, start_line: usize) -> Vec<Tok> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut line = start_line;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphabetic() || c == '_' {
            let s = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok {
                text: chars[s..i].iter().collect(),
                kind: Kind::Ident,
                line,
                hide: HashSet::new(),
            });
        } else if c.is_whitespace() {
            let s = i;
            while i < chars.len() && chars[i].is_whitespace() {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            out.push(Tok {
                text: chars[s..i].iter().collect(),
                kind: Kind::Space,
                line,
                hide: HashSet::new(),
            });
        } else {
            out.push(Tok {
                text: c.to_string(),
                kind: Kind::Other,
                line,
                hide: HashSet::new(),
            });
            i += 1;
        }
    }
    out
}

fn expand_tokens(
    input: Vec<Tok>,
    macros: &Macros,
    depth: usize,
    expanded: &mut Vec<(String, usize)>,
    refused: &mut BTreeSet<(String, &'static str)>,
) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let tok = &input[i];
        if tok.kind != Kind::Ident || tok.hide.contains(&tok.text) {
            out.push(tok.clone());
            i += 1;
            continue;
        }
        let Some(mac) = macros.get(&tok.text) else {
            out.push(tok.clone());
            i += 1;
            continue;
        };
        if let Some(why) = mac.unsupported() {
            refused.insert((mac.name.clone(), why));
            out.push(tok.clone());
            i += 1;
            continue;
        }
        if depth >= MAX_DEPTH {
            refused.insert((mac.name.clone(), "expansion too deep"));
            out.push(tok.clone());
            i += 1;
            continue;
        }

        match &mac.params {
            None => {
                let mut hide = tok.hide.clone();
                hide.insert(mac.name.clone());
                let body = with_hide(tokenize(&mac.body, tok.line), &hide);
                expanded.push((mac.name.clone(), tok.line));
                let done = expand_tokens(body, macros, depth + 1, expanded, refused);
                out.extend(done);
                i += 1;
            }
            Some(params) => {
                let Some((args, end)) = collect_args(&input, i + 1) else {
                    out.push(tok.clone()); // a bare mention, not an invocation
                    i += 1;
                    continue;
                };
                if args.len() != params.len() {
                    refused.insert((mac.name.clone(), "argument count mismatch"));
                    out.push(tok.clone());
                    i += 1;
                    continue;
                }
                let mut hide = tok.hide.clone();
                hide.insert(mac.name.clone());
                let substituted = substitute(&mac.body, params, &args, tok.line, &hide);
                expanded.push((mac.name.clone(), tok.line));
                let done = expand_tokens(substituted, macros, depth + 1, expanded, refused);
                out.extend(done);
                i = end;
            }
        }
    }
    out
}

fn with_hide(mut toks: Vec<Tok>, hide: &HashSet<String>) -> Vec<Tok> {
    for t in &mut toks {
        t.hide.extend(hide.iter().cloned());
    }
    toks
}

/// Arguments of an invocation starting at `from`, which must be `(` after
/// optional whitespace. Returns the argument token lists and the index just
/// past the closing paren.
fn collect_args(input: &[Tok], from: usize) -> Option<(Vec<Vec<Tok>>, usize)> {
    let mut i = from;
    while i < input.len() && input[i].kind == Kind::Space {
        i += 1;
    }
    if input.get(i).map(|t| t.text.as_str()) != Some("(") {
        return None;
    }
    i += 1;
    let mut depth = 1usize;
    let mut args: Vec<Vec<Tok>> = vec![Vec::new()];
    while i < input.len() {
        let t = &input[i];
        match t.text.as_str() {
            "(" if t.kind == Kind::Other => {
                depth += 1;
                args.last_mut()?.push(t.clone());
            }
            ")" if t.kind == Kind::Other => {
                depth -= 1;
                if depth == 0 {
                    // `FOO()` with one empty argument is zero arguments.
                    if args.len() == 1 && args[0].iter().all(|t| t.kind == Kind::Space) {
                        args.clear();
                    }
                    // Whitespace around an argument is not part of it, so
                    // `f(a, b)` substitutes `b` rather than ` b`.
                    for arg in &mut args {
                        while arg.first().is_some_and(|t| t.kind == Kind::Space) {
                            arg.remove(0);
                        }
                        while arg.last().is_some_and(|t| t.kind == Kind::Space) {
                            arg.pop();
                        }
                    }
                    return Some((args, i + 1));
                }
                args.last_mut()?.push(t.clone());
            }
            "," if t.kind == Kind::Other && depth == 1 => args.push(Vec::new()),
            _ => args.last_mut()?.push(t.clone()),
        }
        i += 1;
    }
    None // unbalanced: not an invocation we can trust
}

fn substitute(
    body: &str,
    params: &[String],
    args: &[Vec<Tok>],
    line: usize,
    hide: &HashSet<String>,
) -> Vec<Tok> {
    let mut out = Vec::new();
    for tok in tokenize(body, line) {
        if tok.kind == Kind::Ident {
            if let Some(idx) = params.iter().position(|p| *p == tok.text) {
                out.extend(with_hide(args[idx].clone(), hide));
                continue;
            }
        }
        out.push(tok);
    }
    with_hide(out, hide)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn macros(src: &str) -> Macros {
        let mut m = Macros::new();
        m.add_source(src);
        m
    }

    #[test]
    fn object_like_macro_in_declaration_position() {
        let m = macros("#define THREAD_LOCAL __thread\n");
        let e = expand("THREAD_LOCAL int adjustment = 0;\n", &m);
        assert_eq!(e.text.trim(), "__thread int adjustment = 0;");
        assert_eq!(e.expanded, vec![("THREAD_LOCAL".to_string(), 1)]);
    }

    #[test]
    fn function_like_macro_with_arguments() {
        let m = macros(
            "#define list_for_each(p, head) for (p = (head)->next; p != (head); p = p->next)\n",
        );
        let e = expand("list_for_each(li, &q->ifaces) { work(); }\n", &m);
        assert!(
            e.text.starts_with("for (li = (&q->ifaces)->next;"),
            "got {:?}",
            e.text
        );
        assert!(e.text.contains("{ work(); }"), "the block must survive");
    }

    #[test]
    fn a_type_can_be_passed_as_an_argument() {
        let m = macros("#define list_entry(ptr, type, member) ((type *)((char *)(ptr)))\n");
        let e = expand("x = list_entry(a, struct file_element, file_list);\n", &m);
        assert!(
            e.text.contains("((struct file_element *)"),
            "got {:?}",
            e.text
        );
    }

    #[test]
    fn a_macro_never_expands_inside_its_own_expansion() {
        let m = macros("#define foo foo\n#define bar bar + baz\n#define baz bar\n");
        let e = expand("foo; bar;\n", &m);
        assert!(e.text.contains("foo;"), "self-reference must terminate");
        assert!(
            e.text.contains("bar"),
            "mutual recursion must terminate: {:?}",
            e.text
        );
    }

    #[test]
    fn nested_macros_expand_through() {
        let m = macros("#define INNER 1\n#define OUTER INNER + INNER\n");
        let e = expand("int x = OUTER;\n", &m);
        assert_eq!(e.text.trim(), "int x = 1 + 1;");
    }

    #[test]
    fn directives_comments_and_strings_are_never_expanded() {
        let m = macros("#define FOO bar\n");
        let src = "#include <FOO.h>\n/* FOO in a comment */\nconst char *s = \"FOO\";\nFOO;\n";
        let e = expand(src, &m);
        assert!(e.text.contains("#include <FOO.h>"), "directive untouched");
        assert!(
            e.text.contains("/* FOO in a comment */"),
            "comment untouched"
        );
        assert!(e.text.contains("\"FOO\""), "string literal untouched");
        assert!(e.text.contains("bar;"), "real use expanded: {:?}", e.text);
    }

    #[test]
    fn unsupported_constructs_are_refused_by_name_not_guessed() {
        let m = macros("#define GLUE(a, b) a ## b\n#define STR(x) #x\n");
        let e = expand("GLUE(x, y); STR(hello);\n", &m);
        assert!(e.text.contains("GLUE(x, y)"), "token pasting left alone");
        assert!(e.text.contains("STR(hello)"), "stringify left alone");
        let reasons: Vec<&str> = e.refused.iter().map(|(_, why)| *why).collect();
        assert!(reasons.contains(&"token pasting"));
        assert!(reasons.contains(&"stringify"));
    }

    #[test]
    fn a_bare_mention_of_a_function_like_macro_is_not_an_invocation() {
        let m = macros("#define f(x) ((x) + 1)\n");
        let e = expand("void (*p)(int) = f;\n", &m);
        assert!(
            e.text.contains("= f;"),
            "no parens, no expansion: {:?}",
            e.text
        );
    }

    #[test]
    fn a_macro_shadowing_a_keyword_is_never_expanded() {
        // Packages really do ship `#define const` and `#define __attribute__(x)`
        // for non-GNU compilers. Expanding them deletes keywords from live code.
        let m = macros("#define const\n#define __attribute__(x)\n");
        let e = expand("static const int x __attribute__((unused)) = 1;\n", &m);
        assert!(
            e.text.contains("static const int x"),
            "keyword survived: {:?}",
            e.text
        );
        assert!(
            e.text.contains("__attribute__((unused))"),
            "attribute survived"
        );
        assert!(e
            .refused
            .iter()
            .any(|(_, why)| *why == "shadows a keyword the grammar models itself"));
    }

    #[test]
    fn argument_count_mismatch_is_refused() {
        let m = macros("#define pair(a, b) a, b\n");
        let e = expand("pair(1);\n", &m);
        assert!(e.text.contains("pair(1)"));
        assert!(e
            .refused
            .iter()
            .any(|(_, why)| *why == "argument count mismatch"));
    }

    #[test]
    fn expansions_are_attributed_to_the_original_line() {
        let m = macros("#define A 1\n");
        let e = expand("int w;\nint x;\nint y = A;\n", &m);
        assert_eq!(e.expanded, vec![("A".to_string(), 3)]);
        assert_eq!(e.near(3), vec!["A"]);
        assert!(e.near(9).is_empty());
    }

    #[test]
    fn later_definitions_win_and_undef_removes() {
        let m = macros("#define X 1\n#define X 2\n#define Y 3\n#undef Y\n");
        assert_eq!(m.get("X").unwrap().body, "2");
        assert!(m.get("Y").is_none());
    }

    #[test]
    fn multi_line_definitions_are_joined() {
        let m = macros("#define LOOP(p, h) \\\n    for (p = h; p; p = p->next)\n");
        let e = expand("LOOP(a, b) {}\n", &m);
        assert!(
            e.text.contains("for (a = b; a; a = a->next)"),
            "got {:?}",
            e.text
        );
    }
}
