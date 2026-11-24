//! In-memory test doubles: a clock that moves only when told to, counting randomness, and a team runtime.
use agency_core::config::HostedTeam;
use agency_core::ports::{Clock, Random, TeamRuntime};
use agency_proto::clock::{Duration, OffsetDateTime};
use agency_proto::runtime::RuntimeError;
use agency_proto::view::{AnswerWith, Request, RequestKind};
use agency_proto::{clock, EnvelopeKind, RuntimeDecision, TeamView, Timestamp};
use async_trait::async_trait;
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
    pub applied: Vec<RuntimeDecision>,
}

#[derive(Clone, Default)]
pub struct MemoryRuntime(pub Arc<Mutex<MemoryRuntimeState>>);

fn poisoned() -> RuntimeError {
    RuntimeError {
        status: 503,
        message: "the memory runtime is poisoned".into(),
    }
}

fn question(id: &str, title: &str, at: &Timestamp) -> Request {
    Request {
        id: id.to_owned(),
        kind: RequestKind::Question,
        title: title.to_owned(),
        context: String::new(),
        document: None,
        from: "lead".into(),
        to: None,
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

#[async_trait]
impl TeamRuntime for MemoryRuntime {
    async fn view(&self, team: &HostedTeam) -> Result<TeamView, RuntimeError> {
        let state = self.0.lock().map_err(|_| poisoned())?;
        let at = Timestamp::parse("2026-01-01T00:00:00Z").map_err(|error| RuntimeError {
            status: 503,
            message: error.to_string(),
        })?;
        let requests = state
            .requests
            .iter()
            .map(|(id, title)| question(id, title, &at))
            .collect();
        Ok(TeamView {
            org: team.org.clone(),
            team: team.team.clone(),
            label: team.team.to_string(),
            generated_at: at,
            people: vec![],
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
        if !state.requests.iter().any(|(id, _)| id == &decision.target) {
            return Err(RuntimeError {
                status: 404,
                message: format!("no such request: {}", decision.target),
            });
        }
        state.applied.push(decision);
        Ok(())
    }
}
