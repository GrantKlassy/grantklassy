#!/bin/sh
# ============================================================================
# in-container.sh — install.rs assertions, run INSIDE a distro container (root)
# ============================================================================
# Mounted at /in-container.sh; the repo snapshot is mounted at /gk.tar. Exits 0
# if every assertion passes, 1 otherwise, so run.sh can build a pass/fail matrix.
#
# Assertions (all offline — no gh auth, no claude download):
#   1. install.rs refuses to run as root
#   2. the Rust unit tests pass on this distro (pure logic is cross-platform)
#   3. the shell library tests pass under this distro's bash AND zsh
#   4. install.rs's shell-env wiring is correct and idempotent (run as non-root)
#   5. bootstrap.sh refuses bare root (uid 0 with no SUDO_USER)
#   6. bootstrap.sh's unit tests pass under this distro's sh (dash!) AND bash
#   7. bootstrap.sh's shell-env wiring is correct and idempotent (run as non-root)
set -u

PASS=0
FAIL=0
ok()  { PASS=$(( PASS + 1 )); printf '  PASS: %s\n' "$1"; }
bad() { FAIL=$(( FAIL + 1 )); printf '  FAIL: %s\n' "$1" >&2; }

REPO=/opt/gk
GK="$REPO/.grantklassy"

echo "===== distro: $( . /etc/os-release 2>/dev/null; echo "${PRETTY_NAME:-unknown}" ) ====="
echo "rust: $(cargo +nightly --version 2>&1)"

echo "== extract repo snapshot -> $REPO =="
mkdir -p "$REPO"
tar -C "$REPO" -xf /gk.tar
chmod +x "$GK/install.rs" "$GK/macos/gkdisk.rs" "$GK/shell/tests/run.sh" 2>/dev/null || true

# --- 1. refuse_root --------------------------------------------------------
echo "== 1. refuse_root (install.rs as root) =="
export CARGO_HOME=/root/.cargo
out=$( cd "$GK" && ./install.rs 2>&1 )
rc=$?
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -qi 'refusing to run as root'; then
  ok "install.rs refuses root (rc=$rc)"
else
  bad "install.rs did not refuse root (rc=$rc)"
  printf '%s\n' "$out" | tail -4
fi

# --- 2. rust unit tests ----------------------------------------------------
echo "== 2. rust unit tests =="
for m in install.rs macos/gkdisk.rs; do
  if ( cd "$GK" && cargo +nightly -Zscript test --manifest-path "$m" ) >/tmp/rust.log 2>&1; then
    ok "cargo test $m ($(grep -oE '[0-9]+ passed' /tmp/rust.log | head -1))"
  else
    bad "cargo test $m"
    tail -15 /tmp/rust.log
  fi
done

# --- 3. shell library tests (bash + zsh) -----------------------------------
echo "== 3. shell library tests (bash + zsh) =="
if sh "$GK/shell/tests/run.sh" >/tmp/shell.log 2>&1; then
  ok "shell harness ($(grep -c ' passed' /tmp/shell.log) test runs)"
else
  bad "shell harness"
  cat /tmp/shell.log
fi

# --- 4. shell-env wiring + idempotency (as a non-root user) ----------------
echo "== 4. shell-env wiring + idempotency (as tester) =="
id tester >/dev/null 2>&1 || useradd -m -s /bin/bash tester 2>/dev/null || adduser -D -s /bin/sh tester
THOME=$(eval echo "~tester")
tar -C "$THOME" -xf /gk.tar
chown -R tester "$THOME"

# install.rs as tester, only the offline shell-env step. env(1) sets PATH so the
# `cargo` proxy is found (su resets PATH), and a writable per-user CARGO_HOME.
run_as_tester() {
  su tester -c "cd '$THOME/.grantklassy' && env PATH=/usr/local/cargo/bin:\$PATH HOME='$THOME' CARGO_HOME='$THOME/.cargo' RUSTUP_HOME=/usr/local/rustup GK_INSTALL_SKIP=nightly,claude,karabiner,clone ./install.rs" 2>&1
}

first=$( run_as_tester ); rc1=$?
missing=
for f in .profile .zshenv .bashrc .bash_profile; do
  grep -q 'grantklassy/install.rs' "$THOME/$f" 2>/dev/null || missing="$missing $f"
