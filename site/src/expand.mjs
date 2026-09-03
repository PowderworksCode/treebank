// Nominal query expansion, in the browser.
//
// A nominal term is not a grammar rule, so `(_callable name: (_name) @n)`
// cannot run against the parser as written. This rewrites every nominal pattern
// into the
// alternation the pack's own manifest defines:
//
//   (_callable)               ->  [(function_definition) (lambda)]
//   (_callable name: (x) @n)  ->  [(function_definition name: (x) @n)
//                                  (lambda name: (x) @n)]
//
// Supertypes are left alone: they are real rules in the parse table and
// tree-sitter matches them natively. String literals and `;` comments are
// copied verbatim, never rewritten inside.
//
// This is a PORT of crates/treebank/src/expand.rs, and the two must not
// drift: a query that means one thing here and another in a consumer's build
// is worse than a query that fails. crates/treebank/tests/expand_parity.rs
// runs both over every grammar's nominal terms and fails on any difference, so this
// file is checked against its original rather than trusted to match it.
//
// It mirrors `Pack::expand_query`, filtering included. A member is dropped
// from the alternation when a depth-0 field constraint cannot hold for it --
// `(_callable name: (_) @n)` must not keep `lambda`, which has no `name`,
// because tree-sitter rejects the whole alternation if any one branch is an
// impossible pattern. node-types.json ships inside every pack, so the browser
// has the same evidence the crate does.

// Rust's `char::is_alphanumeric() || c == '_'`, which is Unicode-aware rather
// than ASCII -- so the two agree about where a name ends even for input no
// grammar would ever produce.
const NAME = /[\p{Alphabetic}\p{Nd}\p{Nl}\p{No}_]*/uy;

// Index just past the closing quote of the string starting at `start`.
function skipString(s, start) {
  let i = start + 1;
  while (i < s.length) {
    if (s[i] === "\\") i += 2;
    else if (s[i] === '"') return i + 1;
    else i += 1;
  }
  throw new Error("unterminated string literal in query");
}

function skipStringOrEnd(s, start) {
  try {
    return skipString(s, start);
  } catch {
    return s.length;
  }
}

// Index of the `)` matching the `(` at `open`.
function matchingParen(s, open) {
  let depth = 0;
  let i = open;
  while (i < s.length) {
    const c = s[i];
    if (c === '"') i = skipString(s, i) - 1;
    else if (c === "(") depth += 1;
    else if (c === ")") {
      depth -= 1;
      if (depth === 0) return i;
    } else if (c === ";") {
      const nl = s.indexOf("\n", i);
      i = nl === -1 ? s.length : nl;
    }
    i += 1;
  }
  throw new Error("unbalanced parentheses in query");
}

// node-types.json, in the shape the filtering needs. Same projection as
// crates/treebank/src/node_types.rs: named types, supertype -> direct named
// subtypes, and type -> field -> the types that field may hold.
export function parseNodeTypes(json) {
  const raw = typeof json === "string" ? JSON.parse(json) : json;
  const named = new Set();
  const supertypes = new Map();
  const fields = new Map();
  for (const e of raw) {
    if (!e.named) continue;
    named.add(e.type);
    const perField = new Map();
    for (const [fname, fval] of Object.entries(e.fields ?? {})) {
      perField.set(fname, new Set((fval?.types ?? []).map((t) => t.type)));
    }
    fields.set(e.type, perField);
    if (e.subtypes?.length) {
      supertypes.set(
        e.type,
        e.subtypes.filter((x) => x.named).map((x) => x.type),
      );
    }
  }
  return { named, supertypes, fields };
}

// Every named type reachable through nested supertypes, the supertype itself
// included.
function closure(nt, supertype) {
  const out = new Set();
  const stack = [supertype];
  while (stack.length) {
    const name = stack.pop();
    if (out.has(name)) continue;
    out.add(name);
    for (const sub of nt.supertypes.get(name) ?? []) stack.push(sub);
  }
  return out;
}

