//! The metrics contract: the shared sample answer reads and writes back unchanged, its series agree with
//! themselves, and what a series holds is only what the protocol allows.
use agency_proto::metrics::{
    group_of, is_error_status, is_in_node_read, valid_name, valid_status, Metrics, Series,
    SeriesKind, CALL_GROUPS, IN_NODE_READ_P95_BUDGET_MS, ROUTE_GROUPS,
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
        ["serve", "serve", "serve", "call", "call", "client", "client"]
    );
    assert_eq!(serde_json::to_value(SeriesKind::Client).unwrap(), "client");
    assert!(series[0].get("peer").is_none() && series[0].get("client").is_none());
    assert_eq!(series[3]["peer"], "node-b.example");
    assert_eq!(series[5]["client"], "web");
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

#[test]
fn a_route_is_in_the_group_of_its_template_and_a_call_in_the_group_of_its_first_word() {
    use SeriesKind::{Call, Client, Serve};
    for (name, group) in [
        ("GET /api/me", "desk"),
        ("GET /api/session", "desk"),
        ("GET /api/decisions", "desk"),
        ("GET /api/about", "desk"),
        ("GET /api/orgs/{org}", "teams"),
        ("GET /api/orgs/{org}/teams/{team}/view", "teams"),
        ("GET /api/orgs/{org}/teams/{team}/activity", "teams"),
        ("POST /api/orgs/{org}/teams/{team}/envelopes", "teams"),
        ("GET /api/orgs/{org}/teams/{team}/usage", "usage"),
        ("GET /api/orgs/{org}/evaluation", "evaluation"),
        ("GET /api/orgs/{org}/effort", "evaluation"),
        ("GET /api/orgs/{org}/outcomes", "outcomes"),
        ("POST /api/orgs/{org}/outcomes/observations", "outcomes"),
        ("GET /api/devices", "devices"),
        ("POST /api/devices/requests", "devices"),
        ("GET /api/push/vapid", "push"),
        ("DELETE /api/push/tokens/{token}", "push"),
        ("GET /fed/v1/teams/{org}/{team}/view", "federation"),
        ("POST /fed/v1/orgs/{org}/join", "federation"),
        ("GET /api/metrics", "metrics"),
        ("GET /metrics", "metrics"),
        ("unmatched", "unmatched"),
        ("GET /api/something/new", "other"),
        ("GET /api/mentions", "other"),
        ("GET /api/me/extra", "other"),
        ("GET /{*path}", "other"),
        ("overflow", "other"),
        ("invalid", "other"),
    ] {
        assert_eq!(group_of(Serve, name), group, "{name}");
        assert_eq!(
            group_of(Client, name),
            group,
            "a client reports the route it called: {name}"
        );
        assert!(ROUTE_GROUPS.contains(&group_of(Serve, name)));
    }
    for (name, group) in [
        ("runtime view", "runtime"),
        ("runtime token_usage", "runtime"),
        ("peer get_view", "peer"),
        ("peer post_envelope", "peer"),
        ("apns send", "push"),
        ("webpush send", "push"),
        ("github graphql", "github"),
        ("something else", "other"),
        ("overflow", "other"),
    ] {
        assert_eq!(group_of(Call, name), group, "{name}");
        assert!(CALL_GROUPS.contains(&group_of(Call, name)));
    }
}

#[test]
fn the_budget_applies_to_the_reads_a_node_answers_from_its_own_state_and_to_no_others() {
    use SeriesKind::{Call, Client, Serve};
    for name in [
        "GET /api/me",
        "GET /api/session",
        "GET /api/decisions",
        "GET /api/devices",
        "GET /api/push/vapid",
        "GET /api/orgs/{org}",
        "GET /api/orgs/{org}/teams/{team}/view",
        "GET /api/orgs/{org}/outcomes",
        "GET /fed/v1/orgs/{org}/about",
        "GET /fed/v1/orgs/{org}/versions",
        "GET /fed/v1/teams/{org}/{team}/view",
    ] {
        assert!(is_in_node_read(Serve, name), "{name}");
    }
    for name in [
        "GET /api/orgs/{org}/teams/{team}/usage",
        "GET /api/orgs/{org}/teams/{team}/activity",
        "GET /api/orgs/{org}/evaluation",
        "GET /fed/v1/teams/{org}/{team}/usage",
        "GET /fed/v1/teams/{org}/{team}/evaluation",
        "POST /api/orgs/{org}/teams/{team}/envelopes",
        "POST /api/devices/requests",
        "DELETE /api/push/tokens/{token}",
        "GET /api/metrics",
        "unmatched",
        "overflow",
    ] {
        assert!(!is_in_node_read(Serve, name), "{name}");
    }
    assert!(
        !is_in_node_read(Client, "GET /api/me"),
        "what a client observed includes its network"
    );
    assert!(!is_in_node_read(Call, "runtime view"));
    assert_eq!(IN_NODE_READ_P95_BUDGET_MS, 100);
    assert_eq!(sample().budget.in_node_read_p95_ms, 100);
}

#[test]
fn a_series_that_says_another_group_or_another_budget_than_its_name_is_refused() {
    let mut other_group = first();
    other_group.group = "usage".into();
    let mut budget_where_there_is_none = series_of(SeriesKind::Call);
    budget_where_there_is_none.is_in_node_read = true;
    let mut no_budget = first();
    no_budget.is_in_node_read = false;
    for bad in [other_group, budget_where_there_is_none, no_budget] {
        assert!(!bad.is_consistent(), "{bad:?}");
    }
}
