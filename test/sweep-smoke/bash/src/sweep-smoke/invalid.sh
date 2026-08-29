# Deliberately invalid bash: the sweep smoke test needs one file that FAILS to parse.
# shellcheck shell=bash disable=SC1046,SC1047,SC1072,SC1073
if true; then
  echo unfinished
