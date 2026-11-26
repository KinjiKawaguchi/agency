//! `agency logs`: reads the node's log files (the closed ones and the current one) and prints the lines
//! that match. It reads files only, so it needs neither a running node nor the account key.
use agency_adapters::files::DataDir;
use agency_adapters::logging::reader::{read, Entry, Filter};
use agency_core::logging_config::LogLevel;
use agency_proto::clock::{format, Duration, OffsetDateTime};
use anyhow::{bail, Result};
use clap::Args;
use std::io::Write;

#[derive(Args)]
pub struct LogsArgs {
    /// Only lines from this long ago until now: a number and `s`, `m`, `h`, or `d` (`30m`, `2h`, `1d`)
    #[arg(long)]
    since: Option<String>,
    /// Only lines of this level or a more urgent one: `error`, `warn`, `info`, or `debug`
    #[arg(long)]
    level: Option<String>,
    /// Only lines of this event (`runtime`, `refused`, `access`, `usage`, `push`, and so on)
    #[arg(long)]
    event: Option<String>,
    /// Print the lines as they are in the file (JSON, one on each line), for `jq`
    #[arg(long)]
    json: bool,
}

/// How long ago `text` says: a whole number of seconds, minutes, hours, or days, written with the unit
/// (`90s`, `30m`, `2h`, `1d`). At least 1, and at most 10 years.
fn parse_since(text: &str, now: OffsetDateTime) -> Result<OffsetDateTime> {
    let wrong = || {
        anyhow::anyhow!(
            "--since is a whole number and s, m, h, or d, such as 30m or 2h; not \"{text}\""
        )
    };
    let unit = text.chars().last().ok_or_else(wrong)?;
    let seconds_in_unit: i64 = match unit {
        's' => 1,
        'm' => 60,
        'h' => 3600,
        'd' => 86_400,
        _ => return Err(wrong()),
    };
    let number = &text[..text.len() - 1];
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(wrong());
    }
    let count: i64 = number.parse().map_err(|_| wrong())?;
    let ten_years = 3650 * 86_400;
    match count.checked_mul(seconds_in_unit) {
        Some(seconds) if (1..=ten_years).contains(&seconds) => Ok(now - Duration::seconds(seconds)),
        _ => Err(wrong()),
    }
}

fn filter(args: &LogsArgs, now: OffsetDateTime) -> Result<Filter> {
    let level = match &args.level {
        None => None,
        Some(text) => match LogLevel::parse(text) {
            Some(level) => Some(level),
            None => bail!("--level is error, warn, info, or debug; not \"{text}\""),
        },
    };
    let since = args
        .since
        .as_deref()
        .map(|text| parse_since(text, now))
        .transpose()?;
    Ok(Filter {
        since,
        level,
        event: args.event.clone(),
    })
}

/// A line for a person: the time, the level, the event, and the message.
fn describe(entry: &Entry) -> String {
    format!(
        "{} {:<5} {:<9} {}",
        format(entry.at),
        entry.level.as_str(),
        entry.event,
        entry.message
    )
}

/// The lines to print, oldest first.
fn lines(data: &DataDir, args: &LogsArgs, now: OffsetDateTime) -> Result<Vec<String>> {
    let entries = read(&data.root().join("logs"), &filter(args, now)?);
    Ok(entries
        .iter()
        .map(|entry| {
            if args.json {
                entry.raw.clone()
            } else {
                describe(entry)
            }
        })
        .collect())
}

/// Prints the lines. A reader that stops early (`| head`) is not an error.
pub fn run(data: &DataDir, args: &LogsArgs) -> Result<()> {
    let lines = lines(data, args, OffsetDateTime::now_utc())?;
    let mut out = std::io::stdout().lock();
    for line in lines {
        if writeln!(out, "{line}").is_err() {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::clock::parse;

    fn at(text: &str) -> OffsetDateTime {
        parse(text).unwrap()
    }

    fn now() -> OffsetDateTime {
        at("2026-11-26T12:00:00Z")
    }

    fn args(since: Option<&str>, level: Option<&str>, event: Option<&str>, json: bool) -> LogsArgs {
        LogsArgs {
            since: since.map(str::to_owned),
            level: level.map(str::to_owned),
            event: event.map(str::to_owned),
            json,
        }
    }

    #[test]
    fn since_is_a_whole_number_with_a_unit() {
        assert_eq!(
            parse_since("90s", now()).unwrap(),
            at("2026-11-26T11:58:30Z")
        );
        assert_eq!(
            parse_since("30m", now()).unwrap(),
            at("2026-11-26T11:30:00Z")
        );
        assert_eq!(
            parse_since("2h", now()).unwrap(),
            at("2026-11-26T10:00:00Z")
        );
        assert_eq!(
            parse_since("1d", now()).unwrap(),
            at("2026-11-25T12:00:00Z")
        );
        assert_eq!(
            parse_since("3650d", now()).unwrap(),
            now() - Duration::days(3650)
        );
        for bad in [
            "",
            "m",
            "5",
            "5x",
            "-1h",
            "+1h",
            "1.5h",
            "0m",
            " 1h",
            "1h ",
            "3651d",
            "99999999999999999999d",
            "１h",
        ] {
            let error = parse_since(bad, now()).unwrap_err().to_string();
            assert!(error.contains("--since"), "{bad}: {error}");
        }
    }

    #[test]
    fn a_level_must_be_one_of_the_four() {
        assert_eq!(
            filter(&args(None, Some("warn"), None, false), now())
                .unwrap()
                .level,
            Some(LogLevel::Warn)
        );
        let error = filter(&args(None, Some("loud"), None, false), now())
            .err()
            .unwrap()
            .to_string();
        assert_eq!(
            error,
            "--level is error, warn, info, or debug; not \"loud\""
        );
    }

    fn write_log(dir: &std::path::Path) {
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        let line = |ts: &str, level: &str, event: &str, message: &str| {
            format!(r#"{{"ts":"{ts}","level":"{level}","event":"{event}","message":"{message}"}}"#)
        };
        let text = [
            line("2026-11-26T09:00:00.000Z", "info", "start", "node running"),
            line(
                "2026-11-26T11:30:00.000Z",
                "warn",
                "runtime",
                "team acme/app: the team runtime fails: x",
            ),
            line(
                "2026-11-26T11:45:00.000Z",
                "error",
                "push",
                "a push notification was not sent: y",
            ),
        ]
        .join("\n");
        std::fs::write(dir.join("logs/node.jsonl"), text + "\n").unwrap();
    }

    #[test]
    fn lines_are_for_a_person_or_the_file_s_own() {
        let dir = tempfile::tempdir().unwrap();
        write_log(dir.path());
        let data = DataDir::new(dir.path().to_owned());
        assert_eq!(
            lines(&data, &args(Some("1h"), Some("warn"), None, false), now()).unwrap(),
            [
                "2026-11-26T11:30:00Z warn  runtime   team acme/app: the team runtime fails: x",
                "2026-11-26T11:45:00Z error push      a push notification was not sent: y",
            ]
        );
        let raw = lines(&data, &args(None, None, Some("start"), true), now()).unwrap();
        assert_eq!(
            raw,
            [
                r#"{"ts":"2026-11-26T09:00:00.000Z","level":"info","event":"start","message":"node running"}"#
            ]
        );
        assert!(lines(&data, &args(Some("5m"), None, None, false), now())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_node_that_never_logged_has_no_lines_and_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path().to_owned());
        assert!(lines(&data, &args(None, None, None, false), now())
            .unwrap()
            .is_empty());
    }
}
