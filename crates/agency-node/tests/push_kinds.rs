//! The kinds of push notification: a token names the kinds it wants, and the node notifies a device that
//! asks to be added and a decision that was refused, once each, besides the requests that wait.
mod common;

use agency_core::config::PushConfig;
use agency_core::notice::{Notice, NoticeKind};
use agency_core::push::notify_waiting;
use agency_proto::AccountKey;
use common::{alice, body, device, id, joined_bob, post, sign_answer, Running, ENVELOPES, TOKEN};
use reqwest::StatusCode;
use serde_json::{json, Value};

const TOKEN_A: &str = "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12";
const TOKEN_B: &str = "cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34";

async fn write(running: &Running, path: &str, input: &Value) -> (StatusCode, Value) {
    let request = post(running, path, input).header("x-agency-token", TOKEN);
    body(request.send().await.unwrap()).await
}

async fn register(running: &Running, token: &str, events: Option<Value>) -> (StatusCode, Value) {
    let mut input = json!({ "token": token, "environment": "sandbox", "label": "iPhone" });
    if let Some(events) = events {
        input["events"] = events;
    }
    write(running, "/api/push/tokens", &input).await
}

async fn ask_to_add(running: &Running, label: &str, seed: u8) {
    let key = AccountKey::from_seed([seed; 32]).public();
    let input = json!({ "label": label, "publicKey": key.encoded() });
    let (status, answer) = write(running, "/api/devices/requests", &input).await;
    assert_eq!(status, 200, "{answer}");
}

/// What the notifier was asked to send so far: the token it went to, and the notice.
fn sent(running: &Running) -> Vec<(String, Notice)> {
    running.notifier.0.lock().unwrap().sent.clone()
}

fn hide_titles(running: &Running) {
    let push = PushConfig {
        key_path: "/nonexistent.p8".into(),
        key_id: "ABCDE12345".into(),
        team_id: "ABCDE12345".into(),
        bundle_id: "com.example.agency".into(),
        hide_titles: true,
    };
    running
        .node
        .update_config(|config| config.push = Some(push))
        .unwrap();
}

#[tokio::test]
async fn a_token_names_the_kinds_it_wants() {
    let (alice, _) = alice().await;
    let listed = |alice: &Running| {
        let tokens = alice.node.store().push_tokens();
        tokens.into_iter().map(|one| one.events).collect::<Vec<_>>()
    };
    let (status, registered) = register(&alice, TOKEN_A, None).await;
    assert_eq!(
        (status, &registered["events"]),
        (StatusCode::OK, &json!(["request"]))
    );
    let all = Some(json!(["decision", "request", "device", "request"]));
    let (_, registered) = register(&alice, TOKEN_B, all).await;
    assert_eq!(
        registered["events"],
        json!(["request", "device", "decision"]),
        "each kind once, in one order"
    );
    let (_, none) = register(&alice, TOKEN_A, Some(json!([]))).await;
    assert_eq!(
        none["events"],
        json!([]),
        "an empty list is no notifications"
    );
    assert_eq!(
        listed(&alice).len(),
        2,
        "a known token is updated, not added"
    );

    let (status, refused) = register(&alice, TOKEN_A, Some(json!(["banner"]))).await;
    assert_eq!(
        status, 400,
        "an unknown kind is refused, not ignored: {refused}"
    );
    let after = body(
        common::get(&alice, "/api/push/tokens")
            .send()
            .await
            .unwrap(),
    )
    .await;
    let kept = after.1["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["token"] == TOKEN_A[TOKEN_A.len() - 8..])
        .unwrap();
    assert_eq!(
        kept["events"],
        json!([]),
        "a refused registration changes nothing"
    );
}

#[tokio::test]
async fn a_device_that_asks_to_be_added_is_notified_once_to_the_tokens_that_want_it() {
    let (alice, _) = alice().await;
    register(&alice, TOKEN_A, Some(json!(["request", "device"]))).await;
    register(&alice, TOKEN_B, None).await;
    let first = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(first.sent, 0, "the first pass only records");

    ask_to_add(&alice, "Tablet", 21).await;
    assert_eq!(notify_waiting(&alice.node).await.unwrap().sent, 1);
    let notices = sent(&alice);
    assert_eq!(notices.len(), 1, "only the token that wants devices");
    let (to, notice) = &notices[0];
    assert_eq!(to, TOKEN_A);
    let NoticeKind::Device { device, label } = &notice.kind else {
        panic!("a device notice: {notice:?}");
    };
    let key = AccountKey::from_seed([21; 32]).public();
    assert_eq!(device, &key.device_id());
    assert_eq!(label.as_deref(), Some("Tablet"));
    assert_eq!(
        notice.badge, 1,
        "r-1 waits for alice; the device does not count"
    );

    assert_eq!(notify_waiting(&alice.node).await.unwrap().sent, 0, "once");
    ask_to_add(&alice, "Tablet again", 21).await;
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        0,
        "the same key is the same device"
    );
}

