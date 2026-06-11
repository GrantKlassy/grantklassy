#!/bin/sh
# ============================================================================
# bootstrap.sh — stage 1 of the grantklassy 2-stage launch
# ============================================================================
# The grantklassy repo is a dotfiles + machine-setup repo checked out directly
# into $HOME. Setup is two commands:
#
#   1. curl -fsSL https://raw.githubusercontent.com/GrantKlassy/grantklassy/main/bootstrap.sh | sudo sh
#   2. ~/.grantklassy/install.rs
#
# This file is stage 1. It does everything needed to make stage 2 *exist*.
#
# Written in POSIX sh — no bashisms — so the same script works whether piped
# into sh, bash, or zsh, on macOS, Fedora, Debian/Ubuntu, Arch, openSUSE, or
# Alpine. That portability is the whole point: a single /bin/sh is the lowest
# common denominator present on every target, so bootstrapping never depends on
# a language runtime (python, etc.) that a fresh or minimal box might not have.
#
# Sudo-aware: when invoked as `sudo sh bootstrap.sh`, every $HOME-touching step
# (brew, rustup, dotfile writes, the clone) drops back down to SUDO_USER so root
# never owns files under the user's home. Invoked directly (no sudo) it runs as
# you and uses `sudo` itself for the package installs. Bare root (uid 0 with no
# SUDO_USER) is refused — there's no account to own the home files.
#
# What gets installed (logical names; per-distro package names in package_list):
#   - vcs:        git, gh
#   - lang:       rust (rustup, stable + nightly — nightly is required by
#                 ~/.grantklassy/install.rs, which runs as `cargo +nightly -Zscript`)
#   - net:        curl, wget, ca-certificates
#   - process:    top (procps), htop
#   - files:      less, unzip, rsync
#   - net debug:  ssh client, dig, ping, nc
#   - modern:     jq, tmux
#   - build:      gcc, make, pkg-config
#   - editor:     vim
#   - audio:      pactl, paplay (pulseaudio-utils; libpulse on Arch)
#
# Then it wires `. "$HOME/.cargo/env"` into ~/.zshenv and ~/.bashrc (and `brew
# shellenv` into ~/.zprofile / ~/.bash_profile on macOS) so future shells find
# cargo/rustup/brew on PATH, and finally clones the (public) grantklassy repo
# into $HOME so ~/.grantklassy/install.rs is on disk for stage 2. Cloning a
# *public* repo needs no `gh auth login`, which is what lets stage 1 be a single
# command.
#
# Steps can be skipped with GK_BOOTSTRAP_SKIP — a comma/space-separated list of
# step names (packages, rust, shell-env, clone). The container tests use it to
# exercise the offline shell-env wiring without the network/package steps. The
# twin of install.rs's GK_INSTALL_SKIP.
#
# Run the unit tests with:
#   sh ~/.grantklassy/test/bootstrap/run.sh

# The public repo this script clones into $HOME for stage 2. Cloning a *public*
# repo needs no auth, which is the whole reason the two-command launch works.
REPO_URL=https://github.com/GrantKlassy/grantklassy.git

# Tools whose package name is identical across every package manager. The rest
# are spelled out per-manager in package_list (names diverge: dig is
# bind-utils/dnsutils/bind/bind-tools, nc is nmap-ncat/openbsd-netcat/
# netcat-openbsd, the C toolchain is gcc+make vs build-essential vs base-devel
# vs build-base, the pulseaudio client tools are pulseaudio-utils vs libpulse on
# Arch, etc.).
COMMON='curl wget less unzip rsync jq tmux htop'

