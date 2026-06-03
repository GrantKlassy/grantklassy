#!/bin/sh
# ============================================================================
# test.sh — run the whole grantklassy test suite
# ============================================================================
# Three halves, one command:
#   1. the pure-shell library tests, under BOTH bash and zsh (shell/tests/)
#   2. the bootstrap.sh unit tests, under sh + bash (test/bootstrap/)
#   3. the Rust cargo-script unit tests (install.rs, macos/gkdisk.rs)
#
#   ~/.grantklassy/test.sh
set -u
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
rc=0

echo "############ shell library tests (bash + zsh) ############"
sh "$root/shell/tests/run.sh" || rc=1

echo
echo "############ bootstrap.sh unit tests (sh + bash) ############"
sh "$root/test/bootstrap/run.sh" || rc=1

echo
echo "############ rust unit tests (cargo +nightly -Zscript) ############"
if command -v cargo >/dev/null 2>&1; then
  for manifest in install.rs macos/gkdisk.rs; do
    echo "--- $manifest ---"
    ( cd "$root" && cargo +nightly -Zscript test --manifest-path "$manifest" ) || rc=1
  done
else
  echo "cargo not found — skipping rust tests" >&2
fi

echo
if [ "$rc" -eq 0 ]; then echo "OK — all green"; else echo "FAIL — see above"; fi
exit "$rc"
