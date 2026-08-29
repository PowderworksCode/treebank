// Facet query expansion, in the browser.
//
// A facet is not a grammar rule, so `(_callable name: (_name) @n)` cannot run
// against the parser as written. This rewrites every facet pattern into the
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
// runs both over every grammar's facets and fails on any difference, so this
// file is checked against its original rather than trusted to match it.
//
// It mirrors `Pack::expand_query`, which passes no node-types, so the
// member-filtering branch of the Rust has no counterpart here. That is the
// behaviour a consumer of the crate gets, and the playground should not
// quietly be cleverer than the library it demonstrates.

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

// Rewrite `query` against `facets`, a plain object of name -> member list as
// it appears in the pack's roles manifest. Member order is the manifest's and
// is not sorted here: the expansion is compared byte for byte against the
// Rust, which preserves it too.
export function expandQuery(query, facets) {
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
      const members = Object.prototype.hasOwnProperty.call(facets, name)
        ? facets[name]
        : undefined;
      if (members) {
        if (members.length === 0) throw new Error(`facet \`${name}\` has no members`);
        const close = matchingParen(query, i);
        // The body is everything between the name and the close paren, itself
        // expanded first so a facet nested inside a facet resolves inside out.
        const body = expandQuery(query.slice(nameEnd, close), facets);
        out += "[";
        members.forEach((m, k) => {
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

// The facet names a query may use, for a hint under the box.
export function facetNames(roles) {
  return Object.keys(roles?.facets ?? {}).sort();
}
