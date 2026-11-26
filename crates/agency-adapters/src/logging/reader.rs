//! Reading the node's log back: the closed files and the current one, filtered. It reads files only, so
//! it works while the node is stopped, and it skips a line that is not a whole log line (the one a crash
//! may have cut at the end of a file).

use super::files::{closed_files, CURRENT};
use agency_core::logging_config::LogLevel;
use serde::Deserialize;
use std::path::Path;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// One line of the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub at: OffsetDateTime,
    pub level: LogLevel,
    pub event: String,
    pub message: String,
    /// The line as it is in the file.
    pub raw: String,
}

#[derive(Deserialize)]
struct RawLine {
    ts: String,
    level: String,
    event: String,
    message: String,
}

impl Entry {
    fn parse(line: &str) -> Option<Self> {
        let raw: RawLine = serde_json::from_str(line).ok()?;
        Some(Self {
            at: OffsetDateTime::parse(&raw.ts, &Rfc3339).ok()?,
            level: LogLevel::parse(&raw.level)?,
            event: raw.event,
            message: raw.message,
            raw: line.to_owned(),
        })
    }
}

/// Which lines to keep. Every part left out keeps everything.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    /// Lines at this time or later.
    pub since: Option<OffsetDateTime>,
    /// Lines of this level or a more urgent one, so `warn` keeps `warn` and `error`.
    pub level: Option<LogLevel>,
    /// Lines of this event only.
    pub event: Option<String>,
}

impl Filter {
    fn keeps(&self, entry: &Entry) -> bool {
        self.since.is_none_or(|since| entry.at >= since)
            && self.level.is_none_or(|level| entry.level <= level)
            && self
                .event
                .as_ref()
                .is_none_or(|event| &entry.event == event)
    }
}

