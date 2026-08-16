//! Facet query expansion. Facet roles are not grammar rules, so a query
//! like `(_callable name: (_name) @n)` cannot run against the parser as
//! written; this rewrites every facet pattern into the concrete
//! alternation the manifest defines:
//!
//!   (_callable)                ->  [(function_definition) (lambda)]
//!   (_callable name: (x) @n)   ->  [(function_definition name: (x) @n)
//!                                   (lambda name: (x) @n)]
//!
//! Table-tier supertypes pass through untouched — they are real node types
//! and tree-sitter matches them natively. String literals and `;` comments
//! are copied verbatim, never rewritten inside.

use std::collections::BTreeMap;

use anyhow::{bail, Result};

pub fn expand(query: &str, facets: &BTreeMap<String, Vec<String>>) -> Result<String> {
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
                if let Some(members) = facets.get(name) {
                    if members.is_empty() {
                        bail!("facet `{name}` has no members");
                    }
                    let close = matching_paren(query, i)?;
                    let body = expand(&query[name_end..close], facets)?;
                    out.push('[');
                    for (k, m) in members.iter().enumerate() {
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
