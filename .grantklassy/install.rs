#!/bin/sh
#![allow(unused)] /*
exec cargo +nightly -Zscript "$0" "$@"
*/
// ^ POSIX-sh/Rust polyglot launcher: `./install.rs` runs on any POSIX system
// because /bin/sh reaches the `exec` line and re-execs us under nightly cargo,
// while cargo strips the shebang and compiles the rest (the `#![allow]` is a
// harmless inner attribute; the exec line lives inside the block comment).
// This avoids `env -S` (GNU coreutils 8.30+ / 2018-only), so the launcher is
// portable to any POSIX sh; the only hard requirement is a nightly toolchain.
//
// Single-file cargo script for user-space setup (nightly-only — needs
// `-Zscript`). Add deps later via a `---` frontmatter block above `fn main`.
//
// Prerequisites (git, gh, rust stable + nightly) come from stage 1 — bootstrap.sh
// at the repo root, which also clones this repo into $HOME:
//   curl -fsSL https://raw.githubusercontent.com/GrantKlassy/grantklassy/main/bootstrap.sh | sudo sh
//
// This script runs as the user, never as root — `refuse_root` enforces it
// (writing $HOME files as root is exactly what breaks the next unprivileged
// run). No `dnf`/`sudo` calls live here; it writes only under $HOME.
//
// Run unit tests with:
//   cargo +nightly -Zscript test --manifest-path install.rs
//
// Steps can be skipped with GK_INSTALL_SKIP — a comma/space-separated list of
// step names (nightly, shell-env, claude, karabiner, clone). The container
// tests use it to run the offline shell-env wiring without the network steps.

#[path = "util/log.rs"]
mod log;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, exit};

fn run(cmd: &str, args: &[&str]) {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .unwrap_or_else(|e| { error!("spawn {cmd}: {e}"); exit(1) });
    if !status.success() {
        error!("{cmd} {args:?} exited {status}");
        exit(1);
    }
}

/// How many times a flaky network step is attempted before giving up. The
/// claude download in particular hits transient TLS/socket errors —
/// `curl: (56) ... bad decrypt`, `socket connection was closed unexpectedly` —
/// that a re-attempt a few seconds later usually clears.
const NETWORK_ATTEMPTS: u32 = 3;

/// Seconds to wait after the `attempt`-th (1-based) failure before retrying:
/// exponential backoff (2, 4, 8, …) capped at 30s so a sustained outage can't
/// stall the install for minutes. `saturating_pow` keeps a large `attempt`
/// from overflowing the shift. Pure, so it's unit-testable.
fn backoff_secs(attempt: u32) -> u64 {
    2u64.saturating_pow(attempt).min(30)
}

/// Like `run`, but returns whether the command succeeded instead of aborting on
/// failure, so callers can retry or warn-and-continue. Stdio is inherited (not
/// captured), so the subprocess's own errors still reach the user. Only a spawn
/// failure warns here; a non-zero exit is left for the caller to narrate.
fn try_run(cmd: &str, args: &[&str]) -> bool {
    match Command::new(cmd).args(args).status() {
        Ok(status) => status.success(),
        Err(e) => { warn!("spawn {cmd}: {e}"); false }
    }
}

/// Run a flaky network command, retrying with exponential backoff. Returns true
/// as soon as an attempt succeeds, false once all NETWORK_ATTEMPTS have failed.
/// Unlike `run`, never exits — a download that stays broken shouldn't sink the
/// whole install (the steps after it are independent), so the caller decides.
fn run_with_retries(cmd: &str, args: &[&str]) -> bool {
    for attempt in 1..=NETWORK_ATTEMPTS {
        if try_run(cmd, args) {
            return true;
        }
        if attempt < NETWORK_ATTEMPTS {
            let secs = backoff_secs(attempt);
            warn!("attempt {attempt}/{NETWORK_ATTEMPTS} failed — retrying in {secs}s");
            std::thread::sleep(std::time::Duration::from_secs(secs));
        } else {
            warn!("attempt {attempt}/{NETWORK_ATTEMPTS} failed");
        }
    }
    false
}

