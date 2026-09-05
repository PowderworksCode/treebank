//! A reader for SDF3 modules, written with winnow.
//!
//! The subset is what `spike/mini/mini.sdf3` exercises plus the obvious
//! neighbours: every section kind Spoofax documents, both production forms
//! (productive and template), `{Elem Sep}*` lists, grouped alternatives,
//! character classes with SDF's backslash escaping, attributes, priority
//! chains with associativity groups, restrictions with dotted lookaheads,
//! and template options. Anything outside that fails to parse rather than
//! being skipped, so an unsupported construct is a loud error at the read
//! rather than a silent hole in the grammar.

use winnow::ascii::{dec_uint, multispace1};
use winnow::combinator::{
    alt, cut_err, delimited, eof, fail, opt, peek, preceded, repeat, separated, terminated,
};
use winnow::error::{ContextError, ErrMode, StrContext, StrContextValue};
use winnow::prelude::*;
use winnow::token::{any, literal, take_until, take_while};

use crate::ast::*;

type In<'a> = &'a str;
type R<T> = ModalResult<T, ContextError>;

pub fn parse_module(text: &str) -> anyhow::Result<Module> {
    let mut input = text;
    match module.parse_next(&mut input) {
        Ok(mut m) => {
            apply_tokenize(&mut m);
            Ok(m)
        }
        Err(e) => {
            let consumed = text.len() - input.len();
            let line = text[..consumed].matches('\n').count() + 1;
            let col = consumed - text[..consumed].rfind('\n').map(|i| i + 1).unwrap_or(0) + 1;
            let rest: String = input.chars().take(40).collect();
            anyhow::bail!("sdf3 parse error at {line}:{col}: {e}\n  near: {rest:?}")
        }
    }
}

// ── trivia ──────────────────────────────────────────────────────────

/// `tokenize: "():,"`: a template literal run is split at each listed
/// character, so `else:` is the two tokens `else` and `:`. SDF3 applies the
/// option module-wide, and options usually come last, so this is a pass
/// over the read module rather than part of the template reader.
fn apply_tokenize(m: &mut Module) {
    let chars: Vec<char> = m
        .template_options()
        .filter_map(|o| match o {
            TemplateOption::Tokenize(s) => Some(s.chars().collect::<Vec<_>>()),
            _ => None,
        })
        .flatten()
        .collect();
    if chars.is_empty() {
        return;
    }
    for section in &mut m.sections {
        let Section::ContextFreeSyntax(prods) = section else {
            continue;
        };
        for p in prods {
            let Rhs::Template(parts) = &mut p.rhs else {
                continue;
            };
            let mut split = Vec::with_capacity(parts.len());
            for part in parts.drain(..) {
                match part {
                    TemplatePart::Lit(l) if l.chars().any(|c| chars.contains(&c)) => {
                        let mut run = String::new();
                        for c in l.chars() {
                            if chars.contains(&c) {
                                if !run.is_empty() {
                                    split.push(TemplatePart::Lit(std::mem::take(&mut run)));
                                }
                                split.push(TemplatePart::Lit(c.to_string()));
                            } else {
                                run.push(c);
                            }
                        }
                        if !run.is_empty() {
                            split.push(TemplatePart::Lit(run));
                        }
                    }
                    other => split.push(other),
                }
            }
            *parts = split;
        }
    }
}

fn ws(i: &mut In) -> R<()> {
    repeat::<_, _, (), _, _>(
        0..,
        alt((
            multispace1.void(),
            ("//", take_while(0.., |c| c != '\n')).void(),
            ("/*", take_until(0.., "*/"), "*/").void(),
        )),
    )
    .parse_next(i)
}

/// Lexeme: the parser, then trivia.
fn lex<'a, O, P>(p: P) -> impl Parser<In<'a>, O, ErrMode<ContextError>>
where
    P: Parser<In<'a>, O, ErrMode<ContextError>>,
{
    terminated(p, ws)
}

fn kw<'a>(word: &'static str) -> impl Parser<In<'a>, &'a str, ErrMode<ContextError>> {
    lex(terminated(
        literal(word),
        peek(alt((
            eof.void(),
            take_while(1, |c: char| !is_ident_char(c)).void(),
        ))),
    ))
    .context(StrContext::Expected(StrContextValue::StringLiteral(word)))
}

fn sym<'a>(s: &'static str) -> impl Parser<In<'a>, &'a str, ErrMode<ContextError>> {
    lex(literal(s)).context(StrContext::Expected(StrContextValue::StringLiteral(s)))
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '\''
}

