"""Render a tree-sitter grammar the way a language manual renders one.

Input is `src/grammar.json` -- the NORMALIZED grammar tree-sitter itself
consumes, not grammar.js -- plus `src/node-types.json` for what is public
and `roles.json` for treebank's vocabulary. grammar.json is already an
EBNF syntax tree: SEQ, CHOICE, REPEAT, SYMBOL, STRING, PATTERN, PREC,
FIELD, ALIAS. Rendering it is a fold, not a parse.
"""
import html
import json
import sys
from pathlib import Path

import railroad as rr

PREC_KINDS = {'PREC': 'prec', 'PREC_LEFT': 'prec.left',
              'PREC_RIGHT': 'prec.right', 'PREC_DYNAMIC': 'prec.dynamic'}


# ---------------------------------------------------------------- grammar

class Grammar:
    def __init__(self, crate: Path):
        self.g = json.loads((crate / 'src' / 'grammar.json').read_text())
        self.nt = json.loads((crate / 'src' / 'node-types.json').read_text())
        rp = crate / 'roles.json'
        self.roles = json.loads(rp.read_text()) if rp.exists() else {}
        self.rules = self.g['rules']
        self.name = self.g['name']
        self.supertypes = set(self.g.get('supertypes', []))
        self.externals = {e['name'] for e in self.g.get('externals', [])
                          if e.get('type') == 'SYMBOL'}
        self.word = self.g.get('word')
        # public node types, so we can say which rules a consumer ever sees
        self.visible = {n['type'] for n in self.nt if n.get('named')}

    def hidden(self, name):
        return name.startswith('_')


# ------------------------------------------------------------- to railroad

def commasep(node):
    """Recognise seq(X, repeat(seq(sep, X))) -- the list idiom. Returns
    (item, sep) so it renders as one loop instead of a chain."""
    if node.get('type') != 'SEQ' or len(node['members']) != 2:
        return None
    first, second = node['members']
    if second.get('type') != 'REPEAT':
        return None
    inner = second['content']
    if inner.get('type') != 'SEQ' or len(inner['members']) != 2:
        return None
    sep, again = inner['members']
    if json.dumps(again, sort_keys=True) != json.dumps(first, sort_keys=True):
        return None
    if sep.get('type') not in ('STRING', 'SYMBOL'):
        return None
    return first, sep


def to_rr(node, g: Grammar):
    t = node['type']
    if t == 'STRING':
        return rr.Leaf(node['value'], 'term')
    if t == 'PATTERN':
        v = node['value']
        short = v if len(v) <= 32 else v[:29] + '...'
        return rr.Leaf('/' + short + '/', 'regex', title=v)
    if t == 'SYMBOL':
        n = node['name']
        cls = 'external' if n in g.externals else 'nonterm'
        return rr.Leaf(n, cls, href='#r-' + n)
    if t == 'BLANK':
        return rr.Skip()
    if t == 'SEQ':
        cs = commasep(node)
        if cs:
            item, sep = cs
            return rr.Repeat(to_rr(item, g), to_rr(sep, g))
        return rr.Seq([to_rr(m, g) for m in node['members']])
    if t == 'CHOICE':
        members = node['members']
        blanks = [i for i, m in enumerate(members) if m.get('type') == 'BLANK']
        rest = [m for m in members if m.get('type') != 'BLANK']
        if blanks and len(rest) == 1:
            return rr.Optional(to_rr(rest[0], g))
        if blanks:
            return rr.Optional(rr.Choice([to_rr(m, g) for m in rest]))
        return rr.Choice([to_rr(m, g) for m in members])
    if t == 'REPEAT':
        return rr.Optional(rr.Repeat(to_rr(node['content'], g)))
    if t == 'REPEAT1':
        return rr.Repeat(to_rr(node['content'], g))
    if t == 'FIELD':
        return rr.Labelled(to_rr(node['content'], g), node['name'] + ':', 'field')
    if t == 'ALIAS':
        inner = to_rr(node['content'], g)
        return rr.Labelled(inner, 'as ' + node['value'], 'alias')
    if t in PREC_KINDS:
        label = PREC_KINDS[t]
        v = node.get('value', 0)
        return rr.Labelled(to_rr(node['content'], g), f'{label} {v}', 'prec')
    if t in ('TOKEN', 'IMMEDIATE_TOKEN'):
        lab = 'token' if t == 'TOKEN' else 'token.immediate'
        return rr.Labelled(to_rr(node['content'], g), lab, 'token')
    if t == 'RESERVED':
        return to_rr(node['content'], g)
    raise SystemExit(f'unhandled grammar node {t}')


