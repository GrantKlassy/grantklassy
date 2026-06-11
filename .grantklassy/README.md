# `.grantklassy/` — how this repo works

The `grantklassy` repo is a **dotfiles + machine-setup repo that is checked out
directly into `$HOME`** (`origin = github.com/GrantKlassy/grantklassy.git`). The
top-level `~/README.md` is the install guide; this file documents the internals
under `~/.grantklassy/`.

## `$HOME` is a git repo

`~/.gitignore` ignores **everything** (`*`) and then re-includes only the paths
worth tracking with `!` rules — so `$HOME` can be a working tree without git
trying to track all of your home directory. Anything under `~/.grantklassy/` is
whitelisted (`!.grantklassy/**`), so new tools/files dropped here are tracked
automatically.

## Setup flow (two stages)

1. **`bootstrap.sh`** (repo root, POSIX sh) — stage 1, run via
   `curl … /bootstrap.sh | sudo sh`. Installs OS-level prerequisites across
   macOS/Fedora/Debian/Ubuntu/Arch/openSUSE/Alpine: `git`, `gh`, **rustup with
   stable + nightly** (nightly is required by `install.rs`), plus the usual CLI
   kit; wires cargo/brew onto PATH; then clones this **public** repo into
   `$HOME` (no `gh auth login` — it's public), so stage 2 is on disk. Sudo-aware;
   refuses *bare* root (uid 0 with no `SUDO_USER`). Steps are skippable via
   **`GK_BOOTSTRAP_SKIP`** (`packages,rust,shell-env,clone`) — the container
   tests use it to run the offline shell-env wiring without the network steps.
2. **`~/.grantklassy/install.rs`** — stage 2, userspace setup, run **as you,
   never root**.

`bootstrap.sh` used to be a separate repo (`GrantKlassy/bootstrap`, `install.sh`);
it now lives here so the whole setup is one public repo. POSIX sh — not Python or
anything else — precisely so stage 1 needs no language runtime a fresh box might
lack; a `/bin/sh` is always there.

## The `.grantklassy/` tree

```
install.rs        # userspace installer (stage 2) — single-file cargo script (nightly -Zscript)
util/log.rs       # shared logging macros (info!/warn!/error!/ok!/step!)
macos/gkdisk.rs   # interactive exFAT formatter for external disks (macOS, drives diskutil)
shell/            # the interactive shell config, split for testability
  lib.sh          # SHARED, side-effect-free helpers (sourced by bash AND zsh)
  prompt.bash     # bash prompt pieces (\[ \]-wrapped colours)
  prompt.zsh      # zsh prompt pieces (%F{n} colours)
  tests/          # pure-shell test harness — runs every test under both shells
test/
  bootstrap/      # unit tests for ../../bootstrap.sh (run under sh + bash)
  docker/         # end-to-end install.rs + bootstrap.sh tests across distros
test.sh           # one command: shell tests + bootstrap tests + all rust unit tests
```

Stage 1, **`bootstrap.sh`**, is the one setup file that lives at the *repo root*
(`~/bootstrap.sh`), not under `.grantklassy/` — it has to be `curl`-able and
runnable before `.grantklassy/` exists on disk.

`install.rs` / `gkdisk.rs` are **POSIX-sh/Rust polyglots**: the shebang block
`exec cargo +nightly -Zscript "$0"` lets `./install.rs` run on any POSIX system
(needs a nightly toolchain), while cargo compiles the rest. `install.rs` steps:
ensure nightly → idempotently wire shell startup files → install Claude Code →
Karabiner (macOS only) → clone the user's public repos into `~/git/grantklassy/`.
Steps can be skipped with **`GK_INSTALL_SKIP`** (comma/space list:
`nightly,shell-env,claude,karabiner,clone`) — used by the container tests to run
the offline shell-env wiring without the network steps.

## Shell loading chain

The interactive config is hand-maintained in two mirrored entrypoints — bash
`~/.bashrc.d/grantklassy.sh` and zsh `~/.zshrc` — both thin: they keep their
non-interactive `return` guard, source `shell/lib.sh` + their prompt shim, then
do shell-specific side effects (history, PATH, completions, macOS tweaks,
aliases, prompt). The testable logic lives in `lib.sh`.

| file | shell | who writes it | sourced when |
|------|-------|---------------|--------------|
| `.zshenv` | zsh | bootstrap/install.rs | always (incl. non-interactive) |
| `.zprofile` | zsh | bootstrap | login shells (brew shellenv) |
| `.zshrc` | zsh | **tracked** | interactive |
| `.profile` | sh | bootstrap/install.rs | POSIX `sh` login |
| `.bash_profile` | bash | install.rs | login → sources `.bashrc` |
| `.bashrc` | bash | install.rs (generated) | sources `.bashrc.d/*.sh` |
| `.bashrc.d/grantklassy.sh` | bash | **tracked** | via the `.bashrc` loop |

Only `.zshrc` and `.bashrc.d/grantklassy.sh` are tracked; the rest are generated
with idempotent `# marker` comments so re-running bootstrap/install.rs never
duplicates a line.

## Running the tests

```sh
~/.grantklassy/test.sh                              # everything
sh ~/.grantklassy/shell/tests/run.sh               # shell helpers, bash + zsh
sh ~/.grantklassy/test/bootstrap/run.sh            # bootstrap.sh helpers, sh + bash
cargo +nightly -Zscript test --manifest-path ~/.grantklassy/install.rs
cargo +nightly -Zscript test --manifest-path ~/.grantklassy/macos/gkdisk.rs
~/.grantklassy/test/docker/run.sh                  # install.rs + bootstrap.sh, all distros (below)
```

The shell harness runs each `shell/tests/test_*.sh` under **both** bash and zsh;
that is what enforces the cross-shell invariants the two configs must share
(e.g. `host_color` picking the same colour despite zsh's 1-indexed vs bash's
0-indexed arrays).

The Docker matrix covers one distro per package-manager family `bootstrap.sh`
supports — **Fedora** (dnf), **Ubuntu + Debian** (apt), **Arch** (pacman),
**openSUSE Tumbleweed** (zypper), and **Alpine** (apk; also the busybox-`sh` +
musl torture test). `.github/workflows/test.yml` runs the unit suites on
Ubuntu + macOS runners and the full Docker matrix on every push, so the
cross-distro claims stay continuously proven rather than aspirational.
