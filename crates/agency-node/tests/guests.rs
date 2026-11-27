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
    let (alice, memory) = behind_access_with_runtime(Some("ABCDE12345.dev.example.agency")).await;
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

/// A path of the route table with its parameters filled in for alice's org.
fn filled(pattern: &str) -> String {
    pattern
        .replace("{org}", "acme")
        .replace("{team}", "app")
        .replace("{token}", &"ab12".repeat(16))
        .replace("{id}", "x")
}

#[tokio::test]
async fn every_route_outside_the_guest_table_answers_a_guest_403_and_the_owner_something_real() {
    let (alice, _) = with_guest().await;
    let guest_routes = agency_adapters::client_api::guest_routes();
    let owners: Vec<(&str, &str)> = agency_adapters::client_api::routes_behind_the_guard()
        .into_iter()
        .filter(|route| !guest_routes.contains(route))
        .collect();
    assert!(owners.len() >= 15, "{owners:?}");
    for (method, pattern) in owners {
        let method = Method::from_bytes(method.as_bytes()).unwrap();
        let path = filled(pattern);
        let body = (method != Method::GET).then(|| json!({}));
        let (status, answer) = call(&alice, method.clone(), &path, CAROL, body.clone()).await;
        assert_eq!(status, 403, "a guest: {method} {path}: {answer}");
        let (mut status, mut answer): (u16, Value) =
            call(&alice, method.clone(), &path, "alice@example.com", body).await;
        // A route of the node's own machine only (`/metrics`) is reached by a local read instead.
        if status == 403 && method == Method::GET {
            let (local, read) =
                common::body(common::get(&alice, &path).send().await.unwrap()).await;
            (status, answer) = (local.as_u16(), read);
        }
        // A route that is not there answers "no such route" (or 405 for a method it lacks); a real one may
        // answer 404 for a reason of its own (no Web Push set up), but never that.
        let is_missing = status == 405 || (status == 404 && answer["error"] == "no such route");
        assert!(
            status != 403 && !is_missing,
            "the owner reaches {method} {path}, so the route is real: {status} {answer}"
        );
    }
    let elsewhere: [(Method, &str); 4] = [
        (Method::GET, "/api/orgs/other/teams/app/view"),
        (Method::POST, "/api/orgs/other/teams/app/envelopes"),
        (Method::GET, "/api/me/"),
        (Method::HEAD, "/api/orgs/acme"),
    ];
    for (method, path) in elsewhere {
        let body = (method == Method::POST).then(|| json!({}));
        let (status, answer) = call(&alice, method.clone(), path, CAROL, body).await;
        assert_eq!(status, 403, "a guest: {method} {path}: {answer}");
    }
}

#[tokio::test]
async fn a_guest_changes_no_device_of_another_org_or_member() {
    use agency_proto::device::{DeviceChange, DeviceOp};
    use agency_proto::{Kind, Signed};
    let (alice, laptop) = with_guest().await;
    for (org, account) in [("other", "carol"), ("acme", "alice")] {
        let change = DeviceChange {
            org: id(org),
            account: id(account),
            op: DeviceOp::Remove {
                device: laptop.public().device_id(),
            },
            at: Timestamp::parse(&alice.node.now_text()).unwrap(),
        };
        let signed =
            Signed::sign(Kind::Device, change, laptop.public().device_id(), &laptop).unwrap();
        let (status, answer) = call(
            &alice,
            Method::POST,
            "/api/devices/changes",
            CAROL,
            Some(json!(signed)),
        )
        .await;
        assert_eq!(status, 403, "{org}/{account}: {answer}");
    }
}

