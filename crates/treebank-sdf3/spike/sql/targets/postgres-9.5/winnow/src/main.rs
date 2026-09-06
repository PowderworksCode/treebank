// GENERATED from postgres/9.5.sdf3 by treebank-sdf3's winnow backend. Do not edit.

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
        if let Ok(()) = run(((one_of(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r')),).void()).void(), i) { progressed = true; continue; }
        { let s = pos(i); if let Ok(()) = run(((literal("--"), star(none_of(|c: char| matches!(c, '\n' | '\r'))),).void()).void(), i) { let e = pos(i); i.state.comments.insert(s, (e, "comment")); progressed = true; continue; } }
        if !progressed { break; }
    }
    Ok(())
}
fn layout(i: &mut In) -> ModalResult<()> {
    let before = pos(i);
    loop {
        let mut progressed = false;
        if let Ok(()) = run(((one_of(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r')),).void()).void(), i) { progressed = true; continue; }
        { let s = pos(i); if let Ok(()) = run(((literal("--"), star(none_of(|c: char| matches!(c, '\n' | '\r'))),).void()).void(), i) { let e = pos(i); i.state.comments.insert(s, (e, "comment")); progressed = true; continue; } }
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
fn lxb_dollar(i: &mut In) -> ModalResult<()> {
    run((literal("$$"), star(none_of(|c: char| matches!(c, '$'))), literal("$$"),).void(), i)?;
    Ok(())
}
fn lx_dollar(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_dollar(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_ci(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("dollar", start, end))
}
fn lxb_dquoted(i: &mut In) -> ModalResult<()> {
    run((literal("\""), star(none_of(|c: char| matches!(c, '"'))), literal("\""),).void(), i)?;
    Ok(())
}
fn lx_dquoted(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_dquoted(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_ci(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("dquoted", start, end))
}
fn lxb_int(i: &mut In) -> ModalResult<()> {
    run((plus(one_of(|c: char| matches!(c, '0'..='9'))),).void(), i)?;
    run(not(one_of(|c: char| matches!(c, '0'..='9'))), i)?;
    Ok(())
}
fn lx_int(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_int(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_ci(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("int", start, end))
}
fn lxb_name(i: &mut In) -> ModalResult<()> {
    run((one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '_')), star(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))),).void(), i)?;
    run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?;
    Ok(())
}
fn lx_name(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_name(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &["AND", "AS", "ASC", "BY", "CONFLICT", "CREATE", "DELETE", "DESC", "DO", "DROP", "FROM", "ILIKE", "INSERT", "INT", "INTO", "LIKE", "LIMIT", "NOT", "NOTHING", "NULL", "OFFSET", "OIDS", "ON", "OR", "ORDER", "OVER", "PARTITION", "RETURNING", "SELECT", "SET", "TABLE", "TEXT", "UPDATE", "VALUES", "VARCHAR", "WHERE", "WITH", "WITHOUT"];
    if REJECT.iter().any(|k| eq_ci(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("name", start, end))
}
fn lxb_string(i: &mut In) -> ModalResult<()> {
    run(alt(((literal("'"), star(alt(((literal("''"),).void(), (none_of(|c: char| matches!(c, '\'')),).void(),))), literal("'"),).void(), (lxb_dollar,).void(),)), i)?;
    Ok(())
}
fn lx_string(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_string(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &[];
    if REJECT.iter().any(|k| eq_ci(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("string", start, end))
}
fn r_script(i: &mut In) -> ModalResult<Node> { r_script_prec(i, 0) }
fn r_script_prec(i: &mut In, min: u32) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i);  if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_stmt(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "script", start, end, reach: end, children: ch })
}
fn r_stmt(i: &mut In) -> ModalResult<Node> { r_stmt_prec(i, 0) }
fn r_stmt_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_1(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_13(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_14(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_15(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_17(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_22(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
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
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_with(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("with"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_query(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "stmt_select", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("INSERT")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("INTO")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("table"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_ident(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("columns"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("VALUES")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_exp(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("values"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_upsert(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("upsert"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_returning(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("returning"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "insert", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("UPDATE")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("table"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("SET")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_assign(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_where(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("where"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_returning(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("returning"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "update", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("DELETE")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("FROM")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("table"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_where(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("where"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_returning(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("returning"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "delete", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("CREATE")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("TABLE")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("table"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_coldef(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_createtail(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("tail"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "create_table", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("DROP")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("TABLE")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("table"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "drop_table", start, end, reach: end, children: ch })
}
fn r_query(i: &mut In) -> ModalResult<Node> { r_query_prec(i, 0) }
fn r_query_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_2(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("SELECT")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_item(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("items"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_from(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("from"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_where(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("where"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_orderby(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("order"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_limit(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("limit"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_offset(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("offset"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "select", start, end, reach: end, children: ch })
}
fn r_item(i: &mut In) -> ModalResult<Node> { r_item_prec(i, 0) }
fn r_item_prec(i: &mut In, min: u32) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_alias(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("alias"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "item", start, end, reach: end, children: ch })
}
fn r_alias(i: &mut In) -> ModalResult<Node> { r_alias_prec(i, 0) }
fn r_alias_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_4(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_5(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("AS")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "as", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "bare", start, end, reach: end, children: ch })
}
fn r_from(i: &mut In) -> ModalResult<Node> { r_from_prec(i, 0) }
fn r_from_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_6(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("FROM")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("table"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "from", start, end, reach: end, children: ch })
}
fn r_where(i: &mut In) -> ModalResult<Node> { r_where_prec(i, 0) }
fn r_where_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_7(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("WHERE")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "where", start, end, reach: end, children: ch })
}
fn r_orderby(i: &mut In) -> ModalResult<Node> { r_orderby_prec(i, 0) }
fn r_orderby_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_8(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("ORDER")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("BY")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_orderitem(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "order_by", start, end, reach: end, children: ch })
}
fn r_orderitem(i: &mut In) -> ModalResult<Node> { r_orderitem_prec(i, 0) }
fn r_orderitem_prec(i: &mut In, min: u32) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_dir(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("dir"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "order", start, end, reach: end, children: ch })
}
fn r_dir(i: &mut In) -> ModalResult<Node> { r_dir_prec(i, 0) }
fn r_dir_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_10(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_11(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("ASC")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "asc", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("DESC")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "desc", start, end, reach: end, children: ch })
}
fn r_cte(i: &mut In) -> ModalResult<Node> { r_cte_prec(i, 0) }
fn r_cte_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_12(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("name"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("AS")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_query(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "cte", start, end, reach: end, children: ch })
}
fn r_assign(i: &mut In) -> ModalResult<Node> { r_assign_prec(i, 0) }
fn r_assign_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_16(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("column"), n)); } }
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
    Ok(Node { kind: "assign", start, end, reach: end, children: ch })
}
fn r_coldef(i: &mut In) -> ModalResult<Node> { r_coldef_prec(i, 0) }
fn r_coldef_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_18(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("name"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_type(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "col_def", start, end, reach: end, children: ch })
}
fn r_type(i: &mut In) -> ModalResult<Node> { r_type_prec(i, 0) }
fn r_type_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_19(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_20(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_21(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("INT")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "type_int", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("VARCHAR")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_int(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "varchar", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("TEXT")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "text", start, end, reach: end, children: ch })
}
fn r_ident(i: &mut In) -> ModalResult<Node> { r_ident_prec(i, 0) }
fn r_ident_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_23(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_50(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_23(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_23_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_23_body(i: &mut In) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![lx_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "ident_name", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = vec![lx_dquoted(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "quoted", start, end, reach: end, children: ch })
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
    restore(i, &cp); if let Ok(n) = c_24(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_25(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_26(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_27(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_28(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_29(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_30(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_31(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_39(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_42(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    #[allow(unused_assignments)]
    let mut block: Option<u32> = None;
    loop {
        if 16 >= min && block != Some(16) { let cp = save(i); match t_32(i, &left, start) { Ok(n) => { left = n; block = if false { Some(16) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 15 >= min && block != Some(15) { let cp = save(i); match t_33(i, &left, start) { Ok(n) => { left = n; block = if false { Some(15) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 15 >= min && block != Some(15) { let cp = save(i); match t_34(i, &left, start) { Ok(n) => { left = n; block = if false { Some(15) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 14 >= min && block != Some(14) { let cp = save(i); match t_35(i, &left, start) { Ok(n) => { left = n; block = if true { Some(14) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 14 >= min && block != Some(14) { let cp = save(i); match t_36(i, &left, start) { Ok(n) => { left = n; block = if true { Some(14) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 14 >= min && block != Some(14) { let cp = save(i); match t_37(i, &left, start) { Ok(n) => { left = n; block = if true { Some(14) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 14 >= min && block != Some(14) { let cp = save(i); match t_38(i, &left, start) { Ok(n) => { left = n; block = if true { Some(14) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 12 >= min && block != Some(12) { let cp = save(i); match t_40(i, &left, start) { Ok(n) => { left = n; block = if false { Some(12) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 11 >= min && block != Some(11) { let cp = save(i); match t_41(i, &left, start) { Ok(n) => { left = n; block = if false { Some(11) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 10 >= min && block != Some(10) { let cp = save(i); match t_46(i, &left, start) { Ok(n) => { left = n; block = if false { Some(10) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 8 >= min && block != Some(8) { let cp = save(i); match t_48(i, &left, start) { Ok(n) => { left = n; block = if false { Some(8) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 8 >= min && block != Some(8) { let cp = save(i); match t_49(i, &left, start) { Ok(n) => { left = n; block = if false { Some(8) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 6 >= min && block != Some(6) { let cp = save(i); match t_51(i, &left, start) { Ok(n) => { left = n; block = if false { Some(6) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 4 >= min && block != Some(4) { let cp = save(i); match t_52(i, &left, start) { Ok(n) => { left = n; block = if true { Some(4) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        break;
    }
    let _ = block;
    let _ = min;
    Ok(left)
}
fn c_24(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_24_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_24_body(i: &mut In) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
}
fn c_25(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_25_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_25_body(i: &mut In) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("table"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("."), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_ident(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("column"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "column", start, end, reach: end, children: ch })
}
fn c_26(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_26_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_26_body(i: &mut In) -> ModalResult<Node> {
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
    Ok(Node { kind: "star", start, end, reach: end, children: ch })
}
fn c_27(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_27_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_27_body(i: &mut In) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![lx_int(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "exp_int", start, end, reach: end, children: ch })
}
fn c_28(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_28_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_28_body(i: &mut In) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![lx_string(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "str", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("NULL")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "null", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = vec![lx_name(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("function"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_exp(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("arguments"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "call", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("-"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 17)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "neg", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("NOT")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 13)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "not", start, end, reach: end, children: ch })
}
fn c_42(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_42_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_42_body(i: &mut In) -> ModalResult<Node> {
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
    Ok(Node { kind: "exp_bracket", start, end, reach: end, children: ch })
}
fn t_32(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_32_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_32_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("*"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 17)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "mul", start, end, reach: end, children: ch })
}
fn t_33(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_33_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_33_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("+"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 16)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "add", start, end, reach: end, children: ch })
}
fn t_34(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_34_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_34_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("-"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 16)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "sub", start, end, reach: end, children: ch })
}
fn t_35(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_35_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_35_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("="), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 15)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "eq", start, end, reach: end, children: ch })
}
fn t_36(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_36_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_36_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("<"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 15)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "lt", start, end, reach: end, children: ch })
}
fn t_37(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_37_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_37_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(">"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 15)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "gt", start, end, reach: end, children: ch })
}
fn t_38(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_38_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_38_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("LIKE")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 15)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "like", start, end, reach: end, children: ch })
}
fn t_40(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_40_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_40_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("AND")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 13)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "and", start, end, reach: end, children: ch })
}
fn t_41(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_41_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_41_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("OR")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 12)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "or", start, end, reach: end, children: ch })
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
    ch.push((None, left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("OVER")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_partition(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("partition"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_orderby(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("order"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "over", start, end, reach: end, children: ch })
}
fn t_48(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_48_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_48_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("->"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 9)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "arrow", start, end, reach: end, children: ch })
}
fn t_49(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_49_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_49_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("->>"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 9)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "arrow_text", start, end, reach: end, children: ch })
}
fn t_51(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_51_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_51_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((None, left.clone()));
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("::"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_type(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "cast", start, end, reach: end, children: ch })
}
fn t_52(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_52_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_52_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("ILIKE")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 5)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "i_like", start, end, reach: end, children: ch })
}
fn r_limit(i: &mut In) -> ModalResult<Node> { r_limit_prec(i, 0) }
fn r_limit_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_43(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_43(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_43_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_43_body(i: &mut In) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("LIMIT")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_int(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("count"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "limit", start, end, reach: end, children: ch })
}
fn r_offset(i: &mut In) -> ModalResult<Node> { r_offset_prec(i, 0) }
fn r_offset_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_44(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_44(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_44_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_44_body(i: &mut In) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("OFFSET")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_int(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("start"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "offset", start, end, reach: end, children: ch })
}
fn r_with(i: &mut In) -> ModalResult<Node> { r_with_prec(i, 0) }
fn r_with_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_45(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_45(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_45_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_45_body(i: &mut In) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("WITH")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_cte(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "with", start, end, reach: end, children: ch })
}
fn r_partition(i: &mut In) -> ModalResult<Node> { r_partition_prec(i, 0) }
fn r_partition_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_47(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
}
fn c_47(i: &mut In) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = c_47_body(i);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn c_47_body(i: &mut In) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("PARTITION")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("BY")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_exp(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "partition", start, end, reach: end, children: ch })
}
fn r_returning(i: &mut In) -> ModalResult<Node> { r_returning_prec(i, 0) }
fn r_returning_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_53(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("RETURNING")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_item(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "returning", start, end, reach: end, children: ch })
}
fn r_createtail(i: &mut In) -> ModalResult<Node> { r_createtail_prec(i, 0) }
fn r_createtail_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_54(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_55(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("WITH")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("OIDS")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "with_oids", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("WITHOUT")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("OIDS")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "without_oids", start, end, reach: end, children: ch })
}
fn r_upsert(i: &mut In) -> ModalResult<Node> { r_upsert_prec(i, 0) }
fn r_upsert_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_56(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_57(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("ON")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("CONFLICT")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("DO")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("NOTHING")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "nothing", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("ON")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("CONFLICT")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_ident(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("DO")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("UPDATE")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(Caseless("SET")), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_assign(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 1 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "upsert_update", start, end, reach: end, children: ch })
}
fn parse_root(src: &str) -> Result<Node, usize> {
    let mut i: In = Stateful { input: LocatingSlice::new(src), state: St::new(src) };
    let r = (|i: &mut In| -> ModalResult<Node> { let n = r_script(i)?; layout(i)?; run(eof, i)?; Ok(n) })(&mut i);
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

