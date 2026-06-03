# shellcheck shell=bash
# Shadow `date` so we assert epoch's ROUTING (mac vs linux), not the wall clock.
# This is the regression test for the old bash-on-macOS bug where epoch only
# ever issued GNU `date -d` and failed on BSD `date`.
date() { echo "date $*"; }

RUNNING_ON_MAC=true
assert_eq "date -r 100"  "$(epoch 100)" "macOS routes to date -r N"

RUNNING_ON_MAC=false
assert_eq "date -d @100" "$(epoch 100)" "linux routes to date -d @N"

# Every argument is converted, in order.
RUNNING_ON_MAC=false
assert_eq "date -d @1
date -d @2" "$(epoch 1 2)" "epoch converts each argument"

unset -f date
gk_done
