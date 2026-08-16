// The closed treebank vocabulary, importable by every treebank grammar.js.
//
// This module is the single source of truth's JS face: the same
// vocabulary.json is embedded in the treebank-core Rust crate, so the
// grammars, the `treebank roles` checker and the facet query expansion can
// never disagree about what the vocabulary is.
//
// A grammar may omit terms its language lacks; it may not invent terms.
// Call `assertTableTerms` on the supertypes list at generate time so a
// misspelled or invented term fails the generate, not a later CI run.

'use strict';

const vocabulary = require('./vocabulary.json');

const TABLE_TIER = vocabulary.table.map((t) => t.name);
const FACET_TIER = vocabulary.facets.map((t) => t.name);

/**
 * Assert every name is a table-tier vocabulary term, and return the list
 * unchanged so it can wrap a grammar's `supertypes:` value in place:
 *
 *   supertypes: $ => assertTableTerms([
 *     '_statement', '_expression', ...
 *   ]).map(name => $[name]),
 *
 * @param {string[]} names
 * @returns {string[]}
 */
function assertTableTerms(names) {
  for (const name of names) {
    if (!TABLE_TIER.includes(name)) {
      throw new Error(
        `"${name}" is not a treebank vocabulary term (vocabulary ${vocabulary.version}). ` +
        `Table tier: ${TABLE_TIER.join(', ')}`,
      );
    }
  }
  return names;
}

module.exports = {
  vocabulary,
  VERSION: vocabulary.version,
  TABLE_TIER,
  FACET_TIER,
  assertTableTerms,
};
