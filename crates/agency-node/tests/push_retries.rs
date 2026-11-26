//! A destination that failed to take a notice is tried again alone: a browser whose push service had a bad
//! minute is not forgotten because the phone took the notice, and one that took it is never sent it twice.
//! The retry keeps the notice's key and the destination, never its words, ends when the thing is no longer true,
//! and is dropped after a day.
mod common;

use agency_core::push::notify_waiting;
use agency_proto::client::VapidKey;
use common::{alice, body, id, post, Running, TOKEN};
use serde_json::{json, Value};

const TOKEN_HEX: &str = "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12";
const P256DH: &str =
    "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
const AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
type Memory = std::sync::Arc<std::sync::Mutex<agency_adapters::memory::MemoryRuntimeState>>;

async fn write(running: &Running, path: &str, input: &Value) -> Value {
    let request = post(running, path, input).header("x-agency-token", TOKEN);
    body(request.send().await.unwrap()).await.1
}

/// A phone (an APNs token) and a browser, both wanting requests; the browser's id.
async fn phone_and_browser(alice: &Running) -> String {
    alice.notifier.0.lock().unwrap().web_key = Some(VapidKey {
        public_key: "BPublic".into(),
        key_id: "key-one".into(),
    });
    let token = json!({ "token": TOKEN_HEX, "environment": "sandbox", "label": "iPhone" });
    write(alice, "/api/push/tokens", &token).await;
    browser(alice, "https://fcm.googleapis.com/fcm/send/one").await
}

async fn browser(alice: &Running, endpoint: &str) -> String {
    let input = json!({ "endpoint": endpoint, "p256dh": P256DH, "auth": AUTH, "label": "Firefox" });
    let answer = write(alice, "/api/push/subscriptions", &input).await;
    answer["id"].as_str().unwrap().to_owned()
}

fn add_request(memory: &Memory, request: &str, title: &str) {
    memory
        .lock()
        .unwrap()
        .requests
        .push((request.into(), title.into()));
}

fn refuse(alice: &Running, destinations: &[&str]) {
    alice.notifier.0.lock().unwrap().refusing =
        destinations.iter().map(|one| (*one).to_owned()).collect();
}

fn counts(alice: &Running) -> (usize, usize) {
    let state = alice.notifier.0.lock().unwrap();
    (state.sent.len(), state.sent_web.len())
}

fn retries(alice: &Running) -> Vec<agency_core::push_retries::Retry> {
    alice.node.store().push_retries()
}

#[tokio::test]
async fn only_the_destination_that_failed_is_sent_the_notice_again_and_only_until_it_takes_it() {
    let (alice, memory) = alice().await;
    let browser = phone_and_browser(&alice).await;
    notify_waiting(&alice.node).await.unwrap();

    add_request(&memory, "r-2", "Ship it?");
    refuse(&alice, &[&browser]);
    let first = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        (first.sent, first.failures.len()),
        (1, 1),
        "the phone took it, the browser failed"
    );
    assert_eq!(counts(&alice), (1, 0));
    assert_eq!(retries(&alice).len(), 1);

    let second = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        (second.sent, second.failures.len()),
        (0, 1),
        "the browser failed again, alone"
    );
    assert_eq!(counts(&alice), (1, 0), "the phone is not sent it again");

    refuse(&alice, &[]);
    let third = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(third.sent, 1);
    assert_eq!(
        counts(&alice),
        (1, 1),
        "the browser has it now, and the phone still has it once"
    );
    assert!(retries(&alice).is_empty());
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        0,
        "nothing is told twice"
    );
    assert_eq!(counts(&alice), (1, 1));
}

#[tokio::test]
async fn a_phone_that_fails_is_sent_the_notice_again_while_the_browser_that_took_it_is_not() {
    let (alice, memory) = alice().await;
    phone_and_browser(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "Ship it?");
    refuse(&alice, &[TOKEN_HEX]);
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(counts(&alice), (0, 1));
    refuse(&alice, &[]);
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        counts(&alice),
        (1, 1),
        "the phone had its retry, the browser was not told again"
    );
}

#[tokio::test]
async fn a_destination_that_is_gone_when_it_is_tried_again_is_removed_and_its_retry_ends() {
    let (alice, memory) = alice().await;
    let browser = phone_and_browser(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "Ship it?");
    refuse(&alice, &[&browser]);
    notify_waiting(&alice.node).await.unwrap();
    refuse(&alice, &[]);
    alice.notifier.0.lock().unwrap().gone.push(browser.clone());
    notify_waiting(&alice.node).await.unwrap();
    assert!(retries(&alice).is_empty());
    assert!(
        alice.node.store().web_subscriptions().is_empty(),
        "the push service said it is gone"
    );
    assert_eq!(counts(&alice), (1, 0));
}

