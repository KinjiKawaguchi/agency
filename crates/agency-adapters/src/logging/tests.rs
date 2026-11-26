use super::*;
use crate::memory::ManualClock;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const NOW: &str = "2026-11-26T04:55:22.123Z";

fn logger_in(dir: &Path, settings: LoggingConfig, clock: &ManualClock) -> Logger {
    Logger::new(dir.join("logs"), settings, Arc::new(clock.clone()), false)
}

fn lines_of(path: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("every line is JSON"))
        .collect()
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join("logs"))
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_line_is_json_with_the_time_level_event_and_message_in_that_order() {
    let dir = tempfile::tempdir().unwrap();
    let logger = logger_in(dir.path(), LoggingConfig::default(), &ManualClock::at(NOW));
    logger.warn(
        "runtime",
        "team acme/app: the team runtime fails: \"x\"\nsecond line",
    );
    let path = dir.path().join("logs/node.jsonl");
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        text,
        "{\"ts\":\"2026-11-26T04:55:22.123Z\",\"level\":\"warn\",\"event\":\"runtime\",\"message\":\"team acme/app: the team runtime fails: \\\"x\\\"\\nsecond line\"}\n"
    );
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o600, "only the node's account reads it");
    assert_eq!(mode(&dir.path().join("logs")), 0o700);
}

