#!/usr/bin/env bash
# Command substitution in both spellings, and nested.
#
# Not a program: this file is never executed, it is parsed. The assignments
# exist to put a substitution in assignment position, so "assigned but never
# used" is the fixture working as intended rather than a defect.
# shellcheck disable=SC2034

output=$(echo modern)
legacy=`echo backtick`
nested=$(echo "$(echo inner)")
in_string="value is $(echo interpolated)"
as_argument cmd "$(echo arg)"
multi=$(
  echo first
  echo second
)
