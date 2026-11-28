//! Someone takes a seat: with a claim link (`invite_for_seat`, then `join`), or a member who is in the org
//! already (`merge_seat`). The seat is gone, its id is the member's `formerly`, and a request addressed to it
//! is theirs. Names here are made up.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_core::claims::{seat_invite, CLAIM_DAYS};
use agency_core::client::team_snapshot;
use agency_core::decisions::Decided;
use agency_core::directory::directory;
use agency_core::invites::{invite, invite_for_seat, join, open_invite, Joined};
use agency_core::ports::Clock;
use agency_core::{departments, seats, ErrorKind};
use agency_proto::clock;
use agency_proto::invite::Invite;
use agency_proto::org::{Department, Role, SignedOrg};
use agency_proto::seat::Seat;
use common::{alice, answer, decide_as, device, id, joined_bob, memory_runtime, node, Running};
use std::fs;
use std::sync::{Arc, Mutex};

type Memory = Arc<Mutex<MemoryRuntimeState>>;

/// Alice's org with the department `ops` and the seat carol in it, which alice answers for, asked about
/// invoices. Alice's team `app` names the seat `finance` and holds one request for it, r-9.
async fn org_with_a_seat() -> (Running, Memory) {
    let (alice, memory) = alice().await;
    let acme = id("acme");
    let ops = Department {
        id: id("ops"),
        label: "Operations".into(),
        parent: None,
        head: None,
    };
    departments::add(&alice.node, &acme, ops).unwrap();
    let carol = Seat {
        id: id("carol"),
        label: "Carol (finance)".into(),
        department: Some(id("ops")),
        ask_about: vec!["invoices".into()],
        relays: vec![id("alice")],
    };
    seats::add_seat(&alice.node, &acme, carol).unwrap();
    alice
        .node
        .update_config(|config| {
            config.teams[0].people.insert("finance".into(), id("carol"));
        })
        .unwrap();
    let mut state = memory.lock().unwrap();
    state
        .requests
        .push(("r-9".into(), "Pay the invoice?".into()));
    state.to.insert("r-9".into(), "finance".into());
    drop(state);
    (alice, memory)
}

fn current(alice: &Running) -> SignedOrg {
    alice.node.store().load_org(&id("acme")).unwrap()
}

fn code_of(link: &str) -> String {
    Invite::parse(link).unwrap().code
}

#[tokio::test]
async fn a_seat_invite_is_bound_to_its_seat_and_lowercased_email_but_keeps_only_the_code_hash() {
    let (alice, _) = org_with_a_seat().await;
    let invite = seat_invite(&alice.node, &id("acme"), id("carol"), "Carol@Example.COM").unwrap();
    assert!(invite
        .link
        .starts_with(&format!("{}/claim#code=", alice.node.url())));
    assert!(invite.local_only);
    assert_eq!(invite.email, "carol@example.com");
    let code = invite.link.split("code=").nth(1).unwrap();
    let saved: String = fs::read_dir(alice.data_dir().join("claims"))
        .unwrap()
        .flat_map(|entry| fs::read_to_string(entry.unwrap().path()))
        .collect();
    assert!(!saved.contains(code), "the secret code is never on disk");
    assert!(saved.contains("carol@example.com"));
    assert!(saved.contains(&clock::format(
        alice.clock.now() + agency_proto::clock::Duration::days(CLAIM_DAYS)
    )));
}

#[tokio::test]
async fn a_new_seat_invite_replaces_the_old_link_and_refuses_bad_email_or_missing_seat() {
    let (alice, _) = org_with_a_seat().await;
    let first = seat_invite(&alice.node, &id("acme"), id("carol"), "carol@example.com").unwrap();
    let second = seat_invite(&alice.node, &id("acme"), id("carol"), "carol@example.com").unwrap();
    let claims: Vec<_> = fs::read_dir(alice.data_dir().join("claims"))
        .unwrap()
        .collect();
    assert_eq!(claims.len(), 1, "one open link remains for this seat");
    assert_ne!(first.link, second.link);
    for email in ["", "a", "a@b@c", "@b", "a@", "a\u{0000}@b"] {
        assert_eq!(
            seat_invite(&alice.node, &id("acme"), id("carol"), email)
                .unwrap_err()
                .kind,
            ErrorKind::BadRequest,
            "{email:?}"
        );
    }
    assert_eq!(
        seat_invite(&alice.node, &id("acme"), id("none"), "a@b")
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    );
}

