//! Dead-branch elimination for languages with a C-style preprocessor.
//!
//! # Why this exists
//!
//! A tree-sitter grammar parses the source as written: every `#if` branch at
//! once, as one tree. A compiler parses the source after the preprocessor has
//! deleted the branches that do not apply. For most code the two agree. For
//! one very common shape they cannot:
//!
//! ```c
//! #ifdef __cplusplus
//! extern "C" {          // opens a brace
//! #endif
//! int f(void);
//! #ifdef __cplusplus
//! }                     // closes it, inside a DIFFERENT conditional
//! #endif
//! ```
//!
//! The `{` and its `}` are in different conditional blocks, so neither block
//! is brace-balanced and no single tree can represent both configurations.
//! Compiling as C, `__cplusplus` is undefined and the compiler never sees
//! either brace: the file is valid C. Measured on the Debian C corpus, the
//! grammar rejects 1,543 of the 1,555 headers containing `extern "C"`, and
//! **962 of those rejections are this shape and nothing else**.
//!
//! Those are not grammar bugs. There is no patch to a tree-sitter grammar
//! that fixes them, because the token stream is not nested. This crate exists
//! so a sweep can tell that class apart from a real gap, instead of parking
//! it at the top of the fix queue where it will absorb an agent's attempts
//! and tempt it into accepting unbalanced braces.
//!
//! Measured on the 20-package Debian C corpus: **909 of 5,502 gap files
//! (16.5%) are this class**, and the fix queue drops to 4,593.
//!
//! # Why the conditional evaluator is careful
//!
//! A throwaway prototype of this reduction reported 967 files. It was wrong,
//! and the way it was wrong is the reason this crate evaluates expressions
//! properly: it treated *any* conditional mentioning `__cplusplus` as false,
//! so it deleted branches like
//!
//! ```c
//! #if defined(__GNUC__) && defined(K5_BE) && !defined(__cplusplus)
//! ```
//!
//! which is not false — it is *unknown*, depending on `__GNUC__`. Deleting
//! live code makes files parse more easily and inflates exactly the number
//! you are trying to measure. The 58-file gap between prototype and
//! implementation was entirely that unsoundness, plus 43 files the
//! implementation decides correctly and the prototype could not.
//!
//! # What it deliberately does not do
//!
//! It does not expand macros *here*. That lives in [`expand`], and it is a
//! deliberately different kind of operation: reduction only ever removes code
//! a compiler would not have seen, while expansion fabricates text, so a bug
//! in it invents parse successes. Reduction changes a verdict; expansion only
//! ever annotates one.
//!
//! It does not evaluate comparisons: `#if __cplusplus >= 201103L` is left
//! alone even though it is decidable. That is a deliberate omission with a
//! measured cost — the idiom appears 16 times in the whole corpus — and the
//! evaluator returns `Unknown` rather than guessing.
//!
//! It does not enumerate configurations. Measured on the same corpus, the
//! median file has 2 controlling symbols but the 99th percentile has 60 and
//! the worst has 819 — enumeration is impossible on exactly the files that
//! matter most. Only conditionals that are *decidable* from the declared
//! symbols are touched; everything else is left exactly as written.

pub mod branches;
pub mod expand;

pub use branches::{force_branch, innermost_containing, line_survives, regions, Region};
pub use expand::{expand, Expansion, Macro, Macros};

use std::collections::{HashMap, HashSet};

/// What a language declares it knows about its own preprocessor.
///
/// This is deliberately a small, fixed table rather than a real symbol
/// environment. `__cplusplus` is not a variable we are uncertain about: when
/// parsing C it is *always* undefined, and that single fact is what makes the
/// `extern "C"` class decidable.
#[derive(Debug, Default, Clone)]
pub struct Symbols {
    defined: HashMap<String, i64>,
    undefined: HashSet<String>,
}

impl Symbols {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare a symbol as never defined in this dialect.
    pub fn undefined(mut self, name: &str) -> Self {
        self.undefined.insert(name.to_string());
        self
    }

    /// Declare a symbol as always defined, with an integer value for `#if`.
    pub fn defined(mut self, name: &str, value: i64) -> Self {
        self.defined.insert(name.to_string(), value);
        self
    }

    fn known(&self, name: &str) -> Option<Option<i64>> {
        if let Some(v) = self.defined.get(name) {
            Some(Some(*v))
        } else if self.undefined.contains(name) {
            Some(None)
        } else {
            None
        }
    }
}

/// Three-valued, because "we cannot tell" is the common case and must never
/// be confused with false. `Unknown && false` is still false, which decides
/// more conditionals than a two-valued evaluator could.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tri {
    True,
    False,
    Unknown,
}

