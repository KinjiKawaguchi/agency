//! In-memory test doubles: a clock that moves only when told to, counting randomness, and a team runtime.
use agency_core::config::{DeliverySite, HostedTeam};
use agency_core::delivery::DeliveryFacts;
use agency_core::ports::{
    Clock, DeliveryError, DeliverySource, Journal, Notifier, Random, TeamRuntime,
};
use agency_core::push::{Notice, PushError};
use agency_core::web_push::WebSubscription;
use agency_proto::activity::{Activity, ActivityEvent};
use agency_proto::client::{PushToken, VapidKey};
use agency_proto::clock::{Duration, OffsetDateTime, UtcOffset};
use agency_proto::runtime::RuntimeError;
use agency_proto::view::{AnswerWith, EpicForecast, PersonStatus, Request, RequestKind};
use agency_proto::{clock, EnvelopeKind, RuntimeDecision, TeamView, Timestamp, TokenUsage};
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A clock that stays where it is until `advance` or `set`.
#[derive(Clone)]
pub struct ManualClock(Arc<Mutex<OffsetDateTime>>, Arc<Mutex<UtcOffset>>);

impl ManualClock {
    pub fn at(raw: &str) -> Self {
        Self(
            Arc::new(Mutex::new(clock::parse(raw).expect("an RFC 3339 time"))),
            Arc::new(Mutex::new(UtcOffset::UTC)),
        )
    }

