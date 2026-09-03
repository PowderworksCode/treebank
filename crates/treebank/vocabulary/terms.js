// The closed treebank vocabulary, importable by every treebank grammar.js.
//
// This module is the single source of truth's JS face: the same
// vocabulary.json is embedded in the treebank Rust crate, so the
// grammars, the `treebank terms` checker and nominal query expansion can
// never disagree about what the vocabulary is.
//
// A grammar may omit terms its language lacks; it may not invent terms.
// Call `assertStructuralTerms` on the supertypes list at generate time so a
// misspelled or invented term fails the generate, not a later CI run.

'use strict';

const vocabulary = require('./vocabulary.json');

const STRUCTURAL_TERMS = vocabulary.structural.map((t) => t.name);
const NOMINAL_TERMS = vocabulary.nominal.map((t) => t.name);
const DEMOTABLE = vocabulary.demotable || [];

/**
 * Assert every name is a vocabulary term this grammar may deliver
 * structurally, and return the list unchanged so it can wrap a grammar's
 * `supertypes:` value in place:
 *
 *   supertypes: $ => assertStructuralTerms([
 *     '_statement', '_expression', ...
 *   ]).map(name => $[name]),
 *
 * @param {string[]} names
 * @returns {string[]}
 */
function assertStructuralTerms(names) {
  for (const name of names) {
    if (!STRUCTURAL_TERMS.includes(name)) {
      throw new Error(
        `"${name}" is not a treebank vocabulary term (vocabulary ${vocabulary.version}). ` +
        `Structural terms: ${STRUCTURAL_TERMS.join(', ')}`,
      );
    }
  }
  return names;
}

/**
 * Assert a term this grammar delivers nominally rather than structurally is
 * one the vocabulary allows to vary by grammar. Call it beside
 * `supertypes:` so an omission that is really a typo fails the generate:
 *
 *   supertypes: $ => assertStructuralTerms([...]).map(name => $[name]),
 *   // _parameter is demoted; see terms.json
 *   ...assertDemotable(['_parameter'])
 *
 * The reason for the demotion, and the check that the term is a nominal key
 * and not also a supertype, live in the grammar's terms.json and are
 * enforced by `treebank terms`.
 *
 * @param {string[]} names
 * @returns {string[]}
 */
function assertDemotable(names) {
  for (const name of names) {
    if (!DEMOTABLE.includes(name)) {
      throw new Error(
        `"${name}" may not be delivered nominally (vocabulary ${vocabulary.version}). ` +
        `Demotable: ${DEMOTABLE.join(', ') || '(none)'}`,
      );
    }
  }
  return names;
}

module.exports = {
  vocabulary,
  VERSION: vocabulary.version,
  STRUCTURAL_TERMS,
  NOMINAL_TERMS,
  DEMOTABLE,
  assertStructuralTerms,
  assertDemotable,
};
