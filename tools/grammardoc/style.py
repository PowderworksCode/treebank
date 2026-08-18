"""The look. Drafting linen and signal colours, not manuscript cream.

Type concept: this document is mostly identifiers, so the MONOSPACE face is
the primary voice -- rule names, productions, diagram labels, section
labels -- and the serif is reserved for the few paragraphs of prose. That
inverts the usual document hierarchy on purpose, because it is true to
what is on the page.

C059 is URW's Century Schoolbook, the face printed language reports were
actually set in. DejaVu Sans Mono is embedded for a second, harder reason:
the railroad SVG's box widths are computed here, from a fixed character
advance, so a font fallback in the browser would clip every label.
"""
import base64
from pathlib import Path

FONTS = [
    ('rulebook-mono', '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf', 400, 'truetype'),
    ('rulebook-mono', '/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf', 700, 'truetype'),
    ('rulebook-text', '/usr/share/fonts/opentype/urw-base35/C059-Roman.otf', 400, 'opentype'),
    ('rulebook-text', '/usr/share/fonts/opentype/urw-base35/C059-Bold.otf', 700, 'opentype'),
    ('rulebook-text', '/usr/share/fonts/opentype/urw-base35/C059-Italic.otf', 400, 'opentype'),
]


def faces(embed=True):
    if not embed:
        return ''
    out = []
    for fam, path, weight, fmt in FONTS:
        p = Path(path)
        if not p.exists():
            continue
        mime = 'font/ttf' if fmt == 'truetype' else 'font/otf'
        b64 = base64.b64encode(p.read_bytes()).decode()
        style = 'italic' if 'Italic' in p.name else 'normal'
        out.append(f"@font-face{{font-family:{fam};font-weight:{weight};"
                   f"font-style:{style};font-display:block;"
                   f"src:url(data:{mime};base64,{b64}) format('{fmt}')}}")
    return '\n'.join(out)


# One palette, two themes, defined once. Both the stylesheet and the
# offline preview rasteriser read these, so a colour cannot drift between
# what the browser shows and what a check renders.
LIGHT = {
    'paper': '#e9ece8', 'sheet': '#f7f9f6', 'sunk': '#e1e6e1',
    'ink': '#14181a', 'muted': '#5a6a6b', 'faint': '#8b9896',
    'rule': '#c8d1cd', 'hair': '#d8dfdb',
    'accent': '#16496e',        # ink blue -- structure, other productions
    'literal': '#8a6212',       # ochre    -- literal text in the source
    'outside': '#7c3d63',       # plum     -- handed to the external scanner
    'accent-bg': '#dce6ee', 'literal-bg': '#f0e6cf', 'outside-bg': '#eddce6',
    'focus': '#16496e',
}
DARK = {
    'paper': '#0f1312', 'sheet': '#171c1b', 'sunk': '#111615',
    'ink': '#e4e9e6', 'muted': '#8d9b98', 'faint': '#6b7a77',
    'rule': '#2a3230', 'hair': '#232a29',
    'accent': '#7cb2dc', 'literal': '#d6ab4e', 'outside': '#c98cb4',
    'accent-bg': '#152531', 'literal-bg': '#2c2515', 'outside-bg': '#2b1c26',
    'focus': '#7cb2dc',
}


def _vars(p):
    return ''.join(f'--{k}:{v};' for k, v in p.items())


# Light is the base, on bare :root, so the un-stamped default state -- which
# is what most viewers are in -- always resolves. Dark redefines only the
# tokens, twice: once for the OS preference (losing to an explicit light
# choice) and once for an explicit dark choice. No component rule ever sets
# a colour inside a media or [data-theme] block.
TOKENS = f"""
:root{{{_vars(LIGHT)}}}
@media (prefers-color-scheme: dark){{
  :root:not([data-theme="light"]){{{_vars(DARK)}}}
}}
:root[data-theme="dark"]{{{_vars(DARK)}}}
"""

