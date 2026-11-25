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
    assert_eq!(sent.status(), 200);
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
    assert_eq!(refused.status(), 404);
    assert_eq!(
        decisions(&alice, "").await["entries"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "a decision the runtime refused is not logged"
    );

    let elsewhere = agency_core::decisions::DecisionRecord {
        id: agency_proto::EnvelopeId::from_bytes([7; 16]),
        org: id("other"),
        team: id("app"),
        kind: agency_proto::EnvelopeKind::Answer,
        target: "r-9".into(),
        title: None,
        choice_label: None,
        payload: agency_proto::Payload {
            text: "x".into(),
            choice: None,
        },
        at: "2026-11-24T12:00:00Z".into(),
        is_local: false,
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
        vec![json!("r-1")],
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
        (StatusCode::OK, Some("queued"))
    );
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
    assert_eq!(status, 409, "the team's node refused");
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