#[tokio::test]
async fn a_guest_loads_the_web_client_but_not_the_app_sign_in() {
    let (alice, _) = with_guest().await;
    let page = reqwest::Client::new()
        .get(format!("{}/desk", alice.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", access_token(CAROL, |_| {}))
        .send()
        .await
        .unwrap();
    assert_ne!(
        page.status().as_u16(),
        403,
        "the page lets the guest shell run"
    );
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

#[tokio::test]
async fn me_has_no_guest_key_at_all_for_the_owner_a_local_request_or_another_sign_in() {
    let (alice, _) = with_guest().await;
    let (status, me) = call(&alice, Method::GET, "/api/me", "alice@example.com", None).await;
    assert_eq!(status, 200, "{me}");
    assert!(me.as_object().unwrap().get("guest").is_none(), "{me}");
    let (status, other) = call(&alice, Method::GET, "/api/me", "erin@example.com", None).await;
    assert_eq!(status, 403, "another sign-in reads no me at all: {other}");
    let (_, local) = common::body(common::get(&alice, "/api/me").send().await.unwrap()).await;
    assert!(local.as_object().unwrap().get("guest").is_none(), "{local}");
}

#[tokio::test]
async fn a_guest_decides_in_no_team_on_another_node_so_nothing_is_logged_for_a_seat_it_does_not_answer(
) {
    use agency_proto::{Envelope, EnvelopeId, EnvelopeKind, Kind, Payload, Signed};
    let (alice, laptop) = with_guest().await;
    // bob's node is read already and speaks everything this release does.
    let read_at = agency_proto::clock::format(alice.node.now());
    let entry = agency_core::peer_about::PeerAbout {
        about: Some(agency_proto::about::About::of_release("0.3.0")),
        read_at: Some(read_at.clone()),
        tried_at: read_at,
        ..agency_core::peer_about::PeerAbout::default()
    };
    alice
        .node
        .store()
        .save_peer_abouts(
            &id("acme"),
            &BTreeMap::from([("https://bob.example".to_owned(), entry)]),
        )
        .unwrap();
    orgs::add_team(
        &alice.node,
        &id("acme"),
        TeamPlace::new(id("web"), "Web", "https://bob.example"),
    )
    .unwrap();
    let envelope = Envelope {
        id: EnvelopeId::from_bytes([0x83; 16]),
        org: id("acme"),
        team: id("web"),
        kind: EnvelopeKind::Answer,
        target: "r-9".into(),
        payload: Payload {
            text: "ok".into(),
            choice: None,
        },
        account: id("carol"),
        at: alice.node.now_text(),
    };
    let body = json!({ "envelope": Signed::sign(Kind::Envelope, envelope, laptop.public().device_id(), &laptop).unwrap() });
    let (status, answer) = call(
        &alice,
        Method::POST,
        "/api/orgs/acme/teams/web/envelopes",
        CAROL,
        Some(body),
    )
    .await;
    assert_eq!(status, 404, "{answer}");
    let (_, log) = call(&alice, Method::GET, "/api/decisions", CAROL, None).await;
    assert_eq!(log["entries"], json!([]), "{log}");
}

#[tokio::test]
async fn a_record_on_a_node_that_is_not_the_roots_admits_no_one() {
    let (alice, _) = with_guest().await;
    let (_, me) = call(&alice, Method::GET, "/api/me", CAROL, None).await;
    assert!(
        me.get("guest").is_some(),
        "carol is a guest while alice is root: {me}"
    );
    // The org as a node that is not its root holds it: bob's root, kept as read.
    let mut doc = alice.node.store().load_org(&id("acme")).unwrap();
    doc.body.root.account = id("bob");
    doc.body.root.public_key = AccountKey::from_seed([0x84; 32]).public();
    alice.node.store().save_org(&doc).unwrap();
    let (_, me) = call(&alice, Method::GET, "/api/me", CAROL, None).await;
    assert!(
        me.get("guest").is_none(),
        "only the root's node hosts members: {me}"
    );
}

/// The reads an unknown sign-in must not make: each one answered only to the owner or a guest.
const OWNERS_READS: [&str; 5] = [
    "/api/orgs/acme/teams/app/view",
    "/api/orgs/acme",
    "/api/me",
    "/api/session",
    "/api/decisions",
];

#[tokio::test]
async fn a_guest_whose_member_left_the_org_is_refused_everywhere_and_never_read_as_the_owner() {
    let (alice, _) = with_guest().await;
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.people.retain(|person| person.id != id("carol"));
        Ok(())
    })
    .unwrap();
    for path in OWNERS_READS {
        let (status, answer) = call(&alice, Method::GET, path, CAROL, None).await;
        assert_eq!(status, 403, "{path}: {answer}");
    }
    let (status, _) = call(&alice, Method::GET, "/api/me", "alice@example.com", None).await;
    assert_eq!(status, 200, "the owner still reads");
}

#[tokio::test]
async fn a_sign_in_without_an_email_or_with_another_one_is_refused_everywhere() {
    let (alice, _) = with_guest().await;
    let tokens = [
        access_token("", |claims| {
            claims.as_object_mut().unwrap().remove("email");
            claims["common_name"] = json!("a-service-token");
        }),
        access_token("dan@example.com", |_| {}),
    ];
    for token in tokens {
        for path in OWNERS_READS.iter().chain(&["/desk"]) {
            let response = reqwest::Client::new()
                .get(format!("{}{path}", alice.url()))
                .header("host", PUBLIC_HOST)
                .header("cf-ray", "1")
                .header("cf-access-jwt-assertion", &token)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 403, "{path}");
        }
    }
}
