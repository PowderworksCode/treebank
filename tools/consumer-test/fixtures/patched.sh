#!/bin/sh
# One construct per grammar patch, so a published crate that lost a patch
# fails here rather than silently in a consumer's parser.

# 0003 — a case pattern with more than three concatenated parts, as every
# autoconf-generated configure writes it.
case $ac_user_opts in
  *"
"enable_$ac_useropt"
"*) ;;
  *) ac_unrecognized_opts="$ac_unrecognized_opts--disable-$ac_useropt_orig" ;;
esac

# 0004 — substring expansion with a variable offset.
test "${actual:$offset:1}" = "$3"
stale_dirs=("${sorted_dirs[@]:$keep_count}")

# 0005 — a for loop with no word list; the `do` delimits by itself.
for arg do
  echo "--> $arg"
done
