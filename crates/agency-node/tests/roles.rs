//! Who may decide what, and who reads what. Only a request's addressee decides it, by the rule that
//! also makes it theirs (`mine`); an answerer reads only the requests addressed to them.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_adapters::peers::read_headers;
use agency_core::decisions::{accept_envelope, Accepted, Decided};
use agency_core::invites::{invite, join};
use agency_core::read_auth::sign_read;
use agency_core::ErrorKind;
use agency_proto::{AccountKey, EnvelopeKind, Kind, Role, Signed};
use common::{
    alice, answer, decide_as, device, id, joined_bob, memory_runtime, node, signed_answer, Running,
};
use std::sync::{Arc, Mutex};

type Memory = Arc<Mutex<MemoryRuntimeState>>;

/// alice's team app: r-1 for the owner (alice), r-2 for `reviewer` (bob), r-3 for `qa` (carol),
/// r-4 for a name the team does not map (so the owner's), and the board item app#7 waiting for
/// `reviewer`'s decision.
fn address(alice: &Running, memory: &Memory) {
    alice
        .node
        .update_config(|config| {
            let people = &mut config.teams[0].people;
            people.insert("reviewer".into(), id("bob"));
            people.insert("qa".into(), id("carol"));
        })
        .unwrap();
    let mut state = memory.lock().unwrap();
    for (request, to) in [
        ("r-2", Some("reviewer")),
        ("r-3", Some("qa")),
        ("r-4", Some("someone")),
    ] {
        state
            .requests
            .push((request.into(), format!("Request {request}")));
        if let Some(to) = to {
            state.to.insert(request.into(), to.into());
        }
    }
    state.pending.push(("app#7".into(), "Merge it?".into()));
    state.to.insert("app#7".into(), "reviewer".into());
}

/// The decisions the runtime got: target, member, and runtime name.
fn applied(memory: &Memory) -> Vec<(String, String)> {
    let state = memory.lock().unwrap();
    let decisions = state.applied.iter();
    decisions
        .map(|one| {
            let name = one.by_name.clone().unwrap_or_default();
            (one.target.clone(), format!("{} as {name}", one.by))
        })
        .collect()
}

/// carol's node, joined to acme as an answerer.
async fn joined_answerer(alice: &Running) -> Running {
    let carol = node("carol", memory_runtime().0).await;
    let link = invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    join(&carol.node, &link, |_| true).await.unwrap();
    carol
}

/// A decision of `kind` on `target`, signed by `key` as the account of `running`.
fn decision(
    running: &Running,
    key: &AccountKey,
    kind: EnvelopeKind,
    target: &str,
) -> Signed<agency_proto::Envelope> {
    let mut body = signed_answer(running, key, target, "a").body;
    body.kind = kind;
    Signed::sign(Kind::Envelope, body, key.public().device_id(), key).unwrap()
}

#[tokio::test]
async fn only_the_addressee_decides_a_request() {
    let (alice, memory) = alice().await;
    address(&alice, &memory);
    let bob = joined_bob(&alice).await;
    // bob's device first: his node keeps only the next version, which alice's node writes for it
    let bob_key = device(&bob).await;
    let alice_key = device(&alice).await;

    assert_eq!(
        decide_as(&alice, &alice_key, "r-1", answer("a"))
            .await
            .unwrap(),
        Decided::Applied,
        "the owner decides a request without `to`"
    );
    assert_eq!(
        decide_as(&alice, &alice_key, "r-4", answer("a"))
            .await
            .unwrap(),
        Decided::Applied,
        "and one whose `to` the team does not map"
    );
    assert_eq!(
        decide_as(&bob, &bob_key, "r-2", answer("a")).await.unwrap(),
        Decided::Delivered,
        "the reviewer decides the request addressed to the reviewer"
    );

    let refused = decide_as(&alice, &alice_key, "r-2", answer("b"))
        .await
        .unwrap_err();
    assert_eq!(
        refused.kind,
        ErrorKind::Forbidden,
        "the owner does not decide the reviewer's request"
    );
    for target in ["r-1", "r-3", "r-4"] {
        let outcome = decide_as(&bob, &bob_key, target, answer("b"))
            .await
            .unwrap();
        assert!(
            matches!(outcome, Decided::Refused(_)),
            "bob on {target}: {outcome:?}"
        );
    }
    let direct = accept_envelope(
        &alice.node,
        &id("acme"),
        &id("app"),
        decision(&bob, &bob_key, EnvelopeKind::Answer, "r-1"),
    )
    .await
    .unwrap_err();
    assert_eq!(
        direct.kind,
        ErrorKind::Forbidden,
        "the team's node checks it itself"
    );
    assert_eq!(
        applied(&memory),
        [
            ("r-1".into(), "alice as owner".into()),
            ("r-4".into(), "alice as owner".into()),
            ("r-2".into(), "bob as reviewer".into())
        ]
    );
}

