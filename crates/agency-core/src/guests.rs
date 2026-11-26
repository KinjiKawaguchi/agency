//! Guests: people who sign in to this node through Access as a member it hosts (docs/glossary.md, "Hosted
//! member"). The root's node keeps which sign-in email is which hosted member, here and never in the org's
//! definition, and admits such a sign-in as that member only while the definition still names it hosted.
use crate::error::Result;
use crate::node::Node;
use agency_proto::Id;
use serde::{Deserialize, Serialize};

/// A sign-in email that is a hosted member of an org on this node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guest {
    /// Lowercased, and compared exactly.
    pub email: String,
    pub org: Id,
    pub member: Id,
}

impl Guest {
    pub fn new(email: &str, org: Id, member: Id) -> Self {
        Self {
            email: email.trim().to_lowercase(),
            org,
            member,
        }
    }
}

/// Keeps `guest`, replacing any record for the same email.
pub fn keep(node: &Node, guest: &Guest) -> Result<()> {
    node.store().put_guest(guest)
}

/// The guest a verified Access email is, if it is one: a record for it, whose member the org's definition on
/// this node still lists as hosted. A record whose member is gone or is no longer hosted admits no one.
pub fn admit(node: &Node, email: &str) -> Option<Guest> {
    let guest = node.store().guest(&email.trim().to_lowercase())?;
    let doc = node.store().load_org(&guest.org)?;
    let member = doc.body.member(&guest.member)?;
    member.is_hosted().then_some(guest)
}

/// Who a Client API request acts as: the node's own account (its owner, or the machine's user), or a member
/// this node hosts (a guest), whose reach the adapter has already limited to its own org.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caller {
    Owner,
    Guest(Guest),
}

impl Caller {
    /// The member id the caller acts as.
    pub fn account(&self, node: &Node) -> Id {
        match self {
            Self::Owner => node.account(),
            Self::Guest(guest) => guest.member.clone(),
        }
    }

    /// Whether the caller may act in `org` at all: the owner in any org its account belongs to, a guest only
    /// in its own.
    pub fn is_in(&self, org: &Id) -> bool {
        match self {
            Self::Owner => true,
            Self::Guest(guest) => &guest.org == org,
        }
    }
}
