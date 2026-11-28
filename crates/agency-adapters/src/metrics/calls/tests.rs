use super::*;
use crate::memory::ManualClock;
use agency_core::config::RuntimeCommand;
use agency_core::ports::DefinitionAccess;
use agency_proto::runtime::RuntimeError;
use agency_proto::{EnvelopeKind, Id, RuntimeDecision};
use std::sync::Mutex;

const SHORT: Duration = Duration::from_millis(100);

fn recorder() -> (Recorder, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let clock = ManualClock::at("2026-11-24T11:30:00Z");
    (
        Recorder::new(dir.path().join("metrics"), Arc::new(clock)),
        dir,
    )
}

/// `(name, status, peer, count)` of every `call` series, sorted.
fn series(recorder: &Recorder) -> Vec<(String, String, Option<String>, u64)> {
    let metrics = recorder.read(1, Some(SeriesKind::Call), None);
    let mut found: Vec<_> = metrics
        .series
        .into_iter()
        .map(|one| (one.name, one.status, one.peer, one.count))
        .collect();
    found.sort();
    found
}

fn row(
    name: &str,
    status: &str,
    peer: Option<&str>,
    count: u64,
) -> (String, String, Option<String>, u64) {
    (name.into(), status.into(), peer.map(str::to_owned), count)
}

// ── Other nodes ──

/// A peer that answers every call with `error` after `delay`, and `post_definition` with the unit when it has none.
struct FakePeers {
    error: Option<PeerError>,
    delay: Duration,
}

impl FakePeers {
    async fn fail<T>(&self) -> Result<T, PeerError> {
        tokio::time::sleep(self.delay).await;
        Err(self
            .error
            .clone()
            .unwrap_or(PeerError::Unreachable("unused".into())))
    }

    async fn unit(&self) -> Result<(), PeerError> {
        tokio::time::sleep(self.delay).await;
        self.error.clone().map_or(Ok(()), Err)
    }
}

#[async_trait]
impl Peers for FakePeers {
    async fn get_view(
        &self,
        _: &str,
        _: &str,
        _: &ReadAuth,
        _: Option<&str>,
    ) -> Result<ViewRead, PeerError> {
        self.fail().await
    }
    async fn get_usage(&self, _: &str, _: &str, _: &ReadAuth) -> Result<TokenUsage, PeerError> {
        self.fail().await
    }
    async fn get_activity(&self, _: &str, _: &str, _: &ReadAuth) -> Result<Activity, PeerError> {
        self.fail().await
    }
    async fn get_evaluation(
        &self,
        _: &str,
        _: &str,
        _: &ReadAuth,
    ) -> Result<Evaluation, PeerError> {
        self.fail().await
    }
    async fn get_effort(&self, _: &str, _: &str, _: &ReadAuth) -> Result<Effort, PeerError> {
        self.fail().await
    }
    async fn get_about(&self, _: &str, _: &str, _: &ReadAuth) -> Result<About, PeerError> {
        self.fail().await
    }
    async fn get_definition(
        &self,
        _: &str,
        _: &str,
        _: &DefinitionAccess,
    ) -> Result<SignedOrg, PeerError> {
        self.fail().await
    }
    async fn get_versions(&self, _: &str, _: &str, _: &ReadAuth) -> Result<Versions, PeerError> {
        self.fail().await
    }
    async fn post_envelope(&self, _: &str, _: &str, _: &Signed<Envelope>) -> Result<(), PeerError> {
        self.unit().await
    }
    async fn post_definition(&self, _: &str, _: &str, _: &SignedOrg) -> Result<(), PeerError> {
        self.unit().await
    }
    async fn post_device_change(
        &self,
        _: &str,
        _: &str,
        _: &Signed<DeviceChange>,
    ) -> Result<RootAnswer, PeerError> {
        self.fail().await
    }
    async fn post_join(
        &self,
        _: &str,
        _: &str,
        _: &Signed<JoinRequest>,
    ) -> Result<RootAnswer, PeerError> {
        self.fail().await
    }
    async fn get_join(&self, _: &str, _: &str) -> Result<RootAnswer, PeerError> {
        self.fail().await
    }
}

