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
        recorder.record(usage_route("2xx"), ms(millis));
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
    recorder.record(usage_route("5xx"), ms(30));
    recorder.record(usage_route("2xx"), ms(30));
    recorder.record(
        SeriesKey::new(Call, "peer get_view", "ok").with_peer("b.example"),
        ms(60),
    );
    recorder.record(
        SeriesKey::new(Call, "peer get_view", "ok").with_peer("a.example"),
        ms(60),
    );
    recorder.record(
        SeriesKey::new(Client, "GET /x", "network").with_client("ios"),
        ms(5000),
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
    recorder.record(usage_route("2xx"), ms(3));
    recorder.record(SeriesKey::new(Call, "runtime view", "ok"), ms(3));
    recorder.record(SeriesKey::new(Call, "runtime decide", "ok"), ms(3));
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
    first.record(usage_route("2xx"), ms(10));
    first.record(usage_route("2xx"), ms(10));
    first.flush();
    let file = dir.path().join("metrics/2026-11-24.jsonl");
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&dir.path().join("metrics")), 0o700);
    // More in the same hour, and a second flush: the numbers of the line add up.
    first.record(usage_route("2xx"), ms(10));
    first.flush();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap().lines().count(),
        1,
        "the second flush adds to the line of the same series and hour; it does not add a line"
    );
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
    second.record(usage_route("2xx"), ms(10));
    assert_eq!(one_series(&second.read(1, None, None)).count, 4);
    second.flush();
    assert_eq!(one_series(&second.read(1, None, None)).count, 4);
}

#[test]
fn hours_are_read_oldest_first_and_a_range_leaves_out_the_hours_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let clock = ManualClock::at("2026-11-22T10:15:00Z");
    let recorder = recorder(dir.path(), &clock);
    recorder.record(usage_route("2xx"), ms(10));
    clock.advance(Span::hours(30));
    recorder.record(usage_route("2xx"), ms(10));
    clock.advance(Span::hours(1));
    recorder.record(usage_route("2xx"), ms(10));
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
fn a_line_that_is_cut_or_does_not_add_up_or_adds_up_past_what_a_count_holds_is_skipped() {
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
    let overflowing = format!(
        r#"{{"hour":1795518000,"kind":"serve","name":"GET /e","status":"2xx","count":0,"errors":0,"sumMs":2,"buckets":[{},1,0,0,0,0,0,0,0,0,0,0,0,0,0]}}"#,
        u64::MAX
    );
    let text = [
        good,
        wrong_sum,
        short.to_owned(),
        too_many_errors,
        overflowing,
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
    recorder.record(usage_route("2xx"), ms(10));
    recorder.flush();
    assert_eq!(
        one_series(&recorder.read(1, None, None)).count,
        1,
        "not lost"
    );
    std::fs::remove_file(dir.path().join("metrics")).unwrap();
    recorder.record(usage_route("2xx"), ms(10));
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
        recorder.record(SeriesKey::new(Serve, format!("GET /r{n:05}"), "2xx"), ms(1));
    }
    recorder.record(SeriesKey::new(Serve, "GET /r00000", "2xx"), ms(1));
    let metrics = recorder.read(1, None, None);
    assert_eq!(
        metrics.series.len(),
        MOST_SERIES_IN_AN_HOUR + 1,
        "the series there was room for, and the one for the rest"
    );
    assert_eq!(
        metrics.series[0].count, 2,
        "a series that is there keeps counting"
    );
    let overflow = metrics
        .series
        .iter()
        .find(|s| s.name == "overflow")
        .unwrap();
    assert_eq!(
        (overflow.kind, overflow.status.as_str(), overflow.count),
        (Serve, "2xx", 50)
    );
    assert!(metrics.series.iter().all(Series::is_consistent));
    assert_eq!(
        recorder.totals().len(),
        MOST_SERIES_IN_AN_HOUR + 1,
        "the totals since the start are bounded the same way"
    );
    recorder.flush();
    recorder.record(SeriesKey::new(Serve, "GET /after", "2xx"), ms(1));
    assert!(
        recorder.read(1, None, Some("GET /after")).series.len() == 1,
        "a flush makes room"
    );
}

