# shellcheck shell=sh
# ============================================================================
# test_bootstrap.sh — unit tests for bootstrap.sh's pure helpers
# ============================================================================
# Sourced by run.sh after assert.sh + bootstrap.sh (GK_BOOTSTRAP_LIB=1). Pins
# the side-effect-free logic — the same testing philosophy install.rs applies to
# parse_skip / parse_env0. Anything that actually installs packages or mutates
# the system is left to the Docker end-to-end suite (test/docker/).

# A space-delimited word-membership check, so we can assert a package list does
# (or doesn't) contain a tool without "gh" matching inside "github-cli".
has_word() {
  case " $1 " in
    *" $2 "*) echo yes ;;
    *)        echo no ;;
  esac
}

# --- os_from_uname ---------------------------------------------------------
assert_eq mac         "$(os_from_uname Darwin)"  "Darwin -> mac"
assert_eq linux       "$(os_from_uname Linux)"   "Linux -> linux"
assert_eq unsupported "$(os_from_uname FreeBSD)" "other -> unsupported"

# --- distro_from_os_release ------------------------------------------------
assert_eq fedora "$(printf 'NAME=Fedora\nID=fedora\n' | distro_from_os_release)" \
  "unquoted ID"
assert_eq ubuntu "$(printf 'ID="ubuntu"\nVERSION_ID="24.04"\n' | distro_from_os_release)" \
  "quoted ID, ignores later lines"
assert_eq debian "$(printf 'PRETTY_NAME="Debian GNU/Linux 12"\nID=debian\nID_LIKE=\n' | distro_from_os_release)" \
  "ID picked, ID_LIKE not mistaken for ID"
assert_eq alpine "$(printf "ID='alpine'\n" | distro_from_os_release)" \
  "single-quoted ID (os-release(5) allows both quote styles)"
assert_eq "" "$(printf 'NAME=Weird\nVERSION=1\n' | distro_from_os_release)" \
  "no ID line -> empty"

# --- real_user_decision ----------------------------------------------------
assert_eq self   "$(real_user_decision 1000 '')"       "non-root -> self"
assert_eq self   "$(real_user_decision 1000 'someone')" "non-root ignores SUDO_USER"
assert_eq drop   "$(real_user_decision 0 alice)"        "root + SUDO_USER -> drop"
assert_eq refuse "$(real_user_decision 0 '')"           "bare root -> refuse"

# --- detect_manager (with a faked `have`) ----------------------------------
# detect_manager calls `have`; redefine it per case to simulate which managers
# are present. Resolution is dynamic, so the new `have` is what it sees.
have() { case "$1" in dnf|apt-get) return 0 ;; *) return 1 ;; esac; }
assert_eq dnf "$(detect_manager)" "dnf+apt both present -> dnf wins (fixed order)"
have() { case "$1" in apt-get) return 0 ;; *) return 1 ;; esac; }
assert_eq apt "$(detect_manager)" "apt-get normalizes to apt"
have() { case "$1" in apk) return 0 ;; *) return 1 ;; esac; }
assert_eq apk "$(detect_manager)" "apk detected"
have() { return 1; }
assert_rc 1 detect_manager "no manager -> rc 1"

