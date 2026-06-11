# shellcheck shell=bash
# targz DIR creates DIR.tar.gz next to DIR, prints the command it runs,
# tolerates a trailing slash, handles spaces in names (the old eval-based
# version word-split them), and refuses missing/invalid args with rc 2
# (an empty $1 used to expand to `tar cvzf .tar.gz /`).
if ! command -v tar >/dev/null 2>&1; then
  printf '  (skipping targz: tar not on PATH)\n'
  gk_done
fi

tmp=$(cd "$(mktemp -d)" && pwd -P)

# Plain directory: tarball appears, and the command is echoed first.
mkdir -p "$tmp/pics" && : > "$tmp/pics/a.txt"
out=$(cd "$tmp" && targz pics 2>/dev/null)
assert_contains "$out" "tar cvzf pics.tar.gz pics/" "prints the command it runs"
assert_rc 0 test -f "$tmp/pics.tar.gz"

# Trailing slash is tolerated: still dir.tar.gz, not dir/.tar.gz.
mkdir -p "$tmp/docs" && : > "$tmp/docs/b.txt"
(cd "$tmp" && targz docs/ >/dev/null 2>&1)
assert_rc 0 test -f "$tmp/docs.tar.gz"

# Spaces in the directory name survive (regression test for the eval version).
mkdir -p "$tmp/two words" && : > "$tmp/two words/c.txt"
(cd "$tmp" && targz "two words" >/dev/null 2>&1)
assert_rc 0 test -f "$tmp/two words.tar.gz"

# No arg / nonexistent dir -> usage error, nothing created.
assert_rc 2 targz
assert_rc 2 targz "$tmp/no-such-dir"
assert_rc 1 test -f "$tmp/.tar.gz"

rm -rf "$tmp"
gk_done
