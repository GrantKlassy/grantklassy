#!/bin/sh
# ============================================================================
# run.sh — exercise bootstrap.sh's pure helpers under sh AND bash
# ============================================================================
# bootstrap.sh is POSIX sh and is *executed* by /bin/sh (or bash) — never
# sourced by an interactive shell — so we test it under those two, not zsh
# (that's what shell/tests/ is for, the interactive lib). Each test file is run
# in a fresh subshell that sources assert.sh + bootstrap.sh (GK_BOOTSTRAP_LIB=1,
# so main never runs) + the test, then calls gk_done (which exits 0/1).
#
#   sh ~/.grantklassy/test/bootstrap/run.sh
set -u

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
gk_root=$(CDPATH= cd -- "$here/../.." && pwd)        # ~/.grantklassy
repo_root=$(CDPATH= cd -- "$gk_root/.." && pwd)      # $HOME (repo root)
assert="$gk_root/shell/tests/assert.sh"
bootstrap="$repo_root/bootstrap.sh"

[ -f "$bootstrap" ] || { echo "run.sh: $bootstrap not found" >&2; exit 1; }
[ -f "$assert" ]    || { echo "run.sh: $assert not found" >&2; exit 1; }

# Which of sh/bash are installed here? (On Debian/Ubuntu containers `sh` is
# dash — the strictest POSIX shell, so it catches accidental bashisms.)
shells=
for s in sh bash; do
  if command -v "$s" >/dev/null 2>&1; then shells="$shells $s"; fi
done
[ -n "$shells" ] || { echo "run.sh: need sh or bash on PATH" >&2; exit 1; }

fails=0
ran=0
for shell in $shells; do
  printf '\n=== %s ===\n' "$shell"
  for t in "$here"/test_*.sh; do
    [ -e "$t" ] || continue
    ran=$(( ran + 1 ))
    # $0=gk_runner, $1=assert.sh, $2=bootstrap.sh, $3=test file.
    GK_TEST_NAME=$(basename "$t" .sh) GK_BOOTSTRAP_LIB=1 "$shell" -c '
      . "$1"
      . "$2"
      . "$3"
      gk_done
    ' gk_runner "$assert" "$bootstrap" "$t" || fails=$(( fails + 1 ))
  done
done

printf '\n'
if [ "$fails" -eq 0 ]; then
  printf 'ALL GREEN — %s test run(s), 0 failed\n' "$ran"
  exit 0
fi
printf 'RED — %s test run(s) failed\n' "$fails" >&2
exit 1
