//! A guest's push registrations follow the guest's own devices, those of its member in its org: it may name
//! one of them and no device of the owner's, and a definition keeps its registration while the member has the
//! device and takes it once the member no longer does. The owner, likewise, names only the account's devices.
mod common;

use agency_core::orgs;
use agency_proto::org::Device;
use agency_proto::{AccountKey, Timestamp};
use common::guests::{call, with_guest, CAROL};
use common::{id, Running};
use reqwest::Method;
use serde_json::{json, Value};

const OWNER: &str = "alice@example.com";
const GUEST_TOKEN: &str = "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12";
const OWNER_TOKEN: &str = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";

fn token(raw: &str, device: &str) -> Value {
    json!({ "token": raw, "environment": "sandbox", "label": "iPhone", "device": device })
}

async fn register(running: &Running, email: &str, raw: &str, device: &str) -> (u16, Value) {
    let input = token(raw, device);
    call(
        running,
        Method::POST,
        "/api/push/tokens",
        email,
        Some(input),
    )
    .await
}

fn kept_tokens(running: &Running) -> Vec<String> {
    let tokens = running.node.store().push_tokens();
    tokens.into_iter().map(|one| one.token).collect()
}

#[tokio::test]
async fn a_guest_names_its_own_device_and_no_device_of_the_owner() {
    let (alice, laptop) = with_guest().await;
    let owners_device = common::device(&alice).await.public().device_id();
    let guests_device = laptop.public().device_id();

    let (status, answer) = register(&alice, CAROL, GUEST_TOKEN, guests_device.as_str()).await;
    assert_eq!(status, 200, "the guest's own device: {answer}");
    let (status, answer) = register(&alice, CAROL, OWNER_TOKEN, owners_device.as_str()).await;
    assert_eq!(status, 400, "a device of the owner's: {answer}");
    let (status, answer) = register(&alice, OWNER, OWNER_TOKEN, guests_device.as_str()).await;
    assert_eq!(
        status, 400,
        "the owner names no device of the guest's: {answer}"
    );
    let (status, answer) = register(&alice, OWNER, OWNER_TOKEN, owners_device.as_str()).await;
    assert_eq!(status, 200, "the owner's own device: {answer}");
}

#[tokio::test]
async fn a_guests_registration_stays_while_its_member_has_the_device_and_goes_with_it() {
    let (alice, laptop) = with_guest().await;
    let guests_device = laptop.public().device_id();
    let (status, answer) = register(&alice, CAROL, GUEST_TOKEN, guests_device.as_str()).await;
    assert_eq!(status, 200, "{answer}");

    // Any new version is kept through the check of the devices that registrations name.
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.label = "Acme, Inc.".into();
        Ok(())
    })
    .unwrap();
    assert_eq!(
        kept_tokens(&alice),
        [GUEST_TOKEN],
        "carol still has the device"
    );

    // A hosted member always has a device, so the laptop goes by being replaced with another one.
    let phone = AccountKey::from_seed([0x82; 32]);
    let added_at = Timestamp::parse(&alice.node.now_text()).unwrap();
    orgs::amend(&alice.node, &id("acme"), |body| {
        if let Some(carol) = body.people.iter_mut().find(|one| one.id == id("carol")) {
            carol.devices = vec![Device {
                id: phone.public().device_id(),
                label: "Phone".into(),
                public_key: phone.public(),
                added_at,
            }];
        }
        Ok(())
    })
    .unwrap();
    assert!(
        kept_tokens(&alice).is_empty(),
        "the laptop is gone, and so is its registration"
    );
}
