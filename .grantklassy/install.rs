#!/usr/bin/env -S cargo
// Single-file cargo script (stable since Rust 1.87).
// Add deps later via a `---` frontmatter block above `fn main`.

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
    step!("vim");
    run("sudo", &["dnf", "install", "-y", "vim"]);

    step!("snapd");
    run("sudo", &["dnf", "install", "-y", "snapd"]);
    run("sudo", &["systemctl", "enable", "--now", "snapd.socket"]);
    // /snap symlink — ignore failure if it already exists.
    let _ = Command::new("sudo")
        .args(["ln", "-s", "/var/lib/snapd/snap", "/snap"])
        .status();

    step!("chrome");
    let rpm = "/tmp/google-chrome-stable_current_x86_64.rpm";
    run("curl", &[
        "-fsSL", "-o", rpm,
        "https://dl.google.com/linux/direct/google-chrome-stable_current_x86_64.rpm",
    ]);
    run("sudo", &["dnf", "install", "-y", rpm]);

    step!("claude code");
    run("sh", &["-c", "curl -fsSL https://claude.ai/install.sh | bash"]);
}
