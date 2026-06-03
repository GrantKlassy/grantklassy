#!/bin/sh
#![allow(unused)] /*
exec cargo +nightly -Zscript "$0" "$@"
*/
// ^ POSIX-sh/Rust polyglot launcher (avoids `env -S`, GNU coreutils 8.30+ /
// 2018-only) — see install.rs for the full rationale. Needs a nightly Rust
// toolchain for `-Zscript`.
//
// gkdisk — grantklassy disk util (macOS). Reformat an external USB stick or
// SSD as exFAT, interactively and behind a typed confirmation, so a slip of
// the finger can't nuke the wrong drive.
//
// Single-file cargo script, same machinery as install.rs (nightly-only, needs
// `-Zscript`). Run it directly:
//
//   ~/.grantklassy/macos/gkdisk.rs
//
// Run the unit tests (the parser + validators are pure and covered) with:
//   cargo +nightly -Zscript test --manifest-path macos/gkdisk.rs
//
// macOS-only by construction: every disk operation shells out to `diskutil`.
// We never call mkfs/parted and never pull in a dependency — diskutil already
// knows how to lay down a GPT map and an exFAT volume in one `eraseDisk`, and
// staying std-only matches the rest of this repo.
//
// On sudo: erasing *external* media only needs ownership of that disk, which
// the logged-in user already has, so gkdisk neither asks for nor needs root.
// (diskutil itself also flatly refuses to erase the boot disk.)
//
// Safety model, in layers:
//   1. The picker is fed `diskutil list external physical`, so internal and
//      boot disks never even appear as choices.
//   2. Before formatting we re-read the chosen disk's info and bail if it
//      reports Internal — defense in depth against the listing ever lying.
//   3. The user must type the disk identifier (e.g. `disk4`) verbatim to
//      proceed; empty input or any mismatch aborts without touching anything.

#[path = "../util/log.rs"]
mod log;

use std::io::{self, IsTerminal, Write};
use std::process::{Command, exit};

/// exFAT volume-label ceiling enforced by newfs_exfat: "no longer than 11
/// UTF-16 characters" — so an emoji, being a surrogate pair, costs 2.
const MAX_LABEL_UTF16: usize = 11;

/// Volume name used when the user just hits Enter at the name prompt.
const DEFAULT_LABEL: &str = "GKDISK";

/// DOS-8.3-style punctuation newfs_exfat rejects in a volume label. We
/// pre-screen these for a friendly error rather than letting the erase fail
/// partway through; control characters are checked separately.
const FORBIDDEN_LABEL_CHARS: &[char] = &['"', '*', '/', ':', '<', '>', '?', '\\', '|'];

/// One external, physical whole-disk as gkdisk cares about it, built from
/// `diskutil info -plist`. Only the fields the picker and the safety re-check
/// actually use are kept.
#[derive(Debug, Clone, PartialEq)]
struct Disk {
    id: String,         // "disk4"
    node: String,       // "/dev/disk4"
    media_name: String, // "SanDisk Ultra USB 3.0 Media"
    size: u64,          // bytes
    bus: String,        // "USB", "Thunderbolt", ...
    solid_state: bool,
    internal: bool,
}

