//! The node's log: JSON lines in `<data dir>/logs/node.jsonl`, closed and renamed when the file gets too
//! big, with the old files deleted by age and count. Nothing the node says reaches the log with a
//! request body, a token, or a query string: callers pass finished sentences, as they did to stderr.

pub mod files;
#[cfg(test)]
mod tests;

use crate::Log;
use agency_core::logging_config::{LogLevel, LoggingConfig};
use agency_core::ports::Clock;
use serde::Serialize;
use std::fs::File;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::{Duration, OffsetDateTime, UtcOffset};

/// A line longer than this many characters is cut, so one failing runtime cannot fill a file.
const MESSAGE_LIMIT: usize = 4000;

const TIMESTAMP: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

/// One line of the file. The fields stay in this order.
#[derive(Serialize)]
struct Line<'a> {
    ts: String,
    level: &'a str,
    event: &'a str,
    message: &'a str,
}

struct Open {
    file: File,
    size: u64,
}

struct Inner {
    dir: PathBuf,
    settings: LoggingConfig,
    clock: Arc<dyn Clock>,
    /// Also print each line to standard error: for a node run in a terminal.
    mirrors_to_stderr: bool,
    file: Mutex<Option<Open>>,
    has_reported_failure: AtomicBool,
}

/// Writes the node's log. Cloning shares the file.
#[derive(Clone)]
pub struct Logger {
    inner: Arc<Inner>,
}

impl Logger {
    /// A logger that writes to `dir`, which is created when the first line is written. When standard
    /// error is a terminal, every line is also printed there.
    pub fn open(dir: PathBuf, settings: LoggingConfig, clock: Arc<dyn Clock>) -> Self {
        Self::new(dir, settings, clock, std::io::stderr().is_terminal())
    }

    pub fn new(
        dir: PathBuf,
        settings: LoggingConfig,
        clock: Arc<dyn Clock>,
        mirrors_to_stderr: bool,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                dir,
                settings,
                clock,
                mirrors_to_stderr,
                file: Mutex::new(None),
                has_reported_failure: AtomicBool::new(false),
            }),
        }
    }

    pub fn error(&self, event: &str, message: &str) {
        self.record(LogLevel::Error, event, message);
    }

    pub fn warn(&self, event: &str, message: &str) {
        self.record(LogLevel::Warn, event, message);
    }

    /// Writes a line, unless its level is more verbose than the configured one. When the file cannot be
    /// written, the line goes to standard error instead, as every line did before there was a log file.
    pub fn record(&self, level: LogLevel, event: &str, message: &str) {
        let inner = &self.inner;
        if level > inner.settings.level {
            return;
        }
        let message = cut(message);
        let now = inner.clock.now();
        match self.write(now, &line_of(now, level, event, message)) {
            Ok(()) if inner.mirrors_to_stderr => eprintln!("agency: {message}"),
            Ok(()) => {}
            Err(error) => {
                eprintln!("agency: {message}");
                if !inner.has_reported_failure.swap(true, Ordering::Relaxed) {
                    eprintln!(
                        "agency: cannot write the log in {}: {error}",
                        inner.dir.display()
                    );
                }
            }
        }
    }

    /// A function for the adapters that take a `Log`: it writes each line at `level` as `event`, without
    /// the `agency: ` the lines carried on standard error.
    pub fn sink(&self, level: LogLevel, event: &'static str) -> Log {
        let logger = self.clone();
        Box::new(move |line| {
            logger.record(level, event, line.strip_prefix("agency: ").unwrap_or(line));
        })
    }

    fn write(&self, now: OffsetDateTime, line: &str) -> std::io::Result<()> {
        let inner = &self.inner;
        let mut guard = inner.file.lock().unwrap_or_else(PoisonError::into_inner);
        // One write for the line and its newline, so a crash leaves at most one cut line at the end.
        let bytes = format!("{line}\n");
        let size = bytes.len() as u64;
        let mut open = match guard.take() {
            Some(open) => open,
            None => self.start(now)?,
        };
        // A line is far smaller than the least size the settings allow, so a file is never closed empty.
        if open.size + size > inner.settings.max_bytes() {
            drop(open);
            open = self.rotate(now)?;
        }
        let written = open.file.write_all(bytes.as_bytes());
        if written.is_ok() {
            open.size += size;
        }
        *guard = Some(open);
        written
    }

    /// Opens the current file, after deleting the closed files that have expired.
    fn start(&self, now: OffsetDateTime) -> std::io::Result<Open> {
        let inner = &self.inner;
        let mut directory = std::fs::DirBuilder::new();
        directory.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut directory, 0o700);
        directory.create(&inner.dir)?;
        self.delete_expired(now);
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let file = options.open(inner.dir.join(files::CURRENT))?;
        let size = file.metadata()?.len();
        Ok(Open { file, size })
    }

    /// Closes the current file under the name of the time and starts a new one.
    fn rotate(&self, now: OffsetDateTime) -> std::io::Result<Open> {
        let dir = &self.inner.dir;
        let name = (0..)
            .map(|nth| files::closed_name(now, nth))
            .find(|name| !dir.join(name).exists())
            .unwrap_or_default();
        std::fs::rename(dir.join(files::CURRENT), dir.join(name))?;
        self.start(now)
    }

    /// Deleting is best effort: a file that cannot be deleted is tried again at the next rotation.
    fn delete_expired(&self, now: OffsetDateTime) {
        let settings = &self.inner.settings;
        let closed = files::closed_files(&self.inner.dir);
        let max_age = Duration::days(settings.max_age_days.into());
        for file in files::expired(&closed, now, max_age, settings.max_files as usize) {
            let _ = std::fs::remove_file(&file.path);
        }
    }
}

fn line_of(now: OffsetDateTime, level: LogLevel, event: &str, message: &str) -> String {
    let line = Line {
        ts: now
            .to_offset(UtcOffset::UTC)
            .format(TIMESTAMP)
            .unwrap_or_default(),
        level: level.as_str(),
        event,
        message,
    };
    serde_json::to_string(&line).unwrap_or_default()
}

fn cut(message: &str) -> &str {
    match message.char_indices().nth(MESSAGE_LIMIT) {
        Some((end, _)) => &message[..end],
        None => message,
    }
}
