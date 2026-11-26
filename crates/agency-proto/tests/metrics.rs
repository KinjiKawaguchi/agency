//! The metrics contract: the shared sample answer reads and writes back unchanged, its series agree with
//! themselves, and what a series holds is only what the protocol allows.
use agency_proto::metrics::{
    is_error_status, valid_name, valid_status, Metrics, Series, SeriesKind,
};
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
        .map(|s| {
            (
                s.kind,
                s.name.clone(),
                s.status.clone(),
                s.peer.clone(),
                s.client.clone(),
            )
        })
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

fn first() -> Series {
    sample().series[0].clone()
}

fn series_of(kind: SeriesKind) -> Series {
    sample()
        .series
        .into_iter()
        .find(|s| s.kind == kind)
        .unwrap()
}

#[test]
fn a_name_is_a_template_or_a_fixed_phrase_and_never_what_a_requester_could_choose() {
    use SeriesKind::{Call, Client, Serve};
    for good in [
        "GET /api/me",
        "OTHER /api/me",
        "POST /fed/v1/teams/{org}/{team}/envelopes",
        "unmatched",
        "overflow",
        "invalid",
    ] {
        assert!(valid_name(Serve, good), "{good}");
        assert!(valid_name(Client, good), "{good}");
    }
    for bad in [
        "",
        "/api/me",
        "GET",
        "GET ",
        "get /api/me",
        "TRACE /api/me",
        "BREW /coffee",
        "GET /api/me?token=abc",
        "GET /api/me#frag",
        "GET /a\nb",
        "GET /a\u{7f}b",
    ] {
        assert!(!valid_name(Serve, bad), "{bad:?}");
    }
    assert!(valid_name(Serve, &format!("GET /{}", "x".repeat(115))));
    assert!(
        !valid_name(Serve, &format!("GET /{}", "x".repeat(117))),
        "121 characters"
    );
    assert!(
        valid_name(Call, "runtime view")
            && valid_name(Call, "peer get_view")
            && valid_name(Call, "overflow")
    );
    assert!(
        !valid_name(Call, "") && !valid_name(Call, "runtime?view") && !valid_name(Call, "a\tb")
    );
}

#[test]
fn a_status_is_one_of_the_set_of_its_kind_and_a_series_has_the_extra_fields_only_where_they_apply()
{
    use SeriesKind::{Call, Client, Serve};
    for (kind, status, valid) in [
        (Serve, "2xx", true),
        (Serve, "5xx", true),
        (Serve, "ok", false),
        (Serve, "network", false),
        (Serve, "500", false),
        (Call, "ok", true),
        (Call, "timeout", true),
        (Call, "error", true),
        (Call, "4xx", true),
        (Call, "network", false),
        (Client, "network", true),
        (Client, "2xx", true),
        (Client, "ok", false),
        (Client, "timeout", false),
    ] {
        assert_eq!(valid_status(kind, status), valid, "{kind:?} {status}");
    }
    let mut serve = first();
    serve.status = "ok".into();
    let mut peer_on_a_route = first();
    peer_on_a_route.peer = Some("b.example".into());
    let mut client_on_a_route = first();
    client_on_a_route.client = Some("web".into());
    let mut other_client = series_of(Client);
    other_client.client = Some("android".into());
    let mut no_client = series_of(Client);
    no_client.client = None;
    let mut bad_name = first();
    bad_name.name = "GET /api/me?token=abc".into();
    for bad in [
        serve,
        peer_on_a_route,
        client_on_a_route,
        other_client,
        no_client,
        bad_name,
    ] {
        assert!(!bad.is_consistent(), "{bad:?}");
    }
}

#[test]
fn errors_are_all_of_the_calls_or_none_as_the_status_says() {
    use SeriesKind::{Call, Client, Serve};
    assert!(
        is_error_status(Serve, "5xx")
            && !is_error_status(Serve, "4xx")
            && !is_error_status(Serve, "2xx")
    );
    assert!(
        is_error_status(Call, "error")
            && is_error_status(Call, "timeout")
            && is_error_status(Call, "5xx")
            && !is_error_status(Call, "ok")
    );
    assert!(is_error_status(Client, "network") && !is_error_status(Serve, "network"));
    assert!(
        !is_error_status(Serve, "timeout")
            && !is_error_status(Client, "error")
            && !is_error_status(Client, "timeout")
    );
    let five_xx = sample()
        .series
        .into_iter()
        .find(|s| s.status == "5xx")
        .unwrap();
    assert_eq!(five_xx.errors, five_xx.count);
    let mut some = five_xx.clone();
    some.errors = some.count - 1;
    let mut errors_on_a_success = first();
    errors_on_a_success.errors = 1;
    assert!(!some.is_consistent() && !errors_on_a_success.is_consistent());
    assert!(five_xx.is_consistent() && first().is_consistent());
}
