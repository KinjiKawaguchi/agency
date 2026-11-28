//! What the recorder keeps in memory: the series, their buckets and percentiles, the limits, and the totals.
use super::*;

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
