//! Ports: everything a use case needs from outside the domain. Adapters implement them.
use crate::clients_seen::ClientsSeen;
use crate::config::{FundingConfig, HostedTeam, NodeConfig};
use crate::decisions::DecisionRecord;
use crate::delivery::DeliveryFacts;
use crate::devices::DeviceRequestRecord;
use crate::error::Result;
use crate::invites::InviteRecord;
use crate::outbox::Parcel;
use crate::peer_about::PeerAbout;
use crate::push::{Notice, PushError};
use crate::read_auth::ReadAuth;
use crate::views::Replica;
use crate::web_push::WebSubscription;
use agency_proto::about::About;
use agency_proto::activity::Activity;
use agency_proto::client::{PushToken, UsageWindow, VapidKey};
use agency_proto::clock::OffsetDateTime;
use agency_proto::effort::Effort;
use agency_proto::evaluation::Evaluation;
use agency_proto::observation::Observation;
use agency_proto::org::{SignedOrg, Versions};
use agency_proto::participant_metrics::ParticipantMetrics;
use agency_proto::root_change::RootAnswer;
use agency_proto::runtime::RuntimeError;
use agency_proto::view::FederatedView;
use agency_proto::{
    DeviceChange, Envelope, EnvelopeId, Id, JoinRequest, PublicKey, RuntimeDecision, Signed,
    TeamView, Timestamp, TokenUsage,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;

    /// The node's time zone, as the offset from UTC now. The days of the delivery reports are its
    /// calendar days. UTC unless the clock knows better.
    fn utc_offset(&self) -> agency_proto::clock::UtcOffset {
        agency_proto::clock::UtcOffset::UTC
    }
}

pub trait Random: Send + Sync {
    /// 16 bytes from a cryptographically secure source.
    fn bytes16(&self) -> [u8; 16];
}

/// Serializes the writers of org definitions, invites, and applied envelopes (one writer each).
#[async_trait]
pub trait WriteLock: Send + Sync {
    /// Held until the returned guard is dropped.
    async fn lock(&self) -> Box<dyn Send + '_>;
}

pub trait ConfigStore {
    fn load_config(&self) -> Result<NodeConfig>;
    fn save_config(&self, config: &NodeConfig) -> Result<()>;
}

/// The root keys of the orgs this node is root of, kept apart from its account key: the account key
/// signs automated things (reads, join requests), a root key only org definition versions, which a
/// person makes. An adapter reads a secret only to sign, may ask for the owner's presence first, and
/// never lets it out. An org created before root keys were kept apart uses the account key as its root
/// until it is rotated (`orgs::rotate_root`).
pub trait RootKeys: Send + Sync {
    /// Makes a new root key for `org`, keeps it, and answers its public key.
    fn create(&self, org: &Id) -> std::result::Result<PublicKey, String>;
    /// Whether this node keeps the secret of `key` as a root key of `org`. Never asks for presence.
    fn holds(&self, org: &Id, key: &PublicKey) -> bool;
    /// Whether the key may be here: `holds`, or a store that cannot say right now (a locked keychain). Only
    /// for knowing that this node is the root; anything that acts on the key being here uses `holds`.
    fn may_hold(&self, org: &Id, key: &PublicKey) -> bool {
        self.holds(org, key)
    }
    /// Signs `message` with `key`, a root key of `org` kept here. `purpose` says what it signs, for a
    /// presence prompt. Refused when the owner does not confirm, or the key is not kept here.
    fn sign(
        &self,
        org: &Id,
        key: &PublicKey,
        message: &[u8],
        purpose: &str,
    ) -> std::result::Result<String, String>;
    /// Forgets a root key of `org` that no version will be signed with again.
    fn forget(&self, org: &Id, key: &PublicKey) -> std::result::Result<(), String>;
    /// Whether signing asks for the owner's presence (a keychain). A root's node then never signs because
    /// someone else asked: it holds the change for `agency root-sign`.
    fn needs_presence(&self) -> bool {
        false
    }
    /// Asks for presence once, saying `purpose`, then hands `work` a signer for that one run: several
    /// versions in a row, one press. The secret is held in memory for the run only.
    fn with_signer(
        &self,
        org: &Id,
        key: &PublicKey,
        purpose: &str,
        work: &mut dyn FnMut(&RootSigner) -> std::result::Result<(), String>,
    ) -> std::result::Result<(), String> {
        let signer = |message: &[u8]| self.sign(org, key, message, purpose);
        work(&signer)
    }
    /// Whether a key made here cannot be copied out (a keychain), so that an org must have a recovery key
    /// before its root moves here: without one, losing this machine would lose the org.
    fn needs_recovery(&self) -> bool {
        false
    }
}

