//! The `prioritize` envelope: what makes its payload well formed, that a view never offers it as the answer
//! to a request, and the runtime call that carries it.
use agency_proto::envelope::EnvelopeKind;
use agency_proto::prioritize::{PriorityChoice, PRIORITIZE_OP};
use agency_proto::runtime::Describe;
use agency_proto::{Envelope, Id, RuntimeCall, RuntimeRequest};
use serde_json::json;
mod common;
use common::*;

fn with_payload(text: &str, choice: Option<&str>) -> Result<(), String> {
    let mut body = prioritize_envelope();
    body.payload.text = text.into();
    body.payload.choice = choice.map(str::to_owned);
    body.validate()
}

#[test]
fn a_priority_decision_has_one_of_the_six_words_and_no_text() {
    assert_eq!(prioritize_envelope().validate(), Ok(()));
    for word in ["highest", "high", "medium", "low", "deferred", "none"] {
        assert_eq!(with_payload("", Some(word)), Ok(()), "{word}");
    }
    for (text, choice) in [
        ("", None),
        ("", Some("")),
        ("", Some("urgent")),
        ("", Some("High")),
        ("high", None),
        ("a comment", Some("high")),
        (" ", Some("high")),
    ] {
        assert_eq!(
            with_payload(text, choice),
            Err("malformed priority".to_owned()),
            "{text:?} {choice:?}"
        );
    }
}

#[test]
fn a_priority_decision_names_its_ticket_by_the_target_rule() {
    for target in ["app#12", "ENG-123", &"a".repeat(120)] {
        let mut body = prioritize_envelope();
        body.target = target.into();
        assert_eq!(body.validate(), Ok(()), "{target}");
    }
    for target in ["", "owner/repo#12", &"a".repeat(121)] {
        let mut body = prioritize_envelope();
        body.target = target.into();
        assert_eq!(
            body.validate(),
            Err("malformed target".to_owned()),
            "{target}"
        );
    }
}

#[test]
fn an_answer_keeps_its_own_rules_and_a_priority_word_is_no_answer_to_a_priority() {
    // `none` and `high` are ordinary choices of an answer; only `prioritize` reads them as priorities.
    let mut answer = envelope(AT);
    answer.payload.choice = Some("none".into());
    assert_eq!(answer.validate(), Ok(()));
    answer.payload.choice = None;
    assert!(
        answer.validate().is_err(),
        "an answer still needs a choice or a text"
    );
}

#[test]
fn the_kind_is_written_prioritize_and_a_signature_for_it_does_not_pass_for_an_answer() {
    let body = prioritize_envelope();
    assert!(agency_proto::canonical::canonical_of(&body)
        .unwrap()
        .contains(r#""kind":"prioritize""#));
    let mut as_answer: Envelope = body.clone();
    as_answer.kind = EnvelopeKind::Answer;
    assert_ne!(
        agency_proto::canonical::canonical_of(&body).unwrap(),
        agency_proto::canonical::canonical_of(&as_answer).unwrap(),
        "the kind is part of the signed body"
    );
}

#[test]
fn a_view_never_offers_a_request_that_is_answered_with_a_priority() {
    let mut view: agency_proto::TeamView =
        serde_json::from_str(include_str!("../../../testdata/teamview.sample.json")).unwrap();
    assert_eq!(view.check_limits(), Ok(()));
    view.requests[0].answer_with.kind = EnvelopeKind::Prioritize;
    assert!(view.check_limits().unwrap_err().contains("answered with"));
}

#[test]
fn the_runtime_op_carries_the_target_the_choice_and_who_decided() {
    let call = RuntimeCall::Prioritize {
        target: "app#12".into(),
        priority: PriorityChoice::Deferred,
        by: Id::parse("alice").unwrap(),
        by_participant: Some("owner".into()),
    };
    let request = RuntimeRequest::new(Id::parse("acme").unwrap(), Id::parse("app").unwrap(), call);
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({ "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "prioritize",
                "target": "app#12", "priority": "deferred", "by": "alice", "byParticipant": "owner" })
    );
    let read: RuntimeRequest = serde_json::from_value(json!({
        "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "prioritize",
        "target": "app#12", "priority": "none", "by": "alice" }))
    .unwrap();
    assert!(matches!(
        read.call,
        RuntimeCall::Prioritize {
            priority: PriorityChoice::Clear,
            by_participant: None,
            ..
        }
    ));
}

#[test]
fn the_op_is_optional_and_found_by_its_name() {
    assert_eq!(PRIORITIZE_OP, "prioritize");
    let described: Describe = serde_json::from_value(json!({
        "runtimes": ["agency-runtime/1"], "ops": ["describe", "view", "decide", "prioritize"], "label": "App" }))
    .unwrap();
    assert!(described.supports(PRIORITIZE_OP));
}
