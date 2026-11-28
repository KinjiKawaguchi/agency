//! A node that reads the kind `prioritize` and does not apply it yet refuses it, and never hands it to the
//! team's runtime as a decision (`decide` reads only answers and judgments).
mod node_fixture;

use agency_core::decisions::{accept_envelope, DecisionNote};
use agency_core::kept::{keep_signed, Kept};
use agency_core::ErrorKind;
use agency_proto::{Envelope, EnvelopeId, EnvelopeKind, Kind, Payload, Signed};
use node_fixture::*;

/// alice's decision to set the priority of `app#12`, signed by `signer` with the fixture's device key.
fn prioritize(byte: u8, signer: &str) -> Signed<Envelope> {
    let body = Envelope {
        id: EnvelopeId::from_bytes([byte; 16]),
        kind: EnvelopeKind::Prioritize,
        org: id("acme"),
        team: id("app"),
        target: "app#12".into(),
        payload: Payload {
            text: String::new(),
            choice: Some("high".into()),
        },
        account: id("alice"),
        at: T0.into(),
    };
    let signer = if signer.is_empty() {
        laptop().public().device_id()
    } else {
        id(signer)
    };
    Signed::sign(Kind::Envelope, body, signer, &laptop()).unwrap()
}

#[tokio::test]
async fn the_teams_node_refuses_a_priority_and_the_runtime_is_not_asked() {
    let alice = alice();
    let signed = prioritize(1, "");
    let error = accept_envelope(&alice.node, &id("acme"), &id("app"), signed.clone())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::BadRequest);
    assert!(error.message.contains("does not take priorities"));
    assert!(
        alice.runtime.0.lock().unwrap().applied.is_empty(),
        "a priority never goes through `decide`"
    );
    assert!(
        !alice.node.store().is_applied(&signed.body.id),
        "a refused envelope is not remembered as applied"
    );
}

#[tokio::test]
async fn a_caller_that_is_not_a_member_is_told_nothing_of_what_the_node_takes() {
    let alice = alice();
    let error = accept_envelope(&alice.node, &id("acme"), &id("app"), prioritize(2, "bob"))
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "the refusal of the kind comes after the signature and the membership are checked"
    );
}

#[test]
fn the_home_node_refuses_a_priority_before_it_keeps_it_and_keeps_an_answer() {
    let alice = alice();
    let account = id("alice");
    let (org, team) = (id("acme"), id("app"));
    let error = keep_signed(
        &alice.node,
        &account,
        &org,
        &team,
        prioritize(3, ""),
        DecisionNote::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::BadRequest);
    assert!(
        alice.node.store().recent_decisions(10).is_empty(),
        "nothing is logged or queued for a refused priority"
    );
    let kept = keep_signed(
        &alice.node,
        &account,
        &org,
        &team,
        envelope(T0, 4),
        DecisionNote::default(),
    )
    .unwrap();
    assert!(matches!(kept, Kept::Queued(_)), "an answer is kept as ever");
}
