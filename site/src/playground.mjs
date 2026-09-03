// Parse something with a real treebank grammar, in the browser.
//
// The parser here is the same artifact a consumer downloads: one wasm pack,
// byte-reproducible, carrying its own provenance. Nothing is re-implemented
// for the web and nothing is approximated -- if this page disagrees with the
// CLI about a tree, one of them is wrong and it matters.
//
// The tree is the point, so the tree is what is shown: every named node, its
// field name where it has one, and its byte range. Node types link into the
// grammar reference, which is the pairing this site exists to make possible
// -- see a `function_definition` in your own code, click it, read the
// production that admitted it.

import { expandQuery } from "./expand.mjs";
import { errorsIn, Pack, Query, walk } from "./pack.mjs";
import { SAMPLES } from "./samples.mjs";

const E = (s) =>
  String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#x27;");

// Packs are content-addressed: `treebank-python-<hash>.wasm` is those bytes
// or does not exist, because the build asserts byte reproducibility. The
// manifest says which hash is current, and `treebank-python.wasm` is the
// moving pointer for anyone who does not care.
//
// Resolving through the manifest rather than fetching the pointer buys the
// thing a playground otherwise cannot offer: the URL in the address bar names
// the exact parser, so a report about a mis-parse can be reproduced instead of
// being about whatever was current that day.
const PACKS = "/packs/";
const POINTER_URL = (name) => `${PACKS}treebank-${name}.wasm`;
const HASHED_URL = (name, hash) =>
  `${PACKS}treebank-${name}-${hash.slice(0, 12)}.wasm`;

// tb_parse runs on the main thread. A megabyte of source is a fraction of a
// second and fine; ten is a frozen tab. The cap is honest about what this
// page is for -- trying constructs, not parsing a repository.
const MAX_BYTES = 400_000;
// A tree view is one DOM node per syntax node. Past a few thousand the cost
// is the browser's layout, not the parse, so the walk stops and says so.
const MAX_NODES = 4_000;

// A capture list is one row each; a query like `(_) @x` matches everything.
const MAX_CAPTURES = 500;

// The box starts with a query that works, because a nominal term is easier
// to understand from its expansion than from a description of one. `_callable`
// exists in every grammar, which is the point of the shared vocabulary.
const DEFAULT_QUERY = "(_callable) @callable";

class Playground {
  constructor(root) {
    this.root = root;
    this.pack = null;
    this.name = null;
    this.timer = null;
    this.render();
  }

  render() {
    this.root.innerHTML = `
<div class="pg-bar">
  <label>Grammar <select class="pg-grammar"></select></label>
  <span class="pg-state dim">choose a grammar</span>
</div>
<div class="pg-cols">
  <div class="pg-pane">
    <label class="pg-label" for="pg-src">Source</label>
    <textarea id="pg-src" class="pg-src" spellcheck="false" autocapitalize="off"
              autocomplete="off" autocorrect="off" wrap="off"
              placeholder="Pick a grammar, then type here."></textarea>
    <div class="pg-errors"></div>
  </div>
  <div class="pg-pane">
    <label class="pg-label">Tree</label>
    <div class="pg-tree"><p class="dim">No parse yet.</p></div>
  </div>
</div>
<div class="pg-query">
  <label class="pg-label" for="pg-q">Query</label>
  <input id="pg-q" class="pg-q mono" spellcheck="false" autocapitalize="off"
         autocomplete="off" autocorrect="off">
  <div class="pg-qhint dim"></div>
  <div class="pg-qout"></div>
</div>
<div class="pg-prov dim"></div>`;

    this.select = this.root.querySelector(".pg-grammar");
    this.source = this.root.querySelector(".pg-src");
    this.treeBox = this.root.querySelector(".pg-tree");
    this.errorBox = this.root.querySelector(".pg-errors");
    this.stateBox = this.root.querySelector(".pg-state");
    this.provBox = this.root.querySelector(".pg-prov");
    this.queryBox = this.root.querySelector(".pg-q");
    this.queryOut = this.root.querySelector(".pg-qout");
    this.queryHint = this.root.querySelector(".pg-qhint");

    this.select.addEventListener("change", () =>
      this.choose(this.select.value),
    );
    this.source.addEventListener("input", () => this.schedule());
    this.queryBox.addEventListener("input", () => this.schedule());
    // A capture names a byte range, so clicking one can show it rather than
    // describe it.
    this.queryOut.addEventListener("click", (event) => {
      const row = event.target.closest("[data-start]");
      if (row) this.select_(Number(row.dataset.start), Number(row.dataset.end));
    });
    this.fillGrammars();
  }

