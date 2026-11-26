use super::*;
use crate::memory::ManualClock;
use agency_proto::clock::Duration as Span;
use agency_proto::metrics::SeriesKind::{Call, Client, Serve};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const NOW: &str = "2026-11-24T11:30:00Z";

fn recorder(dir: &Path, clock: &ManualClock) -> Recorder {
    Recorder::new(dir.join("metrics"), Arc::new(clock.clone()))
}

fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

fn usage_route(status: &str) -> SeriesKey {
    SeriesKey::new(Serve, "GET /api/orgs/{org}/teams/{team}/usage", status)
}

fn one_series(metrics: &Metrics) -> &Series {
    assert_eq!(metrics.series.len(), 1, "{:?}", metrics.series);
    &metrics.series[0]
}

#[test]
fn what_is_recorded_is_read_back_as_a_series_with_its_buckets_and_percentiles() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    for millis in [1, 1, 1, 1, 1, 20, 20, 20, 20, 900] {
        recorder.record(usage_route("2xx"), ms(millis), false);
    }
    let metrics = recorder.read(1, None, None);
    let series = one_series(&metrics);
    assert_eq!((series.kind, series.status.as_str()), (Serve, "2xx"));
    assert_eq!(
        (series.count, series.errors, series.sum_ms),
        (10, 0, 5 + 80 + 900)
    );
    assert!(series.is_consistent());
    assert_eq!(
        (series.p50_ms, series.p95_ms, series.p99_ms),
        (Some(1), Some(1000), Some(1000))
    );
    assert_eq!(series.by_hour.len(), 1);
    assert_eq!(series.by_hour[0].hour.as_str(), "2026-11-24T11:00:00Z");
    assert_eq!(
        (metrics.from.as_str(), metrics.until.as_str()),
        ("2026-11-23T12:00:00Z", "2026-11-24T12:00:00Z")
    );
    assert_eq!(metrics.generated_at.as_str(), NOW);
}

#[test]
fn errors_peers_and_clients_are_their_own_series_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    recorder.record(usage_route("5xx"), ms(30), true);
    recorder.record(usage_route("2xx"), ms(30), false);
    recorder.record(
        SeriesKey::new(Call, "peer get_view", "ok").with_peer("b.example"),
        ms(60),
        false,
    );
    recorder.record(
        SeriesKey::new(Call, "peer get_view", "ok").with_peer("a.example"),
        ms(60),
        false,
    );
    recorder.record(
        SeriesKey::new(Client, "GET /x", "network").with_client("ios"),
        ms(5000),
        true,
    );
    let metrics = recorder.read(1, None, None);
    let keys: Vec<(SeriesKind, &str, &str, Option<&str>)> = metrics
        .series
        .iter()
        .map(|s| {
            (
                s.kind,
                s.name.as_str(),
                s.status.as_str(),
                s.peer.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        keys,
        [
            (Serve, "GET /api/orgs/{org}/teams/{team}/usage", "2xx", None),
            (Serve, "GET /api/orgs/{org}/teams/{team}/usage", "5xx", None),
            (Call, "peer get_view", "ok", Some("a.example")),
            (Call, "peer get_view", "ok", Some("b.example")),
            (Client, "GET /x", "network", None),
        ]
    );
    assert_eq!(metrics.series[1].errors, 1);
    assert_eq!(metrics.series[4].client.as_deref(), Some("ios"));
    assert!(metrics.series.iter().all(Series::is_consistent));
}

#[test]
fn a_read_can_be_narrowed_to_a_kind_and_a_name() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    recorder.record(usage_route("2xx"), ms(3), false);
    recorder.record(SeriesKey::new(Call, "runtime view", "ok"), ms(3), false);
    recorder.record(SeriesKey::new(Call, "runtime decide", "ok"), ms(3), false);
    let calls = recorder.read(1, Some(Call), None);
    assert_eq!(calls.series.len(), 2);
    let decide = recorder.read(1, Some(Call), Some("runtime decide"));
    assert_eq!(one_series(&decide).name, "runtime decide");
    assert!(recorder.read(1, Some(Client), None).series.is_empty());
    assert!(recorder.read(1, None, Some("nothing")).series.is_empty());
}