/// Signs one message with a root key, within `RootKeys::with_signer`.
pub type RootSigner<'a> = dyn Fn(&[u8]) -> std::result::Result<String, String> + 'a;

/// Root changes held for the owner, by their handle, and the joins this node asked for that wait for
/// another org's owner (`pending_root`).
pub trait PendingRootStore {
    fn put_pending_root(&self, change: &crate::pending_root::PendingRootChange) -> Result<()>;
    fn pending_roots(&self) -> Vec<crate::pending_root::PendingRootChange>;
    fn remove_pending_root(&self, id: &str) -> Result<()>;
    fn put_waiting_join(&self, join: &crate::pending_root::WaitingJoin) -> Result<()>;
    fn waiting_joins(&self) -> Vec<crate::pending_root::WaitingJoin>;
    fn remove_waiting_join(&self, org: &Id) -> Result<()>;
}

pub trait OrgStore {
    fn load_org(&self, org: &Id) -> Option<SignedOrg>;
    /// Replaces the current definition and keeps the previous versions as history.
    fn save_org(&self, doc: &SignedOrg) -> Result<()>;
    fn orgs(&self) -> Vec<SignedOrg>;
    /// One version of an org's definition, from the history this node kept.
    fn load_org_version(&self, org: &Id, version: u64) -> Option<SignedOrg>;
    /// Notes when this node took a version that starts a recovery, by its own clock.
    fn note_recovery_start(&self, org: &Id, version: u64, at: &str) -> Result<()>;
    /// When this node took the version `version` of `org` that starts a recovery, if it did.
    fn recovery_started(&self, org: &Id, version: u64) -> Option<String>;
}

/// Invites, keyed by the SHA-256 of their code. The code itself is never stored.
pub trait InviteStore {
    fn load_invite(&self, digest: &str) -> Option<InviteRecord>;
    fn save_invite(&self, digest: &str, record: &InviteRecord) -> Result<()>;
}

/// The envelopes already applied, for replay protection.
pub trait AppliedLog {
    fn is_applied(&self, id: &EnvelopeId) -> bool;
    fn record_applied(&self, id: &EnvelopeId, at: &str) -> Result<()>;
}

pub trait OutboxStore {
    fn put_parcel(&self, parcel: &Parcel) -> Result<()>;
    fn pending_parcels(&self) -> Vec<Parcel>;
    fn remove_parcel(&self, id: &str) -> Result<()>;
    /// Moves a parcel the receiver refused out of the outbox, keeping it for people to read.
    fn refuse_parcel(&self, parcel: &Parcel) -> Result<()>;
    /// Parcels the receiver refused.
    fn refused_parcels(&self) -> Vec<Parcel>;
}

/// The decisions this node's account made, oldest first in storage.
pub trait DecisionLogStore {
    fn append_decision(&self, record: &DecisionRecord) -> Result<()>;
    /// At most `limit` records, newest first.
    fn recent_decisions(&self, limit: usize) -> Vec<DecisionRecord>;
}

/// Devices waiting to be added, newest last.
pub trait DeviceRequestStore {
    fn device_requests(&self) -> Vec<DeviceRequestRecord>;
    fn save_device_requests(&self, requests: &[DeviceRequestRecord]) -> Result<()>;
}

