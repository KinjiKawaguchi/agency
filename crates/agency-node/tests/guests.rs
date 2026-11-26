//! A guest: a member this node hosts, signed in through Access with the email its record names. It reaches
//! only the routes of its own table, scoped to its member, and never writes as the owner.
mod common;

use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::guests::{self, Guest};
use agency_core::orgs;
use agency_proto::org::{Device, Person, TeamPlace};
use agency_proto::{AccountKey, Timestamp};
use common::access::{access_token, behind_access_with_runtime, PUBLIC_HOST};
use common::{id, Running, TOKEN};
use reqwest::Method;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const CAROL: &str = "carol@example.com";

/// Alice's org with team `app` on her node, addressing `carol` (a hosted member with one device) as
/// `finance`, with request r-2 for carol and r-1 for the owner; and carol's guest record.
async fn with_guest() -> (Running, AccountKey) {
    let (alice, memory) = behind_access_with_runtime(None).await;
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
    (alice, laptop)
}

async fn call(
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

#[tokio::test]
async fn a_guest_reads_its_own_member_org_and_requests_and_nothing_of_the_owners() {
    let (alice, _) = with_guest().await;
    let (status, me) = call(&alice, Method::GET, "/api/me", CAROL, None).await;
    assert_eq!(status, 200, "{me}");
    assert_eq!(me["account"]["id"], "carol");
    assert_eq!(me["guest"], json!({ "member": "carol", "org": "acme" }));
    assert_eq!(me["funding"], json!([]));
    assert!(
        me.get("homeNodes").is_none() && me.get("rootChanges").is_none(),
        "{me}"
    );
    assert_eq!(me["orgs"][0]["teams"][0]["id"], "app");

    let (status, team) = call(
        &alice,
        Method::GET,
        "/api/orgs/acme/teams/app/view",
        CAROL,
        None,
    )
    .await;
    assert_eq!(status, 200, "{team}");
    let ids: Vec<&str> = team["view"]["requests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["r-2"], "only the request addressed to carol");
    assert_eq!(
        team["view"]["people"],
        json!([]),
        "nothing of the team's people"
    );

    let (status, devices) = call(&alice, Method::GET, "/api/devices", CAROL, None).await;
    assert_eq!(status, 200, "{devices}");
    assert_eq!(devices["orgs"][0]["devices"].as_array().unwrap().len(), 1);
    assert!(
        devices.get("node").is_none() && devices["requests"] == json!([]),
        "{devices}"
    );
}

#[tokio::test]
async fn every_route_outside_the_guest_table_answers_a_guest_403() {
    let (alice, _) = with_guest().await;
    let owner_routes: [(Method, &str); 18] = [
        (Method::GET, "/api/orgs/acme"),
        (Method::GET, "/api/orgs/acme/teams/app/usage"),
        (Method::GET, "/api/orgs/acme/evaluation"),
        (Method::GET, "/api/orgs/acme/teams/app/activity"),
        (Method::GET, "/api/push/tokens"),
        (Method::POST, "/api/push/tokens"),
        (Method::DELETE, "/api/push/tokens/abcd"),
        (Method::GET, "/api/push/vapid"),
        (Method::GET, "/api/push/subscriptions"),
        (Method::POST, "/api/push/subscriptions"),
        (Method::DELETE, "/api/push/subscriptions/x"),
        (Method::POST, "/api/devices/requests"),
        (Method::GET, "/api/outcomes/acme"),
        (Method::POST, "/api/outcomes/acme/observations"),
        (Method::GET, "/api/metrics/acme"),
        (Method::GET, "/api/orgs/other/teams/app/view"),
        (Method::POST, "/api/orgs/other/teams/app/envelopes"),
        (Method::GET, "/app/handoff"),
    ];
    for (method, path) in owner_routes {
        let body = (method != Method::GET).then(|| json!({}));
        let (status, answer) = call(&alice, method.clone(), path, CAROL, body).await;
        assert_eq!(status, 403, "{method} {path}: {answer}");
    }
}

#[tokio::test]
async fn the_owners_email_wins_and_a_record_whose_member_is_not_hosted_admits_no_one() {
    let (alice, _) = with_guest().await;
    guests::keep(
        &alice.node,
        &Guest::new("alice@example.com", id("acme"), id("carol")),
    )
    .unwrap();
    let (_, me) = call(&alice, Method::GET, "/api/me", "alice@example.com", None).await;
    assert!(
        me.get("guest").is_none(),
        "the owner is never a guest: {me}"
    );

    guests::keep(
        &alice.node,
        &Guest::new("dan@example.com", id("acme"), id("alice")),
    )
    .unwrap();
    let (_, me) = call(&alice, Method::GET, "/api/me", "dan@example.com", None).await;
    assert!(me.get("guest").is_none(), "alice is not hosted: {me}");
}

#[tokio::test]
async fn a_guest_decides_only_as_its_member_and_never_as_the_owner() {
    use agency_proto::{Envelope, EnvelopeId, EnvelopeKind, Kind, Payload, Signed};
    let (alice, laptop) = with_guest().await;
    let envelope = |account: &str, target: &str| Envelope {
        id: EnvelopeId::from_bytes([0x82; 16]),
        org: id("acme"),
        team: id("app"),
        kind: EnvelopeKind::Answer,
        target: target.into(),
        payload: Payload {
            text: "ok".into(),
            choice: None,
        },
        account: id(account),
        at: alice.node.now_text(),
    };
    let signed = |body: Envelope| json!({ "envelope": Signed::sign(Kind::Envelope, body, laptop.public().device_id(), &laptop).unwrap() });
    let as_owner = signed(envelope("alice", "r-1"));
    let (status, answer) = call(
        &alice,
        Method::POST,
        "/api/orgs/acme/teams/app/envelopes",
        CAROL,
        Some(as_owner),
    )
    .await;
    assert_eq!(status, 403, "a guest never decides as the owner: {answer}");

    let as_carol = signed(envelope("carol", "r-2"));
    let (status, answer) = call(
        &alice,
        Method::POST,
        "/api/orgs/acme/teams/app/envelopes",
        CAROL,
        Some(as_carol),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    let (_, log) = call(&alice, Method::GET, "/api/decisions", CAROL, None).await;
    assert_eq!(log["entries"].as_array().unwrap().len(), 1, "{log}");
    let (_, owners) = call(
        &alice,
        Method::GET,
        "/api/decisions",
        "alice@example.com",
        None,
    )
    .await;
    assert_eq!(
        owners["entries"],
        json!([]),
        "the owner's log is not the guest's: {owners}"
    );
}
