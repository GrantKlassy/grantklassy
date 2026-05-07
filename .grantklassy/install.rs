#!/usr/bin/env -S cargo +nightly -Zscript
// Single-file cargo script (still nightly-only — requires `-Zscript`).
// Add deps later via a `---` frontmatter block above `fn main`.
//
// Bootstrap dependencies — must be present before the first invocation,
// since cargo-script can't link its own binary without them:
//   - bash  (preinstalled on Fedora)
//   - rust  (rustup, or `sudo dnf install -y rust cargo`)
//   - gcc   (provides `cc` for linking; `sudo dnf install -y gcc`)
// On subsequent runs the gcc step below keeps it installed.
//
// Run unit tests with:
//   cargo +nightly -Zscript test --manifest-path install.rs

#[path = "util/log.rs"]
mod log;

use std::process::{Command, exit};
use std::thread;
use std::time::{Duration, Instant};

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

fn rpm_installed(pkg: &str) -> bool {
    Command::new("rpm")
        .args(["-q", pkg])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn dnf_repo_enabled(repo_id: &str) -> bool {
    let out = match Command::new("dnf").arg("repolist").output() {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };
    parse_repolist_has(&String::from_utf8_lossy(&out.stdout), repo_id)
}

// First whitespace-separated token of each non-header line is the repo id.
fn parse_repolist_has(output: &str, repo_id: &str) -> bool {
    output
        .lines()
        .skip(1)
        .filter_map(|l| l.split_whitespace().next())
        .any(|id| id == repo_id)
}

fn fedora_release() -> Option<String> {
    let out = Command::new("rpm").args(["-E", "%fedora"]).output().ok()?;
    if !out.status.success() { return None; }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!v.is_empty() && v != "%fedora").then_some(v)
}

fn nvidia_module_version() -> Option<String> {
    let out = Command::new("modinfo").args(["-F", "version", "nvidia"]).output().ok()?;
    if !out.status.success() { return None; }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!v.is_empty()).then_some(v)
}

// akmod-nvidia compiles the kernel module asynchronously after install. Booting
// before it finishes means a black screen, so we block until modinfo can find it.
fn wait_for_nvidia_module(timeout: Duration) -> Option<String> {
    let start = Instant::now();
    let mut last_log = Instant::now()
        .checked_sub(Duration::from_secs(60))
        .unwrap_or_else(Instant::now);
    loop {
        if let Some(v) = nvidia_module_version() {
            return Some(v);
        }
        if start.elapsed() >= timeout {
            return None;
        }
        if last_log.elapsed() >= Duration::from_secs(30) {
            info!("waiting for nvidia kmod build ({}s elapsed)", start.elapsed().as_secs());
            last_log = Instant::now();
        }
        thread::sleep(Duration::from_secs(5));
    }
}

fn main() {
    step!("gcc");
    if rpm_installed("gcc") {
        ok!("already installed");
    } else {
        run("sudo", &["dnf", "install", "-y", "gcc"]);
    }

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
    if rpm_installed("google-chrome-stable") {
        ok!("already installed");
    } else {
        let rpm = "/tmp/google-chrome-stable_current_x86_64.rpm";
        run("curl", &[
            "-fsSL", "-o", rpm,
            "https://dl.google.com/linux/direct/google-chrome-stable_current_x86_64.rpm",
        ]);
        run("sudo", &["dnf", "install", "-y", rpm]);
    }

    step!("claude code");
    run("sh", &["-c", "curl -fsSL https://claude.ai/install.sh | bash"]);

    step!("nvidia driver (rpm fusion + akmod)");
    install_nvidia_driver();

    warn!("reboot required before nvidia driver takes effect");
}

