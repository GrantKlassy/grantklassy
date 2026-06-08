# grantklassy

My dotfiles + machine setup, checked out straight into `$HOME`. 

### 1. Bootstrap

```sh
curl -fsSL https://raw.githubusercontent.com/GrantKlassy/grantklassy/main/bootstrap.sh | sudo sh
```

Installs git, gh, rust (stable + nightly), and a baseline CLI kit.
Clones this (public) repo into `$HOME`.
POSIX sh, so it runs the same on macOS, Fedora, Debian/Ubuntu, Arch, openSUSE, etc.
Sudo-aware: every file under `$HOME` stays user-owned.

### 2. Install user-space tools

```sh
~/.grantklassy/install.rs
```

Then `exec $SHELL -l` to pick up the new environment in your current window.