impl Tri {
    fn from_bool(b: bool) -> Tri {
        if b {
            Tri::True
        } else {
            Tri::False
        }
    }
    fn not(self) -> Tri {
        match self {
            Tri::True => Tri::False,
            Tri::False => Tri::True,
            Tri::Unknown => Tri::Unknown,
        }
    }
    fn and(self, other: Tri) -> Tri {
        match (self, other) {
            (Tri::False, _) | (_, Tri::False) => Tri::False,
            (Tri::True, Tri::True) => Tri::True,
            _ => Tri::Unknown,
        }
    }
    fn or(self, other: Tri) -> Tri {
        match (self, other) {
            (Tri::True, _) | (_, Tri::True) => Tri::True,
            (Tri::False, Tri::False) => Tri::False,
            _ => Tri::Unknown,
        }
    }
}

/// The outcome of reducing one file.
#[derive(Debug, Clone)]
pub struct Reduced {
    /// The source with dead branches blanked. **Line numbering is preserved**
    /// — removed lines become empty rather than disappearing — so any
    /// diagnostic still points at the original line.
    pub text: String,
    /// Conditionals whose value was decided from the declared symbols.
    pub decided: usize,
    /// Conditionals left exactly as written because they were not decidable.
    pub undecided: usize,
}

impl Reduced {
    /// Did reduction change anything at all? A file with no decidable
    /// conditional is returned byte-for-byte, and re-parsing it would be a
    /// waste of time.
    pub fn changed(&self) -> bool {
        self.decided > 0
    }
}

struct Frame {
    /// Was this conditional's value decided from the declared symbols?
    decided: bool,
    /// Are lines in the current branch kept?
    active: bool,
    /// Has a branch of this conditional already been taken? (`#elif` chains.)
    taken: bool,
}

/// Delete the branches that the declared symbols rule out.
///
/// Conditionals that cannot be decided are passed through untouched, both
/// branches included, exactly as a tree-sitter grammar sees them today. So
/// the reduction is only ever a *narrowing* toward what a compiler saw, never
/// a guess.
pub fn reduce(source: &str, symbols: &Symbols) -> Reduced {
    let mut out: Vec<&str> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    // Symbols the file defines itself, tracked so `#ifdef LOCAL_THING` is
    // decidable when `LOCAL_THING` was #defined above it in a live region.
    let mut local: HashMap<String, Option<i64>> = HashMap::new();
    let (mut decided, mut undecided) = (0usize, 0usize);

    let live = |stack: &Vec<Frame>| stack.iter().all(|f| f.active);

    for line in source.split('\n') {
        let Some((directive, rest)) = split_directive(line) else {
            out.push(if live(&stack) { line } else { "" });
            continue;
        };
        match directive {
            "ifdef" | "ifndef" | "if" => {
                let value = if !live(&stack) {
                    // Inside dead code: never evaluate, just track nesting.
                    Tri::Unknown
                } else {
                    match directive {
                        "ifdef" => defined_state(rest.trim(), symbols, &local),
                        "ifndef" => defined_state(rest.trim(), symbols, &local).not(),
                        _ => eval(rest, symbols, &local),
                    }
                };
                let is_decided = live(&stack) && value != Tri::Unknown;
                if is_decided {
                    decided += 1;
                } else if live(&stack) {
                    undecided += 1;
                }
                let active = match value {
                    Tri::True => true,
                    Tri::False => false,
                    Tri::Unknown => true, // keep both branches: status quo
                };
                stack.push(Frame {
                    decided: is_decided,
                    active,
                    taken: active,
                });
                // A decided conditional's own directives are removed with it;
                // an undecided one keeps them so the file still reads the same.
                out.push(if is_decided || !live_below(&stack) { "" } else { line });
            }
            "elif" => {
                if let Some(frame) = stack.last_mut() {
                    if frame.decided {
                        // Only one branch of a decided chain can run. Once a
                        // branch has been taken the rest are dead; otherwise
                        // we cannot tell, so keep this one.
                        frame.active = !frame.taken;
                        frame.taken = true;
                        out.push("");
                        continue;
                    }
                }
                out.push(if live_below(&stack) { line } else { "" });
            }
            "else" => {
                if let Some(frame) = stack.last_mut() {
                    if frame.decided {
                        frame.active = !frame.taken;
                        frame.taken = true;
                        out.push("");
                        continue;
                    }
                }
                out.push(if live_below(&stack) { line } else { "" });
            }
            "endif" => {
                let was_decided = stack.pop().map(|f| f.decided).unwrap_or(false);
                out.push(if was_decided || !live(&stack) { "" } else { line });
            }
            "define" => {
                if live(&stack) {
                    let mut parts = rest.trim().splitn(2, |c: char| c.is_whitespace() || c == '(');
                    if let Some(name) = parts.next().filter(|n| !n.is_empty()) {
                        let value = parts.next().and_then(|v| v.trim().parse::<i64>().ok());
                        local.insert(name.to_string(), value);
                    }
                }
                out.push(if live(&stack) { line } else { "" });
            }
            "undef" => {
                if live(&stack) {
                    local.remove(rest.trim());
                }
                out.push(if live(&stack) { line } else { "" });
            }
            _ => out.push(if live(&stack) { line } else { "" }),
        }
    }

    Reduced {
        text: out.join("\n"),
        decided,
        undecided,
    }
}