#[test]
fn a_flush_writes_the_hours_to_a_file_of_the_day_that_only_the_account_reads_and_a_new_recorder_reads_it(
) {
    let dir = tempfile::tempdir().unwrap();
    let clock = ManualClock::at(NOW);
    let first = recorder(dir.path(), &clock);
    first.record(usage_route("2xx"), ms(10), false);
    first.record(usage_route("2xx"), ms(10), false);
    first.flush();
    let file = dir.path().join("metrics/2026-11-24.jsonl");
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&dir.path().join("metrics")), 0o700);
    // More in the same hour, and a second flush: the lines add up.
    first.record(usage_route("2xx"), ms(10), false);
    first.flush();
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 2);
    // A node that restarted reads what the first wrote.
    let second = recorder(dir.path(), &clock);
    let metrics = second.read(1, None, None);
    assert_eq!(one_series(&metrics).count, 3);
    assert_eq!(
        metrics.series[0].by_hour.len(),
        1,
        "the lines of the hour are one hour"
    );
    // What is not flushed yet is in a read too, and not twice.
    second.record(usage_route("2xx"), ms(10), false);
    assert_eq!(one_series(&second.read(1, None, None)).count, 4);
    second.flush();
    assert_eq!(one_series(&second.read(1, None, None)).count, 4);
}

#[test]
fn hours_are_read_oldest_first_and_a_range_leaves_out_the_hours_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let clock = ManualClock::at("2026-11-22T10:15:00Z");
    let recorder = recorder(dir.path(), &clock);
    recorder.record(usage_route("2xx"), ms(10), false);
    clock.advance(Span::hours(30));
    recorder.record(usage_route("2xx"), ms(10), false);
    clock.advance(Span::hours(1));
    recorder.record(usage_route("2xx"), ms(10), false);
    recorder.flush();
    let two_days = recorder.read(2, None, None);
    let hours: Vec<&str> = one_series(&two_days)
        .by_hour
        .iter()
        .map(|h| h.hour.as_str())
        .collect();
    assert_eq!(
        hours,
        [
            "2026-11-22T10:00:00Z",
            "2026-11-23T16:00:00Z",
            "2026-11-23T17:00:00Z"
        ]
    );
    let one_day = recorder.read(1, None, None);
    assert_eq!(
        one_series(&one_day).count,
        2,
        "the hour of 30 hours ago is out of one day"
    );
    assert_eq!(one_series(&one_day).by_hour.len(), 2);
}

#[test]
fn files_of_days_past_the_thirty_kept_are_deleted_and_others_are_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let metrics = dir.path().join("metrics");
    std::fs::create_dir_all(&metrics).unwrap();
    for name in [
        "2026-10-25.jsonl",
        "2026-10-26.jsonl",
        "2026-11-23.jsonl",
        "notes.txt",
        "2026-10-01.log",
    ] {
        std::fs::write(metrics.join(name), "").unwrap();
    }
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    recorder.flush();
    let mut names: Vec<String> = std::fs::read_dir(&metrics)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "2026-10-01.log",
            "2026-10-26.jsonl",
            "2026-11-23.jsonl",
            "notes.txt"
        ],
        "30 days end with today (24 November): the 26th of October is the oldest kept, and the 25th is not"
    );
}

#[test]
fn a_line_that_is_cut_or_does_not_add_up_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let metrics = dir.path().join("metrics");
    std::fs::create_dir_all(&metrics).unwrap();
    let buckets = |n: u64| format!("[{n},0,0,0,0,0,0,0,0,0,0,0,0,0,0]");
    let good = format!(
        r#"{{"hour":1795518000,"kind":"serve","name":"GET /a","status":"2xx","count":2,"errors":0,"sumMs":2,"buckets":{}}}"#,
        buckets(2)
    );
    let wrong_sum = format!(
        r#"{{"hour":1795518000,"kind":"serve","name":"GET /b","status":"2xx","count":5,"errors":0,"sumMs":2,"buckets":{}}}"#,
        buckets(2)
    );
    let short = r#"{"hour":1795518000,"kind":"serve","name":"GET /c","status":"2xx","count":1,"errors":0,"sumMs":2,"buckets":[1]}"#;
    let too_many_errors = format!(
        r#"{{"hour":1795518000,"kind":"serve","name":"GET /d","status":"2xx","count":2,"errors":3,"sumMs":2,"buckets":{}}}"#,
        buckets(2)
    );
    let text = [
        good,
        wrong_sum,
        short.to_owned(),
        too_many_errors,
        "{\"hour\":1795518000,\"kind\":\"se".into(),
        "nonsense".into(),
    ]
    .join("\n");
    std::fs::write(metrics.join("2026-11-24.jsonl"), text + "\n").unwrap();
    let read = recorder(dir.path(), &ManualClock::at(NOW)).read(1, None, None);
    assert_eq!(one_series(&read).name, "GET /a");
}

