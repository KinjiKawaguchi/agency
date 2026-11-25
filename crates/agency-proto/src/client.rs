//! The Client API between a client (web, iOS, desktop) and its home node (`/api`). See "Client API" in
//! docs/protocol.md. Every `Option` is omitted from JSON when absent.
use crate::envelope::{Envelope, EnvelopeKind};
use crate::ids::Id;
use crate::keys::PublicKey;
use crate::org::Device;
use crate::signed::Signed;
use crate::timestamp::Timestamp;
use crate::view::TeamView;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a client needs before it writes: the write token, and the node's clock, which an envelope's `at`
/// must agree with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Session {
    /// The value every write sends in `x-agency-token`. It changes each time the node starts.
    pub write_token: String,
    /// The node's time when it answered.
    pub now: Timestamp,
}

/// The node's account and everything it can reach.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Me {
    pub account: AccountRef,
    pub orgs: Vec<OrgRef>,
    pub funding: Vec<Funding>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccountRef {
    pub id: Id,
    pub label: String,
    /// The account key's fingerprint.
    pub fingerprint: String,
}

/// An org the account belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgRef {
    pub id: Id,
    pub label: String,
    /// Whether this node holds the org's root key.
    pub is_root: bool,
    pub teams: Vec<TeamRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct TeamRef {
    pub id: Id,
    pub label: String,
    pub location: TeamLocation,
    /// The viewer's name in the team runtime. Absent when the team does not address the viewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TeamLocation {
    /// The team runs on this node.
    Local,
    /// The team runs on another node; this node keeps a replica.
    Remote,
}

/// A team's view as the client needs it: the view, who the viewer is in it, and how fresh it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct TeamSnapshot {
    pub view: TeamView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer: Option<String>,
    /// The ids of the requests that are the viewer's to decide, by the rule in "Requests"
    /// (docs/protocol.md). Clients use this instead of working it out from `to`.
    pub mine: Vec<String>,
    /// The `id`s of the team's people whose `waitingOn` is the viewer (see "People" in docs/protocol.md).
    pub waiting_on_viewer: Vec<String>,
    pub source: SnapshotSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum SnapshotSource {
    /// Read from the team runtime just now.
    Local,
    /// The last replica this node read from the team's node. `error` is the last failed read, if the
    /// most recent read failed; the view is then older than `triedAt`.
    #[serde(rename_all = "camelCase")]
    Replica {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        fetched_at: Option<Timestamp>,
        tried_at: Timestamp,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        error: Option<String>,
    },
}

/// The longest `title` and `choiceLabel`, in characters.
pub const MAX_NOTE: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DecisionResult {
    pub outcome: DecisionOutcome,
    /// Why a decision was queued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DecisionOutcome {
    /// Applied to a team on this node.
    Applied,
    /// Delivered to the team's node, which applied it.
    Delivered,
    /// The team's node could not be reached; the signed decision waits in this node's outbox.
    Queued,
}

/// A source of funding of the account, such as a subscription.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct Funding {
    pub id: Id,
    pub label: String,
    pub kind: FundingKind,
    /// Usage windows that could be read. Empty when the usage is unknown.
    pub windows: Vec<UsageWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<Timestamp>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FundingKind {
    Subscription,
    ApiKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageWindow {
    pub name: UsageWindowName,
    /// The window's length: 5 for `fiveHour`, 168 for `sevenDay`.
    pub length_hours: u32,
    /// Percent of the window used, rounded to an integer.
    pub utilization: u32,
    pub resets_at: Timestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UsageWindowName {
    FiveHour,
    SevenDay,
}

impl UsageWindowName {
    pub fn length_hours(self) -> u32 {
        match self {
            Self::FiveHour => 5,
            Self::SevenDay => 7 * 24,
        }
    }
}

/// The decisions the node's account made recently, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DecisionLog {
    pub entries: Vec<DecisionEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DecisionEntry {
    /// The decision envelope's id.
    pub id: String,
    pub org: Id,
    pub team: Id,
    pub kind: EnvelopeKind,
    /// The request id or work item ref (`repo#N`) it answered.
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice_label: Option<String>,
    pub decided_at: Timestamp,
    pub state: DecisionState,
}

/// Where a decision went.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum DecisionState {
    /// Applied to a team on this node.
    Applied,
    /// Delivered to the team's node, which applied it.
    Delivered,
    /// Waiting in the outbox; the team's node has not been reached yet.
    #[serde(rename_all = "camelCase")]
    Queued {
        attempts: u32,
        next_at: Timestamp,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        last_error: Option<String>,
    },
    /// The team's node refused it; it will not be retried.
    Refused { message: String },
}

/// A decision signed by one of the account's devices, with notes for the decision log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct SignedDecisionInput {
    pub envelope: Signed<Envelope>,
    /// The request's title, kept in the decision log for people to read (at most 200 characters).
    /// Never signed or sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The label of the chosen option, kept in the decision log like `title`. Only with a choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice_label: Option<String>,
}

/// A new device asking to be added. One of the account's devices (or, for the first device, the node's
/// terminal) adds it after the person compares the pairing codes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DeviceRequestInput {
    pub label: String,
    pub public_key: PublicKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceRequest {
    pub label: String,
    pub public_key: PublicKey,
    /// The id the device gets when it is added: `d-` and the pairing code without dashes.
    pub device_id: Id,
    pub pairing_code: String,
    pub requested_at: Timestamp,
}

/// The account's devices in each org it belongs to, and the devices waiting to be added.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceList {
    pub orgs: Vec<OrgDevices>,
    pub requests: Vec<DeviceRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgDevices {
    pub org: Id,
    pub devices: Vec<Device>,
}

/// Where a device change went.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DeviceChangeResult {
    pub outcome: DeviceChangeOutcome,
    /// Why a change was queued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DeviceChangeOutcome {
    /// This node holds the org's root key and wrote the next version.
    Applied,
    /// The root's node wrote the next version; this node has it.
    Delivered,
    /// The root's node could not be reached; the change waits in the outbox.
    Queued,
}

/// Which APNs host a device token belongs to: `sandbox` for development builds, `production` for
/// TestFlight and App Store builds. The app says which; the node never guesses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PushEnvironment {
    Production,
    Sandbox,
}

/// An APNs device token to register for the owner's notifications.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PushTokenInput {
    /// The device token as hex.
    pub token: String,
    pub environment: PushEnvironment,
    pub label: String,
}

/// A registered device token. In a list, `token` holds only its last 8 characters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PushToken {
    pub token: String,
    pub environment: PushEnvironment,
    pub label: String,
    pub registered_at: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PushTokenList {
    pub tokens: Vec<PushToken>,
}
