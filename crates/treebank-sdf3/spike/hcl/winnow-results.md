# winnow results for hcl

17 of 17 corpus expectations hold under the winnow lowering.

## PASS: literals, and the three keywords that are literals only where a value may go

## PASS: the operator ladder, with HCL's own precedence

## PASS: the conditional, right associative, and not an ExprTerm

## PASS: tuples and objects, with the separators each admits

## PASS: for expressions over both collection kinds, with a condition and grouping

## PASS: function calls, including a provider-namespaced one and an expanded argument

## PASS: the access operators: attribute, index, legacy index, and both splats

## PASS: a resource block, its labels and a nested block

## PASS: a one-line block holds at most one attribute, and a multi-line body may be empty

## PASS: naked identifier labels, and a dash in a name

## PASS: comments in all three spellings, and blank lines between items

## PASS: a .tfvars body: attributes only, no blocks

## PASS: a quoted template: literal runs, interpolations, escapes and the escaped introductions

## PASS: a heredoc, in both markers, and its terminator rules

## PASS: template directives, nested, with an else branch and both strip markers

## PASS: a heredoc carries the same template sub-language, and either may nest in the other

## PASS: block labels are string literals, not templates: no interpolation reaches them

