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

import { errorsIn, Pack, walk } from "./pack.mjs";

const E = (s) =>
  String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;").replace(/'/g, "&#x27;");

// Where packs are served from. One place, because the answer is a deployment
// decision rather than a property of this file: same-origin today, and the
// only thing that changes if they move is this line.
const PACK_URL = (name) => `/packs/treebank-${name}.wasm`;

// tb_parse runs on the main thread. A megabyte of source is a fraction of a
// second and fine; ten is a frozen tab. The cap is honest about what this
// page is for -- trying constructs, not parsing a repository.
const MAX_BYTES = 400_000;
// A tree view is one DOM node per syntax node. Past a few thousand the cost
// is the browser's layout, not the parse, so the walk stops and says so.
const MAX_NODES = 4_000;

const SAMPLES = {
  bash: 'for f in *.txt; do\n  echo "${f@Q}" >&2\ndone\n',
  c: "int main(void) {\n    int xs[] = {1, 2, 3};\n    return xs[0];\n}\n",
  cpp: "template <typename T>\nauto sum(const std::vector<T>& xs) -> T {\n    return std::accumulate(xs.begin(), xs.end(), T{});\n}\n",
  java: "record Point(int x, int y) {\n    Point {\n        if (x < 0) throw new IllegalArgumentException();\n    }\n}\n",
  python: "def greet(name: str = 'world') -> str:\n    match name.split():\n        case [first, *rest]:\n            return f'hello {first}'\n        case _:\n            return 'hello'\n",
  ruby: "class Greeter\n  def initialize(name) = @name = name\n  def call = \"hello #{@name}\"\nend\n",
  rust: "fn largest<T: PartialOrd>(xs: &[T]) -> Option<&T> {\n    xs.iter().reduce(|a, b| if a > b { a } else { b })\n}\n",
  typescript: "type Result<T> = { ok: true; value: T } | { ok: false; error: string };\n\nconst unwrap = <T,>(r: Result<T>): T => {\n  if (!r.ok) throw new Error(r.error);\n  return r.value;\n};\n",
  zig: "const std = @import(\"std\");\n\npub fn main() !void {\n    const xs = [_]u8{ 1, 2, 3 };\n    std.debug.print(\"{d}\\n\", .{xs.len});\n}\n",
};

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
<div class="pg-prov dim"></div>`;

    this.select = this.root.querySelector(".pg-grammar");
    this.source = this.root.querySelector(".pg-src");
    this.treeBox = this.root.querySelector(".pg-tree");
    this.errorBox = this.root.querySelector(".pg-errors");
    this.stateBox = this.root.querySelector(".pg-state");
    this.provBox = this.root.querySelector(".pg-prov");

    this.select.addEventListener("change", () => this.choose(this.select.value));
    this.source.addEventListener("input", () => this.schedule());
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
        .map((g) => `<option value="${E(g.name)}">${E(g.name)}</option>`).join("");
      const wanted = new URLSearchParams(location.search).get("g");
      const start = grammars.some((g) => g.name === wanted) ? wanted : "python";
      this.select.value = start;
      await this.choose(start);
    } catch (error) {
      this.stateBox.textContent = `could not list grammars: ${error.message}`;
    }
  }

  async choose(name) {
    if (this.name === name) return;
    this.name = name;
    this.pack = null;
    this.treeBox.innerHTML = '<p class="dim">Loading the parser…</p>';
    this.errorBox.innerHTML = "";
    this.provBox.textContent = "";
    this.stateBox.textContent = `loading treebank-${name}.wasm…`;

    const started = performance.now();
    try {
      this.pack = await Pack.load(PACK_URL(name));
    } catch (error) {
      this.stateBox.textContent = "";
      this.treeBox.innerHTML =
        `<p class="broken">Could not load the ${E(name)} parser: ${E(error.message)}</p>
<p class="dim">The packs are built artifacts. If this is a local checkout, run
<code>./tools/wasm-pack/build.sh ${E(name)} --out site/public/packs</code>.</p>`;
      return;
    }
    const ms = Math.round(performance.now() - started);
    this.stateBox.textContent = `treebank-${name} loaded in ${ms} ms`;
    this.showProvenance();
    if (!this.source.value.trim() || this.sampleShown) {
      this.source.value = SAMPLES[name] ?? "";
      this.sampleShown = true;
    }
    this.parse();
  }

  showProvenance() {
    const p = this.pack.provenance;
    const facets = Object.keys(this.pack.roles.facets ?? {});
    const bits = [
      `grammar <b>${E(p.grammar_name ?? p.language)}</b>`,
      `vocabulary ${E(p.vocabulary ?? "?")}`,
      `tree-sitter ${E(p.generate_cli ?? "?")}`,
      facets.length ? `facets ${facets.map(E).join(" ")}` : null,
    ].filter(Boolean);
    // Read out of the module's own bytes, not from a caption beside it.
    this.provBox.innerHTML = `Read from the pack itself: ${bits.join(" · ")}. ` +
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
    const rows = errors.slice(0, 25).map((e) =>
      `<li><span class="pg-badge ${e.kind === "MISSING" ? "missing" : "err"}">${e.kind}</span>
<span class="mono">${e.row + 1}:${e.column + 1}</span>
${e.type ? `<span class="dim mono">${E(e.type)}</span>` : ""}</li>`).join("");
    this.errorBox.innerHTML = `<p class="pg-bad">${errors.length} error node${
      errors.length === 1 ? "" : "s"
    }.</p><ul class="pg-errlist">${rows}</ul>${
      errors.length > 25 ? '<p class="dim">…first 25 shown.</p>' : ""
    }`;
  }

  showTree(root, ms, bytes) {
    const parts = [];
    const seen = walk(this.pack, root, (n) => {
      const error = n.flags & 2 || n.flags & 8;
      parts.push(
        `<div class="pg-node${error ? " bad" : ""}" style="--d:${n.depth}">` +
          (n.field ? `<span class="pg-field">${E(n.field)}:</span>` : "") +
          `<a class="pg-type" href="/grammars/${E(this.name)}/#r-${E(n.type)}"` +
          ` title="Read the production for ${E(n.type)}">${E(n.type)}</a>` +
          `<span class="pg-span">${n.startByte}–${n.endByte}</span></div>`,
      );
    }, { namedOnly: true, budget: MAX_NODES });

    const capped = seen >= MAX_NODES
      ? `<p class="dim">Stopped at ${MAX_NODES.toLocaleString()} nodes.</p>` : "";
    this.treeBox.innerHTML =
      `<p class="dim pg-stat">${seen.toLocaleString()} named node${seen === 1 ? "" : "s"}
from ${bytes.toLocaleString()} bytes in ${ms.toFixed(1)} ms</p>` +
      parts.join("") + capped;
  }
}

for (const root of document.querySelectorAll(".playground")) new Playground(root);