fn envelope() -> Signed<Envelope> {
    let key = agency_proto::AccountKey::from_seed([7; 32]);
    let body = Envelope {
        id: agency_proto::EnvelopeId::from_bytes([1; 16]),
        org: Id::parse("acme").unwrap(),
        team: Id::parse("app").unwrap(),
        kind: EnvelopeKind::Answer,
        target: "r-1".into(),
        payload: agency_proto::Payload {
            text: String::new(),
            choice: Some("a".into()),
        },
        account: Id::parse("alice").unwrap(),
        at: "2026-11-24T11:30:00Z".into(),
    };
    Signed::sign(
        agency_proto::Kind::Envelope,
        body,
        key.public().device_id(),
        &key,
    )
    .unwrap()
}

fn auth() -> ReadAuth {
    ReadAuth {
        account: "alice".into(),
        date: "2026-11-24T11:30:00Z".into(),
        signature: "sig".into(),
        audience: None,
    }
}

#[tokio::test]
async fn a_call_to_another_node_is_a_series_with_the_host_of_its_origin_and_nothing_else_of_it() {
    let (recorder, _dir) = recorder();
    let peers = TimedPeers::new(
        FakePeers {
            error: None,
            delay: Duration::ZERO,
        },
        recorder.clone(),
        SHORT,
    );
    let origin = "https://user:hunter2@b.example:8443/a/path?token=x";
    let _ = peers
        .post_envelope(origin, "/fed/v1/teams/acme/app/envelopes", &envelope())
        .await;
    assert_eq!(
        series(&recorder),
        [row("peer post_envelope", "2xx", Some("b.example"), 1)]
    );
    let text = format!("{:?}", recorder.read(1, None, None).series);
    for secret in ["hunter2", "8443", "token", "/a/path", "user"] {
        assert!(!text.contains(secret), "{secret} is in a series: {text}");
    }
}

#[tokio::test]
async fn what_another_node_answered_is_the_class_of_its_status_and_a_failure_is_error_or_timeout() {
    let (recorder, _dir) = recorder();
    let call = |error: Option<PeerError>, delay: Duration| {
        let peers = TimedPeers::new(FakePeers { error, delay }, recorder.clone(), SHORT);
        async move {
            let _ = peers
                .get_view("https://b.example", "/v", &auth(), None)
                .await;
        }
    };
    let status = |code: u16| {
        Some(PeerError::Status {
            status: code,
            message: "refused".into(),
        })
    };
    call(status(500), Duration::ZERO).await;
    call(status(404), Duration::ZERO).await;
    call(status(302), Duration::ZERO).await;
    call(status(999), Duration::ZERO).await;
    call(
        Some(PeerError::Unreachable("refused".into())),
        Duration::ZERO,
    )
    .await;
    call(Some(PeerError::Unreachable("slow".into())), SHORT).await;
    let peer = Some("b.example");
    assert_eq!(
        series(&recorder),
        [
            row("peer get_view", "3xx", peer, 1),
            row("peer get_view", "4xx", peer, 1),
            row("peer get_view", "5xx", peer, 1),
            row("peer get_view", "error", peer, 2),
            row("peer get_view", "timeout", peer, 1),
        ],
        "999 and a quick refusal are errors; one that ran the whole time it is given is a time-out"
    );
}

// ── Team runtimes ──

struct FakeRuntime {
    error: Option<RuntimeError>,
    delay: Duration,
}

#[async_trait]
impl TeamRuntime for FakeRuntime {
    async fn view(&self, _: &HostedTeam) -> Result<TeamView, RuntimeError> {
        self.fail().await
    }
    async fn decide(&self, _: &HostedTeam, _: RuntimeDecision) -> Result<(), RuntimeError> {
        tokio::time::sleep(self.delay).await;
        self.error.clone().map_or(Ok(()), Err)
    }
    async fn token_usage(&self, _: &HostedTeam, _: u32) -> Result<TokenUsage, RuntimeError> {
        self.fail().await
    }
    async fn activity(
        &self,
        _: &HostedTeam,
        _: &Timestamp,
        _: u32,
    ) -> Result<Activity, RuntimeError> {
        self.fail().await
    }
    async fn tickets(&self, _: &HostedTeam) -> Result<Tickets, RuntimeError> {
        self.fail().await
    }
}

