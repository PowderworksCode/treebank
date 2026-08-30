// Mounts the grammar reference into a page.
//
// The Python this replaces wrote a finished HTML file: every production, its
// EBNF and its diagram, plus an embedded 1.24 MiB font, ~1.7 MiB per grammar.
// This asks for the parse table instead (3-11 KiB over the wire) and draws it
// where it is read, which buys three things the file could not have:
//
//   - text is MEASURED, so no face has to be embedded to keep labels inside
//     their boxes;
//   - diagrams are laid out as they are scrolled to, so a 264-production
//     grammar costs what is on screen rather than all of it;
//   - the filter is over the real index rather than a search of rendered HTML.

import {
  escapeHtml as E,
  Grammar,
  groupsOf,
  precedences,
  roleIndex,
  toEbnf,
  toRr,
} from "./grammar.mjs";
import { diagram, measureWith } from "./railroad.mjs";
import { grammarStatus } from "./status.mjs";

// The one measurement the layout engine needs. Taken against the face the
// page will actually paint with, which is why the diagrams no longer carry a
// font: a browser knows the width of its own text.
function measurer() {
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  const probe = document.createElement("span");
  probe.className = "rr-probe";
  document.body.appendChild(probe);
  const font = getComputedStyle(probe).font;
  probe.remove();
  ctx.font = font || "12px ui-monospace, monospace";
  const cache = new Map();
  return (text) => {
    let w = cache.get(text);
    if (w === undefined) {
      w = ctx.measureText(text).width;
      cache.set(text, w);
    }
    return w;
  };
}

function stats(g, precs) {
  const items = [
    ["productions", Object.keys(g.rules).length],
    ["node types", g.visible.size],
    ["supertypes", g.supertypes.size],
    ["external tokens", g.externals.size],
    ["declared conflicts", (g.g.conflicts ?? []).length],
    ["precedence levels", precs.size],
  ];
  return `<div class="grammar-stats">${items
    .map(([label, v]) => `<div><b>${v}</b><span>${label}</span></div>`)
    .join("")}</div>`;
}

function vocabulary(g, table, facet) {
  const parts = [];
  for (const [title, group, cls] of [
    ["Supertypes", table, "sup"],
    ["Facets", facet, "fac"],
  ]) {
    const roles = Object.keys(group).sort();
    if (!roles.length) continue;
    parts.push(`<h3 class="vh">${title}</h3><dl class="vocab">`);
    for (const role of roles) {
      const links = group[role]
        .map((m) =>
          m in g.rules
            ? `<a href="#r-${E(m)}">${E(m)}</a>`
            : `<span class="dead">${E(m)}</span>`,
        )
        .join("");
      parts.push(`<dt class="${cls}">${E(role)}</dt><dd>${links}</dd>`);
    }
    parts.push("</dl>");
  }
  if (!parts.length) return "";
  return `<h2 id="vocabulary">Vocabulary</h2>
<p>The roles a query may name. A <b>supertype</b> is threaded through the
productions, so <code>(_expression)</code> matches where the parse went through
it. A <b>facet</b> is a list of node types in <code>roles.json</code>, expanded
when the query loads.</p>${parts.join("")}`;
}

function precedenceTable(precs) {
  if (!precs.size) return "";
  const rows = [...precs.keys()]
    .sort((a, b) => b - a)
    .map((lvl) => {
      const entries = precs.get(lvl);
      const kinds = [...new Set(entries.map(([k]) => k))].sort();
      const rules = [...new Set(entries.map(([, r]) => r))].sort();
      return `<tr><td class="lvl">${lvl}</td><td class="kind">${kinds
        .map((k) => `<code>${E(k)}</code>`)
        .join(" ")}</td><td>${rules
        .map((r) => `<a href="#r-${E(r)}">${E(r)}</a>`)
        .join("")}</td></tr>`;
    });
  return `<h2 id="precedence">Precedence</h2>
<p>What EBNF cannot show. Higher binds tighter; <code>prec.left</code> and
<code>prec.right</code> pick a side when levels tie. For a <em>declared</em>
conflict only <code>prec.dynamic</code> applies — static precedence is ignored
there.</p>
<div class="scroll"><table class="prec"><thead><tr><th>level</th><th>kind</th>
<th>productions</th></tr></thead><tbody>${rows.join("")}</tbody></table></div>`;
}

function badges(name, g) {
  const out = [];
  if (g.supertypes.has(name)) out.push(["supertype", "sup"]);
  if (g.externals.has(name)) out.push(["external", "ext"]);
  if (g.hidden(name)) out.push(["hidden", "hid"]);
  else if (g.visible.has(name)) out.push(["node", "vis"]);
  if (name === g.word) out.push(["word", "word"]);
  return out;
}

