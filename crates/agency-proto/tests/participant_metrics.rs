//! The participant metrics contract: the shared sample report, the `participantMetrics` runtime op, and
//! the checks a node makes before it uses a report.
use agency_proto::participant_metrics::{valid_utc_offset, ParticipantMetrics, PullRequestAuthor};
use agency_proto::runtime::Describe;
use agency_proto::usage::TokenUsage;
use agency_proto::{Id, RuntimeCall, RuntimeRequest};
use serde_json::{json, Value};

const SAMPLE: &str = include_str!("../../../testdata/participantmetrics.sample.json");
const USAGE_SAMPLE: &str = include_str!("../../../testdata/tokenusage.sample.json");

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn sample() -> ParticipantMetrics {
    serde_json::from_str(SAMPLE).unwrap()
}

fn check(report: &ParticipantMetrics) -> Result<(), String> {
    report.check(&id("acme"), &id("app"), 2, Some(540))
}

#[test]
fn sample_report_round_trips_and_passes_the_checks() {
    let value: Value = serde_json::from_str(SAMPLE).unwrap();
    assert_eq!(serde_json::to_value(sample()).unwrap(), value);
    assert_eq!(check(&sample()), Ok(()));
    assert_eq!(
        sample().check(&id("acme"), &id("app"), 2, None),
        Ok(()),
        "a node that asked for no time zone takes the runtime's"
    );
}

#[test]
fn the_sample_costs_and_tokens_are_those_of_the_token_usage_sample() {
    let usage: TokenUsage = serde_json::from_str(USAGE_SAMPLE).unwrap();
    for person in &usage.by_person {
        let report = sample();
        let days = &report
            .participants
            .iter()
            .find(|one| one.name == person.person)
            .unwrap()
            .by_day;
        let cost: f64 = days.iter().map(|day| day.cost_usd).sum();
        assert!((cost - person.cost_usd).abs() < 1e-9, "{}", person.person);
        let input: u64 = days.iter().map(|day| day.tokens.input).sum();
        let cache_read: u64 = days.iter().map(|day| day.tokens.cache_read).sum();
        assert_eq!(input, person.tokens.input, "{}", person.person);
        assert_eq!(cache_read, person.tokens.cache_read, "{}", person.person);
    }
}