impl FakeRuntime {
    async fn fail<T>(&self) -> Result<T, RuntimeError> {
        tokio::time::sleep(self.delay).await;
        Err(self.error.clone().unwrap_or(RuntimeError {
            status: 503,
            message: "unused".into(),
        }))
    }
}

fn team() -> HostedTeam {
    HostedTeam::new(
        Id::parse("acme").unwrap(),
        Id::parse("app").unwrap(),
        Id::parse("alice").unwrap(),
        RuntimeCommand::of(vec!["unused".into()]),
    )
}

fn decision() -> RuntimeDecision {
    serde_json::from_value(serde_json::json!({
        "kind": "answer", "target": "r-1", "by": "alice", "payload": { "text": "", "choice": "a" }
    }))
    .unwrap()
}

fn failure_of(status: u16) -> Option<RuntimeError> {
    Some(RuntimeError {
        status,
        message: "the message".into(),
    })
}

#[tokio::test]
async fn a_call_to_a_runtime_is_ok_error_or_timeout_and_an_op_it_does_not_offer_is_no_call() {
    let (recorder, _dir) = recorder();
    let run = |error: Option<RuntimeError>, delay: Duration| {
        let runtime = TimedRuntime::new(
            Arc::new(FakeRuntime { error, delay }),
            recorder.clone(),
            SHORT,
        );
        async move {
            let _ = runtime.decide(&team(), decision()).await;
            let _ = runtime.token_usage(&team(), 7).await;
        }
    };
    run(None, Duration::ZERO).await;
    run(failure_of(503), Duration::ZERO).await;
    run(failure_of(503), SHORT).await;
    run(failure_of(404), Duration::ZERO).await;
    assert_eq!(
        series(&recorder),
        [
            row("runtime usage", "error", None, 2),
            row("runtime usage", "timeout", None, 1),
        ],
        "the fourth run answers 404: an op the runtime does not offer is no call, so it is not recorded; and a decision \
         is not timed here at all, since the command runtime times each delivery of one itself"
    );
}

#[tokio::test]
async fn a_read_of_a_teams_tickets_is_its_own_call_series() {
    let (recorder, _dir) = recorder();
    for (error, delay) in [
        (failure_of(503), Duration::ZERO),
        (failure_of(503), SHORT),
        (failure_of(404), Duration::ZERO),
    ] {
        let runtime = TimedRuntime::new(
            Arc::new(FakeRuntime { error, delay }),
            recorder.clone(),
            SHORT,
        );
        let _ = runtime.tickets(&team()).await;
    }
    assert_eq!(
        series(&recorder),
        [
            row("runtime tickets", "error", None, 1),
            row("runtime tickets", "timeout", None, 1),
        ],
        "a runtime that does not offer tickets (404) is no call"
    );
}

// ── GitHub ──

struct FakeGh(Result<String, GhError>);

#[async_trait]
impl Gh for FakeGh {
    async fn run(&self, _: &[String]) -> Result<String, GhError> {
        self.0.clone()
    }
}

#[tokio::test]
async fn each_run_of_gh_is_a_series_named_by_its_kind_of_read_and_never_by_what_it_asked() {
    let (recorder, _dir) = recorder();
    let words = |args: &[&str]| args.iter().map(|word| word.to_string()).collect::<Vec<_>>();
    let graphql = words(&["api", "graphql", "-f", "query=secret-repo-name"]);
    let rest = words(&["api", "repos/acme/secret-repo/actions/runs"]);
    let ok = TimedGh::new(FakeGh(Ok("{}".into())), recorder.clone());
    let _ = ok.run(&graphql).await;
    let _ = ok.run(&rest).await;
    let slow = TimedGh::new(FakeGh(Err(GhError::TimedOut)), recorder.clone());
    let _ = slow.run(&graphql).await;
    let refused = TimedGh::new(FakeGh(Err(GhError::NotLoggedIn)), recorder.clone());
    let _ = refused.run(&rest).await;
    assert_eq!(
        series(&recorder),
        [
            row("github api", "error", None, 1),
            row("github api", "ok", None, 1),
            row("github graphql", "ok", None, 1),
            row("github graphql", "timeout", None, 1),
        ]
    );
    assert!(!format!("{:?}", recorder.read(1, None, None).series).contains("secret"));
}