#[test]
fn what_cannot_be_written_is_kept_for_the_next_flush_and_is_in_a_read_meanwhile() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("metrics"),
        "a file where the directory should be",
    )
    .unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    recorder.record(usage_route("2xx"), ms(10), false);
    recorder.flush();
    assert_eq!(
        one_series(&recorder.read(1, None, None)).count,
        1,
        "not lost"
    );
    std::fs::remove_file(dir.path().join("metrics")).unwrap();
    recorder.record(usage_route("2xx"), ms(10), false);
    recorder.flush();
    assert_eq!(one_series(&recorder.read(1, None, None)).count, 2);
    let lines = std::fs::read_to_string(dir.path().join("metrics/2026-11-24.jsonl")).unwrap();
    assert_eq!(
        lines.lines().count(),
        1,
        "both measurements are in the one line of the hour"
    );
}

#[test]
fn an_hour_holds_at_most_so_many_series_and_the_ones_it_has_keep_counting() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    for n in 0..MOST_SERIES_IN_AN_HOUR + 50 {
        recorder.record(
            SeriesKey::new(Serve, format!("GET /r{n:05}"), "2xx"),
            ms(1),
            false,
        );
    }
    recorder.record(SeriesKey::new(Serve, "GET /r00000", "2xx"), ms(1), false);
    let metrics = recorder.read(1, None, None);
    assert_eq!(metrics.series.len(), MOST_SERIES_IN_AN_HOUR);
    assert_eq!(
        metrics.series[0].count, 2,
        "a series that is there keeps counting"
    );
    assert_eq!(
        recorder.totals().len(),
        MOST_SERIES_IN_AN_HOUR + 50,
        "the totals since the start are not capped"
    );
    recorder.flush();
    recorder.record(SeriesKey::new(Serve, "GET /after", "2xx"), ms(1), false);
    assert!(
        recorder.read(1, None, Some("GET /after")).series.len() == 1,
        "a flush makes room"
    );
}

#[test]
fn the_totals_since_the_start_are_not_reset_by_a_flush() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    recorder.record(usage_route("2xx"), ms(10), false);
    recorder.flush();
    recorder.record(usage_route("2xx"), ms(10), true);
    let totals = recorder.totals();
    assert_eq!(totals.len(), 1);
    assert_eq!((totals[0].1.count, totals[0].1.errors), (2, 1));
}

#[test]
fn many_threads_recording_at_once_lose_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let recorder = recorder.clone();
            std::thread::spawn(move || {
                for _ in 0..500 {
                    recorder.record(usage_route("2xx"), ms(3), false);
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(one_series(&recorder.read(1, None, None)).count, 4000);
}

#[test]
fn an_unwritten_hour_before_the_range_and_a_file_of_an_earlier_day_are_not_in_a_read() {
    let dir = tempfile::tempdir().unwrap();
    // The files cannot be written, so the first hour stays pending for days.
    std::fs::write(
        dir.path().join("metrics"),
        "a file where the directory should be",
    )
    .unwrap();
    let clock = ManualClock::at("2026-11-20T10:00:00Z");
    let recorder = recorder(dir.path(), &clock);
    recorder.record(usage_route("2xx"), ms(10), false);
    clock.advance(Span::days(3));
    recorder.record(usage_route("2xx"), ms(10), false);
    assert_eq!(
        one_series(&recorder.read(1, None, None)).count,
        1,
        "the pending hour of 3 days ago is out of one day"
    );
    assert_eq!(one_series(&recorder.read(5, None, None)).count, 2);

    // A file of an earlier day than the range is not even read: a line in it that says it is in the range
    // (which no node writes) is not seen.
    std::fs::remove_file(dir.path().join("metrics")).unwrap();
    std::fs::create_dir_all(dir.path().join("metrics")).unwrap();
    let hour_in_range = 1_795_518_000; // 2026-11-24T10:00Z, inside the range at NOW
    let line = format!(
        r#"{{"hour":{hour_in_range},"kind":"serve","name":"GET /old-file","status":"2xx","count":1,"errors":0,"sumMs":1,"buckets":[1,0,0,0,0,0,0,0,0,0,0,0,0,0,0]}}"#
    );
    std::fs::write(dir.path().join("metrics/2026-11-01.jsonl"), line + "\n").unwrap();
    let fresh = Recorder::new(dir.path().join("metrics"), Arc::new(ManualClock::at(NOW)));
    assert!(fresh.read(1, None, Some("GET /old-file")).series.is_empty());
}
