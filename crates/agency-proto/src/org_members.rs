//! What an org's owner reads of its people in one place (docs/protocol.md, "Members, devices, and open
//! seats"): the members with their roles and devices, the open seats, the external parties, and the changes
//! that wait for the root key. It holds labels, ids and key kinds only: no key, no account, no sign-in
//! address, and no email or phone number. Changing any of it stays the root's signed versions.
use crate::ids::Id;
use crate::org::Role;
use crate::root_change::RootChangeRef;
use crate::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The kind of key a device holds. A client shows it so that an owner can tell a phone's key from a
/// browser's (a browser's is P-256).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KeyKind {
    Ed25519,
    P256,
}

/// A key held by one of a member's clients.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemberDevice {
    pub id: Id,
    pub label: String,
    pub key_kind: KeyKind,
    pub added_at: Timestamp,
}

/// A member of the org.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct OrgMember {
    pub id: Id,
    pub label: String,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub department: Option<Id>,
    /// Whether the root's node hosts the member (a guest): it has no node of its own.
    pub is_hosted: bool,
    /// For a hosted member only: whether a sign-in is set for it on this node. Never the address. Left out
    /// for a member with a node of its own, and on a node that does not host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_sign_in: Option<bool>,
    pub devices: Vec<MemberDevice>,
}

/// A seat that no one has taken, or an external party.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct OrgParty {
    pub id: Id,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub department: Option<Id>,
    /// What to ask them about, in the org's own words.
    pub ask_about: Vec<String>,
    /// The members who answer for them, in order of preference. Empty for a seat that names none: the team's
    /// owner answers.
    pub answered_by: Vec<Id>,
}

/// `GET /api/orgs/<org>/members`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct OrgMembers {
    pub org: Id,
    pub members: Vec<OrgMember>,
    /// The seats no one has taken yet.
    pub seats: Vec<OrgParty>,
    pub externals: Vec<OrgParty>,
    /// The changes that wait for the root key's node to sign them. Present only on that node, and left out
    /// when none waits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<RootChangeRef>>", optional)]
    pub held: Vec<RootChangeRef>,
}