/// The owner's registered APNs device tokens, and the requests already notified.
pub trait PushStore {
    fn push_tokens(&self) -> Vec<PushToken>;
    fn save_push_tokens(&self, tokens: &[PushToken]) -> Result<()>;
    /// Keys (`org/team/request`) of requests already notified, oldest first. `None` until the first pass.
    fn notified(&self) -> Option<Vec<String>>;
    fn save_notified(&self, keys: &[String]) -> Result<()>;
    /// What the iPhones were last woken for: the requests that waited for the owner then, and when. `None`
    /// until the first pass has recorded it.
    fn desk_refresh(&self) -> Option<crate::push::DeskRefresh>;
    fn save_desk_refresh(&self, state: &crate::push::DeskRefresh) -> Result<()>;
    /// The destinations that failed to take a notice and are to be sent it again (docs/protocol.md, "Push notifications"),
    /// oldest first.
    fn push_retries(&self) -> Vec<crate::push_retries::Retry>;
    fn save_push_retries(&self, retries: &[crate::push_retries::Retry]) -> Result<()>;
    /// The owner's registered Web Push subscriptions, oldest first.
    fn web_subscriptions(&self) -> Vec<WebSubscription>;
    fn save_web_subscriptions(&self, subscriptions: &[WebSubscription]) -> Result<()>;
}

/// The numbers of the outcome analysis: business data, kept only on this node, appended and never
/// rewritten (see "Outcomes" in docs/protocol.md).
pub trait OutcomeStore {
    /// Every observation of the org, oldest first (in the order they were appended).
    fn observations(&self, org: &Id) -> Vec<Observation>;
    /// Appends one observation. The caller holds the node's write lock.
    fn append_observation(&self, org: &Id, observation: &Observation) -> Result<()>;
}

/// When the node first saw an epic that a team's plan aims at a KPI finished. A team's view keeps a finished
/// epic only for a limited time, and a reading looks 90 days past the ship day, so the node keeps the day
/// itself (see "Readings" in docs/protocol.md).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShipRecord {
    pub team: Id,
    /// The epic's item reference.
    pub epic: String,
    pub shipped_at: Timestamp,
}

/// The ship days of an org's epics, on this node only, appended and never rewritten.
pub trait ShipLedger {
    /// Every record of the org, in the order they were appended. An epic can have more than one when two
    /// passes saw it at once: the first is the one that counts.
    fn ships(&self, org: &Id) -> Vec<ShipRecord>;
    /// Appends one record.
    fn append_ship(&self, org: &Id, record: &ShipRecord) -> Result<()>;
}

/// What this node has read of the other nodes of an org, by the node's origin (`peer_about`).
pub trait AboutStore {
    fn peer_abouts(&self, org: &Id) -> BTreeMap<String, PeerAbout>;
    fn save_peer_abouts(&self, org: &Id, abouts: &BTreeMap<String, PeerAbout>) -> Result<()>;
}

/// The accounts this node has taken a read from that names it (`read_auth`), kept in the data directory so that a
/// restart does not forget them: once an account's node signs reads that way, this node takes no read that
/// names no node from it.
pub trait BoundReaderStore {
    /// The accounts: none when nothing was ever kept, and an error when something is kept that cannot be read,
    /// which is not the same and must never be written over.
    fn bound_readers(&self) -> Result<BTreeSet<Id>>;
    /// Keeps exactly these accounts.
    fn save_bound_readers(&self, accounts: &BTreeSet<Id>) -> Result<()>;
}

/// The guests of this node, by sign-in email (`guests`), kept in its data directory.
pub trait GuestStore {
    fn guest(&self, email: &str) -> Option<crate::guests::Guest>;
    fn put_guest(&self, guest: &crate::guests::Guest) -> Result<()>;
    fn remove_guest(&self, email: &str) -> Result<()>;
    /// The keys of the requests a guest was told of, like `notified` for the owner; `None` before its first pass.
    fn guest_notified(&self, guest: &agency_proto::client::GuestRef) -> Option<Vec<String>>;
    fn save_guest_notified(
        &self,
        guest: &agency_proto::client::GuestRef,
        keys: &[String],
    ) -> Result<()>;
}

/// The app releases the node has seen (`clients_seen`), kept in its data directory.
pub trait ClientsSeenStore {
    fn clients_seen(&self) -> ClientsSeen;
    fn save_clients_seen(&self, seen: &ClientsSeen) -> Result<()>;
}

pub trait ReplicaStore {
    fn load_replica(&self, org: &Id, team: &Id) -> Option<Replica>;
    fn save_replica(&self, org: &Id, team: &Id, replica: &Replica) -> Result<()>;
    /// Removes a team's replica. A replica that is not there is not an error.
    fn delete_replica(&self, org: &Id, team: &Id) -> Result<()>;
}

