# winnow results for cppish

8 of 8 corpus expectations hold under the winnow lowering.

## PASS: CARRY: a < b > c; is a declaration, because {prefer} on the template reading wins the tie

## PASS: spacing is irrelevant to the same decision

## PASS: with no trailing name only the comparison completes

## PASS: where no declaration is possible the parser never forks: validity decides

## PASS: the template reading dies when the statement keeps going

## PASS: >> closes two templates: the lexer only offers tokens the state accepts

## PASS: cish's own statements, through the import

## PASS: a keyword from the imported module cannot be a name