fn ident(i: &mut In) -> R<String> {
    take_while(1.., is_ident_char)
        .verify(|s: &str| {
            s.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        })
        .map(str::to_string)
        .context(StrContext::Label("identifier"))
        .parse_next(i)
}

/// A section keyword must not be read as a sort name in a production.
const SECTION_WORDS: &[&str] = &[
    "module",
    "imports",
    "sorts",
    "lexical",
    "context-free",
    "template",
    "syntax",
    "priorities",
    "restrictions",
    "start-symbols",
    "options",
];

fn sort_name(i: &mut In) -> R<String> {
    ident
        .verify(|s: &String| !SECTION_WORDS.contains(&s.as_str()))
        .parse_next(i)
}

fn string_lit(i: &mut In) -> R<String> {
    delimited(
        '"',
        repeat::<_, _, Vec<char>, _, _>(
            0..,
            alt((
                preceded('\\', any),
                take_while(1, |c| c != '"' && c != '\\').map(|s: &str| s.chars().next().unwrap()),
            )),
        ),
        '"',
    )
    .map(|cs: Vec<char>| cs.into_iter().collect())
    .context(StrContext::Label("string literal"))
    .parse_next(i)
}

// ── character classes ───────────────────────────────────────────────

fn class_char(i: &mut In) -> R<char> {
    alt((
        preceded(
            '\\',
            any.map(|c| match c {
                't' => '\t',
                'n' => '\n',
                'r' => '\r',
                other => other,
            }),
        ),
        take_while(1, |c: char| c != ']' && c != '\\').map(|s: &str| s.chars().next().unwrap()),
    ))
    .parse_next(i)
}

fn char_class(i: &mut In) -> R<CharClass> {
    let negated = opt('~').parse_next(i)?.is_some();
    let ranges: Vec<(char, char)> = delimited(
        '[',
        repeat::<_, _, Vec<(char, char)>, _, _>(
            0..,
            (class_char, opt(preceded('-', class_char)))
                .map(|(a, b): (char, Option<char>)| (a, b.unwrap_or(a))),
        ),
        ']',
    )
    .context(StrContext::Label("character class"))
    .parse_next(i)?;
    Ok(CharClass { negated, ranges })
}

// ── symbols ─────────────────────────────────────────────────────────

fn postfix(base: Symbol, i: &mut In) -> R<Symbol> {
    let mut s = base;
    loop {
        match opt(alt(('*', '+', '?'))).parse_next(i)? {
            Some('*') => s = Symbol::Star(Box::new(s)),
            Some('+') => s = Symbol::Plus(Box::new(s)),
            Some('?') => s = Symbol::Opt(Box::new(s)),
            _ => return Ok(s),
        }
    }
}

/// `{Elem Sep}` followed by `*` or `+`. Tried before attributes at a `{`.
fn sep_list(i: &mut In) -> R<Symbol> {
    let (elem, sep) = delimited(sym("{"), (lex(symbol), lex(symbol)), sym("}")).parse_next(i)?;
    let min = match alt(('*', '+')).parse_next(i)? {
        '*' => 0,
        _ => 1,
    };
    Ok(Symbol::SepList {
        elem: Box::new(elem),
        sep: Box::new(sep),
        min,
    })
}

/// A symbol without its trailing trivia; callers wrap in `lex`.
fn symbol(i: &mut In) -> R<Symbol> {
    let base = alt((
        sep_list,
        delimited(sym("("), group_alternatives, ')').map(Symbol::Group),
        string_lit.map(Symbol::Lit),
        char_class.map(Symbol::CharClass),
        sort_name.map(Symbol::Sort),
    ))
    .parse_next(i)?;
    postfix(base, i)
}

fn symbol_sequence(i: &mut In) -> R<Vec<Symbol>> {
    repeat(1.., lex(symbol)).parse_next(i)
}

fn group_alternatives(i: &mut In) -> R<Vec<Vec<Symbol>>> {
    separated(1.., symbol_sequence, sym("|")).parse_next(i)
}

/// In a productive right-hand side, an identifier that begins the NEXT
/// production (`Sort.Cons =` or `Sort =`) is not a symbol of this one.
fn starts_production(i: &mut In) -> R<()> {
    peek((
        sort_name,
        ws,
        opt(('.', ident, ws)),
        '=',
        peek(alt((' ', '\t', '\n', '<', '['))),
    ))
    .void()
    .parse_next(i)
}

