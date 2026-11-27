//! A seat becomes a member the root's node hosts, in one signed version: the same id, one device, no account key,
//! and the sign-in address kept once the root has signed. Alice's node is the root of acme, with Access in front
//! of it so that the member's sign-in can be tried.
mod common;

use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::guests;
use agency_core::peer_about::PeerAbout;
use agency_core::seats::SeatChange;
use agency_core::{departments, orgs, seats, ErrorKind};
use agency_proto::about::About;
use agency_proto::org::{Department, Device, TeamPlace};
use agency_proto::seat::Seat;
use agency_proto::{AccountKey, Id, Timestamp};
use common::guests::{call, Runtime, CAROL};
use common::{id, joined_bob, memory_runtime, node, Running};
use reqwest::Method;
use std::collections::BTreeMap;

fn acme() -> Id {
    id("acme")
}

fn seat(raw: &str, label: &str) -> Seat {
    Seat {
        id: id(raw),
        label: label.into(),
        department: None,
        ask_about: vec!["invoices".into()],
        relays: vec![id("alice")],
    }
}

fn device_of(running: &Running, key: &AccountKey) -> Device {
    Device {
        id: key.public().device_id(),
        label: "Laptop".into(),
        public_key: key.public(),
        added_at: Timestamp::parse(&running.node.now_text()).unwrap(),
    }
}

fn version(running: &Running) -> u64 {
    running.node.store().load_org(&acme()).unwrap().body.version
}

/// Alice's org with team `app` on her node, addressing the seat `carol` as `finance`, with request r-2 for it
/// and r-1 for the owner; and a second seat, `dan`.
async fn with_seats() -> (Running, Runtime) {
    let (alice, memory) =
        common::access::behind_access_with_runtime(Some("ABCDE12345.dev.example.agency")).await;
    {
        let mut state = memory.lock().unwrap();
        state
            .requests
            .push(("r-2".into(), "Approve the invoice?".into()));
        state.to.insert("r-2".into(), "finance".into());
    }
    orgs::add_team(
        &alice.node,
        &acme(),
        TeamPlace::new(id("app"), "app", alice.url()),
    )
    .unwrap();
    let ops = Department {
        id: id("ops"),
        label: "Operations".into(),
        parent: None,
        head: None,
    };
    departments::add(&alice.node, &acme(), ops).unwrap();
    seats::add_seat(&alice.node, &acme(), seat("carol", "Carol (finance)")).unwrap();
    seats::add_seat(&alice.node, &acme(), seat("dan", "Dan (legal)")).unwrap();
    let in_ops = SeatChange {
        department: Some(Some(id("ops"))),
        ..SeatChange::default()
    };
    seats::change_seat(&alice.node, &acme(), &id("carol"), in_ops).unwrap();
    alice
        .node
        .update_config(|config| {
            let runtime = RuntimeCommand::of(vec!["/nonexistent".into()]);
            let people = BTreeMap::from([("finance".to_owned(), id("carol"))]);
            config
                .teams
                .push(HostedTeam::new(acme(), id("app"), id("alice"), runtime).with_people(people))
        })
        .unwrap();
    (alice, memory)
}

fn laptop() -> AccountKey {
    AccountKey::from_seed([0x81; 32])
}

