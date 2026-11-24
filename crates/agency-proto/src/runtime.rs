//! agency-runtime/1: how a node talks to the runtime of a team it hosts.
//!
//! The node runs the team's configured command once per call, writes one `RuntimeRequest` as JSON
//! to its stdin, and reads one `RuntimeResponse` as JSON from its stdout. See "agency-runtime/1"
//! in docs/protocol.md.
use crate::envelope::{EnvelopeKind, Payload};
use crate::ids::Id;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const RUNTIME_VERSION: &str = "agency-runtime/1";

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
        Self { runtime: RUNTIME_VERSION.to_owned(), org, team, call }
    }
}

/// The operation and its input. `usage` is reserved for a later version.
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
}

/// A verified decision, applied by the team runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeDecision {
    pub kind: EnvelopeKind,
    /// A request id (`answer`) or a work item ref `repo#N` (`judge`).
    pub target: String,
    /// The org person who decided.
    pub by: Id,
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
}

impl Describe {
    pub fn speaks(&self, version: &str) -> bool {
        self.runtimes.iter().any(|one| one == version)
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
