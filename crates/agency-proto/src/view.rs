//! TeamView: what a team runtime reports about its team, and what clients render.
//!
//! The node reads it from the team runtime (`view` in agency-runtime/1), keeps replicas of
//! other nodes' views, and serves it to clients. See "TeamView" in docs/protocol.md.
//! Every `Option` is omitted from JSON when absent; it is never written as `null`.
use crate::envelope::EnvelopeKind;
use crate::ids::Id;
use crate::timestamp::{Date, Timestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeamView {
    pub org: Id,
    pub team: Id,
    pub label: String,
    pub generated_at: Timestamp,
    pub people: Vec<PersonStatus>,
    pub requests: Vec<Request>,
    pub work: Vec<WorkItem>,
    pub epics: Vec<EpicForecast>,
}

/// One agent or human on the team. `id` is the team runtime's own name for them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct PersonStatus {
    pub id: String,
    pub label: String,
    pub kind: PersonKind,
    pub role: PersonRole,
    pub state: PersonState,
    /// When `state` last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<Timestamp>,
    /// Who can unblock them. Absent when nobody needs to act.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<WaitingOn>,
    /// One line for people to read, for example "Waiting on the owner: agency#3".
    pub status: String,
    /// Work item refs (`repo#N`) assigned to them.
    pub holding: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PersonKind {
    Agent,
    Human,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PersonRole {
    Lead,
    Member,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PersonState {
    Working,
    Idle,
    Away,
    Unknown,
}

/// Who has to act before this person can continue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum WaitingOn {
    /// A person, by the team runtime's own name (the same namespace as `Request::to`).
    Person { person: String },
    /// The team's lead.
    Lead,
}

/// Something the team asks a person to decide or answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct Request {
    pub id: String,
    pub kind: RequestKind,
    pub title: String,
    /// Plain text: the background needed to decide.
    pub context: String,
    /// Markdown body of a proposal. Clients render a safe Markdown subset and never raw HTML.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
    /// Who on the team raised it, by the team runtime's own name.
    pub from: String,
    /// The addressee, by the team runtime's own name. Absent means the team's owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub created_at: Timestamp,
    /// The related work item (`repo#N`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// People and work item refs that wait for the answer.
    pub blocks: Vec<String>,
    pub options: Vec<RequestOption>,
    /// Earlier messages, oldest first. Absent when the request has never been answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<Vec<ThreadMessage>>,
    /// The decision envelope that answers this request.
    pub answer_with: AnswerWith,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RequestKind {
    Question,
    Proposal,
    /// A board item waiting for a decision.
    PendingDecision,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct RequestOption {
    pub key: String,
    pub text: String,
    pub is_recommended: bool,
    /// What choosing it changes (cost, people, scope). Plain text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct ThreadMessage {
    pub from: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    pub at: Timestamp,
}

/// The `kind` and `target` a decision envelope carries, so clients never derive them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnswerWith {
    pub kind: EnvelopeKind,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct WorkItem {
    /// `repo#N`
    #[serde(rename = "ref")]
    #[ts(rename = "ref")]
    pub reference: String,
    pub title: String,
    pub status: WorkStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Blocked>,
    /// The parent epic's ref (`repo#N`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta: Option<Eta>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorkStatus {
    Backlog,
    Next,
    InProgress,
    Release,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Blocked {
    pub reason: BlockedReason,
    pub since: Date,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BlockedReason {
    NeedsDecision,
    External,
    WaitingOnPrevious,
    TimeOrEquipment,
}

/// When a work item is expected to finish, from past items with the same points.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Eta {
    /// Half of past items finished by this time.
    pub p50: Timestamp,
    /// Four in five past items finished by this time.
    pub p80: Timestamp,
    /// What the forecast is based on, for people to read.
    pub basis: String,
    pub is_over_budget: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct EpicForecast {
    /// `repo#N`
    #[serde(rename = "ref")]
    #[ts(rename = "ref")]
    pub reference: String,
    pub title: String,
    /// Child work items: all, finished, and in progress.
    pub total: u32,
    pub done: u32,
    pub doing: u32,
    pub remaining_points: u32,
    /// Blocked children left out of the forecast, counted by reason.
    pub blocked: Vec<BlockedCount>,
    /// Absent when the team has no recent throughput to forecast from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forecast: Option<Forecast>,
    pub basis: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlockedCount {
    pub reason: BlockedReason,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Forecast {
    pub days: u32,
    pub at: Timestamp,
}