fn rhs_symbol(i: &mut In) -> R<Symbol> {
    if starts_production.parse_next(i).is_ok() {
        return Err(ErrMode::Backtrack(ContextError::new()));
    }
    // An attribute list is `{ident ...}`; a separated list is `{Sym Sym}`
    // with a literal or class inside. Let `symbol` try the list first and
    // fall through to the attribute parser on failure.
    if peek(('{', ws, ident, ws, '}')).parse_next(i).is_ok() {
        return Err(ErrMode::Backtrack(ContextError::new()));
    }
    lex(symbol).parse_next(i)
}

// ── templates ───────────────────────────────────────────────────────

fn template(i: &mut In) -> R<Vec<TemplatePart>> {
    let open = alt(('<', '[')).parse_next(i)?;
    let (close, ph_open, ph_close) = if open == '<' {
        ('>', '<', '>')
    } else {
        (']', '[', ']')
    };
    let mut parts = Vec::new();
    let mut lit = String::new();
    let mut layout = String::new();
    loop {
        let c = peek(any).parse_next(i)?;
        if c == close {
            any.parse_next(i)?;
            break;
        }
        if c == '\\' {
            any.parse_next(i)?;
            let e = any.parse_next(i)?;
            flush(&mut parts, &mut layout, false);
            lit.push(e);
            continue;
        }
        if c == ph_open {
            flush(&mut parts, &mut lit, true);
            flush(&mut parts, &mut layout, false);
            any.parse_next(i)?;
            ws.parse_next(i)?;
            let label = opt(terminated(ident, (ws, ':', ws))).parse_next(i)?;
            let sym = cut_err(lex(symbol)).parse_next(i)?;
            cut_err(ph_close)
                .context(StrContext::Label("placeholder close"))
                .parse_next(i)?;
            parts.push(TemplatePart::Placeholder { label, symbol: sym });
            continue;
        }
        any.parse_next(i)?;
        if c.is_whitespace() {
            flush(&mut parts, &mut lit, true);
            layout.push(c);
        } else {
            flush(&mut parts, &mut layout, false);
            lit.push(c);
        }
    }
    flush(&mut parts, &mut lit, true);
    flush(&mut parts, &mut layout, false);
    Ok(parts)
}

fn flush(parts: &mut Vec<TemplatePart>, buf: &mut String, is_lit: bool) {
    if buf.is_empty() {
        return;
    }
    let s = std::mem::take(buf);
    parts.push(if is_lit {
        TemplatePart::Lit(s)
    } else {
        TemplatePart::Layout(s)
    });
}

// ── productions ─────────────────────────────────────────────────────

/// One attribute; `layout(a, b)` is several.
fn attr(i: &mut In) -> R<Vec<Attr>> {
    let name: String = take_while(1.., |c: char| {
        c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'
    })
    .map(str::to_string)
    .parse_next(i)?;
    if name == "layout" {
        let cs = delimited((ws, '(', ws), layout_constraints, (ws, ')')).parse_next(i)?;
        return Ok(cs.into_iter().map(Attr::Layout).collect());
    }
    Ok(vec![match name.as_str() {
        "left" => Attr::Left,
        "right" => Attr::Right,
        "non-assoc" => Attr::NonAssoc,
        "assoc" => Attr::Assoc,
        "bracket" => Attr::Bracket,
        "reject" => Attr::Reject,
        "prefer" => Attr::Prefer,
        "avoid" => Attr::Avoid,
        _ => Attr::Other(name),
    }])
}

/// `1.last.col`: a symbol position, an end, an axis.
fn layout_pos(i: &mut In) -> R<LayoutPos> {
    let symbol: usize = dec_uint.parse_next(i)?;
    '.'.parse_next(i)?;
    let end = alt((
        "first".value(LayoutEnd::First),
        "last".value(LayoutEnd::Last),
    ))
    .parse_next(i)?;
    '.'.parse_next(i)?;
    let axis = alt(("col".value(LayoutAxis::Col), "line".value(LayoutAxis::Line))).parse_next(i)?;
    Ok(LayoutPos { symbol, end, axis })
}

/// The constraints of one `layout(...)`, joined by `,` or `&&`.
fn layout_constraints(i: &mut In) -> R<Vec<LayoutConstraint>> {
    separated(1.., lex(layout_constraint), lex(alt(("&&", ",")))).parse_next(i)
}

