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
    /// The runtime's name of the participant the message is from.
    pub from: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    /// The org member who decided, when the message carries a decision: their member id, the one the
    /// node passes to the runtime as `decision.by`, not a participant name. Absent on a message the
    /// team wrote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<Id>,
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

/// What a team's node answers to a member's signed read of the team: the view, and the reader's name
/// in the team runtime (absent when the team does not address the reader).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct FederatedView {
    pub view: TeamView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer: Option<String>,
}

/// How much a TeamView may hold. A view beyond them is not served or kept: another node chooses what it
/// sends, and a runtime's view is served to other nodes. Lengths are in characters.
pub mod limits {
    /// Ids, labels, names, refs, and the other one-line names.
    pub const NAME: usize = 200;
    pub const TITLE: usize = 200;
    pub const CONTEXT: usize = 4_000;
    pub const DOCUMENT: usize = 40_000;
    /// A person's status line, an option's text, and a forecast's basis.
    pub const LINE: usize = 500;
    /// An option's detail and a thread message.
    pub const TEXT: usize = 4_000;
    pub const PEOPLE: usize = 200;
    pub const REQUESTS: usize = 200;
    pub const WORK: usize = 5_000;
    pub const EPICS: usize = 500;
    pub const OPTIONS: usize = 20;
    pub const THREAD: usize = 200;
    /// A person's `holding`, a request's `blocks`, and an epic's `blocked`.
    pub const LIST: usize = 100;
}

fn within(what: &str, text: &str, limit: usize) -> Result<(), String> {
    if text.chars().count() > limit {
        return Err(format!("{what} is longer than {limit} characters"));
    }
    Ok(())
}

fn at_most<T>(what: &str, items: &[T], limit: usize) -> Result<(), String> {
    if items.len() > limit {
        return Err(format!("{what} has more than {limit} entries"));
    }
    Ok(())
}

fn names<'a>(what: &str, names: impl IntoIterator<Item = &'a String>) -> Result<(), String> {
    names
        .into_iter()
        .try_for_each(|name| within(what, name, limits::NAME))
}

impl PersonStatus {
    fn check_limits(&self) -> Result<(), String> {
        names("a person's name", [&self.id, &self.label])?;
        within("a person's status", &self.status, limits::LINE)?;
        at_most("a person's holding", &self.holding, limits::LIST)?;
        names("a held item", &self.holding)?;
        match &self.waiting_on {
            Some(WaitingOn::Person { person }) => within("waitingOn", person, limits::NAME),
            _ => Ok(()),
        }
    }
}

impl Request {
    fn check_limits(&self) -> Result<(), String> {
        let optional = [&self.to, &self.issue].into_iter().flatten();
        names(
            "a request's name",
            [&self.id, &self.from, &self.answer_with.target],
        )?;
        names("a request's name", optional)?;
        within("a request's title", &self.title, limits::TITLE)?;
        within("a request's context", &self.context, limits::CONTEXT)?;
        if let Some(document) = &self.document {
            within("a request's document", document, limits::DOCUMENT)?;
        }
        at_most("a request's blocks", &self.blocks, limits::LIST)?;
        names("a blocked item", &self.blocks)?;
        at_most("a request's options", &self.options, limits::OPTIONS)?;
        for option in &self.options {
            within("an option's key", &option.key, limits::NAME)?;
            within("an option's text", &option.text, limits::LINE)?;
            if let Some(detail) = &option.detail {
                within("an option's detail", detail, limits::TEXT)?;
            }
        }
        let thread = self.thread.as_deref().unwrap_or_default();
        at_most("a request's thread", thread, limits::THREAD)?;
        for message in thread {
            names(
                "a message's name",
                [&message.from].into_iter().chain(&message.choice),
            )?;
            within("a message", &message.text, limits::TEXT)?;
        }
        Ok(())
    }
}

impl WorkItem {
    fn check_limits(&self) -> Result<(), String> {
        let optional = [&self.assignee, &self.epic].into_iter().flatten();
        names(
            "a work item's name",
            [&self.reference].into_iter().chain(optional),
        )?;
        within("a work item's title", &self.title, limits::TITLE)?;
        match &self.eta {
            Some(eta) => within("an estimate's basis", &eta.basis, limits::LINE),
            None => Ok(()),
        }
    }
}

impl EpicForecast {
    fn check_limits(&self) -> Result<(), String> {
        within("an epic's ref", &self.reference, limits::NAME)?;
        within("an epic's title", &self.title, limits::TITLE)?;
        within("an epic's basis", &self.basis, limits::LINE)?;
        at_most("an epic's blocked", &self.blocked, limits::LIST)
    }
}

impl TeamView {
    /// Checks the view against `limits`, and says what is over.
    pub fn check_limits(&self) -> Result<(), String> {
        within("the team's label", &self.label, limits::NAME)?;
        at_most("people", &self.people, limits::PEOPLE)?;
        at_most("requests", &self.requests, limits::REQUESTS)?;
        at_most("work", &self.work, limits::WORK)?;
        at_most("epics", &self.epics, limits::EPICS)?;
        self.people
            .iter()
            .try_for_each(PersonStatus::check_limits)?;
        self.requests.iter().try_for_each(Request::check_limits)?;
        self.work.iter().try_for_each(WorkItem::check_limits)?;
        self.epics.iter().try_for_each(EpicForecast::check_limits)
    }
}
