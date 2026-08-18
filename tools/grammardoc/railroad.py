"""A railroad-diagram layout engine, small enough to read.

Every node knows three numbers: how wide it is, how far it reaches ABOVE
its entry line, and how far BELOW. Layout is then a matter of parents
asking children for those and handing back x/y. Radius-10 arcs join the
pieces, which is the convention every railroad renderer since Wirth has
used.
"""

AR = 10          # arc radius
CH = 7.23       # DejaVu Sans Mono advance (0.60205 em) at 12px, embedded
VS = 8           # vertical space between choice branches


def esc(s):
    return (s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
             .replace('"', '&quot;'))


class Node:
    def __init__(self):
        self.width = 0
        self.up = 0
        self.down = 0

    @property
    def height(self):
        return self.up + self.down


class Skip(Node):
    def __init__(self):
        super().__init__()
        self.width, self.up, self.down = 0, 0, 0

    def draw(self, x, y, width, out):
        out.append(f'<path d="M{x} {y}h{width}"/>')


class Leaf(Node):
    """A terminal (rounded) or a non-terminal (square)."""

    def __init__(self, text, cls, href=None, title=None):
        super().__init__()
        self.text, self.cls, self.href, self.title = text, cls, href, title
        self.width = max(len(text) * CH + 20, 26)
        self.up = self.down = 12

    def draw(self, x, y, width, out):
        pad = (width - self.width) / 2
        if pad > 0:
            out.append(f'<path d="M{x} {y}h{pad}"/>')
            out.append(f'<path d="M{x + width - pad} {y}h{pad}"/>')
            x += pad
        r = 10 if self.cls == 'term' else 2
        out.append(f'<g class="{self.cls}">')
        if self.href:
            out.append(f'<a href="{esc(self.href)}">')
        out.append(
            f'<rect x="{x:.1f}" y="{y - 12}" width="{self.width:.1f}" '
            f'height="24" rx="{r}" ry="{r}"/>')
        out.append(
            f'<text x="{x + self.width / 2:.1f}" y="{y + 4}">{esc(self.text)}</text>')
        if self.title:
            out.append(f'<title>{esc(self.title)}</title>')
        if self.href:
            out.append('</a>')
        out.append('</g>')


class Seq(Node):
    def __init__(self, items):
        super().__init__()
        self.items = [i for i in items if not isinstance(i, Skip)] or [Skip()]
        self.width = sum(i.width for i in self.items) + 10 * (len(self.items) - 1)
        self.up = max(i.up for i in self.items)
        self.down = max(i.down for i in self.items)

    def draw(self, x, y, width, out):
        pad = (width - self.width) / 2
        if pad > 0:
            out.append(f'<path d="M{x} {y}h{pad}"/>')
            x += pad
        for n, item in enumerate(self.items):
            if n:
                out.append(f'<path d="M{x} {y}h10"/>')
                x += 10
            item.draw(x, y, item.width, out)
            x += item.width
        if pad > 0:
            out.append(f'<path d="M{x} {y}h{pad}"/>')


class Choice(Node):
    """Branches stacked vertically; `default` stays on the entry line.

    The branch offsets are computed ONCE, in the constructor, and `draw`
    reads them back. Sizing and drawing from two separate formulas is how
    a diagram ends up with one branch printed on top of another.
    """

    def __init__(self, items, default=0):
        super().__init__()
        self.items, self.default = items, default
        self.width = max(i.width for i in items) + 4 * AR
        d = items[default]
        self.up, self.down = d.up, d.down
        self.dy = {}                       # branch index -> offset from entry

        cursor = d.up                      # grow upward
        for n in range(default - 1, -1, -1):
            item = items[n]
            dy = max(2 * AR, cursor + VS + item.down)
            self.dy[n] = -dy
            cursor = dy + item.up
        self.up = cursor

        cursor = d.down                    # grow downward
        for n in range(default + 1, len(items)):
            item = items[n]
            dy = max(2 * AR, cursor + VS + item.up)
            self.dy[n] = dy
            cursor = dy + item.down
        self.down = cursor

    def draw(self, x, y, width, out):
        inner = self.width - 4 * AR
        pad = (width - self.width) / 2
        if pad > 0:
            out.append(f'<path d="M{x} {y}h{pad}"/>')
            out.append(f'<path d="M{x + width - pad} {y}h{pad}"/>')
            x += pad
        d = self.items[self.default]
        out.append(f'<path d="M{x} {y}h{2 * AR}"/>')
        d.draw(x + 2 * AR, y, inner, out)
        out.append(f'<path d="M{x + 2 * AR + inner} {y}h{2 * AR}"/>')

        for n, item in enumerate(self.items):
            if n == self.default:
                continue
            dy = self.dy[n]
            up = dy < 0
            s = -1 if up else 1
            span = abs(dy) - 2 * AR      # >= 0 by construction
            out.append(f'<path d="M{x} {y}'
                       f'a{AR} {AR} 0 0 {1 if up else 0} {AR} {s * AR}'
                       f'v{s * span}'
                       f'a{AR} {AR} 0 0 {0 if up else 1} {AR} {s * AR}"/>')
            item.draw(x + 2 * AR, y + dy, inner, out)
            out.append(f'<path d="M{x + 2 * AR + inner} {y + dy}'
                       f'a{AR} {AR} 0 0 {0 if up else 1} {AR} {-s * AR}'
                       f'v{-s * span}'
                       f'a{AR} {AR} 0 0 {1 if up else 0} {AR} {-s * AR}"/>')


