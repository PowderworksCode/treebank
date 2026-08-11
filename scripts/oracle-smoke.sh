#!/usr/bin/env bash
# Smoke test for the ONE oracle property CI cannot otherwise see.
#
# verify.sh runs the ledger, materialize, `tree-sitter test` and the negative
# corpus. It never invokes an oracle. So the whole class of bug fixed in
# "oracles: an unreadable file is not an invalid file" — where an oracle
# answers `invalid` for a file it could not read, the sweep records every
# grammar failure as corpus noise, gap_files goes to zero and the run reports
# a flawless grammar — is invisible to every check in the repository. It was
# found by hand and it could come back the same way.
#
# Two assertions per oracle, and the second one matters as much as the first:
#
#   1. UNREADABLE IS FATAL. A path that does not exist must produce a
#      non-zero exit and NO verdict on stdout.
#   2. THE ORACLE STILL WORKS. A real valid file must come back `valid` and
#      a real invalid one `invalid`, both at exit 0.
#
# Without (2) an oracle that had simply been broken into always failing would
# pass this test, which would make the guard worse than none.
#
# php is checked differently because it is a different shape: it has no batch
# mode and runs through exec_oracle.rs, which already refuses to turn an
# unexpected exit status into a verdict. What it relies on is that `php -l`
# uses DIFFERENT statuses for "syntax error" (255) and "could not open input
# file" (1). That assumption is upstream php's, not ours, so it is pinned
# here rather than trusted.
#
# Usage:
#   scripts/oracle-smoke.sh                 # skip oracles whose runtime is absent
#   scripts/oracle-smoke.sh --require-all   # a skip is a failure (this is CI)
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

REQUIRE_ALL=0
[ "${1:-}" = "--require-all" ] && REQUIRE_ALL=1

TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
MISSING="$TMP/no/such/file"          # never created, on purpose
pass=0; fail=0; skip=0

ok()   { printf '  \033[32mok\033[0m    %s\n' "$1"; pass=$((pass+1)); }
bad()  { printf '  \033[31mFAIL\033[0m  %s\n' "$1"; fail=$((fail+1)); }
note() { printf '        %s\n' "$1"; }
skipped() {
  if [ "$REQUIRE_ALL" = 1 ]; then
    printf '  \033[31mFAIL\033[0m  %s (skipped, but --require-all)\n' "$1"; fail=$((fail+1))
  else
    printf '  \033[33mskip\033[0m  %s — %s\n' "$1" "$2"; skip=$((skip+1))
  fi
}

# assert_oracle <name> <valid-fixture> <invalid-fixture> <cmd...>
#
# The oracle reads paths on stdin and writes "<path>\t<verdict>" per line.
assert_oracle() {
  local name=$1 good=$2 evil=$3; shift 3

  # 1. unreadable must be fatal, and must not answer
  local out status
  out=$(echo "$MISSING" | "$@" 2>/dev/null); status=$?
  if [ "$status" -eq 0 ]; then
    bad "$name: unreadable file exited 0"
    note "a verdict for a file it never read is recorded as corpus noise"
    return
  fi
  if grep -q 'valid' <<<"$out"; then
    bad "$name: unreadable file produced a verdict: $(tr -d '\n' <<<"$out")"
    return
  fi

  # 2. and it must still be a working oracle
  local verdicts
  verdicts=$(printf '%s\n%s\n' "$good" "$evil" | "$@" 2>/dev/null); status=$?
  if [ "$status" -ne 0 ]; then
    bad "$name: exited $status on readable files"; return
  fi
  local gv ev
  gv=$(awk -F'\t' -v p="$good" '$1==p{print $2}' <<<"$verdicts")
  ev=$(awk -F'\t' -v p="$evil" '$1==p{print $2}' <<<"$verdicts")
  if [ "$gv" != valid ] || [ "$ev" != invalid ]; then
    bad "$name: expected valid/invalid, got '${gv:-<none>}'/'${ev:-<none>}'"; return
  fi
  ok "$name"
}

echo "oracle smoke test — unreadable must be fatal, and the oracle must still work"

# ---------------------------------------------------------------- python
if [ -f tools/py-oracle/check.py ]; then
  if command -v python3 >/dev/null; then
    printf 'x = 1\n' > "$TMP/good.py"; printf 'def f(:\n' > "$TMP/evil.py"
    assert_oracle py-oracle "$TMP/good.py" "$TMP/evil.py" python3 tools/py-oracle/check.py
  else
    skipped py-oracle "no python3"
  fi
fi

# ------------------------------------------------------------------ java
if [ -f tools/java-oracle/Check.java ]; then
  if command -v javac >/dev/null; then
    printf 'class A {}\n' > "$TMP/Good.java"; printf 'class A {\n' > "$TMP/Evil.java"
    assert_oracle java-oracle "$TMP/Good.java" "$TMP/Evil.java" java tools/java-oracle/Check.java
  else
    skipped java-oracle "no JDK (a JRE is not enough)"
  fi
fi

