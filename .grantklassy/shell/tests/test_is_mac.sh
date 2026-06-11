# shellcheck shell=bash
# is_mac honors a forced $RUNNING_ON_MAC (how the entrypoints and tests drive
# it) and falls back to `uname -s` when the variable is unset or empty.
RUNNING_ON_MAC=true
assert_rc 0 is_mac

RUNNING_ON_MAC=false
assert_rc 1 is_mac

# Anything that isn't the literal `true` is false-y, not an error.
RUNNING_ON_MAC=yes
assert_rc 1 is_mac

# Unset -> fall back to uname; assert agreement with the real platform so this
# test is correct on macOS and Linux alike.
unset RUNNING_ON_MAC
if [ "$(uname -s)" = Darwin ]; then
  assert_rc 0 is_mac
else
  assert_rc 1 is_mac
fi

gk_done
