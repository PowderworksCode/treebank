#!/usr/bin/env bash
# Parameter expansion: one `$` with a dozen different groupings behind it.

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