  // The list is the same one the reference pages are generated from, so a
  // grammar cannot appear in one and be missing from the other.
  async fillGrammars() {
    try {
      const response = await fetch("/grammars/index.json");
      if (!response.ok) throw new Error(`${response.status}`);
      const grammars = await response.json();
      this.select.innerHTML = grammars
        .map((g) => `<option value="${E(g.name)}">${E(g.name)}</option>`)
        .join("");
      const params = new URLSearchParams(location.search);
      const wanted = params.get("g");
      const start = grammars.some((g) => g.name === wanted) ? wanted : "python";
      const pinned = /^[0-9a-f]{12,64}$/.test(params.get("pack") ?? "")
        ? params.get("pack")
        : null;
      this.select.value = start;
      await this.choose(start, pinned);
    } catch (error) {
      this.stateBox.textContent = `could not list grammars: ${error.message}`;
    }
  }

  // Which deployment this is, when the answer changes what a failure means.
  // Only read after something has already gone wrong, so the ordinary path
  // costs nothing.
  async preview() {
    try {
      const response = await fetch(`${PACKS}preview.json`);
      if (!response.ok) return null;
      const parsed = await response.json();
      return typeof parsed?.branch === "string" ? parsed : null;
    } catch {
      return null;
    }
  }

  // The manifest is advisory: without it the pointer still works, and the
  // page loses pinning rather than the parser.
  async manifest() {
    if (this.packs !== undefined) return this.packs;
    try {
      const response = await fetch(`${PACKS}index.json`);
      this.packs = response.ok ? ((await response.json()).packs ?? null) : null;
    } catch {
      this.packs = null;
    }
    return this.packs;
  }

  async choose(name, wantHash) {
    if (this.name === name && !wantHash) return;
    this.name = name;
    this.dropQuery();
    this.pack = null;
    this.treeBox.innerHTML = '<p class="dim">Loading the parser…</p>';
    this.errorBox.innerHTML = "";
    this.provBox.textContent = "";
    this.stateBox.textContent = `loading treebank-${name}.wasm…`;

    const packs = await this.manifest();
    const entry = packs?.[name];
    // An explicit ?pack= wins over the manifest: pinning is the whole point,
    // so a pin must survive the pointer moving underneath it.
    const hash = wantHash ?? entry?.sha256;
    const url = hash ? HASHED_URL(name, hash) : POINTER_URL(name);
    this.hash = hash ?? null;

    const started = performance.now();
    try {
      this.pack = await Pack.load(url);
    } catch (error) {
      this.stateBox.textContent = "";
      // A missing pack means different things in different places, and the
      // unhelpful version of this message told a reviewer on a preview URL to
      // go and run a build script on their laptop.
      const preview = await this.preview();
      const why = preview
        ? `<p class="dim">This is a preview of <code>${E(preview.branch)}</code>. Its packs are
published by CI after the build that made this page, so a grammar this branch
adds appears here a few minutes after its checks go green — reload then. A
grammar the branch did not change is served from what <code>main</code>
published, so this one is new here.</p>`
        : `<p class="dim">The packs are built artifacts. If this is a local checkout, run
<code>./tools/wasm-pack/build.sh ${E(name)} --out site/public/packs</code>
then <code>bun run packs</code>.</p>`;
      this.treeBox.innerHTML = `<p class="broken">Could not load the ${E(name)} parser: ${E(error.message)}</p>
${why}`;
      return;
    }
    const ms = Math.round(performance.now() - started);
    this.stateBox.textContent = `treebank-${name} loaded in ${ms} ms`;
    this.showProvenance();
    if (!this.source.value.trim() || this.sampleShown) {
      this.source.value = SAMPLES[name] ?? "";
      this.sampleShown = true;
    }
    if (!this.queryBox.value.trim() || this.queryShown) {
      this.queryBox.value = DEFAULT_QUERY;
      this.queryShown = true;
    }
    this.showNominal();
    this.parse();
  }

