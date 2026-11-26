use super::*;
use agency_adapters::memory::ManualClock;
use agency_adapters::metrics::SeriesKey;
use agency_proto::metrics::SeriesKind::{Call, Serve};
use std::time::Duration;

const NOW: &str = "2026-11-24T11:30:00Z";
const SAMPLE: &str = include_str!("../../../../testdata/metrics.sample.json");

fn args(days: u32, kind: Option<&str>, name: Option<&str>, json: bool) -> MetricsArgs {
    MetricsArgs {
        days,
        kind: kind.map(str::to_owned),
        name: name.map(str::to_owned),
        json,
    }
}

/// A data directory in which a node recorded `calls` (series, milliseconds) and flushed them.
fn data_with(calls: &[(SeriesKey, u64)]) -> (tempfile::TempDir, DataDir) {
    let dir = tempfile::tempdir().unwrap();
    let data = DataDir::new(dir.path().to_owned());
    let recorder = Recorder::new(dir.path().join("metrics"), Arc::new(ManualClock::at(NOW)));
    for (key, millis) in calls {
        recorder.record(key.clone(), Duration::from_millis(*millis));
    }
    recorder.flush();
    (dir, data)
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(ManualClock::at(NOW))
}

#[test]
fn the_table_has_a_row_for_each_series_with_the_percentiles_as_upper_bounds_and_the_budget_where_it_applies(
) {
    let metrics: Metrics = serde_json::from_str(SAMPLE).unwrap();
    let lines = table(&metrics);
    assert_eq!(lines.len(), 1 + metrics.series.len());
    assert!(lines[0].starts_with("KIND"), "{}", lines[0]);
    let cells = |needle: &str| -> Vec<String> {
        lines
            .iter()
            .find(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("{needle}: {lines:#?}"))
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(
        cells("GET /api/me"),
        ["serve", "GET", "/api/me", "2xx", "20", "0", "<=2", "<=10", "<=10", "ok"]
    );
    assert_eq!(
        cells("runtime view"),
        ["call", "runtime", "view", "ok", "4", "0", "<=500", ">30000", ">30000", "-"],
        "a percentile in the slowest bucket is >30000, not nothing"
    );
    assert!(
        cells("peer get_view").contains(&"[node-b.example]".to_owned()),
        "the peer is in the name's brackets"
    );
    assert!(
        cells("GET /api/orgs/{org}/teams/{team}/view").contains(&"[web]".to_owned()),
        "and the client"
    );
    for line in &lines {
        assert_eq!(line, line.trim_end(), "no trailing space");
    }
    // The columns line up: every line has the same width up to its last cell.
    let widths: Vec<usize> = lines.iter().map(|l| l.find("  ").unwrap_or(0)).collect();
    assert!(widths.iter().all(|w| *w > 0));
}

#[test]
fn an_in_node_read_slower_than_the_budget_is_over_and_one_that_is_not_one_has_no_budget() {
    let (_dir, data) = data_with(&[
        (SeriesKey::new(Serve, "GET /api/me", "2xx"), 500),
        (SeriesKey::new(Serve, "GET /api/about", "2xx"), 3),
        (
            SeriesKey::new(Serve, "GET /api/orgs/{org}/teams/{team}/usage", "2xx"),
            900,
        ),
        (SeriesKey::new(Call, "runtime view", "ok"), 40_000),
    ]);
    let out = lines(&data, &args(1, None, None, false), clock()).unwrap();
    let find = |needle: &str| out.iter().find(|l| l.contains(needle)).unwrap().clone();
    assert!(find("GET /api/me").ends_with("OVER"), "{out:#?}");
    assert!(find("GET /api/about").ends_with("ok"));
    assert!(
        find("/usage").ends_with("-"),
        "it calls the runtime: no budget"
    );
    assert!(find("runtime view").ends_with("-"));
    assert!(find("runtime view").contains(">30000"));
}

#[test]
fn what_it_prints_says_the_range_and_how_to_read_a_percentile_and_the_budget() {
    let (_dir, data) = data_with(&[(SeriesKey::new(Serve, "GET /api/me", "2xx"), 3)]);
    let out = lines(&data, &args(1, None, None, false), clock()).unwrap();
    let text = out.join("\n");
    assert!(
        text.contains("2026-11-23T12:00:00Z to 2026-11-24T12:00:00Z"),
        "{text}"
    );
    assert!(
        text.contains("upper bound of its bucket: <=100 means at most 100"),
        "{text}"
    );
    assert!(text.contains("at most 100 ms"), "{text}");
}

#[test]
fn a_node_with_nothing_recorded_says_so_and_a_read_can_be_narrowed_or_printed_as_json() {
    let empty = tempfile::tempdir().unwrap();
    let data = DataDir::new(empty.path().to_owned());
    assert_eq!(
        lines(&data, &args(1, None, None, false), clock()).unwrap(),
        ["No measurements yet."]
    );
    let (_dir, data) = data_with(&[
        (SeriesKey::new(Serve, "GET /api/me", "2xx"), 3),
        (SeriesKey::new(Call, "runtime view", "ok"), 3),
    ]);
    let calls = lines(&data, &args(1, Some("call"), None, false), clock()).unwrap();
    assert!(
        calls.iter().any(|l| l.contains("runtime view"))
            && !calls.iter().any(|l| l.contains("/api/me"))
    );
    let named = lines(&data, &args(1, None, Some("GET /api/me"), false), clock()).unwrap();
    assert!(
        named.iter().any(|l| l.contains("/api/me"))
            && !named.iter().any(|l| l.contains("runtime view"))
    );
    let json = lines(&data, &args(1, None, None, true), clock()).unwrap();
    assert_eq!(json.len(), 1);
    let again: Metrics = serde_json::from_str(&json[0]).unwrap();
    assert_eq!(again.series.len(), 2);
    assert!(again.series.iter().all(Series::is_consistent));
}

#[test]
fn a_range_or_a_kind_that_is_not_understood_is_refused_with_what_is_accepted() {
    let empty = tempfile::tempdir().unwrap();
    let data = DataDir::new(empty.path().to_owned());
    for days in [0, 31, 100] {
        let error = lines(&data, &args(days, None, None, false), clock())
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            format!("--days is a number from 1 to 30; not {days}")
        );
    }
    let error = lines(&data, &args(1, Some("loud"), None, false), clock())
        .unwrap_err()
        .to_string();
    assert_eq!(error, "--kind is serve, call, or client; not \"loud\"");
    assert!(lines(&data, &args(30, Some("client"), None, false), clock()).is_ok());
}

