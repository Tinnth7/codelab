//! Tiny dependency-free logger: `[INFO] ...`, `[WARN] ...`, `[ERR] ...` on stderr.
//! Colored when stderr is a terminal (set NO_COLOR to turn that off).
//! Declared first in main.rs with `#[macro_use]`, so the macros work in every module.

use std::io::{IsTerminal, Write};

#[derive(Copy, Clone)]
pub enum Level {
    Info,
    Warn,
    Err,
}

pub fn write(level: Level, msg: &str) {
    let (tag, color) = match level {
        Level::Info => ("[INFO]", "\x1b[32m"),
        Level::Warn => ("[WARN]", "\x1b[33m"),
        Level::Err => ("[ERR]", "\x1b[31m"),
    };
    let stderr = std::io::stderr();
    let colored = stderr.is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let mut out = stderr.lock();
    if colored {
        let _ = writeln!(out, "{color}{tag}\x1b[0m {msg}");
    } else {
        let _ = writeln!(out, "{tag} {msg}");
    }
}

macro_rules! info {
    ($($arg:tt)*) => { $crate::logger::write($crate::logger::Level::Info, &format!($($arg)*)) };
}
macro_rules! warn {
    ($($arg:tt)*) => { $crate::logger::write($crate::logger::Level::Warn, &format!($($arg)*)) };
}
macro_rules! err {
    ($($arg:tt)*) => { $crate::logger::write($crate::logger::Level::Err, &format!($($arg)*)) };
}
