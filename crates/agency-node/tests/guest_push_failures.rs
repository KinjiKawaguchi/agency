//! A guest that cannot be told now does not hold the other guests: each is tried in the same pass, and the
//! pass still says it failed.
mod common;

use agency_core::guests::{self, Guest};
use agency_core::orgs;
use agency_core::push::{notify_waiting, NoticeKind};
use agency_proto::client::VapidKey;
use agency_proto::org::{Device, Person};
use agency_proto::{AccountKey, Timestamp};
use common::guests::{call, with_guest_and_runtime, CAROL};
use common::id;
use reqwest::Method;
use serde_json::json;

const DAVE: &str = "dave@example.com";
const CAROLS_TOKEN: &str = "ca11ca11ca11ca11ca11ca11ca11ca11ca11ca11ca11ca11ca11ca11ca11ca11";
const DAVES_BROWSER: &str =
    "https://fcm.googleapis.com/fcm/send/dave:APA91bHun4MxP5egoKMwt2KZFBaFUH";
const P256DH: &str =
    "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
const AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";

#[tokio::test]
async fn a_guest_whose_notice_cannot_be_kept_does_not_hold_the_next_guest() {
    let (alice, _, memory) = with_guest_and_runtime().await;
    let node = &alice.node;
    // dave: a second hosted member, addressed as `legal`, signed in as a guest with a browser.
    let key = AccountKey::from_seed([0x84; 32]);
    let device = Device {
        id: key.public().device_id(),
        label: "Desktop".into(),
        public_key: key.public(),
        added_at: Timestamp::parse(&node.now_text()).unwrap(),
    };
    orgs::amend(node, &id("acme"), |body| {
        body.people
            .push(Person::hosted(id("dave"), "Dave", vec![device]));
        Ok(())
    })
    .unwrap();
    node.update_config(|config| {
        config.teams[0]
            .people
            .insert("legal".to_owned(), id("dave"));
    })
    .unwrap();
    guests::keep(node, &Guest::new(DAVE, id("acme"), id("dave"))).unwrap();
    alice.notifier.0.lock().unwrap().web_key = Some(VapidKey {
        public_key: "BPublic".into(),
        key_id: "key-one".into(),
    });
    let token = json!({ "token": CAROLS_TOKEN, "environment": "sandbox", "label": "iPhone" });
    let (status, answer) = call(&alice, Method::POST, "/api/push/tokens", CAROL, Some(token)).await;
    assert_eq!(status, 200, "{answer}");
    let browser =
        json!({ "endpoint": DAVES_BROWSER, "p256dh": P256DH, "auth": AUTH, "label": "Firefox" });
    let (status, answer) = call(
        &alice,
        Method::POST,
        "/api/push/subscriptions",
        DAVE,
        Some(browser),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    notify_waiting(node).await.unwrap();

    {
        let mut state = memory.lock().unwrap();
        state.requests.push(("r-3".into(), "Pay it?".into()));
        state.to.insert("r-3".into(), "finance".into());
        state.requests.push(("r-4".into(), "Sign it?".into()));
        state.to.insert("r-4".into(), "legal".into());
    }
    // What is learned of carol's token after a send can no longer be written: the file is put in place
    // through a temporary file of this name, and a directory stands there.
    let blocked = alice
        .data_dir()
        .join(format!("push-tokens.tmp-{}", std::process::id()));
    std::fs::create_dir_all(&blocked).unwrap();

    assert!(
        notify_waiting(node).await.is_err(),
        "the pass says it failed"
    );
    let to_dave: Vec<String> = alice
        .notifier
        .0
        .lock()
        .unwrap()
        .sent_web
        .iter()
        .map(|(_, notice)| match &notice.kind {
            NoticeKind::Request { request, .. } => request.clone(),
            other => panic!("only requests: {other:?}"),
        })
        .collect();
    assert_eq!(to_dave, ["r-4"], "dave is told although carol could not be");
}
