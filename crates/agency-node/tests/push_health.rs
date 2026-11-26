//! What the node keeps of how APNs took the notices sent to each phone: when it last took one, and the last time it
//! did not and why (never more than the status and the reason code). Returned in the token list, kept when the app
//! registers the same token again, and recorded on the first attempt, on a retry, and on the wake of the desk alike.
mod common;

use agency_core::push::notify_waiting;
use common::{alice, body, get, post, Running, TOKEN};
use serde_json::{json, Value};

const TOKEN_HEX: &str = "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12";
type Memory = std::sync::Arc<std::sync::Mutex<agency_adapters::memory::MemoryRuntimeState>>;

async fn register(running: &Running) -> Value {
    let input = json!({ "token": TOKEN_HEX, "environment": "sandbox", "label": "iPhone" });
    let request = post(running, "/api/push/tokens", &input).header("x-agency-token", TOKEN);
    body(request.send().await.unwrap()).await.1
}

async fn listed(running: &Running) -> Vec<Value> {
    let answer = body(get(running, "/api/push/tokens").send().await.unwrap())
        .await
        .1;
    answer["tokens"].as_array().unwrap().clone()
}

fn add_request(memory: &Memory, request: &str, title: &str) {
    memory
        .lock()
        .unwrap()
        .requests
        .push((request.into(), title.into()));
}

fn refuse(running: &Running, refusing: bool) {
    running.notifier.0.lock().unwrap().refusing = if refusing {
        vec![TOKEN_HEX.to_owned()]
    } else {
        vec![]
    };
}

#[tokio::test]
async fn nothing_is_known_of_a_phone_before_a_notice_is_sent_to_it() {
    let (alice, _) = alice().await;
    let registered = register(&alice).await;
    assert!(
        registered.get("lastSucceededAt").is_none() && registered.get("lastFailure").is_none(),
        "{registered}"
    );
    let token = &listed(&alice).await[0];
    assert!(
        token.get("lastSucceededAt").is_none() && token.get("lastFailure").is_none(),
        "{token}"
    );
}

#[tokio::test]
async fn the_node_keeps_when_apns_last_took_a_notice_and_when_it_last_did_not() {
    let (alice, memory) = alice().await;
    register(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "First?");
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        listed(&alice).await[0]["lastSucceededAt"],
        "2026-11-24T12:00:00Z",
        "the first notice was taken"
    );
    assert!(listed(&alice).await[0].get("lastFailure").is_none());

    alice
        .clock
        .advance(agency_proto::clock::Duration::minutes(5));
    add_request(&memory, "r-3", "Ship it?");
    refuse(&alice, true);
    notify_waiting(&alice.node).await.unwrap();
    let after_failure = listed(&alice).await[0].clone();
    assert_eq!(
        after_failure["lastFailure"]["reason"],
        "APNs did not answer"
    );
    assert_eq!(after_failure["lastFailure"]["at"], "2026-11-24T12:05:00Z");
    assert_eq!(
        after_failure["lastSucceededAt"], "2026-11-24T12:00:00Z",
        "the success before it stays"
    );

    alice
        .clock
        .advance(agency_proto::clock::Duration::minutes(5));
    refuse(&alice, false);
    notify_waiting(&alice.node).await.unwrap();
    let after_retry = listed(&alice).await[0].clone();
    assert_eq!(
        after_retry["lastSucceededAt"], "2026-11-24T12:10:00Z",
        "the retry took it"
    );
    assert_eq!(
        after_retry["lastFailure"]["at"], "2026-11-24T12:05:00Z",
        "the last failure stays, older than the success"
    );
}

#[tokio::test]
async fn registering_the_same_token_again_keeps_what_is_known_of_it() {
    let (alice, memory) = alice().await;
    register(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "First?");
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-3", "Again?");
    refuse(&alice, true);
    notify_waiting(&alice.node).await.unwrap();
    let again = register(&alice).await;
    assert!(
        again["lastSucceededAt"].is_string(),
        "the success is kept: {again}"
    );
    assert_eq!(
        again["lastFailure"]["reason"], "APNs did not answer",
        "and the failure: {again}"
    );
}

#[tokio::test]
async fn a_token_apns_calls_gone_is_removed_and_not_kept_as_a_failure() {
    let (alice, memory) = alice().await;
    register(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    alice
        .notifier
        .0
        .lock()
        .unwrap()
        .gone
        .push(TOKEN_HEX.to_owned());
    add_request(&memory, "r-3", "Ship it?");
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(listed(&alice).await, Vec::<Value>::new());
}

#[tokio::test]
async fn what_is_stored_for_a_token_has_the_facts_and_a_token_stored_before_them_reads_without() {
    let (alice, memory) = alice().await;
    register(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "First?");
    notify_waiting(&alice.node).await.unwrap();
    let stored = serde_json::to_value(alice.node.store().push_tokens()).unwrap();
    assert!(stored[0]["lastSucceededAt"].is_string(), "{stored}");
    let before = json!({ "token": TOKEN_HEX, "environment": "sandbox", "label": "iPhone", "registeredAt": "2026-11-24T12:00:00Z", "events": ["request"] });
    let read: agency_proto::client::PushToken = serde_json::from_value(before).unwrap();
    assert!(read.last_succeeded_at.is_none() && read.last_failure.is_none());
}
