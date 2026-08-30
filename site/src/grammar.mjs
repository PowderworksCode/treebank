// Render a tree-sitter grammar the way a language manual renders one.
//
// Input is `src/grammar.json` -- the NORMALIZED grammar tree-sitter itself
// consumes, not grammar.js -- plus `src/node-types.json` for what is public
// and `roles.json` for treebank's vocabulary. grammar.json is already an
// EBNF syntax tree: SEQ, CHOICE, REPEAT, SYMBOL, STRING, PATTERN, PREC,
// FIELD, ALIAS. Rendering it is a fold, not a parse.
//
// The fold is total over grammar.json's node kinds and throws on one it does
// not know. That matters more than it looks: a missing case would silently
// omit part of a production from its documentation, and nothing downstream
// would notice a rule that renders three quarters of itself.

import * as rr from "./railroad.mjs";

export const PREC_KINDS = {
  PREC: "prec",
  PREC_LEFT: "prec.left",
  PREC_RIGHT: "prec.right",
  PREC_DYNAMIC: "prec.dynamic",
};

// Matches Python's `html.escape`, apostrophe included. Nothing here places
// text in a single-quoted attribute, so `&#x27;` is not load-bearing -- but
// the two renderers producing byte-identical EBNF is worth more than the one
// character saved, and it is what the differential test compares.
//
// railroad.mjs keeps its own, deliberately WITHOUT the apostrophe, because
// that is what the engine it was ported from does. The two differ on purpose.
export const escapeHtml = (s) =>
  String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#x27;");

export class Grammar {
  // `bundle` is what tools/build-grammars.mjs writes: the three source files
  // joined, so the viewer makes one request rather than three.
  constructor(bundle) {
    this.g = bundle.grammar;
    this.nt = bundle.nodeTypes;
    this.roles = bundle.roles ?? {};
    this.rules = this.g.rules;
    this.name = this.g.name;
    this.supertypes = new Set(this.g.supertypes ?? []);
    this.externals = new Set(
      (this.g.externals ?? [])
        .filter((e) => e.type === "SYMBOL")
        .map((e) => e.name),
    );
    this.word = this.g.word;
    // public node types, so we can say which rules a consumer ever sees
    this.visible = new Set(this.nt.filter((n) => n.named).map((n) => n.type));
  }

  hidden(name) {
    return name.startsWith("_");
  }
}

// Recognise seq(X, repeat(seq(sep, X))) -- the list idiom. Returns
// [item, sep] so it renders as one loop instead of a chain of five boxes.
// Without this roughly half the diagrams are unreadable.
export function commasep(node) {
  if (node.type !== "SEQ" || node.members.length !== 2) return null;
  const [first, second] = node.members;
  if (second.type !== "REPEAT") return null;
  const inner = second.content;
  if (inner?.type !== "SEQ" || inner.members.length !== 2) return null;
  const [sep, again] = inner.members;
  if (stable(again) !== stable(first)) return null;
  if (sep.type !== "STRING" && sep.type !== "SYMBOL") return null;
  return [first, sep];
}

// Key order in grammar.json is not guaranteed to match between two nodes
// that mean the same thing, so the comparison sorts keys before stringifying.
function stable(node) {
  return JSON.stringify(node, (_key, value) =>
    value && typeof value === "object" && !Array.isArray(value)
      ? Object.fromEntries(
          Object.keys(value)
            .sort()
            .map((k) => [k, value[k]]),
        )
      : value,
  );
}

export function toRr(node, g) {
  const t = node.type;
  switch (t) {
    case "STRING":
      return new rr.Leaf(node.value, "term");
    case "PATTERN": {
      const v = node.value;
      const short = v.length <= 32 ? v : v.slice(0, 29) + "...";
      return new rr.Leaf("/" + short + "/", "regex", { title: v });
    }
    case "SYMBOL": {
      const n = node.name;
      const cls = g.externals.has(n) ? "external" : "nonterm";
      return new rr.Leaf(n, cls, { href: "#r-" + n });
    }
    case "BLANK":
      return new rr.Skip();
    case "SEQ": {
      const cs = commasep(node);
      if (cs) return new rr.Repeat(toRr(cs[0], g), toRr(cs[1], g));
      return new rr.Seq(node.members.map((m) => toRr(m, g)));
    }
    case "CHOICE": {
      const members = node.members;
      const blanks = members.filter((m) => m.type === "BLANK").length;
      const rest = members.filter((m) => m.type !== "BLANK");
      if (blanks && rest.length === 1) return rr.Optional(toRr(rest[0], g));
      if (blanks)
        return rr.Optional(new rr.Choice(rest.map((m) => toRr(m, g))));
      return new rr.Choice(members.map((m) => toRr(m, g)));
    }
    case "REPEAT":
      return rr.Optional(new rr.Repeat(toRr(node.content, g)));
    case "REPEAT1":
      return new rr.Repeat(toRr(node.content, g));
    case "FIELD":
      return new rr.Labelled(toRr(node.content, g), node.name + ":", "field");
    case "ALIAS":
      return new rr.Labelled(
        toRr(node.content, g),
        "as " + node.value,
        "alias",
      );
    case "TOKEN":
    case "IMMEDIATE_TOKEN":
      return new rr.Labelled(
        toRr(node.content, g),
        t === "TOKEN" ? "token" : "token.immediate",
        "token",
      );
    case "RESERVED":
      return toRr(node.content, g);
    default:
      if (t in PREC_KINDS) {
        return new rr.Labelled(
          toRr(node.content, g),
          `${PREC_KINDS[t]} ${node.value ?? 0}`,
          "prec",
        );
      }
      throw new Error(`unhandled grammar node ${t}`);
  }
}

