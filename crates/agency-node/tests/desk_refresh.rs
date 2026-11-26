//! The push that wakes the iPhones to read the desk, so that the widgets show what waits for the owner while
//! the app is closed: when it is sent, how often, to whom, and that a request's alert which already woke the
//! app makes it unnecessary.
mod common;

use agency_core::push::{notify_waiting, DeskRefresh, REFRESH_FLOOR_MINUTES};
use agency_proto::clock::Duration;
use common::{alice, body, Running, TOKEN};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const TOKEN_HEX: &str = "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12";

async fn register(running: &Running, extra: Value) {
    let mut input = json!({ "token": TOKEN_HEX, "environment": "sandbox", "label": "iPhone" });
    input
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let (status, answer) = body(
        Client::new()
            .post(format!("{}/api/push/tokens", running.url()))
            .header("x-agency-token", TOKEN)
            .json(&input)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
}

type Memory = Arc<Mutex<agency_adapters::memory::MemoryRuntimeState>>;

fn add_request(memory: &Memory, id: &str) {
    memory
        .lock()
        .unwrap()
        .requests
        .push((id.into(), format!("Title of {id}")));
}

fn decide(memory: &Memory, id: &str) {
    memory.lock().unwrap().requests.retain(|(one, _)| one != id);
}

fn woken(running: &Running) -> usize {
    running.notifier.0.lock().unwrap().desk_changed.len()
}

fn alerts(running: &Running) -> usize {
    running.notifier.0.lock().unwrap().sent.len()
}

fn attempts(running: &Running) -> usize {
    running.notifier.0.lock().unwrap().desk_attempts
}

fn pass_after_seconds(running: &Running, seconds: i64) {
    running.clock.advance(Duration::seconds(seconds));
}

fn pass_after(running: &Running, minutes: i64) {
    running.clock.advance(Duration::minutes(minutes));
}

#[tokio::test]
async fn the_first_pass_only_records_what_waits() {
    let (alice, _) = alice().await;
    register(&alice, json!({})).await;
    let pass = notify_waiting(&alice.node).await.unwrap();
    assert_eq!((pass.sent, woken(&alice)), (0, 0));
    let state = alice.node.store().desk_refresh().expect("it is recorded");
    assert_eq!(state.waiting, vec!["acme/app/r-1".to_owned()]);
}

#[tokio::test]
async fn a_request_that_is_decided_wakes_the_iphones_once_the_floor_has_passed() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    decide(&memory, "r-1");
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(woken(&alice), 1, "one iPhone, woken with no alert");
    assert_eq!(
        alerts(&alice),
        0,
        "a request leaving the desk is not an alert"
    );
    assert_eq!(
        alice.node.store().desk_refresh().unwrap().waiting,
        Vec::<String>::new()
    );
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(woken(&alice), 1, "the same desk is not told twice");
}

#[tokio::test]
async fn nothing_is_sent_while_the_desk_does_not_change() {
    let (alice, _) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    for _ in 0..3 {
        pass_after(&alice, 60);
        notify_waiting(&alice.node).await.unwrap();
    }
    assert_eq!(woken(&alice), 0);
}

#[tokio::test]
async fn a_change_inside_the_window_waits_for_its_end() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        REFRESH_FLOOR_MINUTES, 15,
        "the protocol says fifteen minutes"
    );
    pass_after(&alice, 14);
    decide(&memory, "r-1");
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(woken(&alice), 0, "one minute short of the floor");
    pass_after(&alice, 1);
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(woken(&alice), 1, "it goes when the window ends");
    pass_after(&alice, 1);
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(woken(&alice), 1);
}

#[tokio::test]
async fn changes_inside_one_window_go_together_and_a_return_to_the_same_desk_sends_nothing() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, 1);
    decide(&memory, "r-1");
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, 1);
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-1".into(), "Make a key?".into()));
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        woken(&alice),
        0,
        "r-1 left and came back: the desk is as it was"
    );
}

#[tokio::test]
async fn a_request_alert_that_reached_an_iphone_is_the_wake_and_no_push_follows() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    add_request(&memory, "r-2");
    let pass = notify_waiting(&alice.node).await.unwrap();
    assert_eq!((pass.sent, alerts(&alice), woken(&alice)), (1, 1, 0));
    assert_eq!(
        alice.node.store().desk_refresh().unwrap().waiting,
        vec!["acme/app/r-1".to_owned(), "acme/app/r-2".to_owned()],
        "the alert counts as the wake: the desk it asked about is recorded"
    );
    pass_after(&alice, 1);
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(woken(&alice), 0);
}

#[tokio::test]
async fn an_iphone_that_wants_no_request_alerts_is_still_woken_for_a_new_request() {
    let (alice, memory) = alice().await;
    register(&alice, json!({ "events": [] })).await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    add_request(&memory, "r-2");
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(alerts(&alice), 0, "it chose no alerts");
    assert_eq!(
        woken(&alice),
        1,
        "but the widgets are not its alerts: it is woken to read the desk"
    );
}

