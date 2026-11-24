//! Ports: everything a use case needs from outside the domain. Adapters implement them.
use crate::config::{FundingConfig, HostedTeam, NodeConfig};
use crate::error::Result;
use crate::invites::InviteRecord;
use crate::outbox::Parcel;
use crate::read_auth::ReadAuth;
use crate::views::Replica;
use agency_proto::client::UsageWindow;
use agency_proto::clock::OffsetDateTime;
use agency_proto::org::SignedOrg;
use agency_proto::runtime::RuntimeError;
use agency_proto::view::FederatedView;
use agency_proto::{
    Envelope, EnvelopeId, Id, JoinRequest, RuntimeDecision, Signed, TeamView, Timestamp,
};
use async_trait::async_trait;

pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
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
}

pub trait ReplicaStore {
    fn load_replica(&self, org: &Id, team: &Id) -> Option<Replica>;
    fn save_replica(&self, org: &Id, team: &Id, replica: &Replica) -> Result<()>;
}

pub trait Store:
    ConfigStore + OrgStore + InviteStore + AppliedLog + OutboxStore + ReplicaStore + Send + Sync
{
}

impl<
        T: ConfigStore
            + OrgStore
            + InviteStore
            + AppliedLog
            + OutboxStore
            + ReplicaStore
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
    async fn get_definition(
        &self,
        node: &str,
        path: &str,
        access: &DefinitionAccess,
    ) -> Result<SignedOrg, PeerError>;
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
}

/// Reads how much of a funding source has been used.
pub trait FundingSource: Send + Sync {
    /// The windows that could be read and when; nothing when the usage is unknown.
    fn usage(&self, source: &FundingConfig) -> (Vec<UsageWindow>, Option<Timestamp>);
}