// The EBNF is cheap and goes in immediately; the diagram is the expensive
// half, so the figure is left empty and filled when it is scrolled to.
function ruleBlock(name, g) {
  const marks = badges(name, g)
    .map(([label, cls]) => `<span class="badge b-${cls}">${label}</span>`)
    .join("");
  let ebnf;
  try {
    ebnf = toEbnf(g.rules[name], g);
  } catch (error) {
    ebnf = `<span class="broken">${E(error.message)}</span>`;
  }
  return `<section class="rule" id="r-${E(name)}">
<div class="rule-head"><a class="rule-name mono" href="#r-${E(name)}">${E(name)}</a>${marks}</div>
<pre class="ebnf">${ebnf}</pre>
<figure class="rrbox" data-rule="${E(name)}"></figure>
</section>`;
}

function wire(root, g) {
  // Diagrams are laid out when they come into view. cpp is 264 productions
  // and some of them are large; drawing all of it up front is seconds of
  // blocked main thread for a page the reader scrolls a tenth of.
  const draw = (figure) => {
    if (figure.dataset.drawn) return;
    figure.dataset.drawn = "1";
    const name = figure.dataset.rule;
    try {
      figure.innerHTML = diagram(toRr(g.rules[name], g), name);
    } catch (error) {
      figure.innerHTML = `<p class="broken">${E(error.message)}</p>`;
    }
  };

  const figures = root.querySelectorAll(".rrbox");
  if (typeof IntersectionObserver === "undefined") {
    figures.forEach(draw);
  } else {
    const seen = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          draw(entry.target);
          seen.unobserve(entry.target);
        }
      },
      { rootMargin: "400px 0px" },
    );
    figures.forEach((f) => {
      seen.observe(f);
    });
  }

  // A link into a rule that has not been drawn yet must still land on it.
  const jump = () => {
    const id = location.hash.slice(1);
    if (!id.startsWith("r-")) return;
    const figure = root.querySelector(
      `.rrbox[data-rule="${CSS.escape(id.slice(2))}"]`,
    );
    if (figure) draw(figure);
  };
  addEventListener("hashchange", jump);
  jump();

  const box = root.querySelector(".grammar-filter");
  const links = [...root.querySelectorAll(".idx a[data-name]")];
  const heads = [...root.querySelectorAll(".idx h4")];
  box?.addEventListener("input", () => {
    const q = box.value.trim().toLowerCase();
    for (const a of links) {
      a.hidden = Boolean(q) && !a.dataset.name.includes(q);
    }
    for (const h of heads) {
      let any = false;
      for (
        let n = h.nextElementSibling;
        n && n.tagName === "A";
        n = n.nextElementSibling
      ) {
        if (!n.hidden) {
          any = true;
          break;
        }
      }
      h.hidden = !any;
    }
  });
  box?.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      box.value = "";
      box.dispatchEvent(new Event("input"));
    }
  });
}

export function render(root, bundle, status) {
  const g = new Grammar(bundle);
  const { table, facet } = roleIndex(g);
  const precs = precedences(g);
  const groups = groupsOf(g, table);

  const rail = groups
    .map(
      ([role, members]) =>
        `<h4>${E(role)}</h4>` +
        members
          .map(
            (m) =>
              `<a href="#r-${E(m)}" data-name="${E(m.toLowerCase())}">${E(m)}</a>`,
          )
          .join(""),
    )
    .join("");

  const productions = groups
    .map(
      ([role, members]) =>
        `<h3 class="grp">${E(role)}</h3>` +
        members.map((m) => ruleBlock(m, g)).join(""),
    )
    .join("");

  root.innerHTML = `${stats(g, precs)}
<div class="grammar-cols">
<nav class="idx" aria-label="Productions">
<input class="grammar-filter" type="search" placeholder="filter productions…"
       aria-label="Filter productions">
<div class="idx-scroll">${rail}</div></nav>
<div class="grammar-main">
${status ? grammarStatus(status.grammars?.[g.name], g.name) : ""}
${vocabulary(g, table, facet)}
${precedenceTable(precs)}
<h2 id="productions">Productions</h2>
<div class="key"><span><i class="k-term"></i>literal text</span>
<span><i class="k-nt"></i>another production</span>
<span><i class="k-re"></i>pattern</span>
<span><i class="k-ext"></i>external scanner</span></div>
${productions}
</div></div>`;

  wire(root, g);
  return g;
}

async function mount(root) {
  const name = root.dataset.grammar;
  try {
    // The inventory is a separate, much smaller request, and a missing or
    // broken one must not cost the reader the parse table: the grammar is the
    // page, the status is an addition to it.
    const [response, status] = await Promise.all([
      fetch(`/grammars/${name}.json`),
      fetch("/status.json")
        .then((r) => (r.ok ? r.json() : null))
        .catch(() => null),
    ]);
    if (!response.ok) throw new Error(`${response.status} fetching ${name}`);
    // Layout depends on text metrics, and metrics before the webfont lands
    // are the fallback's. Waiting means measuring what will be painted.
    await document.fonts?.ready;
    measureWith(measurer());
    render(root, await response.json(), status);
  } catch (error) {
    root.innerHTML = `<p class="broken">Could not render ${E(name)}: ${E(error.message)}</p>`;
  }
}

for (const root of document.querySelectorAll(".grammar-viewer")) mount(root);
