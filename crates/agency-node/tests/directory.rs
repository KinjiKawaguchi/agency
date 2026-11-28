//! `agency directory`: who an org's agents may ask for what, from the definition and the teams this node
//! runs. The shared sample is what a real node answers for the org it describes. Names here are made up.
mod common;

use agency_core::directory::directory;
use agency_core::invites::{invite, join};
use agency_core::seats;
use agency_core::{departments, ErrorKind};
use agency_proto::directory::Directory;
use agency_proto::org::{Department, Role};
use agency_proto::seat::{External, Seat};
use common::{alice, id, joined_bob, memory_runtime, node, Running};

const SAMPLE: &str = include_str!("../../../testdata/directory.sample.json");

/// The org of the sample: alice and bob, the department `ops`, the seat carol, and the external party erin,
/// with the names the team `app` on alice's node gives them.
async fn org_of_the_sample() -> (Running, Running) {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let link = invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    let cara = node("cara", memory_runtime().0).await;
    join(&cara.node, &link, |_| true).await.unwrap();
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
        unit: None,
        ask_about: vec!["invoices".into(), "budget approval".into()],
        relays: vec![id("bob")],
    };
    seats::add_seat(&alice.node, &acme, carol).unwrap();
    let erin = External {
        id: id("erin"),
        label: "Erin (outside counsel)".into(),
        ask_about: vec!["contract review".into(), "legal checks".into()],
        relays: vec![id("alice"), id("bob")],
    };
    seats::add_external(&alice.node, &acme, erin).unwrap();
    alice
        .node
        .update_config(|config| {
            let people = &mut config.teams[0].people;
            for (name, who) in [
                ("reviewer", "bob"),
                ("finance", "carol"),
                ("counsel", "erin"),
            ] {
                people.insert(name.into(), id(who));
            }
        })
        .unwrap();
    (alice, bob)
}

#[tokio::test]
async fn the_shared_sample_is_what_a_node_answers_for_the_org_it_describes() {
    let (alice, _) = org_of_the_sample().await;
    let found = directory(&alice.node, &id("acme")).unwrap();
    let sample: Directory = serde_json::from_str(SAMPLE).unwrap();
    assert_eq!(found, sample);
    assert_eq!(
        serde_json::to_value(&found).unwrap(),
        serde_json::from_str::<serde_json::Value>(SAMPLE).unwrap(),
        "and writes it as the sample does, leaving out what has no value"
    );
}

#[tokio::test]
async fn it_holds_labels_and_what_to_ask_about_and_nothing_of_a_key_a_device_or_an_account() {
    let (alice, _) = org_of_the_sample().await;
    let text = serde_json::to_string(&directory(&alice.node, &id("acme")).unwrap()).unwrap();
    for secret in [
        "publicKey",
        "devices",
        "account",
        "fingerprint",
        "email",
        "phone",
        "token",
    ] {
        assert!(!text.contains(secret), "{secret}: {text}");
    }
}

#[tokio::test]
async fn a_node_that_runs_no_team_of_the_org_gives_the_people_no_names() {
    // Alice stays up: bob reads the org from her node.
    let (_alice, bob) = org_of_the_sample().await;
    agency_core::views::refresh(&bob.node).await.unwrap();
    // Bob's node joined the org but runs none of its teams, so it does not know a team's names.
    let found = directory(&bob.node, &id("acme")).unwrap_or_else(|error| panic!("{error:?}"));
    assert!(found.members.iter().all(|one| one.runtime_names.is_empty()));
    assert!(found
        .seats
        .iter()
        .chain(&found.externals)
        .all(|one| one.runtime_names.is_empty()));
    assert_eq!(
        found.seats[0].ask_about,
        ["invoices", "budget approval"],
        "the rest is the definition's"
    );
}

#[tokio::test]
async fn an_org_the_account_is_not_in_is_not_found() {
    let (alice, _) = org_of_the_sample().await;
    let missing = directory(&alice.node, &id("elsewhere")).unwrap_err();
    assert_eq!(missing.kind, ErrorKind::NotFound);
}

#[tokio::test]
async fn the_names_are_those_of_this_orgs_teams_not_of_another_orgs_with_the_same_team_id() {
    use agency_core::config::{HostedTeam, RuntimeCommand};
    use agency_proto::org::TeamPlace;
    let (alice, _) = org_of_the_sample().await;
    // Another org on the same node, with a team that has the same id and calls alice something else.
    let other = id("other");
    agency_core::orgs::create(&alice.node, other.clone(), "Other".into()).unwrap();
    let place = TeamPlace::new(id("app"), "App", alice.url());
    agency_core::orgs::add_team(&alice.node, &other, place).unwrap();
    alice
        .node
        .update_config(|config| {
            let hosted = HostedTeam::new(
                other.clone(),
                id("app"),
                id("alice"),
                RuntimeCommand::of(vec![]),
            )
            .with_people([("elsewhere".to_owned(), id("bob"))].into());
            config.teams.push(hosted);
        })
        .unwrap();
    let found = directory(&alice.node, &id("acme")).unwrap();
    let names: Vec<_> = found
        .members
        .iter()
        .map(|one| (one.id.as_str(), one.runtime_names.len()))
        .collect();
    assert_eq!(
        names,
        [("alice", 1), ("bob", 1), ("cara", 0)],
        "one team of acme for each, not two"
    );
}
