//! A guest's second browser asks to be added: the request is kept for the guest's hosted member alone, the
//! guest approves it with its existing device through `POST /api/devices/changes`, and the new device then
//! signs as that member. The owner's requests and every other guest's are neither shown to it nor changed.
mod common;

use agency_core::guests::{self, Guest};
use agency_core::orgs;
use agency_proto::clock::Duration;
use agency_proto::org::{Device, Person};
use agency_proto::{AccountKey, DeviceChange, DeviceOp, Kind, Signed, Timestamp};
use common::access::{access_token, PUBLIC_HOST};
use common::guests::{with_guest, CAROL};
use common::{id, Running, TOKEN};
use reqwest::Method;
use serde_json::{json, Value};

const OWNER: &str = "alice@example.com";
const DAVE: &str = "dave@example.com";

/// `common::guests::call`, with a sign-in that lasts three hours so that a test can move the clock.
async fn call(
    running: &Running,
    method: Method,
    path: &str,
    email: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let token = access_token(email, |claims| {
        claims["exp"] = json!(1_795_521_600u64 + 10_800)
    });
    let mut request = reqwest::Client::new()
        .request(method, format!("{}{path}", running.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", token)
        .header("x-agency-token", TOKEN);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
}

fn key(seed: u8) -> AccountKey {
    AccountKey::from_seed([seed; 32])
}

async fn ask(running: &Running, email: &str, seed: u8, label: &str) -> (u16, Value) {
    let body = json!({ "label": label, "publicKey": key(seed).public() });
    call(
        running,
        Method::POST,
        "/api/devices/requests",
        email,
        Some(body),
    )
    .await
}

/// The labels of the requests `email` is shown, oldest first.
async fn waiting(running: &Running, email: &str) -> Vec<String> {
    let (status, list) = call(running, Method::GET, "/api/devices", email, None).await;
    assert_eq!(status, 200, "{list}");
    list["requests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["label"].as_str().unwrap().to_owned())
        .collect()
}

/// A second hosted member, dave, with his own guest record and one device.
fn add_dave(running: &Running) {
    let laptop = key(0x91);
    let device = Device {
        id: laptop.public().device_id(),
        label: "Laptop".into(),
        public_key: laptop.public(),
        added_at: Timestamp::parse(&running.node.now_text()).unwrap(),
    };
    orgs::amend(&running.node, &id("acme"), |body| {
        body.people
            .push(Person::hosted(id("dave"), "Dave", vec![device]));
        Ok(())
    })
    .unwrap();
    guests::keep(&running.node, &Guest::new(DAVE, id("acme"), id("dave"))).unwrap();
}

#[tokio::test]
async fn a_guests_request_is_listed_for_its_member_with_its_pairing_code_and_no_command() {
    let (alice, _) = with_guest().await;
    let (status, asked) = ask(&alice, CAROL, 0x82, "Phone").await;
    assert_eq!(status, 200, "{asked}");
    assert_eq!(asked["label"], "Phone");
    let code = asked["pairingCode"].as_str().unwrap();
    assert_eq!(code, key(0x82).public().pairing_code());

    let (_, list) = call(&alice, Method::GET, "/api/devices", CAROL, None).await;
    assert_eq!(list["requests"][0]["pairingCode"], code);
    assert_eq!(
        list["requests"][0]["deviceId"],
        key(0x82).public().device_id().as_str()
    );
    assert!(list.get("node").is_none(), "no command for a guest: {list}");
}

#[tokio::test]
async fn the_owners_list_never_shows_a_guests_request_nor_the_guests_the_owners() {
    let (alice, _) = with_guest().await;
    assert_eq!(ask(&alice, CAROL, 0x82, "Carol's phone").await.0, 200);
    assert_eq!(ask(&alice, OWNER, 0x83, "Owner's tablet").await.0, 200);
    assert_eq!(waiting(&alice, OWNER).await, ["Owner's tablet"]);
    assert_eq!(waiting(&alice, CAROL).await, ["Carol's phone"]);
    let terminal = agency_core::devices::pending_requests(&alice.node).unwrap();
    assert_eq!(terminal.len(), 1, "the terminal lists the owner's only");
}

#[tokio::test]
async fn a_guests_requests_are_not_another_guests() {
    let (alice, _) = with_guest().await;
    add_dave(&alice);
    assert_eq!(ask(&alice, CAROL, 0x82, "Carol's phone").await.0, 200);
    assert_eq!(ask(&alice, DAVE, 0x92, "Dave's phone").await.0, 200);
    assert_eq!(waiting(&alice, CAROL).await, ["Carol's phone"]);
    assert_eq!(waiting(&alice, DAVE).await, ["Dave's phone"]);
}

#[tokio::test]
async fn each_member_keeps_at_most_ten_requests_for_an_hour_and_never_costs_another_its_own() {
    let (alice, _) = with_guest().await;
    assert_eq!(ask(&alice, OWNER, 0x83, "Owner's tablet").await.0, 200);
    for n in 0..11u8 {
        assert_eq!(
            ask(&alice, CAROL, 0xA0 + n, &format!("Phone {n}")).await.0,
            200
        );
    }
    let kept = waiting(&alice, CAROL).await;
    assert_eq!(kept.len(), 10, "{kept:?}");
    assert_eq!(
        (kept[0].as_str(), kept[9].as_str()),
        ("Phone 1", "Phone 10"),
        "the oldest went"
    );
    assert_eq!(
        waiting(&alice, OWNER).await,
        ["Owner's tablet"],
        "a guest's flood does not push out the owner's request"
    );

    alice.clock.advance(Duration::minutes(59));
    assert_eq!(
        waiting(&alice, CAROL).await.len(),
        10,
        "still within the hour"
    );
    alice.clock.advance(Duration::minutes(2));
    assert!(waiting(&alice, CAROL).await.is_empty(), "an hour passed");
    assert!(waiting(&alice, OWNER).await.is_empty());
}

#[tokio::test]
async fn the_guest_approves_with_its_device_and_the_new_device_then_decides_as_its_member() {
    use agency_proto::{Envelope, EnvelopeId, EnvelopeKind, Payload};
    let (alice, laptop) = with_guest().await;
    let (_, asked) = ask(&alice, CAROL, 0x82, "Phone").await;
    let phone = key(0x82);
    assert_eq!(asked["deviceId"], phone.public().device_id().as_str());

    let device = Device {
        id: phone.public().device_id(),
        label: "Phone".into(),
        public_key: phone.public(),
        added_at: Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    let change = DeviceChange {
        org: id("acme"),
        account: id("carol"),
        op: DeviceOp::Add {
            device: Box::new(device),
        },
        at: Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    let signed = Signed::sign(Kind::Device, change, laptop.public().device_id(), &laptop).unwrap();
    let (status, answer) = call(
        &alice,
        Method::POST,
        "/api/devices/changes",
        CAROL,
        Some(json!(signed)),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["outcome"], "applied");

    let (_, list) = call(&alice, Method::GET, "/api/devices", CAROL, None).await;
    assert_eq!(
        list["orgs"][0]["devices"].as_array().unwrap().len(),
        2,
        "{list}"
    );
    assert!(
        list["requests"].as_array().unwrap().is_empty(),
        "no longer waiting: {list}"
    );

    let envelope = Envelope {
        id: EnvelopeId::from_bytes([0x83; 16]),
        org: id("acme"),
        team: id("app"),
        kind: EnvelopeKind::Answer,
        target: "r-2".into(),
        payload: Payload {
            text: "ok".into(),
            choice: None,
        },
        account: id("carol"),
        at: alice.node.now_text(),
    };
    let signed =
        Signed::sign(Kind::Envelope, envelope, phone.public().device_id(), &phone).unwrap();
    let body = json!({ "envelope": signed });
    let (status, answer) = call(
        &alice,
        Method::POST,
        "/api/orgs/acme/teams/app/envelopes",
        CAROL,
        Some(body),
    )
    .await;
    assert_eq!(status, 200, "the new device decides as carol: {answer}");
}

#[tokio::test]
async fn a_request_for_a_key_the_node_cannot_use_or_without_a_label_is_refused() {
    let (alice, _) = with_guest().await;
    let account_key = json!({ "label": "x", "publicKey": alice.node.key().public() });
    let (status, answer) = call(
        &alice,
        Method::POST,
        "/api/devices/requests",
        CAROL,
        Some(account_key),
    )
    .await;
    assert_eq!(
        status, 400,
        "the account key is never a device key: {answer}"
    );
    assert_eq!(
        ask(&alice, CAROL, 0x82, "   ").await.0,
        400,
        "a blank label"
    );
    assert_eq!(
        ask(&alice, CAROL, 0x82, &"x".repeat(81)).await.0,
        400,
        "a long label"
    );
    assert!(waiting(&alice, CAROL).await.is_empty());
}

#[tokio::test]
async fn a_guest_reaches_no_other_route_of_the_device_requests_and_a_stranger_none_at_all() {
    let (alice, _) = with_guest().await;
    for method in [Method::GET, Method::PUT, Method::DELETE] {
        let (status, answer) = call(
            &alice,
            method.clone(),
            "/api/devices/requests",
            CAROL,
            Some(json!({})),
        )
        .await;
        assert_eq!(status, 403, "{method}: {answer}");
    }
    let (status, answer) = ask(&alice, "stranger@example.com", 0x82, "Phone").await;
    assert_ne!(
        status, 200,
        "an Access sign-in that is no guest and no owner: {answer}"
    );
    assert!(
        waiting(&alice, OWNER).await.is_empty(),
        "nothing was kept for the owner"
    );
}
