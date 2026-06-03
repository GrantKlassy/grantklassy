# shellcheck shell=bash
# ============================================================================
# assert.sh — minimal, dependency-free assert library for the shell tests
# ============================================================================
# Works in bash and zsh. Counters accumulate across asserts within one test
# file; gk_done prints a one-line tally and sets the process exit code (0 if
# everything passed, 1 otherwise) so run.sh can tell pass from fail by rc.

GK_PASS=0
GK_FAIL=0
: "${GK_TEST_NAME:=test}"

_gk_pass() { GK_PASS=$(( GK_PASS + 1 )); }
_gk_fail() { GK_FAIL=$(( GK_FAIL + 1 )); printf '    FAIL: %s\n' "$1" >&2; }

# assert_eq EXPECTED ACTUAL [MSG]
assert_eq() {
  if [ "$1" = "$2" ]; then _gk_pass
  else _gk_fail "${3:-assert_eq}: expected [$1] got [$2]"; fi
}

# assert_ne A B [MSG]
assert_ne() {
  if [ "$1" != "$2" ]; then _gk_pass
  else _gk_fail "${3:-assert_ne}: both were [$1]"; fi
}

# assert_contains HAYSTACK NEEDLE [MSG]
assert_contains() {
  case "$1" in
    *"$2"*) _gk_pass ;;
    *) _gk_fail "${3:-assert_contains}: [$1] does not contain [$2]" ;;
  esac
}

# assert_rc EXPECTED_RC CMD [ARGS...] — run CMD, compare its exit code.
assert_rc() {
  local want=$1; shift
  "$@" >/dev/null 2>&1
  local got=$?
  if [ "$got" = "$want" ]; then _gk_pass
  else _gk_fail "assert_rc: '$*' expected rc $want got $got"; fi
}

# fail MSG — unconditional failure (for unreachable branches etc.)
fail() { _gk_fail "$1"; }

# gk_done — print the tally and exit with 0 (all passed) or 1 (any failed).
gk_done() {
  if [ "$GK_FAIL" -eq 0 ]; then
    printf '  %-24s %s passed\n' "$GK_TEST_NAME" "$GK_PASS"
    exit 0
  fi
  printf '  %-24s %s passed, %s FAILED\n' "$GK_TEST_NAME" "$GK_PASS" "$GK_FAIL" >&2
  exit 1
}
