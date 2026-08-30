// A railroad-diagram layout engine, small enough to read.
//
// Every node knows three numbers: how wide it is, how far it reaches ABOVE
// its entry line, and how far BELOW. Layout is then a matter of parents
// asking children for those and handing back x/y. Radius-10 arcs join the
// pieces, which is the convention every railroad renderer since Wirth has
// used.
//
// Sizing happens in the constructor and drawing reads those numbers back,
// never recomputing them. Both layout bugs in this engine's history were a
// node sized by one formula and drawn by another, which reads as plausible
// source and as obvious nonsense in a picture.
//
// The one thing this port does differently from its Python original: text is
// MEASURED rather than assumed. The Python computed box widths from a fixed
// character advance and therefore had to embed the very face it assumed,
// ~1.24 MiB per page, or every label would clip under a fallback. A browser
// can measure its own text, so the constant and the font both go away.

export const AR = 10; // arc radius
export const VS = 8; // vertical space between choice branches

// Captions inside a Labelled box are set smaller than the labels inside a
// Leaf. The ratio lives here rather than as a bare number in a width
// formula, because it has to match the two font sizes the stylesheet sets:
// change one and this is the other.
export const CAP_RATIO = 10 / 12;

const esc = (s) =>
  String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

// Neumaier summation. Widths are floats and a sequence can hold dozens of
// them; adding left to right lets error accumulate into a coordinate. This
// keeps a diagram a function of its grammar rather than of its addition
// order, which is the same property the Python original reached for.
function sum(values) {
  let total = 0,
    compensation = 0;
  for (const value of values) {
    const next = total + value;
    compensation +=
      Math.abs(total) >= Math.abs(value)
        ? total - next + value
        : value - next + total;
    total = next;
  }
  return total + compensation;
}

// One number the whole engine depends on. Injected rather than imported so
// the browser can measure against the face it will actually paint with, and
// a test can pin a deterministic advance instead.
let advance = (text) => text.length * 7.23;
export function measureWith(fn) {
  advance = fn;
}

class Node {
  constructor() {
    this.width = 0;
    this.up = 0;
    this.down = 0;
  }
  get height() {
    return this.up + this.down;
  }
}

export class Skip extends Node {
  draw(x, y, width, out) {
    out.push(`<path d="M${x} ${y}h${width}"/>`);
  }
}

// A terminal (rounded) or a non-terminal (square).
export class Leaf extends Node {
  constructor(text, cls, { href, title } = {}) {
    super();
    this.text = text;
    this.cls = cls;
    this.href = href;
    this.title = title;
    this.width = Math.max(advance(text) + 20, 26);
    this.up = this.down = 12;
  }

  draw(x, y, width, out) {
    const pad = (width - this.width) / 2;
    if (pad > 0) {
      out.push(`<path d="M${x} ${y}h${pad}"/>`);
      out.push(`<path d="M${x + width - pad} ${y}h${pad}"/>`);
      x += pad;
    }
    const r = this.cls === "term" ? 10 : 2;
    out.push(`<g class="${this.cls}">`);
    if (this.href) out.push(`<a href="${esc(this.href)}">`);
    out.push(
      `<rect x="${x.toFixed(1)}" y="${y - 12}" width="${this.width.toFixed(1)}" ` +
        `height="24" rx="${r}" ry="${r}"/>`,
    );
    out.push(
      `<text x="${(x + this.width / 2).toFixed(1)}" y="${y + 4}">${esc(this.text)}</text>`,
    );
    if (this.title) out.push(`<title>${esc(this.title)}</title>`);
    if (this.href) out.push("</a>");
    out.push("</g>");
  }
}

export class Seq extends Node {
  constructor(items) {
    super();
    const kept = items.filter((i) => !(i instanceof Skip));
    this.items = kept.length ? kept : [new Skip()];
    this.width =
      sum(this.items.map((i) => i.width)) + 10 * (this.items.length - 1);
    this.up = Math.max(...this.items.map((i) => i.up));
    this.down = Math.max(...this.items.map((i) => i.down));
  }

  draw(x, y, width, out) {
    const pad = (width - this.width) / 2;
    if (pad > 0) {
      out.push(`<path d="M${x} ${y}h${pad}"/>`);
      x += pad;
    }
    this.items.forEach((item, n) => {
      if (n) {
        out.push(`<path d="M${x} ${y}h10"/>`);
        x += 10;
      }
      item.draw(x, y, item.width, out);
      x += item.width;
    });
    if (pad > 0) out.push(`<path d="M${x} ${y}h${pad}"/>`);
  }
}

// Branches stacked vertically; `default` stays on the entry line. The branch
// offsets are computed ONCE, here, and `draw` reads them back.
export class Choice extends Node {
  constructor(items, defaultIndex = 0) {
    super();
    this.items = items;
    this.default = defaultIndex;
    this.width = Math.max(...items.map((i) => i.width)) + 4 * AR;
    const d = items[defaultIndex];
    this.dy = new Map(); // branch index -> offset from the entry line

    let cursor = d.up; // grow upward
    for (let n = defaultIndex - 1; n >= 0; n--) {
      const item = items[n];
      const dy = Math.max(2 * AR, cursor + VS + item.down);
      this.dy.set(n, -dy);
      cursor = dy + item.up;
    }
    this.up = cursor;

    cursor = d.down; // grow downward
    for (let n = defaultIndex + 1; n < items.length; n++) {
      const item = items[n];
      const dy = Math.max(2 * AR, cursor + VS + item.up);
      this.dy.set(n, dy);
      cursor = dy + item.down;
    }
    this.down = cursor;
  }

