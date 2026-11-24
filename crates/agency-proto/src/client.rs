//! The Client API between a client (web, iOS, desktop) and its home node (`/api`). See "Client API" in
//! docs/protocol.md. Every `Option` is omitted from JSON when absent.
use crate::envelope::EnvelopeKind;
use crate::ids::Id;
use crate::timestamp::Timestamp;
use crate::view::TeamView;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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

/// A decision from the client. `kind` and `target` are the request's `answerWith`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct DecisionInput {
    pub kind: EnvelopeKind,
    pub target: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
}

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
