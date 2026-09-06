// GENERATED from hcl.sdf3 by treebank-sdf3's winnow backend. Do not edit.

#![allow(dead_code, unused_imports, unused_variables, unused_mut, unused_assignments, clippy::all)]
use std::collections::BTreeMap;
use winnow::prelude::*;
use winnow::ascii::Caseless;
use winnow::combinator::{alt, eof, not, opt, repeat};
use winnow::error::{ContextError, ErrMode};
use winnow::stream::{LocatingSlice, Location, Stateful, Stream};
use winnow::token::{literal, none_of, one_of};
use winnow::ModalResult;

type In<'a> = Stateful<LocatingSlice<&'a str>, St<'a>>;

#[derive(Debug)]
struct St<'a> {
    src: &'a str,
    line_starts: Vec<usize>,
    /// comment start -> (end, node name)
    comments: BTreeMap<usize, (usize, &'static str)>,
    /// offside limits: a token on a later line must sit at a greater column
    offside: Vec<usize>,
    last_end: usize,
    furthest: usize,
    /// The span of the captured sort inside the last lexical token.
    cap: Option<(usize, usize)>,
    /// Open delimiters: the closer's parser and the word it must carry.
    delim: Vec<(fn(&mut In<'a>) -> ModalResult<Node>, String)>,
}

impl<'a> St<'a> {
    fn new(src: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (k, c) in src.char_indices() {
            if c == '\n' { line_starts.push(k + 1); }
        }
        St { src, line_starts, comments: BTreeMap::new(), offside: Vec::new(), last_end: 0, furthest: 0, cap: None, delim: Vec::new() }
    }
}

#[derive(Clone, Debug)]
struct Node {
    kind: &'static str,
    start: usize,
    end: usize,
    /// How far the node claims extras: `end`, or the end of its last line
    /// for a production the tree-sitter lowering terminates with a hidden
    /// newline.
    reach: usize,
    children: Vec<(Option<&'static str>, Node)>,
}

impl Node {
    fn leaf(kind: &'static str, start: usize, end: usize) -> Node {
        Node { kind, start, end, reach: end, children: Vec::new() }
    }
}
fn line_end(i: &In, p: usize) -> usize {
    i.state.src[p..].find('\n').map(|k| p + k + 1).unwrap_or(i.state.src.len())
}

fn bt() -> ErrMode<ContextError> { ErrMode::Backtrack(ContextError::new()) }
fn no_layout(_i: &mut In) -> ModalResult<()> { Ok(()) }
/// Is the innermost open delimiter's closer here, at the start of a line?
fn closer_here(i: &mut In) -> bool {
    let Some((f, w)) = i.state.delim.last().cloned() else { return false };
    if col(i, pos(i)) != 0 { return false; }
    let cp = save(i);
    let hit = f(i).is_ok() && i.state.cap.map(|(s, e)| i.state.src[s..e] == *w).unwrap_or(false);
    restore(i, &cp);
    hit
}
type Cp<'a> = (<In<'a> as Stream>::Checkpoint, usize);
/// A checkpoint that also remembers the last token end, which winnow's
/// reset would not restore.
fn save<'a>(i: &In<'a>) -> Cp<'a> { (i.checkpoint(), i.state.last_end) }
fn restore<'a>(i: &mut In<'a>, cp: &Cp<'a>) { i.reset(&cp.0); i.state.last_end = cp.1; }
/// Pins the error type of an inline parser expression.
fn run<'a, O>(mut p: impl Parser<In<'a>, O, ErrMode<ContextError>>, i: &mut In<'a>) -> ModalResult<O> { p.parse_next(i) }
fn pos(i: &In) -> usize { i.current_token_start() }
fn token_end(i: &mut In, end: usize) { i.state.last_end = end; i.state.furthest = i.state.furthest.max(end); }
fn line(i: &In, p: usize) -> usize {
    match i.state.line_starts.binary_search(&p) { Ok(l) => l, Err(l) => l - 1 }
}
fn col(i: &In, p: usize) -> usize {
    let l = line(i, p);
    i.state.src[i.state.line_starts[l]..p].chars().count()
}
fn eq_cs(a: &str, b: &str) -> bool { a == b }
fn eq_ci(a: &str, b: &str) -> bool { a.eq_ignore_ascii_case(b) }

fn star<'a, O, P: Parser<In<'a>, O, ErrMode<ContextError>>>(p: P) -> impl Parser<In<'a>, (), ErrMode<ContextError>> {
    repeat::<_, _, (), _, _>(0.., p)
}
fn plus<'a, O, P: Parser<In<'a>, O, ErrMode<ContextError>>>(p: P) -> impl Parser<In<'a>, (), ErrMode<ContextError>> {
    repeat::<_, _, (), _, _>(1.., p)
}
fn optional<'a, O, P: Parser<In<'a>, O, ErrMode<ContextError>>>(p: P) -> impl Parser<In<'a>, (), ErrMode<ContextError>> {
    opt(p).void()
}

/// An extra goes to the innermost node whose span strictly holds it.
fn attach(n: &mut Node, extra: Node) {
    for (_, c) in n.children.iter_mut() {
        if c.start < extra.start && extra.start < c.reach {
            return attach(c, extra);
        }
    }
    let at = n.children.iter().position(|(_, c)| c.start > extra.start).unwrap_or(n.children.len());
    n.children.insert(at, (None, extra));
}