done
if [ "$rc1" -eq 0 ] && [ -z "$missing" ]; then
  ok "wrote markers to .profile/.zshenv/.bashrc/.bash_profile"
else
  bad "shell-env wiring (rc=$rc1 missing:${missing:- none})"
  printf '%s\n' "$first" | tail -8
fi

# second run: idempotent — must say "already has" and not duplicate the marker
marker='grantklassy/install.rs: PATH additions'
before=$( grep -c "$marker" "$THOME/.bashrc" 2>/dev/null || echo 0 )
second=$( run_as_tester ); rc2=$?
after=$( grep -c "$marker" "$THOME/.bashrc" 2>/dev/null || echo 0 )
if [ "$rc2" -eq 0 ] && printf '%s' "$second" | grep -qi 'already has' && [ "$before" = "$after" ]; then
  ok "second run idempotent (marker count stayed $before)"
else
  bad "idempotency (rc=$rc2 before=$before after=$after)"
  printf '%s\n' "$second" | tail -8
fi

# --- 5. bootstrap.sh refuses bare root -------------------------------------
echo "== 5. bootstrap.sh refuses bare root (uid 0, no SUDO_USER) =="
# Refusal happens in compute_real_user, before any package/network step, so this
# never actually installs anything. Unset SUDO_USER in a subshell to be sure.
out=$( cd "$REPO" && ( unset SUDO_USER; sh bootstrap.sh ) 2>&1 )
rc=$?
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -qi 'bare root'; then
  ok "bootstrap.sh refuses bare root (rc=$rc)"
else
  bad "bootstrap.sh did not refuse bare root (rc=$rc)"
  printf '%s\n' "$out" | tail -4
fi

# --- 6. bootstrap.sh unit tests (sh + bash) --------------------------------
echo "== 6. bootstrap.sh unit tests (sh + bash) =="
if sh "$GK/test/bootstrap/run.sh" >/tmp/bootstrap.log 2>&1; then
  ok "bootstrap unit tests ($(grep -c ' passed' /tmp/bootstrap.log) test runs)"
else
  bad "bootstrap unit tests"
  cat /tmp/bootstrap.log
fi

# --- 7. bootstrap.sh shell-env wiring + idempotency (as a non-root user) ----
echo "== 7. bootstrap.sh shell-env wiring + idempotency (as tester) =="
# Fresh HOME so this is independent of section 4's install.rs run (which shares
# the cargo-env marker). Only the offline shell-env step runs — packages/rust/
# clone are all network/root and skipped via GK_BOOTSTRAP_SKIP.
BHOME=$(mktemp -d /tmp/gk-bhome.XXXXXX)
chown tester "$BHOME"
run_bootstrap_tester() {
  su tester -c "cd '$BHOME' && env HOME='$BHOME' GK_BOOTSTRAP_SKIP=packages,rust,clone sh '$REPO/bootstrap.sh'" 2>&1
}

bfirst=$( run_bootstrap_tester ); brc1=$?
bmissing=
for f in .zshenv .bashrc; do
  grep -q 'grantklassy/bootstrap: cargo env' "$BHOME/$f" 2>/dev/null || bmissing="$bmissing $f"
done
if [ "$brc1" -eq 0 ] && [ -z "$bmissing" ]; then
  ok "bootstrap wrote cargo-env marker to .zshenv + .bashrc"
else
  bad "bootstrap shell-env wiring (rc=$brc1 missing:${bmissing:- none})"
  printf '%s\n' "$bfirst" | tail -8
fi

# second run: idempotent — must say "already" and not duplicate the marker
bmarker='grantklassy/bootstrap: cargo env'
bbefore=$( grep -c "$bmarker" "$BHOME/.bashrc" 2>/dev/null || echo 0 )
bsecond=$( run_bootstrap_tester ); brc2=$?
bafter=$( grep -c "$bmarker" "$BHOME/.bashrc" 2>/dev/null || echo 0 )
if [ "$brc2" -eq 0 ] && printf '%s' "$bsecond" | grep -qi 'already has' && [ "$bbefore" = "$bafter" ]; then
  ok "bootstrap second run idempotent (marker count stayed $bbefore)"
else
  bad "bootstrap idempotency (rc=$brc2 before=$bbefore after=$bafter)"
  printf '%s\n' "$bsecond" | tail -8
fi

echo
echo "RESULT: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
