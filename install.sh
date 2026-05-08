#!/usr/bin/env bash
# Bootstrap: dnf installs (run as root), then hand off to install.rs as the
# original user for everything user-space. Run as: sudo ./install.sh
set -euo pipefail

if [ "$EUID" -ne 0 ]; then
  echo "install.sh must be run as root: sudo $0" >&2
  exit 1
fi
if [ -z "${SUDO_USER:-}" ] || [ "$SUDO_USER" = "root" ]; then
  echo "install.sh must be invoked via sudo from a non-root user" >&2
  exit 1
fi

USER_HOME=$(getent passwd "$SUDO_USER" | cut -d: -f6)

REPO_URL="https://github.com/GrantKlassy/grantklassy.git"
BRANCH="main"

# --- dnf installs (system-wide; idempotent) ---

dnf install -y \
  vim-enhanced \
  gcc \
  rust \
  snapd \
  https://dl.google.com/linux/direct/google-chrome-stable_current_x86_64.rpm

# snapd: socket + classic /snap symlink.
systemctl enable --now snapd.socket
[ -e /snap ] || ln -s /var/lib/snapd/snap /snap

# --- user-space prerequisites for install.rs ---

# Rustup nightly — install.rs is a cargo-script that needs `-Zscript`.
# Not a dnf install (Fedora's `rust` is stable only), so it lives here as a
# bootstrap step rather than in install.rs (which itself requires cargo).
sudo -u "$SUDO_USER" -H bash -c '
  set -euo pipefail
  if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain nightly
  fi
'

# Pull dotfiles into $HOME on a fresh user (idempotent: skipped if .git exists).
sudo -u "$SUDO_USER" -H bash -s "$REPO_URL" "$BRANCH" <<'EOF'
set -euo pipefail
url=$1
branch=$2
cd "$HOME"
if [ ! -d "$HOME/.git" ]; then
  git init -b "$branch"
  git remote add origin "$url"
  git fetch origin
  # -f overwrites pre-existing dotfiles in $HOME with the tracked versions.
  git checkout -f -t "origin/$branch"
fi
EOF

# --- hand off to install.rs as the user ---

chmod +x "$USER_HOME/.grantklassy/install.rs"
exec sudo -u "$SUDO_USER" -H bash -c '
  set -euo pipefail
  . "$HOME/.cargo/env"
  exec "$HOME/.grantklassy/install.rs"
'