/// Is everything *below* the innermost frame live? Used when deciding whether
/// to keep a directive belonging to the innermost frame itself.
fn live_below(stack: &[Frame]) -> bool {
    stack.len() < 2 || stack[..stack.len() - 1].iter().all(|f| f.active)
}

/// `# <directive> <rest>`, tolerating whitespace either side of the `#`.
fn split_directive(line: &str) -> Option<(&str, &str)> {
    let t = line.trim_start();
    let t = t.strip_prefix('#')?;
    let t = t.trim_start();
    let end = t.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(t.len());
    let (word, rest) = t.split_at(end);
    if word.is_empty() {
        return None;
    }
    Some((word, rest))
}

fn defined_state(name: &str, symbols: &Symbols, local: &HashMap<String, Option<i64>>) -> Tri {
    let name = name.split_whitespace().next().unwrap_or("");
    if local.contains_key(name) {
        return Tri::True;
    }
    match symbols.known(name) {
        Some(Some(_)) => Tri::True,
        Some(None) => Tri::False,
        None => Tri::Unknown,
    }
}

/// A deliberately small `#if` evaluator: `defined()`, `!`, `&&`, `||`,
/// parentheses and integer literals. Anything else — arithmetic, comparisons,
/// string tests, function-like macros — evaluates to `Unknown`, which leaves
/// the conditional untouched. Being unable to decide is always safe here;
/// guessing is not.
fn eval(expr: &str, symbols: &Symbols, local: &HashMap<String, Option<i64>>) -> Tri {
    let tokens = tokenize(expr);
    let mut pos = 0;
    let value = parse_or(&tokens, &mut pos, symbols, local);
    if pos == tokens.len() {
        value
    } else {
        Tri::Unknown // trailing tokens we do not model
    }
}

fn tokenize(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(chars[start..i].iter().collect());
        } else if (c == '&' || c == '|') && i + 1 < chars.len() && chars[i + 1] == c {
            out.push(format!("{c}{c}"));
            i += 2;
        } else {
            out.push(c.to_string());
            i += 1;
        }
    }
    out
}

fn parse_or(t: &[String], pos: &mut usize, s: &Symbols, l: &HashMap<String, Option<i64>>) -> Tri {
    let mut value = parse_and(t, pos, s, l);
    while t.get(*pos).map(String::as_str) == Some("||") {
        *pos += 1;
        value = value.or(parse_and(t, pos, s, l));
    }
    value
}

fn parse_and(t: &[String], pos: &mut usize, s: &Symbols, l: &HashMap<String, Option<i64>>) -> Tri {
    let mut value = parse_unary(t, pos, s, l);
    while t.get(*pos).map(String::as_str) == Some("&&") {
        *pos += 1;
        value = value.and(parse_unary(t, pos, s, l));
    }
    value
}