fn layout_constraint(i: &mut In) -> R<LayoutConstraint> {
    alt((
        layout_decl.map(LayoutConstraint::Decl),
        layout_rel.map(LayoutConstraint::Rel),
    ))
    .parse_next(i)
}

/// `indent 1 4`, `align-list 1`: a declarative constraint and its positions.
fn layout_decl(i: &mut In) -> R<LayoutDecl> {
    let kind = alt((
        "align-list".value(LayoutDeclKind::AlignList),
        "align".value(LayoutDeclKind::Align),
        "indent".value(LayoutDeclKind::Indent),
        "offside".value(LayoutDeclKind::Offside),
        "newline-indent".value(LayoutDeclKind::NewlineIndent),
        "single-line".value(LayoutDeclKind::SingleLine),
    ))
    .parse_next(i)?;
    let refs: Vec<usize> = repeat(1.., preceded(multispace1, dec_uint::<_, usize, _>)).parse_next(i)?;
    Ok(LayoutDecl { kind, refs })
}

/// `1.last.col + 1 == 2.first.col`.
fn layout_rel(i: &mut In) -> R<LayoutRel> {
    let lhs = lex(layout_pos).parse_next(i)?;
    let offset: Option<usize> = opt(preceded(sym("+"), lex(dec_uint))).parse_next(i)?;
    let op = lex(alt((
        "==".value(LayoutOp::Eq),
        "<".value(LayoutOp::Lt),
        ">".value(LayoutOp::Gt),
    )))
    .parse_next(i)?;
    let rhs = layout_pos.parse_next(i)?;
    Ok(LayoutRel {
        lhs,
        offset: offset.unwrap_or(0) as i32,
        op,
        rhs,
    })
}

fn attr_list(i: &mut In) -> R<Vec<Attr>> {
    let groups: Vec<Vec<Attr>> = separated(0.., lex(attr), sym(",")).parse_next(i)?;
    Ok(groups.into_iter().flatten().collect())
}

fn attrs(i: &mut In) -> R<Vec<Attr>> {
    opt(delimited(sym("{"), attr_list, sym("}")))
        .map(|a: Option<Vec<Attr>>| a.unwrap_or_default())
        .parse_next(i)
}

/// A `[` opens either a square template or a character class. SDF3 uses
/// templates for context-free syntax and character classes for lexical
/// syntax, so the section decides which reading a leading `[` gets.
fn production(allow_template: bool) -> impl FnMut(&mut In) -> R<Production> {
    move |i: &mut In| {
        let sort = lex(sort_name).parse_next(i)?;
        let constructor = opt(preceded(sym("."), lex(ident))).parse_next(i)?;
        sym("=").parse_next(i)?;
        let rhs = if allow_template {
            alt((
                lex(template).map(Rhs::Template),
                rhs_symbols.map(Rhs::Symbols),
            ))
            .parse_next(i)?
        } else {
            rhs_symbols.map(Rhs::Symbols).parse_next(i)?
        };
        let attrs = attrs.parse_next(i)?;
        Ok(Production {
            sort,
            constructor,
            rhs,
            attrs,
        })
    }
}

fn rhs_symbols(i: &mut In) -> R<Vec<Symbol>> {
    repeat(0.., rhs_symbol).parse_next(i)
}

fn cf_productions(i: &mut In) -> R<Vec<Production>> {
    repeat(0.., production(true)).parse_next(i)
}

fn lexical_productions(i: &mut In) -> R<Vec<Production>> {
    repeat(0.., production(false)).parse_next(i)
}

// ── restrictions, priorities, options ───────────────────────────────

fn restriction(i: &mut In) -> R<Restriction> {
    let symbols: Vec<String> = repeat(
        1..,
        lex((sort_name, opt('?'))
            .map(|(s, q): (String, Option<char>)| if q.is_some() { format!("{s}?") } else { s })),
    )
    .parse_next(i)?;
    sym("-/-").parse_next(i)?;
    let lookaheads: Vec<Vec<CharClass>> = separated(1.., lookahead, sym("|")).parse_next(i)?;
    Ok(Restriction {
        symbols,
        lookaheads,
    })
}

fn lookahead(i: &mut In) -> R<Vec<CharClass>> {
    separated(1.., lex(char_class), sym(".")).parse_next(i)
}

fn prod_ref(i: &mut In) -> R<String> {
    (sort_name, opt(preceded('.', ident)))
        .map(|(s, c)| match c {
            Some(c) => format!("{s}.{c}"),
            None => s,
        })
        .parse_next(i)
}

