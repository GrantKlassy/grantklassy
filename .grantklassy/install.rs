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
    // Pick up PATH additions from .bashrc/.cargo/env that the sudo-spawned
    // bash didn't see. Repeated between steps so each install pulls in the
    // PATH the previous one wrote.
    refresh_env();

    step!("claude code");
    run("sh", &["-c", "curl -fsSL https://claude.ai/install.sh | bash"]);
    refresh_env();

    step!("facecam");
    let dir = format!("{}/facecam", home());
    if Path::new(&dir).join(".git").exists() {
        info!("facecam already cloned at {dir}");
    } else {
        run("git", &["clone", "https://github.com/GrantKlassy/facecam.git", &dir]);
    }
    run("cargo", &["build", "--release", "--manifest-path", &format!("{dir}/Cargo.toml")]);
    refresh_env();

    launch_facecam(&format!("{dir}/target/release/facecam"));

    step!("done");
    info!("to pick up new env in your current shell, run: `exec bash -l` or `source ~/.bashrc`");
}

/// Re-source ~/.bashrc and ~/.cargo/env into install.rs's own env so any
/// subprocess we spawn afterwards sees PATH additions written by earlier
/// install steps. install.rs starts in a sudo-spawned subshell created
/// before any of our installs ran; without this, freshly installed
/// binaries (cargo from rustup, claude) wouldn't be on PATH for the next
/// step.
fn refresh_env() {
    let out = Command::new("bash")
        .args([
            "-c",
            // `set -a` exports every assignment, so PATH/etc. set inside
            // .bashrc become real env vars in the spawned bash, visible to
            // `env -0`. Sourcing failures are non-fatal — fresh systems
            // may lack one or both files on first run.
            r#"set -a; . "$HOME/.bashrc" 2>/dev/null; . "$HOME/.cargo/env" 2>/dev/null; set +a; env -0"#,
        ])
        .output();
    let Ok(o) = out else { return };
    if !o.status.success() { return }
    for (k, v) in parse_env0(&o.stdout, ENV_PASSTHROUGH) {
        // SAFETY: install.rs is single-threaded until launch_facecam.
        unsafe { std::env::set_var(&k, &v); }
    }
}

/// Spawn facecam detached, with the user's live session env restored, the
/// currently selected audio output as the visualizer source, and a brief
/// liveness check so we don't lie about a launch that died on startup.
fn launch_facecam(bin: &str) {
    let session = read_session_env();
    if !session.iter().any(|(k, _)| k == "WAYLAND_DISPLAY" || k == "DISPLAY") {
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
    parse_env0(&stdout, SESSION_KEYS)
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
    sink_to_monitor(std::str::from_utf8(&out.stdout).ok()?)
}

/// Keys facecam needs from the user's live graphical session.
const SESSION_KEYS: &[&str] = &[
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XAUTHORITY",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_TYPE",
    "DBUS_SESSION_BUS_ADDRESS",
    "PULSE_SERVER",
];

/// Keys we let `refresh_env` overwrite in our own env after sourcing
/// .bashrc/.cargo/env. PATH is the load-bearing one (newly installed
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

/// Append `.monitor` to a sink name from `pactl get-default-sink`. Returns
/// `None` if the input is empty or whitespace-only (no default sink set).
fn sink_to_monitor(sink_stdout: &str) -> Option<String> {
    let s = sink_stdout.trim();
    if s.is_empty() { return None }
    Some(format!("{s}.monitor"))
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
        // sudo-stripped env can leave WAYLAND_DISPLAY=<empty>; passing that
        // through would prevent facecam's Wayland init from running.
        let input = b"DISPLAY=\0WAYLAND_DISPLAY=wayland-0\0";
        let got = parse_env0(input, &["DISPLAY", "WAYLAND_DISPLAY"]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "WAYLAND_DISPLAY");
    }

    #[test]
    fn parse_env0_preserves_equals_in_values() {
        let input = b"DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus\0";
        let got = parse_env0(input, &["DBUS_SESSION_BUS_ADDRESS"]);
        assert_eq!(got[0].1, "unix:path=/run/user/1000/bus");
    }

    #[test]
    fn parse_env0_handles_empty_chunks() {
        // Real `env -0` output from bash has no trailing-double-null, but
        // we tolerate either form to stay defensive.
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
    fn sink_to_monitor_appends_suffix() {
        assert_eq!(
            sink_to_monitor("alsa_output.pci-0000_00_1f.3.iec958-stereo\n"),
            Some("alsa_output.pci-0000_00_1f.3.iec958-stereo.monitor".into())
        );
    }

    #[test]
    fn sink_to_monitor_trims_surrounding_whitespace() {
        assert_eq!(sink_to_monitor("  foo\n"), Some("foo.monitor".into()));
        assert_eq!(sink_to_monitor("\tfoo\t\n"), Some("foo.monitor".into()));
    }

    #[test]
    fn sink_to_monitor_returns_none_for_empty() {
        assert_eq!(sink_to_monitor(""), None);
        assert_eq!(sink_to_monitor("\n"), None);
        assert_eq!(sink_to_monitor("   "), None);
        assert_eq!(sink_to_monitor("\t\n  \n"), None);
    }
}
