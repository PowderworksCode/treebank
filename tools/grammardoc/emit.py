"""Emit the manual: vocabulary index, precedence table, and every
production as EBNF plus a railroad diagram."""
import html
import sys
from pathlib import Path

import railroad as rr
from grammardoc import Grammar, to_rr, to_ebnf, precedences
from style import stylesheet

E = html.escape


def role_index(g: Grammar):
    """Which rules answer which vocabulary term. Supertypes come from
    node-types.json -- they are real rules in the parse table. Facets come
    from roles.json and are expanded into an alternation at query load."""
    table = {n['type']: sorted(s['type'] for s in n['subtypes'])
             for n in g.nt if n.get('subtypes')}
    facet = {k: sorted(v) for k, v in (g.roles.get('facets') or {}).items()}
    return table, facet


def badges(name, g: Grammar):
    out = []
    if name in g.supertypes:
        out.append(('supertype', 'sup'))
    if name in g.externals:
        out.append(('external', 'ext'))
    if g.hidden(name):
        out.append(('hidden', 'hid'))
    elif name in g.visible:
        out.append(('node', 'vis'))
    if name == g.word:
        out.append(('word', 'word'))
    return out


def rule_block(name, g: Grammar):
    o = [f'<section class="rule" id="r-{E(name)}">',
         '<div class="rule-head">',
         f'<a class="rule-name mono" href="#r-{E(name)}">{E(name)}</a>']
    for label, cls in badges(name, g):
        o.append(f'<span class="badge b-{cls}">{label}</span>')
    o.append('</div>')
    o.append(f'<pre class="ebnf">{to_ebnf(g.rules[name], g)}</pre>')
    o.append('<figure class="rrbox">'
             + rr.diagram(to_rr(g.rules[name], g), title=name)
             + '</figure>')
    o.append('</section>')
    return '\n'.join(o)


FILTER_JS = """
<script>
(function(){
  var box = document.getElementById('filter');
  var links = Array.prototype.slice.call(
      document.querySelectorAll('.idx a[data-name]'));
  var heads = Array.prototype.slice.call(document.querySelectorAll('.idx h4'));
  box.addEventListener('input', function(){
    var q = box.value.trim().toLowerCase();
    links.forEach(function(a){
      a.style.display = (!q || a.dataset.name.indexOf(q) !== -1) ? '' : 'none';
    });
    heads.forEach(function(h){
      var any = false, n = h.nextElementSibling;
      while (n && n.tagName === 'A') {
        if (n.style.display !== 'none') { any = true; break; }
        n = n.nextElementSibling;
      }
      h.style.display = any ? '' : 'none';
    });
  });
  box.addEventListener('keydown', function(e){
    if (e.key === 'Escape') { box.value = ''; box.dispatchEvent(new Event('input')); }
  });
})();
</script>
"""


