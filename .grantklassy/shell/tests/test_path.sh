# shellcheck shell=bash
# path_prepend adds a dir to the front of PATH unless it's already present, and
# is idempotent (so re-sourcing the rc via `reload` never grows PATH).
PATH=/usr/bin:/bin

path_prepend /usr/bin
assert_eq "/usr/bin:/bin" "$PATH" "skips an entry already on PATH"

path_prepend /opt/x
assert_eq "/opt/x:/usr/bin:/bin" "$PATH" "prepends a new entry"

path_prepend /opt/x
assert_eq "/opt/x:/usr/bin:/bin" "$PATH" "is idempotent on repeat"

gk_done