    /// Puts the clock in a time zone, UTC until it is set (the days of the delivery reports are its days).
    pub fn set_utc_offset(&self, offset: UtcOffset) {
        *self
            .1
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = offset;
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

    fn utc_offset(&self) -> UtcOffset {
        *self
            .1
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

/// What a `MemoryDelivery` answers, and what it was asked.
#[derive(Default, Debug)]
pub struct MemoryDeliveryState {
    /// The facts it answers; the reason it fails with; `None` fails as GitHub being unavailable.
    pub facts: Option<Result<DeliveryFacts, String>>,
    /// The site (the host and the repositories) and the start of the period of each read.
    pub asked: Vec<(DeliverySite, OffsetDateTime)>,
}

/// A source of delivery facts that answers from memory.
#[derive(Clone, Default)]
pub struct MemoryDelivery(pub Arc<Mutex<MemoryDeliveryState>>);

#[async_trait]
impl DeliverySource for MemoryDelivery {
    async fn facts(
        &self,
        site: &DeliverySite,
        since: OffsetDateTime,
    ) -> Result<DeliveryFacts, DeliveryError> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.asked.push((site.clone(), since));
        match &state.facts {
            Some(Ok(facts)) => Ok(facts.clone()),
            Some(Err(reason)) => Err(DeliveryError(reason.clone())),
            None => Err(DeliveryError("GitHub did not answer the request".into())),
        }
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
    /// The epics the view shows.
    pub epics: Vec<EpicForecast>,
    pub applied: Vec<RuntimeDecision>,
    /// The participant metrics it reports, with `days` set to the range asked for, and its own time zone as
    /// it is; `None` answers 404 like a runtime without `participantMetrics`. The ranges and time zones
    /// asked for are in `participant_asks`.
    pub participants: Option<agency_proto::participant_metrics::ParticipantMetrics>,
    pub participant_asks: Vec<(u32, i32)>,
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
            epics: state.epics.clone(),
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

    async fn participant_metrics(
        &self,
        _team: &HostedTeam,
        days: u32,
        utc_offset_minutes: i32,
    ) -> Result<agency_proto::participant_metrics::ParticipantMetrics, RuntimeError> {
        let mut state = self.0.lock().map_err(|_| poisoned())?;
        state.participant_asks.push((days, utc_offset_minutes));
        let metrics = state.participants.clone().map(|metrics| {
            agency_proto::participant_metrics::ParticipantMetrics { days, ..metrics }
        });
        metrics.ok_or_else(|| RuntimeError {
            status: 404,
            message: "the memory runtime does not report participant metrics".into(),
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

/// Writes nothing: the log of a node that is not kept.
pub struct NoJournal;

impl Journal for NoJournal {
    fn info(&self, _event: &str, _line: &str) {}
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
    /// The tokens that were asked to read the desk (a push with no alert).
    pub desk_changed: Vec<String>,
    /// How many times a wake was tried, those that failed included.
    pub desk_attempts: usize,
    pub gone: Vec<String>,
    pub failing: bool,
    /// Tokens and subscription ids whose sends fail (not gone: a push service having a bad minute); the others are taken.
    pub refusing: Vec<String>,
    /// The VAPID key of a node with Web Push; none for a node without it.
    pub web_key: Option<VapidKey>,
    /// What was sent to browser subscriptions: the subscription's id and the notice.
    pub sent_web: Vec<(String, Notice)>,
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
        if state.failing || state.refusing.contains(&token.token) {
            return Err(PushError::Failed("APNs did not answer".into()));
        }
        if state.gone.contains(&token.token) {
            return Err(PushError::Gone);
        }
        state.sent.push((token.token.clone(), notice.clone()));
        Ok(())
    }

    async fn send_desk_changed(&self, token: &PushToken) -> Result<(), PushError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| PushError::Failed("the memory notifier is poisoned".into()))?;
        state.desk_attempts += 1;
        if state.failing {
            return Err(PushError::Failed("APNs did not answer".into()));
        }
        if state.gone.contains(&token.token) {
            return Err(PushError::Gone);
        }
        state.desk_changed.push(token.token.clone());
        Ok(())
    }

    fn web_push_key(&self) -> Option<VapidKey> {
        self.0.lock().ok()?.web_key.clone()
    }

    fn web_push_check_keys(&self, p256dh: &str, auth: &str) -> Result<(), String> {
        crate::web_push::check_keys(p256dh, auth)
    }

    async fn send_web(
        &self,
        subscription: &WebSubscription,
        notice: &Notice,
    ) -> Result<(), PushError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| PushError::Failed("the memory notifier is poisoned".into()))?;
        if state.failing || state.refusing.contains(&subscription.id) {
            return Err(PushError::Failed("a push service did not answer".into()));
        }
        if state.gone.contains(&subscription.id) {
            return Err(PushError::Gone);
        }
        state
            .sent_web
            .push((subscription.id.clone(), notice.clone()));
        Ok(())
    }
}

/// Root keys in memory for tests: made from a seed and a counter, so they repeat; signing can be
/// refused (as the owner declining a presence prompt), and each signature is counted.
#[derive(Default)]
pub struct MemoryRootKeys {
    state: Mutex<MemoryRootKeysState>,
}

#[derive(Default)]
struct MemoryRootKeysState {
    /// Without a seed, keys are random.
    seed: Option<u8>,
    made: u8,
    keys: Vec<(agency_proto::Id, agency_proto::AccountKey)>,
    is_refusing: bool,
    signed: usize,
    /// Stands for a keychain: signing asks for presence, and each prompt's words are kept.
    is_asking: bool,
    prompts: Vec<String>,
    is_needing_recovery: bool,
    /// Stands for a locked keychain: it may hold the keys but says nothing for sure, and signs nothing.
    is_locked: bool,
}

impl MemoryRootKeys {
    /// Keys made from `seed` (and how many were made before), so a seeded node repeats them.
    pub fn seeded(seed: u8) -> Self {
        Self {
            state: Mutex::new(MemoryRootKeysState {
                seed: Some(seed),
                ..MemoryRootKeysState::default()
            }),
        }
    }

    /// Refuses every signature from now on, or stops refusing.
    pub fn refuse(&self, is_refusing: bool) {
        self.state.lock().unwrap().is_refusing = is_refusing;
    }

    /// Stands for a locked keychain.
    pub fn lock(&self, is_locked: bool) {
        self.state.lock().unwrap().is_locked = is_locked;
    }

    /// Stands for a keychain: keys made here cannot be copied out.
    pub fn need_recovery(&self, is_needing_recovery: bool) {
        self.state.lock().unwrap().is_needing_recovery = is_needing_recovery;
    }

    /// The secret of every root key made here, as `AccountKey::encoded` writes it: for a test that checks a
    /// secret is in no file a node writes.
    pub fn secrets(&self) -> Vec<String> {
        let state = self.state.lock().unwrap();
        state.keys.iter().map(|(_, key)| key.encoded()).collect()
    }

    /// How many signatures were made.
    pub fn signed(&self) -> usize {
        self.state.lock().unwrap().signed
    }

    /// Makes signing ask for presence, as a keychain does.
    pub fn ask_presence(&self, is_asking: bool) {
        self.state.lock().unwrap().is_asking = is_asking;
    }

    /// The words of each presence prompt so far.
    pub fn prompts(&self) -> Vec<String> {
        self.state.lock().unwrap().prompts.clone()
    }
}

impl agency_core::ports::RootKeys for MemoryRootKeys {
    fn needs_recovery(&self) -> bool {
        self.state.lock().unwrap().is_needing_recovery
    }

    fn create(&self, org: &agency_proto::Id) -> Result<agency_proto::PublicKey, String> {
        let mut state = self.state.lock().unwrap();
        state.made = state.made.wrapping_add(1);
        let key = match state.seed {
            Some(byte) => {
                let mut seed = [byte; 32];
                seed[0] = 0xb0;
                seed[31] = state.made;
                agency_proto::AccountKey::from_seed(seed)
            }
            None => agency_proto::AccountKey::generate(),
        };
        let public = key.public();
        state.keys.push((org.clone(), key));
        Ok(public)
    }

    fn holds(&self, org: &agency_proto::Id, key: &agency_proto::PublicKey) -> bool {
        let state = self.state.lock().unwrap();
        !state.is_locked
            && state
                .keys
                .iter()
                .any(|(one, kept)| one == org && &kept.public() == key)
    }

    fn may_hold(&self, org: &agency_proto::Id, key: &agency_proto::PublicKey) -> bool {
        let is_locked = self.state.lock().unwrap().is_locked;
        is_locked || self.holds(org, key)
    }

    fn sign(
        &self,
        org: &agency_proto::Id,
        key: &agency_proto::PublicKey,
        message: &[u8],
        purpose: &str,
    ) -> Result<String, String> {
        let mut state = self.state.lock().unwrap();
        if state.is_asking {
            state.prompts.push(purpose.to_owned());
        }
        if state.is_locked {
            return Err("the keychain is locked".into());
        }
        if state.is_refusing {
            return Err("the owner did not confirm".into());
        }
        let found = state
            .keys
            .iter()
            .find(|(one, kept)| one == org && &kept.public() == key);
        let signature = found
            .map(|(_, kept)| kept.sign(message))
            .ok_or_else(|| format!("no root key of {org} here"))?;
        state.signed += 1;
        Ok(signature)
    }

    fn needs_presence(&self) -> bool {
        self.state.lock().unwrap().is_asking
    }

    fn with_signer(
        &self,
        org: &agency_proto::Id,
        key: &agency_proto::PublicKey,
        purpose: &str,
        work: &mut dyn FnMut(&agency_core::ports::RootSigner) -> Result<(), String>,
    ) -> Result<(), String> {
        let secret = {
            let mut state = self.state.lock().unwrap();
            if state.is_asking {
                state.prompts.push(purpose.to_owned());
            }
            if state.is_locked {
                return Err("the keychain is locked".into());
            }
            if state.is_refusing {
                return Err("the owner did not confirm".into());
            }
            let found = state
                .keys
                .iter()
                .find(|(one, kept)| one == org && &kept.public() == key);
            let (_, kept) = found.ok_or_else(|| format!("no root key of {org} here"))?;
            agency_proto::AccountKey::from_seed(kept.seed())
        };
        let signer = |message: &[u8]| {
            self.state.lock().unwrap().signed += 1;
            Ok(secret.sign(message))
        };
        work(&signer)
    }

    fn forget(&self, org: &agency_proto::Id, key: &agency_proto::PublicKey) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        state
            .keys
            .retain(|(one, kept)| !(one == org && &kept.public() == key));
        Ok(())
    }
}