fn sexp(n: &Node, field: Option<&str>, out: &mut String) {
    if n.kind.starts_with('_') {
        for (f, c) in &n.children { sexp(c, f.as_deref().or(field), out); }
        return;
    }
    if !out.is_empty() && !out.ends_with('(') { out.push(' '); }
    if let Some(f) = field { out.push_str(f); out.push_str(": "); }
    out.push('(');
    out.push_str(n.kind);
    for (f, c) in &n.children { sexp(c, f.as_deref(), out); }
    out.push(')');
}

fn layout_nl(i: &mut In) -> ModalResult<()> {
    loop {
        let mut progressed = false;
        if let Ok(()) = run(((one_of(|c: char| matches!(c, ' ' | '\t')),).void()).void(), i) { progressed = true; continue; }
        { let s = pos(i); if let Ok(()) = run(((alt(((literal("#"),).void(), (literal("//"),).void(),)), star(none_of(|c: char| matches!(c, '\n' | '\r'))),).void()).void(), i) { let e = pos(i); i.state.comments.insert(s, (e, "comment")); progressed = true; continue; } }
        { let s = pos(i); if let Ok(()) = run(((literal("/*"), star(alt(((none_of(|c: char| matches!(c, '*')),).void(), (plus(one_of(|c: char| matches!(c, '*'))), none_of(|c: char| matches!(c, '*' | '/')),).void(),))), plus(one_of(|c: char| matches!(c, '*'))), literal("/"),).void()).void(), i) { let e = pos(i); i.state.comments.insert(s, (e, "block_comment")); progressed = true; continue; } }
        if !progressed { break; }
    }
    Ok(())
}
fn layout(i: &mut In) -> ModalResult<()> {
    let before = pos(i);
    loop {
        let mut progressed = false;
        if let Ok(()) = run(((one_of(|c: char| matches!(c, ' ' | '\t')),).void()).void(), i) { progressed = true; continue; }
        if let Ok(()) = run(((optional(one_of(|c: char| matches!(c, '\r'))), one_of(|c: char| matches!(c, '\n')),).void()).void(), i) { progressed = true; continue; }
        { let s = pos(i); if let Ok(()) = run(((alt(((literal("#"),).void(), (literal("//"),).void(),)), star(none_of(|c: char| matches!(c, '\n' | '\r'))),).void()).void(), i) { let e = pos(i); i.state.comments.insert(s, (e, "comment")); progressed = true; continue; } }
        { let s = pos(i); if let Ok(()) = run(((literal("/*"), star(alt(((none_of(|c: char| matches!(c, '*')),).void(), (plus(one_of(|c: char| matches!(c, '*'))), none_of(|c: char| matches!(c, '*' | '/')),).void(),))), plus(one_of(|c: char| matches!(c, '*'))), literal("/"),).void()).void(), i) { let e = pos(i); i.state.comments.insert(s, (e, "block_comment")); progressed = true; continue; } }
        if !progressed { break; }
    }
    let after = pos(i);
    if let Some(&limit) = i.state.offside.last() {
        if i.state.src[before..after].contains('\n') && after < i.state.src.len() && col(i, after) <= limit {
            return Err(bt());
        }
    }
    Ok(())
}
fn lxb_delim(i: &mut In) -> ModalResult<()> {
    run((plus(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))),).void(), i)?;
    run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?;
    Ok(())
}
fn lx_delim(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_delim(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("delim", start, end))
}
fn lxb_escape_sequence(i: &mut In) -> ModalResult<()> {
    run((literal("\\"), alt(((one_of(|c: char| matches!(c, 'n' | 'r' | 't' | '"' | '\\')),).void(), (literal("u"), lxb_hex, lxb_hex, lxb_hex, lxb_hex,).void(), (literal("U"), lxb_hex, lxb_hex, lxb_hex, lxb_hex, lxb_hex, lxb_hex, lxb_hex, lxb_hex,).void(),)),).void(), i)?;
    Ok(())
}
fn lx_escape_sequence(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_escape_sequence(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("escape_sequence", start, end))
}
fn lxb_float(i: &mut In) -> ModalResult<()> {
    run(alt(((plus(one_of(|c: char| matches!(c, '0'..='9'))), literal("."), plus(one_of(|c: char| matches!(c, '0'..='9'))), optional((one_of(|c: char| matches!(c, 'e' | 'E')), optional(one_of(|c: char| matches!(c, '-' | '+'))), plus(one_of(|c: char| matches!(c, '0'..='9'))),).void()),).void(), (plus(one_of(|c: char| matches!(c, '0'..='9'))), one_of(|c: char| matches!(c, 'e' | 'E')), optional(one_of(|c: char| matches!(c, '-' | '+'))), plus(one_of(|c: char| matches!(c, '0'..='9'))),).void(),)), i)?;
    run(not(one_of(|c: char| matches!(c, '0'..='9'))), i)?;
    Ok(())
}
fn lx_float(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_float(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("float", start, end))
}
fn lxb_heredoc_end(i: &mut In) -> ModalResult<()> {
    run((star(one_of(|c: char| matches!(c, ' ' | '\t'))), (|i: &mut In| -> ModalResult<()> { let s = pos(i); lxb_delim(i)?; i.state.cap = Some((s, pos(i))); Ok(()) }), star(one_of(|c: char| matches!(c, ' ' | '\t'))),).void(), i)?;
    run(not(one_of(|c: char| !matches!(c, '\n' | '\r'))), i)?;
    run(not(eof), i)?;
    run(not(one_of(|c: char| matches!(c, '\u{10ffff}'))), i)?;
    run(not(eof), i)?;
    Ok(())
}
fn lx_heredoc_end(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_heredoc_end(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("heredoc_end", start, end))
}
fn lxb_heredoc_start(i: &mut In) -> ModalResult<()> {
    run((literal("<<"), optional(literal("-")), (|i: &mut In| -> ModalResult<()> { let s = pos(i); lxb_delim(i)?; i.state.cap = Some((s, pos(i))); Ok(()) }), optional(one_of(|c: char| matches!(c, '\r'))), one_of(|c: char| matches!(c, '\n')),).void(), i)?;
    Ok(())
}
fn lx_heredoc_start(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_heredoc_start(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("heredoc_start", start, end))
}
fn lxb_hex(i: &mut In) -> ModalResult<()> {
    run((one_of(|c: char| matches!(c, '0'..='9' | 'a'..='f' | 'A'..='F')),).void(), i)?;
    Ok(())
}
fn lx_hex(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_hex(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("hex", start, end))
}
fn lxb_identifier(i: &mut In) -> ModalResult<()> {
    run((one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '_')), star(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))),).void(), i)?;
    run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?;
    Ok(())
}
fn lx_identifier(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_identifier(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("identifier", start, end))
}
fn lxb_integer(i: &mut In) -> ModalResult<()> {
    run((plus(one_of(|c: char| matches!(c, '0'..='9'))),).void(), i)?;
    run(not(one_of(|c: char| matches!(c, '0'..='9' | '.' | 'e' | 'E'))), i)?;
    Ok(())
}
fn lx_integer(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_integer(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("integer", start, end))
}
fn lxb_quote(i: &mut In) -> ModalResult<()> {
    run((literal("\""),).void(), i)?;
    Ok(())
}
fn lx_quote(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_quote(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("quote", start, end))
}
fn lxb_string_lit(i: &mut In) -> ModalResult<()> {
    run((literal("\""), star(alt(((none_of(|c: char| matches!(c, '"' | '\\' | '\r' | '\n')),).void(), (literal("\\"), one_of(|c: char| matches!(c, 'n' | 'r' | 't' | '"' | '\\')),).void(), (literal("\\u"), lxb_hex, lxb_hex, lxb_hex, lxb_hex,).void(), (literal("\\U"), lxb_hex, lxb_hex, lxb_hex, lxb_hex, lxb_hex, lxb_hex, lxb_hex, lxb_hex,).void(),))), literal("\""),).void(), i)?;
    Ok(())
}
fn lx_string_lit(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_string_lit(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("string_lit", start, end))
}
fn lxb__dir_else(i: &mut In) -> ModalResult<()> {
    run((literal("%{"), optional(literal("~")), star(one_of(|c: char| matches!(c, ' ' | '\t' | '\r' | '\n'))), literal("else"), star(one_of(|c: char| matches!(c, ' ' | '\t' | '\r' | '\n'))), optional(literal("~")), literal("}"),).void(), i)?;
    Ok(())
}
fn lx__dir_else(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__dir_else(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_dir_else", start, end))
}
fn lxb__dir_endfor(i: &mut In) -> ModalResult<()> {
    run((literal("%{"), optional(literal("~")), star(one_of(|c: char| matches!(c, ' ' | '\t' | '\r' | '\n'))), literal("endfor"), star(one_of(|c: char| matches!(c, ' ' | '\t' | '\r' | '\n'))), optional(literal("~")), literal("}"),).void(), i)?;
    Ok(())
}
fn lx__dir_endfor(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__dir_endfor(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_dir_endfor", start, end))
}
fn lxb__dir_endif(i: &mut In) -> ModalResult<()> {
    run((literal("%{"), optional(literal("~")), star(one_of(|c: char| matches!(c, ' ' | '\t' | '\r' | '\n'))), literal("endif"), star(one_of(|c: char| matches!(c, ' ' | '\t' | '\r' | '\n'))), optional(literal("~")), literal("}"),).void(), i)?;
    Ok(())
}
fn lx__dir_endif(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__dir_endif(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_dir_endif", start, end))
}
fn lxb__hchunk(i: &mut In) -> ModalResult<()> {
    run(alt(((star(alt(((lxb__htext,).void(), (lxb__qesc,).void(), (lxb__qsigil,).void(),))), optional(one_of(|c: char| matches!(c, '\r'))), one_of(|c: char| matches!(c, '\n')),).void(), (plus(alt(((lxb__htext,).void(), (lxb__qesc,).void(), (lxb__qsigil,).void(),))),).void(),)), i)?;
    Ok(())
}
fn lx__hchunk(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__hchunk(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_hchunk", start, end))
}
fn lxb__htext(i: &mut In) -> ModalResult<()> {
    run((none_of(|c: char| matches!(c, '$' | '%' | '\n' | '\r')),).void(), i)?;
    Ok(())
}
fn lx__htext(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__htext(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_htext", start, end))
}
fn lxb__legacy_key(i: &mut In) -> ModalResult<()> {
    run((literal("."), plus(one_of(|c: char| matches!(c, '0'..='9'))),).void(), i)?;
    Ok(())
}
fn lx__legacy_key(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__legacy_key(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_legacy_key", start, end))
}
fn lxb__nl(i: &mut In) -> ModalResult<()> {
    run((optional(one_of(|c: char| matches!(c, '\r'))), one_of(|c: char| matches!(c, '\n')), star((star(one_of(|c: char| matches!(c, ' ' | '\t'))), optional(one_of(|c: char| matches!(c, '\r'))), one_of(|c: char| matches!(c, '\n')),).void()),).void(), i)?;
    Ok(())
}
fn lx__nl(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__nl(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_nl", start, end))
}
fn lxb__qchunk(i: &mut In) -> ModalResult<()> {
    run((plus(alt(((lxb__qtext,).void(), (lxb__qesc,).void(), (lxb__qsigil,).void(),))),).void(), i)?;
    Ok(())
}
fn lx__qchunk(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__qchunk(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_qchunk", start, end))
}
fn lxb__qesc(i: &mut In) -> ModalResult<()> {
    run(alt(((literal("$${"),).void(), (literal("%%{"),).void(),)), i)?;
    Ok(())
}
fn lx__qesc(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__qesc(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_qesc", start, end))
}
fn lxb__qsigil(i: &mut In) -> ModalResult<()> {
    run((one_of(|c: char| matches!(c, '$' | '%')),).void(), i)?;
    run(not(one_of(|c: char| matches!(c, '{'))), i)?;
    Ok(())
}
fn lx__qsigil(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__qsigil(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_qsigil", start, end))
}
fn lxb__qtext(i: &mut In) -> ModalResult<()> {
    run((none_of(|c: char| matches!(c, '"' | '\\' | '$' | '%' | '\n' | '\r')),).void(), i)?;
    Ok(())
}
fn lx__qtext(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb__qtext(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("_qtext", start, end))
}
fn r_configfile(i: &mut In) -> ModalResult<Node> { r_configfile_prec(i, 0) }
fn r_configfile_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_0(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_0(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_0_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_0_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ let cp = save(i); 'g: { if let Ok(v) = (|i: &mut In| -> ModalResult<Vec<Node>> { let mut v: Vec<Node> = Vec::new(); layout(i)?; v.extend(vec![r_decl(i)?]); layout_nl(i)?; v.extend(vec![lx__nl(i)?]); Ok(v) })(i) { break 'g v; } restore(i, &cp); return Err(bt()); } }) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok(vec![r_decl(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "config_file", start, end, reach: end, children: ch })
}
fn r_decl(i: &mut In) -> ModalResult<Node> { r_decl_prec(i, 0) }
fn r_decl_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_1(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_2(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_1(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_1_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_1_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_attribute(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_2(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_2_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_2_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_block(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn r_attribute(i: &mut In) -> ModalResult<Node> { r_attribute_prec(i, 0) }
fn r_attribute_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_3(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_3(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_3_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_3_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("name"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("="), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("value"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "attribute", start, end, reach: end, children: ch })
}
fn r_block(i: &mut In) -> ModalResult<Node> { r_block_prec(i, 0) }
fn r_block_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_4(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_4(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_4_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_4_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("type"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r__label(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("label"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_body(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("body"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "block", start, end, reach: end, children: ch })
}
fn r__label(i: &mut In) -> ModalResult<Node> { r__label_prec(i, 0) }
fn r__label_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_5(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_6(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_5(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_5_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_5_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_6(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_6_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_6_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_string_lit(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn r_body(i: &mut In) -> ModalResult<Node> { r_body_prec(i, 0) }
fn r_body_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_7(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_8(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_7(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_7_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_7_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("{"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout_nl(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx__nl(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ let cp = save(i); 'g: { if let Ok(v) = (|i: &mut In| -> ModalResult<Vec<Node>> { let mut v: Vec<Node> = Vec::new(); layout(i)?; v.extend(vec![r_decl(i)?]); layout_nl(i)?; v.extend(vec![lx__nl(i)?]); Ok(v) })(i) { break 'g v; } restore(i, &cp); return Err(bt()); } }) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("}"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "body", start, end, reach: end, children: ch })
}
fn c_8(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_8_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_8_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("{"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok(vec![r_attribute(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("}"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "body", start, end, reach: end, children: ch })
}
fn r_name(i: &mut In) -> ModalResult<Node> { r_name_prec(i, 0) }
fn r_name_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_9(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_9(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_9_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_9_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_identifier(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn r_exp(i: &mut In) -> ModalResult<Node> { r_exp_prec(i, 0) }
fn r_exp_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_10(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_11(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_12(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_13(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_14(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_15(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_16(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_17(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_18(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_19(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_20(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    #[allow(unused_assignments)]
    let mut block: Option<u32> = None;
    loop {
        if 9 >= min && block != Some(9) { let cp = save(i); match t_43(i, &left, start) { Ok(n) => { left = n; block = if false { Some(9) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 9 >= min && block != Some(9) { let cp = save(i); match t_44(i, &left, start) { Ok(n) => { left = n; block = if false { Some(9) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 9 >= min && block != Some(9) { let cp = save(i); match t_45(i, &left, start) { Ok(n) => { left = n; block = if false { Some(9) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 9 >= min && block != Some(9) { let cp = save(i); match t_46(i, &left, start) { Ok(n) => { left = n; block = if false { Some(9) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 9 >= min && block != Some(9) { let cp = save(i); match t_47(i, &left, start) { Ok(n) => { left = n; block = if false { Some(9) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 7 >= min && block != Some(7) { let cp = save(i); match t_23(i, &left, start) { Ok(n) => { left = n; block = if false { Some(7) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 6 >= min && block != Some(6) { let cp = save(i); match t_24(i, &left, start) { Ok(n) => { left = n; block = if false { Some(6) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 5 >= min && block != Some(5) { let cp = save(i); match t_25(i, &left, start) { Ok(n) => { left = n; block = if false { Some(5) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 4 >= min && block != Some(4) { let cp = save(i); match t_26(i, &left, start) { Ok(n) => { left = n; block = if false { Some(4) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 3 >= min && block != Some(3) { let cp = save(i); match t_27(i, &left, start) { Ok(n) => { left = n; block = if false { Some(3) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 2 >= min && block != Some(2) { let cp = save(i); match t_28(i, &left, start) { Ok(n) => { left = n; block = if false { Some(2) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 1 >= min && block != Some(1) { let cp = save(i); match t_42(i, &left, start) { Ok(n) => { left = n; block = if false { Some(1) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        break;
    }
    let _ = block;
    let _ = min;
    Ok(left)
}
fn c_10(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_10_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_10_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_literal(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_11(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_11_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_11_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_identifier(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_12(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_12_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_12_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_quotedtemplate(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_13(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_13_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_13_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_heredoctemplate(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_14(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_14_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_14_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_tuple(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_15(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_15_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_15_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_object(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_16(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_16_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_16_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_fortupleexpr(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_17(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_17_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_17_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_forobjectexpr(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_18(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_18_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_18_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_functionname(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("function"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok(vec![r_arguments(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "function_call", start, end, reach: end, children: ch })
}
fn c_19(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_19_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_19_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "parenthesized_expression", start, end, reach: end, children: ch })
}
fn c_20(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_20_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_20_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__unop(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("operator"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 8)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("operand"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "unary_expression", start, end, reach: end, children: ch })
}
fn t_43(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_43_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_43_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("operand"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("."), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("name"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "get_attr", start, end, reach: end, children: ch })
}
fn t_44(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_44_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_44_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("operand"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("["), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("key"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("]"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "index", start, end, reach: end, children: ch })
}
fn t_45(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_45_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_45_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("operand"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx__legacy_key(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("key"), n)); } }
    if pr[0] && pr[1] && !((col(i, sp[0].1.saturating_sub(1).max(sp[0].0)) as i64) + (1) == (col(i, sp[1].0) as i64)) { return Err(bt()); }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "legacy_index", start, end, reach: end, children: ch })
}
fn t_46(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_46_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_46_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("operand"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("."), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("*"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r__splatname(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "attr_splat", start, end, reach: end, children: ch })
}
fn t_47(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_47_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_47_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("operand"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("["), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("*"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("]"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r__splatsuffix(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "full_splat", start, end, reach: end, children: ch })
}
fn t_23(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_23_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_23_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("left"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__binopmul(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("operator"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 8)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "binary_expression", start, end, reach: end, children: ch })
}
fn t_24(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_24_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_24_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("left"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__binopadd(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("operator"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 7)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "binary_expression", start, end, reach: end, children: ch })
}
fn t_25(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_25_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_25_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("left"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__binopcmp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("operator"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 6)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "binary_expression", start, end, reach: end, children: ch })
}
fn t_26(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_26_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_26_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("left"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__binopeq(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("operator"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 5)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "binary_expression", start, end, reach: end, children: ch })
}
fn t_27(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_27_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_27_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("left"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__binopand(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("operator"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 4)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "binary_expression", start, end, reach: end, children: ch })
}
fn t_28(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_28_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_28_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("left"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__binopor(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("operator"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 3)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "binary_expression", start, end, reach: end, children: ch })
}
fn t_42(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_42_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_42_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("condition"), left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("?"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("consequence"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(":"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 1)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("alternative"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "conditional", start, end, reach: end, children: ch })
}
fn r__unop(i: &mut In) -> ModalResult<Node> { r__unop_prec(i, 0) }
fn r__unop_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_21(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_22(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_21(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_21_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_21_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("-"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__un_op", start, end, reach: end, children: ch })
}
fn c_22(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_22_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_22_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("!"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__un_op", start, end, reach: end, children: ch })
}
fn r__binopmul(i: &mut In) -> ModalResult<Node> { r__binopmul_prec(i, 0) }
fn r__binopmul_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_29(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_30(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_31(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_29(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_29_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_29_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("*"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_mul", start, end, reach: end, children: ch })
}
fn c_30(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_30_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_30_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("/"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_mul", start, end, reach: end, children: ch })
}
fn c_31(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_31_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_31_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("%"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_mul", start, end, reach: end, children: ch })
}
fn r__binopadd(i: &mut In) -> ModalResult<Node> { r__binopadd_prec(i, 0) }
fn r__binopadd_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_32(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_33(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_32(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_32_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_32_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("+"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_add", start, end, reach: end, children: ch })
}
fn c_33(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_33_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_33_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("-"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_add", start, end, reach: end, children: ch })
}
fn r__binopcmp(i: &mut In) -> ModalResult<Node> { r__binopcmp_prec(i, 0) }
fn r__binopcmp_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_34(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_35(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_36(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_37(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_34(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_34_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_34_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(">"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_cmp", start, end, reach: end, children: ch })
}
fn c_35(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_35_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_35_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(">="), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_cmp", start, end, reach: end, children: ch })
}
fn c_36(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_36_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_36_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("<"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_cmp", start, end, reach: end, children: ch })
}
fn c_37(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_37_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_37_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("<="), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_cmp", start, end, reach: end, children: ch })
}
fn r__binopeq(i: &mut In) -> ModalResult<Node> { r__binopeq_prec(i, 0) }
fn r__binopeq_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_38(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_39(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_38(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_38_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_38_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("=="), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_eq", start, end, reach: end, children: ch })
}
fn c_39(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_39_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_39_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("!="), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_eq", start, end, reach: end, children: ch })
}
fn r__binopand(i: &mut In) -> ModalResult<Node> { r__binopand_prec(i, 0) }
fn r__binopand_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_40(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_40(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_40_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_40_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("&&"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_and", start, end, reach: end, children: ch })
}
fn r__binopor(i: &mut In) -> ModalResult<Node> { r__binopor_prec(i, 0) }
fn r__binopor_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_41(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_41(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_41_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_41_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("||"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__bin_op_or", start, end, reach: end, children: ch })
}
fn r__splatname(i: &mut In) -> ModalResult<Node> { r__splatname_prec(i, 0) }
fn r__splatname_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_48(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_48(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_48_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_48_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("."), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("name"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__splat_name", start, end, reach: end, children: ch })
}
fn r__splatsuffix(i: &mut In) -> ModalResult<Node> { r__splatsuffix_prec(i, 0) }
fn r__splatsuffix_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_49(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_50(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_49(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_49_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_49_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("."), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("name"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__splat_suffix", start, end, reach: end, children: ch })
}
fn c_50(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_50_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_50_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("["), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("key"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("]"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__splat_suffix", start, end, reach: end, children: ch })
}
fn r_literal(i: &mut In) -> ModalResult<Node> { r_literal_prec(i, 0) }
fn r_literal_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_51(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_52(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_53(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_54(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_55(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_51(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_51_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_51_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_integer(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_52(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_52_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_52_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_float(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_53(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_53_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_53_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("true"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "true", start, end, reach: end, children: ch })
}
fn c_54(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_54_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_54_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("false"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "false", start, end, reach: end, children: ch })
}
fn c_55(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_55_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_55_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("null"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "null", start, end, reach: end, children: ch })
}
fn r_functionname(i: &mut In) -> ModalResult<Node> { r_functionname_prec(i, 0) }
fn r_functionname_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_56(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_56(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_56_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_56_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal("::"), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_name(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "function_name", start, end, reach: end, children: ch })
}
fn r_arguments(i: &mut In) -> ModalResult<Node> { r_arguments_prec(i, 0) }
fn r_arguments_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_57(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_57(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_57_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_57_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_argument(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok({ let cp = save(i); 'g: { if let Ok(v) = (|i: &mut In| -> ModalResult<Vec<Node>> { let mut v: Vec<Node> = Vec::new(); layout(i)?; v.extend({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }); Ok(v) })(i) { break 'g v; } restore(i, &cp); if let Ok(v) = (|i: &mut In| -> ModalResult<Vec<Node>> { let mut v: Vec<Node> = Vec::new(); layout(i)?; v.extend(vec![r_ellipsis(i)?]); Ok(v) })(i) { break 'g v; } restore(i, &cp); return Err(bt()); } }) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "arguments", start, end, reach: end, children: ch })
}
fn r_argument(i: &mut In) -> ModalResult<Node> { r_argument_prec(i, 0) }
fn r_argument_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_58(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_58(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_58_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_58_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn r_ellipsis(i: &mut In) -> ModalResult<Node> { r_ellipsis_prec(i, 0) }
fn r_ellipsis_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_59(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_59(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_59_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_59_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("..."), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "ellipsis", start, end, reach: end, children: ch })
}
fn r_tuple(i: &mut In) -> ModalResult<Node> { r_tuple_prec(i, 0) }
fn r_tuple_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_60(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_60(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_60_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_60_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("["), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok({ let cp = save(i); 'g: { if let Ok(v) = (|i: &mut In| -> ModalResult<Vec<Node>> { let mut v: Vec<Node> = Vec::new(); layout(i)?; v.extend({ let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_exp(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v }); layout(i)?; v.extend({ let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } }); Ok(v) })(i) { break 'g v; } restore(i, &cp); return Err(bt()); } }) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("]"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "tuple", start, end, reach: end, children: ch })
}
fn r_object(i: &mut In) -> ModalResult<Node> { r_object_prec(i, 0) }
fn r_object_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_61(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_61(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_61_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_61_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("{"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout_nl(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout_nl(i)?; Ok(vec![lx__nl(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok(vec![r__objelems(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("}"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "object", start, end, reach: end, children: ch })
}
fn r__objelems(i: &mut In) -> ModalResult<Node> { r__objelems_prec(i, 0) }
fn r__objelems_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_62(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_62(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_62_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_62_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_objectelem(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout_nl(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if layout_nl(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ let cp = save(i); 'g: { if let Ok(v) = (|i: &mut In| -> ModalResult<Vec<Node>> { let mut v: Vec<Node> = Vec::new(); layout_nl(i)?; v.extend(vec![r__objsep(i)?]); layout(i)?; v.extend(vec![r_objectelem(i)?]); Ok(v) })(i) { break 'g v; } restore(i, &cp); return Err(bt()); } }) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout_nl(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout_nl(i)?; Ok(vec![r__objsep(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__obj_elems", start, end, reach: end, children: ch })
}
fn r__objsep(i: &mut In) -> ModalResult<Node> { r__objsep_prec(i, 0) }
fn r__objsep_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout_nl(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_63(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_64(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_63(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_63_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_63_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout_nl(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout_nl(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout_nl(i)?; Ok(vec![lx__nl(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__obj_sep", start, end, reach: end, children: ch })
}
fn c_64(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_64_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_64_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout_nl(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout_nl(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx__nl(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn r_objectelem(i: &mut In) -> ModalResult<Node> { r_objectelem_prec(i, 0) }
fn r_objectelem_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_65(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_65(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_65_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_65_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("key"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__objassign(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("value"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "object_elem", start, end, reach: end, children: ch })
}
fn r__objassign(i: &mut In) -> ModalResult<Node> { r__objassign_prec(i, 0) }
fn r__objassign_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_66(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_67(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_66(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_66_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_66_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("="), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__obj_assign", start, end, reach: end, children: ch })
}
fn c_67(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_67_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_67_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(":"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__obj_assign", start, end, reach: end, children: ch })
}
fn r_fortupleexpr(i: &mut In) -> ModalResult<Node> { r_fortupleexpr_prec(i, 0) }
fn r_fortupleexpr_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_68(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_68(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_68_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_68_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("["), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__forintro(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("result"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok(vec![r_forcond(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("condition"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("]"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "for_tuple_expr", start, end, reach: end, children: ch })
}
fn r_forobjectexpr(i: &mut In) -> ModalResult<Node> { r_forobjectexpr_prec(i, 0) }
fn r_forobjectexpr_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_69(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_69(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_69_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_69_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("{"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout_nl(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout_nl(i)?; Ok(vec![lx__nl(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__forintro(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("key"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("=>"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("value"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok(vec![r_ellipsis(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("grouping"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok(vec![r_forcond(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("condition"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("}"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "for_object_expr", start, end, reach: end, children: ch })
}
fn r__forintro(i: &mut In) -> ModalResult<Node> { r__forintro_prec(i, 0) }
fn r__forintro_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_70(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_70(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_70_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_70_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("for"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("binding"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok(vec![r__forsecond(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("in"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("collection"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(":"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__for_intro", start, end, reach: end, children: ch })
}
fn r__forsecond(i: &mut In) -> ModalResult<Node> { r__forsecond_prec(i, 0) }
fn r__forsecond_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_71(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_71(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_71_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_71_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("binding"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__for_second", start, end, reach: end, children: ch })
}
fn r_forcond(i: &mut In) -> ModalResult<Node> { r_forcond_prec(i, 0) }
fn r_forcond_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_72(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_72(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_72_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_72_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("if"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("condition"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "for_cond", start, end, reach: end, children: ch })
}
fn r_interp(i: &mut In) -> ModalResult<Node> { r_interp_prec(i, 0) }
fn r_interp_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_73(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_73(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_73_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_73_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__interpopen(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("expression"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__interpclose(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "template_interpolation", start, end, reach: end, children: ch })
}
fn r__interpopen(i: &mut In) -> ModalResult<Node> { r__interpopen_prec(i, 0) }
fn r__interpopen_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_74(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_75(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_74(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_74_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_74_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("${~"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__interp_open", start, end, reach: end, children: ch })
}
fn c_75(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_75_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_75_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("${"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__interp_open", start, end, reach: end, children: ch })
}
fn r__interpclose(i: &mut In) -> ModalResult<Node> { r__interpclose_prec(i, 0) }
fn r__interpclose_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_76(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_77(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_76(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_76_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_76_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("~}"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__interp_close", start, end, reach: end, children: ch })
}
fn c_77(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_77_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_77_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("}"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__interp_close", start, end, reach: end, children: ch })
}
fn r__diropen(i: &mut In) -> ModalResult<Node> { r__diropen_prec(i, 0) }
fn r__diropen_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_78(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_79(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_78(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_78_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_78_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("%{~"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__dir_open", start, end, reach: end, children: ch })
}
fn c_79(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_79_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_79_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("%{"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__dir_open", start, end, reach: end, children: ch })
}
fn r__dirclose(i: &mut In) -> ModalResult<Node> { r__dirclose_prec(i, 0) }
fn r__dirclose_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_80(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_81(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_80(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_80_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_80_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("~}"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__dir_close", start, end, reach: end, children: ch })
}
fn c_81(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_81_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_81_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    layout(i)?;
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("}"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__dir_close", start, end, reach: end, children: ch })
}
fn r__dirif(i: &mut In) -> ModalResult<Node> { r__dirif_prec(i, 0) }
fn r__dirif_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_82(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_82(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_82_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_82_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__diropen(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("if"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("condition"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__dirclose(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__dir_if", start, end, reach: end, children: ch })
}
fn r__dirfor(i: &mut In) -> ModalResult<Node> { r__dirfor_prec(i, 0) }
fn r__dirfor_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_83(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_83(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_83_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_83_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__diropen(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("for"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("binding"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } layout(i)?; Ok(vec![r__forsecond(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("in"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("collection"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__dirclose(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "_inj__dir_for", start, end, reach: end, children: ch })
}
fn r__qpart(i: &mut In) -> ModalResult<Node> { r__qpart_prec(i, 0) }
fn r__qpart_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_84(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_85(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_86(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_87(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_84(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_84_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_84_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_qlit(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_85(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_85_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_85_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_interp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_86(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_86_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_86_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_qif(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_87(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_87_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_87_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_qfor(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn r__hpart(i: &mut In) -> ModalResult<Node> { r__hpart_prec(i, 0) }
fn r__hpart_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_88(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_89(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_90(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_91(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_88(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_88_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_88_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_hlit(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_89(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_89_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_89_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_interp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_90(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_90_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_90_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_hif(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_91(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_91_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_91_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_hfor(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn r_quotedtemplate(i: &mut In) -> ModalResult<Node> { r_quotedtemplate_prec(i, 0) }
fn r_quotedtemplate_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_92(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_92(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_92_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_92_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_quote(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if no_layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r__qpart(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_quote(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "quoted_template", start, end, reach: end, children: ch })
}
fn r_qlit(i: &mut In) -> ModalResult<Node> { r_qlit_prec(i, 0) }
fn r_qlit_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_93(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_93(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_93_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_93_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if no_layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ let cp = save(i); 'g: { if let Ok(v) = (|i: &mut In| -> ModalResult<Vec<Node>> { let mut v: Vec<Node> = Vec::new(); no_layout(i)?; v.extend(vec![lx__qchunk(i)?]); Ok(v) })(i) { break 'g v; } restore(i, &cp); if let Ok(v) = (|i: &mut In| -> ModalResult<Vec<Node>> { let mut v: Vec<Node> = Vec::new(); no_layout(i)?; v.extend(vec![lx_escape_sequence(i)?]); Ok(v) })(i) { break 'g v; } restore(i, &cp); return Err(bt()); } }) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "template_literal", start, end, reach: end, children: ch })
}
fn r_qif(i: &mut In) -> ModalResult<Node> { r_qif_prec(i, 0) }
fn r_qif_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_94(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_94(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_94_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_94_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__dirif(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } no_layout(i)?; Ok(vec![r_qbody(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("consequence"), n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } no_layout(i)?; Ok(vec![r_qelse(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx__dir_endif(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "template_if", start, end, reach: end, children: ch })
}
fn r_qelse(i: &mut In) -> ModalResult<Node> { r_qelse_prec(i, 0) }
fn r_qelse_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_95(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_95(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_95_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_95_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx__dir_else(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } no_layout(i)?; Ok(vec![r_qbody(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("alternative"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "else_clause", start, end, reach: end, children: ch })
}
fn r_qfor(i: &mut In) -> ModalResult<Node> { r_qfor_prec(i, 0) }
fn r_qfor_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_96(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_96(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_96_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_96_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__dirfor(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } no_layout(i)?; Ok(vec![r_qbody(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("body"), n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx__dir_endfor(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "template_for", start, end, reach: end, children: ch })
}
fn r_qbody(i: &mut In) -> ModalResult<Node> { r_qbody_prec(i, 0) }
fn r_qbody_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_97(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_97(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_97_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_97_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if no_layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r__qpart(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "template_body", start, end, reach: end, children: ch })
}
fn r_heredoctemplate(i: &mut In) -> ModalResult<Node> { r_heredoctemplate_prec(i, 0) }
fn r_heredoctemplate_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_98(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_98(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_98_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_98_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_heredoc_start(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    open_word = i.state.cap.map(|(s, e)| i.state.src[s..e].to_string()).unwrap_or_default();
    i.state.delim.push((lx_heredoc_end, open_word.clone()));
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if no_layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r__hpart(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    i.state.delim.pop();
    if col(i, pos(i)) != 0 { return Err(bt()); }
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_heredoc_end(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    if i.state.cap.map(|(s, e)| &i.state.src[s..e] != open_word.as_str()).unwrap_or(true) { return Err(bt()); }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "heredoc_template", start, end, reach: end, children: ch })
}
fn r_hlit(i: &mut In) -> ModalResult<Node> { r_hlit_prec(i, 0) }
fn r_hlit_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_99(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_99(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_99_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_99_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if no_layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![lx__hchunk(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "template_literal", start, end, reach: end, children: ch })
}
fn r_hif(i: &mut In) -> ModalResult<Node> { r_hif_prec(i, 0) }
fn r_hif_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_100(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_100(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_100_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_100_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__dirif(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } no_layout(i)?; Ok(vec![r_hbody(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("consequence"), n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } no_layout(i)?; Ok(vec![r_helse(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx__dir_endif(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "template_if", start, end, reach: end, children: ch })
}
fn r_helse(i: &mut In) -> ModalResult<Node> { r_helse_prec(i, 0) }
fn r_helse_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_101(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_101(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_101_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_101_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx__dir_else(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } no_layout(i)?; Ok(vec![r_hbody(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("alternative"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "else_clause", start, end, reach: end, children: ch })
}
fn r_hfor(i: &mut In) -> ModalResult<Node> { r_hfor_prec(i, 0) }
fn r_hfor_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_102(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_102(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_102_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_102_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = vec![r__dirfor(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> { if closer_here(i) { return Err(bt()); } no_layout(i)?; Ok(vec![r_hbody(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("body"), n)); } }
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx__dir_endfor(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "template_for", start, end, reach: end, children: ch })
}
fn r_hbody(i: &mut In) -> ModalResult<Node> { r_hbody_prec(i, 0) }
fn r_hbody_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_103(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_103(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_103_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_103_body(i: &mut In) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    let start = pos(i);
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i); if closer_here(i) { restore(i, &cp); break; } if no_layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r__hpart(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "template_body", start, end, reach: end, children: ch })
}
fn parse_root(src: &str) -> Result<Node, usize> {
    let mut i: In = Stateful { input: LocatingSlice::new(src), state: St::new(src) };
    let r = (|i: &mut In| -> ModalResult<Node> { let n = r_configfile(i)?; layout(i)?; run(eof, i)?; Ok(n) })(&mut i);
    match r {
        Ok(mut root) => { root.start = 0; root.end = src.len(); root.reach = src.len(); let comments: Vec<(usize, usize, &'static str)> = i.state.comments.iter().map(|(s, (e, n))| (*s, *e, *n)).collect(); for (s, e, n) in comments { attach(&mut root, Node::leaf(n, s, e)); } Ok(root) }
        Err(_) => Err(i.state.furthest.max(pos(&i))),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let src = match args.first() {
        Some(p) => std::fs::read_to_string(p).expect("read"),
        None => { let mut s = String::new(); std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).expect("stdin"); s }
    };
    match parse_root(&src) {
        Ok(root) => { let mut out = String::new(); sexp(&root, None, &mut out); println!("{out}"); }
        Err(at) => {
            let line = src[..at].matches('\n').count() + 1;
            let col = at - src[..at].rfind('\n').map(|k| k + 1).unwrap_or(0);
            println!("ERROR at {line}:{col}");
            std::process::exit(1);
        }
    }
}