async fn joined_as_carol(alice: &Running) -> Running {
    let link = invite_for_seat(&alice.node, &id("acme"), id("carol"), Role::Member).unwrap();
    let cara = node("cara", memory_runtime().0).await;
    let joined = join(&cara.node, &link, |_| true).await.unwrap();
    assert!(matches!(joined, Joined::Member(_)), "{joined:?}");
    cara
}

#[tokio::test]
async fn whoever_joins_with_a_claim_link_takes_the_seat_its_department_and_what_to_ask_about_it() {
    let (alice, _) = org_with_a_seat().await;
    let before = current(&alice).body.version;
    let _cara = joined_as_carol(&alice).await;

    let body = current(&alice).body;
    assert_eq!(body.version, before + 1, "one version, signed by the root");
    assert!(body.seats.is_empty(), "the seat is gone");
    let cara = body.member(&id("cara")).unwrap();
    assert_eq!(cara.formerly, [id("carol")]);
    assert_eq!(cara.department, Some(id("ops")));
    assert_eq!(cara.ask_about, ["invoices"]);
    assert_eq!(cara.role, Role::Member);
    assert!(body.validate().is_ok());
}

#[tokio::test]
async fn a_request_for_the_seat_is_the_new_members_own_and_no_longer_answered_for_by_anyone() {
    let (alice, memory) = org_with_a_seat().await;
    let before = team_snapshot(&alice.node, &id("acme"), &id("app"))
        .await
        .unwrap();
    assert!(
        before.mine.contains(&"r-9".to_owned()),
        "alice answers for the seat"
    );

    let cara = joined_as_carol(&alice).await;
    let after = team_snapshot(&alice.node, &id("acme"), &id("app"))
        .await
        .unwrap();
    assert!(!after.mine.contains(&"r-9".to_owned()));
    assert!(after.relayed_for.is_empty(), "it is cara's own, for no one");

    let alice_key = device(&alice).await;
    let refused = decide_as(&alice, &alice_key, "r-9", answer("a")).await;
    assert_eq!(refused.unwrap_err().kind, ErrorKind::Forbidden);

    let key = device(&cara).await;
    let decided = decide_as(&cara, &key, "r-9", answer("a")).await.unwrap();
    assert_eq!(decided, Decided::Delivered);
    let state = memory.lock().unwrap();
    let applied = state.applied.last().unwrap();
    assert_eq!(
        (applied.target.as_str(), applied.by.as_str()),
        ("r-9", "cara")
    );
    assert_eq!(applied.on_behalf_of, None, "nobody is answered for");
}

#[tokio::test]
async fn the_directory_gives_the_member_what_the_seat_said_and_the_seats_names() {
    let (alice, _) = org_with_a_seat().await;
    let _cara = joined_as_carol(&alice).await;
    let found = directory(&alice.node, &id("acme")).unwrap();
    assert!(found.seats.is_empty());
    let cara = found
        .members
        .iter()
        .find(|one| one.id == id("cara"))
        .unwrap();
    assert_eq!(cara.ask_about, ["invoices"]);
    assert_eq!(cara.runtime_names.len(), 1);
    assert_eq!(cara.runtime_names[0].names, ["finance"]);
}

#[tokio::test]
async fn a_claim_link_for_a_seat_the_org_does_not_have_is_not_made() {
    let (alice, _) = org_with_a_seat().await;
    let error = invite_for_seat(&alice.node, &id("acme"), id("zed"), Role::Member).unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotFound);
    assert!(error.message.contains("zed"), "{}", error.message);
}

