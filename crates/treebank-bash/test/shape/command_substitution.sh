#!/usr/bin/env bash
# Command substitution in both spellings, and nested.

output=$(echo modern)
legacy=`echo backtick`
nested=$(echo "$(echo inner)")
in_string="value is $(echo interpolated)"
as_argument cmd "$(echo arg)"
multi=$(
  echo first
  echo second
)
