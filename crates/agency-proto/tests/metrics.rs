//! The metrics contract: the shared sample answer reads and writes back unchanged, its series agree with
//! themselves, and what a series holds is only what the protocol allows.
use agency_proto::metrics::{Metrics, SeriesKind};
use serde_json::Value;

const SAMPLE: &str = include_str!("../../../testdata/metrics.sample.json");

fn sample() -> Metrics {
    serde_json::from_str(SAMPLE).unwrap()
}

#[test]
fn the_sample_round_trips_and_leaves_out_what_has_no_value() {
    let value: Value = serde_json::from_str(SAMPLE).unwrap();
    assert_eq!(serde_json::to_value(sample()).unwrap(), value);
    assert!(
        !SAMPLE.contains("null"),
        "an optional field is left out, never null"
    );
}

#[test]
fn every_series_of_the_sample_agrees_with_itself() {
    for series in &sample().series {
        assert!(
            series.is_consistent(),
            "{} {} {}",
            series.name,
            series.status,
            series.count
        );
    }
}

#[test]
fn the_sample_is_ordered_and_a_percentile_in_the_slowest_bucket_is_left_out() {
    let metrics = sample();
    let keys: Vec<_> = metrics
        .series
        .iter()
        .map(|s| (s.kind, s.name.clone(), s.status.clone()))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "ordered by kind, then name, then status");
    let runtime = metrics
        .series
        .iter()
        .find(|s| s.name == "runtime view")
        .unwrap();
    assert_eq!(
        (runtime.p50_ms, runtime.p95_ms, runtime.p99_ms),
        (Some(500), None, None)
    );
}

#[test]
fn an_inconsistent_series_is_told_apart() {
    let consistent = sample().series[0].clone();
    let mut wrong_count = consistent.clone();
    wrong_count.count += 1;
    let mut wrong_percentile = consistent.clone();
    wrong_percentile.p95_ms = Some(50);
    let mut short = consistent.clone();
    short.buckets.pop();
    let mut too_many_errors = consistent.clone();
    too_many_errors.errors = consistent.count + 1;
    let mut wrong_median = consistent.clone();
    wrong_median.p50_ms = Some(25);
    let mut wrong_p99 = consistent.clone();
    wrong_p99.p99_ms = None;
    for bad in [
        wrong_count,
        wrong_percentile,
        wrong_median,
        wrong_p99,
        short,
        too_many_errors,
    ] {
        assert!(!bad.is_consistent());
    }
    assert!(consistent.is_consistent());
}

#[test]
fn a_series_kind_is_written_in_lower_case_and_the_peer_and_client_only_where_they_apply() {
    let value = serde_json::to_value(sample()).unwrap();
    let series = value["series"].as_array().unwrap();
    let kinds: Vec<&str> = series.iter().map(|s| s["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["serve", "serve", "call", "call", "client", "client"]
    );
    assert_eq!(serde_json::to_value(SeriesKind::Client).unwrap(), "client");
    assert!(series[0].get("peer").is_none() && series[0].get("client").is_none());
    assert_eq!(series[2]["peer"], "node-b.example");
    assert_eq!(series[4]["client"], "web");
}
