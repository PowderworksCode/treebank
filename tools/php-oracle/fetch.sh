#!/usr/bin/env bash
# Fetch a PHP new enough to be an honest validity oracle, without root.
#
# The oracle is `php -l`, and WHICH php runs it is load-bearing exactly the
# way ledger.json's generate_cli is. Measured on 1703 files from the top 40
# Packagist packages: PHP 8.3 rejects 7 of them and PHP 8.4/8.5 reject none.
# All 7 are current Symfony, which declares "php": ">=8.4.1" and uses
# property hooks, asymmetric visibility and new-without-parentheses. On 8.3
# those files are scored invalid, recorded as corpus noise, and any grammar
# gap in PHP 8.4 syntax vanishes from the sweep — on the most-downloaded
# package family in the ecosystem. So php.rs refuses to run below 8.4.
#
# The right php is a distribution one:
#
#     sudo add-apt-repository ppa:ondrej/php && sudo apt install php8.5-cli
#
# and if `php8.5` or `php8.4` is on PATH, php.rs finds it and this script is
# not needed. Ubuntu 24.04 ships only 8.3, though, and a sweep should not
# require root on a machine that does not have it. This fetches a pinned,
# checksummed static build instead, and php.rs picks it up via TREEBANK_PHP.
#
# The cost, stated plainly: this build is ~55 ms per invocation against
# ~11 ms for a distribution build, because a static PHP initialises its
# extensions in every forked process and cannot lazily load a shared object.
# That is 4.8x, and it is affordable only because validate() runs on files
# the grammar ALREADY failed — tens to hundreds, not the whole corpus. At
# the machine's core count that is well under a second either way. If you
# are sweeping a corpus where it is not, install the distribution package.
#
# Verdicts are identical: the minimal build was compared against the full
# static build across 1703 corpus files plus the 22-file adversarial battery
# and disagreed on none. Parsing does not depend on which extensions are
# compiled in, and `-n` means php.ini cannot change that either.
#
# Usage: tools/php-oracle/fetch.sh
#        export TREEBANK_PHP=$(tools/php-oracle/fetch.sh --print)
set -euo pipefail
cd "$(dirname "$0")"

VERSION=8.5.8
SHA256=739689b906ebac0dc55e2453a733e681157840ab84149623fce8638a32b37bbd
URL="https://dl.static-php.dev/static-php-cli/minimal/php-$VERSION-cli-linux-x86_64.tar.gz"
BIN="$PWD/php-$VERSION"

if [ "${1:-}" = "--print" ]; then
  [ -x "$BIN" ] || { echo "php-oracle: $BIN not fetched yet; run $0 first" >&2; exit 1; }
  echo "$BIN"
  exit 0
fi

if [ -x "$BIN" ]; then
  echo "php-oracle: already have $("$BIN" --version | head -1)"
  exit 0
fi

case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) ;;
  *)
    echo "php-oracle: no pinned build for $(uname -s)/$(uname -m)." >&2
    echo "            Install php8.5-cli (or any PHP >= 8.4) and put it on PATH," >&2
    echo "            or point TREEBANK_PHP at one." >&2
    exit 1
    ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "php-oracle: fetching PHP $VERSION (static, minimal)"
curl -sSLo "$tmp/php.tar.gz" "$URL"
echo "$SHA256  $tmp/php.tar.gz" | sha256sum -c - >/dev/null || {
  echo "php-oracle: FAIL — checksum mismatch on $URL" >&2
  echo "  expected $SHA256" >&2
  echo "  got      $(sha256sum "$tmp/php.tar.gz" | cut -d' ' -f1)" >&2
  exit 1
}
tar xzf "$tmp/php.tar.gz" -C "$tmp"
mv "$tmp/php" "$BIN"
chmod +x "$BIN"
"$BIN" --version | head -1
echo "php-oracle: ready at $BIN"