CSS = """
*,*::before,*::after{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{
  margin:0;background:var(--paper);color:var(--ink);
  font-family:rulebook-text,"Century Schoolbook",Charter,Georgia,serif;
  font-size:16px;line-height:1.62;
}
.mono,code,pre,.rule-name,.badge,.eyebrow,dl.vocab dd,table.prec td a,
.idx a,input{
  font-family:rulebook-mono,ui-monospace,SFMono-Regular,Menlo,monospace;
}
a{color:var(--accent)}
:focus-visible{outline:2px solid var(--focus);outline-offset:2px;border-radius:3px}

/* ---- shell -------------------------------------------------------- */
.wrap{max-width:88rem;margin:0 auto;padding:0 1.5rem}
header.top{border-bottom:1px solid var(--rule);background:var(--sheet)}
header.top .wrap{padding-top:3.5rem;padding-bottom:2.5rem}
.eyebrow{
  font-size:.7rem;letter-spacing:.18em;text-transform:uppercase;
  color:var(--muted);margin:0 0 .9rem;
}
h1{
  font-family:rulebook-mono,ui-monospace,monospace;
  font-size:clamp(2.1rem,5vw,3.1rem);font-weight:700;letter-spacing:-.02em;
  margin:0;line-height:1.05;text-wrap:balance;
}
h1 .dot{color:var(--accent)}
.lede{
  margin:1.1rem 0 0;max-width:38em;color:var(--muted);font-size:1.02rem;
}
.lede strong{color:var(--ink);font-weight:400}

.stats{
  display:grid;grid-template-columns:repeat(auto-fit,minmax(8.5rem,1fr));
  gap:1px;margin:2.25rem 0 0;background:var(--rule);
  border:1px solid var(--rule);border-radius:4px;overflow:hidden;
}
.stats div{background:var(--sheet);padding:.85rem 1rem}
.stats b{
  display:block;font-family:rulebook-mono,monospace;font-size:1.55rem;
  font-weight:700;font-variant-numeric:tabular-nums;line-height:1.1;
}
.stats span{
  display:block;margin-top:.15rem;font-family:rulebook-mono,monospace;
  font-size:.63rem;letter-spacing:.11em;text-transform:uppercase;color:var(--muted);
}

/* ---- two-column body ---------------------------------------------- */
.cols{display:grid;grid-template-columns:15.5rem minmax(0,1fr);gap:3rem;
  align-items:start;padding-top:2.5rem;padding-bottom:6rem}
@media (max-width:70rem){.cols{grid-template-columns:minmax(0,1fr);gap:1.5rem}}

nav.idx{position:sticky;top:1rem;max-height:calc(100vh - 2rem);
  display:flex;flex-direction:column;gap:.6rem}
@media (max-width:70rem){nav.idx{position:static;max-height:none}}
nav.idx input{
  width:100%;padding:.5rem .65rem;font-size:.8rem;color:var(--ink);
  background:var(--sheet);border:1px solid var(--rule);border-radius:4px;
}
nav.idx input::placeholder{color:var(--faint)}
.idx-scroll{overflow-y:auto;min-height:0;padding-right:.25rem}
.idx h4{
  margin:1rem 0 .3rem;font-family:rulebook-mono,monospace;font-size:.65rem;
  letter-spacing:.12em;text-transform:uppercase;color:var(--faint);font-weight:400;
}
.idx h4:first-child{margin-top:0}
.idx a{display:block;font-size:.76rem;line-height:1.75;color:var(--muted);
  text-decoration:none;padding:0 .3rem;border-radius:3px;
  overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.idx a:hover{color:var(--ink);background:var(--sunk)}
.idx a.on{color:var(--accent)}
.idx .hit{display:none}

main{min-width:0}
h2{
  font-family:rulebook-mono,monospace;font-size:1.15rem;font-weight:700;
  letter-spacing:-.01em;margin:3.5rem 0 .6rem;padding-bottom:.5rem;
  border-bottom:1px solid var(--rule);text-wrap:balance;
}
h2:first-child{margin-top:0}
main > p{color:var(--muted);max-width:44em;margin:.6rem 0 1.4rem;font-size:.95rem}
main > p code{color:var(--ink)}
code{font-size:.85em;background:var(--sunk);padding:.08em .32em;border-radius:3px}

/* ---- vocabulary ---------------------------------------------------- */
h3.vh{font-family:rulebook-mono,monospace;font-size:.68rem;letter-spacing:.12em;
  text-transform:uppercase;color:var(--faint);font-weight:400;margin:1.6rem 0 .5rem}
dl.vocab{display:grid;grid-template-columns:max-content minmax(0,1fr);
  gap:.4rem 1.2rem;margin:0;align-items:baseline}
dl.vocab dt{font-family:rulebook-mono,monospace;font-size:.8rem;font-weight:700}
dl.vocab dt.sup{color:var(--accent)}
dl.vocab dt.fac{color:var(--outside)}
dl.vocab dd{margin:0;font-size:.72rem;line-height:2}
dl.vocab dd a,dl.vocab dd .dead{
  display:inline-block;padding:.1em .4em;margin:0 .1em .15em 0;border-radius:3px;
  text-decoration:none;border:1px solid var(--hair);background:var(--sheet);
  color:var(--ink);
}
dl.vocab dd a:hover{border-color:var(--accent);color:var(--accent)}
dl.vocab dd .dead{color:var(--faint);border-style:dashed}

/* ---- precedence ---------------------------------------------------- */
.scroll{overflow-x:auto}
table.prec{width:100%;border-collapse:collapse;font-size:.82rem}
table.prec th{
  font-family:rulebook-mono,monospace;text-align:left;font-weight:400;
  font-size:.63rem;letter-spacing:.11em;text-transform:uppercase;
  color:var(--faint);border-bottom:1px solid var(--rule);padding:.45rem .6rem;
}
table.prec td{border-bottom:1px solid var(--hair);padding:.45rem .6rem;
  vertical-align:baseline}
table.prec tr:hover td{background:var(--sheet)}
table.prec td.lvl{
  font-family:rulebook-mono,monospace;font-weight:700;text-align:right;
  width:3.2rem;color:var(--accent);font-variant-numeric:tabular-nums;
}
table.prec td.kind{white-space:nowrap;width:9rem}
table.prec td.kind code{background:none;padding:0;color:var(--muted)}
table.prec td a{font-size:.74rem;color:var(--muted);text-decoration:none;
  margin-right:.5rem;white-space:nowrap}
table.prec td a:hover{color:var(--accent)}

/* ---- rules --------------------------------------------------------- */
h3.grp{
  font-family:rulebook-mono,monospace;font-size:.68rem;letter-spacing:.12em;
  text-transform:uppercase;color:var(--faint);font-weight:400;
  margin:2.5rem 0 .7rem;padding-bottom:.35rem;border-bottom:1px dashed var(--hair);
}
section.rule{
  background:var(--sheet);border:1px solid var(--rule);border-radius:4px;
  padding:.9rem 1rem;margin:0 0 .7rem;scroll-margin-top:1rem;
}
section.rule:target{border-color:var(--accent);background:var(--accent-bg)}
.rule-head{display:flex;flex-wrap:wrap;gap:.45rem;align-items:center;
  margin:0 0 .55rem}
.rule-name{font-size:.95rem;font-weight:700;color:var(--ink);
  text-decoration:none}
.rule-name:hover{color:var(--accent)}
.badge{
  font-size:.6rem;letter-spacing:.09em;text-transform:uppercase;
  padding:.22em .45em;border-radius:2px;border:1px solid;line-height:1.4;
}
.b-sup{color:var(--accent);border-color:var(--accent);background:var(--accent-bg)}
.b-ext{color:var(--outside);border-color:var(--outside);background:var(--outside-bg)}
.b-hid{color:var(--faint);border-color:var(--hair)}
.b-vis{color:var(--literal);border-color:var(--literal);background:var(--literal-bg)}
.b-word{color:var(--ink);border-color:var(--muted)}

pre.ebnf{
  margin:0;padding:.6rem .75rem;background:var(--sunk);border-radius:3px;
  font-size:.78rem;line-height:1.95;overflow-x:auto;white-space:pre-wrap;
  word-break:break-word;
}
pre.ebnf b{color:var(--literal);font-weight:700}
pre.ebnf i{color:var(--outside);font-style:normal}
pre.ebnf a{text-decoration:none;border-bottom:1px solid transparent}
pre.ebnf a.sym{color:var(--accent)}
pre.ebnf a.hid{color:var(--muted)}
pre.ebnf a.ext{color:var(--outside)}
pre.ebnf a:hover{border-bottom-color:currentColor}
pre.ebnf .fld{color:var(--literal);opacity:.85;font-size:.9em;margin-right:.15em}
pre.ebnf .al{color:var(--faint);font-size:.9em}
pre.ebnf .eps{color:var(--faint)}

figure.rrbox{margin:.65rem 0 0;overflow-x:auto;padding:.35rem 0}
svg.rr{display:block}
svg.rr path{fill:none;stroke:var(--muted);stroke-width:1.3}
svg.rr path.cap-line{stroke:var(--ink);stroke-width:2.4}
svg.rr text{
  font-family:rulebook-mono,ui-monospace,monospace;font-size:12px;
  font-weight:700;text-anchor:middle;fill:var(--ink);
}
svg.rr text.cap{
  font-size:8.5px;font-weight:400;text-anchor:start;fill:var(--faint);
  letter-spacing:.04em;
}
svg.rr g.term rect{fill:var(--literal-bg);stroke:var(--literal);stroke-width:1.2}
svg.rr g.nonterm rect{fill:var(--accent-bg);stroke:var(--accent);stroke-width:1.1}
svg.rr g.regex rect{fill:var(--sunk);stroke:var(--rule);stroke-width:1.1}
svg.rr g.external rect{fill:var(--outside-bg);stroke:var(--outside);stroke-width:1.2}
svg.rr g.regex text{fill:var(--muted);font-weight:400}
svg.rr rect.field{fill:none;stroke:var(--literal);stroke-dasharray:3 2.5;opacity:.75}
svg.rr rect.prec{fill:none;stroke:var(--accent);stroke-dasharray:1 3;opacity:.7}
svg.rr rect.alias,svg.rr rect.token{fill:none;stroke:var(--rule);stroke-dasharray:3 3}
svg.rr a{cursor:pointer}
svg.rr a:hover rect{stroke-width:2}

.key{display:flex;flex-wrap:wrap;gap:1rem;margin:.4rem 0 1.4rem;
  font-size:.72rem;color:var(--muted)}
.key i{display:inline-block;width:1.6rem;height:.85rem;border-radius:2px;
  border:1px solid;vertical-align:-.1em;margin-right:.35rem}
.key .k-term{background:var(--literal-bg);border-color:var(--literal)}
.key .k-nt{background:var(--accent-bg);border-color:var(--accent);border-radius:1px}
.key .k-ext{background:var(--outside-bg);border-color:var(--outside)}
.key .k-re{background:var(--sunk);border-color:var(--rule)}

footer{border-top:1px solid var(--rule);background:var(--sheet);
  padding:2rem 0 3rem;font-size:.8rem;color:var(--muted)}
footer .wrap{display:flex;flex-wrap:wrap;gap:1rem;justify-content:space-between}
@media (prefers-reduced-motion:reduce){*{animation:none!important;transition:none!important}}
"""


def stylesheet(embed=True):
    return '<style>' + faces(embed) + TOKENS + CSS + '</style>'
