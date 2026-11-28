//! Removing a member: the root signs a version without the person and with their devices' keys revoked. From then on
//! nothing they sign is taken on a node of the org, a key of theirs can't be any other member's, and what the root's
//! node kept for them as a hosted member is gone. While something still names them, the removal is refused, naming it.
mod common;

use agency_adapters::peers::read_headers;
use agency_core::decisions::accept_envelope;
use agency_core::members::remove_member;
use agency_core::read_auth::sign_read;
use agency_core::{departments, guests, orgs, routes, seats, ErrorKind};
use agency_proto::org::{Department, Device};
use agency_proto::seat::{External, Seat};
use agency_proto::{Id, Timestamp};
use common::guests::{with_guest, CAROL};
use common::{alice, device, id, joined_bob, signed_answer, Running};

fn acme() -> Id {
    id("acme")
}

fn current(alice: &Running) -> agency_proto::org::SignedOrg {
    alice.node.store().load_org(&acme()).unwrap()
}

#[tokio::test]
async fn a_removed_member_signs_nothing_that_is_taken_and_its_keys_are_revoked() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let phone = device(&bob).await;
    let before = current(&alice).body.version;

    let doc = remove_member(&alice.node, &acme(), &id("bob")).unwrap();
    assert_eq!(doc.body.version, before + 1);
    assert!(doc.body.member(&id("bob")).is_none());
    assert!(
        doc.body.revoked.contains(&phone.public()),
        "its device's key is revoked"
    );

    // bob's signed read of alice's team.
    let path = routes::view(&acme(), &id("app"));
    let auth = sign_read(bob.node.key(), &id("bob"), "GET", &path, alice.node.now());
    let mut request = reqwest::Client::new().get(format!("{}{path}", alice.url()));
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    assert_eq!(
        request.send().await.unwrap().status(),
        404,
        "a stranger's read"
    );

    // A decision signed by bob's device.
    let envelope = signed_answer(&bob, &phone, "r-1", "a");
    let refused = accept_envelope(&alice.node, &acme(), &id("app"), envelope)
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::NotFound, "{refused:?}");

    // bob's device key can't become another member's.
    let added = orgs::amend(&alice.node, &acme(), |body| {
        let alice = body
            .people
            .iter_mut()
            .find(|one| one.id == id("alice"))
            .unwrap();
        alice.devices.push(Device {
            id: phone.public().device_id(),
            label: "Phone".into(),
            public_key: phone.public(),
            added_at: Timestamp::parse("2026-11-24T12:00:00Z").unwrap(),
        });
        Ok(())
    });
    assert!(added.is_err(), "{added:?}");
}

#[tokio::test]
async fn the_presence_prompt_names_the_removal() {
    let (alice, _) = alice().await;
    joined_bob(&alice).await;
    alice.root_keys.ask_presence(true);
    remove_member(&alice.node, &acme(), &id("bob")).unwrap();
    let prompts = alice.root_keys.prompts();
    assert!(
        prompts.iter().any(|one| one.contains("remove member bob")),
        "{prompts:?}"
    );
}

#[tokio::test]
async fn the_root_an_unknown_member_and_a_member_still_named_are_refused() {
    let (alice, _) = alice().await;
    joined_bob(&alice).await;
    let error = remove_member(&alice.node, &acme(), &id("alice")).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Forbidden, "{error:?}");
    let error = remove_member(&alice.node, &acme(), &id("dave")).unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotFound, "{error:?}");

    let seat = Seat {
        id: id("finance"),
        label: "Finance".into(),
        department: None,
        unit: None,
        ask_about: vec![],
        relays: vec![id("bob")],
    };
    seats::add_seat(&alice.node, &acme(), seat).unwrap();
    let counsel = External {
        id: id("counsel"),
        label: "Counsel".into(),
        ask_about: vec![],
        relays: vec![id("bob")],
    };
    seats::add_external(&alice.node, &acme(), counsel).unwrap();
    let department = Department {
        id: id("ops"),
        label: "Ops".into(),
        parent: None,
        head: Some(id("bob")),
    };
    departments::add(&alice.node, &acme(), department).unwrap();
    alice
        .node
        .update_config(|config| {
            config.teams[0].people.insert("reviewer".into(), id("bob"));
        })
        .unwrap();
    let before = current(&alice).body.version;

    let error = remove_member(&alice.node, &acme(), &id("bob")).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict, "{error:?}");
    for place in [
        "seat finance",
        "external party counsel",
        "department ops",
        "team app",
    ] {
        assert!(error.message.contains(place), "{place}: {error:?}");
    }
    assert_eq!(current(&alice).body.version, before, "nothing was signed");
}

#[tokio::test]
async fn a_hosted_members_sign_in_and_push_registrations_go_with_it() {
    let (alice, laptop) = with_guest().await;
    let token = serde_json::json!({ "token": "ab12".repeat(16), "environment": "sandbox", "label": "iPhone" });
    let (status, answer) = common::guests::call(
        &alice,
        reqwest::Method::POST,
        "/api/push/tokens",
        CAROL,
        Some(token),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    // The team addresses carol as `finance`: that is changed first.
    alice
        .node
        .update_config(|config| config.teams.iter_mut().for_each(|team| team.people.clear()))
        .unwrap();

    remove_member(&alice.node, &acme(), &id("carol")).unwrap();
    assert!(guests::admit(&alice.node, CAROL).is_none());
    assert!(
        alice.node.store().guest(CAROL).is_none(),
        "the record is dropped"
    );
    assert!(
        alice.node.store().push_tokens().is_empty(),
        "and its app's token"
    );
    assert!(current(&alice).body.revoked.contains(&laptop.public()));
}

#[tokio::test]
async fn a_team_that_addresses_a_seat_the_member_took_names_the_member() {
    let (alice, _) = alice().await;
    joined_bob(&alice).await;
    let seat = Seat {
        id: id("dan"),
        label: "Dan (tax)".into(),
        department: None,
        unit: None,
        ask_about: vec![],
        relays: vec![],
    };
    seats::add_seat(&alice.node, &acme(), seat).unwrap();
    seats::merge_seat(&alice.node, &acme(), &id("dan"), &id("bob")).unwrap();
    alice
        .node
        .update_config(|config| {
            config.teams[0].people.insert("tax".into(), id("dan"));
        })
        .unwrap();
    let before = current(&alice).body.version;

    let error = remove_member(&alice.node, &acme(), &id("bob")).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict, "{error:?}");
    assert!(error.message.contains("team app"), "{error:?}");
    assert_eq!(current(&alice).body.version, before);
}
