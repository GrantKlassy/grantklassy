#!/bin/sh
# ============================================================================
# run.sh — run every test_*.sh under BOTH bash and zsh
# ============================================================================
# Pure POSIX-sh orchestrator, no external deps. For each available shell and
# each test file it spawns a fresh subshell that sources assert.sh + the shared
# lib.sh + the test, then calls gk_done (which exits 0/1). Running the SAME test
# files under both shells is what enforces the bash/zsh mirror invariants (e.g.
# host_color must pick the same colour despite 0- vs 1-indexed arrays).
#
#   sh ~/.grantklassy/shell/tests/run.sh
set -u

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
: "${GK_SHELL_DIR:=$(CDPATH= cd -- "$here/.." && pwd)}"   # the shell/ dir (parent of tests/)
export GK_SHELL_DIR

# Which of bash/zsh are installed here?
shells=
for s in bash zsh; do
  if command -v "$s" >/dev/null 2>&1; then shells="$shells $s"; fi
done
if [ -z "$shells" ]; then echo "run.sh: need bash or zsh on PATH" >&2; exit 1; fi

fails=0
ran=0
for shell in $shells; do
  printf '\n=== %s ===\n' "$shell"
  for t in "$here"/test_*.sh; do
    [ -e "$t" ] || continue
    ran=$(( ran + 1 ))
    # $0=gk_runner, $1=tests dir, $2=test file. GK_TEST_NAME passed via env.
    GK_TEST_NAME=$(basename "$t" .sh) "$shell" -c '
      . "$1/assert.sh"
      . "$GK_SHELL_DIR/lib.sh"
      . "$2"
      gk_done
    ' gk_runner "$here" "$t" || fails=$(( fails + 1 ))
  done
done

printf '\n'
if [ "$fails" -eq 0 ]; then
  printf 'ALL GREEN — %s test run(s), 0 failed\n' "$ran"
  exit 0
fi
printf 'RED — %s test run(s) failed\n' "$fails" >&2
exit 1