#[test]
fn a_line_more_verbose_than_the_level_is_not_written() {
    let clock = ManualClock::at(NOW);
    let dir = tempfile::tempdir().unwrap();
    let logger = logger_in(dir.path(), LoggingConfig::default(), &clock);
    logger.record(LogLevel::Debug, "x", "debug");
    assert!(
        !dir.path().join("logs").exists(),
        "nothing written, nothing created"
    );
    logger.record(LogLevel::Info, "x", "info");
    logger.error("x", "error");
    let quiet = LoggingConfig {
        level: LogLevel::Warn,
        ..LoggingConfig::default()
    };
    let other = tempfile::tempdir().unwrap();
    let logger_quiet = logger_in(other.path(), quiet, &clock);
    logger_quiet.record(LogLevel::Info, "x", "info");
    logger_quiet.warn("x", "warn");
    let levels = |dir: &Path| -> Vec<String> {
        lines_of(&dir.join("logs/node.jsonl"))
            .iter()
            .map(|line| line["level"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(levels(dir.path()), ["info", "error"]);
    assert_eq!(levels(other.path()), ["warn"]);
    let debug = LoggingConfig {
        level: LogLevel::Debug,
        ..LoggingConfig::default()
    };
    let third = tempfile::tempdir().unwrap();
    logger_in(third.path(), debug, &clock).record(LogLevel::Debug, "x", "debug");
    assert_eq!(levels(third.path()), ["debug"]);
}

/// About 1 MiB of lines, each about 1 KiB.
fn fill(logger: &Logger, lines: usize) {
    let filler = "x".repeat(1000);
    for number in 0..lines {
        logger.warn("fill", &format!("{number} {filler}"));
    }
}

const ONE_MIB: LoggingConfig = LoggingConfig {
    level: LogLevel::Info,
    max_size_mib: 1,
    max_age_days: 14,
    max_files: 50,
};

#[test]
fn the_file_is_closed_before_a_line_that_would_pass_the_size_and_no_line_is_cut() {
    let clock = ManualClock::at(NOW);
    let dir = tempfile::tempdir().unwrap();
    let logger = logger_in(dir.path(), ONE_MIB, &clock);
    fill(&logger, 1500);
    assert_eq!(
        names(dir.path()),
        ["node-20261126T045522Z.jsonl", "node.jsonl"]
    );
    let closed = lines_of(&dir.path().join("logs/node-20261126T045522Z.jsonl"));
    let current = lines_of(&dir.path().join("logs/node.jsonl"));
    assert_eq!(
        closed.len() + current.len(),
        1500,
        "every line is in one of them, whole"
    );
    let size = std::fs::metadata(dir.path().join("logs/node-20261126T045522Z.jsonl"))
        .unwrap()
        .len();
    assert!(size <= 1024 * 1024, "{size} bytes");
    assert!(closed[0]["message"].as_str().unwrap().starts_with("0 "));
    assert_eq!(
        current[0]["message"].as_str().unwrap().split(' ').next(),
        Some(closed.len().to_string().as_str()),
        "the new file starts where the old one ended"
    );
}

#[test]
fn files_closed_in_the_same_second_keep_their_own_names() {
    let clock = ManualClock::at(NOW);
    let dir = tempfile::tempdir().unwrap();
    let logger = logger_in(dir.path(), ONE_MIB, &clock);
    fill(&logger, 3200);
    assert_eq!(
        names(dir.path()),
        [
            "node-20261126T045522Z-1.jsonl",
            "node-20261126T045522Z-2.jsonl",
            "node-20261126T045522Z.jsonl",
            "node.jsonl"
        ]
    );
    let total: usize = names(dir.path())
        .iter()
        .map(|name| lines_of(&dir.path().join("logs").join(name)).len())
        .sum();
    assert_eq!(total, 3200, "none overwritten");
}

#[test]
fn old_files_and_files_past_the_count_are_deleted_when_the_log_starts_and_when_it_rotates() {
    let clock = ManualClock::at(NOW);
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    for name in [
        "node-20261101T000000Z.jsonl",
        "node-20261120T000000Z.jsonl",
        "node-20261125T000000Z.jsonl",
        "access.jsonl",
    ] {
        std::fs::write(logs.join(name), "{}\n").unwrap();
    }
    let settings = LoggingConfig {
        max_files: 2,
        ..ONE_MIB
    };
    let logger = logger_in(dir.path(), settings.clone(), &clock);
    logger.warn("x", "first");
    assert_eq!(
        names(dir.path()),
        [
            "access.jsonl",
            "node-20261120T000000Z.jsonl",
            "node-20261125T000000Z.jsonl",
            "node.jsonl"
        ],
        "past 14 days goes at the start, and files that are not the node's log are left alone"
    );
    fill(&logger, 1100);
    assert_eq!(
        names(dir.path()),
        [
            "access.jsonl",
            "node-20261125T000000Z.jsonl",
            "node-20261126T045522Z.jsonl",
            "node.jsonl"
        ],
        "a rotation keeps the newest two closed files"
    );
}

#[test]
fn a_log_that_cannot_be_written_does_not_stop_the_node() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("logs");
    std::fs::write(&blocker, "a file where the directory should be").unwrap();
    let logger = logger_in(dir.path(), LoggingConfig::default(), &ManualClock::at(NOW));
    logger.warn("x", "goes to standard error instead");
    logger.warn("x", "and again");
    std::fs::remove_file(&blocker).unwrap();
    logger.warn("x", "and is written once the directory can be made");
    assert_eq!(lines_of(&dir.path().join("logs/node.jsonl")).len(), 1);
}

#[test]
fn a_sink_writes_each_line_at_its_level_and_event_without_the_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let logger = logger_in(dir.path(), LoggingConfig::default(), &ManualClock::at(NOW));
    let sink = logger.sink(LogLevel::Warn, "runtime");
    sink("agency: team acme/app: the team runtime answers again");
    sink("no prefix here");
    let lines = lines_of(&dir.path().join("logs/node.jsonl"));
    let seen: Vec<(&str, &str, &str)> = lines
        .iter()
        .map(|l| {
            (
                l["level"].as_str().unwrap(),
                l["event"].as_str().unwrap(),
                l["message"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                "warn",
                "runtime",
                "team acme/app: the team runtime answers again"
            ),
            ("warn", "runtime", "no prefix here")
        ]
    );
}

#[test]
fn a_very_long_message_is_cut() {
    let dir = tempfile::tempdir().unwrap();
    let logger = logger_in(dir.path(), LoggingConfig::default(), &ManualClock::at(NOW));
    logger.warn("x", &"é".repeat(10_000));
    let lines = lines_of(&dir.path().join("logs/node.jsonl"));
    assert_eq!(
        lines[0]["message"].as_str().unwrap().chars().count(),
        MESSAGE_LIMIT
    );
}

#[test]
fn lines_written_at_once_by_many_threads_do_not_mix() {
    let dir = tempfile::tempdir().unwrap();
    let logger = logger_in(dir.path(), LoggingConfig::default(), &ManualClock::at(NOW));
    let threads: Vec<_> = (0..8)
        .map(|thread| {
            let logger = logger.clone();
            std::thread::spawn(move || {
                for number in 0..100 {
                    logger.warn("t", &format!("{thread}-{number}"));
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(lines_of(&dir.path().join("logs/node.jsonl")).len(), 800);
}
