//! Root signatures that someone else asked for (a join, a device change), held by a root's node whose root
//! key needs the owner's presence until the owner signs them together (docs/protocol.md, "Pending root
//! changes").
use crate::ids::Id;
use crate::org::SignedOrg;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How long a held change waits for the owner before it is dropped.
pub const PENDING_ROOT_DAYS: i64 = 7;

/// The answer `202 Accepted` carries: the handle of the held change, and what to tell the person who asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PendingRootAnswer {
    /// 32 random bytes as unpadded base64url. For a join, the joiner fetches the signed version with it.
    pub pending: String,
    pub message: String,
}

/// What the root's node answered to a join or a device change.
#[derive(Clone, Debug, PartialEq)]
pub enum RootAnswer {
    /// It signed the next version at once.
    Signed(Box<SignedOrg>),
    /// It holds the change for the owner to sign.
    Pending(PendingRootAnswer),
}

/// A held change, as the owner's desk shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RootChangeRef {
    pub id: String,
    pub org: Id,
    /// What it changes, in words: "add member carol (Carol)", "add device iPhone for bob".
    pub what: String,
    pub received_at: String,
}

/// What the owner is told while a change waits for the root key's node.
pub fn waiting_message() -> String {
    "Approved here; it takes effect once the root key's node signs it: run `agency root-sign` there.".into()
}