fn prod_refs(i: &mut In) -> R<Vec<String>> {
    repeat(1.., lex(prod_ref)).parse_next(i)
}

fn priority_group(i: &mut In) -> R<PriorityGroup> {
    alt((
        delimited(
            sym("{"),
            (opt(terminated(lex(attr), sym(":"))), prod_refs),
            sym("}"),
        )
        .map(|(assoc, members): (Option<Vec<Attr>>, Vec<String>)| PriorityGroup {
            assoc: assoc.and_then(|mut a| a.pop()),
            members,
        }),
        lex(prod_ref).map(|m| PriorityGroup {
            assoc: None,
            members: vec![m],
        }),
    ))
    .parse_next(i)
}

fn priority_chain(i: &mut In) -> R<PriorityChain> {
    separated(1.., priority_group, sym(">"))
        .map(|groups| PriorityChain { groups })
        .parse_next(i)
}

fn priority_chains(i: &mut In) -> R<Vec<PriorityChain>> {
    separated(0.., priority_chain, sym(",")).parse_next(i)
}

fn restrictions(i: &mut In) -> R<Vec<Restriction>> {
    repeat(0.., restriction).parse_next(i)
}

fn template_options(i: &mut In) -> R<Vec<TemplateOption>> {
    repeat(0.., template_option).parse_next(i)
}

fn template_option(i: &mut In) -> R<TemplateOption> {
    alt((
        (lex(sort_name), sym("="), kw("keyword"), attrs)
            .verify(|(_, _, _, a)| a.contains(&Attr::Reject))
            .map(|(sort, _, _, _)| TemplateOption::KeywordReject { sort }),
        preceded((kw("keyword"), sym("-/-")), lex(char_class)).map(TemplateOption::KeywordFollow),
        preceded((kw("tokenize"), sym(":")), lex(string_lit)).map(TemplateOption::Tokenize),
    ))
    .parse_next(i)
}

// ── sections and module ─────────────────────────────────────────────

fn names(i: &mut In) -> R<Vec<String>> {
    repeat(0.., lex(sort_name)).parse_next(i)
}

fn sec_start_symbols(i: &mut In) -> R<Section> {
    preceded((kw("context-free"), kw("start-symbols")), names)
        .map(Section::StartSymbols)
        .parse_next(i)
}

fn sec_cf_sorts(i: &mut In) -> R<Section> {
    preceded((kw("context-free"), kw("sorts")), names)
        .map(Section::ContextFreeSorts)
        .parse_next(i)
}

fn sec_cf_syntax(i: &mut In) -> R<Section> {
    preceded((kw("context-free"), kw("syntax")), cf_productions)
        .map(Section::ContextFreeSyntax)
        .parse_next(i)
}

fn sec_cf_restrictions(i: &mut In) -> R<Section> {
    preceded((kw("context-free"), kw("restrictions")), restrictions)
        .map(Section::ContextFreeRestrictions)
        .parse_next(i)
}

fn sec_cf_priorities(i: &mut In) -> R<Section> {
    preceded((kw("context-free"), kw("priorities")), priority_chains)
        .map(Section::ContextFreePriorities)
        .parse_next(i)
}

fn sec_lex_sorts(i: &mut In) -> R<Section> {
    preceded((kw("lexical"), kw("sorts")), names)
        .map(Section::LexicalSorts)
        .parse_next(i)
}

fn sec_lex_syntax(i: &mut In) -> R<Section> {
    preceded((kw("lexical"), kw("syntax")), lexical_productions)
        .map(Section::LexicalSyntax)
        .parse_next(i)
}

fn sec_lex_restrictions(i: &mut In) -> R<Section> {
    preceded((kw("lexical"), kw("restrictions")), restrictions)
        .map(Section::LexicalRestrictions)
        .parse_next(i)
}

fn sec_sorts(i: &mut In) -> R<Section> {
    preceded(kw("sorts"), names)
        .map(Section::Sorts)
        .parse_next(i)
}

fn sec_template_options(i: &mut In) -> R<Section> {
    preceded((kw("template"), kw("options")), template_options)
        .map(Section::TemplateOptions)
        .parse_next(i)
}

fn no_section(i: &mut In) -> R<Section> {
    fail.context(StrContext::Label("section")).parse_next(i)
}

