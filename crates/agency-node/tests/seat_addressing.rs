//! A request addressed to a seat or an external party is answered for them by the members named in their
//! `relays`, by the one rule that also gives each reader their requests. Names here are made up.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_adapters::peers::read_headers;
use agency_core::client::{decision_log, submit_signed_decision, team_snapshot};
use agency_core::decisions::Decided;
use agency_core::invites::{invite, join};
use agency_core::read_auth::sign_read;
use agency_core::seats;
use agency_core::ErrorKind;
use agency_proto::client::SignedDecisionInput;
use agency_proto::org::Role;
use agency_proto::seat::{External, Seat};
use agency_proto::{AccountKey, Id};
use common::{
    alice, decide_as, device, id, joined_bob, memory_runtime, node, signed_answer, Running,
};
use std::sync::{Arc, Mutex};

type Memory = Arc<Mutex<MemoryRuntimeState>>;

fn seat(raw: &str, label: &str, relays: &[&str]) -> Seat {
    Seat {
        id: id(raw),
        label: label.into(),
        department: None,
        ask_about: vec![],
        relays: relays.iter().map(|one| id(one)).collect(),
    }
}

fn answer(choice: &str) -> agency_proto::Payload {
    common::answer(choice)
}

/// Alice's team `app` with four requests: r-1 for the seat carol (answered for by bob), r-2 for the seat dan
/// (no one: the owner), r-3 for the external party erin (answered for by bob and cara), r-4 for bob himself.
/// Bob is a member, cara an answerer.
async fn world() -> (Running, Memory, Running, Running) {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    let link = invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    let cara = node("cara", memory_runtime().0).await;
    join(&cara.node, &link, |_| true).await.unwrap();

    let acme = id("acme");
    seats::add_seat(
        &alice.node,
        &acme,
        seat("carol", "Carol (finance)", &["bob"]),
    )
    .unwrap();
    seats::add_seat(&alice.node, &acme, seat("dan", "Dan (tax)", &[])).unwrap();
    let counsel = External {
        id: id("erin"),
        label: "Erin (outside counsel)".into(),
        ask_about: vec![],
        relays: vec![id("bob"), id("cara")],
    };
    seats::add_external(&alice.node, &acme, counsel).unwrap();
    alice
        .node
        .update_config(|config| {
            let people = &mut config.teams[0].people;
            for (name, member) in [
                ("finance", "carol"),
                ("tax", "dan"),
                ("counsel", "erin"),
                ("reviewer", "bob"),
            ] {
                people.insert(name.into(), id(member));
            }
        })
        .unwrap();
    let mut state = memory.lock().unwrap();
    for (request, to) in [
        ("r-1", "finance"),
        ("r-2", "tax"),
        ("r-3", "counsel"),
        ("r-4", "reviewer"),
    ] {
        state
            .requests
            .push((request.into(), format!("Question {request}")));
        state.to.insert(request.into(), to.into());
    }
    drop(state);
    (alice, memory, bob, cara)
}

fn applied(memory: &Memory) -> Vec<(String, String, Option<String>)> {
    let state = memory.lock().unwrap();
    let decisions = state.applied.iter();
    decisions
        .map(|one| {
            (
                one.target.clone(),
                one.by.to_string(),
                one.on_behalf_of.clone(),
            )
        })
        .collect()
}

/// The outcome of deciding `target`, whether it was accepted or refused.
async fn attempt(
    running: &Running,
    key: &AccountKey,
    target: &str,
) -> agency_core::Result<Decided> {
    decide_as(running, key, target, answer("a")).await
}

async fn decide(running: &Running, key: &AccountKey, target: &str) -> Decided {
    attempt(running, key, target).await.unwrap()
}

/// A refusal comes back as a refused decision when the team is on another node and as a forbidden error
/// when it is on this one.
async fn is_refused(running: &Running, key: &AccountKey, target: &str) -> bool {
    match attempt(running, key, target).await {
        Ok(decided) => matches!(decided, Decided::Refused(_)),
        Err(error) => error.kind == ErrorKind::Forbidden,
    }
}

#[tokio::test]
async fn a_member_who_answers_for_a_seat_decides_its_request_and_the_runtime_is_told_for_whom() {
    let (alice, memory, bob, _) = world().await;
    let key = device(&bob).await;
    assert_eq!(decide(&bob, &key, "r-1").await, Decided::Delivered);
    assert_eq!(
        applied(&memory),
        [("r-1".into(), "bob".into(), Some("carol".into()))]
    );
    // His own request is his own: nobody is answered for.
    assert_eq!(decide(&bob, &key, "r-4").await, Decided::Delivered);
    assert_eq!(applied(&memory)[1], ("r-4".into(), "bob".into(), None));
    drop(alice);
}

#[tokio::test]
async fn the_owner_is_not_the_one_who_answers_for_a_seat_that_names_someone_else() {
    let (alice, memory, _, _) = world().await;
    let key = device(&alice).await;
    assert!(
        is_refused(&alice, &key, "r-1").await,
        "r-1 is for carol, answered for by bob"
    );
    assert!(applied(&memory).is_empty());
}

