//! A guest of alice's node for the integration tests: carol, a member alice's node hosts, signed in through
//! Access with the email of her record.
use super::access::{access_token, behind_access_with_runtime, PUBLIC_HOST};
use super::{id, Running, TOKEN};
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::guests::{self, Guest};
use agency_core::orgs;
use agency_proto::org::{Device, Person, TeamPlace};
use agency_proto::{AccountKey, Timestamp};
use reqwest::Method;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub type Runtime = Arc<Mutex<agency_adapters::memory::MemoryRuntimeState>>;

pub const CAROL: &str = "carol@example.com";

/// Alice's org with team `app` on her node, addressing `carol` (a hosted member with one device) as
/// `finance`, with request r-2 for carol and r-1 for the owner; and carol's guest record.
pub async fn with_guest() -> (Running, AccountKey) {
    let (alice, laptop, _) = with_guest_and_runtime().await;
    (alice, laptop)
}

/// `with_guest`, with the state of the team's runtime, to add requests to.
pub async fn with_guest_and_runtime() -> (Running, AccountKey, Runtime) {
    let (alice, memory) = behind_access_with_runtime(Some("ABCDE12345.dev.example.agency")).await;
    let laptop = add_carol(&alice, &memory);
    (alice, laptop, memory)
}

/// Gives alice's node the org, team and request of `with_guest`, and carol's guest record, whatever signs
/// people in. Returns carol's device key.
pub fn add_carol(alice: &Running, memory: &Runtime) -> AccountKey {
    {
        let mut state = memory.lock().unwrap();
        state
            .requests
            .push(("r-2".into(), "Approve the invoice?".into()));
        state.to.insert("r-2".into(), "finance".into());
    }
    let node = &alice.node;
    orgs::add_team(
        node,
        &id("acme"),
        TeamPlace::new(id("app"), "app", alice.url()),
    )
    .unwrap();
    let laptop = AccountKey::from_seed([0x81; 32]);
    let device = Device {
        id: laptop.public().device_id(),
        label: "Laptop".into(),
        public_key: laptop.public(),
        added_at: Timestamp::parse(&node.now_text()).unwrap(),
    };
    orgs::amend(node, &id("acme"), |body| {
        body.people
            .push(Person::hosted(id("carol"), "Carol", vec![device]));
        Ok(())
    })
    .unwrap();
    node.update_config(|config| {
        let runtime = RuntimeCommand::of(vec!["/nonexistent".into()]);
        let people = BTreeMap::from([("finance".to_owned(), id("carol"))]);
        config
            .teams
            .push(HostedTeam::new(id("acme"), id("app"), id("alice"), runtime).with_people(people))
    })
    .unwrap();
    guests::keep(node, &Guest::new(CAROL, id("acme"), id("carol"))).unwrap();
    laptop
}

pub async fn call(
    running: &Running,
    method: Method,
    path: &str,
    email: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let mut request = reqwest::Client::new()
        .request(method, format!("{}{path}", running.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", access_token(email, |_| {}))
        .header("x-agency-token", TOKEN);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
}
