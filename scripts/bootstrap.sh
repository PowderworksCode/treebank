#!/usr/bin/env bash
# One-time (or occasional) corpus bootstrap: get each language to the point
# where `scripts/daily.sh` can rank, fetch and sweep unattended.
#
# typescript needs nothing — it ranks from npm-high-impact and resolves
# versions from the npm registry at fetch time.
#
# rust needs the crates.io database dump, because that is the only public
# source of all-time download counts. `treebank rank --lang rust` reads four
# CSVs from corpus/rust/db:
#
#   crate_downloads.csv  crates.csv  default_versions.csv  versions.csv
#
# The published dump nests them under <date>/data/, so this script extracts
# just those four, flat, where rank's default --db path expects them, and
# throws the other 14 CSVs away (dependencies.csv and version_downloads.csv
# alone are 2.2 GB and rank never opens them).
#
# Usage: scripts/bootstrap.sh [rust|typescript|all]     (default: all)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:/usr/local/bin:$PATH"
WHAT="${1:-all}"
DUMP_URL="${TREEBANK_DUMP_URL:-https://static.crates.io/db-dump.tar.gz}"
NEEDED=(crate_downloads.csv crates.csv default_versions.csv versions.csv)

cargo build --release --quiet
TB="$PWD/target/release/treebank"

bootstrap_rust() {
  local db="corpus/rust/db" missing=0
  for f in "${NEEDED[@]}"; do [ -f "$db/$f" ] || missing=1; done
  if [ "$missing" = 0 ] && [ -z "${TREEBANK_REFRESH_DUMP:-}" ]; then
    echo "bootstrap: rust db dump already present in $db (TREEBANK_REFRESH_DUMP=1 to re-download)"
  else
    mkdir -p "$db"
    local tgz="$db/db-dump.tar.gz"
    echo "bootstrap: downloading $DUMP_URL (~1.7 GB) — this is the slow part"
    curl -fL --retry 3 -o "$tgz" "$DUMP_URL"
    echo "bootstrap: extracting ${#NEEDED[@]} CSVs (the dump holds 18; the rest are not read)"
    # The dump's paths are <date>/data/<name>.csv; --strip-components=2 lands
    # them flat in $db, and --wildcards matches whatever the date happens to be.
    local pats=()
    for f in "${NEEDED[@]}"; do pats+=("*/data/$f"); done
    tar xzf "$tgz" -C "$db" --strip-components=2 --wildcards "${pats[@]}"
    rm -f "$tgz"
    du -sh "$db"
  fi
  "$TB" rank --lang rust --k "${TREEBANK_RANK_K:-1000}"
}

bootstrap_typescript() {
  "$TB" rank --lang typescript --k "${TREEBANK_RANK_K:-1000}"
}

case "$WHAT" in
  rust) bootstrap_rust ;;
  typescript) bootstrap_typescript ;;
  all) bootstrap_typescript; bootstrap_rust ;;
  *) echo "usage: scripts/bootstrap.sh [rust|typescript|all]" >&2; exit 2 ;;
esac

echo "bootstrap: done — scripts/daily.sh can now run unattended"