main() {
  # set -eu lives here, not at file scope, so the test harness can `source`
  # this script (GK_BOOTSTRAP_LIB=1) to reach the pure helpers without flipping
  # errexit on in the harness's own shell. Once main starts, -eu is in effect
  # for everything it calls — same behavior as the install.sh this replaces.
  set -eu

  step "platform"
  detect_platform
  compute_real_user
  info "os=$OS${DISTRO:+ distro=$DISTRO} arch=$(uname -m) user=$REAL_USER home=$REAL_HOME"

  skips=$(parse_skip "${GK_BOOTSTRAP_SKIP:-}")

  step "packages"
  run_step "$skips" packages && install_essentials

  step "rust"
  if run_step "$skips" rust; then
    install_rust
    ensure_nightly
  fi

  step "shell env"
  run_step "$skips" shell-env && ensure_shell_env

  # Clone last: once it returns, ~/.grantklassy/install.rs is on disk, so the
  # very next thing the user runs is stage 2.
  step "clone grantklassy"
  run_step "$skips" clone && clone_grantklassy

  step "done"
  info "stage 2 — run as yourself, never sudo:"
  info "    ~/.grantklassy/install.rs"
  info "to pick up the new env in this same window first, run: \`exec \$SHELL -l\`"
}

# --- logging ---------------------------------------------------------------

step() { printf '\n\033[1;36m>>> %s\033[0m\n' "$1"; }
info() { printf '\033[34minfo\033[0m %s\n' "$1"; }
warn() { printf '\033[33mwarn\033[0m %s\n' "$1" >&2; }
die()  { printf '\033[31merror\033[0m %s\n' "$1" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

SUDO=
OS=
DISTRO=
REAL_USER=
REAL_HOME=

# --- pure helpers (no I/O, no globals) -------------------------------------
# These hold the logic the unit tests pin down; the impure steps below are thin
# wrappers over them. Keeping them side-effect-free is what makes them testable
# without actually being root or mutating the system (cf. install.rs's
# parse_skip / parse_env0).

# parse_skip — normalize a GK_BOOTSTRAP_SKIP value (comma/space list) into a
# space-separated, lowercased token string suitable for run_step. [:upper:]
# rather than A-Z so the fold is locale-safe (and shellcheck-clean: SC2018).
parse_skip() {
  printf '%s' "$1" | tr ',' ' ' | tr '[:upper:]' '[:lower:]'
}

# run_step STEP-LIST NAME — succeed (0) if NAME should run; if NAME is in the
# skip list, print a note and return 1, so callers read as
# `run_step "$skips" name && do_it`.
run_step() {
  for _s in $1; do
    if [ "$_s" = "$2" ]; then
      info "skipping \`$2\` (GK_BOOTSTRAP_SKIP)"
      return 1
    fi
  done
  return 0
}

# os_from_uname STRING — map `uname -s` output to our OS tag.
os_from_uname() {
  case "$1" in
    Darwin) echo mac ;;
    Linux)  echo linux ;;
    *)      echo unsupported ;;
  esac
}

# distro_from_os_release — read /etc/os-release text on stdin, echo the `ID=`
# value with surrounding quotes stripped (e.g. fedora, ubuntu, debian), or
# nothing if there's no ID line. `ID_LIKE=` etc. don't match the `ID=*` glob.
# os-release(5) allows single OR double quoting, so both are stripped.
distro_from_os_release() {
  while IFS= read -r _line; do
    case "$_line" in
      ID=*)
        _v=${_line#ID=}
        _v=${_v#\"}; _v=${_v%\"}
        _v=${_v#\'}; _v=${_v%\'}
        echo "$_v"
        return
        ;;
    esac
  done
  echo ""
}

# real_user_decision UID SUDO_USER — decide whose $HOME we operate on:
#   not root          -> self    (we are the user already)
#   root + SUDO_USER  -> drop    (sudo'd in; drop back down)
#   bare root         -> refuse  (no account to own files)
real_user_decision() {
  if [ "$1" -ne 0 ]; then echo self; return; fi
  if [ -n "$2" ]; then echo drop; return; fi
  echo refuse
}

# detect_manager — echo the first supported package manager present on PATH.
# Order is fixed: a box with both dnf and apt is Fedora-family, so dnf wins.
# Returns 1 (echoing nothing) when none is found.
detect_manager() {
  for _m in dnf apt-get pacman zypper apk; do
    if have "$_m"; then
      case "$_m" in
        apt-get) echo apt ;;
        *)       echo "$_m" ;;
      esac
      return 0
    fi
  done
  return 1
}