#[test]
fn participant_metrics_is_an_optional_op_with_its_range_and_time_zone_inline() {
    let call = |offset| RuntimeCall::ParticipantMetrics {
        days: 14,
        utc_offset_minutes: offset,
    };
    let request = |offset| RuntimeRequest::new(id("acme"), id("app"), call(offset));
    assert_eq!(
        serde_json::to_value(request(Some(540))).unwrap(),
        json!({ "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "participantMetrics", "days": 14, "utcOffsetMinutes": 540 })
    );
    assert_eq!(
        serde_json::to_value(request(None)).unwrap(),
        json!({ "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "participantMetrics", "days": 14 }),
        "no time zone asked for: none is sent"
    );
    let read: RuntimeRequest = serde_json::from_value(
        json!({ "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "participantMetrics", "days": 7 }),
    )
    .unwrap();
    assert_eq!(
        read,
        RuntimeRequest::new(
            id("acme"),
            id("app"),
            RuntimeCall::ParticipantMetrics {
                days: 7,
                utc_offset_minutes: None
            }
        )
    );
    let describe: Describe = serde_json::from_value(
        json!({ "runtimes": ["agency-runtime/1"], "ops": ["describe", "view", "decide", "participantMetrics"], "label": "App" }),
    )
    .unwrap();
    assert!(describe.supports("participantMetrics"));
}

#[test]
fn time_zones_are_utc_minus_twelve_to_plus_fourteen() {
    assert!(
        valid_utc_offset(-720)
            && valid_utc_offset(840)
            && valid_utc_offset(0)
            && valid_utc_offset(345)
    );
    assert!(!valid_utc_offset(-721) && !valid_utc_offset(841));
}

#[test]
fn refuses_a_report_of_another_team_range_or_time_zone() {
    assert!(sample()
        .check(&id("other"), &id("app"), 2, Some(540))
        .is_err());
    assert!(sample()
        .check(&id("acme"), &id("other"), 2, Some(540))
        .is_err());
    assert!(sample()
        .check(&id("acme"), &id("app"), 7, Some(540))
        .is_err());
    assert!(
        sample().check(&id("acme"), &id("app"), 2, Some(0)).is_err(),
        "cut elsewhere than asked"
    );
    let mut nowhere = sample();
    nowhere.utc_offset_minutes = 900;
    assert!(nowhere.check(&id("acme"), &id("app"), 2, None).is_err());
}

type Edit = fn(&mut ParticipantMetrics);

#[test]
fn refuses_a_report_that_is_not_well_formed() {
    let edits: [(&str, Edit); 14] = [
        ("an unnamed participant", |r| {
            r.participants[0].name = String::new()
        }),
        ("the same participant twice", |r| {
            r.participants[1].name = "backend".into()
        }),
        ("more days than the range", |r| {
            let day = r.participants[0].by_day[0].clone();
            r.participants[0].by_day.push(day);
        }),
        ("days newest first", |r| r.participants[0].by_day.reverse()),
        ("the same day twice", |r| {
            r.participants[0].by_day[1].date = r.participants[0].by_day[0].date.clone();
        }),
        ("more time than a day holds", |r| {
            r.participants[0].by_day[0].busy_seconds = 90_001
        }),
        ("the times add up past a day", |r| {
            r.participants[0].by_day[0].busy_seconds = 50_000;
            r.participants[0].by_day[0].idle_seconds = 50_000;
        }),
        ("a negative cost", |r| {
            r.participants[0].by_day[0].cost_usd = -0.01
        }),
        ("an infinite cost", |r| {
            r.participants[0].by_day[0].cost_usd = f64::INFINITY
        }),
        ("a pull request without an author", |r| {
            r.pull_requests[0].by = String::new()
        }),
        ("a pull request twice", |r| {
            r.pull_requests[1] = r.pull_requests[0].clone()
        }),
        ("a pull request number zero", |r| {
            r.pull_requests[0].number = 0
        }),
        ("a repository that is not owner/name", |r| {
            r.pull_requests[0].repo = "acme".into()
        }),
        ("a repository with a path", |r| {
            r.pull_requests[0].repo = "acme/app/extra".into()
        }),
    ];
    for (name, edit) in edits {
        let mut report = sample();
        edit(&mut report);
        assert!(check(&report).is_err(), "{name}");
    }
    let some_day_of_25_hours = {
        let mut report = sample();
        report.participants[0].by_day[0].busy_seconds = 90_000;
        report.participants[0].by_day[0].waiting_seconds = 0;
        report.participants[0].by_day[0].idle_seconds = 0;
        report
    };
    assert_eq!(
        check(&some_day_of_25_hours),
        Ok(()),
        "the day the clocks go back"
    );
    let other = PullRequestAuthor {
        repo: "acme/web.app_2".into(),
        number: 7,
        by: "lead".into(),
    };
    let mut with_other = sample();
    with_other.pull_requests.push(other);
    assert_eq!(
        check(&with_other),
        Ok(()),
        "dots, underscores, and dashes are in repository names"
    );
}

#[test]
fn counts_dates_and_the_pull_request_number_must_be_well_formed() {
    let cases: [(&str, Value); 6] = [
        ("/participants/0/byDay/0/busySeconds", json!(-1)),
        ("/participants/0/byDay/0/idleSeconds", json!(1.5)),
        ("/participants/0/byDay/0/tokens/input", json!(-1)),
        (
            "/participants/0/byDay/0/date",
            json!("2026-11-23T00:00:00Z"),
        ),
        ("/pullRequests/0/number", json!("101")),
        ("/generatedAt", json!("yesterday")),
    ];
    for (pointer, bad) in cases {
        let mut value: Value = serde_json::from_str(SAMPLE).unwrap();
        *value.pointer_mut(pointer).unwrap() = bad;
        assert!(
            serde_json::from_value::<ParticipantMetrics>(value).is_err(),
            "{pointer}"
        );
    }
}
