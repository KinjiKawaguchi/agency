//! The TeamView and agency-runtime/1 contracts: the shared sample fixture, and the JSON shapes in docs/protocol.md.
use agency_proto::runtime::{Decided, Describe, RuntimeError};
use agency_proto::view::{PersonState, RequestKind, WaitingOn, WorkStatus, WorkType};
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

/// docs/glossary.md, "Work type": the wire names, and a work item that has none leaves the field out.
#[test]
fn work_types_have_the_glossary_wire_names() {
    for (name, work_type) in [
        ("newFeature", WorkType::NewFeature),
        ("improvement", WorkType::Improvement),
        ("maintenance", WorkType::Maintenance),
        ("debt", WorkType::Debt),
        ("incident", WorkType::Incident),
    ] {
        assert_eq!(serde_json::to_value(work_type).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<WorkType>(json!(name)).unwrap(),
            work_type
        );
    }
    let view: TeamView = serde_json::from_str(SAMPLE).unwrap();
    assert!(view.work.iter().any(|item| item.work_type.is_some()));
    assert!(view.work.iter().any(|item| item.work_type.is_none()));
}

/// docs/protocol.md, "Work items": `doneAt` on a done item and on a finished epic, left out otherwise.
#[test]
fn done_at_is_optional_on_work_items_and_epics() {
    let view: TeamView = serde_json::from_str(SAMPLE).unwrap();
    assert!(view.work.iter().any(|item| item.done_at.is_some()));
    assert!(view.work.iter().any(|item| item.done_at.is_none()));
    let mut value = sample_value();
    value["epics"][1]["doneAt"] = json!("2026-11-21T10:00:00Z");
    let view: TeamView = serde_json::from_value(value.clone()).unwrap();
    assert!(view.epics[1].done_at.is_some() && view.epics[0].done_at.is_none());
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        value,
        "written back as read"
    );
    let mut bad = sample_value();
    bad["work"][4]["doneAt"] = json!("2026-11-20");
    assert!(
        serde_json::from_value::<TeamView>(bad).is_err(),
        "a time, not a date"
    );
}

#[test]
fn teamview_rejects_values_outside_the_contract() {
    let cases: [(&str, Value); 7] = [
        ("/org", json!("Acme")),
        ("/generatedAt", json!("2026-11-24T21:00:00+09:00")),
        ("/people/0/state", json!("waiting")),
        ("/work/1/blocked/since", json!("2026-11-23T00:00:00Z")),
        ("/requests/0/answerWith/kind", json!("approve")),
        ("/requests/0/thread/0/by", json!("Alice Smith")),
        ("/work/0/workType", json!("bug")),
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
        on_behalf_of: None,
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

#[test]
fn a_team_view_beyond_its_limits_is_refused() {
    use agency_proto::view::limits;
    let sample: agency_proto::TeamView =
        serde_json::from_str(include_str!("../../../testdata/teamview.sample.json")).unwrap();
    assert_eq!(sample.check_limits(), Ok(()), "the shared sample fits");

    let mut long_title = sample.clone();
    long_title.requests[0].title = "x".repeat(limits::TITLE + 1);
    assert!(long_title.check_limits().unwrap_err().contains("title"));
    let mut at_limit = sample.clone();
    at_limit.requests[0].title = "あ".repeat(limits::TITLE);
    assert_eq!(at_limit.check_limits(), Ok(()), "characters, not bytes");

    let mut long_document = sample.clone();
    long_document.requests[0].document = Some("x".repeat(limits::DOCUMENT + 1));
    assert!(long_document.check_limits().is_err());

    let mut many = sample.clone();
    let request = many.requests[0].clone();
    many.requests = vec![request; limits::REQUESTS + 1];
    assert!(many.check_limits().unwrap_err().contains("requests"));

    let mut long_status = sample;
    long_status.people[0].status = "x".repeat(limits::LINE + 1);
    assert!(long_status.check_limits().is_err());
}

#[test]
fn a_home_node_origin_is_https_or_loopback_http_and_nothing_more() {
    use agency_proto::client::is_home_node_origin;
    for accepted in [
        "https://work.example.com",
        "https://work.example.com:8443",
        "http://127.0.0.1:4400",
        "http://localhost:4400",
        "http://[::1]:4400",
        "https://work.example.com/",
        "https://Work.Example.COM:443",
    ] {
        assert!(is_home_node_origin(accepted), "{accepted}");
    }
    for refused in [
        "http://work.example.com",
        "http://192.168.1.2:4400",
        "https://work.example.com/desk",
        "https://alice@work.example.com",
        "https://work.example.com?x=1",
        "https://work.example.com#top",
        "https://",
        "https://work.example.com:",
        "https://work.example.com:84a3",
        "https://work.example.com:123456",
        "https://.example.com",
        "ftp://work.example.com",
        "work.example.com",
        "https://[2001:db8::1]",
        "https://[::1]",
        "https://work.example.com:0",
        "https://work.example.com:65536",
        "https://-work.example.com",
        "https://work-.example.com",
        "https://work..example.com",
        "https://work_1.example.com",
        "https://work.example.com.",
        "HTTPS://work.example.com",
    ] {
        assert!(!is_home_node_origin(refused), "{refused}");
    }
}

#[test]
fn a_home_node_origin_is_written_one_way() {
    use agency_proto::client::home_node_origin;
    for (given, written) in [
        ("https://Work.Example.com", "https://work.example.com"),
        ("https://work.example.com:443/", "https://work.example.com"),
        (
            "https://work.example.com:8443",
            "https://work.example.com:8443",
        ),
        ("http://LOCALHOST:80", "http://localhost"),
        ("http://127.0.0.1:4400/", "http://127.0.0.1:4400"),
        ("http://[::1]:4400", "http://[::1]:4400"),
    ] {
        assert_eq!(home_node_origin(given).as_deref(), Some(written), "{given}");
    }
}
