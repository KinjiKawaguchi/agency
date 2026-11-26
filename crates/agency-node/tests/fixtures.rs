//! The Client API's answers as JSON files for clients' tests (`testdata/client/`). Two seeded nodes on
//! fixed ports answer real requests; the files must equal what they answer, so a client that decodes
//! them decodes what the node sends. `AGENCY_WRITE_FIXTURES=1` writes the files instead of comparing.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_core::config::FundingConfig;
use agency_core::views::refresh;
use agency_proto::client::FundingKind;
use agency_proto::{
    AccountKey, Device, DeviceChange, DeviceOp, Kind, Payload, PublicKey, Signed, Timestamp,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use common::{
    alice_on, body, decide_as, get, id, joined, memory_runtime, post, seeded_node, signed_answer,
    Running, ENVELOPES, T0, TOKEN,
};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const SAMPLE_USAGE: &str = include_str!("../../../testdata/tokenusage.sample.json");
const SAMPLE_ACTIVITY: &str = include_str!("../../../testdata/activity.sample.json");
/// Fixed ports, so the node URLs inside the answers (and the signed documents that hold them) repeat.
const ALICE_PORT: u16 = 7701;
const BOB_PORT: u16 = 7702;
/// A comment with characters a client's JSON must carry unchanged.
/// The application server key and a browser's key from RFC 8291, appendix A.
const SAMPLE_VAPID_KEY: &str =
    "BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8";
const SAMPLE_BROWSER_KEY: &str =
    "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
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

/// The public key of a P-256 device whose private scalar is `byte` repeated, as an iPhone's hardware key
/// is written (`p256:` and the uncompressed point).
fn p256_public(byte: u8) -> PublicKey {
    let key = p256::ecdsa::SigningKey::from_slice(&[byte; 32]).unwrap();
    let point = key.verifying_key().to_encoded_point(false);
    PublicKey::try_from(format!("p256:{}", STANDARD.encode(point.as_bytes()))).unwrap()
}

/// A device change that adds `key` to `account`'s devices, signed by the device `signer`.
fn signed_add(account: &str, label: &str, key: &PublicKey, signer: &AccountKey) -> Value {
    let at = Timestamp::parse(T0).unwrap();
    let device = Device {
        id: key.device_id(),
        label: label.into(),
        public_key: key.clone(),
        added_at: at.clone(),
    };
    let change = DeviceChange {
        org: id("acme"),
        account: id(account),
        op: DeviceOp::Add {
            device: Box::new(device),
        },
        at,
    };
    let signer_id = signer.public().device_id();
    json!(Signed::sign(Kind::Device, change, signer_id, signer).unwrap())
}

/// alice's `/api/me` with two funding sources: a subscription whose usage file holds both windows, and
/// an API key, which has none. The usage file is written at a fixed time, so `readAt` repeats.
async fn me_with_funding(alice: &Running) -> Value {
    let dir = tempfile::TempDir::new().unwrap();
    let usage_file = dir.path().join("max.json");
    let usage = r#"{"five_hour":{"utilization":41.6,"resets_at":"2026-11-24T15:00:00Z"},"seven_day":{"utilization":63.2,"resets_at":"2026-11-30T00:00:00Z"}}"#;
    std::fs::write(&usage_file, usage).unwrap();
    let written = agency_proto::clock::parse("2026-11-24T11:57:00Z").unwrap();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&usage_file)
        .unwrap();
    file.set_modified(written.into()).unwrap();
    let source = |name: &str, label: &str, kind, usage_file| FundingConfig {
        id: id(name),
        label: label.into(),
        kind,
        usage_file,
        collector: None,
        seven_day_threshold: None,
    };
    let sources = vec![
        source("max", "Max", FundingKind::Subscription, Some(usage_file)),
        source("api", "API", FundingKind::ApiKey, None),
    ];
    alice
        .node
        .update_config(|config| config.funding = sources)
        .unwrap();
    let me = read(alice, "/api/me").await;
    alice
        .node
        .update_config(|config| config.funding = vec![])
        .unwrap();
    me
}

/// alice's `/api/me` when her node keeps titles off notifications: `hidesTitles` is there, `true`.
async fn me_hiding_titles(alice: &Running) -> Value {
    alice
        .node
        .update_config(|config| config.hide_titles = true)
        .unwrap();
    let me = read(alice, "/api/me").await;
    alice
        .node
        .update_config(|config| config.hide_titles = false)
        .unwrap();
    me
}

/// Bob's node reads alice's newest org definition. It takes only the next version of the one it has, so
/// this runs after each change of alice's, and a second after the clock moved on: a node accepts a
/// signed read once, and a manual clock repeats the time of the last one.
async fn catch_up(bob: &Running) {
    bob.clock.advance(agency_proto::clock::Duration::seconds(1));
    refresh(&bob.node).await.unwrap();
}

