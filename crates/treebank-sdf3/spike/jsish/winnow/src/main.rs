// GENERATED from jsish.sdf3 by treebank-sdf3's winnow backend. Do not edit.

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
        { let s = pos(i); if let Ok(()) = run(((literal("//"), star(none_of(|c: char| matches!(c, '\n' | '\r'))),).void()).void(), i) { let e = pos(i); i.state.comments.insert(s, (e, "comment")); progressed = true; continue; } }
        if !progressed { break; }
    }
    Ok(())
}
fn layout(i: &mut In) -> ModalResult<()> {
    let before = pos(i);
    loop {
        let mut progressed = false;
        if let Ok(()) = run(((one_of(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r')),).void()).void(), i) { progressed = true; continue; }
        { let s = pos(i); if let Ok(()) = run(((literal("//"), star(none_of(|c: char| matches!(c, '\n' | '\r'))),).void()).void(), i) { let e = pos(i); i.state.comments.insert(s, (e, "comment")); progressed = true; continue; } }
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
fn lxb_id(i: &mut In) -> ModalResult<()> {
    run((one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '$')), star(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))),).void(), i)?;
    run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))), i)?;
    Ok(())
}
fn lx_id(i: &mut In) -> ModalResult<Node> {
    let start = pos(i);
    i.state.furthest = i.state.furthest.max(start);
    i.state.cap = None;
    lxb_id(i)?;
    let end = pos(i);
    let text = &i.state.src[start..end];
    const REJECT: &[&str] = &["else", "function", "if", "let", "return", "var", "while"];
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("id", start, end))
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
    if REJECT.iter().any(|k| eq_cs(k, text)) { return Err(bt()); }
    token_end(i, end);
    Ok(Node::leaf("int", start, end))
}
fn r_program(i: &mut In) -> ModalResult<Node> { r_program_prec(i, 0) }
fn r_program_prec(i: &mut In, min: u32) -> ModalResult<Node> {
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
    Ok(Node { kind: "program", start, end, reach: end, children: ch })
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
    restore(i, &cp); if let Ok(n) = c_2(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_3(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_4(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_5(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_6(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_7(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_8(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_9(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_10(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("function"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_id(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("name"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; let mut first = true; loop { let cp = save(i);  if !first { if layout(i).is_err() { restore(i, &cp); break; } if (|i: &mut In| -> ModalResult<Vec<Node>> { Ok({ i.state.furthest = i.state.furthest.max(pos(i)); run(literal(","), i)?;  let e = pos(i); token_end(i, e); Vec::new() }) })(i).is_err() { restore(i, &cp); break; } } if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_param(i)?]) })(i) { Ok(ns) => { v.extend(ns); first = false; } Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("parameters"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_block(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("body"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "function", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("var"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_id(i)?];
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
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "var", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("let"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![lx_id(i)?];
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
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "let", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = vec![lx_id(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("target"), n)); } }
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
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "assign", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("console.log"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("value"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "print", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("return"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("value"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "return", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("if"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("condition"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_block(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("consequence"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let cp = save(i); match (|i: &mut In| -> ModalResult<Vec<Node>> {  layout(i)?; Ok(vec![r_else(i)?]) })(i) { Ok(v) => v, Err(_) => { restore(i, &cp); Vec::new() } } };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("alternative"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "if", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("while"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("("), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("condition"), n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(")"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_block(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("body"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "while", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal(";"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "expr", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = vec![r_block(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let (_, n) = ch.pop().unwrap();
    Ok(n)
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
    restore(i, &cp); if let Ok(n) = c_11(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("{"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { let mut v: Vec<Node> = Vec::new(); #[allow(unused_mut, unused_variables)] let mut col0: Option<usize> = None; loop { let cp = save(i);  if layout(i).is_err() { restore(i, &cp); break; }  match (|i: &mut In| -> ModalResult<Vec<Node>> { Ok(vec![r_stmt(i)?]) })(i) { Ok(ns) => v.extend(ns), Err(_) => { restore(i, &cp); break; } } } if v.len() < 0 { return Err(bt()); } v };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("}"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "block", start, end, reach: end, children: ch })
}
fn r_else(i: &mut In) -> ModalResult<Node> { r_else_prec(i, 0) }
fn r_else_prec(i: &mut In, min: u32) -> ModalResult<Node> {
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("else"), i)?; run(not(one_of(|c: char| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$'))), i)?; let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_block(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("body"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "else_clause", start, end, reach: end, children: ch })
}
fn r_param(i: &mut In) -> ModalResult<Node> { r_param_prec(i, 0) }
fn r_param_prec(i: &mut In, min: u32) -> ModalResult<Node> {
    layout(i)?;
    let start = pos(i);
    let cp = save(i);
    // Longest match among the primaries: ordered choice would let an
    // injection `Exp = ID` shadow `Exp.Call = ID(..)`; ties go to the
    // first in prefer/source order.
    let mut best: Option<(usize, Node, Cp)> = None;
    restore(i, &cp); if let Ok(n) = c_13(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = vec![lx_id(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("name"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "param", start, end, reach: end, children: ch })
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
    restore(i, &cp); if let Ok(n) = c_14(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_15(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_17(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    restore(i, &cp); if let Ok(n) = c_22(i) { let e = pos(i); if best.as_ref().map_or(true, |b| e > b.0) { best = Some((e, n, save(i))); } }
    let mut left: Node = match best { Some((_, n, at)) => { restore(i, &at); n } None => { restore(i, &cp); return Err(bt()); } };
    #[allow(unused_assignments)]
    let mut block: Option<u32> = None;
    loop {
        if 5 >= min && block != Some(5) { let cp = save(i); match t_16(i, &left, start) { Ok(n) => { left = n; block = if false { Some(5) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 3 >= min && block != Some(3) { let cp = save(i); match t_18(i, &left, start) { Ok(n) => { left = n; block = if false { Some(3) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 2 >= min && block != Some(2) { let cp = save(i); match t_19(i, &left, start) { Ok(n) => { left = n; block = if false { Some(2) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 2 >= min && block != Some(2) { let cp = save(i); match t_20(i, &left, start) { Ok(n) => { left = n; block = if false { Some(2) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        if 1 >= min && block != Some(1) { let cp = save(i); match t_21(i, &left, start) { Ok(n) => { left = n; block = if true { Some(1) } else { None }; continue; } Err(_) => restore(i, &cp) } }
        break;
    }
    let _ = block;
    let _ = min;
    Ok(left)
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
      let ns: Vec<Node> = vec![lx_id(i)?];
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
      let ns: Vec<Node> = vec![lx_int(i)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "exp_int", start, end, reach: end, children: ch })
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
      let ns: Vec<Node> = { i.state.furthest = i.state.furthest.max(pos(i)); run(literal("-"), i)?;  let e = pos(i); token_end(i, e); Vec::new() };
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((None, n)); } }
    layout(i)?;
    { let s = pos(i);
      let ns: Vec<Node> = vec![r_exp_prec(i, 4)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("operand"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "neg", start, end, reach: end, children: ch })
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
fn t_16(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_16_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_16_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    #[allow(unused_mut)]
    let mut ch: Vec<(Option<&'static str>, Node)> = Vec::new();
    #[allow(unused_mut)]
    let mut sp: Vec<(usize, usize)> = Vec::new();
    #[allow(unused_mut)]
    let mut pr: Vec<bool> = Vec::new();
    #[allow(unused_mut)]
    let mut open_word: String = String::new();
    sp.push((left.start, left.end)); pr.push(true);
    ch.push((Some("function"), left.clone()));
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
fn t_18(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_18_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_18_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![r_exp_prec(i, 4)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "mul", start, end, reach: end, children: ch })
}
fn t_19(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_19_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_19_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![r_exp_prec(i, 3)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "add", start, end, reach: end, children: ch })
}
fn t_20(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_20_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_20_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![r_exp_prec(i, 3)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "sub", start, end, reach: end, children: ch })
}
fn t_21(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
    let guard = i.state.offside.len();
    let dguard = i.state.delim.len();
    let r = t_21_body(i, left, start);
    i.state.offside.truncate(guard);
    if r.is_err() { i.state.delim.truncate(dguard); }
    r
}
fn t_21_body(i: &mut In, left: &Node, start: usize) -> ModalResult<Node> {
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
      let ns: Vec<Node> = vec![r_exp_prec(i, 2)?];
      let e = i.state.last_end.max(s); sp.push((s, e)); pr.push(e > s);
      for n in ns { ch.push((Some("right"), n)); } }
    let _ = (&sp, &pr, &open_word);
    let end = i.state.last_end.max(start);
    Ok(Node { kind: "lt", start, end, reach: end, children: ch })
}
fn parse_root(src: &str) -> Result<Node, usize> {
    let mut i: In = Stateful { input: LocatingSlice::new(src), state: St::new(src) };
    let r = (|i: &mut In| -> ModalResult<Node> { let n = r_program(i)?; layout(i)?; run(eof, i)?; Ok(n) })(&mut i);
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

