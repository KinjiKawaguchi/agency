//! The TeamView and agency-runtime/1 contracts: the shared sample fixture, and the JSON shapes in docs/protocol.md.
use agency_proto::runtime::{Decided, Describe, RuntimeError};
use agency_proto::view::{PersonState, RequestKind, WaitingOn, WorkStatus};
use agency_proto::{
    EnvelopeKind, Id, Payload, RuntimeCall, RuntimeDecision, RuntimeRequest, RuntimeResponse,
    TeamView,
};
use serde_json::{json, Value};

const SAMPLE: &str = include_str!("../../../testdata/teamview.sample.json");

fn sample_value() -> Value {
    serde_json::from_str(SAMPLE).unwrap()
}

#[test]
fn sample_teamview_round_trips_without_losing_or_adding_fields() {
    let view: TeamView = serde_json::from_str(SAMPLE).unwrap();
    assert_eq!(serde_json::to_value(&view).unwrap(), sample_value());
}

#[test]
fn sample_teamview_covers_every_variant_clients_render() {
    let view: TeamView = serde_json::from_str(SAMPLE).unwrap();
    for state in [
        PersonState::Working,
        PersonState::Idle,
        PersonState::Away,
        PersonState::Unknown,
    ] {
        assert!(
            view.people.iter().any(|one| one.state == state),
            "{state:?}"
        );
    }
    assert!(view
        .people
        .iter()
        .any(|one| matches!(one.waiting_on, Some(WaitingOn::Person { .. }))));
    assert!(view
        .people
        .iter()
        .any(|one| matches!(one.waiting_on, Some(WaitingOn::Lead))));
    for kind in [
        RequestKind::Question,
        RequestKind::Proposal,
        RequestKind::PendingDecision,
    ] {
        assert!(view.requests.iter().any(|one| one.kind == kind), "{kind:?}");
    }
    for status in [
        WorkStatus::Backlog,
        WorkStatus::Next,
        WorkStatus::InProgress,
        WorkStatus::Release,
        WorkStatus::Done,
    ] {
        assert!(
            view.work.iter().any(|one| one.status == status),
            "{status:?}"
        );
    }
    assert!(view.epics.iter().any(|one| one.forecast.is_none()));
}

#[test]
fn teamview_rejects_values_outside_the_contract() {
    let cases: [(&str, Value); 5] = [
        ("/org", json!("Acme")),
        ("/generatedAt", json!("2026-11-24T21:00:00+09:00")),
        ("/people/0/state", json!("waiting")),
        ("/work/1/blocked/since", json!("2026-11-23T00:00:00Z")),
        ("/requests/0/answerWith/kind", json!("approve")),
    ];
    for (pointer, bad) in cases {
        let mut value = sample_value();
        *value.pointer_mut(pointer).unwrap() = bad;
        assert!(
            serde_json::from_value::<TeamView>(value).is_err(),
            "{pointer}"
        );
    }
}

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

#[test]
fn runtime_requests_carry_the_version_and_the_op_inline() {
    let describe = RuntimeRequest::new(id("acme"), id("app"), RuntimeCall::Describe);
    assert_eq!(
        serde_json::to_value(&describe).unwrap(),
        json!({ "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "describe" })
    );

    let decision = RuntimeDecision {
        kind: EnvelopeKind::Judge,
        target: "app#14".into(),
        by: id("owner"),
        by_participant: Some("owner".into()),
        payload: Payload {
            text: "Drop it".into(),
            choice: None,
        },
    };
    let decide = RuntimeRequest::new(id("acme"), id("app"), RuntimeCall::Decide { decision });
    let expected = json!({
        "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "decide",
        "decision": { "kind": "judge", "target": "app#14", "by": "owner", "byParticipant": "owner", "payload": { "text": "Drop it" } }
    });
    assert_eq!(serde_json::to_value(&decide).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<RuntimeRequest>(expected).unwrap(),
        decide
    );
}

#[test]
fn runtime_responses_are_either_a_result_or_an_error() {
    let ok: RuntimeResponse<Decided> = serde_json::from_value(json!({ "result": {} })).unwrap();
    assert_eq!(ok, RuntimeResponse::Result(Decided {}));
    let failed: RuntimeResponse<Decided> =
        serde_json::from_value(json!({ "error": { "status": 404, "message": "no such request" } }))
            .unwrap();
    assert_eq!(
        failed,
        RuntimeResponse::Error(RuntimeError {
            status: 404,
            message: "no such request".into()
        })
    );
    assert!(serde_json::from_value::<RuntimeResponse<Decided>>(json!({ "ok": true })).is_err());
    let view: RuntimeResponse<TeamView> =
        serde_json::from_value(json!({ "result": sample_value() })).unwrap();
    assert!(matches!(view, RuntimeResponse::Result(_)));
}

#[test]
fn only_malformed_and_missing_targets_are_refusals() {
    let error = |status| RuntimeError {
        status,
        message: String::new(),
    };
    assert!(error(400).is_refusal());
    assert!(error(404).is_refusal());
    for retried in [408, 409, 429, 500, 503] {
        assert!(!error(retried).is_refusal(), "{retried}");
    }
}

#[test]
fn describe_tells_whether_a_runtime_speaks_a_version() {
    let described: Describe = serde_json::from_value(json!({ "runtimes": ["agency-runtime/1"], "ops": ["describe", "view", "decide", "later"], "label": "App" })).unwrap();
    assert!(described.speaks(agency_proto::RUNTIME_VERSION));
    assert!(!described.speaks("agency-runtime/2"));
}