/// What alice's node answers to device changes signed by her laptop: the phone that asked is added
/// (`applied`, since her node holds the root key), then a P-256 device is added, and a P-256 key asks.
async fn alice_device_changes(alice: &Running, bob: &Running, laptop: &AccountKey) -> Answers {
    catch_up(bob).await;
    let phone = AccountKey::from_seed([10; 32]).public();
    let added = write(
        alice,
        "/api/devices/changes",
        &signed_add("alice", "iPhone", &phone, laptop),
    )
    .await;
    catch_up(bob).await;
    let hardware = signed_add("alice", "iPad", &p256_public(0x0b), laptop);
    write(alice, "/api/devices/changes", &hardware).await;
    catch_up(bob).await;
    bob.clock.set(T0);
    let tablet = json!({ "label": "Tablet", "publicKey": p256_public(0x0c) });
    write(alice, "/api/devices/requests", &tablet).await;
    vec![
        ("device-change-result", added),
        ("device-list-p256", read(alice, "/api/devices").await),
    ]
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
        let activity: agency_proto::activity::Activity =
            serde_json::from_str(SAMPLE_ACTIVITY).unwrap();
        state.activity = Some(activity.events);
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
async fn alice_answers(alice: &Running, bob: &Running) -> Answers {
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
    alice.notifier.0.lock().unwrap().web_key = Some(agency_proto::client::VapidKey {
        public_key: SAMPLE_VAPID_KEY.into(),
        key_id: "a1b2c3d4e5f6".into(),
    });
    let subscription = json!({
        "endpoint": "https://updates.push.services.mozilla.com/wpush/v2/gAAAAABk-example",
        "p256dh": SAMPLE_BROWSER_KEY,
        "auth": "BTBZMqHH6r4Tts7J_aSIgg",
        "label": "Firefox on a laptop",
    });
    let subscribed = write(alice, "/api/push/subscriptions", &subscription).await;
    let (status, error) = body(get(alice, "/api/orgs/other").send().await.unwrap()).await;
    assert_eq!(status, 404);
    let mut answers = vec![
        ("me", read(alice, "/api/me").await),
        ("push-vapid", read(alice, "/api/push/vapid").await),
        ("push-subscription", subscribed),
        (
            "push-subscription-list",
            read(alice, "/api/push/subscriptions").await,
        ),
        ("me-funding", me_with_funding(alice).await),
        ("me-hides-titles", me_hiding_titles(alice).await),
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
            "usage",
            read(alice, "/api/orgs/acme/teams/app/usage?days=7").await,
        ),
        (
            "activity",
            read(
                alice,
                "/api/orgs/acme/teams/app/activity?since=2026-11-24T11:00:00Z",
            )
            .await,
        ),
        ("error", error),
    ];
    answers.extend(alice_device_changes(alice, bob, &key).await);
    answers.push(("evaluation", evaluation_answer(alice).await));
    answers
}

/// alice's `/api/orgs/acme/evaluation` for a team that reads two repositories: three pull requests
/// (a feature that closes an issue, a fix, and a revert) merged on the last two days of a week, and a
/// default branch that was red for an hour. The node's time is 2026-11-24T12:00Z and its time zone UTC.
async fn evaluation_answer(alice: &Running) -> Value {
    use agency_core::delivery::{DeliveryFacts, MainRun, MergedPullRequest, Verdict};
    let at = |raw: &str| agency_proto::clock::parse(raw).unwrap();
    let pull_request =
        |title: &str, opened: &str, merged: &str, started: Option<&str>| MergedPullRequest {
            title: title.into(),
            body: String::new(),
            opened_at: at(opened),
            merged_at: at(merged),
            first_commit_at: Some(at(opened)),
            has_issue: started.is_some(),
            issue_started_at: started.map(at),
        };
    let run = |commit: &str, finished: &str, verdict| MainRun {
        repo: "acme/app".into(),
        commit: commit.into(),
        finished_at: at(finished),
        verdict,
    };
    let facts = DeliveryFacts {
        pull_requests: vec![
            pull_request(
                "feat(web): add a screen",
                "2026-11-23T08:00:00Z",
                "2026-11-23T10:00:00Z",
                Some("2026-11-23T06:00:00Z"),
            ),
            pull_request(
                "fix(core): refuse it",
                "2026-11-24T08:30:00Z",
                "2026-11-24T09:00:00Z",
                None,
            ),
            pull_request(
                "Revert \"fix(core): refuse it\"",
                "2026-11-24T10:00:00Z",
                "2026-11-24T10:10:00Z",
                None,
            ),
        ],
        runs: vec![
            run("a1", "2026-11-23T15:00:00Z", Verdict::Failed),
            run("a2", "2026-11-23T16:00:00Z", Verdict::Passed),
        ],
    };
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts));
    alice
        .node
        .update_config(|config| {
            let at = config
                .teams
                .iter()
                .position(|team| team.team == id("app"))
                .unwrap();
            let team = config.teams.remove(at);
            config
                .teams
                .insert(at, team.with_delivery(vec!["acme/app".into()]));
        })
        .unwrap();
    read(alice, "/api/orgs/acme/evaluation?days=7").await
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
    let phone = AccountKey::from_seed([13; 32]).public();
    let change = signed_add("bob", "iPhone", &phone, &key);
    let change_queued = write(bob, "/api/devices/changes", &change).await;
    assert_eq!(change_queued["outcome"], "queued", "{change_queued}");
    vec![
        ("device-change-result-queued", change_queued),
        ("team-snapshot-replica", replica),
        ("me-member", me),
        ("decision-result-queued", queued),
        ("decision-log-member", read(bob, "/api/decisions").await),
    ]
}

#[tokio::test]
async fn the_client_fixtures_are_what_the_node_answers() {
    let (mut alice, bob, _memory) = nodes().await;
    let mut answers = alice_answers(&alice, &bob).await;
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
