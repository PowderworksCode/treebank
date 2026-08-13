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

# 0006 — `=` is a synonym for `==` in a test command.
if [ "$arg" = -- ]; then :; fi

# 0007 — a herestring after another redirect.
cat - 2>/dev/null <<< "$xml"

# 0008 / 0016 — arithmetic operands built by concatenation, and an explicit
# base whose digits are an expansion.
rvd=$((0x$(echo 1234)))
off=$((16#$addr))

# 0009 — a backtick substitution whose body starts with whitespace, after an
# earlier one in the same file.
x=`date`
y=` echo hi`

# 0010 — one assignment and a redirection, with no command name.
__PLUSEQ_TEST+=" b" 2>/dev/null && echo yes

# 0011 — parentheses in an expansion default value, escaped and unescaped.
DATA_SECTION_ALIGNMENT="${CREATE_SHLIB-${CREATE_PIE-ALIGN(8)}}"
CPP_FOR_BUILD="${CPP_FOR_BUILD-\$(CC_FOR_BUILD) -E}"
DATA_START_SYMBOLS="${CREATE_SHLIB+PROVIDE (}__data_start = .;"

# 0012 — a trailing compound command with no terminator.
wait_service() {(
  echo waiting
)}

# 0013 — read-write redirection.
exec 4<>/tmp/lockfile

# 0014 — a bracket in an expansion pattern.
SPARE=${SPARE%%]*}

# 0015 — `[` as an ordinary command, in a pipeline.
[ docker --help |& grep -q podman ]

# 0017 — parenthesised text in an expansion value.
echo "BUG: don't know how to print $1${2:+ (from $2)}"

# 0018 — `@` as an ordinary word character.
[ @MYSQL_TCP_PORT_DEFAULT@ -eq 0 ] && echo templated

# 0019 — a heredoc delimiter that ends at a metacharacter.
cat <<HELP; echo after
help text
HELP

# 0020 — a literal dollar before a non-expansion character.
echo one | sed -e s/.ct$//

# 0021 — backtick substitutions joined by a colon.
case $in_lang:`out_lang_tex`:`echo tex` in
  *) ;;
esac

# 0022 — a case pattern with an escaped bracket.
case $1 in
  *\]:*) real_server="${1}" ;;
esac

# 0023 — a trailing list whose right-hand side is a compound.
{ enabled avisynth && { echo require; } }

# 0024 — a substring offset with a comparison and a conditional.
address=${address: ${#address} < 8 ? 0 : -8}

# 0025 — `==` as a command argument inside a compound.
{
  if true; then
    echo "INOTIFY.INODES.RAW" == EXPECTED_JSON
  fi
}

# 0026 — a brace inside a bracket expression in a pattern.
CONF["${v%%[={ ]*}"]=1

# 0027 — a semicolon inside an expansion default value.
OTHER_SYMBOLS="${CREATE_SHLIB-${CREATE_PIE-__elf_header = ${TEXT_START_ADDR};}}"
