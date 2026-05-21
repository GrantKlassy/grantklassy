#!/usr/bin/env -S cargo +nightly -Zscript
// Single-file cargo script for user-space setup (nightly-only — needs
// `-Zscript`). Add deps later via a `---` frontmatter block above `fn main`.
//
// Prerequisites (git, gh, stable Rust) come from the public bootstrap repo:
//   curl -fsSL https://raw.githubusercontent.com/GrantKlassy/bootstrap/main/install.sh | bash
// Then `rustup toolchain install nightly` so `-Zscript` is available.
//
// This script runs as the user, never as root. No `dnf`/`sudo` calls live
// here — writes only under $HOME.
//
// Run unit tests with:
//   cargo +nightly -Zscript test --manifest-path install.rs

#[path = "util/log.rs"]
mod log;

use std::process::{Command, exit};

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

fn main() {
    // Pick up PATH additions from .bashrc/.cargo/env that the sudo-spawned
    // bash didn't see. Repeated between steps so each install pulls in the
    // PATH the previous one wrote.
    refresh_env();

    step!("claude code");
    run("sh", &["-c", "curl -fsSL https://claude.ai/install.sh | bash"]);
    refresh_env();

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
        // SAFETY: install.rs is single-threaded.
        unsafe { std::env::set_var(&k, &v); }
    }
}

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

}