fn main() {
    // diskutil is the whole engine; don't pretend to work elsewhere. cfg! is
    // a compile-time bool so this costs nothing at runtime, and keeping it a
    // runtime check (rather than compile_error!) lets the pure-logic unit
    // tests build and run on any platform.
    if cfg!(not(target_os = "macos")) {
        error!("gkdisk is macOS-only — it drives `diskutil`.");
        exit(1);
    }

    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("-h" | "--help") => {
            print_help();
            return;
        }
        Some(other) => {
            error!("unknown argument `{other}`");
            print_help();
            exit(2);
        }
    }

    // The whole point is an interactive confirm; without a terminal we can't
    // ask the user to "type the disk id", so refuse rather than guess.
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        error!("gkdisk is interactive — run it in a terminal.");
        exit(1);
    }

    let disks = list_external_disks();
    if disks.is_empty() {
        info!("No external disks found. Plug in a USB stick or SSD and re-run.");
        return;
    }

    println!("External disks:");
    for (i, d) in disks.iter().enumerate() {
        let kind = if d.solid_state { "SSD" } else { "USB" };
        println!(
            "  {}) {:<7} {:<24} {:>9}  {} ({})",
            i + 1,
            d.id,
            truncate(&d.media_name, 24),
            human_size(d.size),
            d.bus,
            kind,
        );
    }

    let disk = match pick(&disks) {
        Some(d) => d,
        None => {
            info!("Cancelled.");
            return;
        }
    };

    // Layer 2 of the safety model: never format something that reports as
    // internal, even if it somehow slipped through the external-only listing.
    if disk.internal {
        error!("{} reports as an internal disk — refusing to format it.", disk.id);
        exit(1);
    }

    // Show the *current* contents straight from diskutil so the user sees
    // exactly what they're about to destroy — partitions, volume names, sizes.
    println!();
    warn!(
        "About to ERASE {} — {} ({}). Current contents:",
        disk.id,
        disk.media_name,
        human_size(disk.size),
    );
    println!();
    let _ = Command::new("diskutil").args(["list", &disk.id]).status();
    println!();

    let label = match prompt_label() {
        Some(l) => l,
        None => {
            info!("Cancelled.");
            return;
        }
    };

    println!();
    warn!("This will create an exFAT volume \"{label}\" (GPT) on {} and", disk.id);
    warn!("PERMANENTLY ERASE everything currently on {}.", disk.node);
    let typed = match prompt(&format!("Type '{}' to confirm (anything else aborts): ", disk.id)) {
        Ok(s) => s,
        Err(_) => {
            info!("Cancelled.");
            return;
        }
    };
    if typed != disk.id {
        info!("Confirmation didn't match — aborted. Nothing was changed.");
        return;
    }

    step!("Formatting {} as exFAT \"{label}\"", disk.id);
    if erase_exfat(&disk.id, &label) {
        ok!("{} is now an exFAT volume named \"{label}\".", disk.id);
    } else {
        error!("diskutil failed to format {}. Nothing else was attempted.", disk.id);
        exit(1);
    }
}

fn print_help() {
    println!(
        "gkdisk — format an external USB stick or SSD as exFAT (macOS).

Usage:
  gkdisk        Pick an external disk and reformat it as exFAT, interactively.
  gkdisk -h     Show this help.

gkdisk only ever lists and touches external, physical disks; internal and
boot disks never appear and are refused. The disk you pick is ERASED — you
confirm by typing its identifier (e.g. disk4) before anything is written."
    );
}

/// Prompt for a 1-based selection into `disks`. Returns None if the user
/// quits (empty line or `q`). Re-asks on bad input rather than aborting, so a
/// fat-fingered number doesn't drop you back to the shell.
fn pick(disks: &[Disk]) -> Option<Disk> {
    loop {
        let line = prompt(&format!("Format which? [1-{}, q to quit]: ", disks.len())).ok()?;
        let line = line.trim();
        if line.is_empty() || line.eq_ignore_ascii_case("q") {
            return None;
        }
        match line.parse::<usize>() {
            Ok(n) if (1..=disks.len()).contains(&n) => return Some(disks[n - 1].clone()),
            _ => warn!("Enter a number from 1 to {} (or q to quit).", disks.len()),
        }
    }
}

/// Ask for the exFAT volume label. Empty input takes DEFAULT_LABEL. Re-asks on
/// a name diskutil would reject, so the erase never fails for a label we could
/// have caught up front. Returns None on EOF (^D).
fn prompt_label() -> Option<String> {
    loop {
        let raw = prompt(&format!("Volume name [{DEFAULT_LABEL}]: ")).ok()?;
        if raw.trim().is_empty() {
            return Some(DEFAULT_LABEL.to_string());
        }
        match validate_label(&raw) {
            Ok(l) => return Some(l),
            Err(why) => warn!("Invalid name: {why}. Try again."),
        }
    }
}

/// Print `msg` (no trailing newline), flush, and read one line from stdin.
/// Returns the line with its trailing newline stripped, or Err on EOF/IO error
/// so callers can treat ^D as "cancel".
fn prompt(msg: &str) -> io::Result<String> {
    print!("{msg}");
    io::stdout().flush()?;
    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof"));
    }
    Ok(line.trim_end_matches(['\n', '\r']).to_string())
}