fn section(i: &mut In) -> R<Section> {
    // winnow's `Alt` stops short of eleven alternatives, so nest.
    alt((
        alt((
            sec_start_symbols,
            sec_cf_sorts,
            sec_cf_syntax,
            sec_cf_restrictions,
            sec_cf_priorities,
        )),
        alt((
            sec_lex_sorts,
            sec_lex_syntax,
            sec_lex_restrictions,
            sec_sorts,
            sec_template_options,
        )),
        no_section,
    ))
    .parse_next(i)
}

fn module(i: &mut In) -> R<Module> {
    ws.parse_next(i)?;
    kw("module").parse_next(i)?;
    let name =
        lex(take_while(1.., |c: char| !c.is_whitespace()).map(str::to_string)).parse_next(i)?;
    let imports: Vec<String> = opt(preceded(kw("imports"), names))
        .map(Option::unwrap_or_default)
        .parse_next(i)?;
    let sections: Vec<Section> = repeat(0.., section).parse_next(i)?;
    eof.context(StrContext::Label("end of module"))
        .parse_next(i)?;
    Ok(Module {
        name,
        imports,
        sections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p<T>(mut f: impl FnMut(&mut In) -> R<T>, s: &str) -> T {
        let mut i = s;
        let v = f(&mut i).unwrap_or_else(|e| panic!("{s:?}: {e}"));
        assert_eq!(i.trim(), "", "unconsumed input after {s:?}: {i:?}");
        v
    }

    #[test]
    fn character_classes_carry_sdf_escapes() {
        let c = p(char_class, r"[a-zA-Z\_]");
        assert_eq!(c.ranges, vec![('a', 'z'), ('A', 'Z'), ('_', '_')]);
        let c = p(char_class, r"~[\n\r]");
        assert!(c.negated);
        assert_eq!(c.ranges, vec![('\n', '\n'), ('\r', '\r')]);
        let c = p(char_class, r"[\ \t]");
        assert_eq!(c.ranges, vec![(' ', ' '), ('\t', '\t')]);
    }

    #[test]
    fn templates_split_literals_on_layout_and_read_placeholders() {
        let t = p(template, "<if (<condition:Exp>) <Block> else <Block>>");
        let literals: Vec<&str> = t
            .iter()
            .filter_map(|x| match x {
                TemplatePart::Lit(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(literals, vec!["if", "(", ")", "else"]);
        let labels: Vec<Option<&str>> = t
            .iter()
            .filter_map(|x| match x {
                TemplatePart::Placeholder { label, .. } => Some(label.as_deref()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, vec![Some("condition"), None, None]);
    }

    #[test]
    fn square_templates_let_angle_brackets_be_literal() {
        let t = p(template, "[[left:Exp] < [right:Exp]]");
        assert!(matches!(&t[2], TemplatePart::Lit(s) if s == "<"));
    }

    #[test]
    fn sep_lists_and_attributes_both_start_with_a_brace() {
        let pr = p(
            production(true),
            r#"Exp.Call = ID "(" {Exp ","}* ")" {left}"#,
        );
        assert_eq!(pr.attrs, vec![Attr::Left]);
        match pr.rhs {
            Rhs::Symbols(s) => assert!(matches!(s[2], Symbol::SepList { min: 0, .. })),
            _ => panic!(),
        }
    }

    #[test]
    fn a_productive_rhs_stops_at_the_next_production() {
        let ps = p(
            cf_productions,
            "Exp.Add = Exp \"+\" Exp {left}\nExp.Sub = Exp \"-\" Exp\nExp = ID",
        );
        assert_eq!(ps.len(), 3);
        assert_eq!(ps[2].constructor, None);
    }

    #[test]
    fn priority_chains_carry_groups_and_associativity() {
        let c = p(
            priority_chain,
            "{Exp.Neg Exp.Not} > {left: Exp.Mul Exp.Div} > Exp.Eq",
        );
        assert_eq!(c.groups.len(), 3);
        assert_eq!(c.groups[1].assoc, Some(Attr::Left));
        assert_eq!(c.groups[1].members, vec!["Exp.Mul", "Exp.Div"]);
        assert_eq!(c.groups[2].members, vec!["Exp.Eq"]);
    }

    #[test]
    fn restrictions_read_dotted_lookaheads() {
        let r = p(restriction, r"LAYOUT? -/- [\/].[\/]");
        assert_eq!(r.symbols, vec!["LAYOUT?"]);
        assert_eq!(r.lookaheads[0].len(), 2);
    }
}