// ── Push services ──

struct FakePush(Mutex<Vec<Result<(), PushError>>>);

impl FakePush {
    fn next(&self) -> Result<(), PushError> {
        let mut answers = self.0.lock().unwrap();
        if answers.is_empty() {
            Ok(())
        } else {
            answers.remove(0)
        }
    }
}

#[async_trait]
impl Notifier for FakePush {
    async fn send(&self, _: &PushToken, _: &Notice) -> Result<(), PushError> {
        self.next()
    }
    async fn send_desk_changed(&self, _: &PushToken) -> Result<(), PushError> {
        self.next()
    }
    async fn send_web(&self, _: &WebSubscription, _: &Notice) -> Result<(), PushError> {
        self.next()
    }
}

fn token() -> PushToken {
    serde_json::from_value(serde_json::json!({
        "id": "x", "token": "ab".repeat(32), "environment": "sandbox", "label": "Phone",
        "registeredAt": "2026-11-24T11:00:00Z"
    }))
    .unwrap()
}

fn subscription() -> WebSubscription {
    serde_json::from_value(serde_json::json!({
        "id": "s", "endpoint": "https://push.example/abc", "p256dh": "k", "auth": "a", "label": "Laptop",
        "registeredAt": "2026-11-24T11:00:00Z", "events": ["request"], "keyId": "k1"
    }))
    .unwrap()
}

fn notice() -> Notice {
    Notice {
        kind: agency_core::notice::NoticeKind::Device {
            device: Id::parse("d-aaaabbbbcccc").unwrap(),
            label: None,
        },
        badge: 1,
    }
}

#[tokio::test]
async fn a_send_is_timed_on_its_own_way_to_a_device_and_a_gone_device_is_not_a_failure() {
    let (recorder, _dir) = recorder();
    let apns = TimedNotifier::apns(
        FakePush(Mutex::new(vec![
            Ok(()),
            Err(PushError::Gone),
            Err(PushError::Failed("x".into())),
            Ok(()),
        ])),
        recorder.clone(),
    );
    let _ = apns.send(&token(), &notice()).await;
    let _ = apns.send(&token(), &notice()).await;
    let _ = apns.send(&token(), &notice()).await;
    let _ = apns.send_desk_changed(&token()).await;
    let _ = apns.send_web(&subscription(), &notice()).await;
    let web = TimedNotifier::web_push(
        FakePush(Mutex::new(vec![Ok(()), Err(PushError::Failed("y".into()))])),
        recorder.clone(),
    );
    let _ = web.send_web(&subscription(), &notice()).await;
    let _ = web.send_web(&subscription(), &notice()).await;
    let _ = web.send(&token(), &notice()).await;
    assert_eq!(
        series(&recorder),
        [
            row("apns desk_changed", "ok", None, 1),
            row("apns send", "4xx", None, 1),
            row("apns send", "error", None, 1),
            row("apns send", "ok", None, 1),
            row("webpush send", "error", None, 1),
            row("webpush send", "ok", None, 1),
        ],
        "the apns notifier does not time a browser send and the web one does not time a token's"
    );
}

#[tokio::test]
async fn a_batch_to_browsers_is_one_call_and_an_empty_batch_is_none() {
    let (recorder, _dir) = recorder();
    let web = TimedNotifier::web_push(
        FakePush(Mutex::new(vec![Ok(()), Ok(()), Err(PushError::Gone)])),
        recorder.clone(),
    );
    let two = [subscription(), subscription()];
    let _ = web.send_web_many(&two, &notice()).await;
    let _ = web.send_web_many(&two[..1], &notice()).await;
    let _ = web.send_web_many(&[], &notice()).await;
    assert_eq!(
        series(&recorder),
        [
            row("webpush send_many", "error", None, 1),
            row("webpush send_many", "ok", None, 1)
        ]
    );
}