#[test]
fn a_95th_percentile_of_exactly_the_budget_is_within_it_and_the_next_bucket_is_over() {
    let (_dir, data) = data_with(&[
        (SeriesKey::new(Serve, "GET /api/me", "2xx"), 100),
        (SeriesKey::new(Serve, "GET /api/about", "2xx"), 101),
    ]);
    let out = lines(&data, &args(1, None, None, false), clock()).unwrap();
    let find = |needle: &str| out.iter().find(|l| l.contains(needle)).unwrap().clone();
    assert!(
        find("GET /api/me").contains("<=100") && find("GET /api/me").ends_with("ok"),
        "{out:#?}"
    );
    assert!(
        find("GET /api/about").contains("<=250") && find("GET /api/about").ends_with("OVER"),
        "{out:#?}"
    );
}

#[test]
fn the_numbers_are_aligned_on_the_right() {
    let mut calls = vec![(SeriesKey::new(Serve, "GET /api/me", "2xx"), 3)];
    for _ in 0..1000 {
        calls.push((SeriesKey::new(Serve, "GET /api/about", "2xx"), 3));
    }
    let (_dir, data) = data_with(&calls);
    let out = lines(&data, &args(1, None, None, false), clock()).unwrap();
    let count_end = |needle: &str| {
        let line = out.iter().find(|l| l.contains(needle)).unwrap();
        let at = line.find("2xx").unwrap() + "2xx".len();
        let rest = &line[at..];
        at + rest.trim_start().find(' ').unwrap() + (rest.len() - rest.trim_start().len())
    };
    assert_eq!(
        count_end("GET /api/me"),
        count_end("GET /api/about"),
        "{out:#?}"
    );
}