#[tokio::test]
async fn a_seat_becomes_a_hosted_member_with_its_id_one_device_and_no_account_key() {
    let (alice, _) = with_seats().await;
    let before = version(&alice);
    let device = device_of(&alice, &laptop());

    seats::host_seat(&alice.node, &acme(), &id("carol"), device.clone(), CAROL).unwrap();

    let doc = alice.node.store().load_org(&acme()).unwrap();
    assert_eq!(doc.body.version, before + 1, "one version");
    assert!(
        doc.body.seats.iter().all(|one| one.id != id("carol")),
        "the seat is gone"
    );
    assert!(doc.body.seats.iter().any(|one| one.id == id("dan")));
    let carol = doc
        .body
        .member(&id("carol"))
        .expect("the member has the seat's id");
    assert_eq!(carol.label, "Carol (finance)");
    assert_eq!(carol.account, id("carol"));
    assert!(carol.public_key.is_none() && carol.is_hosted());
    assert_eq!(carol.devices, vec![device]);
    assert_eq!(carol.ask_about, vec!["invoices".to_owned()]);
    assert_eq!(carol.department, Some(id("ops")), "the seat's department");
    assert!(
        carol.formerly.is_empty(),
        "its id is its own, not a former one"
    );
    let guest = guests::admit(&alice.node, "Carol@Example.com").expect("the sign-in is kept");
    assert_eq!((guest.org, guest.member), (acme(), id("carol")));
}

#[tokio::test]
async fn a_request_addressed_to_the_seat_is_the_hosted_members_own() {
    let (alice, _) = with_seats().await;
    let (status, _) = call(
        &alice,
        Method::GET,
        "/api/orgs/acme/teams/app/view",
        CAROL,
        None,
    )
    .await;
    assert_eq!(status, 403, "before: the address is nobody's");

    seats::host_seat(
        &alice.node,
        &acme(),
        &id("carol"),
        device_of(&alice, &laptop()),
        CAROL,
    )
    .unwrap();

    let (status, snapshot) = call(
        &alice,
        Method::GET,
        "/api/orgs/acme/teams/app/view",
        CAROL,
        None,
    )
    .await;
    assert_eq!(status, 200, "{snapshot}");
    assert_eq!(snapshot["mine"], serde_json::json!(["r-2"]));

    // Those who answered for the seat no longer do: the request is the member's own, and no one else's.
    let owners = common::body(
        common::get(&alice, "/api/orgs/acme/teams/app/view")
            .send()
            .await
            .unwrap(),
    )
    .await
    .1;
    let mine = owners["mine"].as_array().expect("the owner's list");
    assert!(!mine.contains(&serde_json::json!("r-2")), "{owners}");
}

#[tokio::test]
async fn a_seat_that_is_not_there_is_refused_and_nothing_is_kept() {
    let (alice, _) = with_seats().await;
    let before = version(&alice);
    let error = seats::host_seat(
        &alice.node,
        &acme(),
        &id("erin"),
        device_of(&alice, &laptop()),
        CAROL,
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotFound, "{error:?}");
    assert_eq!(version(&alice), before);
    assert!(guests::admit(&alice.node, CAROL).is_none());
    assert!(
        alice.node.store().guest(CAROL).is_none(),
        "no record either"
    );
}

#[tokio::test]
async fn a_node_that_does_not_hold_the_root_key_refuses() {
    let (alice, _) = common::alice().await;
    seats::add_seat(&alice.node, &acme(), seat("carol", "Carol")).unwrap();
    let bob = joined_bob(&alice).await;
    let error = seats::host_seat(
        &bob.node,
        &acme(),
        &id("carol"),
        device_of(&bob, &laptop()),
        CAROL,
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Forbidden, "{error:?}");
    assert!(bob.node.store().guest(CAROL).is_none());
}

#[tokio::test]
async fn a_device_that_is_not_the_keys_own_or_whose_key_the_org_has_is_refused_and_nothing_is_kept()
{
    let (alice, _) = with_seats().await;
    let before = version(&alice);
    let mut wrong_id = device_of(&alice, &laptop());
    wrong_id.id = id("d-000000000000");
    let error = seats::host_seat(&alice.node, &acme(), &id("carol"), wrong_id, CAROL).unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::BadRequest,
        "an id that is not the key's: {error:?}"
    );

    let account_key = alice.node.key().public();
    let used = Device {
        id: account_key.device_id(),
        label: "Laptop".into(),
        public_key: account_key,
        added_at: Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    let error = seats::host_seat(&alice.node, &acme(), &id("carol"), used, CAROL).unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::BadRequest,
        "a key the org has: {error:?}"
    );
    assert_eq!(version(&alice), before);
    assert!(
        alice.node.store().guest(CAROL).is_none(),
        "the sign-in is kept only when the root signed"
    );
}

