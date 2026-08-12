//! Forcing one conditional branch, to tell a *split construct* from a
//! grammar gap.
//!
//! [`crate::reduce`] can only decide conditionals whose symbols a language
//! declares. Most of the ones that break a parse are not like that:
//!
//! ```c
//! #ifdef F_DUPFD_CLOEXEC
//! int dup_fd_cloexec(int oldfd, int lowfd)
//! #else
//! int dup_fd_cloexec(int oldfd, int lowfd __attribute__((__unused__)))
//! #endif
//! { ... }
//! ```
//!
//! `F_DUPFD_CLOEXEC` is a feature macro from a system header. Nothing here
//! knows whether it is defined — and it does not matter, because **both
//! branches are valid C and the grammar fails on neither of them**. It fails
//! only because it must see both at once. That is the same class as the
//! `extern "C"` case and equally unfixable in a grammar.
//!
//! So the test is not "what is this symbol" but: *does forcing either branch
//! make the error go away?*
//!
//! # The guard that makes this sound
//!
//! Forcing a branch can remove an error two ways: by resolving the split, or
//! by **deleting the offending code**. The second proves nothing. Every C
//! header is wrapped in `#ifndef FOO_H`, so every error in one is "inside a
//! conditional", and blanking that branch empties the file and clears every
//! error in it.
//!
//! Measured, that artifact is most of the raw signal: of 243 sampled gap
//! files whose first error sat inside a conditional, 173 "cleared" only by
//! deletion and 69 were genuine splits. So a caller must require that **the
//! error's own line survives** the forcing. [`force_branch`] preserves line
//! numbering precisely so that check is possible.
//!
//! # How much this is worth, corpus-wide
//!
//! On the 20-package Debian C corpus: **51 gap files are explained entirely by
//! split constructs, and 43 more had a split sitting ahead of their real
//! problem** and are now clustered on that instead.
//!
//! That is far less than the ~790 an earlier sample projected, and the
//! difference was a sampling mistake worth recording. The sample was drawn
//! from cluster *examples* — at most five per cluster — rather than from the
//! gap files themselves, which over-weights small clusters about fourfold.
//! Conditional splits concentrate in exactly those small odd clusters, so the
//! estimate came out roughly eight times too high. The implementation was
//! right; the projection was not.
//!
//! Refusing `#elif` chains costs almost nothing: 9 of 828 sampled gap files
//! have their first error inside one.

/// One `#if … [#else] … #endif`, by 1-based line number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub start: usize,
    /// The `#else` line, when there is exactly one alternative.
    pub mid: Option<usize>,
    pub end: usize,
}

fn directive(line: &str) -> Option<&str> {
    let t = line.trim_start().strip_prefix('#')?.trim_start();
    let end = t.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(t.len());
    Some(&t[..end])
}

