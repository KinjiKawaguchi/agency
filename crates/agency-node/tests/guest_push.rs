//! A guest's notices: carol's app and browsers are hers on alice's node, get only the requests that wait for
//! carol in the teams there, with a badge of her own, and nothing of the owner's.
mod common;

use agency_core::push::{notify_waiting, NoticeKind};
use common::guests::{call, with_guest, with_guest_and_runtime, CAROL};
use common::id;
use reqwest::Method;
use serde_json::json;

const OWNER: &str = "alice@example.com";
const CAROLS_TOKEN: &str = "CA11CA11CA11CA11CA11CA11CA11CA11CA11CA11CA11CA11CA11CA11CA11CA11";
const ALICES_TOKEN: &str = "A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1A1";

fn token(raw: &str) -> serde_json::Value {
    json!({
        "token": raw,
        "environment": "sandbox",
        "label": "iPhone",
        "events": ["request", "device", "decision", "root"],
    })
}

#[tokio::test]
async fn a_guests_app_is_its_own_and_gets_only_requests() {
    let (alice, _) = with_guest().await;
    let (status, carols) = call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        CAROL,
        Some(token(CAROLS_TOKEN)),
    )
    .await;
    assert_eq!(status, 200, "{carols}");
    assert_eq!(
        carols["events"],
        json!(["request"]),
        "a guest gets only requests"
    );
    assert_eq!(carols["guest"], json!({ "member": "carol", "org": "acme" }));
    let (status, alices) = call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        OWNER,
        Some(token(ALICES_TOKEN)),
    )
    .await;
    assert_eq!(status, 200, "{alices}");
    assert!(
        alices.get("guest").is_none(),
        "the owner's is not a guest's: {alices}"
    );
    assert_eq!(alices["events"].as_array().unwrap().len(), 4);

    let (_, listed) = call(&alice, Method::GET, "/api/push/tokens", CAROL, None).await;
    assert_eq!(
        listed["tokens"].as_array().unwrap().len(),
        1,
        "carol's own: {listed}"
    );
    assert_eq!(
        listed["tokens"][0]["token"],
        CAROLS_TOKEN[56..].to_ascii_lowercase()
    );
    let (_, listed) = call(&alice, Method::GET, "/api/push/tokens", OWNER, None).await;
    assert_eq!(
        listed["tokens"].as_array().unwrap().len(),
        1,
        "alice's own: {listed}"
    );

    let (status, answer) = call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        CAROL,
        Some(token(ALICES_TOKEN)),
    )
    .await;
    assert_eq!(
        status, 409,
        "a guest does not take over the owner's app: {answer}"
    );
    let (status, _) = call(
        &alice,
        Method::DELETE,
        &format!("/api/push/tokens/{ALICES_TOKEN}"),
        CAROL,
        None,
    )
    .await;
    assert_eq!(status, 204);
    let (_, listed) = call(&alice, Method::GET, "/api/push/tokens", OWNER, None).await;
    assert_eq!(
        listed["tokens"].as_array().unwrap().len(),
        1,
        "nor removes it"
    );
    let (status, _) = call(
        &alice,
        Method::DELETE,
        &format!("/api/push/tokens/{CAROLS_TOKEN}"),
        CAROL,
        None,
    )
    .await;
    assert_eq!(status, 204);
    let (_, listed) = call(&alice, Method::GET, "/api/push/tokens", CAROL, None).await;
    assert_eq!(listed["tokens"], json!([]), "she removes her own");
}

