//! Ports: everything a use case needs from outside the domain. Adapters implement them.
use crate::config::{FundingConfig, HostedTeam, NodeConfig};
use crate::decisions::DecisionRecord;
use crate::delivery::DeliveryFacts;
use crate::devices::DeviceRequestRecord;
use crate::error::Result;
use crate::invites::InviteRecord;
use crate::outbox::Parcel;
use crate::push::{Notice, PushError};
use crate::read_auth::ReadAuth;
use crate::views::Replica;
use agency_proto::activity::Activity;
use agency_proto::client::{PushToken, UsageWindow};
use agency_proto::clock::OffsetDateTime;
use agency_proto::evaluation::Evaluation;
use agency_proto::org::{SignedOrg, Versions};
use agency_proto::runtime::RuntimeError;
use agency_proto::view::FederatedView;
use agency_proto::{
    DeviceChange, Envelope, EnvelopeId, Id, JoinRequest, RuntimeDecision, Signed, TeamView,
    Timestamp, TokenUsage,
};
use async_trait::async_trait;

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

pub trait OrgStore {
    fn load_org(&self, org: &Id) -> Option<SignedOrg>;
    /// Replaces the current definition and keeps the previous versions as history.
    fn save_org(&self, doc: &SignedOrg) -> Result<()>;
    fn orgs(&self) -> Vec<SignedOrg>;
    /// One version of an org's definition, from the history this node kept.
    fn load_org_version(&self, org: &Id, version: u64) -> Option<SignedOrg>;
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
}

pub trait ReplicaStore {
    fn load_replica(&self, org: &Id, team: &Id) -> Option<Replica>;
    fn save_replica(&self, org: &Id, team: &Id, replica: &Replica) -> Result<()>;
    /// Removes a team's replica. A replica that is not there is not an error.
    fn delete_replica(&self, org: &Id, team: &Id) -> Result<()>;
}

pub trait Store:
    ConfigStore
    + OrgStore
    + InviteStore
    + AppliedLog
    + OutboxStore
    + ReplicaStore
    + DecisionLogStore
    + DeviceRequestStore
    + PushStore
    + Send
    + Sync
{
}

impl<
        T: ConfigStore
            + OrgStore
            + InviteStore
            + AppliedLog
            + OutboxStore
            + ReplicaStore
            + DecisionLogStore
            + DeviceRequestStore
            + PushStore
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
    /// A team's delivery evaluation, from the node it runs on.
    async fn get_evaluation(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Evaluation, PeerError>;
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
    /// Sends a device change to the root's node, which answers with the version it wrote.
    async fn post_device_change(
        &self,
        node: &str,
        path: &str,
        change: &Signed<DeviceChange>,
    ) -> Result<SignedOrg, PeerError>;
    async fn post_join(
        &self,
        node: &str,
        path: &str,
        request: &Signed<JoinRequest>,
    ) -> Result<SignedOrg, PeerError>;
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

/// Sends a notification to one of the owner's devices (APNs). Without a configured key, it sends nothing.
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn send(&self, token: &PushToken, notice: &Notice) -> Result<(), PushError>;
}