#[tokio::test]
async fn a_seat_with_no_one_named_falls_to_the_teams_owner_and_to_no_one_else() {
    let (alice, memory, bob, _) = world().await;
    let (owner, member) = (device(&alice).await, device(&bob).await);
    assert!(is_refused(&bob, &member, "r-2").await);
    assert_eq!(decide(&alice, &owner, "r-2").await, Decided::Applied);
    assert_eq!(
        applied(&memory),
        [("r-2".into(), "alice".into(), Some("dan".into()))]
    );
}

#[tokio::test]
async fn an_external_party_is_answered_for_by_each_member_named_and_by_no_one_else() {
    let (alice, memory, bob, _) = world().await;
    let (owner, member) = (device(&alice).await, device(&bob).await);
    assert!(
        is_refused(&alice, &owner, "r-3").await,
        "alice is not named"
    );
    assert_eq!(decide(&bob, &member, "r-3").await, Decided::Delivered);
    assert_eq!(
        applied(&memory),
        [("r-3".into(), "bob".into(), Some("erin".into()))]
    );
}

#[tokio::test]
async fn an_answerer_reads_the_requests_she_answers_for_and_can_answer_exactly_those() {
    let (alice, memory, _, cara) = world().await;
    let key = device(&cara).await;
    let path = "/fed/v1/teams/acme/app/view";
    let auth = sign_read(cara.node.key(), &id("cara"), "GET", path, cara.node.now());
    let mut request = reqwest::Client::new().get(format!("{}{path}", alice.url()));
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    let view: serde_json::Value = request.send().await.unwrap().json().await.unwrap();
    let shown: Vec<&str> = view["view"]["requests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        shown,
        ["r-3"],
        "the one request addressed to an external party she answers for"
    );

    // The same rule decides what she may answer: what she is shown, and nothing else.
    for target in ["r-1", "r-2", "r-4"] {
        assert!(is_refused(&cara, &key, target).await, "{target}");
    }
    assert_eq!(decide(&cara, &key, "r-3").await, Decided::Delivered);
    assert_eq!(
        applied(&memory),
        [("r-3".into(), "cara".into(), Some("erin".into()))]
    );
}

#[tokio::test]
async fn the_snapshot_says_for_whom_each_of_the_readers_requests_is() {
    let (alice, _, _, _) = world().await;
    let snapshot = team_snapshot(&alice.node, &id("acme"), &id("app"))
        .await
        .unwrap();
    assert_eq!(
        snapshot.mine,
        ["r-2"],
        "the owner's: only the seat that names no one"
    );
    assert_eq!(
        snapshot.relayed_for.into_iter().collect::<Vec<_>>(),
        [("r-2".to_owned(), "Dan (tax)".to_owned())]
    );
}

#[tokio::test]
async fn the_decision_log_keeps_whom_a_decision_answered_for_as_the_node_found_it() {
    let (alice, memory, _, _) = world().await;
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-5".into(), "Question r-5".into()));
    let key = device(&alice).await;
    decide(&alice, &key, "r-2").await;
    decide(&alice, &key, "r-5").await;
    let log = decision_log(&alice.node, None);
    let by_target = |target: &str| log.entries.iter().find(|one| one.target == target).unwrap();
    let relayed = by_target("r-2").relayed_for.as_ref().unwrap();
    assert_eq!(
        (relayed.id.as_str(), relayed.label.as_str()),
        ("dan", "Dan (tax)")
    );
    assert_eq!(
        by_target("r-5").relayed_for,
        None,
        "a request that names no one is the owner's own"
    );
}

#[tokio::test]
async fn a_client_cannot_label_a_decision_as_answered_for_someone() {
    let (alice, _, _, _) = world().await;
    let key = device(&alice).await;
    let input = SignedDecisionInput {
        envelope: signed_answer(&alice, &key, "r-2", "a"),
        title: None,
        choice_label: None,
    };
    let mut claim = serde_json::to_value(&input).unwrap();
    claim["relayedFor"] = serde_json::json!("Erin (outside counsel)");
    assert!(
        serde_json::from_value::<SignedDecisionInput>(claim).is_err(),
        "the node alone says for whom a decision was made"
    );
    submit_signed_decision(&alice.node, &id("acme"), &id("app"), input)
        .await
        .unwrap();
    let log = decision_log(&alice.node, None);
    assert_eq!(
        log.entries[0]
            .relayed_for
            .as_ref()
            .map(|one| one.id.as_str()),
        Some("dan"),
        "never erin"
    );
}

#[tokio::test]
async fn a_people_entry_that_names_no_one_is_found_and_the_rest_are_not() {
    let (alice, _, _, _) = world().await;
    alice
        .node
        .update_config(|config| {
            config.teams[0].people.insert("typo".into(), id("zed"));
        })
        .unwrap();
    let unknown = agency_core::orgs::unknown_addressees(&alice.node);
    let named: Vec<(String, Id)> = unknown.into_iter().map(|one| (one.name, one.id)).collect();
    assert_eq!(
        named,
        [("typo".to_owned(), id("zed"))],
        "members, seats and external parties are fine"
    );
}
