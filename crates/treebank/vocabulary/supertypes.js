// The closed treebank vocabulary, importable by every treebank grammar.js.
//
// This module is the single source of truth's JS face: the same
// vocabulary.json is embedded in the treebank Rust crate, so the
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
const EITHER_TIER = vocabulary.either_tier || [];

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

/**
 * Assert a table-tier term this grammar delivers as a facet instead is one
 * the vocabulary allows to vary by grammar. Call it beside `supertypes:` so
 * an omission that is really a typo fails the generate:
 *
 *   supertypes: $ => assertTableTerms([...]).map(name => $[name]),
 *   // _parameter is demoted; see roles.json
 *   ...assertDemotable(['_parameter'])
 *
 * The reason for the demotion, and the check that the term is a facet key
 * and not also a supertype, live in the grammar's roles.json and are
 * enforced by `treebank roles`.
 *
 * @param {string[]} names
 * @returns {string[]}
 */
function assertDemotable(names) {
  for (const name of names) {
    if (!EITHER_TIER.includes(name)) {
      throw new Error(
        `"${name}" may not be demoted to the facet tier (vocabulary ${vocabulary.version}). ` +
        `Demotable: ${EITHER_TIER.join(', ') || '(none)'}`,
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
  EITHER_TIER,
  assertTableTerms,
  assertDemotable,
};