// The field constraints at DEPTH 0 of a pattern body: the field name plus the
// node types its value pattern demands. An empty list means presence is
// enough. Only depth-0 fields bind to the nominal member itself; a `#`-predicate
// opens a paren and so is already depth 1.
function topLevelFieldConstraints(body) {
  const out = [];
  let depth = 0;
  let i = 0;
  const wordish = (c) => c !== undefined && /[0-9A-Za-z_]/.test(c);
  while (i < body.length) {
    const c = body[i];
    if (c === '"') {
      i = skipStringOrEnd(body, i);
    } else if (c === "(" || c === "[") {
      depth += 1;
      i += 1;
    } else if (c === ")" || c === "]") {
      depth -= 1;
      i += 1;
    } else if (depth === 0 && /[a-z_]/.test(c)) {
      const start = i;
      while (i < body.length && wordish(body[i])) i += 1;
      if (body[i] !== ":") continue;
      const field = body.slice(start, i);
      i += 1;
      while (i < body.length && /\s/.test(body[i])) i += 1;
      // The value pattern: `(name ...)`, or `[(a ...) (b ...)]` where a term
      // has already been expanded in place.
      const names = [];
      let j = i;
      if (body[j] === "[") j += 1;
      while (j < body.length) {
        while (j < body.length && /\s/.test(body[j])) j += 1;
        if (body[j] !== "(") break;
        j += 1;
        const ns = j;
        while (j < body.length && wordish(body[j])) j += 1;
        if (j > ns) names.push(body.slice(ns, j));
        // Skip to this pattern's close, so an alternation yields every member.
        let d = 1;
        while (j < body.length && d > 0) {
          const k = body[j];
          if (k === "(") d += 1;
          else if (k === ")") d -= 1;
          else if (k === '"') {
            j = skipStringOrEnd(body, j);
            continue;
          }
          j += 1;
        }
        if (body[i] !== "[") break;
      }
      // `_` is the WILDCARD, not a node type: it constrains presence only.
      // Treated as a type name it filtered every member out, since no field
      // declares one.
      out.push([field, names.includes("_") ? [] : names]);
      // `i` is left at the value, which the loop then walks as usual.
    } else {
      i += 1;
    }
  }
  return out;
}

// Rewrite `query` against `nominal`, a plain object of name -> member list as
// it appears in the pack's terms manifest. Member order is the manifest's and
// is not sorted here: the expansion is compared byte for byte against the
// Rust, which preserves it too.
export function expandQuery(query, nominal, nodeTypes = null) {
  let out = "";
  let i = 0;
  while (i < query.length) {
    const c = query[i];
    if (c === '"') {
      const end = skipString(query, i);
      out += query.slice(i, end);
      i = end;
    } else if (c === ";") {
      const nl = query.indexOf("\n", i);
      const end = nl === -1 ? query.length : nl;
      out += query.slice(i, end);
      i = end;
    } else if (c === "(") {
      const nameStart = i + 1;
      NAME.lastIndex = nameStart;
      const name = (NAME.exec(query) ?? [""])[0];
      const nameEnd = nameStart + name.length;
      const members = Object.hasOwn(nominal, name) ? nominal[name] : undefined;
      if (members) {
        if (members.length === 0)
          throw new Error(`nominal term \`${name}\` has no members`);
        const close = matchingParen(query, i);
        // The body is everything between the name and the close paren, itself
        // expanded first so a term nested inside one resolves inside out.
        const body = expandQuery(
          query.slice(nameEnd, close),
          nominal,
          nodeTypes,
        );

        // Drop members a depth-0 field constraint cannot hold for. This is
        // what tree-sitter itself checks for a NATIVE supertype pattern,
        // where the pattern survives if any one subtype satisfies it. A
        // member absent from node-types is kept; an expansion that filters
        // every member is an error rather than an empty alternation.
        const needed = topLevelFieldConstraints(body);
        const kept = members.filter((m) => {
          if (!nodeTypes) return true;
          const declared = nodeTypes.fields.get(m);
          if (!declared) return true;
          return needed.every(([f, want]) => {
            const have = declared.get(f);
            if (!have) return false;
            if (want.length === 0) return true;
            // Derivation-based: the field must DECLARE the type asked for, or
            // declare a supertype that derives to it. The closure runs from
            // what is declared toward what is asked, never the other way.
            return want.some(
              (w) =>
                have.has(w) ||
                [...have].some((h) => closure(nodeTypes, h).has(w)),
            );
          });
        });
        if (kept.length === 0) {
          throw new Error(
            `nominal term \`${name}\`: no member satisfies the field constraint(s) ` +
              needed.map(([f, w]) => `${f}: [${w.join(", ")}]`).join(", "),
          );
        }

        out += "[";
        kept.forEach((m, k) => {
          if (k > 0) out += " ";
          out += "(" + m + body + ")";
        });
        out += "]";
        i = close + 1;
      } else {
        out += "(";
        i += 1;
      }
    } else {
      out += c;
      i += 1;
    }
  }
  return out;
}

// The nominal term names a query may use, for a hint under the box.
export function nominalNames(terms) {
  return Object.keys(terms?.nominal ?? {}).sort();
}
