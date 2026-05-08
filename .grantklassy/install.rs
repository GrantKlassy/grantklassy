#!/usr/bin/env -S cargo +nightly -Zscript
// Single-file cargo script (still nightly-only — requires `-Zscript`).
// Add deps later via a `---` frontmatter block above `fn main`.
//
// install.sh runs as root via sudo, does all the dnf installs, then exec's
// this script as the original user. So:
//   - this script must run as the user, never as root
//   - no `dnf` or `sudo` calls live here — system packages belong in install.sh
//   - everything below should be user-space (writes under $HOME, etc.)
//
// Run unit tests with:
//   cargo +nightly -Zscript test --manifest-path install.rs

#[path = "util/log.rs"]
mod log;

use std::path::Path;
use std::process::{Command, Stdio, exit};
use std::thread;
use std::time::Duration;

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

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| { error!("HOME not set"); exit(1) })
}

fn main() {
    step!("claude code");
    run("sh", &["-c", "curl -fsSL https://claude.ai/install.sh | bash"]);

    step!("facecam");
    let dir = format!("{}/facecam", home());
    if Path::new(&dir).join(".git").exists() {
        info!("facecam already cloned at {dir}");
    } else {
        run("git", &["clone", "https://github.com/GrantKlassy/facecam.git", &dir]);
    }
    run("cargo", &["build", "--release", "--manifest-path", &format!("{dir}/Cargo.toml")]);

    launch_facecam(&format!("{dir}/target/release/facecam"));
}

/// Recover the live session env (DISPLAY, WAYLAND_DISPLAY, XAUTHORITY, …),
/// pick the user's currently selected audio output as the visualizer source,
/// then spawn facecam detached and verify it actually stayed up.
fn launch_facecam(bin: &str) {
    let session = read_session_env();
    if session.iter().all(|(k, _)| k != "WAYLAND_DISPLAY" && k != "DISPLAY") {
        warn!("no DISPLAY/WAYLAND_DISPLAY in session env; window may fail to open");
    }

    let device = default_sink_monitor(&session);
    match &device {
        Some(d) => info!("FACECAM_DEVICE={d}"),
        None => warn!("no default sink from pactl; falling back to facecam's first-input default"),
    }

    let mut cmd = Command::new("setsid");
    cmd.arg(bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (k, v) in &session {
        cmd.env(k, v);
    }
    if let Some(d) = device {
        cmd.env("FACECAM_DEVICE", d);
    }

    let mut child = cmd.spawn()
        .unwrap_or_else(|e| { error!("spawn facecam: {e}"); exit(1) });

    // spawn() returns Ok the instant fork+exec succeeds — even if facecam
    // dies a millisecond later because there's no display or audio source.
    // Wait briefly and confirm the process is still alive before claiming
    // success; otherwise we'd silently lie about a launched window.
    thread::sleep(Duration::from_millis(800));
    match child.try_wait() {
        Ok(None) => ok!("facecam launched in background (pid {})", child.id()),
        Ok(Some(status)) => {
            error!("facecam exited immediately ({status})");
            error!("rerun directly to see stderr: {bin}");
            exit(1);
        }
        Err(e) => { error!("checking facecam status: {e}"); exit(1) }
    }
}

/// Read the user's live graphical-session env via systemd-logind.
///
/// `sudo` strips most of the session env (notably `WAYLAND_DISPLAY` and
/// `XAUTHORITY`), so the binary inherits a stripped env from the install
/// pipeline and can't open a window. `systemctl --user show-environment`
/// is the canonical, well-supported primitive for reading the live session
/// env back: pam_systemd seeds it at login from `import-environment`.
///
/// We restrict to the keys facecam actually needs — overwriting HOME or
/// PATH from this dump would be wrong.
fn read_session_env() -> Vec<(String, String)> {
    // `set -a` auto-exports every assignment, so `eval` of systemctl's
    // `KEY=VALUE` lines makes them visible to `env -0` in the same shell.
    // bash + eval handles systemctl's `$'…'` quoting for values with
    // whitespace; `env -0` emits unambiguous null-separated KEY=VALUE pairs.
    let out = Command::new("bash")
        .args([
            "-c",
            r#"set -a; eval "$(systemctl --user show-environment 2>/dev/null)"; set +a; env -0"#,
        ])
        .output();
    let stdout = match out {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };

    const WANTED: &[&str] = &[
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "DBUS_SESSION_BUS_ADDRESS",
        "PULSE_SERVER",
    ];

    let mut env = Vec::new();
    for chunk in stdout.split(|b| *b == 0) {
        if chunk.is_empty() { continue }
        let Ok(s) = std::str::from_utf8(chunk) else { continue };
        let Some((k, v)) = s.split_once('=') else { continue };
        if WANTED.contains(&k) && !v.is_empty() {
            env.push((k.to_string(), v.to_string()));
        }
    }
    env
}

/// Resolve the user's currently selected audio output (default sink) and
/// return the name of its monitor source, suitable for `FACECAM_DEVICE`.
///
/// Uses `pactl get-default-sink`, the standard PulseAudio/PipeWire CLI for
/// "what is the user listening through right now". The matching capture
/// source is `<sink>.monitor`.
fn default_sink_monitor(session: &[(String, String)]) -> Option<String> {
    let mut cmd = Command::new("pactl");
    cmd.arg("get-default-sink");
    for (k, v) in session {
        cmd.env(k, v);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() { return None }
    let sink = std::str::from_utf8(&out.stdout).ok()?.trim();
    if sink.is_empty() { return None }
    Some(format!("{sink}.monitor"))
}
