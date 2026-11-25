//! Push notifications for the owner: registering APNs device tokens through the Client API, and
//! notifying once per request that starts waiting for the owner, on a team here or on another node.
mod common;

use agency_core::push::notify_waiting;
use agency_core::views::refresh;
use common::{alice, body, get, id, joined_bob, Running, TOKEN};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

const TOKEN_HEX: &str = "AB12CD34EF56AB12CD34EF56AB12CD34EF56AB12CD34EF56AB12CD34EF56AB12";

fn tokens_path(running: &Running) -> String {
    format!("{}/api/push/tokens", running.url())
}

async fn register(running: &Running, input: &Value) -> (StatusCode, Value) {
    body(
        Client::new()
            .post(tokens_path(running))
            .header("x-agency-token", TOKEN)
            .json(input)
            .send()
            .await
            .unwrap(),
    )
    .await
}

fn token(environment: &str) -> Value {
    json!({ "token": TOKEN_HEX, "environment": environment, "label": "iPhone" })
}

#[tokio::test]
async fn the_owner_registers_and_removes_device_tokens() {
    let (alice, _) = alice().await;
    let (status, registered) = register(&alice, &token("sandbox")).await;
    assert_eq!(status, 200, "{registered}");
    assert_eq!(registered["token"], TOKEN_HEX.to_ascii_lowercase());
    let (status, _) = register(&alice, &token("production")).await;
    assert_eq!(status, 200);
    let listed = body(get(&alice, "/api/push/tokens").send().await.unwrap())
        .await
        .1;
    assert_eq!(
        listed["tokens"],
        json!([{ "token": "34ef56ab12"[2..], "environment": "production", "label": "iPhone", "registeredAt": "2026-11-24T12:00:00Z" }]),
        "one entry per token, shortened in lists"
    );

    for (name, input) in [
        (
            "not hex",
            json!({ "token": "zz".repeat(32), "environment": "sandbox", "label": "iPhone" }),
        ),
        (
            "too short",
            json!({ "token": "ab".repeat(15), "environment": "sandbox", "label": "iPhone" }),
        ),
        (
            "odd length",
            json!({ "token": "a".repeat(65), "environment": "sandbox", "label": "iPhone" }),
        ),
        (
            "no label",
            json!({ "token": TOKEN_HEX, "environment": "sandbox", "label": " " }),
        ),
        (
            "unknown environment",
            json!({ "token": TOKEN_HEX, "environment": "staging", "label": "iPhone" }),
        ),
    ] {
        assert_eq!(register(&alice, &input).await.0, 400, "{name}");
    }
    let unsigned = Client::new()
        .post(tokens_path(&alice))
        .json(&token("sandbox"))
        .send()
        .await
        .unwrap();
    assert_eq!(unsigned.status(), 403, "a write needs the page's token");

    let remove = |with_token: bool| {
        let request = Client::new().delete(format!("{}/{TOKEN_HEX}", tokens_path(&alice)));
        if with_token {
            request.header("x-agency-token", TOKEN)
        } else {
            request
        }
    };
    assert_eq!(remove(false).send().await.unwrap().status(), 403);
    assert_eq!(remove(true).send().await.unwrap().status(), 204);
    assert_eq!(
        remove(true).send().await.unwrap().status(),
        204,
        "already gone"
    );
    let listed = body(get(&alice, "/api/push/tokens").send().await.unwrap())
        .await
        .1;
    assert_eq!(listed["tokens"], json!([]));
}

fn add_request(
    memory: &std::sync::Arc<std::sync::Mutex<agency_adapters::memory::MemoryRuntimeState>>,
    id: &str,
    title: &str,
) {
    memory
        .lock()
        .unwrap()
        .requests
        .push((id.into(), title.into()));
}

#[tokio::test]
async fn each_new_request_for_the_owner_is_notified_once() {
    let (alice, memory) = alice().await;
    register(&alice, &token("sandbox")).await;
    let first = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(first.sent, 0, "requests already waiting are only recorded");

    add_request(&memory, "r-2", "Ship it?");
    assert_eq!(notify_waiting(&alice.node).await.unwrap().sent, 1);
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        0,
        "once per request"
    );
    let sent = alice.notifier.0.lock().unwrap().sent.clone();
    assert_eq!(sent.len(), 1);
    let (to, notice) = &sent[0];
    assert_eq!(to, &TOKEN_HEX.to_ascii_lowercase());
    assert_eq!(
        (
            notice.title.as_str(),
            notice.body.as_str(),
            notice.request.as_str()
        ),
        ("app", "Ship it?", "r-2")
    );
    assert_eq!(notice.badge, 2, "r-1 and r-2 wait for alice");
    assert_eq!(
        (notice.org.clone(), notice.team.clone()),
        (id("acme"), id("app"))
    );

    alice
        .node
        .update_config(|config| config.teams[0].owner = id("bob"))
        .unwrap();
    add_request(&memory, "r-3", "For bob");
    assert_eq!(
        notify_waiting(&alice.node).await.unwrap().sent,
        0,
        "a request addressed to someone else"
    );
    alice
        .node
        .update_config(|config| config.teams[0].owner = id("alice"))
        .unwrap();

    alice.notifier.0.lock().unwrap().failing = true;
    add_request(&memory, "r-4", "Later");
    let failed = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(
        (failed.sent, failed.failures.len()),
        (0, 2),
        "r-3 (alice's again) and r-4 both fail"
    );
    alice.notifier.0.lock().unwrap().failing = false;
    let retried = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(retried.sent, 2, "both are tried again");
}

#[tokio::test]
async fn a_token_apns_no_longer_accepts_is_removed() {
    let (alice, memory) = alice().await;
    register(&alice, &token("production")).await;
    notify_waiting(&alice.node).await.unwrap();
    alice
        .notifier
        .0
        .lock()
        .unwrap()
        .gone
        .push(TOKEN_HEX.to_ascii_lowercase());
    add_request(&memory, "r-2", "Ship it?");
    notify_waiting(&alice.node).await.unwrap();
    let listed = body(get(&alice, "/api/push/tokens").send().await.unwrap())
        .await
        .1;
    assert_eq!(listed["tokens"], json!([]));
}

#[tokio::test]
async fn a_member_is_notified_of_requests_on_another_node_addressed_to_them() {
    let (alice, memory) = alice().await;
    alice
        .node
        .update_config(|config| {
            config.teams[0].people.insert("reviewer".into(), id("bob"));
        })
        .unwrap();
    let bob = joined_bob(&alice).await;
    register(&bob, &token("production")).await;
    refresh(&bob.node).await.unwrap();
    notify_waiting(&bob.node).await.unwrap();

    add_request(&memory, "r-2", "Ship it?");
    refresh(&bob.node).await.unwrap();
    assert_eq!(
        notify_waiting(&bob.node).await.unwrap().sent,
        0,
        "a request without `to` goes to the team's owner, not to the reviewer"
    );

    alice
        .node
        .update_config(|config| {
            config.teams[0].people.clear();
            config.teams[0].owner = id("bob");
        })
        .unwrap();
    add_request(&memory, "r-3", "Ship this too?");
    refresh(&bob.node).await.unwrap();
    let pass = notify_waiting(&bob.node).await.unwrap();
    assert_eq!(
        pass.sent, 3,
        "bob now owns the team, so r-1, r-2, and r-3 start waiting for him"
    );
    let sent = bob.notifier.0.lock().unwrap().sent.clone();
    assert!(sent.iter().any(|(_, notice)| notice.request == "r-3"));
    assert!(sent.iter().all(|(_, notice)| notice.badge == 3));
}
