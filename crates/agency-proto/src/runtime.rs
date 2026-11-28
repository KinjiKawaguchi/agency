//! agency-runtime/1: how a node talks to the runtime of a team it hosts.
//!
//! The node runs the team's configured command once per call, writes one `RuntimeRequest` as JSON
//! to its stdin, and reads one `RuntimeResponse` as JSON from its stdout. See "agency-runtime/1"
//! in docs/protocol.md.
use crate::envelope::{EnvelopeKind, Payload};
use crate::ids::Id;
use crate::prioritize::PriorityChoice;
use crate::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const RUNTIME_VERSION: &str = "agency-runtime/1";

/// Names a team runtime gives meaning of its own: `owner` is the team's owner (see "agency-runtime/1"), and
/// `board` and `unverified` are participants of the reference runtime that are no org member. The node passes
/// a member's id to the runtime as `by`, so no member, seat, or external party may newly take one of these
/// as its id: a runtime would read a decision by it as one by that name.
pub const RESERVED_NAMES: [&str; 3] = ["owner", "board", "unverified"];

/// Whether `id` is one of `RESERVED_NAMES`.
pub fn is_reserved_name(id: &Id) -> bool {
    RESERVED_NAMES.contains(&id.as_str())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeRequest {
    /// Always `agency-runtime/1` (`RUNTIME_VERSION`).
    pub runtime: String,
    pub org: Id,
    pub team: Id,
    #[serde(flatten)]
    #[ts(flatten)]
    pub call: RuntimeCall,
}

impl RuntimeRequest {
    pub fn new(org: Id, team: Id, call: RuntimeCall) -> Self {
        Self {
            runtime: RUNTIME_VERSION.to_owned(),
            org,
            team,
            call,
        }
    }
}

/// The operation and its input. `describe`, `view`, and `decide` are required; `tokenUsage`, `activity`,
/// `participantMetrics`, `tickets`, and `prioritize` are optional, and the node calls one only when `Describe::ops` lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "op", rename_all = "camelCase")]
#[ts(export)]
pub enum RuntimeCall {
    /// Answered with `Describe`.
    Describe,
    /// Answered with `TeamView`.
    View,
    /// Answered with `Decided`.
    Decide { decision: RuntimeDecision },
    /// Answered with `TokenUsage` over the last `days` days (1 to 90).
    TokenUsage { days: u32 },
    /// Answered with `Activity`: at most `limit` (1 to 500) events after `since`, oldest first.
    Activity { since: Timestamp, limit: u32 },
    /// Answered with `ParticipantMetrics` over the last `days` days (1 to 90), cut at `utcOffsetMinutes`
    /// (minutes east of UTC) when the node gives one, and at the runtime's own time zone when it does not.
    ParticipantMetrics {
        days: u32,
        #[serde(
            default,
            rename = "utcOffsetMinutes",
            skip_serializing_if = "Option::is_none"
        )]
        #[ts(optional)]
        utc_offset_minutes: Option<i32>,
    },
    /// Answered with `Tickets`: the epics the team answers for with every child, and its tickets in no epic.
    Tickets,
    /// Answered with `Decided`: sets the board's Priority of the ticket `target` names (a ticket's
    /// `prioritizeWith`), or clears it. `by` and `byParticipant` are as in `RuntimeDecision`. A priority
    /// never goes through `Decide`.
    Prioritize {
        target: String,
        priority: PriorityChoice,
        by: Id,
        #[serde(
            default,
            rename = "byParticipant",
            skip_serializing_if = "Option::is_none"
        )]
        #[ts(optional)]
        by_participant: Option<String>,
    },
}

/// The optional operation name for `RuntimeCall::TokenUsage` in `Describe::ops`.
pub const TOKEN_USAGE_OP: &str = "tokenUsage";
/// The optional operation name for `RuntimeCall::Activity` in `Describe::ops`.
pub const ACTIVITY_OP: &str = "activity";
/// The optional operation name for `RuntimeCall::ParticipantMetrics` in `Describe::ops`.
pub const PARTICIPANT_METRICS_OP: &str = "participantMetrics";
/// The optional operation name for `RuntimeCall::Tickets` in `Describe::ops`.
pub const TICKETS_OP: &str = "tickets";

/// A verified decision, applied by the team runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeDecision {
    pub kind: EnvelopeKind,
    /// A request id (`answer`) or a work item's item reference (`judge`).
    pub target: String,
    /// The org member who decided.
    pub by: Id,
    /// The decider as a participant of this team: their runtime name, in the same name space as a
    /// request's `to` (their key in the hosted team's `people`, or `owner`). The node applies a decision
    /// only from the request's addressee, and sends this so the runtime can check it too without
    /// knowing org member ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub by_participant: Option<String>,
    /// The id of the seat or external party the request was addressed to, when `by` answered for it (a
    /// member in its `relays`). Absent when the request was `by`'s own. A runtime that does not know it
    /// ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub on_behalf_of: Option<String>,
    pub payload: Payload,
}

/// What a runtime supports. The node calls `describe` first and stops unless `runtimes`
/// contains the version it speaks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Describe {
    pub runtimes: Vec<String>,
    /// Operation names. Unknown names are ignored, so a newer runtime still describes itself.
    pub ops: Vec<String>,
    pub label: String,
    /// The harness release that runs the team, as a short text. Optional: a runtime without it is read as
    /// before. Read it through `release`, which leaves out a value beyond `MOST_RELEASE_CHARS`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub version: Option<String>,
}

/// The most characters of a runtime's `version` that a node takes.
pub const MOST_RELEASE_CHARS: usize = 64;

impl Describe {
    /// The harness release the runtime reported, or `None` when it reported none or a text that is empty,
    /// longer than `MOST_RELEASE_CHARS` characters, or holds a control character or one that reorders or hides
    /// how a line reads (`is_hidden_char`).
    pub fn release(&self) -> Option<&str> {
        self.version.as_deref().filter(|text| {
            !text.is_empty()
                && text.chars().count() <= MOST_RELEASE_CHARS
                && !text.chars().any(crate::org::is_hidden_char)
        })
    }

    pub fn speaks(&self, version: &str) -> bool {
        self.runtimes.iter().any(|one| one == version)
    }

    pub fn supports(&self, op: &str) -> bool {
        self.ops.iter().any(|one| one == op)
    }
}

/// The result of `decide`: an empty object. Extra fields are ignored.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Decided {}

/// `{"result": ...}` on success, `{"error": {...}}` on failure.
/// The exit code is 0 with `result` and 1 with `error`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RuntimeResponse<T> {
    Result(T),
    Error(RuntimeError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeError {
    /// An HTTP status: 400 (malformed), 404 (no such target), 503 (cannot run now).
    pub status: u16,
    pub message: String,
}

impl RuntimeError {
    /// 400 and 404 are refusals: the same decision will never succeed, so it is not retried.
    pub fn is_refusal(&self) -> bool {
        matches!(self.status, 400 | 404)
    }
}
