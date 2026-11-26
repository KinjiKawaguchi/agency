//! The log's files: the one being written, `node.jsonl`, and the closed ones, `node-<UTC time>.jsonl`.

use std::path::{Path, PathBuf};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};

/// The file being written.
pub const CURRENT: &str = "node.jsonl";

const STAMP: &[FormatItem<'static>] =
    format_description!("[year][month][day]T[hour][minute][second]Z");

/// A closed file: when it was closed, and which of those closed in the same second it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Closed {
    pub path: PathBuf,
    pub at: OffsetDateTime,
    pub nth: u32,
}

/// The name a file closed at `at` gets: `node-20261126T045522Z.jsonl`, and `-1`, `-2`, ... for the
/// ones closed in the same second after it.
pub fn closed_name(at: OffsetDateTime, nth: u32) -> String {
    let stamp = at
        .to_offset(time::UtcOffset::UTC)
        .format(STAMP)
        .unwrap_or_default();
    match nth {
        0 => format!("node-{stamp}.jsonl"),
        _ => format!("node-{stamp}-{nth}.jsonl"),
    }
}

/// When a file named by `closed_name` was closed, and its number; `None` for any other name.
fn parse_closed(name: &str) -> Option<(OffsetDateTime, u32)> {
    let middle = name.strip_prefix("node-")?.strip_suffix(".jsonl")?;
    let (stamp, nth) = match middle.split_once('-') {
        Some((stamp, nth)) => (stamp, nth.parse().ok()?),
        None => (middle, 0),
    };
    let at = PrimitiveDateTime::parse(stamp, STAMP).ok()?;
    Some((at.assume_utc(), nth))
}

/// The closed files in `dir`, the newest first. Other files are left alone.
pub fn closed_files(dir: &Path) -> Vec<Closed> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut files: Vec<Closed> = entries
        .flatten()
        .filter_map(|entry| {
            let (at, nth) = parse_closed(entry.file_name().to_str()?)?;
            Some(Closed {
                path: entry.path(),
                at,
                nth,
            })
        })
        .collect();
    files.sort_by(|a, b| (b.at, b.nth).cmp(&(a.at, a.nth)));
    files
}

/// The closed files to delete: all but the newest `max_files`, and any closed more than `max_age`
/// before `now`. `files` is newest first.
pub fn expired(
    files: &[Closed],
    now: OffsetDateTime,
    max_age: Duration,
    max_files: usize,
) -> Vec<&Closed> {
    files
        .iter()
        .enumerate()
        .filter(|(position, file)| *position >= max_files || now - file.at > max_age)
        .map(|(_, file)| file)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn a_closed_name_says_when_and_reads_back() {
        let at = datetime!(2026-11-26 04:55:22 UTC);
        assert_eq!(closed_name(at, 0), "node-20261126T045522Z.jsonl");
        assert_eq!(closed_name(at, 2), "node-20261126T045522Z-2.jsonl");
        assert_eq!(parse_closed("node-20261126T045522Z.jsonl"), Some((at, 0)));
        assert_eq!(parse_closed("node-20261126T045522Z-2.jsonl"), Some((at, 2)));
        for other in [
            "node.jsonl",
            "node-x.jsonl",
            "access.jsonl",
            "node-20261126T045522Z.log",
            "node-20261126T045522Z-a.jsonl",
        ] {
            assert_eq!(parse_closed(other), None, "{other}");
        }
        let offset = at.to_offset(time::UtcOffset::from_hms(9, 0, 0).unwrap());
        assert_eq!(closed_name(offset, 0), closed_name(at, 0), "always in UTC");
    }

    fn file(at: OffsetDateTime, nth: u32) -> Closed {
        Closed {
            path: closed_name(at, nth).into(),
            at,
            nth,
        }
    }

    #[test]
    fn the_oldest_and_the_surplus_expire() {
        let now = datetime!(2026-11-26 12:00:00 UTC);
        let files = [
            file(datetime!(2026-11-26 11:00:00 UTC), 0),
            file(datetime!(2026-11-20 00:00:00 UTC), 0),
            file(datetime!(2026-11-12 11:59:59 UTC), 0),
            file(datetime!(2026-11-01 00:00:00 UTC), 0),
        ];
        let names = |kept: Vec<&Closed>| kept.iter().map(|f| f.at.day()).collect::<Vec<_>>();
        // Past 14 days: the last two; the count is not reached.
        assert_eq!(names(expired(&files, now, Duration::days(14), 10)), [12, 1]);
        // Exactly 14 days is not past.
        let edge = [file(datetime!(2026-11-12 12:00:00 UTC), 0)];
        assert!(expired(&edge, now, Duration::days(14), 10).is_empty());
        // Past the count: all but the newest two, whatever their age.
        assert_eq!(names(expired(&files, now, Duration::days(365), 2)), [12, 1]);
        assert_eq!(
            names(expired(&files, now, Duration::days(365), 1)),
            [20, 12, 1]
        );
        assert!(expired(&[], now, Duration::days(14), 1).is_empty());
    }

    #[test]
    fn closed_files_are_listed_newest_first_and_others_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "node-20261125T000000Z.jsonl",
            "node-20261126T000000Z.jsonl",
            "node-20261126T000000Z-1.jsonl",
            "node.jsonl",
            "access.jsonl",
            "notes.txt",
        ] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        let names: Vec<String> = closed_files(dir.path())
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "node-20261126T000000Z-1.jsonl",
                "node-20261126T000000Z.jsonl",
                "node-20261125T000000Z.jsonl"
            ]
        );
        assert!(closed_files(&dir.path().join("missing")).is_empty());
    }
}
