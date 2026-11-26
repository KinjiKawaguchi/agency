//! Who an org's agents may ask for what: the members, the seats, and the external parties, with what to ask
//! each about and the names they have in the teams this node runs (docs/protocol.md, "The directory for
//! agents"). A team's harness reads it from `agency directory --json` on its own node, because the org
//! definition reaches a runtime no other way. It holds labels and what to ask about only: no key, no
//! device, no account, no address, no email or phone number.
use crate::ids::Id;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The names one team of this node's runtime knows an addressee by: its keys in the team's `people`, and
/// `owner` for the team's owner. A request addressed to one of them is for the addressee.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuntimeNames {
    pub team: Id,
    pub names: Vec<String>,
}

/// A department, for reading the others.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DirectoryDepartment {
    pub id: Id,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<Id>,
}

/// A member of the org: who they are, and the names they have in this node's teams.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DirectoryMember {
    pub id: Id,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub department: Option<Id>,
    /// `member` or `answerer`.
    pub role: String,
    /// The teams of this node that address them. Empty when none does.
    pub runtime_names: Vec<RuntimeNames>,
}

/// A seat or an external party: whom to ask about what, and who answers for them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DirectoryParty {
    pub id: Id,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub department: Option<Id>,
    /// What to ask them about, in the org's own words.
    pub ask_about: Vec<String>,
    /// The members who answer for them, in order of preference. Empty for a seat: the team's owner does.
    pub answered_by: Vec<Id>,
    /// The teams of this node that address them.
    pub runtime_names: Vec<RuntimeNames>,
}

/// `agency directory --org <org> --json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Directory {
    pub org: Id,
    pub departments: Vec<DirectoryDepartment>,
    pub members: Vec<DirectoryMember>,
    pub seats: Vec<DirectoryParty>,
    pub externals: Vec<DirectoryParty>,
}