#[tokio::test]
async fn a_seat_that_is_gone_when_someone_joins_refuses_the_join_and_leaves_the_invite_open() {
    let (alice, _) = org_with_a_seat().await;
    let link = invite_for_seat(&alice.node, &id("acme"), id("carol"), Role::Member).unwrap();
    seats::remove_seat(&alice.node, &id("acme"), &id("carol")).unwrap();
    let before = current(&alice).body.version;

    let cara = node("cara", memory_runtime().0).await;
    let error = join(&cara.node, &link, |_| true).await.unwrap_err();
    assert!(error.message.contains("carol"), "{}", error.message);
    assert_eq!(current(&alice).body.version, before, "nothing was signed");
    assert!(
        open_invite(&alice.node, &id("acme"), &code_of(&link)).is_some(),
        "the invite is not used up"
    );
}

#[tokio::test]
async fn a_second_claim_link_for_the_same_seat_is_refused_once_the_first_took_it() {
    let (alice, _) = org_with_a_seat().await;
    let second = invite_for_seat(&alice.node, &id("acme"), id("carol"), Role::Member).unwrap();
    let _cara = joined_as_carol(&alice).await;
    let dan = node("dan", memory_runtime().0).await;
    let error = join(&dan.node, &second, |_| true).await.unwrap_err();
    assert!(error.message.contains("carol"), "{}", error.message);
    assert!(current(&alice).body.member(&id("dan")).is_none());
}

#[tokio::test]
async fn an_ordinary_invite_does_not_take_a_seat() {
    let (alice, _) = org_with_a_seat().await;
    let link = invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    let dan = node("dan", memory_runtime().0).await;
    join(&dan.node, &link, |_| true).await.unwrap();
    let body = current(&alice).body;
    assert_eq!(body.seats.len(), 1);
    let dan = body.member(&id("dan")).unwrap();
    assert!(dan.formerly.is_empty() && dan.ask_about.is_empty());
}

#[tokio::test]
async fn a_member_who_was_in_the_org_already_takes_a_seat_by_merging_it() {
    let (alice, _) = org_with_a_seat().await;
    let bob = joined_bob(&alice).await;
    let acme = id("acme");
    seats::merge_seat(&alice.node, &acme, &id("carol"), &id("bob")).unwrap();

    let body = current(&alice).body;
    assert!(body.seats.is_empty());
    let member = body.member(&id("bob")).unwrap();
    assert_eq!(member.formerly, [id("carol")]);
    assert_eq!(member.ask_about, ["invoices"]);
    assert_eq!(member.department, None, "their department stays theirs");

    let key = device(&bob).await;
    assert_eq!(
        decide_as(&bob, &key, "r-9", answer("a")).await.unwrap(),
        Decided::Delivered,
        "the request for the seat is bob's"
    );
}

#[tokio::test]
async fn merging_a_seat_that_is_not_there_or_into_one_who_is_not_a_member_is_refused() {
    let (alice, _) = org_with_a_seat().await;
    let acme = id("acme");
    let no_seat = seats::merge_seat(&alice.node, &acme, &id("zed"), &id("alice")).unwrap_err();
    assert_eq!(no_seat.kind, ErrorKind::NotFound);
    let version = current(&alice).body.version;
    let no_member = seats::merge_seat(&alice.node, &acme, &id("carol"), &id("zed")).unwrap_err();
    assert_eq!(no_member.kind, ErrorKind::NotFound);
    assert_eq!(current(&alice).body.version, version);
    assert_eq!(
        current(&alice).body.seats.len(),
        1,
        "the seat is still there"
    );
}

#[tokio::test]
async fn the_id_of_a_seat_that_was_taken_is_not_free_for_another() {
    let (alice, _) = org_with_a_seat().await;
    let _cara = joined_as_carol(&alice).await;
    let again = Seat {
        id: id("carol"),
        label: "Another Carol".into(),
        department: None,
        ask_about: vec![],
        relays: vec![],
    };
    let error = seats::add_seat(&alice.node, &id("acme"), again).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
}