# package_list MANAGER — echo the baseline package set for that manager. gh is
# intentionally absent from the apt list: Debian/Ubuntu don't ship it, so
# install_gh_debian adds GitHub's apt source separately. Returns 1 on an
# unknown manager so a typo fails loudly instead of installing nothing.
package_list() {
  case "$1" in
    dnf)    echo "git gh ca-certificates $COMMON gcc make pkgconf-pkg-config vim-enhanced procps-ng bind-utils iputils openssh-clients nmap-ncat pulseaudio-utils" ;;
    apt)    echo "git ca-certificates $COMMON build-essential pkg-config vim procps dnsutils iputils-ping openssh-client netcat-openbsd pulseaudio-utils" ;;
    pacman) echo "git github-cli ca-certificates $COMMON base-devel vim procps-ng bind iputils openssh openbsd-netcat libpulse" ;;
    zypper) echo "git gh ca-certificates $COMMON gcc make pkg-config vim procps bind-utils iputils openssh netcat-openbsd pulseaudio-utils" ;;
    apk)    echo "git github-cli ca-certificates $COMMON build-base pkgconf vim procps bind-tools iputils openssh netcat-openbsd pulseaudio-utils" ;;
    *)      return 1 ;;
  esac
}

# --- platform + identity ---------------------------------------------------

detect_platform() {
  OS=$(os_from_uname "$(uname -s)")
  [ "$OS" = unsupported ] && die "unsupported OS: $(uname -s)"
  DISTRO=
  if [ "$OS" = linux ] && [ -r /etc/os-release ]; then
    DISTRO=$(distro_from_os_release < /etc/os-release)
  fi
  # When we're the user (not sudo'd in) and sudo exists, package installs need
  # to elevate themselves.
  if [ "$(id -u)" -ne 0 ] && have sudo; then
    SUDO=sudo
  fi
}

# Resolve REAL_USER/REAL_HOME from euid + $SUDO_USER, or refuse bare root.
# See real_user_decision for the pure core. When sudo'd in, $HOME is root's, so
# resolve the invoking user's home via `eval echo ~user` (the POSIX way to map a
# username to its home dir; SUDO_USER comes from sudo, not user input).
compute_real_user() {
  case "$(real_user_decision "$(id -u)" "${SUDO_USER:-}")" in
    self)
      REAL_USER=$(id -un)
      REAL_HOME=$HOME
      ;;
    drop)
      REAL_USER=$SUDO_USER
      REAL_HOME=$(eval echo "~$SUDO_USER")
      ;;
    refuse)
      die "refusing to run as bare root — run as your normal user (it sudo's for package installs itself), or \`sudo sh bootstrap.sh\` from your account, so files in your home stay user-owned"
      ;;
  esac
  [ -n "$REAL_HOME" ] && [ -d "$REAL_HOME" ] || die "could not resolve home dir for $REAL_USER"
}

# Run "$@" as REAL_USER. No-op wrapper when we're already that user; otherwise
# prefixes `sudo -u USER -H` so HOME points at the user's home (not /var/root).
as_user() {
  if [ "$(id -u)" -eq 0 ] && [ -n "${SUDO_USER:-}" ]; then
    sudo -u "$SUDO_USER" -H -- "$@"
  else
    "$@"
  fi
}

# --- packages --------------------------------------------------------------

# Install everything in one pass per package manager — one update + one install
# is faster than per-package loops. The package names come from package_list;
# the install verb differs per manager.
install_essentials() {
  if [ "$OS" = mac ]; then
    ensure_brew
    # macOS already ships ssh/dig/ping/ca-certs; Xcode CLT (auto-installed by
    # brew on first install) gives gcc/make. Just brew the rest. as_user because
    # brew refuses to run as root; absolute brew path because under `sudo -u`
    # the inherited PATH is sudo's secure_path, which omits /opt/homebrew/bin.
    # shellcheck disable=SC2086
    as_user "$(brew_bin)" install git gh pkg-config vim $COMMON
    return
  fi

  manager=$(detect_manager) || die "no supported package manager (dnf/apt/pacman/zypper/apk)"
  pkgs=$(package_list "$manager")
  # shellcheck disable=SC2086
  case "$manager" in
    dnf)    $SUDO dnf install -y $pkgs ;;
    apt)    $SUDO apt-get update; $SUDO apt-get install -y $pkgs; install_gh_debian ;;
    pacman) $SUDO pacman -S --noconfirm --needed $pkgs ;;
    zypper) $SUDO zypper install -y $pkgs ;;
    apk)    $SUDO apk add $pkgs ;;
  esac
}

