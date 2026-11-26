//! In-memory test doubles: a clock that moves only when told to, counting randomness, and a team runtime.
use agency_core::config::HostedTeam;
use agency_core::ports::{Clock, Notifier, Random, TeamRuntime};
use agency_core::push::{Notice, PushError};
use agency_proto::activity::{Activity, ActivityEvent};
use agency_proto::client::PushToken;
use agency_proto::clock::{Duration, OffsetDateTime};
use agency_proto::runtime::RuntimeError;
use agency_proto::view::{AnswerWith, PersonStatus, Request, RequestKind};
use agency_proto::{clock, EnvelopeKind, RuntimeDecision, TeamView, Timestamp, TokenUsage};
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A clock that stays where it is until `advance` or `set`.
#[derive(Clone)]
pub struct ManualClock(Arc<Mutex<OffsetDateTime>>);

impl ManualClock {
    pub fn at(raw: &str) -> Self {
        Self(Arc::new(Mutex::new(
            clock::parse(raw).expect("an RFC 3339 time"),
        )))
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *now += by;
    }

    pub fn set(&self, raw: &str) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            clock::parse(raw).expect("an RFC 3339 time");
    }
}

impl Clock for ManualClock {
    fn now(&self) -> OffsetDateTime {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Distinct, predictable 16-byte values: a counter in the last 8 bytes. Not random; tests only.
#[derive(Default)]
pub struct CountingRandom(AtomicU64);

impl Random for CountingRandom {
    fn bytes16(&self) -> [u8; 16] {
        let next = self.0.fetch_add(1, Ordering::Relaxed) + 1;
        let mut bytes = [0u8; 16];
        bytes[8..].copy_from_slice(&next.to_be_bytes());
        bytes
    }
}

/// A team runtime that keeps its requests in memory and records the decisions applied to them.
#[derive(Default, Debug)]
pub struct MemoryRuntimeState {
    /// Requests waiting for an answer: (id, title).
    pub requests: Vec<(String, String)>,
    /// The runtime name a request is addressed to (`to`), by request id; absent for the team's owner.
    pub to: BTreeMap<String, String>,
    /// Board items waiting for a decision (`pendingDecision`, judged by their item reference): (ref, title).
    pub pending: Vec<(String, String)>,
    /// The people the view shows.
    pub people: Vec<PersonStatus>,
    pub applied: Vec<RuntimeDecision>,
    /// The usage it reports, with `days` set to the range asked for; `None` answers 404 like a runtime
    /// without `tokenUsage`.
    pub usage: Option<TokenUsage>,
    /// The ranges `token_usage` was asked for.
    pub usage_days: Vec<u32>,
    /// The events it reports, oldest first; `None` answers 404 like a runtime without `activity`. A read
    /// gets the events after its `since`, cut at its `limit`.
    pub activity: Option<Vec<ActivityEvent>>,
    /// What `activity` was asked for: `since` and `limit`.
    pub activity_asked: Vec<(Timestamp, u32)>,
}

#[derive(Clone, Default)]
pub struct MemoryRuntime(pub Arc<Mutex<MemoryRuntimeState>>);

fn poisoned() -> RuntimeError {
    RuntimeError {
        status: 503,
        message: "the memory runtime is poisoned".into(),
    }
}

fn question(id: &str, title: &str, to: Option<&String>, at: &Timestamp) -> Request {
    Request {
        id: id.to_owned(),
        kind: RequestKind::Question,
        title: title.to_owned(),
        context: String::new(),
        document: None,
        from: "lead".into(),
        to: to.cloned(),
        created_at: at.clone(),
        issue: None,
        blocks: vec![],
        options: vec![],
        thread: None,
        answer_with: AnswerWith {
            kind: EnvelopeKind::Answer,
            target: id.to_owned(),
        },
    }
}

fn pending_decision(item: &str, title: &str, to: Option<&String>, at: &Timestamp) -> Request {
    Request {
        id: format!("d-{item}"),
        kind: RequestKind::PendingDecision,
        answer_with: AnswerWith {
            kind: EnvelopeKind::Judge,
            target: item.to_owned(),
        },
        ..question(item, title, to, at)
    }
}

#[async_trait]
impl TeamRuntime for MemoryRuntime {
    async fn view(&self, team: &HostedTeam) -> Result<TeamView, RuntimeError> {
        let state = self.0.lock().map_err(|_| poisoned())?;
        let at = Timestamp::parse("2026-01-01T00:00:00Z").map_err(|error| RuntimeError {
            status: 503,
            message: error.to_string(),
        })?;
        let questions = state
            .requests
            .iter()
            .map(|(id, title)| question(id, title, state.to.get(id), &at));
        let pending = state
            .pending
            .iter()
            .map(|(item, title)| pending_decision(item, title, state.to.get(item), &at));
        let requests = questions.chain(pending).collect();
        Ok(TeamView {
            org: team.org.clone(),
            team: team.team.clone(),
            label: team.team.to_string(),
            generated_at: at,
            people: state.people.clone(),
            requests,
            work: vec![],
            epics: vec![],
        })
    }

    async fn decide(
        &self,
        _team: &HostedTeam,
        decision: RuntimeDecision,
    ) -> Result<(), RuntimeError> {
        let mut state = self.0.lock().map_err(|_| poisoned())?;
        let waiting = match decision.kind {
            EnvelopeKind::Answer => &state.requests,
            EnvelopeKind::Judge => &state.pending,
        };
        if !waiting.iter().any(|(id, _)| id == &decision.target) {
            return Err(RuntimeError {
                status: 404,
                message: format!("no such request: {}", decision.target),
            });
        }
        state.applied.push(decision);
        Ok(())
    }

    async fn token_usage(&self, _team: &HostedTeam, days: u32) -> Result<TokenUsage, RuntimeError> {
        let mut state = self.0.lock().map_err(|_| poisoned())?;
        state.usage_days.push(days);
        let usage = state
            .usage
            .clone()
            .map(|usage| TokenUsage { days, ..usage });
        usage.ok_or_else(|| RuntimeError {
            status: 404,
            message: "the memory runtime does not report token usage".into(),
        })
    }

    async fn activity(
        &self,
        team: &HostedTeam,
        since: &Timestamp,
        limit: u32,
    ) -> Result<Activity, RuntimeError> {
        let mut state = self.0.lock().map_err(|_| poisoned())?;
        state.activity_asked.push((since.clone(), limit));
        let Some(all) = state.activity.as_ref() else {
            return Err(RuntimeError {
                status: 404,
                message: "the memory runtime does not report activity".into(),
            });
        };
        let after: Vec<&ActivityEvent> = all
            .iter()
            .filter(|event| clock::parse(event.at.as_str()) > clock::parse(since.as_str()))
            .collect();
        let limit = limit as usize;
        let truncated = after.len() > limit;
        let events: Vec<ActivityEvent> = after.into_iter().take(limit).cloned().collect();
        let generated_at = events
            .last()
            .map_or_else(|| since.clone(), |last| last.at.clone());
        Ok(Activity {
            org: team.org.clone(),
            team: team.team.clone(),
            generated_at,
            truncated,
            events,
        })
    }
}

/// Sends nothing: the notifier of a node without an APNs key.
pub struct NoNotifier;

#[async_trait]
impl Notifier for NoNotifier {
    async fn send(&self, _token: &PushToken, _notice: &Notice) -> Result<(), PushError> {
        Ok(())
    }
}

/// A notifier that records what it was asked to send. Tokens in `gone` answer like an unregistered
/// device; with `failing`, every send fails.
#[derive(Default, Debug)]
pub struct MemoryNotifierState {
    pub sent: Vec<(String, Notice)>,
    pub gone: Vec<String>,
    pub failing: bool,
}

#[derive(Clone, Default)]
pub struct MemoryNotifier(pub Arc<Mutex<MemoryNotifierState>>);

#[async_trait]
impl Notifier for MemoryNotifier {
    async fn send(&self, token: &PushToken, notice: &Notice) -> Result<(), PushError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| PushError::Failed("the memory notifier is poisoned".into()))?;
        if state.failing {
            return Err(PushError::Failed("APNs did not answer".into()));
        }
        if state.gone.contains(&token.token) {
            return Err(PushError::Gone);
        }
        state.sent.push((token.token.clone(), notice.clone()));
        Ok(())
    }
}