  showProvenance() {
    const p = this.pack.provenance;
    const nominal = Object.keys(this.pack.terms.nominal ?? {});
    const bits = [
      `grammar <b>${E(p.grammar_name ?? p.language)}</b>`,
      `vocabulary ${E(p.vocabulary ?? "?")}`,
      `tree-sitter ${E(p.generate_cli ?? "?")}`,
      nominal.length ? `nominal ${nominal.map(E).join(" ")}` : null,
    ].filter(Boolean);
    // Read out of the module's own bytes, not from a caption beside it.
    // The permalink is the useful half: it names the exact parser, so a
    // report about a mis-parse can be reproduced rather than being about
    // whatever happened to be current that day.
    const pin = this.hash
      ? ` <a class="pg-pin" href="?g=${E(this.name)}&pack=${E(this.hash.slice(0, 12))}"
title="A link to this exact parser, which cannot change under you">pack ${E(
          this.hash.slice(0, 12),
        )} — permalink</a>`
      : ' <span class="dim">(unpinned: no manifest, so this is whatever is current)</span>';
    this.provBox.innerHTML =
      `Read from the pack itself: ${bits.join(" · ")}.${pin}<br>` +
      `<a href="/grammars/${E(this.name)}/">Read the ${E(this.name)} grammar reference →</a>`;
  }

