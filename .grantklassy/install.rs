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
    step!("claude code");
    run("sh", &["-c", "curl -fsSL https://claude.ai/install.sh | bash"]);
}
