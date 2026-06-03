// Tiny logging helpers for setup scripts. Load from a sibling cargo-script with:
//
//     #[path = "util/log.rs"]
//     mod log;
//
// Then call info!/warn!/error!/ok!/step! — they're exported at crate root.

use std::io::IsTerminal;

// Colors are emitted only when (a) the *target* stream is a real terminal,
// (b) NO_COLOR is unset (https://no-color.org), and (c) TERM isn't "dumb".
// Checking the *target* stream matters: warn!/error! write to stderr, so their
// coloring must follow stderr's TTY-ness, not stdout's — otherwise `cmd >file`
// strips color from terminal-bound errors, and `cmd 2>file` writes raw escape
// bytes into the file.
fn colors_ok(stream_is_tty: bool) -> bool {
    stream_is_tty
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
}

fn paint_on(code: &str, body: &str, stream_is_tty: bool) -> String {
    if colors_ok(stream_is_tty) {
        format!("\x1b[{code}m{body}\x1b[0m")
    } else {
        body.to_string()
    }
}

/// Colorize for stdout (info!/ok!/step!).
pub fn paint(code: &str, body: &str) -> String {
    paint_on(code, body, std::io::stdout().is_terminal())
}

/// Colorize for stderr (warn!/error!).
pub fn paint_err(code: &str, body: &str) -> String {
    paint_on(code, body, std::io::stderr().is_terminal())
}

// Every line is prefixed with `GK <emoji>` so grantklassy's own logs are
// distinguishable from output emitted by the subprocesses we spawn (rustup,
// the claude installer, etc.). The emoji also encodes the level, since the
// `GK` text marker is the same on every line.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        println!("{} {}", $crate::log::paint("34", "GK ℹ️"), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        eprintln!("{} {}", $crate::log::paint_err("33", "GK ⚠️"), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        eprintln!("{} {}", $crate::log::paint_err("31", "GK ❌"), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! ok {
    ($($arg:tt)*) => {
        println!("{} {}", $crate::log::paint("32", "GK ✅"), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! step {
    ($($arg:tt)*) => {{
        let body = format!("GK 🚀 {}", format_args!($($arg)*));
        println!("\n{}", $crate::log::paint("1;36", &body));
    }};
}
