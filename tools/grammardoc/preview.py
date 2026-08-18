"""Rasterise one production's diagram offline, for eyeballing layout.

The page styles the SVG with CSS, which no simple rasteriser applies, so
this rewrites the classes into presentation attributes first. Colours come
from style.LIGHT, never from a copy, so a preview always shows the palette
the page actually uses.

    python3 tools/grammardoc/preview.py crates/treebank-python /tmp string
    convert -density 140 /tmp/string.svg /tmp/string.png

Worth doing whenever the layout engine changes: the two bugs it caught were
both branches of a `choice` drawn on top of each other, which reads as
plausible source and obvious nonsense as a picture.
"""
import re
import sys
from pathlib import Path

import railroad as rr
from grammardoc import Grammar, to_rr
from style import LIGHT as P

BOX = {'term': ('literal-bg', 'literal'), 'nonterm': ('accent-bg', 'accent'),
       'regex': ('sunk', 'rule'), 'external': ('outside-bg', 'outside')}
DASH = {'field': 'literal', 'prec': 'accent', 'alias': 'rule', 'token': 'rule'}


def preview(svg):
    svg = svg.replace('<svg ', f'<svg style="background:{P["paper"]}" ', 1)
    svg = svg.replace('<path class="cap-line" d=',
                      f'<path fill="none" stroke="{P["ink"]}" stroke-width="2.4" d=')
    svg = svg.replace('<path d=',
                      f'<path fill="none" stroke="{P["muted"]}" stroke-width="1.3" d=')

    def g_sub(m):
        cls, body = m.group(1), m.group(2)
        if cls in BOX:
            fill, stroke = (P[k] for k in BOX[cls])
            body = body.replace('<rect ', f'<rect fill="{fill}" stroke="{stroke}" '
                                          f'stroke-width="1.2" ')
            colour = P['muted'] if cls == 'regex' else P['ink']
            body = body.replace('<text ', f'<text fill="{colour}" text-anchor="middle" '
                                          f'font-family="monospace" font-size="12" '
                                          f'font-weight="700" ')
        return f'<g>{body}</g>'

    svg = re.sub(r'<g class="(\w+)">(.*?)</g>', g_sub, svg, flags=re.S)
    for cls, tok in DASH.items():
        svg = svg.replace(f'<rect class="{cls}" ',
                          f'<rect fill="none" stroke="{P[tok]}" '
                          f'stroke-dasharray="3 2.5" ')
    return svg.replace('<text class="cap" ',
                       f'<text fill="{P["faint"]}" font-family="sans-serif" '
                       f'font-size="8.5" ')


if __name__ == '__main__':
    g = Grammar(Path(sys.argv[1]))
    out = Path(sys.argv[2])
    for rule in sys.argv[3:]:
        if rule not in g.rules:
            raise SystemExit(f'no production named {rule} in {g.name}')
        (out / f'{rule}.svg').write_text(preview(rr.diagram(to_rr(g.rules[rule], g))))
        print(f'{out}/{rule}.svg')