#[tokio::test]
async fn a_guest_is_told_of_its_own_requests_with_its_own_badge_and_the_owner_of_none_of_them() {
    let (alice, _, memory) = with_guest_and_runtime().await;
    call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        CAROL,
        Some(token(CAROLS_TOKEN)),
    )
    .await;
    call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        OWNER,
        Some(token(ALICES_TOKEN)),
    )
    .await;
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        0,
        "what waits is only recorded"
    );

    {
        let mut state = memory.lock().unwrap();
        state
            .requests
            .push(("r-3".into(), "Pay the invoice?".into()));
        state.to.insert("r-3".into(), "finance".into());
        state.requests.push(("r-4".into(), "Ship it?".into()));
    }
    let pass = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(pass.sent, 2, "{pass:?}");
    let sent = alice.notifier.0.lock().unwrap().sent.clone();
    let to = |raw: &str| -> Vec<(String, u32)> {
        sent.iter()
            .filter(|(token, _)| *token == raw.to_ascii_lowercase())
            .map(|(_, notice)| match &notice.kind {
                NoticeKind::Request { request, .. } => (request.clone(), notice.badge),
                other => panic!("only requests: {other:?}"),
            })
            .collect()
    };
    assert_eq!(
        to(CAROLS_TOKEN),
        vec![("r-3".to_owned(), 2)],
        "r-2 and r-3 wait for carol"
    );
    assert_eq!(
        to(ALICES_TOKEN),
        vec![("r-4".to_owned(), 2)],
        "r-1 and r-4 wait for alice"
    );
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        0,
        "once each"
    );

    alice
        .node
        .update_config(|config| config.hide_titles = true)
        .unwrap();
    {
        let mut state = memory.lock().unwrap();
        state.requests.push(("r-5".into(), "Secret".into()));
        state.to.insert("r-5".into(), "finance".into());
    }
    notify_waiting(&alice.node).await.unwrap();
    let sent = alice.notifier.0.lock().unwrap().sent.clone();
    let (_, last) = sent.last().unwrap();
    assert_eq!(
        last.kind,
        NoticeKind::Request {
            org: id("acme"),
            team: id("app"),
            request: "r-5".into(),
            team_label: None,
            title: None,
        },
        "titles follow hideTitles"
    );
}

#[tokio::test]
async fn a_guest_who_is_no_longer_hosted_is_told_nothing() {
    let (alice, _, memory) = with_guest_and_runtime().await;
    call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        CAROL,
        Some(token(CAROLS_TOKEN)),
    )
    .await;
    notify_waiting(&alice.node).await.unwrap();
    agency_core::orgs::amend(&alice.node, &id("acme"), |body| {
        body.people.retain(|person| person.id != id("carol"));
        Ok(())
    })
    .unwrap();
    {
        let mut state = memory.lock().unwrap();
        state
            .requests
            .push(("r-3".into(), "Pay the invoice?".into()));
        state.to.insert("r-3".into(), "finance".into());
    }
    notify_waiting(&alice.node).await.unwrap();
    let sent = alice.notifier.0.lock().unwrap().sent.clone();
    assert!(
        sent.iter()
            .all(|(token, _)| *token != CAROLS_TOKEN.to_ascii_lowercase()),
        "{sent:?}"
    );
}

const P256DH: &str =
    "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
const AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
const CAROLS_BROWSER: &str =
    "https://fcm.googleapis.com/fcm/send/carol:APA91bHun4MxP5egoKMwt2KZFBaFUH";
const ALICES_BROWSER: &str =
    "https://fcm.googleapis.com/fcm/send/alice:APA91bHun4MxP5egoKMwt2KZFBaFUH";

fn browser(endpoint: &str) -> serde_json::Value {
    json!({
        "endpoint": endpoint,
        "p256dh": P256DH,
        "auth": AUTH,
        "label": "Firefox",
        "events": ["request", "device"],
    })
}

