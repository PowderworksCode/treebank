// The inventory, rendered.
//
// A pass rate on its own is the number a project quotes when it wants to be
// trusted. These panels put it next to the things that decide whether it means
// anything: how many of the failures are the grammar's fault rather than the
// corpus's, whether the evidence behind it is still bound to the grammar it
// was measured against, whether lint is enforced or merely advisory, and what
// the grammar is known to get wrong.
//
// Everything here comes from `treebank status --format json`, snapshotted by
// tools/build-status.mjs and regenerated in CI, so the page cannot quote a
// number the repository has stopped agreeing with.

export const escapeHtml = (s) =>
  String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;").replace(/'/g, "&#x27;");

const E = escapeHtml;
const n = (v) => (typeof v === "number" ? v.toLocaleString("en-US") : "—");

// A pass rate is read as a grade, so it gets a band -- but the bands are wide
// and unlabelled by colour alone, because "24.70%" on C++ is a fact about how
// much of the language macros make unparseable, not a failing report card.
function band(rate) {
  const v = parseFloat(rate);
  if (!Number.isFinite(v)) return "";
  return v >= 99 ? "good" : v >= 90 ? "fair" : "thin";
}

const flag = (on, yes, no) =>
  `<span class="flag ${on ? "on" : "off"}">${on ? E(yes) : E(no)}</span>`;

function corporaTable(corpora) {
  if (!corpora.length) return "<p class=\"dim\">No corpus evidence.</p>";
  const rows = corpora.map((c) => `<tr>
<td class="mono">${E(c.language)}</td>
<td class="num">${n(c.files)}</td>
<td class="num">${n(c.passed)}</td>
<td class="num rate ${band(c.pass_rate)}">${E(c.pass_rate ?? "—")}</td>
<td class="num">${n(c.grammar_gaps)}</td>
<td class="num dim">${n(c.noise)}</td>
<td>${flag(c.freshness === "current", "current", c.freshness ?? "unknown")}</td>
</tr>`).join("");
  return `<div class="scroll"><table class="status-table">
<thead><tr><th>corpus</th><th class="num">files</th><th class="num">passed</th>
<th class="num">pass rate</th><th class="num">gaps</th><th class="num">noise</th>
<th>evidence</th></tr></thead><tbody>${rows}</tbody></table></div>
<p class="dim note"><b>gaps</b> are failures this grammar is responsible for.
<b>noise</b> is code the reference parser rejects too, so it is not a defect.
<b>evidence</b> is <i>current</i> when the committed measurement still matches
the grammar revision it was taken against.</p>`;
}

function listOf(title, items, blurb) {
  if (!items?.length) return "";
  const rows = items.map((g) =>
    `<li>${E(g.summary ?? "")}${
      g.files ? ` <span class="dim">(${n(g.files)} file${g.files === 1 ? "" : "s"})</span>` : ""
    }</li>`).join("");
  return `<h3 class="vh">${E(title)} <span class="count">${items.length}</span></h3>
<p class="dim note">${blurb}</p><ul class="ledger">${rows}</ul>`;
}