# ----------------------------------------------------------------- to EBNF
# Precedence for parenthesising: choice binds loosest, then seq, then the
# postfix repetition operators.
P_CHOICE, P_SEQ, P_POSTFIX, P_ATOM = 0, 1, 2, 3


def to_ebnf(node, g: Grammar, ctx=P_CHOICE):
    def paren(s, mine):
        return f'({s})' if mine < ctx else s

    t = node['type']
    if t == 'STRING':
        return "<b>" + html.escape(node['value']) + "</b>"
    if t == 'PATTERN':
        return '<i>/' + html.escape(node['value']) + '/</i>'
    if t == 'SYMBOL':
        n = node['name']
        cls = 'ext' if n in g.externals else ('hid' if g.hidden(n) else 'sym')
        return f'<a class="{cls}" href="#r-{n}">{html.escape(n)}</a>'
    if t == 'BLANK':
        return '<span class="eps">&#949;</span>'
    if t == 'SEQ':
        return paren(' '.join(to_ebnf(m, g, P_SEQ) for m in node['members']), P_SEQ)
    if t == 'CHOICE':
        members = node['members']
        rest = [m for m in members if m.get('type') != 'BLANK']
        if len(rest) < len(members):
            if len(rest) == 1:
                return to_ebnf(rest[0], g, P_POSTFIX) + '?'
            body = ' | '.join(to_ebnf(m, g, P_CHOICE) for m in rest)
            return f'({body})?'
        return paren(' | '.join(to_ebnf(m, g, P_CHOICE) for m in members), P_CHOICE)
    if t == 'REPEAT':
        return to_ebnf(node['content'], g, P_POSTFIX) + '*'
    if t == 'REPEAT1':
        return to_ebnf(node['content'], g, P_POSTFIX) + '+'
    if t == 'FIELD':
        return (f'<span class="fld">{html.escape(node["name"])}:</span>'
                + to_ebnf(node['content'], g, P_POSTFIX))
    if t == 'ALIAS':
        return (to_ebnf(node['content'], g, P_POSTFIX)
                + f'<span class="al"> as {html.escape(node["value"])}</span>')
    if t in PREC_KINDS:
        return to_ebnf(node['content'], g, ctx)
    if t in ('TOKEN', 'IMMEDIATE_TOKEN', 'RESERVED'):
        return to_ebnf(node['content'], g, ctx)
    raise SystemExit(f'unhandled grammar node {t}')


# --------------------------------------------------------------- precedence

def precedences(g: Grammar):
    """Every prec in the grammar, by numeric level. This is the half of the
    grammar EBNF cannot show and the manuals always print separately."""
    found = {}

    def walk(node, rule):
        if not isinstance(node, dict):
            return
        t = node.get('type')
        if t in PREC_KINDS:
            v = node.get('value', 0)
            if isinstance(v, int):
                found.setdefault(v, []).append((PREC_KINDS[t], rule))
        for k in ('members', 'content'):
            val = node.get(k)
            if isinstance(val, list):
                for x in val:
                    walk(x, rule)
            elif isinstance(val, dict):
                walk(val, rule)

    for name, body in g.rules.items():
        walk(body, name)
    return found
