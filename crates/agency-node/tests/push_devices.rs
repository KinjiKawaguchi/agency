//! Push registrations follow the account's devices: one may name the device it belongs to, only a current
//! device is accepted, and a definition that removes the device takes its registrations with it, in the
//! log without the token or the endpoint.
mod common;

use agency_core::devices::add_device;
use agency_proto::client::VapidKey;
use agency_proto::{AccountKey, Device, DeviceChange, DeviceOp, Kind, Signed, Timestamp};
use common::{alice, body, get, id, joined_bob, post, Running, T0, TOKEN};
use reqwest::StatusCode;
use serde_json::{json, Value};

const TOKEN_HEX: &str = "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12";
const OTHER_TOKEN_HEX: &str = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
const THIRD_TOKEN_HEX: &str = "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321";
const P256DH: &str =
    "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
const AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
const ENDPOINT: &str =
    "https://fcm.googleapis.com/fcm/send/dQw4w9WgXcQ:APA91bHun4MxP5egoKMwt2KZFBaFUH";
const OTHER_ENDPOINT: &str =
    "https://fcm.googleapis.com/fcm/send/other-browser-APA91bHun4MxP5egoKMw";

fn with_web_push(running: &Running) {
    running.notifier.0.lock().unwrap().web_key = Some(VapidKey {
        public_key: "BPublic".into(),
        key_id: "key-one".into(),
    });
}

async fn send(running: &Running, path: &str, payload: &Value) -> (StatusCode, Value) {
    let request = post(running, path, payload).header("x-agency-token", TOKEN);
    body(request.send().await.unwrap()).await
}

async fn register_token(running: &Running, token: &str, device: Option<&str>) -> StatusCode {
    let mut input = json!({ "token": token, "environment": "sandbox", "label": "iPhone" });
    if let Some(device) = device {
        input["device"] = json!(device);
    }
    send(running, "/api/push/tokens", &input).await.0
}

async fn register_browser(running: &Running, endpoint: &str, device: Option<&str>) -> StatusCode {
    let mut input =
        json!({ "endpoint": endpoint, "p256dh": P256DH, "auth": AUTH, "label": "Firefox" });
    if let Some(device) = device {
        input["device"] = json!(device);
    }
    send(running, "/api/push/subscriptions", &input).await.0
}

async fn listed(running: &Running, path: &str, key: &str) -> Vec<Value> {
    let answer = body(get(running, path).send().await.unwrap()).await.1;
    answer[key].as_array().unwrap().clone()
}

async fn tokens(running: &Running) -> Vec<Value> {
    listed(running, "/api/push/tokens", "tokens").await
}

async fn browsers(running: &Running) -> Vec<Value> {
    listed(running, "/api/push/subscriptions", "subscriptions").await
}

async fn delete(running: &Running, path: &str) -> StatusCode {
    reqwest::Client::new()
        .delete(format!("{}{path}", running.url()))
        .header("x-agency-token", TOKEN)
        .send()
        .await
        .unwrap()
        .status()
}

fn device_change(account: &str, op: DeviceOp) -> DeviceChange {
    DeviceChange {
        org: id("acme"),
        account: id(account),
        op,
        at: Timestamp::parse(T0).unwrap(),
    }
}

fn add_op(key: &AccountKey, label: &str) -> DeviceOp {
    DeviceOp::Add {
        device: Box::new(Device {
            id: key.public().device_id(),
            label: label.into(),
            public_key: key.public(),
            added_at: Timestamp::parse(T0).unwrap(),
        }),
    }
}

async fn change(running: &Running, account: &str, op: DeviceOp, by: &AccountKey) {
    let signer = by.public().device_id();
    let signed = Signed::sign(Kind::Device, device_change(account, op), signer, by).unwrap();
    let (status, answer) = send(running, "/api/devices/changes", &json!(signed)).await;
    assert_eq!(status, 200, "{answer}");
}

fn device_id(key: &AccountKey) -> String {
    key.public().device_id().to_string()
}

/// An account with two devices: the first added by the terminal, the second by the first.
async fn with_two_devices(running: &Running, account: &str) -> (AccountKey, AccountKey) {
    let (phone, laptop) = (AccountKey::generate(), AccountKey::generate());
    add_device(&running.node, "Phone", &phone.public(), None).await;
    change(running, account, add_op(&laptop, "Laptop"), &phone).await;
    (phone, laptop)
}

