//! The decision log in the Client API: what the account decided, with the notes people read and where
//! each decision went.
mod common;

use agency_proto::{AccountKey, Payload};
use common::{
    alice, body, device, get, id, joined_bob, post, sign_answer, Running, ENVELOPES, TOKEN,
};
use reqwest::StatusCode;
use serde_json::{json, Value};

/// A device-signed answer "Go" with choice "a", with notes for the log.
fn titled(running: &Running, key: &AccountKey, target: &str, title: &str) -> Value {
    let payload = Payload {
        text: "Go".into(),
        choice: Some("a".into()),
    };
    let envelope = sign_answer(running, key, target, payload);
    json!({ "envelope": envelope, "title": title, "choiceLabel": " Option A " })
}

/// A second device of the account, added by the first: only a first device is added with the account key.
async fn second_device(running: &Running, first: &AccountKey) -> AccountKey {
    let key = AccountKey::generate();
    let id = first.public().device_id();
    let signer = agency_core::devices::DeviceSigner {
        id: &id,
        key: first,
    };
    let added =
        agency_core::devices::add_device(&running.node, "Second", &key.public(), Some(&signer))
            .await;
    assert!(
        added
            .iter()
            .all(|(_, outcome)| matches!(outcome, agency_core::devices::Added::Done(_))),
        "{added:?}"
    );
    key
}

async fn decisions(running: &Running, query: &str) -> Value {
    body(
        get(running, &format!("/api/decisions{query}"))
            .send()
            .await
            .unwrap(),
    )
    .await
    .1
}