#[tokio::test]
async fn a_pending_decision_is_judged_by_its_addressee_and_only_with_judge() {
    let (alice, memory) = alice().await;
    address(&alice, &memory);
    let bob = joined_bob(&alice).await;
    // bob's device first: his node keeps only the next version, which alice's node writes for it
    let bob_key = device(&bob).await;
    let alice_key = device(&alice).await;
    let (org, team) = (id("acme"), id("app"));

    let by_owner = decision(&alice, &alice_key, EnvelopeKind::Judge, "app#7");
    let refused = accept_envelope(&alice.node, &org, &team, by_owner)
        .await
        .unwrap_err();
    assert_eq!(
        refused.kind,
        ErrorKind::Forbidden,
        "the item waits for the reviewer"
    );
    let as_answer = decision(&bob, &bob_key, EnvelopeKind::Answer, "app#7");
    let refused = accept_envelope(&alice.node, &org, &team, as_answer)
        .await
        .unwrap_err();
    assert_eq!(
        refused.kind,
        ErrorKind::Forbidden,
        "an item is judged, not answered"
    );

    let judged = decision(&bob, &bob_key, EnvelopeKind::Judge, "app#7");
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, judged.clone())
            .await
            .unwrap(),
        Accepted::Applied
    );
    assert_eq!(
        applied(&memory),
        [("app#7".into(), "bob as reviewer".into())]
    );

    memory.lock().unwrap().pending.clear();
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, judged)
            .await
            .unwrap(),
        Accepted::Duplicate,
        "a retried delivery is a duplicate, even after the item stopped waiting"
    );
}

#[tokio::test]
async fn an_answerer_answers_and_reads_only_what_is_addressed_to_them() {
    let (alice, memory) = alice().await;
    address(&alice, &memory);
    memory.lock().unwrap().usage = Some(
        serde_json::from_str(include_str!("../../../testdata/tokenusage.sample.json")).unwrap(),
    );
    let carol = joined_answerer(&alice).await;
    let carol_key = device(&carol).await;

    assert_eq!(
        decide_as(&carol, &carol_key, "r-3", answer("a"))
            .await
            .unwrap(),
        Decided::Delivered
    );
    for target in ["r-1", "r-2", "r-4"] {
        let outcome = decide_as(&carol, &carol_key, target, answer("a"))
            .await
            .unwrap();
        assert!(
            matches!(outcome, Decided::Refused(_)),
            "carol on {target}: {outcome:?}"
        );
    }
    let judged = decision(&carol, &carol_key, EnvelopeKind::Judge, "app#7");
    let refused = accept_envelope(&alice.node, &id("acme"), &id("app"), judged)
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Forbidden);
    assert_eq!(applied(&memory), [("r-3".into(), "carol as qa".into())]);

    let read = |path: &str| {
        let auth = sign_read(
            carol.node.key(),
            &id("carol"),
            "GET",
            path,
            carol.node.now(),
        );
        let mut request = reqwest::Client::new().get(format!("{}{path}", alice.url()));
        for (name, value) in read_headers(&auth) {
            request = request.header(name, value);
        }
        request.send()
    };
    let view: serde_json::Value = read("/fed/v1/teams/acme/app/view")
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<&str> = view["view"]["requests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|request| request["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["r-3"], "only the requests addressed to carol");
    assert_eq!(view["viewer"], "qa");
    for part in ["people", "work", "epics"] {
        assert_eq!(view["view"][part], serde_json::json!([]), "{part}");
    }
    let usage = read("/fed/v1/teams/acme/app/usage?days=7").await.unwrap();
    assert_eq!(usage.status(), 403, "an answerer does not read usage");

    let bob = joined_bob(&alice).await;
    let auth = sign_read(
        bob.node.key(),
        &id("bob"),
        "GET",
        "/fed/v1/teams/acme/app/view",
        bob.node.now(),
    );
    let mut request =
        reqwest::Client::new().get(format!("{}/fed/v1/teams/acme/app/view", alice.url()));
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    let member: serde_json::Value = request.send().await.unwrap().json().await.unwrap();
    assert_eq!(
        member["view"]["requests"].as_array().unwrap().len(),
        5,
        "a member reads every request"
    );
}
