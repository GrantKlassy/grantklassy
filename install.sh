#!/usr/bin/env bash
# Bootstrap: install git + rust, clone repo into $HOME, hand off to install.rs.
set -euo pipefail

REPO_URL="https://github.com/GrantKlassy/grantklassy.git"
BRANCH="main"

sudo dnf install -y git

if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
. "$HOME/.cargo/env"

cd "$HOME"
if [ ! -d "$HOME/.git" ]; then
  git init -b "$BRANCH"
  git remote add origin "$REPO_URL"
  git fetch origin
  # -f overwrites any pre-existing dotfiles in $HOME with tracked versions.
  git checkout -f -t "origin/$BRANCH"
fi

chmod +x "$HOME/install.rs"
exec "$HOME/install.rs"
