#!/usr/bin/env bash
# Parameter expansion: one `$` with a dozen different groupings behind it.
#
# Not a program: this file is never executed, it is parsed. The names are
# deliberately unassigned because what is being pinned is the SHAPE of each
# expansion, and assigning them first would add syntax without adding a
# grouping. SC2320 likewise: `$?` here is a token to parse, not a status to
# read.
# shellcheck disable=SC2154,SC2034,SC2320

echo "$plain"
echo "${braced}"
echo "${with_default:-fallback}"
echo "${assign_default:=value}"
echo "${alt_value:+replacement}"
echo "${strip_shortest_prefix#pre}"
echo "${strip_longest_prefix##*/}"
echo "${strip_shortest_suffix%suf}"
echo "${strip_longest_suffix%%.*}"
echo "${#length}"
echo "${array[0]}"
echo "${array[@]}"
echo "${all_positional[*]}"
echo "${nested:-${inner}}"
echo "prefix${adjacent}suffix"
echo "$1" "$@" "$#" "$?"
