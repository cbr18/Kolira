//! Tiny best-effort error log: only failures, kept about a day, capped at 50 MiB.
//!
//! The Quick Switch and the main GUI surface an apply failure only in the
//! moment — a panel, a notification — and once the overlay closes the text is
//! gone. This appends one line per error to
//! `$XDG_CACHE_HOME/kolira/error.log` so the last day of failures can be
//! inspected after the fact (`tail -f ~/.cache/kolira/error.log`).
//!
//! It is deliberately dumb and never blocks or panics: every step is
//! best-effort, and a broken log must never fail the command that hit the
//! error. Only errors are written — successes and progress never reach here.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Local};

/// Hard ceiling for the log file; the oldest lines are dropped past this.
const MAX_BYTES: u64 = 50 * 1024 * 1024;
/// How long a line is kept.
const RETENTION_DAYS: i64 = 1;

fn log_path() -> PathBuf {
    crate::paths::cache_home().join("kolira").join("error.log")
}

/// Collapse a multi-line message to one log line.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

/// Append one error line, then trim the file to the retention window and size.
pub fn log_error(context: &str, message: &str) {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let now = Local::now();
    let line = format!(
        "{} ERROR {}: {}\n",
        now.to_rfc3339(),
        one_line(context),
        one_line(message)
    );
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
    }
    prune(&path, now);
}

/// Drop lines older than the retention window, then cap the total size.
fn prune(path: &Path, now: DateTime<Local>) {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return,
    };
    let cutoff = now - Duration::days(RETENTION_DAYS);
    let mut kept = String::with_capacity(content.len());
    for line in content.lines() {
        let timestamp = line
            .split(' ')
            .next()
            .and_then(|token| DateTime::parse_from_rfc3339(token).ok());
        if let Some(stamp) = timestamp {
            if stamp.with_timezone(&Local) < cutoff {
                continue;
            }
        }
        kept.push_str(line);
        kept.push('\n');
    }
    if kept.len() as u64 > MAX_BYTES {
        let bytes = kept.as_bytes();
        let mut start = bytes.len() - MAX_BYTES as usize;
        // Start on a line boundary so the first kept line is not half-written.
        while start < bytes.len() && bytes[start] != b'\n' {
            start += 1;
        }
        kept = String::from_utf8_lossy(&bytes[start..]).into_owned();
    }
    if kept != content {
        let _ = fs::write(path, kept);
    }
}
