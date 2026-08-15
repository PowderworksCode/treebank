#!/usr/bin/env bash
# Fetch the scalameta jars the oracle runs on, exactly as pinned.
#
# jars.lock is a lockfile in the same spirit as `npm ci, never npm install`:
# every jar of the resolved transitive set, by URL and sha256, so the oracle
# runs on the bytes the ledger's sweep numbers were produced with and a
# silent upstream republish cannot change what "invalid" means. It was
# produced with coursier (`cs fetch org.scalameta:scalameta_2.13:4.17.3`),
# but nothing here needs coursier: the URLs are plain Maven Central.
#
# Writes jars/ and classpath, both gitignored. Idempotent; verifies on every
# run, so a truncated download is caught rather than cached.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p jars

: > classpath.tmp
while read -r sha url; do
  [ -n "$sha" ] || continue
  jar="jars/$(basename "$url")"
  if [ ! -f "$jar" ] || [ "$(sha256sum "$jar" | cut -d' ' -f1)" != "$sha" ]; then
    echo "scala-oracle: fetching $(basename "$url")"
    curl -fLsS "$url" -o "$jar"
  fi
  have=$(sha256sum "$jar" | cut -d' ' -f1)
  if [ "$have" != "$sha" ]; then
    echo "scala-oracle: FAIL — $jar sha256 $have, jars.lock says $sha" >&2
    exit 1
  fi
  printf '%s:' "$PWD/$jar" >> classpath.tmp
done < jars.lock

sed 's/:$//' classpath.tmp > classpath
rm -f classpath.tmp
echo "scala-oracle: $(wc -l < jars.lock) jars verified -> $PWD/classpath"
