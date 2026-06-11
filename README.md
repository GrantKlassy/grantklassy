# grantklassy

My dotfiles + machine setup, checked out straight into `$HOME`. 

### 1. Bootstrap

```sh
curl -fsSL https://raw.githubusercontent.com/GrantKlassy/grantklassy/main/bootstrap.sh | sudo sh
```

* Installs git, gh, rust (stable + nightly), and a baseline CLI kit.
* Clones this (public) repo into `$HOME`.
* POSIX sh, so it runs the same on macOS, Fedora, Debian/Ubuntu, Arch, openSUSE, Alpine, etc.
* Sudo-aware: every file under `$HOME` stays user-owned.

### 2. Install user-space tools

```sh
~/.grantklassy/install.rs
```
and
```sh
exec $SHELL -l
```
to pick up the new environment in your current window

### Tests

```sh
~/.grantklassy/test.sh               # unit suites: shell (bash+zsh), bootstrap (sh+bash), rust
~/.grantklassy/test/docker/run.sh    # end-to-end on Fedora, Ubuntu, Debian, Arch, openSUSE, Alpine
```

CI (`.github/workflows/test.yml`) runs both on every push. Internals are
documented in [`.grantklassy/README.md`](.grantklassy/README.md).