# gh isn't in Debian/Ubuntu default repos; add the official GitHub apt source
# first. Idempotent — overwrites keyring + list each run.
install_gh_debian() {
  if have gh; then return; fi
  keyring=/etc/apt/keyrings/githubcli-archive-keyring.gpg
  list=/etc/apt/sources.list.d/github-cli.list
  arch=$(dpkg --print-architecture)
  $SUDO mkdir -p -m 755 /etc/apt/keyrings /etc/apt/sources.list.d
  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    | $SUDO tee "$keyring" >/dev/null
  $SUDO chmod go+r "$keyring"
  printf 'deb [arch=%s signed-by=%s] https://cli.github.com/packages stable main\n' \
    "$arch" "$keyring" | $SUDO tee "$list" >/dev/null
  $SUDO apt-get update
  $SUDO apt-get install -y gh
}

# --- rust ------------------------------------------------------------------

# Ensure a rust toolchain. Check the absolute ~/.cargo/bin/rustup path first:
# under `sudo sh bootstrap.sh` our PATH is sudo's secure_path and omits
# ~/.cargo/bin, so a PATH probe would miss a perfectly good user rustup and
# trigger a wasteful reinstall.
install_rust() {
  if [ -x "$REAL_HOME/.cargo/bin/rustup" ]; then
    info "rustup present: $(as_user "$REAL_HOME/.cargo/bin/rustup" --version 2>/dev/null | head -n1)"
    as_user "$REAL_HOME/.cargo/bin/rustup" update stable
  elif have rustup; then
    info "rustup on PATH (non-standard location): $(rustup --version 2>/dev/null | head -n1)"
    as_user rustup update stable
  elif have rustc || have cargo; then
    info "non-rustup install detected: $(rustc --version 2>/dev/null || cargo --version)"
    info "skipping rustup — manage rust via your package manager"
  else
    # rustup-init from stdin: `sh -s -- ARGS` runs the piped script with ARGS as
    # its positional args. --no-modify-path: we wire the env ourselves in
    # ensure_shell_env so the marker lands alongside the others. as_user so
    # rustup lands in REAL_USER's $HOME, not /var/root, when we were sudo'd in.
    as_user sh -c 'curl --proto "=https" --tlsv1.2 -fsSL https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain stable --no-modify-path'
    PATH="$REAL_HOME/.cargo/bin:$PATH"
  fi
}

# nightly is required for `cargo +nightly -Zscript install.rs`. `rustup toolchain
# install` is a no-op when nightly is already present.
ensure_nightly() {
  if [ -x "$REAL_HOME/.cargo/bin/rustup" ]; then
    as_user "$REAL_HOME/.cargo/bin/rustup" toolchain install nightly
  elif have rustup; then
    as_user rustup toolchain install nightly
  else
    warn "no rustup — cannot ensure the nightly toolchain install.rs needs"
  fi
}

# --- shell env -------------------------------------------------------------

# Append `line` to `rc` iff a unique `marker` comment isn't already there. File
# and any new content are created as REAL_USER (via as_user) so root never ends
# up owning dotfiles under the user's home.
ensure_in_rc() {
  rc=$1
  marker=$2
  line=$3
  as_user touch "$rc"
  if as_user grep -qF "$marker" "$rc" 2>/dev/null; then
    info "$rc already has $marker — skipping"
    return
  fi
  # Heredoc-via-sh to avoid quoting headaches with $HOME inside `line` (we want
  # the literal `$HOME` written to the rc, not expanded now).
  as_user sh -c "cat >> \"\$1\" <<'EOF'

# $marker
$line
EOF" sh "$rc"
  info "wrote $marker to $rc"
}