// Precedence for parenthesising: choice binds loosest, then seq, then the
// postfix repetition operators.
export const P_CHOICE = 0,
  P_SEQ = 1,
  P_POSTFIX = 2;

export function toEbnf(node, g, ctx = P_CHOICE) {
  const paren = (s, mine) => (mine < ctx ? `(${s})` : s);
  const t = node.type;
  switch (t) {
    case "STRING":
      return "<b>" + escapeHtml(node.value) + "</b>";
    case "PATTERN":
      return "<i>/" + escapeHtml(node.value) + "/</i>";
    case "SYMBOL": {
      const n = node.name;
      const cls = g.externals.has(n) ? "ext" : g.hidden(n) ? "hid" : "sym";
      return `<a class="${cls}" href="#r-${escapeHtml(n)}">${escapeHtml(n)}</a>`;
    }
    case "BLANK":
      return '<span class="eps">&#949;</span>';
    case "SEQ":
      return paren(
        node.members.map((m) => toEbnf(m, g, P_SEQ)).join(" "),
        P_SEQ,
      );
    case "CHOICE": {
      const members = node.members;
      const rest = members.filter((m) => m.type !== "BLANK");
      if (rest.length < members.length) {
        if (rest.length === 1) return toEbnf(rest[0], g, P_POSTFIX) + "?";
        return `(${rest.map((m) => toEbnf(m, g, P_CHOICE)).join(" | ")})?`;
      }
      return paren(
        members.map((m) => toEbnf(m, g, P_CHOICE)).join(" | "),
        P_CHOICE,
      );
    }
    case "REPEAT":
      return toEbnf(node.content, g, P_POSTFIX) + "*";
    case "REPEAT1":
      return toEbnf(node.content, g, P_POSTFIX) + "+";
    case "FIELD":
      return (
        `<span class="fld">${escapeHtml(node.name)}:</span>` +
        toEbnf(node.content, g, P_POSTFIX)
      );
    case "ALIAS":
      return (
        toEbnf(node.content, g, P_POSTFIX) +
        `<span class="al"> as ${escapeHtml(node.value)}</span>`
      );
    case "TOKEN":
    case "IMMEDIATE_TOKEN":
    case "RESERVED":
      return toEbnf(node.content, g, ctx);
    default:
      if (t in PREC_KINDS) return toEbnf(node.content, g, ctx);
      throw new Error(`unhandled grammar node ${t}`);
  }
}

// Every prec in the grammar, by numeric level. This is the half of the
// grammar EBNF cannot show and the manuals always print separately.
export function precedences(g) {
  const found = new Map();

  function walk(node, rule) {
    if (!node || typeof node !== "object") return;
    const t = node.type;
    if (t in PREC_KINDS) {
      const v = node.value ?? 0;
      if (Number.isInteger(v)) {
        if (!found.has(v)) found.set(v, []);
        found.get(v).push([PREC_KINDS[t], rule]);
      }
    }
    for (const key of ["members", "content"]) {
      const value = node[key];
      if (Array.isArray(value)) for (const x of value) walk(x, rule);
      else if (value && typeof value === "object") walk(value, rule);
    }
  }

  for (const [name, body] of Object.entries(g.rules)) walk(body, name);
  return found;
}

// Which rules answer which vocabulary term. Supertypes come from
// node-types.json -- they are real rules in the parse table. Facets come
// from roles.json and are expanded into an alternation at query load.
export function roleIndex(g) {
  const table = {};
  for (const n of g.nt) {
    if (n.subtypes?.length) {
      table[n.type] = n.subtypes.map((s) => s.type).sort();
    }
  }
  const facet = {};
  for (const [k, v] of Object.entries(g.roles.facets ?? {})) {
    facet[k] = [...v].sort();
  }
  return { table, facet };
}

// Section order: rules grouped under the supertype they answer, then the
// rest. A rule appears once, under the first role that claims it.
export function groupsOf(g, table) {
  const groups = [];
  const seen = new Set();
  for (const role of Object.keys(table).sort()) {
    const members = table[role].filter((m) => m in g.rules && !seen.has(m));
    if (members.length) {
      groups.push([role, members]);
      members.forEach((m) => {
        seen.add(m);
      });
    }
  }
  const names = Object.keys(g.rules);
  const pub = names.filter((n) => !seen.has(n) && !g.hidden(n)).sort();
  const hid = names.filter((n) => !seen.has(n) && g.hidden(n)).sort();
  if (pub.length) groups.push(["unclassified named nodes", pub]);
  if (hid.length) groups.push(["hidden rules", hid]);
  return groups;
}
