# shellcheck shell=bash
# lines N prints exactly N blank lines; non-negative-integer input only.
assert_eq 1 "$(lines   | wc -l | tr -d ' ')" "lines with no arg defaults to 1"
assert_eq 3 "$(lines 3 | wc -l | tr -d ' ')" "lines 3 prints 3"
assert_eq 0 "$(lines 0 | wc -l | tr -d ' ')" "lines 0 prints 0"
assert_rc 2 lines abc   # non-numeric
assert_rc 2 lines -1    # negative
assert_rc 2 lines 1.5   # non-integer
gk_done