export function grammarStatus(g, name) {
  if (!g) return `<p class="broken">No inventory entry for ${E(name)}.</p>`;

  const facts = [
    ["evidence", flag(g.evidence_freshness === "current", "current", g.evidence_freshness ?? "—")],
    ["corpus lock", flag(g.corpus_lock, "pinned", "none")],
    ["canary", flag(g.corpus_canary, "yes", "no")],
    ["lint", g.lint_ratchet
      ? '<span class="flag on">ratcheted</span>'
      : '<span class="flag warn">advisory</span>'],
    ["external scanner", flag(g.external_scanner, "yes", "no")],
    ["wasm pack", flag(g.distribution?.wasm_pack, "built", "no")],
  ];

  const caps = [
    ["span oracle", g.capabilities?.spans],
    ["formatter", g.capabilities?.formatter],
    ["printer", g.capabilities?.printer],
  ];

  const counts = [
    ["corpus cases", g.tests?.corpus_cases],
    ["negative files", g.tests?.negative_files],
    ["shape fixtures", g.tests?.shape_files],
    ["supertypes", g.roles?.supertypes],
    ["facets", g.roles?.facets],
    ["uncategorised", g.roles?.uncategorised],
  ];

  // What the grammar declares about itself. Each of these is a policy file
  // in the crate, and each exists so a difference is written down rather than
  // rounded away -- a grammar that admits older syntax, or accepts more than
  // the language does, or builds a tree that knowingly differs from the
  // reference parser's, says so in one of these rather than in nobody's head.
  const declared = [
    ["shape_policy.toml", g.known_deviations?.shape,
      "tree shapes that knowingly differ from the reference parser's"],
    ["fuzz_policy.toml", g.known_deviations?.fuzz,
      "derivations the fuzzer may produce that the language does not accept"],
    ["version_policy.toml", g.known_deviations?.version,
      "older syntax rejected on purpose, where admitting it would change how current code parses"],
    ["lint_policy.toml", g.lint_ratchet,
      "structural ratchets the grammar lint is held to, rather than advising"],
  ];

  return `<h2 id="status">Status</h2>
<p>What this grammar is measured against, and where it is known to be wrong.
Generated by <code>treebank status</code> and regenerated in CI.</p>

${corporaTable(g.corpora ?? [])}

<div class="facts">${
    facts.map(([k, v]) => `<div><span class="k">${E(k)}</span>${v}</div>`).join("")
  }</div>

<div class="facts counts">${
    counts.map(([k, v]) => `<div><span class="k">${E(k)}</span><b>${n(v)}</b></div>`).join("")
  }</div>

<p class="dim note">Reference-parser capabilities: ${
    caps.map(([k, v]) => `${E(k)} ${flag(v, "yes", "no")}`).join(" · ")
  }. A grammar with no span oracle cannot run <code>treebank shape</code>, and
that absence is recorded rather than worked around.</p>

<h3 class="vh">Declarations</h3>
<p class="dim note">Where this grammar knowingly differs from the reference
parser, or from the language, it is declared in one of these. An undeclared
deviation fails the build.</p>
<dl class="declared">${
    declared.map(([file, on, what]) =>
      `<dt class="${on ? "on" : "off"}"><code>${E(file)}</code>${
        on ? '<span class="flag on">declared</span>' : '<span class="flag off">none</span>'
      }</dt><dd>${E(what)}</dd>`).join("")
  }</dl>

${listOf("Known gaps", g.known_gaps,
    "Code the language accepts and this grammar does not.")}
${listOf("Known widenings", g.known_widenings,
    "Code this grammar accepts and the language does not.")}
${listOf("Known deviations", g.deviations,
    "Where the tree differs from the reference parser's, on purpose.")}`;
}

// The index page: every grammar at once, which is the view that answers
// "what shape is the repository in" without opening nine pages.
export function statusOverview(status) {
  const names = Object.keys(status.grammars).sort();
  const rows = names.map((name) => {
    const g = status.grammars[name];
    const c = g.corpora?.[0];
    const gaps = (g.corpora ?? []).reduce((t, x) => t + (x.grammar_gaps ?? 0), 0);
    const files = (g.corpora ?? []).reduce((t, x) => t + (x.files ?? 0), 0);
    const fresh = (g.corpora ?? []).every((x) => x.freshness === "current");
    return `<tr>
<td><a href="/grammars/${E(name)}/">${E(name)}</a></td>
<td class="num">${n(files)}</td>
<td class="num rate ${band(c?.pass_rate)}">${E(c?.pass_rate ?? "—")}</td>
<td class="num">${n(gaps)}</td>
<td class="num">${n(g.tests?.negative_files)}</td>
<td class="num">${n(g.tests?.shape_files)}</td>
<td>${flag(fresh, "current", "stale")}</td>
<td>${g.lint_ratchet
      ? '<span class="flag on">ratcheted</span>'
      : '<span class="flag warn">advisory</span>'}</td>
</tr>`;
  }).join("");

  const warn = (status.warnings ?? []).length
    ? `<h3 class="vh">Open warnings <span class="count">${status.warnings.length}</span></h3>
<p class="dim note">Optional coverage that is not configured yet. These do not
fail the build.</p>
<ul class="ledger">${status.warnings.map((w) => `<li>${E(w)}</li>`).join("")}</ul>`
    : "";

  return `<div class="scroll"><table class="status-table">
<thead><tr><th>grammar</th><th class="num">corpus files</th><th class="num">pass rate</th>
<th class="num">gaps</th><th class="num">negative</th><th class="num">shape</th>
<th>evidence</th><th>lint</th></tr></thead><tbody>${rows}</tbody></table></div>
<p class="dim note">Pass rate is the first corpus where a grammar has more than
one. <b>gaps</b> counts only failures the grammar is responsible for.</p>
${warn}`;
}