# --- package_list ----------------------------------------------------------
assert_eq yes "$(has_word "$(package_list dnf)" git)"          "dnf includes git"
assert_eq yes "$(has_word "$(package_list dnf)" gh)"           "dnf includes gh"
assert_eq yes "$(has_word "$(package_list dnf)" vim-enhanced)" "dnf uses vim-enhanced"
assert_eq yes "$(has_word "$(package_list dnf)" jq)"           "dnf includes COMMON (jq)"
assert_eq no  "$(has_word "$(package_list apt)" gh)"           "apt excludes gh (added via apt source)"
assert_eq yes "$(has_word "$(package_list apt)" build-essential)" "apt uses build-essential"
assert_eq yes "$(has_word "$(package_list pacman)" github-cli)"   "pacman uses github-cli"
assert_eq yes "$(has_word "$(package_list pacman)" base-devel)"   "pacman uses base-devel"
assert_eq yes "$(has_word "$(package_list pacman)" libpulse)"     "pacman uses libpulse (not pulseaudio-utils)"
assert_eq no  "$(has_word "$(package_list pacman)" gh)"           "pacman excludes bare gh (it's github-cli there)"
assert_eq yes "$(has_word "$(package_list zypper)" gh)"           "zypper includes gh"
assert_eq yes "$(has_word "$(package_list zypper)" netcat-openbsd)" "zypper uses netcat-openbsd"
assert_eq yes "$(has_word "$(package_list zypper)" bind-utils)"   "zypper uses bind-utils for dig"
assert_eq yes "$(has_word "$(package_list apk)" github-cli)"      "apk uses github-cli"
assert_eq yes "$(has_word "$(package_list apk)" build-base)"      "apk uses build-base"
assert_eq yes "$(has_word "$(package_list apk)" bind-tools)"      "apk uses bind-tools for dig"
# Every manager's list carries git and the shared COMMON kit (jq as proxy).
for _mgr in dnf apt pacman zypper apk; do
  assert_eq yes "$(has_word "$(package_list "$_mgr")" git)" "$_mgr includes git"
  assert_eq yes "$(has_word "$(package_list "$_mgr")" jq)"  "$_mgr includes COMMON (jq)"
done
assert_rc 1 package_list nonsense "unknown manager -> rc 1"

# --- parse_skip / run_step -------------------------------------------------
assert_contains "$(parse_skip 'CLONE')" clone "parse_skip lowercases"
assert_eq "packages rust" "$(parse_skip 'Packages,Rust')" \
  "parse_skip handles commas + mixed case together"
skips=$(parse_skip 'packages, clone')
assert_rc 1 run_step "$skips" packages  "listed step is skipped (rc 1)"
assert_rc 1 run_step "$skips" clone     "second listed step is skipped"
assert_rc 0 run_step "$skips" rust      "unlisted step runs (rc 0)"
assert_rc 0 run_step "" packages        "empty skip list runs everything"

# --- ensure_in_rc idempotency ----------------------------------------------
# Not root in the test, so as_user is a no-op and writes land in the temp file
# directly. Calling twice must leave exactly one marker and one line.
rc_tmp=$(mktemp 2>/dev/null || echo "${TMPDIR:-/tmp}/gk_rc_$$")
: > "$rc_tmp"
ensure_in_rc "$rc_tmp" "gk-test-marker" 'export GK_TEST=1' >/dev/null 2>&1
ensure_in_rc "$rc_tmp" "gk-test-marker" 'export GK_TEST=1' >/dev/null 2>&1
assert_eq 1 "$(grep -c 'gk-test-marker' "$rc_tmp")" "ensure_in_rc writes marker once"
assert_eq 1 "$(grep -c 'export GK_TEST=1' "$rc_tmp")" "ensure_in_rc writes line once"
assert_contains "$(cat "$rc_tmp")" 'export GK_TEST=1' "ensure_in_rc wrote the line verbatim"
rm -f "$rc_tmp"

# ensure_in_rc appends — pre-existing rc content must survive untouched.
rc_tmp=$(mktemp 2>/dev/null || echo "${TMPDIR:-/tmp}/gk_rc2_$$")
printf '# my existing rc\nalias x=y\n' > "$rc_tmp"
ensure_in_rc "$rc_tmp" "gk-test-marker" 'export GK_TEST=1' >/dev/null 2>&1
assert_contains "$(cat "$rc_tmp")" 'alias x=y' "ensure_in_rc preserves existing content"
assert_contains "$(cat "$rc_tmp")" 'export GK_TEST=1' "ensure_in_rc appended after existing content"
rm -f "$rc_tmp"
