//! A request addressed to a seat or an external party is answered for them by the members named in their
//! `relays`, by the one rule that also gives each reader their requests. Names here are made up.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_adapters::peers::read_headers;
use agency_core::client::{decision_log, submit_signed_decision, team_snapshot};
use agency_core::decisions::Decided;
use agency_core::invites::{invite, join};
use agency_core::outbox::deliver_due;
use agency_core::read_auth::sign_read;
use agency_core::seats;
use agency_core::views::refresh;
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
    submit_signed_decision(
        &alice.node,
        &agency_core::guests::Caller::Owner,
        &id("acme"),
        &id("app"),
        input,
    )
    .unwrap();
    deliver_due(&alice.node).await.unwrap();
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

/// What the team's node says each of its readers answers for: bob (named `reviewer` there) for carol and
/// for erin, cara (not named there) for erin.
#[tokio::test]
async fn a_reader_on_another_node_has_the_requests_of_the_seats_they_answer_for() {
    let (_alice, _, bob, cara) = world().await;
    let (acme, app) = (id("acme"), id("app"));
    refresh(&bob.node).await.unwrap();
    refresh(&cara.node).await.unwrap();

    let snapshot = team_snapshot(&bob.node, &acme, &app).await.unwrap();
    assert_eq!(snapshot.viewer.as_deref(), Some("reviewer"));
    let mut mine = snapshot.mine;
    mine.dedup(); // the runtime fixture already has an r-1 of its own
    assert_eq!(mine, ["r-1", "r-3", "r-4"], "r-2 is the owner's");
    assert_eq!(
        snapshot.relayed_for.into_iter().collect::<Vec<_>>(),
        [
            ("r-1".to_owned(), "Carol (finance)".to_owned()),
            ("r-3".to_owned(), "Erin (outside counsel)".to_owned()),
        ],
        "r-4 is bob's own: no one is answered for"
    );

    let snapshot = team_snapshot(&cara.node, &acme, &app).await.unwrap();
    assert_eq!(snapshot.viewer, None, "the team does not name her");
    assert_eq!(snapshot.mine, ["r-3"]);
    assert_eq!(
        snapshot.relayed_for.into_iter().collect::<Vec<_>>(),
        [("r-3".to_owned(), "Erin (outside counsel)".to_owned())]
    );
}

/// What the team's node answers to `reader`'s signed read of acme/app.
async fn fed_view(team_node: &Running, reader: &Running, account: &str) -> serde_json::Value {
    let path = "/fed/v1/teams/acme/app/view";
    let auth = sign_read(
        reader.node.key(),
        &id(account),
        "GET",
        path,
        reader.node.now(),
    );
    let mut request = reqwest::Client::new().get(format!("{}{path}", team_node.url()));
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    request.send().await.unwrap().json().await.unwrap()
}

#[tokio::test]
async fn the_team_node_says_which_seats_each_reader_answers_for_by_the_name_it_knows_them_by() {
    let (alice, _, bob, cara) = world().await;
    let answered = |view: &serde_json::Value| view["answersFor"].clone();
    assert_eq!(
        answered(&fed_view(&alice, &bob, "bob").await),
        serde_json::json!([
            { "name": "counsel", "id": "erin" },
            { "name": "finance", "id": "carol" },
        ])
    );
    assert_eq!(
        answered(&fed_view(&alice, &cara, "cara").await),
        serde_json::json!([{ "name": "counsel", "id": "erin" }])
    );
    // The seat that names no one is the owner's, and the owner is not named in the people it maps.
    assert_eq!(
        answered(&fed_view(&alice, &alice, "alice").await),
        serde_json::json!([{ "name": "tax", "id": "dan" }])
    );
}

