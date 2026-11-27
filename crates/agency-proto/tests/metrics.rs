//! The metrics contract: the shared sample answer reads and writes back unchanged, its series agree with
//! themselves, and what a series holds is only what the protocol allows.
use agency_proto::metrics::{
    bucket_total, group_of, is_error_status, is_in_node_read, judge, percentile_bucket,
    percentile_ms, valid_name, valid_status, HourSeries, Metrics, ObjectiveResult, ObjectiveState,
    Series, SeriesKind, BUCKETS, CALL_GROUPS, IN_NODE_READ_P95_BUDGET_MS, ROUTE_GROUPS,
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
        ["serve", "serve", "serve", "serve", "call", "call", "client", "client"]
    );
    assert_eq!(serde_json::to_value(SeriesKind::Client).unwrap(), "client");
    assert!(series[0].get("peer").is_none() && series[0].get("client").is_none());
    assert_eq!(series[4]["peer"], "node-b.example");
    assert_eq!(series[6]["client"], "web");
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
        ("POST /api/telemetry", "metrics"),
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

#[test]
fn the_objectives_of_the_sample_are_what_judging_its_series_gives() {
    let metrics = sample();
    assert_eq!(metrics.objectives.len(), 4);
    for stated in &metrics.objectives {
        assert_eq!(
            &judge(&stated.group, stated.p95_ms, &metrics.series),
            stated,
            "{}",
            stated.group
        );
    }
    let value = serde_json::to_value(&metrics).unwrap();
    let states: Vec<&str> = value["objectives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["state"].as_str().unwrap())
        .collect();
    assert_eq!(
        states,
        ["met", "undetermined", "notMet", "unknown"],
        "each of the four"
    );
    assert!(
        value["objectives"][3].get("measuredP95Ms").is_none(),
        "left out when there is none"
    );
}

#[test]
fn an_objective_is_met_when_the_95th_percentile_of_the_whole_group_is_within_it() {
    let series = sample().series;
    let desk = |p95| judge("desk", p95, &series);
    assert_eq!(
        (desk(10).state, desk(10).measured_p95_ms),
        (ObjectiveState::Met, Some(10)),
        "exactly the objective is met"
    );
    assert_eq!(
        desk(9).state,
        ObjectiveState::Undetermined,
        "p95 is in (5, 10]: 9 is inside it"
    );
    assert_eq!(
        desk(5).state,
        ObjectiveState::NotMet,
        "5 is the bucket's lower bound"
    );
    assert_eq!(desk(30_000).state, ObjectiveState::Met);
    // The group's series are added up: usage has a 2xx series (p95 1000) and a 5xx one (30 000 ms).
    let usage = judge("usage", 30_000, &series);
    assert_eq!(
        (usage.state, usage.measured_p95_ms),
        (ObjectiveState::Met, Some(30_000))
    );
    assert_eq!(
        judge("usage", 29_999, &series).state,
        ObjectiveState::Undetermined
    );
    assert_eq!(
        judge("usage", 10_000, &series).state,
        ObjectiveState::NotMet
    );
    // No calls in the group is not met and not failed.
    let none = judge("evaluation", 100, &series);
    assert_eq!(
        (none.state, none.measured_p95_ms),
        (ObjectiveState::Unknown, None)
    );
    // What is not a `serve` series of the group is not in it: a call of the group's name, a client's report.
    let mut only_others: Vec<Series> = series
        .iter()
        .filter(|s| s.kind != SeriesKind::Serve)
        .cloned()
        .collect();
    only_others.iter_mut().for_each(|s| s.group = "desk".into());
    assert_eq!(
        judge("desk", 10, &only_others).state,
        ObjectiveState::Unknown
    );
    // Slower than the last bound is not met, whatever the objective, and has no measured number.
    let mut slow = series[0].clone();
    slow.buckets = vec![0; 15];
    slow.buckets[14] = 5;
    let beyond = judge("desk", 30_000, &[slow]);
    assert_eq!(
        (beyond.state, beyond.measured_p95_ms),
        (ObjectiveState::NotMet, None)
    );
}

#[test]
fn the_states_are_written_in_camel_case_and_the_field_is_left_out_when_there_are_none() {
    assert_eq!(
        serde_json::to_value(ObjectiveState::NotMet).unwrap(),
        "notMet"
    );
    let mut quiet = sample();
    quiet.objectives.clear();
    assert!(serde_json::to_value(&quiet)
        .unwrap()
        .get("objectives")
        .is_none());
    let read: ObjectiveResult =
        serde_json::from_str(r#"{"group":"desk","p95Ms":5,"state":"met"}"#).unwrap();
    assert_eq!(read.measured_p95_ms, None);
}

/// A group whose only series has ten durations in the bucket with the index `bucket`.
fn group_in_bucket(bucket: usize) -> Vec<Series> {
    let mut series = sample()
        .series
        .into_iter()
        .find(|s| s.group == "desk")
        .unwrap();
    series.buckets = vec![0; 15];
    series.buckets[bucket] = 10;
    vec![series]
}

#[test]
fn an_objective_inside_the_bucket_of_the_95th_percentile_is_undetermined_not_met_and_not_unmet() {
    // The 95th percentile is in the bucket (50, 100]: its upper bound is 100 and its lower bound 50.
    let series = group_in_bucket(6);
    let state = |objective| judge("desk", objective, &series).state;
    assert_eq!(
        state(100),
        ObjectiveState::Met,
        "the upper bound is within the objective"
    );
    assert_eq!(state(101), ObjectiveState::Met);
    assert_eq!(
        state(99),
        ObjectiveState::Undetermined,
        "the objective is inside the bucket"
    );
    assert_eq!(
        state(75),
        ObjectiveState::Undetermined,
        "75 ms with a p95 in (50, 100] is not 'over'"
    );
    assert_eq!(state(51), ObjectiveState::Undetermined);
    assert_eq!(
        state(50),
        ObjectiveState::NotMet,
        "everything in the bucket is above 50"
    );
    assert_eq!(state(10), ObjectiveState::NotMet);
    assert_eq!(
        judge("desk", 75, &series).measured_p95_ms,
        Some(100),
        "the number compared is the upper bound"
    );
}

#[test]
fn the_first_bucket_starts_at_zero_and_the_last_has_no_upper_bound() {
    // The first bucket is (0, 1]: an objective of 1 is met, and nothing is below it to be over.
    let first = group_in_bucket(0);
    assert_eq!(judge("desk", 1, &first).state, ObjectiveState::Met);
    // The second is (1, 2]: an objective of 1 is not met (every duration in it is over 1 ms), 2 is.
    let second = group_in_bucket(1);
    assert_eq!(judge("desk", 1, &second).state, ObjectiveState::NotMet);
    assert_eq!(judge("desk", 2, &second).state, ObjectiveState::Met);
    // The slowest bucket is above 30 000 ms: any objective up to the last bound is not met, and there is no
    // number to say.
    let last = group_in_bucket(14);
    for objective in [1, 29_999, 30_000] {
        let result = judge("desk", objective, &last);
        assert_eq!(
            (result.state, result.measured_p95_ms),
            (ObjectiveState::NotMet, None),
            "{objective}"
        );
    }
    // An objective past the last bound (which a node does not accept) is inside that bucket.
    assert_eq!(
        judge("desk", 40_000, &last).state,
        ObjectiveState::Undetermined
    );
    // The bucket before the last is (10 000, 30 000].
    let near = group_in_bucket(13);
    assert_eq!(judge("desk", 30_000, &near).state, ObjectiveState::Met);
    assert_eq!(
        judge("desk", 29_999, &near).state,
        ObjectiveState::Undetermined
    );
    assert_eq!(judge("desk", 10_000, &near).state, ObjectiveState::NotMet);
}

fn hour_with(count: u64, bucket: usize, p95: Option<u32>) -> HourSeries {
    let mut buckets = vec![0u64; 15];
    buckets[bucket] = count;
    HourSeries {
        hour: agency_proto::Timestamp::parse("2026-11-24T11:00:00Z").unwrap(),
        count,
        errors: 0,
        p95_ms: p95,
        buckets: Some(buckets),
    }
}

#[test]
fn the_buckets_of_an_hour_add_up_to_its_count_and_give_its_percentile() {
    assert!(hour_with(4, 5, Some(50)).is_consistent());
    let mut short = hour_with(4, 5, Some(50));
    short.buckets.as_mut().unwrap().pop();
    assert!(!short.is_consistent(), "15 buckets");
    let mut denied = hour_with(4, 5, Some(50));
    denied.count = 5;
    assert!(!denied.is_consistent(), "they add up to the count");
    assert!(
        !hour_with(4, 5, Some(100)).is_consistent(),
        "the percentile is that of the buckets"
    );
    assert!(!hour_with(4, 5, None).is_consistent());
    // The slowest bucket has no bound, so no percentile.
    assert!(hour_with(2, 14, None).is_consistent());
    // An hour without buckets has nothing to disagree with.
    let mut plain = hour_with(4, 5, Some(999));
    plain.buckets = None;
    assert!(plain.is_consistent());
}

#[test]
fn buckets_that_hold_more_than_a_count_can_are_inconsistent_and_give_no_percentile() {
    assert_eq!(bucket_total(&[u64::MAX, 0, 0]), Some(u64::MAX));
    assert_eq!(bucket_total(&[u64::MAX, 1, 0]), None);
    assert_eq!(percentile_ms(&[u64::MAX, 1, 0], 95), None);
    assert_eq!(percentile_bucket(&[u64::MAX, 0, 0], 95), Some(0));
    // The sum wraps to the count it claims; the hour is still told apart.
    let mut wrapped = hour_with(0, 0, None);
    assert!(wrapped.is_consistent());
    wrapped.buckets.as_mut().unwrap()[0] = u64::MAX;
    wrapped.buckets.as_mut().unwrap()[1] = 1;
    assert!(!wrapped.is_consistent());
    let mut series = sample().series.remove(0);
    series.count = 0;
    series.errors = 0;
    series.by_hour = Vec::new();
    series.buckets = vec![0; BUCKETS];
    series.p50_ms = None;
    series.p95_ms = None;
    series.p99_ms = None;
    assert!(series.is_consistent());
    series.buckets[0] = u64::MAX;
    series.buckets[1] = 1;
    assert!(!series.is_consistent());
}

#[test]
fn a_series_is_consistent_only_when_its_hours_are_and_the_buckets_of_an_hour_are_left_out_when_not_asked_for(
) {
    let mut with = sample().series.remove(0);
    assert!(
        with.by_hour.iter().all(|hour| hour.buckets.is_some()),
        "the sample is an answer asked for them"
    );
    assert!(with.is_consistent());
    with.by_hour[0].buckets.as_mut().unwrap()[0] += 1;
    assert!(
        !with.is_consistent(),
        "an hour that denies itself denies the series"
    );
    let without = sample().series.remove(0).without_hour_buckets();
    assert!(without.by_hour.iter().all(|hour| hour.buckets.is_none()) && without.is_consistent());
    let value = serde_json::to_value(&without).unwrap();
    assert!(
        value["byHour"][0].get("buckets").is_none(),
        "left out, not null"
    );
}

#[test]
fn an_objective_over_series_that_add_up_past_what_a_count_holds_says_nothing() {
    let template = sample()
        .series
        .into_iter()
        .find(|one| one.kind == SeriesKind::Serve && one.group == "desk")
        .unwrap();
    let mut half = template.clone();
    half.buckets = vec![0; BUCKETS];
    half.buckets[3] = u64::MAX / 2 + 2;
    let (mut over, mut exact) = (half.clone(), half.clone());
    over.status = "5xx".into();
    exact.buckets[3] = u64::MAX / 2 - 1;
    let result = judge("desk", 10, &[half.clone(), over]);
    assert_eq!(
        (result.state, result.measured_p95_ms),
        (ObjectiveState::Unknown, None),
        "a wrapped sum would have said 2 durations, in a bucket that nobody measured"
    );
    let result = judge("desk", 10, &[half, exact]);
    assert_ne!(
        result.state,
        ObjectiveState::Unknown,
        "what adds up to exactly a u64's most is still a count"
    );
}
