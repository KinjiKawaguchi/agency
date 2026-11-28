//! Guests: people who sign in to this node through Access as a member it hosts (docs/glossary.md, "Hosted
//! member"). The root's node keeps which sign-in email is which hosted member, here and never in the org's
//! definition, and admits such a sign-in as that member only while the definition still names it hosted.
use crate::error::Result;
use crate::node::Node;
use agency_proto::client::{GuestRef, PushEvent};
use agency_proto::Id;
use serde::{Deserialize, Serialize};

/// Normalizes an email the node matches against an Access identity.
pub fn check_email(raw: &str) -> crate::Result<String> {
    let email = raw.trim().to_lowercase();
    let parts: Vec<_> = email.split('@').collect();
    if email.chars().count() > 254
        || email.chars().any(char::is_control)
        || parts.len() != 2
        || parts.iter().any(|part| part.is_empty())
    {
        return Err(crate::Error::bad_request("email must have one @, text on both sides, at most 254 characters, and no control character"));
    }
    Ok(email)
}

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
    // Only the root's node hosts members.
    if !crate::orgs::is_root(node, &doc) {
        return None;
    }
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

    /// Whose its push destinations are: none for the owner, the guest's member and org for a guest.
    pub fn guest_of(&self) -> Option<GuestRef> {
        match self {
            Self::Owner => None,
            Self::Guest(guest) => Some(GuestRef {
                member: guest.member.clone(),
                org: guest.org.clone(),
            }),
        }
    }

    /// The kinds of notice the caller's destination gets of those it asked for: a guest gets only requests,
    /// since devices, decisions and root changes are the owner's.
    pub(crate) fn events(&self, asked: Vec<PushEvent>) -> Vec<PushEvent> {
        match self {
            Self::Owner => asked,
            Self::Guest(_) => asked
                .into_iter()
                .filter(|event| *event == PushEvent::Request)
                .collect(),
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