#[test]
fn the_totals_since_the_start_are_not_reset_by_a_flush() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    recorder.record(usage_route("2xx"), ms(10));
    recorder.flush();
    recorder.record(usage_route("2xx"), ms(10));
    recorder.record(usage_route("5xx"), ms(10));
    let totals = recorder.totals();
    let counted: Vec<(u64, u64)> = totals.iter().map(|(_, c)| (c.count, c.errors)).collect();
    assert_eq!(
        counted,
        [(2, 0), (1, 1)],
        "an error is what the status says"
    );
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
                    recorder.record(usage_route("2xx"), ms(3));
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
    recorder.record(usage_route("2xx"), ms(10));
    clock.advance(Span::days(3));
    recorder.record(usage_route("2xx"), ms(10));
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

#[test]
fn nothing_a_requester_could_choose_gets_into_a_name_and_a_mistake_of_the_caller_is_not_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    // Names a requester could make: a query, a method nobody uses, an empty name, a long one.
    for name in [
        "GET /api/me?token=secret".to_string(),
        "BREW /coffee".to_string(),
        String::new(),
        format!("GET /{}", "x".repeat(200)),
        "GET /a\nb".to_string(),
    ] {
        recorder.record(SeriesKey::new(Serve, name, "2xx"), ms(1));
    }
    recorder.record(SeriesKey::new(Serve, "unmatched", "4xx"), ms(1));
    recorder.record(SeriesKey::new(Serve, "OTHER /api/me", "2xx"), ms(1));
    // Mistakes of the caller: a status that is not one of the kind, a client that is not one, a client
    // on a route, a peer on a route (the peer is dropped, the measurement kept).
    recorder.record(SeriesKey::new(Serve, "GET /x", "ok"), ms(1));
    recorder.record(
        SeriesKey::new(Client, "GET /x", "2xx").with_client("android"),
        ms(1),
    );
    recorder.record(SeriesKey::new(Client, "GET /x", "2xx"), ms(1));
    recorder.record(
        SeriesKey::new(Serve, "GET /x", "2xx").with_client("web"),
        ms(1),
    );
    recorder.record(
        SeriesKey::new(Serve, "GET /y", "2xx").with_peer("b.example"),
        ms(1),
    );
    let metrics = recorder.read(1, None, None);
    let seen: Vec<(&str, &str, u64)> = metrics
        .series
        .iter()
        .map(|s| (s.name.as_str(), s.status.as_str(), s.count))
        .collect();
    assert_eq!(
        seen,
        [
            ("GET /y", "2xx", 1),
            ("OTHER /api/me", "2xx", 1),
            ("invalid", "2xx", 5),
            ("unmatched", "4xx", 1),
        ],
        "the five names are one series, `invalid`, and the mistakes are not there"
    );
    assert!(metrics
        .series
        .iter()
        .all(|s| s.peer.is_none() && s.client.is_none()));
    assert!(metrics.series.iter().all(Series::is_consistent));
    let peer = SeriesKey::new(Call, "peer get_view", "ok").with_peer("evil?x=1");
    recorder.record(peer, ms(1));
    let calls = recorder.read(1, Some(Call), None);
    assert_eq!(one_series(&calls).peer.as_deref(), Some("invalid"));
}

#[test]
fn an_error_is_what_the_status_says_and_every_series_read_back_agrees_with_itself() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    for (kind, status) in [
        (Serve, "5xx"),
        (Serve, "4xx"),
        (Call, "timeout"),
        (Call, "error"),
        (Call, "ok"),
        (Client, "network"),
        (Client, "2xx"),
    ] {
        let key = SeriesKey::new(
            kind,
            if kind == Call {
                "runtime view"
            } else {
                "GET /x"
            },
            status,
        );
        let key = if kind == Client {
            key.with_client("web")
        } else {
            key
        };
        recorder.record(key, ms(7));
        recorder.record(SeriesKey::new(Serve, "GET /z", "2xx"), ms(7));
    }
    recorder.flush();
    let again = Recorder::new(dir.path().join("metrics"), Arc::new(ManualClock::at(NOW)));
    for metrics in [recorder.read(1, None, None), again.read(1, None, None)] {
        assert!(
            metrics.series.iter().all(Series::is_consistent),
            "{:?}",
            metrics.series
        );
        let errors: Vec<(&str, u64)> = metrics
            .series
            .iter()
            .filter(|s| s.errors > 0)
            .map(|s| (s.status.as_str(), s.errors))
            .collect();
        assert_eq!(
            errors,
            [("5xx", 1), ("error", 1), ("timeout", 1), ("network", 1)]
        );
    }
}