#[tokio::test]
async fn titles_and_labels_are_left_out_when_the_node_hides_them() {
    let (alice, memory) = alice().await;
    hide_titles(&alice);
    let all = Some(json!(["request", "device", "decision"]));
    register(&alice, TOKEN_A, all).await;
    notify_waiting(&alice.node).await.unwrap();
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-2".into(), "Ship it?".into()));
    ask_to_add(&alice, "Tablet", 21).await;
    assert_eq!(notify_waiting(&alice.node).await.unwrap().sent, 2);
    let kinds: Vec<NoticeKind> = sent(&alice).into_iter().map(|(_, n)| n.kind).collect();
    assert!(
        kinds.iter().any(|kind| matches!(
            kind,
            NoticeKind::Request { team_label: None, title: None, request, .. } if request == "r-2"
        )),
        "{kinds:?}"
    );
    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, NoticeKind::Device { label: None, .. })),
        "{kinds:?}"
    );
}

/// bob's node after alice's node refused a decision of his (no request r-404 waits), with `bob` wanting
/// decisions. Returns the notices sent for it.
async fn refused_decision(is_hidden: bool) -> (Vec<Notice>, Running) {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    if is_hidden {
        hide_titles(&bob);
    }
    let key = device(&bob).await;
    register(&bob, TOKEN_A, Some(json!(["decision"]))).await;
    notify_waiting(&bob.node).await.unwrap();

    let payload = agency_proto::Payload {
        text: "Go".into(),
        choice: Some("a".into()),
    };
    let envelope = sign_answer(&bob, &key, "r-404", payload);
    let input = json!({ "envelope": envelope, "title": "Unknown" });
    let (status, _) = write(&bob, ENVELOPES, &input).await;
    assert_eq!(status, 409, "the team's node refused it");
    assert_eq!(notify_waiting(&bob.node).await.unwrap().sent, 1);
    assert_eq!(notify_waiting(&bob.node).await.unwrap().sent, 0, "once");
    let notices = sent(&bob).into_iter().map(|(_, notice)| notice).collect();
    (notices, bob)
}

#[tokio::test]
async fn a_decision_is_notified_once_when_it_is_refused() {
    let (notices, bob) = refused_decision(false).await;
    assert_eq!(notices.len(), 1);
    let NoticeKind::Decision {
        org,
        team,
        id: decision,
        title,
    } = &notices[0].kind
    else {
        panic!("a decision notice: {:?}", notices[0]);
    };
    assert_eq!((org, team), (&id("acme"), &id("app")));
    assert_eq!(title.as_deref(), Some("Unknown"));
    let logged = agency_core::client::decision_log(&bob.node, None);
    assert_eq!(&logged.entries[0].id, decision);
    assert_eq!(notices[0].badge, 0, "bob owns no waiting request");
}

#[tokio::test]
async fn a_refused_decisions_title_is_left_out_when_the_node_hides_titles() {
    let (notices, _bob) = refused_decision(true).await;
    assert!(matches!(
        &notices[0].kind,
        NoticeKind::Decision { title: None, .. }
    ));
}

#[tokio::test]
async fn an_upgrade_records_the_device_requests_and_refusals_already_there() {
    let (alice, _) = alice().await;
    register(&alice, TOKEN_A, Some(json!(["request", "device"]))).await;
    ask_to_add(&alice, "Tablet", 21).await;
    // A notified list from a node that only notified requests: no marker of the other kinds.
    alice
        .node
        .store()
        .save_notified(&["acme/app/r-1".to_owned()])
        .unwrap();
    let pass = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(pass.sent, 0, "the waiting device request is not announced");
    ask_to_add(&alice, "Phone", 22).await;
    assert_eq!(notify_waiting(&alice.node).await.unwrap().sent, 1);
}