# ---------------------------------------------------------------- csharp
if [ -f tools/cs-oracle/cs-oracle.csproj ]; then
  DLL=tools/cs-oracle/bin/Release/net8.0/cs-oracle.dll
  if command -v dotnet >/dev/null; then
    [ -f "$DLL" ] || (cd tools/cs-oracle && dotnet build -c Release --nologo >/dev/null 2>&1)
    if [ -f "$DLL" ]; then
      printf 'class A {}\n' > "$TMP/good.cs"; printf 'class A {\n' > "$TMP/evil.cs"
      assert_oracle cs-oracle "$TMP/good.cs" "$TMP/evil.cs" dotnet "$DLL"
    else
      skipped cs-oracle "dotnet build failed"
    fi
  else
    skipped cs-oracle "no .NET SDK"
  fi
fi

# ------------------------------------------------------------ js  and  ts
for pair in "js-oracle:good.js:var x = 1;:evil.js:function f( {" \
            "ts-oracle:good.ts:let x: number = 1;:evil.ts:interface I {"; do
  IFS=: read -r name gf gsrc ef esrc <<<"$pair"
  [ -f "tools/$name/check.mjs" ] || continue
  if command -v node >/dev/null; then
    [ -d "tools/$name/node_modules" ] || (cd "tools/$name" && npm ci --no-audit --no-fund >/dev/null 2>&1)
    printf '%s\n' "$gsrc" > "$TMP/$gf"; printf '%s\n' "$esrc" > "$TMP/$ef"
    assert_oracle "$name" "$TMP/$gf" "$TMP/$ef" node "tools/$name/check.mjs"
  else
    skipped "$name" "no node"
  fi
done

# --------------------------------------------------------------------- go
if [ -f tools/go-oracle/oracle.go ]; then
  if command -v go >/dev/null; then
    [ -x tools/go-oracle/go-oracle ] || tools/go-oracle/build.sh >/dev/null 2>&1
    printf 'package a\n' > "$TMP/good.go"; printf 'package a\n\nfunc f() {\n' > "$TMP/evil.go"
    assert_oracle go-oracle "$TMP/good.go" "$TMP/evil.go" tools/go-oracle/go-oracle
  else
    skipped go-oracle "no go toolchain"
  fi
fi

# ---------------------------------------------------------------------- c
# Different contract: input is "<path>\t<flag>...", output is JSON per line,
# and the verdict is three-valued.
if [ -f tools/c-oracle/oracle.c ]; then
  if [ -x tools/c-oracle/c-oracle ] || tools/c-oracle/build.sh >/dev/null 2>&1; then
    out=$(printf '%s\t-std=gnu17\n' "$MISSING" | tools/c-oracle/c-oracle 2>/dev/null); status=$?
    if [ "$status" -eq 0 ]; then
      bad "c-oracle: unreadable file exited 0"
      note 'c.rs maps every verdict that is not "valid" to false, i.e. to noise'
    elif grep -q '"verdict"' <<<"$out"; then
      bad "c-oracle: unreadable file produced a verdict: $out"
    else
      printf 'int main(void) { return 0; }\n' > "$TMP/good.c"
      printf 'int main(void) { return 0;\n'   > "$TMP/evil.c"
      out=$(printf '%s\t-std=gnu17\n%s\t-std=gnu17\n' "$TMP/good.c" "$TMP/evil.c" \
            | tools/c-oracle/c-oracle 2>/dev/null); status=$?
      gv=$(grep -F "$TMP/good.c" <<<"$out" | sed -n 's/.*"verdict":"\([a-z]*\)".*/\1/p')
      ev=$(grep -F "$TMP/evil.c" <<<"$out" | sed -n 's/.*"verdict":"\([a-z]*\)".*/\1/p')
      if [ "$status" -ne 0 ]; then
        bad "c-oracle: exited $status on readable files"
      elif [ "$gv" != valid ] || [ "$ev" != invalid ]; then
        bad "c-oracle: expected valid/invalid, got '${gv:-<none>}'/'${ev:-<none>}'"
      else
        ok c-oracle
      fi
    fi
  else
    skipped c-oracle "no libclang headers (apt install libclang-20-dev)"
  fi
fi

# -------------------------------------------------------------------- php
# Not a batch oracle: exec_oracle.rs forks `php -l` per file and reads the
# exit status. It only stays honest while php keeps 255 (syntax error) and
# 1 (could not open input file) distinct, so pin that.
if [ -d tools/php-oracle ]; then
  if command -v php >/dev/null; then
    php -l "$MISSING" >/dev/null 2>&1; missing_status=$?
    printf '<?php\nfunction f( {\n' > "$TMP/evil.php"
    php -l "$TMP/evil.php" >/dev/null 2>&1; evil_status=$?
    # shellcheck disable=SC2016  # $x is PHP source, not a shell expansion
    printf '<?php\n$x = 1;\n' > "$TMP/good.php"
    php -l "$TMP/good.php" >/dev/null 2>&1; good_status=$?
    if [ "$good_status" -ne 0 ]; then
      bad "php -l: valid file exited $good_status, expected 0"
    elif [ "$evil_status" -ne 255 ]; then
      bad "php -l: syntax error exited $evil_status, but exec_oracle passes reject_status 255"
    elif [ "$missing_status" -eq 255 ]; then
      bad "php -l: a missing file now exits 255 too, so exec_oracle would call it invalid"
    else
      ok "php -l (missing=$missing_status, syntax error=$evil_status, distinct)"
    fi
  else
    skipped php-oracle "no php"
  fi
fi

echo
if [ "$fail" -gt 0 ]; then
  echo "oracle smoke: $fail failed, $pass passed, $skip skipped"
  exit 1
fi
echo "oracle smoke: $pass passed, $skip skipped"
