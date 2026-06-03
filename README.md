# grantklassy

My dotfiles + machine setup, checked out straight into `$HOME`. Two commands on
a fresh macOS or Linux box.

### 1. Bootstrap — prerequisites + clone this repo into `$HOME`

```sh
curl -fsSL https://raw.githubusercontent.com/GrantKlassy/grantklassy/main/bootstrap.sh | sudo sh
```

Installs git, gh, rust (stable + nightly), and a baseline CLI kit, then clones
this (public) repo into `$HOME`. POSIX sh, so it runs the same on macOS, Fedora,
Debian/Ubuntu, Arch, openSUSE, and Alpine. Sudo-aware: every file under `$HOME`
stays user-owned. (Re-runnable later as `sudo ~/bootstrap.sh`.)

### 2. Install user-space tools (Claude Code, shell rc plumbing)

```sh
~/.grantklassy/install.rs
```

> Run as yourself — never `sudo`. It writes only under `$HOME`, and those files
> must stay user-owned; `install.rs` refuses to run as root.

Then `exec $SHELL -l` to pick up the new environment in your current window.