/// install.rs is a *userspace* script: every path it writes lives under
/// $HOME and must end up owned by the user. Run as root (the `sudo
/// install.rs` reflex), the claude installer stages its download into
/// root-owned ~/.claude/downloads & ~/.local/share/claude — and the next
/// *unprivileged* run then can't write there at all (curl: (56) Failure
/// writing output to destination). So hard-refuse uid 0 up front:
/// userspace stays userspace, root stays root.
fn refuse_root() {
    // geteuid(2) governs ownership of whatever we create, so check it
    // directly rather than trusting $USER/$SUDO_USER (spoofable). libc is
    // always linked into Rust unix binaries, so this FFI needs no dep.
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    if unsafe { geteuid() } != 0 {
        return;
    }
    error!("refusing to run as root — install.rs writes only under $HOME,");
    error!("and those files must be owned by you, not root. Re-run without sudo.");
    error!("if an earlier root run already left root-owned files, reclaim them:");
    error!("  sudo chown -R \"$USER\" ~/.claude ~/.local/share/claude ~/.local/state/claude ~/.local/bin");
    exit(1);
}

/// Parse a GK_INSTALL_SKIP value into a normalized list of step names. Accepts
/// comma- or whitespace-separated names; trims, lowercases, and drops empties.
/// Pure (takes the raw string rather than reading the env) so it's unit-testable.
fn parse_skip(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Whether `step` appears in a parsed skip list.
fn is_skipped(skips: &[String], step: &str) -> bool {
    skips.iter().any(|s| s == step)
}

/// True if `step` should run; logs a note and returns false when it has been
/// skipped via GK_INSTALL_SKIP. Thin wrapper over is_skipped so each step in
/// main() reads as `if run_step(&skips, "name") { … }`.
fn run_step(skips: &[String], step: &str) -> bool {
    if is_skipped(skips, step) {
        info!("skipping `{step}` (GK_INSTALL_SKIP)");
        false
    } else {
        true
    }
}

fn main() {
    // Userspace-only. Refuse uid 0 before doing anything else: a single
    // `sudo install.rs` is what root-owned ~/.claude/downloads &
    // ~/.local/share/claude in the first place (see refuse_root).
    refuse_root();

    // Pick up PATH additions from .profile/.cargo/env that the shell which
    // spawned us didn't see. Called after every step too, so each install
    // pulls in the PATH the previous one wrote — the in-process equivalent
    // of the `reload` shell alias we tell the user to run at the end.
    refresh_env();

    // Steps the user opted to skip via GK_INSTALL_SKIP (comma/space list).
    // Lets container tests and re-runs exercise the offline steps (the
    // shell-env wiring) without the network ones (claude download, gh clone).
    let skips = parse_skip(&std::env::var("GK_INSTALL_SKIP").unwrap_or_default());

    // Defensive: bootstrap should already have installed nightly (we're
    // running under it right now via -Zscript), but a user who installed
    // rust some other way may not have it. `rustup toolchain install` is
    // a no-op when nightly is already present.
    step!("nightly toolchain");
    if run_step(&skips, "nightly") {
        run("rustup", &["toolchain", "install", "nightly"]);
    }
    refresh_env();

    // Defensive: bootstrap writes these too. Re-asserting here lets
    // install.rs recover a system where bootstrap was skipped or the
    // shell rc was clobbered, without breaking idempotency (marker
    // matches bootstrap's so we won't double-write).
    step!("shell env");
    if run_step(&skips, "shell-env") {
        ensure_shell_env();
    }
    refresh_env();

    step!("claude code");
    if run_step(&skips, "claude") {
        // claude's install.sh is `#!/bin/bash` and uses bashisms ([[ ]], =~,
        // ${BASH_REMATCH}), so it must be piped to `bash`, not `sh` — piping to
        // `sh` would break it wherever /bin/sh isn't bash (e.g. Linux/dash). The
        // outer `sh -c` only sets up the pipe; curl + bash are the real prereqs.
        //
        // The download is prone to transient TLS/socket failures (the very
        // `curl: (56) ... bad decrypt` / `socket connection was closed` errors
        // that motivated this), so retry with backoff. If every attempt fails,
        // warn and continue rather than aborting: like karabiner/clone below, a
        // flaky download shouldn't block the rest of setup, and the whole script
        // is idempotent so a later re-run picks up where this left off.
        if !run_with_retries("sh", &["-c", "curl -fsSL https://claude.ai/install.sh | bash"]) {
            warn!("claude code install failed after {NETWORK_ATTEMPTS} attempts —");
            warn!("re-run `./install.rs` once your connection is stable (it's idempotent)");
        }
    }
    refresh_env();

    // Karabiner-Elements (macOS keyboard customizer). No-ops off macOS, so it
    // never aborts the install on a POSIX box — see install_karabiner.
    step!("karabiner-elements");
    if run_step(&skips, "karabiner") {
        install_karabiner();
    }

    // Clone the user's public repos into ~/git/grantklassy. Runs after the
    // tool installs above so `gh` (a bootstrap prerequisite) is on PATH;
    // non-fatal, so a clone hiccup never blocks finishing setup.
    step!("clone repos");
    if run_step(&skips, "clone") {
        clone_repos();
    }

    step!("done");
    info!("to pick up new env in your current shell, run: `exec $SHELL -l`");
}

/// The GitHub account whose public repos `clone_repos` mirrors locally.
/// Hardcoded rather than read from `gh auth status` so the step clones the
/// same set regardless of which account happens to be logged in.
const GITHUB_USER: &str = "GrantKlassy";

/// This repo itself. clone_repos skips it: it's already checked out directly
/// into $HOME (that's the whole design), so mirroring it again under
/// ~/git/grantklassy/grantklassy would only create a second, stale copy.
const SELF_REPO: &str = "grantklassy";

/// Whether a `gh` `OWNER/REPO` name refers to this dotfiles repo. Compares
/// the repo half case-insensitively — GitHub repo names are case-insensitive
/// and `gh` echoes whatever case the server has stored.
fn is_self_repo(name_with_owner: &str) -> bool {
    repo_dir_name(name_with_owner).eq_ignore_ascii_case(SELF_REPO)
}

/// Mirror every public repo owned by GITHUB_USER into ~/git/grantklassy/,
/// one directory per repo — the idempotent form of `mkdir -p
/// ~/git/grantklassy && git clone …`. Repos are enumerated with `gh` (a
/// bootstrap prerequisite) so new repos are picked up automatically on
/// re-run and nothing is hardcoded. The dotfiles repo itself (SELF_REPO) is
/// skipped — it's already checked out into $HOME. A repo whose directory
/// already exists is left untouched: we never clobber or force-pull local
/// work. Non-fatal throughout — a failed enumerate, or any single failed
/// clone, warns and the install continues, since cloning is a convenience
/// and not a prerequisite for the steps around it.
fn clone_repos() {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        warn!("$HOME not set — skipping repo clone");
        return;
    };
    let dest_root = home.join("git").join("grantklassy");
    if let Err(e) = fs::create_dir_all(&dest_root) {
        warn!("mkdir {}: {e}", dest_root.display());
        return;
    }

    // One `OWNER/REPO` per line. `--limit 1000` lifts gh's default 30-repo
    // cap; `--json … --jq` uses gh's built-in jq, so no external jq is needed.
    let out = Command::new("gh")
        .args([
            "repo", "list", GITHUB_USER,
            "--visibility", "public",
            "--limit", "1000",
            "--json", "nameWithOwner",
            "--jq", ".[].nameWithOwner",
        ])
        .output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            warn!("`gh repo list` exited {} — skipping repo clone", o.status);
            // Surface gh's own stderr so a missing login (`gh auth login`)
            // or a network failure is diagnosable straight from the log.
            for line in String::from_utf8_lossy(&o.stderr).lines() {
                warn!("  {line}");
            }
            return;
        }
        Err(e) => { warn!("spawn gh (is it installed?): {e} — skipping repo clone"); return; }
    };

    let repos: Vec<&str> = std::str::from_utf8(&out.stdout)
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if repos.is_empty() {
        info!("no public repos found for {GITHUB_USER}");
        return;
    }
    info!("{} public repo(s) → {}", repos.len(), dest_root.display());

    for repo in repos {
        if is_self_repo(repo) {
            info!("{repo} is this dotfiles repo (checked out in $HOME) — skipping");
            continue;
        }
        let name = repo_dir_name(repo);
        let dir = dest_root.join(name);
        if dir.exists() {
            info!("{name} already present — skipping");
            continue;
        }
        info!("cloning {repo}");
        // `gh repo clone` honors the user's configured git protocol (https
        // here) and token, so it behaves exactly like a manual clone. Pass
        // the PathBuf as an OsStr arg to avoid lossy UTF-8 conversion.
        let status = Command::new("gh")
            .arg("repo").arg("clone").arg(repo).arg(&dir)
            .status();
        match status {
            Ok(s) if s.success() => ok!("cloned {name}"),
            Ok(s) => warn!("clone {repo} exited {s}"),
            Err(e) => warn!("spawn gh clone {repo}: {e}"),
        }
    }
}