#[tokio::test]
async fn a_guests_browser_is_its_own_and_is_told_only_of_its_requests() {
    let (alice, _, memory) = with_guest_and_runtime().await;
    alice.notifier.0.lock().unwrap().web_key = Some(agency_proto::client::VapidKey {
        public_key: "BPublic".into(),
        key_id: "key-one".into(),
    });
    let path = "/api/push/subscriptions";
    let (status, carols) = call(
        &alice,
        Method::POST,
        path,
        CAROL,
        Some(browser(CAROLS_BROWSER)),
    )
    .await;
    assert_eq!(status, 200, "{carols}");
    assert_eq!(
        carols["events"],
        json!(["request"]),
        "a guest gets only requests"
    );
    let (status, alices) = call(
        &alice,
        Method::POST,
        path,
        OWNER,
        Some(browser(ALICES_BROWSER)),
    )
    .await;
    assert_eq!(status, 200, "{alices}");
    let (carols_id, alices_id) = (
        carols["id"].as_str().unwrap(),
        alices["id"].as_str().unwrap(),
    );

    let (_, listed) = call(&alice, Method::GET, path, CAROL, None).await;
    assert_eq!(
        listed["subscriptions"].as_array().unwrap().len(),
        1,
        "{listed}"
    );
    assert_eq!(listed["subscriptions"][0]["id"], carols_id);
    let (_, listed) = call(&alice, Method::GET, path, OWNER, None).await;
    assert_eq!(
        listed["subscriptions"].as_array().unwrap().len(),
        1,
        "{listed}"
    );
    assert_eq!(listed["subscriptions"][0]["id"], alices_id);

    let (status, answer) = call(
        &alice,
        Method::POST,
        path,
        CAROL,
        Some(browser(ALICES_BROWSER)),
    )
    .await;
    assert_eq!(
        status, 409,
        "a guest does not take over the owner's browser: {answer}"
    );
    let (status, _) = call(
        &alice,
        Method::DELETE,
        &format!("{path}/{alices_id}"),
        CAROL,
        None,
    )
    .await;
    assert_eq!(status, 204);
    let (_, listed) = call(&alice, Method::GET, path, OWNER, None).await;
    assert_eq!(
        listed["subscriptions"].as_array().unwrap().len(),
        1,
        "nor removes it"
    );

    notify_waiting(&alice.node).await.unwrap();
    {
        let mut state = memory.lock().unwrap();
        state
            .requests
            .push(("r-3".into(), "Pay the invoice?".into()));
        state.to.insert("r-3".into(), "finance".into());
        state.requests.push(("r-4".into(), "Ship it?".into()));
    }
    notify_waiting(&alice.node).await.unwrap();
    let sent = alice.notifier.0.lock().unwrap().sent_web.clone();
    let to = |id: &str| -> Vec<String> {
        sent.iter()
            .filter(|(to, _)| to == id)
            .map(|(_, notice)| match &notice.kind {
                NoticeKind::Request { request, .. } => request.clone(),
                other => panic!("only requests: {other:?}"),
            })
            .collect()
    };
    assert_eq!(to(carols_id), vec!["r-3".to_owned()]);
    assert_eq!(to(alices_id), vec!["r-4".to_owned()]);
}

#[tokio::test]
async fn the_owners_desk_wakes_only_the_owners_iphones() {
    let (alice, _, memory) = with_guest_and_runtime().await;
    call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        CAROL,
        Some(token(CAROLS_TOKEN)),
    )
    .await;
    call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        OWNER,
        Some(token(ALICES_TOKEN)),
    )
    .await;
    notify_waiting(&alice.node).await.unwrap();
    alice.clock.advance(agency_proto::clock::Duration::minutes(
        agency_core::push::REFRESH_FLOOR_MINUTES,
    ));
    memory
        .lock()
        .unwrap()
        .requests
        .retain(|(one, _)| one != "r-1");
    notify_waiting(&alice.node).await.unwrap();
    let woken = alice.notifier.0.lock().unwrap().desk_changed.clone();
    assert_eq!(woken, vec![ALICES_TOKEN.to_ascii_lowercase()]);
}

#[tokio::test]
async fn an_owners_notice_tried_again_never_goes_to_a_guest_that_registered_the_token_since() {
    let (alice, _, memory) = with_guest_and_runtime().await;
    call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        OWNER,
        Some(token(ALICES_TOKEN)),
    )
    .await;
    notify_waiting(&alice.node).await.unwrap();
    alice.notifier.0.lock().unwrap().failing = true;
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-4".into(), "Ship it?".into()));
    let failed = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(failed.failures.len(), 1, "{failed:?}");

    let gone = format!("/api/push/tokens/{ALICES_TOKEN}");
    assert_eq!(
        call(&alice, Method::DELETE, &gone, OWNER, None).await.0,
        204
    );
    let (status, _) = call(
        &alice,
        Method::POST,
        "/api/push/tokens",
        CAROL,
        Some(token(ALICES_TOKEN)),
    )
    .await;
    assert_eq!(status, 200, "the token is free again");
    alice.notifier.0.lock().unwrap().failing = false;
    notify_waiting(&alice.node).await.unwrap();
    let sent = alice.notifier.0.lock().unwrap().sent.clone();
    assert!(
        sent.iter().all(|(_, notice)| !matches!(
            &notice.kind,
            NoticeKind::Request { request, .. } if request == "r-4"
        )),
        "the owner's r-4 is not carol's: {sent:?}"
    );
}
