# shellcheck shell=bash
# Colour-table sanity: a sampling of the bare-ESC vars defined in lib.sh.
assert_eq "$(printf '\033[0m')"    "$Color_Off" "Color_Off is the reset code"
assert_eq "$(printf '\033[0;31m')" "$Red"       "Red"
assert_eq "$(printf '\033[1;32m')" "$BGreen"    "BGreen (bold green)"
assert_eq "$(printf '\033[4;34m')" "$UBlue"     "UBlue (underline blue)"
assert_eq "$(printf '\033[44m')"   "$On_Blue"   "On_Blue (background)"
assert_eq "$(printf '\033[0;91m')" "$IRed"      "IRed (high-intensity)"
assert_eq "$(printf '\033[1m')"    "$Bold"      "Bold style"
gk_done