/// Whole external physical disks, as Disk records. We ask diskutil for exactly
/// that set (`external physical`), pull the identifiers out of `WholeDisks`,
/// then fetch each one's info. A disk whose info we can't read is skipped
/// rather than guessed at.
fn list_external_disks() -> Vec<Disk> {
    let Some(plist) = diskutil_capture(&["list", "-plist", "external", "physical"]) else {
        return Vec::new();
    };
    extract_string_array(&plist, "WholeDisks")
        .iter()
        .filter_map(|id| disk_info(id))
        .collect()
}

/// Read one whole disk's info via `diskutil info -plist`. Falls back through
/// MediaName → IORegistryEntryName → the identifier for a display name, and
/// TotalSize → Size for the byte count. None if the call itself fails.
fn disk_info(id: &str) -> Option<Disk> {
    let node = format!("/dev/{id}");
    let plist = diskutil_capture(&["info", "-plist", &node])?;
    let media_name = plist_scalar(&plist, "MediaName")
        .filter(|s| !s.is_empty())
        .or_else(|| plist_scalar(&plist, "IORegistryEntryName"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.to_string());
    let size = plist_scalar(&plist, "TotalSize")
        .or_else(|| plist_scalar(&plist, "Size"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Some(Disk {
        id: id.to_string(),
        node,
        media_name,
        size,
        bus: plist_scalar(&plist, "BusProtocol").unwrap_or_else(|| "?".into()),
        solid_state: plist_scalar(&plist, "SolidState").as_deref() == Some("true"),
        internal: plist_scalar(&plist, "Internal").as_deref() == Some("true"),
    })
}

/// Run `diskutil <args>` and return stdout as a String on success. None on a
/// spawn failure, a non-zero exit, or non-UTF-8 output.
fn diskutil_capture(args: &[&str]) -> Option<String> {
    let out = Command::new("diskutil").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// `diskutil eraseDisk ExFAT <label> GPT <id>` — lay down a fresh GPT map and
/// a single exFAT volume across the whole disk. stdio is inherited so the user
/// watches diskutil's own progress and errors live. Returns whether it
/// succeeded. GPT is the modern default and is read by macOS, Windows (Vista+)
/// and Linux alike; nothing here targets the few devices that still want MBR.
fn erase_exfat(id: &str, label: &str) -> bool {
    Command::new("diskutil")
        .args(["eraseDisk", "ExFAT", label, "GPT", id])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Clip `s` to `max` chars for the aligned picker table, adding an ellipsis
/// when it overflows. Counts chars (not bytes) so a multibyte media name can't
/// panic on a byte-boundary slice.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Format bytes the way diskutil does: decimal units (1 GB = 1e9 B), one
/// decimal place, so gkdisk's sizes match what `diskutil list` prints.
fn human_size(bytes: u64) -> String {
    const KB: f64 = 1e3;
    const MB: f64 = 1e6;
    const GB: f64 = 1e9;
    const TB: f64 = 1e12;
    let b = bytes as f64;
    if b >= TB {
        format!("{:.1} TB", b / TB)
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Check an exFAT volume label against newfs_exfat's rules (≤ 11 UTF-16 units,
/// no control chars, none of FORBIDDEN_LABEL_CHARS), trimming first. `raw` is
/// assumed non-empty — callers map empty input to DEFAULT_LABEL. Returns the
/// cleaned label, or a human-readable reason it's invalid.
fn validate_label(raw: &str) -> Result<String, String> {
    let label = raw.trim();
    if label.is_empty() {
        return Err("the name is empty".into());
    }
    let units = label.encode_utf16().count();
    if units > MAX_LABEL_UTF16 {
        return Err(format!(
            "{units} characters — exFAT allows at most {MAX_LABEL_UTF16}"
        ));
    }
    if label.chars().any(char::is_control) {
        return Err("it contains a control character".into());
    }
    if let Some(c) = label.chars().find(|c| FORBIDDEN_LABEL_CHARS.contains(c)) {
        return Err(format!("'{c}' is not allowed in exFAT names"));
    }
    Ok(label.to_string())
}

/// Slice between `<tag>` and `</tag>` when `s` *begins* with `<tag>`. Used
/// right after trimming whitespace past a `<key>`, when the value element sits
/// at the front. None if the open tag isn't first or the close tag is missing.
fn tag_inner<'a>(s: &'a str, tag: &str) -> Option<&'a str> {
    let body = s.strip_prefix(&format!("<{tag}>"))?;
    let end = body.find(&format!("</{tag}>"))?;
    Some(&body[..end])
}

/// Pull a scalar value for `key` out of a diskutil `-plist` document. diskutil
/// emits one flat dict, so "find `<key>NAME</key>`, take the next element"
/// resolves the size/bus/boolean fields we need unambiguously — no general
/// XML/plist parser (std has none, and one would dwarf this tool). Returns the
/// text of a <string>/<integer>, or "true"/"false" for <true/>/<false/>.
/// The `<key>NAME</key>` match is exact, so reading "Size" won't trip over
/// "TotalSize".
fn plist_scalar(xml: &str, key: &str) -> Option<String> {
    let needle = format!("<key>{key}</key>");
    let after = xml.find(&needle)? + needle.len();
    let rest = xml[after..].trim_start();
    if let Some(inner) = tag_inner(rest, "string") {
        Some(xml_unescape(inner))
    } else if let Some(inner) = tag_inner(rest, "integer") {
        Some(inner.trim().to_string())
    } else if rest.starts_with("<true/>") {
        Some("true".into())
    } else if rest.starts_with("<false/>") {
        Some("false".into())
    } else {
        None
    }
}

/// Collect every `<string>` inside the `<array>` that immediately follows
/// `<key>KEY</key>`. Used for `WholeDisks` in `diskutil list -plist`. An empty
/// `<array/>` (no disks attached) or a missing key both yield an empty Vec.
fn extract_string_array(xml: &str, key: &str) -> Vec<String> {
    let needle = format!("<key>{key}</key>");
    let Some(pos) = xml.find(&needle) else {
        return Vec::new();
    };
    let rest = xml[pos + needle.len()..].trim_start();
    let Some(body) = tag_inner(rest, "array") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut s = body;
    while let Some(start) = s.find("<string>") {
        let tail = &s[start + "<string>".len()..];
        let Some(end) = tail.find("</string>") else {
            break;
        };
        out.push(xml_unescape(&tail[..end]));
        s = &tail[end + "</string>".len()..];
    }
    out
}

/// Undo the five XML entity escapes plist uses. `&amp;` is decoded last so a
/// literal "&lt;" stored as "&amp;lt;" doesn't get double-decoded to "<".
fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    // A trimmed-down but faithful `diskutil info -plist` dict: scalars in the
    // exact shapes diskutil emits (string / integer / <true|false/>), with an
    // escaped ampersand in the media name and no `Size` key (so we can prove
    // an exact-key match doesn't read it off "TotalSize").
    const INFO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>BusProtocol</key>
	<string>USB</string>
	<key>DeviceIdentifier</key>
	<string>disk4</string>
	<key>DeviceNode</key>
	<string>/dev/disk4</string>
	<key>Internal</key>
	<false/>
	<key>MediaName</key>
	<string>SanDisk Ultra &amp; Co</string>
	<key>SolidState</key>
	<false/>
	<key>TotalSize</key>
	<integer>64023257088</integer>
	<key>WholeDisk</key>
	<true/>
</dict>
</plist>"#;

    const LIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>AllDisks</key>
	<array>
		<string>disk4</string>
		<string>disk4s1</string>
	</array>
	<key>WholeDisks</key>
	<array>
		<string>disk4</string>
		<string>disk6</string>
	</array>
</dict>
</plist>"#;

    const LIST_EMPTY: &str = r#"<plist version="1.0">
<dict>
	<key>AllDisks</key>
	<array/>
	<key>WholeDisks</key>
	<array/>
</dict>
</plist>"#;

    #[test]
    fn plist_scalar_reads_strings_ints_and_bools() {
        assert_eq!(plist_scalar(INFO, "BusProtocol").as_deref(), Some("USB"));
        assert_eq!(plist_scalar(INFO, "DeviceNode").as_deref(), Some("/dev/disk4"));
        assert_eq!(plist_scalar(INFO, "TotalSize").as_deref(), Some("64023257088"));
        assert_eq!(plist_scalar(INFO, "Internal").as_deref(), Some("false"));
        assert_eq!(plist_scalar(INFO, "WholeDisk").as_deref(), Some("true"));
    }

    #[test]
    fn plist_scalar_unescapes_string_values() {
        assert_eq!(
            plist_scalar(INFO, "MediaName").as_deref(),
            Some("SanDisk Ultra & Co"),
        );
    }

    #[test]
    fn plist_scalar_key_match_is_exact() {
        // "Size" is a suffix of "TotalSize"; reading it must NOT return
        // TotalSize's value — the fixture has no standalone Size key.
        assert_eq!(plist_scalar(INFO, "Size"), None);
    }

    #[test]
    fn plist_scalar_missing_key_is_none() {
        assert_eq!(plist_scalar(INFO, "Nonexistent"), None);
    }

    #[test]
    fn extract_string_array_takes_the_array_after_its_key() {
        assert_eq!(extract_string_array(LIST, "WholeDisks"), vec!["disk4", "disk6"]);
        // Proves we bind to the *named* key's array, not just the first one.
        assert_eq!(extract_string_array(LIST, "AllDisks"), vec!["disk4", "disk4s1"]);
    }

    #[test]
    fn extract_string_array_empty_and_missing() {
        assert!(extract_string_array(LIST_EMPTY, "WholeDisks").is_empty());
        assert!(extract_string_array(LIST, "NoSuchKey").is_empty());
    }

    #[test]
    fn xml_unescape_decodes_all_five_entities() {
        assert_eq!(
            xml_unescape("a &amp; b &lt;c&gt; &quot;d&quot; &apos;e&apos;"),
            r#"a & b <c> "d" 'e'"#,
        );
    }

    #[test]
    fn xml_unescape_does_not_double_decode() {
        // "&amp;lt;" is the escaping of the literal text "&lt;", which must
        // come back as "&lt;", not collapse to "<".
        assert_eq!(xml_unescape("&amp;lt;"), "&lt;");
    }

    #[test]
    fn human_size_matches_diskutil_decimal_units() {
        assert_eq!(human_size(251_000_193_024), "251.0 GB");
        assert_eq!(human_size(64_023_257_088), "64.0 GB");
        assert_eq!(human_size(1_000_000_000_000), "1.0 TB");
        assert_eq!(human_size(1_500_000), "1.5 MB");
        assert_eq!(human_size(2_500), "2.5 KB");
        assert_eq!(human_size(500), "500 B");
    }

    #[test]
    fn validate_label_accepts_and_trims() {
        assert_eq!(validate_label("BACKUP").unwrap(), "BACKUP");
        assert_eq!(validate_label("  spaced  ").unwrap(), "spaced");
        // Exactly 11 chars is the boundary and must pass.
        assert_eq!(validate_label("ELEVENCHARS").unwrap(), "ELEVENCHARS");
    }

    #[test]
    fn validate_label_rejects_too_long_counting_utf16() {
        assert!(validate_label("TWELVELETTER").is_err()); // 12 chars
        // Six emoji = 12 UTF-16 code units even though it's 6 Rust chars, so
        // the limit is enforced in UTF-16 the way newfs_exfat measures it.
        assert!(validate_label("😀😀😀😀😀😀").is_err());
        // Five emoji = 10 units, under the limit.
        assert!(validate_label("😀😀😀😀😀").is_ok());
    }

    #[test]
    fn validate_label_rejects_control_and_forbidden_punctuation() {
        assert!(validate_label("bad\tname").is_err());
        assert!(validate_label("bad/name").is_err());
        assert!(validate_label("a:b").is_err());
    }

    #[test]
    fn truncate_clips_with_ellipsis() {
        assert_eq!(truncate("short", 24), "short");
        assert_eq!(truncate("abcdef", 4), "abc…");
        // Char-based: a multibyte string clipped mid-name must not panic.
        assert_eq!(truncate("ééééé", 3), "éé…");
    }
}