#[tokio::test]
async fn with_no_iphone_registered_nothing_is_sent_and_the_desk_is_recorded_all_the_same() {
    let (alice, memory) = alice().await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    decide(&memory, "r-1");
    let pass = notify_waiting(&alice.node).await.unwrap();
    assert_eq!((woken(&alice), pass.failures.len()), (0, 0));
    assert_eq!(
        alice.node.store().desk_refresh().unwrap().waiting,
        Vec::<String>::new(),
        "an iPhone registered later is not woken for what happened before it"
    );
}

#[tokio::test]
async fn a_push_that_fails_is_tried_again_after_the_floor_not_at_every_pass() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    decide(&memory, "r-1");
    alice.notifier.0.lock().unwrap().failing = true;
    let failed = notify_waiting(&alice.node).await.unwrap();
    assert_eq!((woken(&alice), failed.failures.len()), (0, 1));
    assert_eq!(
        alice.node.store().desk_refresh().unwrap().waiting,
        vec!["acme/app/r-1".to_owned()],
        "the failed change is still pending"
    );
    pass_after(&alice, 1);
    let again = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        again.failures.len(),
        0,
        "no second attempt inside the window"
    );
    assert_eq!(attempts(&alice), 1);
    alice.notifier.0.lock().unwrap().failing = false;
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(woken(&alice), 1, "after the floor the pending change goes");
}

#[tokio::test]
async fn with_apns_down_for_an_hour_a_node_tries_four_times_and_not_every_pass() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    decide(&memory, "r-1");
    alice.notifier.0.lock().unwrap().failing = true;
    for _ in 0..120 {
        pass_after_seconds(&alice, 30);
        notify_waiting(&alice.node).await.unwrap();
    }
    assert_eq!(
        attempts(&alice),
        4,
        "one pass every 30 seconds for an hour: 120 passes"
    );
    alice.notifier.0.lock().unwrap().failing = false;
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(woken(&alice), 1, "and the change was kept the whole time");
}

#[tokio::test]
async fn an_iphone_apns_no_longer_accepts_is_removed_by_a_wake_too() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    decide(&memory, "r-1");
    alice
        .notifier
        .0
        .lock()
        .unwrap()
        .gone
        .push(TOKEN_HEX.to_owned());
    notify_waiting(&alice.node).await.unwrap();
    assert!(alice.node.store().push_tokens().is_empty());
    assert_eq!(
        alice.node.store().desk_refresh().unwrap().waiting,
        Vec::<String>::new(),
        "the token was the only one and is gone: nothing to wake, so it is recorded"
    );
}

#[tokio::test]
async fn a_request_for_someone_else_is_not_the_owners_desk() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    alice
        .node
        .update_config(|config| config.teams[0].owner = common::id("bob"))
        .unwrap();
    add_request(&memory, "r-9");
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(woken(&alice), 1, "r-1 is bob's now: it left alice's desk");
    assert_eq!(alerts(&alice), 0, "and r-9 is bob's, so no alert");
    assert_eq!(
        alice.node.store().desk_refresh().unwrap().waiting,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn a_device_alert_is_not_a_wake_for_the_desk() {
    let (alice, memory) = alice().await;
    register(&alice, json!({ "events": ["request", "device"] })).await;
    notify_waiting(&alice.node).await.unwrap();
    pass_after(&alice, REFRESH_FLOOR_MINUTES);
    let key = agency_proto::AccountKey::from_seed([21; 32]).public();
    let input = json!({ "label": "Tablet", "publicKey": key.encoded() });
    let (status, answer) = body(
        Client::new()
            .post(format!("{}/api/devices/requests", alice.url()))
            .header("x-agency-token", TOKEN)
            .json(&input)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    decide(&memory, "r-1");
    let pass = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(pass.sent, 1, "the device alert");
    assert_eq!(alerts(&alice), 1);
    assert_eq!(
        woken(&alice),
        1,
        "that alert did not carry the desk's change: the push does"
    );
}

#[tokio::test]
async fn a_record_whose_time_cannot_be_read_is_due() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    notify_waiting(&alice.node).await.unwrap();
    alice
        .node
        .store()
        .save_desk_refresh(&DeskRefresh {
            waiting: vec!["acme/app/r-1".to_owned()],
            at: "yesterday".to_owned(),
        })
        .unwrap();
    decide(&memory, "r-1");
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        woken(&alice),
        1,
        "a damaged record does not hold the desk back"
    );
}

#[tokio::test]
async fn a_node_upgraded_with_no_record_of_the_desk_records_it_and_wakes_nothing() {
    let (alice, memory) = alice().await;
    register(&alice, json!({})).await;
    // Alerts were already being sent (the notified list exists) before the node knew how to wake the iPhones.
    alice
        .node
        .store()
        .save_notified(&["acme/app/r-1".to_owned()])
        .unwrap();
    assert!(alice.node.store().desk_refresh().is_none());
    pass_after(&alice, 60);
    decide(&memory, "r-1");
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        woken(&alice),
        0,
        "what happened before there was a record is not announced"
    );
    assert_eq!(
        alice.node.store().desk_refresh().unwrap().waiting,
        Vec::<String>::new(),
        "the desk is recorded as it is now"
    );
}