#[tokio::test]
async fn logs_local_decisions_as_applied() {
    let (alice, _) = alice().await;
    let key = device(&alice).await;
    let path = ENVELOPES;
    let sent = post(&alice, path, &titled(&alice, &key, "r-1", "Make a key?"))
        .header("x-agency-token", TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(sent.status(), 202);
    assert_eq!(
        decisions(&alice, "").await["entries"][0]["state"]["kind"],
        "queued",
        "answered before the runtime was asked"
    );
    agency_core::outbox::deliver_due(&alice.node).await.unwrap();
    let log = decisions(&alice, "").await;
    let entry = &log["entries"][0];
    assert_eq!(entry["state"], json!({ "kind": "applied" }));
    assert_eq!(
        (
            entry["org"].as_str(),
            entry["team"].as_str(),
            entry["target"].as_str()
        ),
        (Some("acme"), Some("app"), Some("r-1"))
    );
    assert_eq!(
        (
            entry["title"].as_str(),
            entry["text"].as_str(),
            entry["choice"].as_str()
        ),
        (Some("Make a key?"), Some("Go"), Some("a"))
    );
    assert_eq!(entry["choiceLabel"], "Option A", "kept trimmed");
    assert_eq!(entry["decidedAt"], "2026-11-24T12:00:00Z");

    let refused = post(&alice, path, &titled(&alice, &key, "r-404", "Gone"))
        .header("x-agency-token", TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 202, "kept; checked when it is applied");
    agency_core::outbox::deliver_due(&alice.node).await.unwrap();
    let log = decisions(&alice, "").await;
    assert_eq!(log["entries"].as_array().unwrap().len(), 2);
    assert_eq!(log["entries"][0]["target"], "r-404");
    assert_eq!(log["entries"][0]["state"]["kind"], "refused");
    assert!(
        !log["entries"][0]["state"]["message"]
            .as_str()
            .unwrap()
            .is_empty(),
        "no request r-404 waits for alice, and the log says why"
    );

    let elsewhere = agency_core::decisions::DecisionRecord {
        id: agency_proto::EnvelopeId::from_bytes([7; 16]),
        org: id("other"),
        team: id("app"),
        kind: agency_proto::EnvelopeKind::Answer,
        target: "r-9".into(),
        title: None,
        choice_label: None,
        relayed_for: None,
        payload: agency_proto::Payload {
            text: "x".into(),
            choice: None,
        },
        at: "2026-11-24T12:00:00Z".into(),
        is_local: false,
        by: None,
        signer: None,
    };
    alice.node.store().append_decision(&elsewhere).unwrap();
    let targets: Vec<Value> = decisions(&alice, "").await["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["target"].clone())
        .collect();
    assert_eq!(
        targets,
        vec![json!("r-404"), json!("r-1")],
        "decisions in orgs the account does not belong to are hidden"
    );
}

#[tokio::test]
async fn logs_sent_decisions_with_their_delivery_state() {
    let (mut alice, memory) = alice().await;
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-2".into(), "Second".into()));
    let bob = joined_bob(&alice).await;
    common::for_bob(&alice, &memory, "r-1");
    let key = device(&bob).await;
    let path = ENVELOPES;
    alice.stop().await;
    let (status, result) = body(
        post(&bob, path, &titled(&bob, &key, "r-1", "First"))
            .header("x-agency-token", TOKEN)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        (status, result["outcome"].as_str()),
        (StatusCode::ACCEPTED, Some("queued"))
    );
    assert_eq!(
        decisions(&bob, "").await["entries"][0]["state"]["attempts"],
        0,
        "not tried when it was kept"
    );
    agency_core::outbox::deliver_due(&bob.node).await.unwrap();
    let queued = decisions(&bob, "").await["entries"][0]["state"].clone();
    assert_eq!(
        (
            queued["kind"].as_str(),
            queued["attempts"].as_u64(),
            queued["nextAt"].as_str()
        ),
        (Some("queued"), Some(1), Some("2026-11-24T12:00:15Z"))
    );
    assert!(queued["lastError"].is_string());

    alice.start().await;
    bob.clock
        .advance(agency_proto::clock::Duration::seconds(15));
    agency_core::outbox::deliver_due(&bob.node).await.unwrap();
    assert_eq!(
        decisions(&bob, "").await["entries"][0]["state"],
        json!({ "kind": "delivered" })
    );

    let (status, _) = body(
        post(&bob, path, &titled(&bob, &key, "r-404", "Unknown"))
            .header("x-agency-token", TOKEN)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 202, "kept; the team's node refuses it when sent");
    agency_core::outbox::deliver_due(&bob.node).await.unwrap();
    let log = decisions(&bob, "").await;
    assert_eq!(
        log["entries"][0]["state"]["kind"], "refused",
        "newest first"
    );
    assert!(log["entries"][0]["state"]["message"]
        .as_str()
        .unwrap()
        .contains("r-404"));
    assert_eq!(log["entries"][1]["target"], "r-1");
    assert_eq!(
        decisions(&bob, "?limit=1").await["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn refuses_long_notes_and_malformed_limits() {
    let (alice, memory) = alice().await;
    let key = device(&alice).await;
    let long = "x".repeat(201);
    let mut long_label = titled(&alice, &key, "r-1", "Make a key?");
    long_label["choiceLabel"] = json!(long);
    let mut label_without_choice = titled(&alice, &key, "r-1", "Make a key?");
    label_without_choice["envelope"]["body"]["payload"]
        .as_object_mut()
        .unwrap()
        .remove("choice");
    for (name, input) in [
        ("long title", titled(&alice, &key, "r-1", &long)),
        ("long choice label", long_label),
        ("choice label without a choice", label_without_choice),
    ] {
        let response = post(&alice, ENVELOPES, &input)
            .header("x-agency-token", TOKEN)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400, "{name}");
    }
    assert!(memory.lock().unwrap().applied.is_empty());
    assert_eq!(
        get(&alice, "/api/decisions?limit=many")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        decisions(&alice, "?limit=0").await["entries"],
        json!([]),
        "nothing decided yet"
    );
}

#[tokio::test]
async fn logs_the_device_that_signed_each_decision() {
    let (alice, memory) = alice().await;
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-2".into(), "Second".into()));
    let laptop = device(&alice).await;
    let phone = second_device(&alice, &laptop).await;
    for (target, key) in [("r-1", &laptop), ("r-2", &phone)] {
        let sent = post(&alice, ENVELOPES, &titled(&alice, key, target, "A title"))
            .header("x-agency-token", TOKEN)
            .send()
            .await
            .unwrap();
        assert_eq!(sent.status(), 200, "{target}");
    }
    let log = decisions(&alice, "").await;
    let signer_of = |target: &str| {
        log["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["target"] == target)
            .map(|entry| entry["signer"].clone())
    };
    assert_eq!(
        signer_of("r-1"),
        Some(json!(laptop.public().device_id().as_str()))
    );
    assert_eq!(
        signer_of("r-2"),
        Some(json!(phone.public().device_id().as_str()))
    );
}

#[tokio::test]
async fn logs_the_device_of_a_decision_that_waits_to_be_sent() {
    let (mut alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    common::for_bob(&alice, &memory, "r-1");
    let key = device(&bob).await;
    alice.stop().await;
    let sent = post(&bob, ENVELOPES, &titled(&bob, &key, "r-1", "First"))
        .header("x-agency-token", TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(sent.status(), 200);
    let entry = decisions(&bob, "").await["entries"][0].clone();
    assert_eq!(entry["state"]["kind"], "queued");
    assert_eq!(entry["signer"], json!(key.public().device_id().as_str()));
}

#[tokio::test]
async fn takes_the_device_from_the_verified_envelope_and_from_nothing_a_client_says() {
    let (alice, _) = alice().await;
    let laptop = device(&alice).await;
    let phone = second_device(&alice, &laptop).await;

    // A body that names a signer beside the envelope is refused: it has no such field.
    let mut with_a_signer = titled(&alice, &laptop, "r-1", "A title");
    with_a_signer["signer"] = json!(phone.public().device_id().as_str());
    let refused = post(&alice, ENVELOPES, &with_a_signer)
        .header("x-agency-token", TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);

    // An envelope signed by the laptop that names the phone as its signer does not verify, so it is not logged.
    let mut forged = titled(&alice, &laptop, "r-1", "A title");
    forged["envelope"]["signer"] = json!(phone.public().device_id().as_str());
    let refused = post(&alice, ENVELOPES, &forged)
        .header("x-agency-token", TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
    assert!(decisions(&alice, "").await["entries"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn an_entry_made_before_the_device_was_kept_has_no_signer() {
    let (alice, _) = alice().await;
    let old = agency_core::decisions::DecisionRecord {
        id: agency_proto::EnvelopeId::from_bytes([8; 16]),
        org: id("acme"),
        team: id("app"),
        kind: agency_proto::EnvelopeKind::Answer,
        target: "r-1".into(),
        title: None,
        choice_label: None,
        relayed_for: None,
        payload: Payload {
            text: "x".into(),
            choice: None,
        },
        at: "2026-11-24T12:00:00Z".into(),
        is_local: true,
        by: None,
        signer: None,
    };
    alice.node.store().append_decision(&old).unwrap();
    let entry = decisions(&alice, "").await["entries"][0].clone();
    assert_eq!(entry["target"], "r-1");
    assert!(
        entry.get("signer").is_none(),
        "left out, never null: {entry}"
    );
    let stored = std::fs::read_to_string(alice.data_dir().join("decisions.json")).unwrap();
    assert!(!stored.contains("signer"), "{stored}");
}
