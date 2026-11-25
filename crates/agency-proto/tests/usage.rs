//! The token usage contract: the shared sample report, the `tokenUsage` runtime op, and the checks a node
//! makes before it passes a report on.
use agency_proto::runtime::Describe;
use agency_proto::usage::{valid_days, TokenUsage};
use agency_proto::{Id, RuntimeCall, RuntimeRequest};
use serde_json::{json, Value};

const SAMPLE: &str = include_str!("../../../testdata/tokenusage.sample.json");

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn sample() -> TokenUsage {
    serde_json::from_str(SAMPLE).unwrap()
}

fn check(usage: &TokenUsage) -> Result<(), String> {
    usage.check(&id("acme"), &id("app"), 14)
}

#[test]
fn sample_report_round_trips_and_passes_the_checks() {
    let value: Value = serde_json::from_str(SAMPLE).unwrap();
    assert_eq!(serde_json::to_value(sample()).unwrap(), value);
    assert_eq!(check(&sample()), Ok(()));
}

#[test]
fn token_usage_is_an_optional_op_with_its_range_inline() {
    let request = RuntimeRequest::new(id("acme"), id("app"), RuntimeCall::TokenUsage { days: 14 });
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({ "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "tokenUsage", "days": 14 })
    );
    let with: Describe = serde_json::from_value(
        json!({ "runtimes": ["agency-runtime/1"], "ops": ["describe", "view", "decide", "tokenUsage"], "label": "App" }),
    )
    .unwrap();
    let without = Describe {
        ops: vec!["describe".into(), "view".into(), "decide".into()],
        ..with.clone()
    };
    assert!(with.supports("tokenUsage"));
    assert!(!without.supports("tokenUsage"));
}

#[test]
fn ranges_are_one_to_ninety_days() {
    assert!(valid_days(1));
    assert!(valid_days(90));
    assert!(!valid_days(0));
    assert!(!valid_days(91));
}

#[test]
fn refuses_a_report_of_another_team_or_range() {
    assert!(sample().check(&id("other"), &id("app"), 14).is_err());
    assert!(sample().check(&id("acme"), &id("other"), 14).is_err());
    assert!(sample().check(&id("acme"), &id("app"), 7).is_err());
}

#[test]
fn refuses_days_beyond_the_range_or_out_of_order() {
    let mut report = sample();
    report.days = 1;
    assert!(
        report.check(&id("acme"), &id("app"), 1).is_err(),
        "2 days in a 1-day range"
    );

    let mut reversed = sample();
    reversed.by_day.reverse();
    assert!(check(&reversed).is_err(), "newest first");

    let mut repeated = sample();
    repeated.by_day[1].date = repeated.by_day[0].date.clone();
    assert!(check(&repeated).is_err(), "the same day twice");
}

type Edit = fn(&mut TokenUsage);

#[test]
fn refuses_malformed_costs_and_unnamed_people() {
    let edits: [(&str, Edit); 5] = [
        ("negative day cost", |usage| {
            usage.by_day[0].cost_usd.insert("lead".into(), -0.01);
        }),
        ("infinite day cost", |usage| {
            usage.by_day[0]
                .cost_usd
                .insert("lead".into(), f64::INFINITY);
        }),
        ("negative person cost", |usage| {
            usage.by_person[0].cost_usd = -1.0
        }),
        ("unnamed day entry", |usage| {
            usage.by_day[0].cost_usd.insert(String::new(), 1.0);
        }),
        ("unnamed person", |usage| {
            usage.by_person[0].person = String::new()
        }),
    ];
    for (name, edit) in edits {
        let mut usage = sample();
        edit(&mut usage);
        assert!(check(&usage).is_err(), "{name}");
    }
}

#[test]
fn token_counts_and_dates_must_be_well_formed() {
    let cases: [(&str, Value); 4] = [
        ("/byPerson/0/tokens/input", json!(-1)),
        ("/byPerson/0/tokens/output", json!(1.5)),
        ("/byDay/0/date", json!("2026-11-23T00:00:00Z")),
        ("/byPerson/0/costUsd", json!("8.45")),
    ];
    for (pointer, bad) in cases {
        let mut value: Value = serde_json::from_str(SAMPLE).unwrap();
        *value.pointer_mut(pointer).unwrap() = bad;
        assert!(
            serde_json::from_value::<TokenUsage>(value).is_err(),
            "{pointer}"
        );
    }
}