/// Local directory name for a `gh` `nameWithOwner`: the repo half of
/// `OWNER/REPO`. Falls back to the whole string when there's no slash.
fn repo_dir_name(name_with_owner: &str) -> &str {
    name_with_owner.rsplit('/').next().unwrap_or(name_with_owner)
}

/// Install Karabiner-Elements — a macOS keyboard customizer — via a Homebrew
/// cask. macOS-only: `brew --cask` installs macOS GUI apps and Karabiner has
/// no Linux build, so we no-op on every other OS rather than abort the install
/// on a POSIX box (the whole point of the portable launcher up top). Non-fatal
/// throughout, like clone_repos: a missing brew or a failed download warns and
/// setup continues, since this is a convenience and not a prerequisite for the
/// steps around it. Idempotent via `brew list --cask` — re-running `brew
/// install --cask` on an already-installed cask is noisy and its exit status is
/// version-dependent, so we short-circuit instead of relying on it.
fn install_karabiner() {
    if std::env::consts::OS != "macos" {
        info!("not macOS — skipping karabiner-elements");
        return;
    }
    // brew is a macOS prerequisite this script doesn't install; without it
    // there's nothing to do but say so and move on.
    let have_brew = Command::new("brew")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_or(false, |s| s.success());
    if !have_brew {
        warn!("brew not found — skipping karabiner-elements");
        return;
    }
    // `brew list --cask <name>` exits 0 only when the cask is already present.
    let installed = Command::new("brew")
        .args(["list", "--cask", "karabiner-elements"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_or(false, |s| s.success());
    if installed {
        info!("karabiner-elements already installed — skipping");
        return;
    }
    info!("installing karabiner-elements");
    let status = Command::new("brew")
        .args(["install", "--cask", "karabiner-elements"])
        .status();
    match status {
        Ok(s) if s.success() => ok!("installed karabiner-elements"),
        Ok(s) => warn!("brew install --cask karabiner-elements exited {s}"),
        Err(e) => warn!("spawn brew: {e}"),
    }
}

/// Append `line` to `rc`, prefaced by a `# {marker}` comment, iff that
/// marker isn't already present. Creates the file if missing. Failures
/// warn rather than abort — a missing dotfile shouldn't fail the whole
/// install when the rest of the steps still succeed.
fn ensure_in_rc(rc: &Path, marker: &str, line: &str) {
    if let Ok(contents) = fs::read_to_string(rc) {
        if contents.contains(marker) {
            info!("{} already has `{marker}` — skipping", rc.display());
            return;
        }
    }
    let body = format!("\n# {marker}\n{line}\n");
    let mut f = match fs::OpenOptions::new().create(true).append(true).open(rc) {
        Ok(f) => f,
        Err(e) => { warn!("open {}: {e}", rc.display()); return; }
    };
    if let Err(e) = f.write_all(body.as_bytes()) {
        warn!("write {}: {e}", rc.display());
        return;
    }
    ok!("wrote `{marker}` to {}", rc.display());
}

/// Wire up shell startup files so:
///   - future zsh, bash, and plain POSIX `sh` sessions find cargo/rustup on
///     PATH — the PATH/cargo lines also go to `~/.profile`, the only startup
///     file a `/bin/sh` login shell reads; without it, sh users get no env
///   - interactive bash actually loads `~/.bashrc.d/*.sh` (the repo's
///     bash rc lives there but nothing sources it by default)
///   - bash *login* shells (macOS Terminal default) read `~/.bashrc`
///     via a `~/.bash_profile` shim
///
/// zsh needs none of the `.bashrc.d`/login-shim plumbing because
/// `~/.zshrc` is auto-sourced for interactive zsh and is checked into
/// the grantklassy repo.
fn ensure_shell_env() {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        warn!("$HOME not set — skipping shell env wiring");
        return;
    };

    // 1. cargo env on PATH for zsh + bash + POSIX sh. Marker matches the
    //    bootstrap script's so running both never duplicates the line.
    let cargo_marker = "grantklassy/bootstrap: cargo env";
    let cargo_line = r#"[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env""#;
    ensure_in_rc(&home.join(".profile"), cargo_marker, cargo_line);
    ensure_in_rc(&home.join(".zshenv"), cargo_marker, cargo_line);
    ensure_in_rc(&home.join(".bashrc"), cargo_marker, cargo_line);

    // 2. Make bash actually load the repo's bash rc. .bashrc.d/*.sh is
    //    a Fedora-style convention; macOS/Debian bash doesn't iterate
    //    that dir by default, so we add the loop ourselves.
    ensure_in_rc(
        &home.join(".bashrc"),
        "grantklassy/install.rs: load .bashrc.d",
        r#"for f in "$HOME"/.bashrc.d/*.sh; do [ -r "$f" ] && . "$f"; done; unset f"#,
    );

    // 3. macOS Terminal launches bash as a *login* shell, which reads
    //    .bash_profile (not .bashrc). Without this shim the .bashrc
    //    above never runs in a normal terminal window.
    ensure_in_rc(
        &home.join(".bash_profile"),
        "grantklassy/install.rs: source .bashrc",
        r#"[ -f "$HOME/.bashrc" ] && . "$HOME/.bashrc""#,
    );

    // 4. Auto-add known user bin dirs to PATH. claude installs to
    //    ~/.local/bin, rustup uses ~/.cargo/bin (already covered by
    //    .cargo/env above but cheap to re-assert), and ~/bin is a
    //    long-standing convention. Goes to .profile (POSIX sh), .zshenv
    //    (non-interactive zsh), and .bashrc (bash); the `case`-guarded
    //    prepend is idempotent so re-sourcing won't duplicate entries.
    let path_marker = "grantklassy/install.rs: PATH additions";
    let path_line = r#"for d in "$HOME/.cargo/bin" "$HOME/.local/bin" "$HOME/bin"; do case ":$PATH:" in *":$d:"*) ;; *) [ -d "$d" ] && PATH="$d:$PATH" ;; esac; done; export PATH"#;
    ensure_in_rc(&home.join(".profile"), path_marker, path_line);
    ensure_in_rc(&home.join(".zshenv"), path_marker, path_line);
    ensure_in_rc(&home.join(".bashrc"), path_marker, path_line);
}

/// Re-source ~/.profile and ~/.cargo/env into install.rs's own env so any
/// subprocess we spawn afterwards sees PATH additions written by earlier
/// install steps. install.rs starts in the shell subprocess that spawned it,
/// created before any of our installs ran; without this, freshly installed
/// binaries (cargo from rustup, claude) wouldn't be on PATH for the next
/// step.
fn refresh_env() {
    // Build the dump script from ENV_PASSTHROUGH so the key list has a single
    // source of truth, then emit `KEY=VALUE\0` for each key. A per-key
    // `printf '%s\0'` loop is portable POSIX sh, whereas `env -0` is a GNU
    // coreutils extension absent on BSD/macOS; `eval "v=\${$k}"` is POSIX
    // indirect expansion.
    let keys = ENV_PASSTHROUGH.join(" ");
    let script = format!(
        // `set -a` exports assignments made while sourcing, so PATH/etc. set
        // in .profile/.cargo/env become real env vars this `sh` can read back.
        // We source ~/.profile (the file POSIX login shells read, and where
        // ensure_shell_env writes our PATH lines) plus ~/.cargo/env. Sourcing
        // failures are non-fatal — fresh systems may lack these on first run.
        r#"set -a; . "$HOME/.profile" 2>/dev/null; . "$HOME/.cargo/env" 2>/dev/null; set +a; for k in {keys}; do eval "v=\${{$k}}"; printf '%s=%s\0' "$k" "$v"; done"#
    );
    let out = Command::new("sh").args(["-c", &script]).output();
    let Ok(o) = out else { return };
    if !o.status.success() { return }
    for (k, v) in parse_env0(&o.stdout, ENV_PASSTHROUGH) {
        // SAFETY: install.rs is single-threaded.
        unsafe { std::env::set_var(&k, &v); }
    }
}

/// Keys we let `refresh_env` overwrite in our own env after sourcing
/// .profile/.cargo/env. PATH is the load-bearing one (newly installed
/// binaries); the others come along for compatibility with shells that
/// expect them.
const ENV_PASSTHROUGH: &[&str] = &[
    "PATH",
    "MANPATH",
    "INFOPATH",
    "PKG_CONFIG_PATH",
    "LD_LIBRARY_PATH",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "EDITOR",
];

/// Parse null-separated `KEY=VALUE` pairs from `env -0` output, keeping
/// only `wanted` keys with non-empty values.
fn parse_env0(stdout: &[u8], wanted: &[&str]) -> Vec<(String, String)> {
    let mut env = Vec::new();
    for chunk in stdout.split(|b| *b == 0) {
        if chunk.is_empty() { continue }
        let Ok(s) = std::str::from_utf8(chunk) else { continue };
        let Some((k, v)) = s.split_once('=') else { continue };
        if wanted.contains(&k) && !v.is_empty() {
            env.push((k.to_string(), v.to_string()));
        }
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env0_filters_to_wanted_keys() {
        let input = b"DISPLAY=:0\0WAYLAND_DISPLAY=wayland-0\0HOME=/home/x\0XAUTHORITY=/run/x\0";
        let got = parse_env0(input, &["DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY"]);
        assert_eq!(got.len(), 3);
        assert!(got.contains(&("DISPLAY".into(), ":0".into())));
        assert!(got.contains(&("WAYLAND_DISPLAY".into(), "wayland-0".into())));
        assert!(got.contains(&("XAUTHORITY".into(), "/run/x".into())));
    }

    #[test]
    fn parse_env0_skips_unwanted_keys() {
        let input = b"DISPLAY=:0\0HOME=/home/x\0PATH=/usr/bin\0";
        let got = parse_env0(input, &["DISPLAY"]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "DISPLAY");
    }

    #[test]
    fn parse_env0_skips_empty_values() {
        // An empty PATH from a partial sourcing would silently break later
        // steps that need it; skip empties rather than overwrite real env.
        let input = b"PATH=\0EDITOR=vim\0";
        let got = parse_env0(input, &["PATH", "EDITOR"]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "EDITOR");
    }

    #[test]
    fn parse_env0_preserves_equals_in_values() {
        let input = b"DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus\0";
        let got = parse_env0(input, &["DBUS_SESSION_BUS_ADDRESS"]);
        assert_eq!(got[0].1, "unix:path=/run/user/1000/bus");
    }

    #[test]
    fn parse_env0_handles_empty_chunks() {
        // Our `printf '%s=%s\0'` dump has no trailing double-null, but we
        // tolerate either form to stay defensive.
        let input = b"\0\0DISPLAY=:0\0\0";
        let got = parse_env0(input, &["DISPLAY"]);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn parse_env0_skips_chunks_without_equals() {
        let input = b"NOEQUALS\0DISPLAY=:0\0";
        let got = parse_env0(input, &["DISPLAY", "NOEQUALS"]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "DISPLAY");
    }

    #[test]
    fn parse_env0_handles_empty_input() {
        assert!(parse_env0(b"", &["DISPLAY"]).is_empty());
    }

    #[test]
    fn parse_env0_skips_invalid_utf8() {
        let mut input: Vec<u8> = b"DISPLAY=".to_vec();
        input.push(0xFF); // lone continuation byte
        input.push(0);
        input.extend_from_slice(b"XAUTHORITY=/run/x\0");
        let got = parse_env0(&input, &["DISPLAY", "XAUTHORITY"]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "XAUTHORITY");
    }

    #[test]
    fn repo_dir_name_strips_owner() {
        assert_eq!(repo_dir_name("GrantKlassy/facecam"), "facecam");
    }

    #[test]
    fn repo_dir_name_without_slash() {
        // A bare repo name (no owner) should pass through unchanged.
        assert_eq!(repo_dir_name("facecam"), "facecam");
    }

    #[test]
    fn repo_dir_name_keeps_last_segment() {
        // rsplit keeps us correct even if extra path segments ever appear.
        assert_eq!(repo_dir_name("a/b/c"), "c");
    }

    #[test]
    fn is_self_repo_matches_this_repo_case_insensitively() {
        // The canonical name gh returns today, plus case variants — GitHub
        // treats repo names case-insensitively, so we must too.
        assert!(is_self_repo("GrantKlassy/grantklassy"));
        assert!(is_self_repo("grantklassy/grantklassy"));
        assert!(is_self_repo("GrantKlassy/GRANTKLASSY"));
        // A bare name (no owner half) still matches.
        assert!(is_self_repo("grantklassy"));
    }

    #[test]
    fn is_self_repo_rejects_other_repos() {
        assert!(!is_self_repo("GrantKlassy/facecam"));
        // Prefix/suffix near-misses are different repos, not this one.
        assert!(!is_self_repo("GrantKlassy/grantklassy-fork"));
        assert!(!is_self_repo("GrantKlassy/old-grantklassy"));
    }

    #[test]
    fn parse_skip_splits_trims_and_lowercases() {
        assert_eq!(parse_skip("claude, clone"), vec!["claude", "clone"]);
        // whitespace is also a separator, and names are case-folded
        assert_eq!(parse_skip("Claude\tCLONE"), vec!["claude", "clone"]);
        // empty fields (doubled separators) are dropped
        assert_eq!(parse_skip("nightly,, ,claude"), vec!["nightly", "claude"]);
    }

    #[test]
    fn parse_skip_empty_inputs_yield_nothing() {
        assert!(parse_skip("").is_empty());
        assert!(parse_skip("   ,  ").is_empty());
    }

    #[test]
    fn is_skipped_checks_membership() {
        let skips = parse_skip("claude, clone");
        assert!(is_skipped(&skips, "claude"));
        assert!(is_skipped(&skips, "clone"));
        assert!(!is_skipped(&skips, "nightly"));
        assert!(!is_skipped(&[], "claude"));
    }

    #[test]
    fn backoff_secs_is_exponential_and_capped() {
        assert_eq!(backoff_secs(1), 2);
        assert_eq!(backoff_secs(2), 4);
        assert_eq!(backoff_secs(3), 8);
        // The 30s cap holds and the shift never overflows for large attempts.
        assert_eq!(backoff_secs(10), 30);
        assert_eq!(backoff_secs(64), 30);
    }

}
