//! The Client API's answers as JSON files for clients' tests (`testdata/client/`). Two seeded nodes on
//! fixed ports answer real requests; the files must equal what they answer, so a client that decodes
//! them decodes what the node sends. `AGENCY_WRITE_FIXTURES=1` writes the files instead of comparing.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_core::views::refresh;
use agency_proto::{AccountKey, Payload};
use common::{
    alice_on, body, decide_as, get, id, joined, memory_runtime, post, seeded_node, signed_answer,
    Running, ENVELOPES, TOKEN,
};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const SAMPLE_USAGE: &str = include_str!("../../../testdata/tokenusage.sample.json");
/// Fixed ports, so the node URLs inside the answers (and the signed documents that hold them) repeat.
const ALICE_PORT: u16 = 7701;
const BOB_PORT: u16 = 7702;
/// A comment with characters a client's JSON must carry unchanged.
const COMMENT: &str = "リリースは来週にしてください 🚀 \"quoted\" \\ /";

type Answers = Vec<(&'static str, Value)>;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/client")
}

async fn read(running: &Running, path: &str) -> Value {
    let (status, answer) = body(get(running, path).send().await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{path}: {answer}");
    answer
}

async fn write(running: &Running, path: &str, input: &Value) -> Value {
    let request = post(running, path, input).header("x-agency-token", TOKEN);
    let (status, answer) = body(request.send().await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{path}: {answer}");
    answer
}

fn waiting_person(id: &str, on: &str) -> Value {
    json!({ "id": id, "label": id, "kind": "agent", "role": "member", "state": "idle",
            "waitingOn": { "kind": "person", "person": on }, "status": format!("Waiting on {on}"), "holding": ["app#7"] })
}

/// alice's node hosts acme/app with two requests, two people waiting (one on the owner, one on the
/// reviewer), and token usage; bob's node is a member that reads it as the reviewer.
async fn nodes() -> (Running, Running, Arc<Mutex<MemoryRuntimeState>>) {
    let (runtime, memory) = memory_runtime();
    let alice = alice_on(seeded_node("alice", runtime, 1, ALICE_PORT).await, &memory);
    alice
        .node
        .update_config(|config| {
            config.teams[0].people.insert("reviewer".into(), id("bob"));
        })
        .unwrap();
    {
        let mut state = memory.lock().unwrap();
        state.requests.push(("r-2".into(), "Ship it?".into()));
        state.usage = Some(serde_json::from_str(SAMPLE_USAGE).unwrap());
        state.people = [("backend", "owner"), ("frontend", "reviewer")]
            .map(|(id, on)| serde_json::from_value(waiting_person(id, on)).unwrap())
            .into();
    }
    let bob = seeded_node("bob", memory_runtime().0, 2, BOB_PORT).await;
    let bob = joined(&alice, bob).await;
    (alice, bob, memory)
}

/// What alice's node answers after an answer and a comment from her laptop, a decision sent through the
/// Client API, a device request, and a push token.
async fn alice_answers(alice: &Running) -> Answers {
    let key = AccountKey::from_seed([9; 32]);
    let added = agency_core::devices::add_device(&alice.node, "Laptop", &key.public(), None).await;
    assert_eq!(added.len(), 1, "{added:?}");
    decide_as(alice, &key, "r-1", common::answer("a"))
        .await
        .unwrap();
    let comment = Payload {
        text: COMMENT.into(),
        choice: None,
    };
    decide_as(alice, &key, "r-2", comment).await.unwrap();
    let envelope = signed_answer(alice, &key, "r-2", "b");
    let decided = write(alice, ENVELOPES, &json!({ "envelope": envelope })).await;
    let phone = AccountKey::from_seed([10; 32]).public();
    let request = json!({ "label": "iPhone", "publicKey": phone });
    let requested = write(alice, "/api/devices/requests", &request).await;
    let token =
        json!({ "token": "ab12".repeat(16), "environment": "production", "label": "iPhone" });
    let registered = write(alice, "/api/push/tokens", &token).await;
    let (status, error) = body(get(alice, "/api/orgs/other").send().await.unwrap()).await;
    assert_eq!(status, 404);
    vec![
        ("me", read(alice, "/api/me").await),
        ("session", read(alice, "/api/session").await),
        ("org", read(alice, "/api/orgs/acme").await),
        (
            "team-snapshot-local",
            read(alice, "/api/orgs/acme/teams/app/view").await,
        ),
        ("decision-log", read(alice, "/api/decisions").await),
        ("decision-result", decided),
        ("device-list", read(alice, "/api/devices").await),
        ("device-request", requested),
        ("push-token", registered),
        ("push-token-list", read(alice, "/api/push/tokens").await),
        (
            "funding",
            read(alice, "/api/orgs/acme/teams/app/usage?days=7").await,
        ),
        ("error", error),
    ]
}

/// What bob's node answers: its replica of acme/app, and a decision queued while alice's node is down.
async fn bob_answers(alice: &mut Running, bob: &Running) -> Answers {
    refresh(&bob.node).await.unwrap();
    let replica = read(bob, "/api/orgs/acme/teams/app/view").await;
    let me = read(bob, "/api/me").await;
    let key = AccountKey::from_seed([11; 32]);
    let added = agency_core::devices::add_device(&bob.node, "Laptop", &key.public(), None).await;
    assert_eq!(added.len(), 1, "{added:?}");
    alice.stop().await;
    let queued = write(
        bob,
        ENVELOPES,
        &json!({ "envelope": signed_answer(bob, &key, "r-1", "b") }),
    )
    .await;
    assert_eq!(queued["outcome"], "queued", "{queued}");
    vec![
        ("team-snapshot-replica", replica),
        ("me-member", me),
        ("decision-result-queued", queued),
        ("decision-log-member", read(bob, "/api/decisions").await),
    ]
}

#[tokio::test]
async fn the_client_fixtures_are_what_the_node_answers() {
    let (mut alice, bob, _memory) = nodes().await;
    let mut answers = alice_answers(&alice).await;
    answers.extend(bob_answers(&mut alice, &bob).await);
    let dir = fixture_dir();
    let is_writing = std::env::var("AGENCY_WRITE_FIXTURES").is_ok_and(|value| value == "1");
    if is_writing {
        std::fs::create_dir_all(&dir).unwrap();
    }
    let mut stale = vec![];
    for (name, answer) in &answers {
        let path = dir.join(format!("{name}.json"));
        let text = serde_json::to_string_pretty(answer).unwrap() + "\n";
        if is_writing {
            std::fs::write(&path, &text).unwrap();
        } else if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
            stale.push(*name);
        }
    }
    assert!(
        stale.is_empty(),
        "testdata/client is out of date for {stale:?}; run AGENCY_WRITE_FIXTURES=1 cargo test -p agency-node --test fixtures and commit the files"
    );
}