#[tokio::test]
async fn the_decision_log_of_a_team_elsewhere_keeps_whom_the_decision_answered_for() {
    let (_alice, _, bob, _) = world().await;
    let key = device(&bob).await;
    refresh(&bob.node).await.unwrap();
    assert_eq!(decide(&bob, &key, "r-1").await, Decided::Delivered);
    assert_eq!(decide(&bob, &key, "r-4").await, Decided::Delivered);
    let log = decision_log(&bob.node, None);
    let by_target = |target: &str| log.entries.iter().find(|one| one.target == target).unwrap();
    let relayed = by_target("r-1").relayed_for.as_ref().unwrap();
    assert_eq!(
        (relayed.id.as_str(), relayed.label.as_str()),
        ("carol", "Carol (finance)")
    );
    assert_eq!(by_target("r-4").relayed_for, None);
}

/// Cara takes the seat carol (finance) by a merge: its requests are hers, no one answers for it any more, and
/// the team's node still says she has them, under the name it knew the seat by.
#[tokio::test]
async fn a_member_on_another_node_who_took_a_seat_has_its_requests_as_her_own() {
    let (alice, _, bob, cara) = world().await;
    let (acme, app) = (id("acme"), id("app"));
    seats::merge_seat(&alice.node, &acme, &id("carol"), &id("cara")).unwrap();
    refresh(&bob.node).await.unwrap();
    refresh(&cara.node).await.unwrap();
    let replica = cara.node.store().load_replica(&acme, &app).unwrap();
    let said: Vec<(&str, &str)> = replica
        .answers_for
        .iter()
        .map(|one| (one.name.as_str(), one.id.as_str()))
        .collect();
    assert_eq!(said, [("counsel", "erin"), ("finance", "carol")]);
    let snapshot = team_snapshot(&cara.node, &acme, &app).await.unwrap();
    let mut mine = snapshot.mine;
    mine.dedup(); // the runtime fixture already has an r-1 of its own
    assert_eq!(mine, ["r-1", "r-3"]);
    assert_eq!(
        snapshot.relayed_for.into_iter().collect::<Vec<_>>(),
        [("r-3".to_owned(), "Erin (outside counsel)".to_owned())],
        "r-1 is hers: it is for no one else"
    );

    let snapshot = team_snapshot(&bob.node, &acme, &app).await.unwrap();
    assert!(
        !snapshot.mine.contains(&"r-1".to_owned()),
        "bob answered for the seat only while it was one"
    );
}

/// The team's node knows a seat this node's definition of the org does not have yet.
#[tokio::test]
async fn a_party_the_definition_does_not_have_yet_is_said_by_its_id() {
    let (_alice, _, bob, _) = world().await;
    let (acme, app) = (id("acme"), id("app"));
    refresh(&bob.node).await.unwrap();
    let mut replica = bob.node.store().load_replica(&acme, &app).unwrap();
    for one in &mut replica.answers_for {
        if one.name == "finance" {
            one.id = id("newseat");
        }
    }
    bob.node
        .store()
        .save_replica(&acme, &app, &replica)
        .unwrap();

    let snapshot = team_snapshot(&bob.node, &acme, &app).await.unwrap();
    assert!(
        snapshot.mine.contains(&"r-1".to_owned()),
        "still bob's to answer"
    );
    assert_eq!(
        snapshot.relayed_for.get("r-1").map(String::as_str),
        Some("newseat")
    );
    let key = device(&bob).await;
    assert_eq!(decide(&bob, &key, "r-1").await, Decided::Delivered);
    let log = decision_log(&bob.node, None);
    let relayed = log.entries[0].relayed_for.as_ref().unwrap();
    assert_eq!(
        (relayed.id.as_str(), relayed.label.as_str()),
        ("newseat", "newseat")
    );
}

#[tokio::test]
async fn a_decision_sent_before_the_node_has_read_the_team_is_logged_without_a_party() {
    let (_alice, _, bob, _) = world().await;
    let key = device(&bob).await;
    assert_eq!(decide(&bob, &key, "r-1").await, Decided::Delivered);
    let log = decision_log(&bob.node, None);
    assert_eq!(log.entries[0].relayed_for, None, "no replica to say it by");
}