/// Every complete conditional region in the file.
///
/// `#elif` chains are deliberately excluded: forcing "not the first branch"
/// would merge the remaining alternatives, which can produce text that was
/// never a configuration of this file. Refusing is the honest option.
pub fn regions(source: &str) -> Vec<Region> {
    let mut open: Vec<(usize, Option<usize>, bool)> = Vec::new();
    let mut out = Vec::new();
    for (i, line) in source.split('\n').enumerate() {
        let n = i + 1;
        match directive(line) {
            Some("if") | Some("ifdef") | Some("ifndef") => open.push((n, None, false)),
            Some("else") => {
                if let Some(top) = open.last_mut() {
                    if top.1.is_some() {
                        top.2 = true; // more than one alternative
                    } else {
                        top.1 = Some(n);
                    }
                }
            }
            Some("elif") => {
                if let Some(top) = open.last_mut() {
                    top.2 = true;
                }
            }
            Some("endif") => {
                if let Some((start, mid, chained)) = open.pop() {
                    if !chained {
                        out.push(Region { start, mid, end: n });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The smallest region containing `line`, if any.
pub fn innermost_containing(source: &str, line: usize) -> Option<Region> {
    regions(source)
        .into_iter()
        .filter(|r| r.start <= line && line <= r.end)
        .min_by_key(|r| r.end - r.start)
}

/// The file with one branch of `region` kept and the other removed.
///
/// Removed lines are blanked rather than deleted, so **every surviving line
/// keeps its original number** and a caller can check whether the line it
/// cares about is still there.
pub fn force_branch(source: &str, region: &Region, keep_if: bool) -> String {
    let boundary = region.mid.unwrap_or(region.end);
    source
        .split('\n')
        .enumerate()
        .map(|(i, line)| {
            let n = i + 1;
            let is_directive = n == region.start || n == region.end || Some(n) == region.mid;
            if is_directive {
                return "";
            }
            let in_if = n > region.start && n < boundary;
            let in_else = region.mid.is_some() && n > boundary && n < region.end;
            if (in_if && !keep_if) || (in_else && keep_if) {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Is `line` (1-based) present and non-blank in `text`?
///
/// The soundness check: an error that only disappears because its own line
/// was deleted has not been explained.
pub fn line_survives(text: &str, line: usize) -> bool {
    text.split('\n')
        .nth(line.saturating_sub(1))
        .is_some_and(|l| !l.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPLIT: &str = "\
#ifdef F_DUPFD_CLOEXEC
int f(int a, int b)
#else
int f(int a, int b __attribute__((unused)))
#endif
{ return a; }
";

    #[test]
    fn a_split_signature_is_found_as_one_region() {
        let rs = regions(SPLIT);
        assert_eq!(rs, vec![Region { start: 1, mid: Some(3), end: 5 }]);
    }

    #[test]
    fn forcing_either_branch_leaves_exactly_one_signature() {
        let r = innermost_containing(SPLIT, 2).unwrap();
        let yes = force_branch(SPLIT, &r, true);
        assert!(yes.contains("int f(int a, int b)"));
        assert!(!yes.contains("unused"));
        let no = force_branch(SPLIT, &r, false);
        assert!(no.contains("unused"));
        assert!(!no.contains("int f(int a, int b)\n"));
    }

    #[test]
    fn line_numbering_is_preserved_so_the_guard_can_be_applied() {
        let r = innermost_containing(SPLIT, 2).unwrap();
        let yes = force_branch(SPLIT, &r, true);
        assert_eq!(yes.split('\n').count(), SPLIT.split('\n').count());
        assert!(line_survives(&yes, 2), "the kept signature is still on line 2");
        assert!(!line_survives(&yes, 4), "the dropped one is blank, not gone");
    }

    #[test]
    fn an_include_guard_only_clears_errors_by_deleting_them() {
        // The artifact this whole guard exists for: forcing the empty branch
        // of a header's include guard blanks the file.
        let src = "#ifndef FOO_H\n#define FOO_H\nint x = ;\n#endif\n";
        let r = innermost_containing(src, 3).unwrap();
        let emptied = force_branch(src, &r, false);
        assert!(!line_survives(&emptied, 3), "the error line is gone, not fixed");
        let kept = force_branch(src, &r, true);
        assert!(line_survives(&kept, 3), "the other choice changes nothing");
    }

    #[test]
    fn the_innermost_region_wins() {
        let src = "#ifndef H\n#ifdef A\nint a;\n#endif\n#endif\n";
        assert_eq!(
            innermost_containing(src, 3).unwrap(),
            Region { start: 2, mid: None, end: 4 }
        );
    }

    #[test]
    fn elif_chains_are_refused_rather_than_merged() {
        let src = "#if A\nint a;\n#elif B\nint b;\n#else\nint c;\n#endif\n";
        assert!(regions(src).is_empty(), "an #elif chain has no safe two-way split");
        assert!(innermost_containing(src, 2).is_none());
    }

    #[test]
    fn a_region_with_no_else_drops_only_its_body() {
        let src = "int before;\n#ifdef A\nint inside;\n#endif\nint after;\n";
        let r = innermost_containing(src, 3).unwrap();
        let without = force_branch(src, &r, false);
        assert!(without.contains("int before;") && without.contains("int after;"));
        assert!(!without.contains("int inside;"));
        let with = force_branch(src, &r, true);
        assert!(with.contains("int inside;"));
    }
}