fn install_nvidia_driver() {
    // 1. RPM Fusion free + nonfree.
    let need_free = !dnf_repo_enabled("rpmfusion-free");
    let need_nonfree = !dnf_repo_enabled("rpmfusion-nonfree");
    if need_free || need_nonfree {
        let fedora_ver = fedora_release().unwrap_or_else(|| {
            error!("could not detect fedora release via `rpm -E %fedora`");
            exit(1);
        });
        info!("enabling rpm fusion (free={need_free}, nonfree={need_nonfree}, fc{fedora_ver})");
        let mut urls: Vec<String> = vec![];
        if need_free {
            urls.push(format!(
                "https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-{fedora_ver}.noarch.rpm"
            ));
        }
        if need_nonfree {
            urls.push(format!(
                "https://mirrors.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-{fedora_ver}.noarch.rpm"
            ));
        }
        let mut args: Vec<&str> = vec!["dnf", "install", "-y"];
        args.extend(urls.iter().map(String::as_str));
        run("sudo", &args);
    } else {
        ok!("rpm fusion already enabled");
    }

    // 2. Refresh metadata so dnf sees the new repos.
    info!("refreshing dnf metadata");
    run("sudo", &["dnf", "makecache", "--refresh"]);

    // 3. Driver packages. libva-nvidia-driver fixes the libva errors that
    //    appear in the journal under nouveau (VA-API hardware video decode).
    let pkgs = ["akmod-nvidia", "xorg-x11-drv-nvidia-cuda", "libva-nvidia-driver"];
    let missing: Vec<&str> = pkgs.iter().copied().filter(|p| !rpm_installed(p)).collect();
    if missing.is_empty() {
        ok!("nvidia driver packages already installed");
    } else {
        info!("installing: {missing:?}");
        let mut args: Vec<&str> = vec!["dnf", "install", "-y"];
        args.extend(missing.iter().copied());
        run("sudo", &args);
    }

    // 4. Block on the akmod build — booting too early gives a black screen.
    info!("waiting for nvidia kmod build (up to 10 min)");
    match wait_for_nvidia_module(Duration::from_secs(600)) {
        Some(v) => ok!("nvidia kmod ready: version {v}"),
        None => {
            error!("nvidia kmod did not build in 10 min");
            error!("check build log: sudo journalctl -u akmods -b");
            error!("do NOT reboot until `modinfo -F version nvidia` reports a version");
            exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_repolist_has;

    #[test]
    fn finds_enabled_repos() {
        let out = "repo id               repo name\n\
                   fedora                Fedora 44 - x86_64\n\
                   updates               Fedora 44 - x86_64 - Updates\n\
                   rpmfusion-free        RPM Fusion 44 - Free\n\
                   rpmfusion-nonfree     RPM Fusion 44 - Nonfree\n";
        assert!(parse_repolist_has(out, "fedora"));
        assert!(parse_repolist_has(out, "updates"));
        assert!(parse_repolist_has(out, "rpmfusion-free"));
        assert!(parse_repolist_has(out, "rpmfusion-nonfree"));
    }

    #[test]
    fn missing_repo_returns_false() {
        let out = "repo id  repo name\n\
                   fedora   Fedora 44\n";
        assert!(!parse_repolist_has(out, "rpmfusion-free"));
    }

    // Guards against false positives from prefix matches:
    // `rpmfusion-free-updates` should not satisfy a check for `rpmfusion-free`.
    #[test]
    fn does_not_match_prefix() {
        let out = "repo id                 repo name\n\
                   rpmfusion-free-updates  RPM Fusion Free Updates\n";
        assert!(parse_repolist_has(out, "rpmfusion-free-updates"));
        assert!(!parse_repolist_has(out, "rpmfusion-free"));
    }

    #[test]
    fn empty_input() {
        assert!(!parse_repolist_has("", "fedora"));
    }

    #[test]
    fn header_only() {
        assert!(!parse_repolist_has("repo id  repo name\n", "fedora"));
    }

    #[test]
    fn ignores_blank_lines() {
        let out = "repo id  repo name\n\
                   \n\
                   fedora   Fedora 44\n";
        assert!(parse_repolist_has(out, "fedora"));
    }
}
