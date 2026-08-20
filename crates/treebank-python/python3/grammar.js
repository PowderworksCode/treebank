/**
 * The `python3` variant: Python 3.0 – 3.13.
 *
 * Everything is in `../common/define-grammar.js`; this file is the variant
 * manifest, and it is meant to stay readable as one — what differs from
 * the shared grammar is a list here, not a conditional there
 * (VARIANTS.md §3). Every extension point is spelled out even when empty,
 * so the two variant files diff against each other.
 *
 * The grammar NAME stays `python`, not `python3`, and that is deliberate:
 * it is the C symbol (`tree_sitter_python`) consumers already link and the
 * scope editors already match. Python 3 is what `treebank_python::LANGUAGE`
 * has always meant, so the default keeps its name and the new variant takes
 * a qualified one.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const lexicon = require('../common/lexicon.js');

module.exports = require('../common/define-grammar.js')({
  name: 'python',

  statements: ['nonlocal_statement', 'type_alias_statement'],
  literals: ['ellipsis'],
  // `...` is an ordinary literal here, so it reaches a subscript through
  // the expression tier and needs no separate mention.
  subscriptMembers: [],
  patternMembers: ['star_pattern'],
  branches: ['match_statement'],
  orTestMembers: ['named_expression'],
  primaryExpressions: ['await_expression'],
  comparisonOperators: [],
  plainParameters: [],
  exceptAliases: ['as'],
  raiseTails: ['from'],
  softKeywords: ['match', 'case', 'type'],
  integers: lexicon.PY3_INTEGERS,
  floats: lexicon.PY3_FLOATS,
  identifier: lexicon.PY3_IDENTIFIER,
  ruleGroups: require('../common/py3-rules.js'),
  features: {
    async: true,
    annotations: true,
    yieldFrom: true,
    exceptStar: true,
    parenthesizedWithItems: true,
    commaIterable: false,
  },

  // The match sub-grammar is where python 3's ambiguity lives: a pattern
  // looks like an expression until the `case` line ends, so every closed
  // pattern forks against the expression tier. They are declared here
  // rather than in the shared list because a variant without `match` must
  // not carry them — a conflict is a fork, and a fork nothing can win is
  // still a fork the table pays for.
  conflicts: ($) => [
    [$._patterns_comma, $._closed_pattern],
    [$._match_shape, $._access],
    [$._match_shape, $._primary_expression],
    [$.case_dict_splat, $._primary_expression],
    [$.case_star_pattern, $._primary_expression],
    [$.dictionary_pattern_pair, $._access],
    [$.case_dict_pattern, $.dictionary],
    [$.case_signed_number, $._literal],
    [$.case_list_pattern, $.list],
    [$.case_group_pattern, $._case_sequence],
    [$.case_tuple_pattern, $.tuple],
    [$._literal_pattern, $._primary_expression],
    [$._closed_pattern, $._access],
    [$.class_pattern, $._access],
    [$._closed_pattern, $._primary_expression],
    [$.class_pattern, $._primary_expression],
    [$.case_complex_number, $._literal],
    [$.match_statement, $._soft_keyword],
    [$.type_alias_statement, $._soft_keyword],
    [$.type_alias_statement, $.conditional_expression],
    [$._case_patterns, $.conditional_expression],
    // In `def f(x: int)` the colon is an annotation; in `lambda x: y` it is
    // the body. Same parameter rule, GLR decides per context — and a
    // variant without annotations never reaches the ambiguity.
    [$.parameter],
    [$.star_parameter],
    [$.double_star_parameter],
  ],
});
