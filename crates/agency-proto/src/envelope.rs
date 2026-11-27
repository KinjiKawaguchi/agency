//! Decision envelopes. Signed with one of the decider's device keys; the team's node checks them before applying.
use crate::clock;
use crate::ids::{EnvelopeId, Id};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_TEXT: usize = 4000;
/// How long after it was made an envelope is still accepted (long enough for a sleeping node to wake up and deliver it).
pub const MAX_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum EnvelopeKind {
    /// Answers a request (`target` is the request id).
    Answer,
    /// Decides a board item waiting for a decision (`target` is the item's reference).
    Judge,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct Payload {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Envelope {
    pub id: EnvelopeId,
    pub kind: EnvelopeKind,
    pub org: Id,
    pub team: Id,
    pub target: String,
    pub payload: Payload,
    pub account: Id,
    pub at: String,
}

/// An envelope's target, a request id or an item reference: 1 to 120 bytes of ASCII letters, digits, and
/// `._#-`. A view allows an item reference of 200 characters, so a pending decision's `answerWith.target`
/// is held to this stricter rule (docs/protocol.md, "Item references"), and the iPhone app checks the same.
fn is_target(raw: &str) -> bool {
    (1..=120).contains(&raw.len())
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._#-".contains(&b))
}

impl Envelope {
    /// Checks the shape (not the signature). Called at the node boundary, before applying.
    pub fn validate(&self) -> Result<(), String> {
        if !is_target(&self.target) {
            return Err("malformed target".into());
        }
        if self.payload.text.chars().count() > MAX_TEXT {
            return Err(format!("the answer text is at most {MAX_TEXT} characters"));
        }
        if let Some(choice) = &self.payload.choice {
            if choice.is_empty() || choice.chars().count() > 20 {
                return Err("malformed choice".into());
            }
        }
        if self.payload.text.trim().is_empty() && self.payload.choice.is_none() {
            return Err("a choice or an answer text is required".into());
        }
        clock::parse(&self.at)
            .map(|_| ())
            .ok_or_else(|| "malformed time".into())
    }

    /// Whether it is older than it can be accepted: more than 7 days old. One that is too far ahead is not expired
    /// (it may become fresh), and a time that does not parse is not either.
    pub fn is_expired(&self, now: time::OffsetDateTime) -> bool {
        clock::parse(&self.at).is_some_and(|at| (now - at).whole_seconds() > MAX_AGE_SECONDS)
    }

    /// Whether it is within its acceptance period: at most 7 days old, and at most 5 minutes ahead (clock skew).
    pub fn is_fresh(&self, now: time::OffsetDateTime) -> bool {
        is_fresh_at(&self.at, now)
    }
}

/// Whether a signed time is at most 7 days old and at most 5 minutes ahead (clock skew).
pub(crate) fn is_fresh_at(at: &str, now: time::OffsetDateTime) -> bool {
    clock::parse(at).is_some_and(|at| {
        let age = (now - at).whole_seconds();
        (-300..=MAX_AGE_SECONDS).contains(&age)
    })
}