fn log_of(running: &Running) -> String {
    std::fs::read_to_string(running.data_dir().join("logs/node.jsonl")).unwrap_or_default()
}

#[tokio::test]
async fn a_registration_names_a_current_device_of_the_account_or_is_refused() {
    let (alice, _) = alice().await;
    with_web_push(&alice);
    let (phone, _) = with_two_devices(&alice, "alice").await;
    let stranger = device_id(&AccountKey::generate());

    assert_eq!(
        register_token(&alice, TOKEN_HEX, Some(&stranger)).await,
        400
    );
    assert_eq!(
        register_browser(&alice, ENDPOINT, Some(&stranger)).await,
        400
    );
    assert_eq!(
        (tokens(&alice).await.len(), browsers(&alice).await.len()),
        (0, 0),
        "a refused registration keeps nothing"
    );

    assert_eq!(
        register_token(&alice, TOKEN_HEX, Some(&device_id(&phone))).await,
        200
    );
    assert_eq!(
        register_browser(&alice, ENDPOINT, Some(&device_id(&phone))).await,
        200
    );
    assert_eq!(tokens(&alice).await[0]["device"], device_id(&phone));
    assert_eq!(browsers(&alice).await[0]["device"], device_id(&phone));

    let not_an_id = send(
        &alice,
        "/api/push/tokens",
        &json!({ "token": TOKEN_HEX, "environment": "sandbox", "label": "iPhone", "device": 7 }),
    )
    .await;
    assert_eq!(not_an_id.0, 400, "a device that is not an id is not read");
}

#[tokio::test]
async fn a_registration_without_a_device_is_as_before_and_names_none() {
    let (alice, _) = alice().await;
    with_web_push(&alice);
    assert_eq!(register_token(&alice, TOKEN_HEX, None).await, 200);
    assert_eq!(register_browser(&alice, ENDPOINT, None).await, 200);
    assert!(tokens(&alice).await[0].get("device").is_none());
    assert!(browsers(&alice).await[0].get("device").is_none());
}

#[tokio::test]
async fn removing_a_device_drops_its_registrations_of_both_kinds_and_logs_it_without_the_secrets() {
    let (alice, _) = alice().await;
    with_web_push(&alice);
    let (phone, laptop) = with_two_devices(&alice, "alice").await;
    let (phone_id, laptop_id) = (device_id(&phone), device_id(&laptop));

    register_token(&alice, TOKEN_HEX, Some(&phone_id)).await;
    register_token(&alice, OTHER_TOKEN_HEX, Some(&laptop_id)).await;
    register_token(&alice, THIRD_TOKEN_HEX, None).await;
    register_browser(&alice, ENDPOINT, Some(&phone_id)).await;
    register_browser(&alice, OTHER_ENDPOINT, Some(&laptop_id)).await;

    let lost = DeviceOp::Remove {
        device: phone.public().device_id(),
    };
    change(&alice, "alice", lost, &laptop).await;

    let kept: Vec<String> = tokens(&alice)
        .await
        .iter()
        .map(|one| one["token"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        kept,
        [
            OTHER_TOKEN_HEX[56..].to_owned(),
            THIRD_TOKEN_HEX[56..].to_owned()
        ],
        "the phone's token is gone; the laptop's and the one naming no device stay"
    );
    let kept = browsers(&alice).await;
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0]["device"], laptop_id);

    let log = log_of(&alice);
    assert!(
        log.contains(&phone_id),
        "the device is named in the log: {log}"
    );
    assert!(
        log.contains("push tokens: 1, browser subscriptions: 1"),
        "{log}"
    );
    for secret in [
        TOKEN_HEX,
        &TOKEN_HEX[56..],
        "fcm.googleapis.com",
        "dQw4w9WgXcQ",
        &ENDPOINT[ENDPOINT.len() - 8..],
        P256DH,
        AUTH,
    ] {
        assert!(!log.contains(secret), "the log holds {secret}: {log}");
    }

    assert_eq!(
        register_token(&alice, TOKEN_HEX, Some(&phone_id)).await,
        400,
        "a removed device is no longer one to register for"
    );
    assert_eq!(
        register_browser(&alice, ENDPOINT, Some(&phone_id)).await,
        400
    );
}