#[test]
fn the_limit_is_five_hundred_series_and_the_overflow_of_a_call_has_no_peer() {
    assert_eq!(
        MOST_SERIES_IN_AN_HOUR, 500,
        "the number the review settled on"
    );
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    for n in 0..MOST_SERIES_IN_AN_HOUR {
        recorder.record(SeriesKey::new(Serve, format!("GET /r{n:05}"), "2xx"), ms(1));
    }
    recorder.record(
        SeriesKey::new(Call, "peer get_view", "ok").with_peer("a.example"),
        ms(1),
    );
    recorder.record(
        SeriesKey::new(Call, "peer get_view", "ok").with_peer("b.example"),
        ms(1),
    );
    let calls = recorder.read(1, Some(Call), None);
    let overflow = one_series(&calls);
    assert_eq!(
        (
            overflow.name.as_str(),
            overflow.count,
            overflow.peer.as_deref()
        ),
        ("overflow", 2, None)
    );
    assert!(overflow.is_consistent());
}

#[test]
fn a_day_of_flushes_is_a_line_for_each_series_and_hour_and_not_a_line_for_each_flush() {
    let dir = tempfile::tempdir().unwrap();
    let clock = ManualClock::at("2026-11-24T00:00:30Z");
    let recorder = recorder(dir.path(), &clock);
    // A flush every minute of a day, with three series active in each.
    for _ in 0..24 * 60 {
        recorder.record(usage_route("2xx"), ms(10));
        recorder.record(usage_route("5xx"), ms(20));
        recorder.record(SeriesKey::new(Call, "runtime view", "ok"), ms(30));
        recorder.flush();
        clock.advance(Span::minutes(1));
    }
    let lines = std::fs::read_to_string(dir.path().join("metrics/2026-11-24.jsonl")).unwrap();
    assert_eq!(
        lines.lines().count(),
        3 * 24,
        "three series in each of 24 hours, not 1440 flushes"
    );
    assert!(
        std::fs::metadata(dir.path().join("metrics/2026-11-24.jsonl"))
            .unwrap()
            .len()
            < 24 * 3 * 600,
        "and the size is that of the lines"
    );
    let again = Recorder::new(
        dir.path().join("metrics"),
        Arc::new(ManualClock::at("2026-11-24T23:59:00Z")),
    );
    let metrics = again.read(1, None, None);
    assert_eq!(
        metrics.series.iter().map(|s| s.count).collect::<Vec<_>>(),
        [1440, 1440, 1440],
        "nothing lost in the rewrites"
    );
    let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("metrics"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        leftovers.len(),
        1,
        "no file beside the day's: {leftovers:?}"
    );
    let mode = std::fs::metadata(dir.path().join("metrics/2026-11-24.jsonl"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "the rewritten file is as private as the first");
}

#[test]
fn two_flushes_at_once_lose_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    let threads: Vec<_> = (0..4)
        .map(|n| {
            let recorder = recorder.clone();
            std::thread::spawn(move || {
                for _ in 0..50 {
                    recorder.record(SeriesKey::new(Serve, format!("GET /t{n}"), "2xx"), ms(1));
                    recorder.flush();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let again = Recorder::new(dir.path().join("metrics"), Arc::new(ManualClock::at(NOW)));
    let counts: Vec<u64> = again
        .read(1, None, None)
        .series
        .iter()
        .map(|s| s.count)
        .collect();
    assert_eq!(counts, [50, 50, 50, 50]);
}

#[test]
fn the_totals_hold_at_most_so_many_series_over_the_hours_and_the_rest_are_one_overflow() {
    let dir = tempfile::tempdir().unwrap();
    let clock = ManualClock::at(NOW);
    let recorder = recorder(dir.path(), &clock);
    assert_eq!(MOST_SERIES_IN_THE_TOTALS, 500);
    // 600 names, 100 an hour, so that no hour reaches its own limit.
    for hour in 0..6 {
        for n in 0..100 {
            recorder.record(
                SeriesKey::new(Serve, format!("GET /h{hour}n{n:03}"), "2xx"),
                ms(1),
            );
        }
        clock.advance(Span::hours(1));
    }
    let totals = recorder.totals();
    assert_eq!(
        totals.len(),
        MOST_SERIES_IN_THE_TOTALS + 1,
        "the 500 there was room for, and `overflow`"
    );
    let overflow = totals
        .iter()
        .find(|(key, _)| key.name == "overflow")
        .unwrap();
    assert_eq!(overflow.1.count, 100);
    recorder.record(SeriesKey::new(Serve, "GET /h0n000", "2xx"), ms(1));
    let first = totals
        .iter()
        .find(|(key, _)| key.name == "GET /h0n000")
        .unwrap();
    assert_eq!(first.1.count, 1, "a series that is there keeps its own");
    assert_eq!(
        recorder
            .totals()
            .iter()
            .find(|(key, _)| key.name == "GET /h0n000")
            .unwrap()
            .1
            .count,
        2
    );
}

#[test]
fn a_file_with_what_the_recorder_would_refuse_is_read_as_the_recorder_would_have_recorded_it() {
    let dir = tempfile::tempdir().unwrap();
    let metrics = dir.path().join("metrics");
    std::fs::create_dir_all(&metrics).unwrap();
    let buckets = "[1,0,0,0,0,0,0,0,0,0,0,0,0,0,0]";
    let line = |name: &str, status: &str, kind: &str, extra: &str| {
        format!(
            r#"{{"hour":1795518000,"kind":"{kind}","name":"{name}","status":"{status}"{extra},"count":1,"errors":0,"sumMs":1,"buckets":{buckets}}}"#
        )
    };
    let text = [
        line("GET /good", "2xx", "serve", ""),
        line("GET /a?token=secret", "2xx", "serve", ""),
        line("BREW /coffee", "2xx", "serve", ""),
        line("GET /x", "ok", "serve", ""),
        line("GET /x", "2xx", "client", r#","client":"android""#),
        line("GET /x", "2xx", "client", ""),
        line("peer get_view", "ok", "call", r#","peer":"evil?x=1""#),
    ]
    .join("\n");
    std::fs::write(metrics.join("2026-11-24.jsonl"), text + "\n").unwrap();
    let read = Recorder::new(metrics, Arc::new(ManualClock::at(NOW))).read(1, None, None);
    assert!(
        read.series.iter().all(Series::is_consistent),
        "{:?}",
        read.series
    );
    let seen: Vec<(&str, &str, Option<&str>, u64)> = read
        .series
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.status.as_str(),
                s.peer.as_deref(),
                s.count,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("GET /good", "2xx", None, 1),
            ("invalid", "2xx", None, 2),
            ("peer get_view", "ok", Some("invalid"), 1),
        ],
        "the two bad names are one series; a bad status and a bad client are not in it"
    );
}

#[test]
fn a_read_says_the_group_the_budget_and_whether_the_budget_applies() {
    let dir = tempfile::tempdir().unwrap();
    let recorder = recorder(dir.path(), &ManualClock::at(NOW));
    recorder.record(SeriesKey::new(Serve, "GET /api/me", "2xx"), ms(3));
    recorder.record(usage_route("2xx"), ms(3));
    recorder.record(SeriesKey::new(Call, "runtime view", "ok"), ms(3));
    let metrics = recorder.read(1, None, None);
    assert_eq!(metrics.budget.in_node_read_p95_ms, 100);
    let facts: Vec<(&str, &str, bool)> = metrics
        .series
        .iter()
        .map(|s| (s.name.as_str(), s.group.as_str(), s.is_in_node_read))
        .collect();
    assert_eq!(
        facts,
        [
            ("GET /api/me", "desk", true),
            ("GET /api/orgs/{org}/teams/{team}/usage", "usage", false),
            ("runtime view", "runtime", false),
        ]
    );
}

#[test]
fn a_day_file_that_cannot_be_read_is_not_replaced_and_the_flush_keeps_its_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let metrics = dir.path().join("metrics");
    let clock = ManualClock::at(NOW);
    let first = recorder(dir.path(), &clock);
    first.record(usage_route("2xx"), ms(10));
    first.record(usage_route("2xx"), ms(10));
    first.flush();
    let file = metrics.join("2026-11-24.jsonl");
    let before = std::fs::read_to_string(&file).unwrap();
    // The file stops being readable (a permission, a mount): not the same as there being none.
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    let second = recorder(dir.path(), &clock);
    second.record(usage_route("2xx"), ms(10));
    second.flush();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        before,
        "the file it could not read was not replaced"
    );
    assert_eq!(
        one_series(&second.read(1, None, None)).count,
        3,
        "its numbers are kept, and the file's are read once it can be"
    );
    // The next flush finds the file again and adds to it: nothing of either is lost.
    second.flush();
    assert_eq!(one_series(&second.read(1, None, None)).count, 3);
    let third = recorder(dir.path(), &clock);
    assert_eq!(one_series(&third.read(1, None, None)).count, 3);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 1);
}

#[test]
fn torn_bytes_in_a_day_file_cost_the_line_they_are_in_and_not_the_day() {
    let dir = tempfile::tempdir().unwrap();
    let metrics = dir.path().join("metrics");
    std::fs::create_dir_all(&metrics).unwrap();
    let line = |name: &str, count: u64| {
        format!(
            r#"{{"hour":1795518000,"kind":"serve","name":"{name}","status":"2xx","count":{count},"errors":0,"sumMs":{count},"buckets":[{count},0,0,0,0,0,0,0,0,0,0,0,0,0,0]}}"#
        )
    };
    let mut bytes = line("GET /before", 2).into_bytes();
    bytes.extend_from_slice(b"\n{\"hour\":17955\xff\xfe\xc3(\n");
    bytes.extend_from_slice(line("GET /after", 4).as_bytes());
    bytes.push(b'\n');
    std::fs::write(metrics.join("2026-11-24.jsonl"), bytes).unwrap();
    let recorder = Recorder::new(metrics.clone(), Arc::new(ManualClock::at(NOW)));
    let names = |recorder: &Recorder| -> Vec<(String, u64)> {
        recorder
            .read(1, None, None)
            .series
            .iter()
            .map(|s| (s.name.clone(), s.count))
            .collect()
    };
    assert_eq!(
        names(&recorder),
        [("GET /after".to_owned(), 4), ("GET /before".to_owned(), 2)]
    );
    recorder.record(usage_route("2xx"), ms(10));
    recorder.flush();
    let after = names(&Recorder::new(metrics, Arc::new(ManualClock::at(NOW))));
    assert_eq!(
        after,
        [
            ("GET /after".to_owned(), 4),
            ("GET /api/orgs/{org}/teams/{team}/usage".to_owned(), 1),
            ("GET /before".to_owned(), 2)
        ],
        "the day's other lines are kept when the file is rewritten"
    );
}

#[test]
fn merging_counts_never_wraps_around_to_a_small_number() {
    let big = |count: u64| {
        let mut buckets = [0; BUCKETS];
        buckets[3] = count;
        Counts {
            count,
            errors: count,
            sum_ms: 0,
            buckets,
        }
    };
    let mut merged = big(u64::MAX / 2 + 1);
    merged.merge(&big(u64::MAX / 2 + 1));
    assert_eq!(
        (merged.count, merged.errors, merged.buckets[3]),
        (u64::MAX, u64::MAX, u64::MAX),
        "it stops at what a count holds"
    );
}