# Wire cargo/rustup onto PATH for future zsh and bash sessions by sourcing
# rustup's env file from .zshenv (zsh) and .bashrc (bash). The marker matches
# install.rs's, so running both never duplicates the line. On macOS, also write
# `brew shellenv` to .zprofile / .bash_profile so future login shells find
# /opt/homebrew/bin without this repo's .zshrc needing to be sourced yet —
# stage 2 (install.rs) runs before that.
ensure_shell_env() {
  ensure_in_rc "$REAL_HOME/.zshenv" "grantklassy/bootstrap: cargo env" \
    '[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"'
  ensure_in_rc "$REAL_HOME/.bashrc" "grantklassy/bootstrap: cargo env" \
    '[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"'

  if [ "$OS" = mac ]; then
    brew_path=$(brew_bin)
    ensure_in_rc "$REAL_HOME/.zprofile" "grantklassy/bootstrap: brew shellenv" \
      "eval \"\$($brew_path shellenv)\""
    ensure_in_rc "$REAL_HOME/.bash_profile" "grantklassy/bootstrap: brew shellenv" \
      "eval \"\$($brew_path shellenv)\""
  fi
}

# Locate the brew binary at its standard install paths. We never rely on PATH
# for this — sudo's secure_path doesn't include /opt/homebrew/bin, so `as_user
# brew` would fail even when brew is clearly installed.
brew_bin() {
  if [ -x /opt/homebrew/bin/brew ]; then
    printf '%s\n' /opt/homebrew/bin/brew
  elif [ -x /usr/local/bin/brew ]; then
    printf '%s\n' /usr/local/bin/brew
  else
    die "brew not found at /opt/homebrew/bin/brew or /usr/local/bin/brew"
  fi
}

# NONINTERACTIVE=1 skips Homebrew's Enter-to-continue prompt so the script stays
# pipe-to-sh friendly. `as_user` because brew's installer aborts hard if run as
# root.
ensure_brew() {
  if [ -x /opt/homebrew/bin/brew ] || [ -x /usr/local/bin/brew ]; then return; fi
  as_user env NONINTERACTIVE=1 /bin/bash -c \
    "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
}

# --- clone -----------------------------------------------------------------

# Clone the public grantklassy repo *in place* into $HOME so stage 2's
# ~/.grantklassy/install.rs exists. $HOME can't be `git clone`d into (it's
# non-empty), so we init + fetch + checkout — the automated form of the manual
# steps the README used to list. All as the real user, so the working tree is
# user-owned. Idempotent: a $HOME that already tracks this repo just fetches.
clone_grantklassy() {
  home=$REAL_HOME
  if [ ! -d "$home/.git" ]; then
    as_user git -C "$home" init -b main
  fi
  # Point origin at REPO_URL whether or not a remote already exists.
  if as_user git -C "$home" remote get-url origin >/dev/null 2>&1; then
    as_user git -C "$home" remote set-url origin "$REPO_URL"
  else
    as_user git -C "$home" remote add origin "$REPO_URL"
  fi
  as_user git -C "$home" fetch origin main
  if as_user git -C "$home" rev-parse --verify main >/dev/null 2>&1; then
    info "\$HOME already tracks grantklassy — fetched latest (left your tree alone)"
    return
  fi
  # Fresh checkout. No -f: we never clobber a file the user already has. If a
  # tracked file collides with one already present, git aborts and we explain
  # the manual fix rather than overwrite.
  if ! as_user git -C "$home" checkout -t origin/main; then
    warn "could not check out origin/main into $home"
    warn "a tracked file collides with one already there; resolve it, then re-run, or:"
    die  "  cd $home && git checkout -t origin/main"
  fi
  info "cloned grantklassy into $home"
}

# Run main unless we're being sourced as a library by the test harness, which
# sets GK_BOOTSTRAP_LIB=1 to reach the pure helpers above without executing any
# of the install steps.
[ "${GK_BOOTSTRAP_LIB:-0}" = 1 ] || main "$@"