#[tokio::test]
async fn a_registration_that_names_no_device_stays_when_a_device_is_removed() {
    let (alice, _) = alice().await;
    with_web_push(&alice);
    let (phone, laptop) = with_two_devices(&alice, "alice").await;
    register_token(&alice, TOKEN_HEX, None).await;
    register_browser(&alice, ENDPOINT, None).await;

    let lost = DeviceOp::Remove {
        device: phone.public().device_id(),
    };
    change(&alice, "alice", lost, &laptop).await;
    assert_eq!(tokens(&alice).await.len(), 1);
    assert_eq!(browsers(&alice).await.len(), 1);
    assert!(
        !log_of(&alice).contains("dropped"),
        "nothing was dropped, so nothing is said"
    );
}

#[tokio::test]
async fn the_list_gives_what_a_delete_takes_for_each_kind() {
    let (alice, _) = alice().await;
    with_web_push(&alice);
    let (phone, _) = with_two_devices(&alice, "alice").await;
    let phone_id = device_id(&phone);
    for token in [TOKEN_HEX, OTHER_TOKEN_HEX, THIRD_TOKEN_HEX] {
        assert_eq!(register_token(&alice, token, Some(&phone_id)).await, 200);
    }
    register_browser(&alice, ENDPOINT, Some(&phone_id)).await;
    register_browser(&alice, OTHER_ENDPOINT, None).await;

    let listed_tokens = tokens(&alice).await;
    assert_eq!(listed_tokens.len(), 3);
    for one in &listed_tokens {
        assert_eq!(
            one["token"].as_str().unwrap().len(),
            8,
            "shortened in a list"
        );
        assert_eq!(one["id"].as_str().unwrap().len(), 16);
    }
    let id_of = |tail: &str| {
        let one = listed_tokens
            .iter()
            .find(|one| one["token"] == tail)
            .unwrap();
        one["id"].as_str().unwrap().to_owned()
    };
    assert_eq!(
        delete(
            &alice,
            &format!("/api/push/tokens/{}", id_of(&OTHER_TOKEN_HEX[56..]))
        )
        .await,
        StatusCode::NO_CONTENT,
        "a token named by the id the list gave"
    );
    assert_eq!(
        delete(&alice, &format!("/api/push/tokens/{TOKEN_HEX}")).await,
        StatusCode::NO_CONTENT,
        "a token named whole, as the app that registered it can"
    );
    let left = tokens(&alice).await;
    assert_eq!(left.len(), 1);
    assert_eq!(left[0]["token"], &THIRD_TOKEN_HEX[56..]);
    assert_eq!(
        delete(&alice, "/api/push/tokens/0000000000000000").await,
        StatusCode::NO_CONTENT,
        "an id that names nothing is not an error"
    );
    assert_eq!(tokens(&alice).await.len(), 1);

    let listed_browsers = browsers(&alice).await;
    assert_eq!(listed_browsers.len(), 2);
    for one in &listed_browsers {
        let path = format!("/api/push/subscriptions/{}", one["id"].as_str().unwrap());
        assert_eq!(delete(&alice, &path).await, StatusCode::NO_CONTENT);
    }
    assert!(browsers(&alice).await.is_empty());
}

#[tokio::test]
async fn a_removal_the_roots_node_returns_drops_the_registrations_here_too() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    common::for_bob(&alice, &memory, "r-1");
    with_web_push(&bob);
    let (laptop, phone) = (AccountKey::generate(), AccountKey::generate());
    add_device(&bob.node, "Laptop", &laptop.public(), None).await;
    change(&bob, "bob", add_op(&phone, "Phone"), &laptop).await;
    register_token(&bob, TOKEN_HEX, Some(&device_id(&phone))).await;
    register_browser(&bob, ENDPOINT, Some(&device_id(&phone))).await;

    let lost = DeviceOp::Remove {
        device: phone.public().device_id(),
    };
    change(&bob, "bob", lost, &laptop).await;
    assert!(tokens(&bob).await.is_empty());
    assert!(browsers(&bob).await.is_empty());
}