#[tokio::test]
async fn an_address_that_cannot_sign_in_is_refused() {
    let (alice, _) = with_seats().await;
    let before = version(&alice);
    for bad in [
        "",
        "carol",
        "@example.com",
        "carol@",
        "a@b@c",
        "ca\nrol@example.com",
    ] {
        let error = seats::host_seat(
            &alice.node,
            &acme(),
            &id("carol"),
            device_of(&alice, &laptop()),
            bad,
        )
        .unwrap_err();
        assert_eq!(error.kind, ErrorKind::BadRequest, "{bad:?}");
    }
    let long = format!("{}@example.com", "a".repeat(250));
    let error = seats::host_seat(
        &alice.node,
        &acme(),
        &id("carol"),
        device_of(&alice, &laptop()),
        &long,
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::BadRequest, "too long");
    assert_eq!(version(&alice), before, "nothing was signed");
}

#[tokio::test]
async fn the_owners_own_address_and_another_members_address_are_refused() {
    let (alice, _) = with_seats().await;
    let owner = alice
        .node
        .config()
        .access
        .expect("behind Access")
        .owner_email;
    let error = seats::host_seat(
        &alice.node,
        &acme(),
        &id("carol"),
        device_of(&alice, &laptop()),
        &owner,
    )
    .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::Conflict,
        "the owner is never a guest: {error:?}"
    );

    seats::host_seat(
        &alice.node,
        &acme(),
        &id("carol"),
        device_of(&alice, &laptop()),
        CAROL,
    )
    .unwrap();
    let phone = AccountKey::from_seed([0x82; 32]);
    let before = version(&alice);
    let error = seats::host_seat(
        &alice.node,
        &acme(),
        &id("dan"),
        device_of(&alice, &phone),
        CAROL,
    )
    .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::Conflict,
        "carol's address: {error:?}"
    );
    assert_eq!(version(&alice), before);
    let doc = alice.node.store().load_org(&acme()).unwrap();
    assert!(
        doc.body.seats.iter().any(|one| one.id == id("dan")),
        "dan is still a seat"
    );
    let guest = guests::admit(&alice.node, CAROL).unwrap();
    assert_eq!(guest.member, id("carol"), "carol's record is as it was");
}

#[tokio::test]
async fn an_org_with_a_node_that_does_not_list_hostedmembers_waits() {
    let (alice, _) = common::alice().await;
    let bob = node("bob", memory_runtime().0).await;
    let bob = common::joined(&alice, bob).await;
    orgs::add_team(
        &alice.node,
        &acme(),
        TeamPlace::new(id("web"), "Web", bob.url()),
    )
    .unwrap();
    let mut older = About::of_release("0.3.0");
    older.capabilities.retain(|one| one != "hostedMembers");
    let read_at = agency_proto::clock::format(alice.node.now());
    let entry = PeerAbout {
        about: Some(older),
        read_at: Some(read_at.clone()),
        tried_at: read_at,
        ..PeerAbout::default()
    };
    alice
        .node
        .store()
        .save_peer_abouts(&acme(), &BTreeMap::from([(bob.url(), entry)]))
        .unwrap();
    seats::add_seat(&alice.node, &acme(), seat("carol", "Carol")).unwrap();
    let before = version(&alice);

    let error = seats::host_seat(
        &alice.node,
        &acme(),
        &id("carol"),
        device_of(&alice, &laptop()),
        CAROL,
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict, "{error:?}");
    assert!(error.message.contains("hostedMembers"), "{error:?}");
    assert_eq!(version(&alice), before);
    assert!(alice.node.store().guest(CAROL).is_none());
}
