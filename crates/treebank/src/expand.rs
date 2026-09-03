//! Nominal query expansion. A nominal term is not a grammar rule, so a
//! query like `(_callable name: (_name) @n)` cannot run against the parser
//! as written; this rewrites every nominal pattern into the concrete
//! alternation the manifest defines:
//!
//!   (_callable)                ->  [(function_definition) (lambda)]
//!   (_callable name: (x) @n)   ->  [(function_definition name: (x) @n)
//!                                   (lambda name: (x) @n)]
//!
//! Structural terms pass through untouched — they are real supertypes and
//! tree-sitter matches them natively. String literals and `;` comments are
//! copied verbatim, never rewritten inside.

use std::collections::BTreeMap;

use anyhow::{bail, Result};

/// The field constraints at DEPTH 0 of a pattern body: each is the field
/// name plus the node type names its value pattern demands (empty means
/// presence is enough -- an anonymous or wildcard value). Only depth-0
/// fields bind to the nominal member itself; `#`-predicates and strings are
/// skipped, and `(#match? ...)` opens a paren so it is already depth 1.
fn top_level_field_constraints(body: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let bytes = body.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = skip_string(body, i).unwrap_or(body.len()),
            b'(' | b'[' => {
                depth += 1;
                i += 1;
            }
            b')' | b']' => {
                depth -= 1;
                i += 1;
            }
            b'a'..=b'z' | b'_' if depth == 0 => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                if bytes.get(i) == Some(&b':') {
                    let field = body[start..i].to_string();
                    i += 1;
                    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    // The value pattern: `(name ...)` or an alternation
                    // `[(a ...) (b ...)]` (a nominal term already expanded).
                    let mut names = Vec::new();
                    let mut j = i;
                    let openers: &[u8] = if bytes.get(j) == Some(&b'[') {
                        j += 1;
                        b"("
                    } else {
                        b"("
                    };
                    let _ = openers;
                    while j < bytes.len() {
                        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                            j += 1;
                        }
                        if bytes.get(j) != Some(&b'(') {
                            break;
                        }
                        j += 1;
                        let ns = j;
                        while j < bytes.len()
                            && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_')
                        {
                            j += 1;
                        }
                        if j > ns {
                            names.push(body[ns..j].to_string());
                        }
                        // Skip to this pattern's close so a bracketed
                        // alternation yields every member name.
                        let mut d = 1i32;
                        while j < bytes.len() && d > 0 {
                            match bytes[j] {
                                b'(' => d += 1,
                                b')' => d -= 1,
                                b'"' => {
                                    j = skip_string(body, j).unwrap_or(body.len());
                                    continue;
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        if !body[i..].starts_with('[') {
                            break;
                        }
                    }
                    // `_` is the WILDCARD, not a node type. A field whose
                    // value pattern is `(_)`, or an alternation containing
                    // one, constrains nothing but presence -- and treating it
                    // as a type named `_` filtered every member out, because
                    // no field declares one.
                    if names.iter().any(|n| n == "_") {
                        names.clear();
                    }
                    out.push((field, names));
                }
            }
            _ => i += 1,
        }
    }
    out
}

pub fn expand(query: &str, nominal: &BTreeMap<String, Vec<String>>) -> Result<String> {
    expand_with_types(query, nominal, None)
}

/// Like [`expand`], but a member is DROPPED from the alternation when a
/// depth-0 field constraint cannot hold for it: the member does not
/// declare the field, or none of the field's declared types intersects
/// the constraint's type (supertype closure included). This mirrors what
/// tree-sitter itself checks for a NATIVE supertype pattern, where the
/// pattern survives if any one subtype satisfies it. A member absent from
/// node-types is kept, and an expansion that filters every member is an
/// error, not an empty alternation.
pub fn expand_with_types(
    query: &str,
    nominal: &BTreeMap<String, Vec<String>>,
    node_types: Option<&crate::node_types::NodeTypes>,
) -> Result<String> {
    let mut out = String::with_capacity(query.len());
    let bytes = query.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let end = skip_string(query, i)?;
                out.push_str(&query[i..end]);
                i = end;
            }
            b';' => {
                let end = query[i..].find('\n').map_or(query.len(), |n| i + n);
                out.push_str(&query[i..end]);
                i = end;
            }
            b'(' => {
                let name_start = i + 1;
                let name_end = name_start
                    + query[name_start..]
                        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .unwrap_or(query.len() - name_start);
                let name = &query[name_start..name_end];
                if let Some(members) = nominal.get(name) {
                    if members.is_empty() {
                        bail!("nominal term `{name}` has no members");
                    }
                    let close = matching_paren(query, i)?;
                    let body = expand_with_types(&query[name_end..close], nominal, node_types)?;
                    let needed = top_level_field_constraints(&body);
                    let compatible = |m: &str| -> bool {
                        let Some(nt) = node_types else { return true };
                        let Some(declared) = nt.fields.get(m) else {
                            return true;
                        };
                        needed.iter().all(|(f, want)| {
                            let Some(have) = declared.get(f) else {
                                return false;
                            };
                            if want.is_empty() {
                                return true;
                            }
                            // Derivation-based, notes/DESIGN.md §2 fact 4: a
                            // supertype pattern only matches where the
                            // field DECLARES that supertype (the value
                            // derives through it). `namespace_definition`'s
                            // body declares concrete `block`, so `(_body)`
                            // never matches there even though block is a
                            // _body subtype -- the closure runs from what
                            // is declared toward what is asked, never the
                            // other way.
                            want.iter().any(|w| {
                                have.contains(w) || have.iter().any(|h| nt.closure(h).contains(w))
                            })
                        })
                    };
                    let kept: Vec<&String> = members.iter().filter(|m| compatible(m)).collect();
                    if kept.is_empty() {
                        bail!(
                            "nominal term `{name}`: no member satisfies the field constraint(s) {needed:?}"
                        );
                    }
                    out.push('[');
                    for (k, m) in kept.iter().enumerate() {
                        if k > 0 {
                            out.push(' ');
                        }
                        out.push('(');
                        out.push_str(m);
                        out.push_str(&body);
                        out.push(')');
                    }
                    out.push(']');
                    i = close + 1;
                } else {
                    out.push('(');
                    i += 1;
                }
            }
            _ => {
                let ch = query[i..].chars().next().unwrap();
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    Ok(out)
}

/// Byte index just past the closing quote of the string starting at `start`.
fn skip_string(s: &str, start: usize) -> Result<usize> {
    let bytes = s.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Ok(i + 1),
            _ => i += 1,
        }
    }
    bail!("unterminated string literal in query");
}

/// Byte index of the `)` matching the `(` at `open`.
fn matching_paren(s: &str, open: usize) -> Result<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = skip_string(s, i)? - 1,
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i);
                }
            }
            b';' => {
                i = s[i..].find('\n').map_or(s.len(), |n| i + n);
            }
            _ => {}
        }
        i += 1;
    }
    bail!("unbalanced parentheses in query");
}
