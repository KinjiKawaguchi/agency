//! The names a team runtime gives meaning of its own (`RESERVED_NAMES`: `owner` is the team's owner) are never
//! newly taken as the id of a member, a seat, or an external party: the node passes a member's id to the runtime
//! as `by`, and the runtime would read a decision by such a member as one by that name.
mod common;

use agency_core::invites::{invite, join, Joined};
use agency_core::seats;
use agency_proto::runtime::RESERVED_NAMES;
use agency_proto::seat::{External, Seat};
use agency_proto::{Id, Role};
use common::{alice, id, memory_runtime, node, Running};

fn acme() -> Id {
    id("acme")
}

fn seat(raw: &str) -> Seat {
    Seat {
        id: id(raw),
        label: "Finance".into(),
        department: None,
        unit: None,
        ask_about: vec![],
        relays: vec![],
    }
}

fn external(raw: &str) -> External {
    External {
        id: id(raw),
        label: "Outside counsel".into(),
        ask_about: vec![],
        relays: vec![id("alice")],
    }
}

fn version(running: &Running) -> u64 {
    running.node.store().load_org(&acme()).unwrap().body.version
}

#[tokio::test]
async fn an_account_named_as_a_runtime_reserves_is_refused_at_the_join_and_the_invite_stays_open() {
    let (alice, _) = alice().await;
    let owner = node("owner", memory_runtime().0).await;
    let link = invite(&alice.node, &acme(), None, Role::Member).unwrap();
    let before = version(&alice);

    let refused = join(&owner.node, &link, |_| true).await;
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(version(&alice), before, "no version names it");
    // An invite is single-use, so another account joining with it shows it was not used up.
    let dave = node("dave", memory_runtime().0).await;
    let joined = join(&dave.node, &link, |_| true).await.unwrap();
    assert!(matches!(joined, Joined::Member(_)), "{joined:?}");
}

#[tokio::test]
async fn a_join_named_as_a_runtime_reserves_is_not_held_for_a_root_that_asks_for_presence() {
    let (alice, _) = alice().await;
    alice.root_keys.ask_presence(true);
    let owner = node("owner", memory_runtime().0).await;
    let link = invite(&alice.node, &acme(), None, Role::Member).unwrap();

    let refused = join(&owner.node, &link, |_| true).await;
    assert!(refused.is_err(), "{refused:?}");
    assert!(
        alice.node.store().pending_roots().is_empty(),
        "nothing waits for the root"
    );
}

#[tokio::test]
async fn no_seat_or_external_party_newly_takes_a_reserved_name() {
    let (alice, _) = alice().await;
    let before = version(&alice);
    for name in RESERVED_NAMES {
        let error = seats::add_seat(&alice.node, &acme(), seat(name)).unwrap_err();
        assert!(error.message.contains("reserves"), "{name}: {error:?}");
        let error = seats::add_external(&alice.node, &acme(), external(name)).unwrap_err();
        assert!(error.message.contains("reserves"), "{name}: {error:?}");
    }
    assert_eq!(version(&alice), before);
    seats::add_seat(&alice.node, &acme(), seat("finance")).unwrap();
}

#[tokio::test]
async fn an_org_that_already_has_a_reserved_name_still_signs_its_other_changes() {
    let (alice, _) = alice().await;
    // Stands in for an org that took the name before it was reserved: the version the node keeps has it.
    let mut kept = alice.node.store().load_org(&acme()).unwrap();
    kept.body.seats.push(seat("owner"));
    alice.node.store().save_org(&kept).unwrap();

    seats::add_seat(&alice.node, &acme(), seat("finance")).unwrap();
    let now = alice.node.store().load_org(&acme()).unwrap();
    let ids: Vec<&str> = now.body.seats.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, ["owner", "finance"]);
}
