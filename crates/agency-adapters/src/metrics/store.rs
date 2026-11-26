//! The files of the node's measurements: one file for each UTC day, `YYYY-MM-DD.jsonl`, with a line for each
//! series and hour that was flushed. A line holds what was recorded since the flush before it, so the lines
//! of one series and hour add up. Only the node's account can read the files.

use super::{Counts, Hour, SeriesKey};
use agency_proto::metrics::{SeriesKind, BUCKETS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use time::format_description::FormatItem;
use time::macros::format_description;
use time::{Date, Duration, OffsetDateTime};

const DAY: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]");

/// One line of a file.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Line {
    /// The start of the hour, in Unix seconds.
    hour: Hour,
    kind: SeriesKind,
    name: String,
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    peer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client: Option<String>,
    count: u64,
    errors: u64,
    sum_ms: u64,
    buckets: Vec<u64>,
}

fn date_of(hour: Hour) -> Option<Date> {
    Some(OffsetDateTime::from_unix_timestamp(hour).ok()?.date())
}

fn file_name(date: Date) -> String {
    format!("{}.jsonl", date.format(DAY).unwrap_or_default())
}

fn date_in(name: &str) -> Option<Date> {
    Date::parse(name.strip_suffix(".jsonl")?, DAY).ok()
}

/// Appends `pending` to the files of its days. The entries that could not be written are given back.
pub(super) fn append(
    dir: &Path,
    pending: BTreeMap<(Hour, SeriesKey), Counts>,
) -> Result<(), BTreeMap<(Hour, SeriesKey), Counts>> {
    let mut by_day: BTreeMap<Option<Date>, BTreeMap<(Hour, SeriesKey), Counts>> = BTreeMap::new();
    for (key, counts) in pending {
        by_day
            .entry(date_of(key.0))
            .or_default()
            .insert(key, counts);
    }
    let mut unwritten = BTreeMap::new();
    for (date, entries) in by_day {
        if write_day(dir, date, &entries).is_err() {
            unwritten.extend(entries);
        }
    }
    if unwritten.is_empty() {
        Ok(())
    } else {
        Err(unwritten)
    }
}

fn write_day(
    dir: &Path,
    date: Option<Date>,
    entries: &BTreeMap<(Hour, SeriesKey), Counts>,
) -> std::io::Result<()> {
    let date = date.ok_or_else(|| std::io::Error::other("an hour that is not a date"))?;
    let mut directory = std::fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut directory, 0o700);
    directory.create(dir)?;
    // The day's file is rewritten with the new numbers added to the lines of the same series and hour, so it
    // holds a line for each series and hour and not one for each flush: it grows with the hours, not with
    // the number of flushes. It is written beside itself and moved into place, so a reader or a stop in the
    // middle sees the old file or the new one whole.
    let path = dir.join(file_name(date));
    let mut merged = read_file(&path)?;
    for (key, counts) in entries {
        merged.entry(key.clone()).or_default().merge(counts);
    }
    let mut text = String::new();
    for ((hour, key), counts) in &merged {
        let line = Line {
            hour: *hour,
            kind: key.kind,
            name: key.name.clone(),
            status: key.status.clone(),
            peer: key.peer.clone(),
            client: key.client.clone(),
            count: counts.count,
            errors: counts.errors,
            sum_ms: counts.sum_ms,
            buckets: counts.buckets.to_vec(),
        };
        text.push_str(&serde_json::to_string(&line).map_err(std::io::Error::other)?);
        text.push('\n');
    }
    let temporary = dir.join(format!(".{}.tmp", file_name(date)));
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&temporary)?.write_all(text.as_bytes())?;
    std::fs::rename(&temporary, &path)
}

/// The lines of one file, added up by hour and series: none when there is no such file, and an error when
/// there is one that cannot be read (a flush then keeps its numbers rather than replace what it could not
/// read). A line that is not whole (torn bytes, which are read as they are and not as a reason to give up
/// on the file), does not have 15 buckets that add up to its count, has more errors than calls, or has a
/// status or client that no series may have is skipped; one whose name no series may have is counted as
/// `invalid`, as the recorder would have.
fn read_file(path: &Path) -> std::io::Result<BTreeMap<(Hour, SeriesKey), Counts>> {
    let mut hours = BTreeMap::new();
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(hours),
        Err(error) => return Err(error),
    };
    let text = String::from_utf8_lossy(&bytes);
    for line in text
        .lines()
        .filter_map(|line| serde_json::from_str::<Line>(line).ok())
    {
        if line.buckets.len() != BUCKETS {
            continue;
        }
        let mut counts = Counts {
            count: line.count,
            errors: line.errors,
            sum_ms: line.sum_ms,
            ..Counts::default()
        };
        counts.buckets.copy_from_slice(&line.buckets);
        if counts.buckets.iter().sum::<u64>() != counts.count || counts.errors > counts.count {
            continue;
        }
        let key = SeriesKey {
            kind: line.kind,
            name: line.name,
            status: line.status,
            peer: line.peer,
            client: line.client,
        };
        let Some(key) = super::normalized(key) else {
            continue;
        };
        hours.entry((line.hour, key)).or_default().merge(&counts);
    }
    Ok(hours)
}

/// The lines of the files from the day of `first_hour` on, added up by hour and series, from `first_hour` on.
pub(super) fn read(dir: &Path, first_hour: Hour) -> BTreeMap<(Hour, SeriesKey), Counts> {
    let mut hours: BTreeMap<(Hour, SeriesKey), Counts> = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return hours;
    };
    let first_day = date_of(first_hour);
    for entry in entries.flatten() {
        let Some(day) = entry.file_name().to_str().and_then(date_in) else {
            continue;
        };
        if first_day.is_some_and(|first| day < first) {
            continue;
        }
        // A file that cannot be read is not in an answer; a flush will not replace it.
        for ((hour, key), counts) in read_file(&entry.path()).unwrap_or_default() {
            if hour >= first_hour {
                hours.entry((hour, key)).or_default().merge(&counts);
            }
        }
    }
    hours
}

/// Deletes the files of days before the `kept_days` days that end with the day of `now` (Unix seconds).
/// Only files named like the node's are touched; one that cannot be deleted is tried again at the next flush.
pub(super) fn prune(dir: &Path, now: i64, kept_days: u32) {
    let Some(today) = date_of(now) else {
        return;
    };
    let oldest_kept = today - Duration::days(i64::from(kept_days) - 1);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .file_name()
            .to_str()
            .and_then(date_in)
            .is_some_and(|day| day < oldest_kept);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}