def build(crate: Path, out: Path, embed_fonts=True):
    g = Grammar(crate)
    table, facet = role_index(g)
    precs = precedences(g)

    # Section order: rules grouped under the supertype they answer, then
    # the rest. A rule appears once, under the first role that claims it.
    groups, seen = [], set()
    for role in sorted(table):
        members = [m for m in table[role] if m in g.rules and m not in seen]
        if members:
            groups.append((role, members))
            seen.update(members)
    pub = sorted(n for n in g.rules if n not in seen and not g.hidden(n))
    hid = sorted(n for n in g.rules if n not in seen and g.hidden(n))
    if pub:
        groups.append(('unclassified named nodes', pub))
    if hid:
        groups.append(('hidden rules', hid))

    o = [f'<title>{g.name.capitalize()} Rulebook</title>', stylesheet(embed_fonts)]

    o.append('<header class="top"><div class="wrap">')
    o.append('<p class="eyebrow">treebank grammar reference</p>')
    o.append(f'<h1>{E(g.name)}<span class="dot">.</span></h1>')
    o.append('<p class="lede">Every production in the parse table, drawn from '
             '<strong>src/grammar.json</strong> &mdash; the normalised grammar '
             'tree-sitter itself consumes, not the hand-written grammar.js. '
             'Nothing here is a summary: if the parser does it, it is on this '
             'page.</p>')
    o.append('<div class="stats">')
    for label, v in [('productions', len(g.rules)),
                     ('node types', len(g.visible)),
                     ('supertypes', len(g.supertypes)),
                     ('external tokens', len(g.externals)),
                     ('declared conflicts', len(g.g.get('conflicts', []))),
                     ('precedence levels', len(precs))]:
        o.append(f'<div><b>{v}</b><span>{label}</span></div>')
    o.append('</div></div></header>')

    o.append('<div class="wrap cols">')

    # ---- index rail ----------------------------------------------------
    o.append('<nav class="idx" aria-label="Productions">')
    o.append('<input id="filter" type="search" placeholder="filter productions'
             '…" aria-label="Filter productions">')
    o.append('<div class="idx-scroll">')
    for role, members in groups:
        o.append(f'<h4>{E(role)}</h4>')
        for m in members:
            o.append(f'<a href="#r-{E(m)}" data-name="{E(m.lower())}">{E(m)}</a>')
    o.append('</div></nav>')

    o.append('<main>')

    # ---- vocabulary ----------------------------------------------------
    o.append('<h2 id="vocabulary">Vocabulary</h2>')
    o.append('<p>The roles a query may name. A <b>supertype</b> is a real rule '
             'threaded through the productions, so <code>(_expression)</code> '
             'matches where the parse actually went through it &mdash; matching '
             'is by derivation, not by node type. A <b>facet</b> is type-level: '
             'it is a list in <code>roles.json</code> that expands into a '
             'concrete alternation when the query is loaded.</p>')
    for title, group, cls in [('Supertypes', table, 'sup'),
                              ('Facets', facet, 'fac')]:
        if not group:
            continue
        o.append(f'<h3 class="vh">{title}</h3><dl class="vocab">')
        for role in sorted(group):
            links = ''.join(
                f'<a href="#r-{E(m)}">{E(m)}</a>' if m in g.rules
                else f'<span class="dead">{E(m)}</span>'
                for m in group[role])
            o.append(f'<dt class="{cls}">{E(role)}</dt><dd>{links}</dd>')
        o.append('</dl>')

    # ---- precedence ----------------------------------------------------
    o.append('<h2 id="precedence">Precedence</h2>')
    o.append('<p>The half of a grammar EBNF cannot show. Higher binds tighter; '
             '<code>prec.left</code> and <code>prec.right</code> also pick a '
             'side when levels tie. <code>prec.dynamic</code> is the only one '
             'consulted for a <em>declared</em> conflict &mdash; there, static '
             'precedence is ignored entirely.</p>')
    o.append('<div class="scroll"><table class="prec"><thead><tr>'
             '<th>level</th><th>kind</th><th>productions</th>'
             '</tr></thead><tbody>')
    for lvl in sorted(precs, reverse=True):
        kinds = sorted({k for k, _ in precs[lvl]})
        rules = sorted({r for _, r in precs[lvl]})
        o.append(
            f'<tr><td class="lvl">{lvl}</td>'
            f'<td class="kind">{" ".join(f"<code>{E(k)}</code>" for k in kinds)}</td>'
            f'<td>{"".join(f'<a href="#r-{E(r)}">{E(r)}</a>' for r in rules)}</td></tr>')
    o.append('</tbody></table></div>')

    # ---- productions ---------------------------------------------------
    o.append('<h2 id="productions">Productions</h2>')
    o.append('<div class="key">'
             '<span><i class="k-term"></i>literal text</span>'
             '<span><i class="k-nt"></i>another production</span>'
             '<span><i class="k-re"></i>pattern</span>'
             '<span><i class="k-ext"></i>external scanner</span>'
             '</div>')
    for role, members in groups:
        o.append(f'<h3 class="grp">{E(role)}</h3>')
        for m in members:
            o.append(rule_block(m, g))

    o.append('</main></div>')
    o.append('<footer><div class="wrap">'
             f'<span>generated from <code>crates/treebank-{E(g.name)}</code></span>'
             '<span>treebank</span></div></footer>')
    o.append(FILTER_JS)

    text = '\n'.join(o)
    out.write_text(text, encoding='utf-8')
    return g, len(text)


def check(crate: Path):
    """Render everything and assert it came out sane. This is the CI job.

    The point is not to check the prose. `to_rr` and `to_ebnf` are total
    over grammar.json's node kinds and raise on anything they do not know,
    so this fails loudly the day a grammar starts using a DSL construct the
    renderer has never seen -- which is the only way that would otherwise be
    noticed, since a missing case silently omits part of a production.
    """
    g = Grammar(crate)
    from style import LIGHT, DARK
    if set(LIGHT) != set(DARK):
        raise SystemExit('light and dark palettes define different tokens: '
                         f'{set(LIGHT) ^ set(DARK)}')
    bad = []
    for name, body in g.rules.items():
        to_ebnf(body, g)                       # raises on an unknown node kind
        node = to_rr(body, g)
        for dim in ('width', 'up', 'down'):
            v = getattr(node, dim)
            if not (v == v) or v < 0 or v > 1e5:   # NaN, negative, runaway
                bad.append(f'{name}: {dim}={v}')
    if bad:
        raise SystemExit('bad diagram geometry:\n  ' + '\n  '.join(bad))
    print(f'grammardoc: {g.name} OK — {len(g.rules)} productions render')


if __name__ == '__main__':
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    flags = {a for a in sys.argv[1:] if a.startswith('--')}
    if '--check' in flags:
        for a in args:
            check(Path(a))
    else:
        crate, out = Path(args[0]), Path(args[1])
        g, n = build(crate, out, embed_fonts='--no-fonts' not in flags)
        print(f'grammardoc: {g.name} — {len(g.rules)} productions -> {out} '
              f'({n / 1024 / 1024:.2f} MiB)')
