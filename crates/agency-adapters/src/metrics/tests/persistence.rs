//! What the recorder writes to the files of the days and reads back: the hours, the files kept and deleted, and what a file
//! that is cut or refused costs.
use super::*;

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