def Optional(item):
    return Choice([Skip(), item], default=1)


class Repeat(Node):
    """`item` once, then a loop back under it -- optionally through `sep`.

    Same discipline as Choice: the loop's depth is computed here and read
    back by draw, never recomputed."""

    def __init__(self, item, sep=None):
        super().__init__()
        self.item, self.sep = item, sep or Skip()
        self.width = max(item.width, self.sep.width) + 4 * AR
        self.up = item.up
        self.dy = max(2 * AR, item.down + VS + self.sep.up)
        self.down = self.dy + self.sep.down

    def draw(self, x, y, width, out):
        inner = self.width - 4 * AR
        pad = (width - self.width) / 2
        if pad > 0:
            out.append(f'<path d="M{x} {y}h{pad}"/>')
            out.append(f'<path d="M{x + width - pad} {y}h{pad}"/>')
            x += pad
        out.append(f'<path d="M{x} {y}h{2 * AR}"/>')
        self.item.draw(x + 2 * AR, y, inner, out)
        out.append(f'<path d="M{x + 2 * AR + inner} {y}h{2 * AR}"/>')
        dy, span = self.dy, self.dy - 2 * AR
        out.append(f'<path d="M{x + self.width} {y}'
                   f'a{AR} {AR} 0 0 1 {-AR} {AR}'
                   f'v{span}'
                   f'a{AR} {AR} 0 0 1 {-AR} {AR}"/>')
        self.sep.draw(x + 2 * AR, y + dy, inner, out)
        out.append(f'<path d="M{x + 2 * AR} {y + dy}'
                   f'a{AR} {AR} 0 0 1 {-AR} {-AR}'
                   f'v{-span}'
                   f'a{AR} {AR} 0 0 1 {-AR} {-AR}"/>')


class Labelled(Node):
    """A dashed box with a caption — a field name, or `prec.left 12`."""

    def __init__(self, item, label, cls='label'):
        super().__init__()
        self.item, self.label, self.cls = item, label, cls
        self.width = max(item.width + 16, len(label) * 6 + 16)
        self.up = item.up + 6
        self.down = item.down + 16

    def draw(self, x, y, width, out):
        pad = (width - self.width) / 2
        if pad > 0:
            out.append(f'<path d="M{x} {y}h{pad}"/>')
            out.append(f'<path d="M{x + width - pad} {y}h{pad}"/>')
            x += pad
        out.append(
            f'<rect class="{self.cls}" x="{x + 1:.1f}" y="{y - self.up:.1f}" '
            f'width="{self.width - 2:.1f}" height="{self.height - 4:.1f}" rx="3"/>')
        out.append(f'<text class="cap" x="{x + 6:.1f}" '
                   f'y="{y + self.item.down + 9:.1f}">{esc(self.label)}</text>')
        out.append(f'<path d="M{x} {y}h8"/>')
        self.item.draw(x + 8, y, self.width - 16, out)
        out.append(f'<path d="M{x + self.width - 8} {y}h8"/>')


def diagram(item, title=None):
    pad = 12
    body = Seq([item])
    w = body.width + 4 * AR + pad * 2
    h = body.height + pad * 2
    y = body.up + pad
    out = [f'<svg class="rr" viewBox="0 0 {w:.0f} {h:.0f}" width="{w:.0f}" '
           f'height="{h:.0f}" xmlns="http://www.w3.org/2000/svg">']
    if title:
        out.append(f'<title>{esc(title)}</title>')
    x = pad
    # start marker
    out.append(f'<path class="cap-line" d="M{x} {y - 6}v12"/>')
    out.append(f'<path d="M{x} {y}h{2 * AR}"/>')
    body.draw(x + 2 * AR, y, body.width, out)
    x2 = x + 2 * AR + body.width
    out.append(f'<path d="M{x2} {y}h{2 * AR}"/>')
    out.append(f'<path class="cap-line" d="M{x2 + 2 * AR} {y - 6}v12"/>')
    out.append('</svg>')
    return '\n'.join(out)
