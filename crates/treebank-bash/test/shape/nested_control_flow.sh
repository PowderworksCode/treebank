#!/usr/bin/env bash
# Control flow nested inside control flow: each body is a list, and the
# boundary that matters is where one construct's body ends.

for item in one two three; do
  if [ -n "$item" ]; then
    while read -r line; do
      case "$line" in
        start) echo "begin" ;;
        stop)  echo "end" ;;
        *)     echo "other" ;;
      esac
    done
  elif [ -z "$item" ]; then
    echo "empty"
  else
    echo "fallthrough"
  fi
done

outer_function() {
  inner_function() {
    until [ -e /tmp/flag ]; do
      echo waiting
    done
  }
  inner_function
}

if true; then if false; then echo nested; fi; fi