  draw(x, y, width, out) {
    const inner = this.width - 4 * AR;
    const pad = (width - this.width) / 2;
    if (pad > 0) {
      out.push(`<path d="M${x} ${y}h${pad}"/>`);
      out.push(`<path d="M${x + width - pad} ${y}h${pad}"/>`);
      x += pad;
    }
    const d = this.items[this.default];
    out.push(`<path d="M${x} ${y}h${2 * AR}"/>`);
    d.draw(x + 2 * AR, y, inner, out);
    out.push(`<path d="M${x + 2 * AR + inner} ${y}h${2 * AR}"/>`);

    this.items.forEach((item, n) => {
      if (n === this.default) return;
      const dy = this.dy.get(n);
      const up = dy < 0;
      const s = up ? -1 : 1;
      const span = Math.abs(dy) - 2 * AR; // >= 0 by construction
      out.push(
        `<path d="M${x} ${y}` +
          `a${AR} ${AR} 0 0 ${up ? 1 : 0} ${AR} ${s * AR}` +
          `v${s * span}` +
          `a${AR} ${AR} 0 0 ${up ? 0 : 1} ${AR} ${s * AR}"/>`,
      );
      item.draw(x + 2 * AR, y + dy, inner, out);
      out.push(
        `<path d="M${x + 2 * AR + inner} ${y + dy}` +
          `a${AR} ${AR} 0 0 ${up ? 0 : 1} ${AR} ${-s * AR}` +
          `v${-s * span}` +
          `a${AR} ${AR} 0 0 ${up ? 1 : 0} ${AR} ${-s * AR}"/>`,
      );
    });
  }
}

export const Optional = (item) => new Choice([new Skip(), item], 1);

// `item` once, then a loop back under it -- optionally through `sep`.
export class Repeat extends Node {
  constructor(item, sep) {
    super();
    this.item = item;
    this.sep = sep ?? new Skip();
    this.width = Math.max(item.width, this.sep.width) + 4 * AR;
    this.up = item.up;
    this.dy = Math.max(2 * AR, item.down + VS + this.sep.up);
    this.down = this.dy + this.sep.down;
  }

  draw(x, y, width, out) {
    const inner = this.width - 4 * AR;
    const pad = (width - this.width) / 2;
    if (pad > 0) {
      out.push(`<path d="M${x} ${y}h${pad}"/>`);
      out.push(`<path d="M${x + width - pad} ${y}h${pad}"/>`);
      x += pad;
    }
    out.push(`<path d="M${x} ${y}h${2 * AR}"/>`);
    this.item.draw(x + 2 * AR, y, inner, out);
    out.push(`<path d="M${x + 2 * AR + inner} ${y}h${2 * AR}"/>`);
    const dy = this.dy,
      span = this.dy - 2 * AR;
    out.push(
      `<path d="M${x + this.width} ${y}` +
        `a${AR} ${AR} 0 0 1 ${-AR} ${AR}` +
        `v${span}` +
        `a${AR} ${AR} 0 0 1 ${-AR} ${AR}"/>`,
    );
    this.sep.draw(x + 2 * AR, y + dy, inner, out);
    out.push(
      `<path d="M${x + 2 * AR} ${y + dy}` +
        `a${AR} ${AR} 0 0 1 ${-AR} ${-AR}` +
        `v${-span}` +
        `a${AR} ${AR} 0 0 1 ${-AR} ${-AR}"/>`,
    );
  }
}

// A dashed box with a caption -- a field name, or `prec.left 12`.
export class Labelled extends Node {
  constructor(item, label, cls = "label") {
    super();
    this.item = item;
    this.label = label;
    this.cls = cls;
    this.width = Math.max(item.width + 16, advance(label) * CAP_RATIO + 16);
    this.up = item.up + 6;
    this.down = item.down + 16;
  }

  draw(x, y, width, out) {
    const pad = (width - this.width) / 2;
    if (pad > 0) {
      out.push(`<path d="M${x} ${y}h${pad}"/>`);
      out.push(`<path d="M${x + width - pad} ${y}h${pad}"/>`);
      x += pad;
    }
    out.push(
      `<rect class="${this.cls}" x="${(x + 1).toFixed(1)}" y="${(y - this.up).toFixed(1)}" ` +
        `width="${(this.width - 2).toFixed(1)}" height="${(this.height - 4).toFixed(1)}" rx="3"/>`,
    );
    out.push(
      `<text class="cap" x="${(x + 6).toFixed(1)}" ` +
        `y="${(y + this.item.down + 9).toFixed(1)}">${esc(this.label)}</text>`,
    );
    out.push(`<path d="M${x} ${y}h8"/>`);
    this.item.draw(x + 8, y, this.width - 16, out);
    out.push(`<path d="M${x + this.width - 8} ${y}h8"/>`);
  }
}

export function diagram(item, title) {
  const pad = 12;
  const body = new Seq([item]);
  const w = body.width + 4 * AR + pad * 2;
  const h = body.height + pad * 2;
  const y = body.up + pad;
  const out = [
    `<svg class="rr" viewBox="0 0 ${w.toFixed(0)} ${h.toFixed(0)}" ` +
      `width="${w.toFixed(0)}" height="${h.toFixed(0)}" ` +
      `xmlns="http://www.w3.org/2000/svg">`,
  ];
  if (title) out.push(`<title>${esc(title)}</title>`);
  const x = pad;
  out.push(`<path class="cap-line" d="M${x} ${y - 6}v12"/>`); // start marker
  out.push(`<path d="M${x} ${y}h${2 * AR}"/>`);
  body.draw(x + 2 * AR, y, body.width, out);
  const x2 = x + 2 * AR + body.width;
  out.push(`<path d="M${x2} ${y}h${2 * AR}"/>`);
  out.push(`<path class="cap-line" d="M${x2 + 2 * AR} ${y - 6}v12"/>`);
  out.push("</svg>");
  return out.join("\n");
}
