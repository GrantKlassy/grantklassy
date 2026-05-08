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

    // Detach: new session via `setsid`, stdio to /dev/null, no wait. Survives
    // install.rs exiting and the controlling terminal going away.
    let bin = format!("{dir}/target/release/facecam");
    Command::new("setsid")
        .arg(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| { error!("spawn facecam: {e}"); exit(1) });
    ok!("facecam launched in background");
}