/// The origins this node has been configured with (`url`), so that a team's place that names any of them is this
/// node, however the node was reached when a version of the org was made.
pub trait OwnOriginStore {
    /// As `agency_proto::client::home_node_origin` writes them, the oldest first.
    fn own_origins(&self) -> Vec<String>;
    fn save_own_origins(&self, origins: &[String]) -> Result<()>;
}

pub trait Store:
    ConfigStore
    + OrgStore
    + PendingRootStore
    + InviteStore
    + AppliedLog
    + OutboxStore
    + ReplicaStore
    + DecisionLogStore
    + DeviceRequestStore
    + PushStore
    + OutcomeStore
    + ShipLedger
    + AboutStore
    + BoundReaderStore
    + ClientsSeenStore
    + OwnOriginStore
    + GuestStore
    + Send
    + Sync
{
}

impl<
        T: ConfigStore
            + OrgStore
            + PendingRootStore
            + InviteStore
            + AppliedLog
            + OutboxStore
            + ReplicaStore
            + DecisionLogStore
            + DeviceRequestStore
            + PushStore
            + OutcomeStore
            + ShipLedger
            + AboutStore
            + BoundReaderStore
            + ClientsSeenStore
            + OwnOriginStore
            + GuestStore
            + Send
            + Sync,
    > Store for T
{
}

/// Why another node did not accept a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeerError {
    /// The node answered with this HTTP status.
    Status { status: u16, message: String },
    /// The node could not be reached, or its answer could not be read.
    Unreachable(String),
}

impl std::fmt::Display for PeerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status { status, message } => write!(f, "{message} ({status})"),
            Self::Unreachable(message) => f.write_str(message),
        }
    }
}

/// How a request for an org definition proves it may read it.
#[derive(Clone, Debug)]
pub enum DefinitionAccess {
    Signed(ReadAuth),
    InviteCode(String),
}