  schedule() {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.parse(), 120);
  }

  parse() {
    if (!this.pack) return;
    const text = this.source.value;
    const bytes = new TextEncoder().encode(text).length;
    if (bytes > MAX_BYTES) {
      this.treeBox.innerHTML = `<p class="broken">${bytes.toLocaleString()} bytes is past
this page's ${MAX_BYTES.toLocaleString()}-byte cap. The parser is not the limit — it
runs on the main thread here, and a large paste would freeze the tab rather
than fail honestly.</p>`;
      return;
    }

    let tree, root;
    const started = performance.now();
    try {
      tree = this.pack.parse(text);
      root = tree.root();
    } catch (error) {
      this.treeBox.innerHTML = `<p class="broken">${E(error.message)}</p>`;
      return;
    }
    const ms = performance.now() - started;

    try {
      this.showErrors(root);
      this.showTree(root, ms, bytes);
      this.runQuery(root, text);
    } finally {
      this.pack.e.tb_node_free(root);
      tree.free();
    }
  }

  showErrors(root) {
    const errors = errorsIn(this.pack, root);
    if (!errors.length) {
      this.errorBox.innerHTML = '<p class="pg-ok">Parses cleanly.</p>';
      return;
    }
    const rows = errors
      .slice(0, 25)
      .map(
        (e) =>
          `<li><span class="pg-badge ${e.kind === "MISSING" ? "missing" : "err"}">${e.kind}</span>
<span class="mono">${e.row + 1}:${e.column + 1}</span>
${e.type ? `<span class="dim mono">${E(e.type)}</span>` : ""}</li>`,
      )
      .join("");
    this.errorBox.innerHTML = `<p class="pg-bad">${errors.length} error node${
      errors.length === 1 ? "" : "s"
    }.</p><ul class="pg-errlist">${rows}</ul>${
      errors.length > 25 ? '<p class="dim">…first 25 shown.</p>' : ""
    }`;
  }

  // The nominal terms this grammar declares, named under the box: a query
  // cannot be written against a vocabulary nobody has listed.
  showNominal() {
    if (!this.pack) return;
    const nominal = Object.keys(this.pack.terms.nominal ?? {}).sort();
    if (!this.pack.canQuery) {
      this.queryHint.innerHTML =
        `This pack is <b>pack_abi ${E(this.pack.provenance.pack_abi)}</b> and queries need 3. ` +
        `It parses fine; it was built before packs could run queries.`;
      this.queryBox.disabled = true;
      return;
    }
    this.queryBox.disabled = false;
    this.queryHint.innerHTML =
      `Nominal terms in ${E(this.name)}: ` +
      nominal
        .map(
          (f) => `<button type="button" class="pg-term mono">${E(f)}</button>`,
        )
        .join(" ") +
      ` — expanded here before the query runs, the same way ` +
      `<code>Pack::query</code> expands them.`;
    for (const button of this.queryHint.querySelectorAll(".pg-term")) {
      button.addEventListener("click", () => {
        this.queryBox.value = `(${button.textContent}) @hit`;
        this.parse();
      });
    }
  }

  dropQuery() {
    if (this.compiled) {
      try {
        this.compiled.query.free();
      } catch {
        // The pack it belonged to is going away regardless.
      }
      this.compiled = null;
    }
  }

  // Compiling is the expensive half, so it is kept while the query text is
  // unchanged -- which is every keystroke in the SOURCE box.
  compile(expanded) {
    if (this.compiled?.text === expanded) return this.compiled.query;
    this.dropQuery();
    const query = new Query(this.pack, expanded);
    this.compiled = { text: expanded, query };
    return query;
  }

  runQuery(root, text) {
    if (!this.pack?.canQuery) return;
    const source = this.queryBox.value.trim();
    if (!source) {
      this.queryOut.innerHTML =
        '<p class="dim">Write a query to run it against the tree above.</p>';
      return;
    }

    let expanded, query;
    try {
      // With node-types, so a member that cannot take a field the pattern
      // asks for is dropped rather than making the whole alternation an
      // impossible pattern. Same call the crate makes.
      expanded = expandQuery(
        source,
        this.pack.terms.nominal ?? {},
        this.pack.nodeTypes,
      );
      query = this.compile(expanded);
    } catch (error) {
      this.dropQuery();
      // When expansion changed the query, the failure is usually about the
      // expansion rather than about what was typed -- so show it. An expanded
      // nominal term fails as a whole when any one member cannot take a field the
      // pattern asks for, and seeing the alternation is what makes that
      // legible instead of mysterious.
      const shown =
        expanded && expanded !== source
          ? `<pre class="pg-expanded mono">${E(expanded)}</pre>`
          : "";
      this.queryOut.innerHTML = `<p class="broken">${E(error.message)}</p>${shown}`;
      return;
    }

    const { captures, truncated } = query.run(root, { limit: MAX_CAPTURES });
    const expansion =
      expanded !== source
        ? `<details class="pg-expansion"><summary class="dim">expanded to ${E(
            query.patternCount,
          )} pattern${query.patternCount === 1 ? "" : "s"}</summary>` +
          `<pre class="pg-expanded mono">${E(expanded)}</pre></details>`
        : "";

    if (!captures.length) {
      this.queryOut.innerHTML = `${expansion}<p class="dim">No matches in this source.</p>`;
      return;
    }

    const rows = captures
      .map(
        (c) =>
          `<li data-start="${c.startByte}" data-end="${c.endByte}" title="Select this range in the source">
<span class="pg-cap mono">@${E(c.name)}</span>
<a class="pg-type" href="/grammars/${E(this.name)}/#r-${E(c.type)}"
   title="Read the production for ${E(c.type)}">${E(c.type)}</a>
<span class="mono dim">${c.startRow + 1}:${c.startColumn + 1}</span>
<span class="pg-span">${c.startByte}–${c.endByte}</span></li>`,
      )
      .join("");

    this.queryOut.innerHTML =
      `${expansion}<p class="pg-ok">${captures.length}${
        truncated ? "+" : ""
      } capture${captures.length === 1 ? "" : "s"}.</p>` +
      `<ul class="pg-caplist">${rows}</ul>` +
      (truncated
        ? `<p class="dim">Stopped at ${MAX_CAPTURES.toLocaleString()}.</p>`
        : "");
  }

  // Byte offsets are what a capture carries; a textarea counts UTF-16 code
  // units. Decoding the prefix is the conversion, and it is only ever done
  // for one click.
  select_(startByte, endByte) {
    const bytes = new TextEncoder().encode(this.source.value);
    const decoder = new TextDecoder();
    const start = decoder.decode(bytes.subarray(0, startByte)).length;
    const end = decoder.decode(bytes.subarray(0, endByte)).length;
    this.source.focus();
    this.source.setSelectionRange(start, end);
    // Put the selection roughly in view: a textarea will not scroll to a
    // selection on its own.
    const before = this.source.value.slice(0, start).split("\n").length - 1;
    const lineHeight =
      parseFloat(getComputedStyle(this.source).lineHeight) || 18;
    this.source.scrollTop = Math.max(0, (before - 3) * lineHeight);
  }

  showTree(root, ms, bytes) {
    const parts = [];
    const seen = walk(
      this.pack,
      root,
      (n) => {
        const error = n.flags & 2 || n.flags & 8;
        parts.push(
          `<div class="pg-node${error ? " bad" : ""}" style="--d:${n.depth}">` +
            (n.field ? `<span class="pg-field">${E(n.field)}:</span>` : "") +
            `<a class="pg-type" href="/grammars/${E(this.name)}/#r-${E(n.type)}"` +
            ` title="Read the production for ${E(n.type)}">${E(n.type)}</a>` +
            `<span class="pg-span">${n.startByte}–${n.endByte}</span></div>`,
        );
      },
      { namedOnly: true, budget: MAX_NODES },
    );

    const capped =
      seen >= MAX_NODES
        ? `<p class="dim">Stopped at ${MAX_NODES.toLocaleString()} nodes.</p>`
        : "";
    this.treeBox.innerHTML =
      `<p class="dim pg-stat">${seen.toLocaleString()} named node${seen === 1 ? "" : "s"}
from ${bytes.toLocaleString()} bytes in ${ms.toFixed(1)} ms</p>` +
      parts.join("") +
      capped;
  }
}

for (const root of document.querySelectorAll(".playground"))
  new Playground(root);
