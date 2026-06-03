# shellcheck shell=bash
# cecho wraps the message in <colour>…<reset> and joins extra args with a space.
assert_eq "${Red}hello world${Color_Off}" "$(cecho "$Red" hello world)" "cecho wraps and joins args"
assert_eq "${Green}hi${Color_Off}"        "$(cecho "$Green" hi)"         "cecho with a single word"
gk_done
