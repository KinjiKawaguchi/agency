//! A client's decision is kept and answered at once (202 `queued`) and applied to the team's runtime in the
//! background, with retries; sending it again is safe; where it ended is in the decision log.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::orgs;
use agency_core::outbox::{deliver_due, Parcel, ParcelBody};
use agency_core::ports::TeamRuntime;
use agency_proto::clock::Duration;
use agency_proto::org::TeamPlace;
use agency_proto::runtime::RuntimeError;
use agency_proto::RuntimeDecision;
use agency_proto::{AccountKey, Envelope, EnvelopeId, Kind, Payload, Signed};
use async_trait::async_trait;
use common::{
    alice_on, body, device, get, id, memory_runtime, node, post, Running, ENVELOPES, TOKEN,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// A runtime that counts the decisions it is asked to apply and fails them as the test says.
struct Scripted {
    inner: Arc<dyn TeamRuntime>,
    decides: AtomicUsize,
    next: Mutex<VecDeque<RuntimeError>>,
    always: Mutex<Option<RuntimeError>>,
    hangs: Mutex<bool>,
    /// A team whose every decision fails as unavailable.
    down: Mutex<Option<agency_proto::Id>>,
}

fn failure(status: u16, message: &str) -> RuntimeError {
    RuntimeError {
        status,
        message: message.into(),
    }
}

#[async_trait]
impl TeamRuntime for Scripted {
    async fn view(&self, team: &HostedTeam) -> Result<agency_proto::TeamView, RuntimeError> {
        self.inner.view(team).await
    }

    async fn decide(
        &self,
        team: &HostedTeam,
        decision: RuntimeDecision,
    ) -> Result<(), RuntimeError> {
        self.decides.fetch_add(1, Ordering::SeqCst);
        if *self.hangs.lock().unwrap() {
            std::future::pending::<()>().await;
        }
        if let Some(error) = self.next.lock().unwrap().pop_front() {
            return Err(error);
        }
        if let Some(error) = self.always.lock().unwrap().clone() {
            return Err(error);
        }
        if self.down.lock().unwrap().as_ref() == Some(&team.team) {
            return Err(failure(503, "this team's runtime is down"));
        }
        self.inner.decide(team, decision).await
    }

    async fn token_usage(
        &self,
        team: &HostedTeam,
        days: u32,
    ) -> Result<agency_proto::TokenUsage, RuntimeError> {
        self.inner.token_usage(team, days).await
    }

    async fn activity(
        &self,
        team: &HostedTeam,
        since: &agency_proto::Timestamp,
        limit: u32,
    ) -> Result<agency_proto::activity::Activity, RuntimeError> {
        self.inner.activity(team, since, limit).await
    }
}

impl Scripted {
    fn decides(&self) -> usize {
        self.decides.load(Ordering::SeqCst)
    }
}

/// alice's node with her device, whose team runtime is `Scripted` over the in-memory one.
async fn with_runtime() -> (
    Running,
    Arc<Scripted>,
    Arc<Mutex<MemoryRuntimeState>>,
    AccountKey,
) {
    let (inner, memory) = memory_runtime();
    let runtime = Arc::new(Scripted {
        inner,
        decides: AtomicUsize::new(0),
        next: Mutex::new(VecDeque::new()),
        always: Mutex::new(None),
        hangs: Mutex::new(false),
        down: Mutex::new(None),
    });
    let alice = alice_on(node("alice", runtime.clone()).await, &memory);
    let key = device(&alice).await;
    (alice, runtime, memory, key)
}

/// An answer to `target` with a fixed envelope id, signed by `key`.
fn answer_with_id(
    alice: &Running,
    key: &AccountKey,
    id_byte: u8,
    target: &str,
    text: &str,
) -> Value {
    answer_in(alice, key, "app", id_byte, target, text)
}

/// An answer to `target` in `team`, with a fixed envelope id, signed by `key`.
fn answer_in(
    alice: &Running,
    key: &AccountKey,
    team: &str,
    id_byte: u8,
    target: &str,
    text: &str,
) -> Value {
    let envelope = Envelope {
        id: EnvelopeId::from_bytes([id_byte; 16]),
        kind: agency_proto::EnvelopeKind::Answer,
        org: id("acme"),
        team: id(team),
        target: target.into(),
        payload: Payload {
            text: text.into(),
            choice: None,
        },
        account: alice.node.account(),
        at: alice.node.now_text(),
    };
    let signed = Signed::sign(Kind::Envelope, envelope, key.public().device_id(), key).unwrap();
    json!({ "envelope": signed })
}

async fn send(alice: &Running, input: &Value) -> (u16, Value) {
    send_to(alice, ENVELOPES, input).await
}

async fn send_to(alice: &Running, path: &str, input: &Value) -> (u16, Value) {
    let (status, answer) = body(
        post(alice, path, input)
            .header("x-agency-token", TOKEN)
            .send()
            .await
            .unwrap(),
    )
    .await;
    (status.as_u16(), answer)
}

async fn entry(alice: &Running, id: u8) -> (u16, Value) {
    let id = EnvelopeId::from_bytes([id; 16]);
    let (status, answer) = body(
        get(alice, &format!("/api/decisions/{}", id.as_str()))
            .send()
            .await
            .unwrap(),
    )
    .await;
    (status.as_u16(), answer)
}

async fn state(alice: &Running, id: u8) -> Value {
    entry(alice, id).await.1["state"].clone()
}

async fn log_len(alice: &Running) -> usize {
    let (_, log) = body(get(alice, "/api/decisions").send().await.unwrap()).await;
    log["entries"].as_array().unwrap().len()
}

#[tokio::test]
async fn answers_queued_at_once_and_never_waits_for_the_runtime() {
    let (alice, runtime, memory, key) = with_runtime().await;
    *runtime.hangs.lock().unwrap() = true;
    let input = answer_with_id(&alice, &key, 0x41, "r-1", "go");
    let sent = tokio::time::timeout(std::time::Duration::from_secs(5), send(&alice, &input))
        .await
        .expect("the node answers while the runtime does not");
    assert_eq!(sent, (202, json!({ "outcome": "queued" })));
    assert_eq!(runtime.decides(), 0, "the runtime was not asked yet");
    assert_eq!(state(&alice, 0x41).await["kind"], "queued");
    assert_eq!(alice.node.store().pending_parcels().len(), 1);
    assert!(memory.lock().unwrap().applied.is_empty());

    *runtime.hangs.lock().unwrap() = false;
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(runtime.decides(), 1);
    assert_eq!(state(&alice, 0x41).await, json!({ "kind": "applied" }));
    assert_eq!(memory.lock().unwrap().applied.len(), 1);
    assert!(alice.node.store().pending_parcels().is_empty());
}

#[tokio::test]
async fn tries_again_after_the_runtime_times_out() {
    let (alice, runtime, memory, key) = with_runtime().await;
    runtime.next.lock().unwrap().push_back(failure(
        503,
        "the team runtime did not answer within 20 seconds",
    ));
    let input = answer_with_id(&alice, &key, 0x42, "r-1", "go");
    assert_eq!(send(&alice, &input).await.0, 202);

    deliver_due(&alice.node).await.unwrap();
    let queued = state(&alice, 0x42).await;
    assert_eq!(queued["kind"], "queued");
    assert_eq!(queued["attempts"], 1);
    assert!(queued["lastError"].as_str().unwrap().contains("20 seconds"));
    assert_eq!(runtime.decides(), 1);

    alice.clock.advance(Duration::seconds(1));
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(runtime.decides(), 1, "not before its time");

    alice.clock.advance(Duration::seconds(1));
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(runtime.decides(), 2);
    assert_eq!(state(&alice, 0x42).await, json!({ "kind": "applied" }));
    assert_eq!(memory.lock().unwrap().applied.len(), 1);
}

#[tokio::test]
async fn gives_up_on_a_runtime_that_never_answers_and_says_why() {
    let (alice, runtime, memory, key) = with_runtime().await;
    *runtime.always.lock().unwrap() = Some(failure(503, "the runtime is down"));
    assert_eq!(
        send(&alice, &answer_with_id(&alice, &key, 0x43, "r-1", "go"))
            .await
            .0,
        202
    );
    for _ in 0..7 {
        deliver_due(&alice.node).await.unwrap();
        alice.clock.advance(Duration::seconds(130));
    }
    assert_eq!(runtime.decides(), 7);
    let refused = state(&alice, 0x43).await;
    assert_eq!(refused["kind"], "refused");
    let message = refused["message"].as_str().unwrap();
    assert!(
        message.contains("7 tries") && message.contains("the runtime is down"),
        "{message}"
    );
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(runtime.decides(), 7, "it is not tried again");
    assert!(memory.lock().unwrap().applied.is_empty());
}

#[tokio::test]
async fn a_resend_is_kept_once_and_never_changes_when_it_was_decided() {
    let (alice, runtime, memory, key) = with_runtime().await;
    let input = answer_with_id(&alice, &key, 0x44, "r-1", "go");
    assert_eq!(send(&alice, &input).await.0, 202);
    let decided_at = entry(&alice, 0x44).await.1["decidedAt"].clone();

    alice.clock.advance(Duration::seconds(60));
    assert_eq!(
        send(&alice, &input).await,
        (202, json!({ "outcome": "queued" })),
        "still waiting"
    );
    assert_eq!(alice.node.store().pending_parcels().len(), 1);
    assert_eq!(log_len(&alice).await, 1);

    deliver_due(&alice.node).await.unwrap();
    assert_eq!(
        send(&alice, &input).await,
        (200, json!({ "outcome": "applied" })),
        "taken now"
    );
    assert_eq!(runtime.decides(), 1, "the runtime is asked once");
    assert_eq!(memory.lock().unwrap().applied.len(), 1);
    assert_eq!(log_len(&alice).await, 1);
    assert_eq!(entry(&alice, 0x44).await.1["decidedAt"], decided_at);
    assert!(alice.node.store().pending_parcels().is_empty());
}

#[tokio::test]
async fn another_decision_under_the_same_id_is_a_conflict_and_changes_nothing() {
    let (alice, runtime, _, key) = with_runtime().await;
    assert_eq!(
        send(&alice, &answer_with_id(&alice, &key, 0x45, "r-1", "go"))
            .await
            .0,
        202
    );
    let other = answer_with_id(&alice, &key, 0x45, "r-1", "stop");
    assert_eq!(send(&alice, &other).await.0, 409, "while it waits");
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(send(&alice, &other).await.0, 409, "after it was applied");
    assert_eq!(runtime.decides(), 1);
    assert_eq!(log_len(&alice).await, 1);
}

#[tokio::test]
async fn a_refusal_is_in_the_log_and_a_resend_of_it_is_409() {
    let (alice, runtime, memory, key) = with_runtime().await;
    let unknown = answer_with_id(&alice, &key, 0x46, "r-404", "go");
    assert_eq!(send(&alice, &unknown).await.0, 202, "checked when applied");
    deliver_due(&alice.node).await.unwrap();
    let refused = state(&alice, 0x46).await;
    assert_eq!(refused["kind"], "refused");
    assert!(!refused["message"].as_str().unwrap().is_empty());
    assert_eq!(send(&alice, &unknown).await.0, 409);

    runtime
        .next
        .lock()
        .unwrap()
        .push_back(failure(400, "the request is not waiting"));
    let bad = answer_with_id(&alice, &key, 0x47, "r-1", "go");
    assert_eq!(send(&alice, &bad).await.0, 202);
    deliver_due(&alice.node).await.unwrap();
    let refused = state(&alice, 0x47).await;
    assert_eq!(refused["kind"], "refused");
    assert!(refused["message"]
        .as_str()
        .unwrap()
        .contains("the request is not waiting"));
    assert_eq!(send(&alice, &bad).await.0, 409);

    alice.clock.advance(Duration::seconds(300));
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(runtime.decides(), 1, "a refusal is not tried again");
    assert!(memory.lock().unwrap().applied.is_empty());
    assert_eq!(log_len(&alice).await, 2);
}

/// A parcel in the outbox whose log record was never written, as after a crash between the two writes.
fn parcel_without_log_record(alice: &Running, input: &Value, to: &str) -> Parcel {
    let signed: Signed<Envelope> = serde_json::from_value(input["envelope"].clone()).unwrap();
    Parcel::new(
        to.into(),
        ParcelBody::Envelope(Box::new(signed)),
        &alice.node.now_text(),
    )
}

#[tokio::test]
async fn a_parcel_kept_without_its_log_record_still_decides_what_is_a_resend() {
    let (alice, _, _, key) = with_runtime().await;
    send(&alice, &answer_with_id(&alice, &key, 0x4a, "r-1", "go")).await;
    let to = alice.node.store().pending_parcels()[0].to.clone();

    let kept = answer_with_id(&alice, &key, 0x4b, "r-1", "go");
    alice
        .node
        .store()
        .put_parcel(&parcel_without_log_record(&alice, &kept, &to))
        .unwrap();
    assert_eq!(entry(&alice, 0x4b).await.0, 404);
    let other = answer_with_id(&alice, &key, 0x4b, "r-1", "stop");
    assert_eq!(send(&alice, &other).await.0, 409, "not the kept one");
    assert_eq!(entry(&alice, 0x4b).await.0, 404, "and nothing was logged");
    assert_eq!(
        send(&alice, &kept).await,
        (202, json!({ "outcome": "queued" }))
    );
    assert_eq!(state(&alice, 0x4b).await["kind"], "queued", "now logged");
    assert_eq!(alice.node.store().pending_parcels().len(), 2);
}

#[tokio::test]
async fn a_refused_parcel_without_its_log_record_still_decides_what_is_a_resend() {
    let (alice, _, _, key) = with_runtime().await;
    send(&alice, &answer_with_id(&alice, &key, 0x4c, "r-1", "go")).await;
    let to = alice.node.store().pending_parcels()[0].to.clone();

    let refused = answer_with_id(&alice, &key, 0x4d, "r-1", "go");
    let mut parcel = parcel_without_log_record(&alice, &refused, &to);
    parcel.last_error = Some("the request is not waiting".into());
    alice.node.store().refuse_parcel(&parcel).unwrap();
    let other = answer_with_id(&alice, &key, 0x4d, "r-1", "stop");
    let (status, answer) = send(&alice, &other).await;
    assert_eq!(status, 409);
    assert!(
        answer["error"]
            .as_str()
            .unwrap()
            .contains("different decision"),
        "not the refused one: {answer}"
    );
    let (status, answer) = send(&alice, &refused).await;
    assert_eq!(status, 409);
    assert!(
        answer["error"].as_str().unwrap().contains("not waiting"),
        "the refused one, with its reason: {answer}"
    );
    assert_eq!(alice.node.store().pending_parcels().len(), 1);
}

#[tokio::test]
async fn reads_one_decision_by_its_id() {
    let (alice, _, _, key) = with_runtime().await;
    assert_eq!(entry(&alice, 0x48).await.0, 404, "never kept");
    send(&alice, &answer_with_id(&alice, &key, 0x48, "r-1", "go")).await;
    let (status, found) = entry(&alice, 0x48).await;
    assert_eq!(status, 200);
    assert_eq!(
        (found["target"].as_str(), found["state"]["kind"].as_str()),
        (Some("r-1"), Some("queued"))
    );
}

#[tokio::test]
async fn a_kept_decision_says_which_device_signed_it_before_and_after_it_is_applied() {
    let (alice, _, _, key) = with_runtime().await;
    let device = json!(key.public().device_id().as_str());
    send(&alice, &answer_with_id(&alice, &key, 0x49, "r-1", "go")).await;
    assert_eq!(entry(&alice, 0x49).await.1["signer"], device, "queued");

    deliver_due(&alice.node).await.unwrap();

    assert_eq!(state(&alice, 0x49).await, json!({ "kind": "applied" }));
    assert_eq!(entry(&alice, 0x49).await.1["signer"], device, "applied");
}

#[tokio::test]
async fn says_503_and_when_to_retry_when_it_cannot_keep_the_decision() {
    let (alice, runtime, _, key) = with_runtime().await;
    let outbox = alice.data_dir().join("outbox");
    let _ = std::fs::remove_dir_all(&outbox);
    std::fs::write(&outbox, "not a directory").unwrap();
    let input = answer_with_id(&alice, &key, 0x49, "r-1", "go");
    let response = post(&alice, ENVELOPES, &input)
        .header("x-agency-token", TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(response.headers()["retry-after"], "5");
    assert_eq!(
        log_len(&alice).await,
        0,
        "a decision not kept is not logged"
    );
    assert_eq!(runtime.decides(), 0);
}

#[tokio::test]
async fn a_team_runtime_that_is_down_holds_back_no_other_team_on_the_node() {
    let (alice, runtime, memory, key) = with_runtime().await;
    orgs::add_team(
        &alice.node,
        &id("acme"),
        TeamPlace::new(id("web"), "web", alice.url()),
    )
    .unwrap();
    alice
        .node
        .update_config(|config| {
            let command = RuntimeCommand::of(vec!["/nonexistent".into()]);
            config
                .teams
                .push(HostedTeam::new(id("acme"), id("web"), id("alice"), command))
        })
        .unwrap();
    *runtime.down.lock().unwrap() = Some(id("app"));
    let first = answer_in(&alice, &key, "app", 0x51, "r-1", "go");
    let second = answer_in(&alice, &key, "web", 0x52, "r-1", "go");
    assert_eq!(send(&alice, &first).await.0, 202);
    let sent = send_to(&alice, "/api/orgs/acme/teams/web/envelopes", &second).await;
    assert_eq!(sent.0, 202, "{sent:?}");

    deliver_due(&alice.node).await.unwrap();
    assert_eq!(state(&alice, 0x51).await["kind"], "queued");
    assert_eq!(
        state(&alice, 0x52).await,
        json!({ "kind": "applied" }),
        "web's decision went in the same pass"
    );
    assert_eq!(memory.lock().unwrap().applied.len(), 1);
}