/// Other nodes, over the federation routes (`/fed/v1`). `node` is the other node's origin.
#[async_trait]
pub trait Peers: Send + Sync {
    async fn get_view(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<FederatedView, PeerError>;
    async fn get_usage(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<TokenUsage, PeerError>;
    /// The events of a team's activity after a time, from the node it runs on (`routes::activity`).
    async fn get_activity(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Activity, PeerError>;
    /// A team's delivery evaluation, from the node it runs on.
    async fn get_evaluation(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Evaluation, PeerError>;
    /// A team's effort by work type, from the node it runs on.
    async fn get_effort(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Effort, PeerError>;
    /// What another node says about itself (`routes::about`), by a member's signed read.
    async fn get_about(&self, node: &str, path: &str, auth: &ReadAuth) -> Result<About, PeerError>;
    async fn get_definition(
        &self,
        node: &str,
        path: &str,
        access: &DefinitionAccess,
    ) -> Result<SignedOrg, PeerError>;
    /// The versions of an org's definition after one, by a signed read of `path` (`routes::versions`).
    async fn get_versions(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Versions, PeerError>;
    async fn post_envelope(
        &self,
        node: &str,
        path: &str,
        envelope: &Signed<Envelope>,
    ) -> Result<(), PeerError>;
    async fn post_definition(
        &self,
        node: &str,
        path: &str,
        doc: &SignedOrg,
    ) -> Result<(), PeerError>;
    /// Sends a device change to the root's node, which answers with the version it wrote, or holds it.
    async fn post_device_change(
        &self,
        node: &str,
        path: &str,
        change: &Signed<DeviceChange>,
    ) -> Result<RootAnswer, PeerError>;
    async fn post_join(
        &self,
        node: &str,
        path: &str,
        request: &Signed<JoinRequest>,
    ) -> Result<RootAnswer, PeerError>;
    /// Asks the root's node for a join it holds, by its handle: still pending, or the version that signed it.
    async fn get_join(&self, node: &str, path: &str) -> Result<RootAnswer, PeerError>;
}

/// The runtimes of the teams this node hosts (agency-runtime/1).
#[async_trait]
pub trait TeamRuntime: Send + Sync {
    async fn view(&self, team: &HostedTeam) -> Result<TeamView, RuntimeError>;
    async fn decide(
        &self,
        team: &HostedTeam,
        decision: RuntimeDecision,
    ) -> Result<(), RuntimeError>;
    /// The team's token usage over the last `days` days. A runtime that does not report usage
    /// answers 404.
    async fn token_usage(&self, team: &HostedTeam, days: u32) -> Result<TokenUsage, RuntimeError>;
    /// The team's participants over the last `days` days, with the days cut at `utc_offset_minutes` (minutes
    /// east of UTC). A runtime that does not report them answers 404, which is what a runtime that does not
    /// implement this answers.
    async fn participant_metrics(
        &self,
        _team: &HostedTeam,
        _days: u32,
        _utc_offset_minutes: i32,
    ) -> Result<ParticipantMetrics, RuntimeError> {
        Err(RuntimeError {
            status: 404,
            message: "the team runtime does not report participant metrics".into(),
        })
    }

    /// At most `limit` events of the team's activity after `since`, oldest first. A runtime that does not
    /// report activity answers 404.
    async fn activity(
        &self,
        team: &HostedTeam,
        since: &Timestamp,
        limit: u32,
    ) -> Result<Activity, RuntimeError>;
}

/// Why a team's delivery facts could not be read. The text is plain, and holds no secret and no address:
/// it goes to clients.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryError(pub String);

/// Reads the facts a team's delivery numbers are worked out from (GitHub, through the machine's `gh`).
#[async_trait]
pub trait DeliverySource: Send + Sync {
    /// The pull requests of `repos` merged since `since`, and the runs of CI on their default branches
    /// from about then on, enough to tell what state each branch was in at `since`.
    async fn facts(
        &self,
        repos: &[String],
        since: OffsetDateTime,
    ) -> Result<DeliveryFacts, DeliveryError>;
}

/// Reads how much of a funding source has been used.
pub trait FundingSource: Send + Sync {
    /// The windows that could be read and when; nothing when the usage is unknown.
    fn usage(&self, source: &FundingConfig) -> (Vec<UsageWindow>, Option<Timestamp>);
}

/// The node's log. A caller passes a finished sentence, never a token, an endpoint, a request body or a query.
pub trait Journal: Send + Sync {
    fn info(&self, event: &str, line: &str);
    /// A line for the owner to act on; a journal that has no levels writes it as `info`.
    fn warn(&self, event: &str, line: &str) {
        self.info(event, line);
    }
}

/// Sends a notification to one of the owner's devices: an iPhone through APNs, a browser through its push
/// service. Without a configured key, it sends nothing.
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn send(&self, token: &PushToken, notice: &Notice) -> Result<(), PushError>;

    /// The node's VAPID key when Web Push is set up; a node without it has no browser subscriptions.
    fn web_push_key(&self) -> Option<VapidKey> {
        None
    }

    /// Wakes the app on one device so that it reads the owner's desk: a push with no alert and nothing of the
    /// desk in it. A notifier that cannot (no key, or a browser's service) does nothing.
    async fn send_desk_changed(&self, _token: &PushToken) -> Result<(), PushError> {
        Ok(())
    }

    /// Whether the keys of a browser's subscription can be sent to; the reason when they cannot.
    fn web_push_check_keys(&self, _p256dh: &str, _auth: &str) -> Result<(), String> {
        Ok(())
    }

    /// Sends a notification to each of some browsers' subscriptions, the answers in the order of the subscriptions. The
    /// default sends one after another; an adapter that can send to several at once (a push service that is slow must
    /// not hold the others, nor the iPhone's notice) does.
    async fn send_web_many(
        &self,
        subscriptions: &[WebSubscription],
        notice: &Notice,
    ) -> Vec<Result<(), PushError>> {
        let mut answers = Vec::with_capacity(subscriptions.len());
        for subscription in subscriptions {
            answers.push(self.send_web(subscription, notice).await);
        }
        answers
    }

    /// Sends a notification to a browser's subscription.
    async fn send_web(
        &self,
        _subscription: &WebSubscription,
        _notice: &Notice,
    ) -> Result<(), PushError> {
        Err(PushError::Failed("Web Push is not set up".to_owned()))
    }
}