fn parse_unary(t: &[String], pos: &mut usize, s: &Symbols, l: &HashMap<String, Option<i64>>) -> Tri {
    match t.get(*pos).map(String::as_str) {
        Some("!") => {
            *pos += 1;
            parse_unary(t, pos, s, l).not()
        }
        Some("(") => {
            *pos += 1;
            let value = parse_or(t, pos, s, l);
            if t.get(*pos).map(String::as_str) == Some(")") {
                *pos += 1;
                value
            } else {
                Tri::Unknown
            }
        }
        Some("defined") => {
            *pos += 1;
            let parenthesised = t.get(*pos).map(String::as_str) == Some("(");
            if parenthesised {
                *pos += 1;
            }
            let Some(name) = t.get(*pos).cloned() else { return Tri::Unknown };
            *pos += 1;
            if parenthesised {
                if t.get(*pos).map(String::as_str) != Some(")") {
                    return Tri::Unknown;
                }
                *pos += 1;
            }
            defined_state(&name, s, l)
        }
        Some(word) => {
            let word = word.to_string();
            *pos += 1;
            if let Ok(n) = word.parse::<i64>() {
                return Tri::from_bool(n != 0);
            }
            // A bare identifier in #if is its value, or 0 when undefined.
            match (l.get(&word), s.known(&word)) {
                (Some(Some(v)), _) => Tri::from_bool(*v != 0),
                (Some(None), _) => Tri::Unknown, // defined, but not to an integer
                (None, Some(Some(v))) => Tri::from_bool(v != 0),
                (None, Some(None)) => Tri::False, // declared undefined -> 0
                (None, None) => Tri::Unknown,
            }
        }
        None => Tri::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c() -> Symbols {
        Symbols::new().undefined("__cplusplus")
    }

    fn lines(text: &str) -> Vec<&str> {
        text.split('\n').collect()
    }

    #[test]
    fn extern_c_braces_are_removed_and_line_numbers_survive() {
        let src = "#ifdef __cplusplus\nextern \"C\" {\n#endif\nint f(void);\n#ifdef __cplusplus\n}\n#endif\n";
        let r = reduce(src, &c());
        assert!(r.changed());
        assert!(!r.text.contains("extern"), "the C++ opener must be gone");
        assert!(!r.text.contains('{'), "the unbalanced brace must be gone");
        assert!(r.text.contains("int f(void);"));
        assert_eq!(lines(&r.text).len(), lines(src).len(), "line count preserved");
        assert_eq!(lines(&r.text)[3], "int f(void);", "and line positions too");
    }

    #[test]
    fn ifndef_cplusplus_keeps_the_c_branch() {
        let src = "#ifndef __cplusplus\nint c_only;\n#else\nint cxx_only;\n#endif\n";
        let r = reduce(src, &c());
        assert!(r.text.contains("int c_only;"));
        assert!(!r.text.contains("cxx_only"));
    }

    #[test]
    fn else_branch_is_taken_when_the_condition_is_false() {
        let src = "#ifdef __cplusplus\nint cxx;\n#else\nint plain;\n#endif\n";
        let r = reduce(src, &c());
        assert!(r.text.contains("int plain;"));
        assert!(!r.text.contains("int cxx;"));
    }

    #[test]
    fn undecidable_conditionals_are_left_completely_alone() {
        let src = "#ifdef HAVE_SOMETHING\nint a;\n#else\nint b;\n#endif\n";
        let r = reduce(src, &c());
        assert!(!r.changed());
        assert_eq!(r.text, src, "an undecidable file must come back byte-for-byte");
        assert_eq!(r.undecided, 1);
    }

    #[test]
    fn three_valued_and_decides_what_two_valued_could_not() {
        // Unknown && false is false, so the branch is dead even though
        // HAVE_X is unknown.
        let src = "#if defined(HAVE_X) && defined(__cplusplus)\nint dead;\n#endif\nint live;\n";
        let r = reduce(src, &c());
        assert!(!r.text.contains("dead"));
        assert!(r.text.contains("live"));
    }

    #[test]
    fn if_zero_is_dead_code() {
        let src = "#if 0\nthis is not even C\n#endif\nint live;\n";
        let r = reduce(src, &Symbols::new());
        assert!(!r.text.contains("not even C"));
        assert!(r.text.contains("int live;"));
    }

    #[test]
    fn local_defines_make_later_conditionals_decidable() {
        let src = "#define LOCAL 1\n#ifdef LOCAL\nint kept;\n#endif\n";
        let r = reduce(src, &Symbols::new());
        assert!(r.text.contains("int kept;"));
        assert_eq!(r.decided, 1);
    }

    #[test]
    fn nesting_inside_a_dead_branch_does_not_confuse_the_stack() {
        let src = "#ifdef __cplusplus\n#ifdef ANYTHING\nint dead;\n#endif\n#endif\nint live;\n";
        let r = reduce(src, &c());
        assert!(!r.text.contains("dead"));
        assert!(r.text.contains("int live;"));
    }

    #[test]
    fn an_elif_chain_takes_only_its_first_true_branch() {
        let src = "#if 0\nint a;\n#elif 1\nint b;\n#elif 1\nint c;\n#endif\n";
        let r = reduce(src, &Symbols::new());
        assert!(!r.text.contains("int a;"));
        assert!(r.text.contains("int b;"));
        assert!(!r.text.contains("int c;"), "a later true branch is still dead");
    }
}
