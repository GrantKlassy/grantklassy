// Tiny logging helpers for setup scripts. Load from a sibling cargo-script with:
//
//     #[path = "util/log.rs"]
//     mod log;
//
// Then call info!/warn!/error!/ok!/step! — they're exported at crate root.

use std::io::IsTerminal;

pub fn use_color() -> bool {
    std::io::stdout().is_terminal()
}

pub fn paint(code: &str, body: &str) -> String {
    if use_color() {
        format!("\x1b[{code}m{body}\x1b[0m")
    } else {
        body.to_string()
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        println!("{} {}", $crate::log::paint("34", "info"), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        eprintln!("{} {}", $crate::log::paint("33", "warn"), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        eprintln!("{} {}", $crate::log::paint("31", "error"), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! ok {
    ($($arg:tt)*) => {
        println!("{} {}", $crate::log::paint("32", "ok"), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! step {
    ($($arg:tt)*) => {{
        let body = format!(">>> {}", format_args!($($arg)*));
        println!("\n{}", $crate::log::paint("1;36", &body));
    }};
}