/// The lines of the log in `dir` that `filter` keeps, oldest first. A directory with no log has none.
pub fn read(dir: &Path, filter: &Filter) -> Vec<Entry> {
    let mut closed = closed_files(dir);
    // A file was closed after its last line, so one closed before `since` holds nothing to keep. Its name
    // has whole seconds, so it may hold lines up to the end of that second.
    let one_second = time::Duration::seconds(1);
    closed.retain(|file| {
        filter
            .since
            .is_none_or(|since| file.at + one_second > since)
    });
    let paths = closed
        .iter()
        .rev()
        .map(|file| file.path.clone())
        .chain(std::iter::once(dir.join(CURRENT)));
    let mut entries = Vec::new();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        entries.extend(
            text.lines()
                .filter_map(Entry::parse)
                .filter(|entry| filter.keeps(entry)),
        );
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn line(ts: &str, level: &str, event: &str, message: &str) -> String {
        format!(r#"{{"ts":"{ts}","level":"{level}","event":"{event}","message":"{message}"}}"#)
    }

    fn write(dir: &Path, name: &str, lines: &[String]) {
        std::fs::write(dir.join(name), lines.join("\n") + "\n").unwrap();
    }

    fn messages(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.message.as_str()).collect()
    }

    fn sample() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "node-20261125T120000Z.jsonl",
            &[
                line("2026-11-25T10:00:00.000Z", "info", "start", "old start"),
                line("2026-11-25T11:00:00.000Z", "warn", "runtime", "old runtime"),
            ],
        );
        write(
            dir.path(),
            "node.jsonl",
            &[
                line("2026-11-26T01:00:00.000Z", "error", "push", "push error"),
                "{\"ts\":\"2026-11-26T02:00:00.000Z\",\"lev".into(),
                "not json at all".into(),
                line("2026-11-26T03:00:00.000Z", "info", "start", "new start"),
                line("2026-11-26T04:00:00.000Z", "warn", "runtime", "new runtime"),
            ],
        );
        dir
    }

    #[test]
    fn the_closed_files_come_first_then_the_current_one_and_a_cut_line_is_skipped() {
        let dir = sample();
        let all = read(dir.path(), &Filter::default());
        assert_eq!(
            messages(&all),
            [
                "old start",
                "old runtime",
                "push error",
                "new start",
                "new runtime"
            ]
        );
        assert_eq!(all[0].at, datetime!(2026-11-25 10:00:00 UTC));
        assert_eq!(
            all[0].raw,
            line("2026-11-25T10:00:00.000Z", "info", "start", "old start")
        );
    }

    #[test]
    fn a_level_keeps_that_level_and_the_more_urgent_ones() {
        let dir = sample();
        let at = |level| Filter {
            level: Some(level),
            ..Filter::default()
        };
        assert_eq!(
            messages(&read(dir.path(), &at(LogLevel::Error))),
            ["push error"]
        );
        assert_eq!(
            messages(&read(dir.path(), &at(LogLevel::Warn))),
            ["old runtime", "push error", "new runtime"]
        );
        assert_eq!(read(dir.path(), &at(LogLevel::Info)).len(), 5);
    }

    #[test]
    fn an_event_and_a_time_narrow_the_lines_and_a_file_closed_before_the_time_is_not_read() {
        let dir = sample();
        let runtime = Filter {
            event: Some("runtime".into()),
            ..Filter::default()
        };
        assert_eq!(
            messages(&read(dir.path(), &runtime)),
            ["old runtime", "new runtime"]
        );
        let since = |at| Filter {
            since: Some(at),
            ..Filter::default()
        };
        assert_eq!(
            messages(&read(
                dir.path(),
                &since(datetime!(2026-11-26 03:00:00 UTC))
            )),
            ["new start", "new runtime"],
            "a line at exactly the time is kept"
        );
        // A closed file's name says when it was closed, after its last line, so a file closed before the
        // time has nothing to keep and is not opened. A line that contradicts its file's name shows it:
        // it would be in the answer if the file were read.
        write(
            dir.path(),
            "node-20261125T120000Z.jsonl",
            &[line(
                "2026-11-26T05:00:00.000Z",
                "warn",
                "runtime",
                "from a file closed earlier",
            )],
        );
        let after = since(datetime!(2026-11-26 00:00:00 UTC));
        assert_eq!(
            messages(&read(dir.path(), &after)),
            ["push error", "new start", "new runtime"]
        );
        assert_eq!(
            read(dir.path(), &Filter::default()).len(),
            4,
            "without a time it is read"
        );
        let combined = Filter {
            since: Some(datetime!(2026-11-26 00:00:00 UTC)),
            level: Some(LogLevel::Warn),
            event: Some("runtime".into()),
        };
        assert_eq!(messages(&read(dir.path(), &combined)), ["new runtime"]);
    }

    #[test]
    fn a_file_closed_in_the_second_of_the_time_is_read_and_files_are_in_order() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "node-20261126T120000Z.jsonl",
            &[
                line("2026-11-26T11:59:59.900Z", "info", "x", "before"),
                line(
                    "2026-11-26T12:00:00.700Z",
                    "info",
                    "x",
                    "in the closing second",
                ),
            ],
        );
        let at = |text: &str| Filter {
            since: Some(OffsetDateTime::parse(text, &Rfc3339).unwrap()),
            ..Filter::default()
        };
        assert_eq!(
            messages(&read(dir.path(), &at("2026-11-26T12:00:00.500Z"))),
            ["in the closing second"],
            "closed at 12:00:00, which is truncated: it holds lines up to 12:00:00.999"
        );
        assert!(read(dir.path(), &at("2026-11-26T12:00:01.000Z")).is_empty());
        write(
            dir.path(),
            "node-20261126T110000Z.jsonl",
            &[line("2026-11-26T10:59:00.000Z", "info", "x", "oldest")],
        );
        write(
            dir.path(),
            "node.jsonl",
            &[line("2026-11-26T12:30:00.000Z", "info", "x", "newest")],
        );
        assert_eq!(
            messages(&read(dir.path(), &Filter::default())),
            ["oldest", "before", "in the closing second", "newest"],
            "the oldest closed file first, the current one last"
        );
    }

    #[test]
    fn a_directory_with_no_log_has_no_lines() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path(), &Filter::default()).is_empty());
        assert!(read(&dir.path().join("missing"), &Filter::default()).is_empty());
    }
}