#[tokio::test]
async fn a_destination_registered_after_a_notice_is_never_told_it() {
    let (alice, memory) = alice().await;
    let browser = phone_and_browser(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "Ship it?");
    refuse(&alice, &[&browser]);
    notify_waiting(&alice.node).await.unwrap();
    let later = self::browser(&alice, "https://fcm.googleapis.com/fcm/send/two").await;
    refuse(&alice, &[]);
    notify_waiting(&alice.node).await.unwrap();
    let told: Vec<String> = alice
        .notifier
        .0
        .lock()
        .unwrap()
        .sent_web
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(
        told,
        vec![browser],
        "the retry went to the one that failed, not to the one that came later"
    );
    assert!(!told.contains(&later));
}

#[tokio::test]
async fn a_retry_ends_when_the_thing_is_no_longer_true_and_is_never_sent_late() {
    let (alice, memory) = alice().await;
    let browser = phone_and_browser(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "Ship it?");
    refuse(&alice, &[&browser]);
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(retries(&alice).len(), 1);
    // The request is decided: it no longer waits for the owner.
    memory
        .lock()
        .unwrap()
        .requests
        .retain(|(request, _)| request != "r-2");
    refuse(&alice, &[]);
    let pass = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(pass.sent, 0);
    assert!(retries(&alice).is_empty());
    assert_eq!(
        counts(&alice),
        (1, 0),
        "the browser was never sent a notice about what has passed"
    );
}

#[tokio::test]
async fn a_retry_is_dropped_after_a_day() {
    let (alice, memory) = alice().await;
    let browser = phone_and_browser(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "Ship it?");
    refuse(&alice, &[&browser]);
    notify_waiting(&alice.node).await.unwrap();
    alice
        .clock
        .advance(agency_proto::clock::Duration::hours(23));
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(retries(&alice).len(), 1, "still kept at 23 hours");
    alice.clock.advance(agency_proto::clock::Duration::hours(2));
    refuse(&alice, &[]);
    let pass = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(pass.sent, 0, "a notice a day late is not sent");
    assert!(retries(&alice).is_empty());
    assert_eq!(counts(&alice), (1, 0));
}

#[tokio::test]
async fn a_retry_carries_no_words_and_the_notice_is_worded_again_with_the_settings_of_the_moment() {
    let (alice, memory) = alice().await;
    let browser = phone_and_browser(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "Ship it? (secret title)");
    refuse(&alice, &[&browser]);
    notify_waiting(&alice.node).await.unwrap();
    let file = std::fs::read_to_string(alice.data_dir().join("push-retries.json")).unwrap();
    assert!(
        !file.contains("secret title"),
        "no title is written: {file}"
    );
    let written: Vec<Value> = serde_json::from_str(&file).unwrap();
    let fields: Vec<Vec<&str>> = written
        .iter()
        .map(|one| {
            one.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect()
        })
        .collect();
    assert_eq!(
        fields,
        vec![vec!["destination", "key", "since"]],
        "a key, a destination, and a time: no label, no words"
    );
    assert!(file.contains("acme/app/r-2") && file.contains(&browser));

    // The node now hides titles: the retry is worded with that, not as it was when it first failed.
    alice
        .node
        .update_config(|config| config.hide_titles = true)
        .unwrap();
    refuse(&alice, &[]);
    notify_waiting(&alice.node).await.unwrap();
    let state = alice.notifier.0.lock().unwrap();
    let agency_core::push::NoticeKind::Request {
        team_label, title, ..
    } = &state.sent_web[0].1.kind
    else {
        panic!("a request");
    };
    assert_eq!((team_label, title), (&None, &None));
    let _ = id("acme");
}

#[tokio::test]
async fn a_node_with_no_retries_file_or_an_old_notified_list_reads_as_before() {
    let (alice, memory) = alice().await;
    phone_and_browser(&alice).await;
    assert!(retries(&alice).is_empty());
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "Ship it?");
    assert_eq!(notify_waiting(&alice.node).await.unwrap().sent, 1);
    assert_eq!(notify_waiting(&alice.node).await.unwrap().sent, 0);
}

#[tokio::test]
async fn of_two_browsers_the_one_that_failed_is_the_one_sent_again() {
    let (alice, memory) = alice().await;
    let first = phone_and_browser(&alice).await;
    let second = browser(&alice, "https://fcm.googleapis.com/fcm/send/two").await;
    notify_waiting(&alice.node).await.unwrap();
    add_request(&memory, "r-2", "Ship it?");
    refuse(&alice, &[&second]);
    notify_waiting(&alice.node).await.unwrap();
    refuse(&alice, &[]);
    notify_waiting(&alice.node).await.unwrap();
    let told: Vec<String> = alice
        .notifier
        .0
        .lock()
        .unwrap()
        .sent_web
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(
        told,
        vec![first, second],
        "the first browser took it once; the second took its retry, and no browser twice"
    );
}
