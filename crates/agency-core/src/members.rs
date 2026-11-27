//! Removing a member from an org (docs/protocol.md, "Org definition"): the root signs a version without the person, and
//! with the keys of their devices revoked, so that nothing they signed is taken any more on any node of the org. What
//! this node kept for them as a hosted member (the sign-in, the push registrations) goes with them.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::{amend, load};
use agency_proto::client::GuestRef;
use agency_proto::org::SignedOrg;
use agency_proto::{Id, OrgDefinition};

/// The places in the org, and in the teams this node runs, that still name `member`: a seat or an external party it
/// answers for, a department it heads, a team it owns or that addresses it, or a seat it took, by a runtime name.
/// Removing the member while one of them names it would leave a request there addressed to no one, so each must be
/// changed first. Teams on other nodes are named in those nodes' configuration, which this node does not see.
fn still_named(node: &Node, org: &OrgDefinition, member: &Id) -> Vec<String> {
    let seats = org
        .seats
        .iter()
        .filter(|seat| seat.relays.contains(member))
        .map(|seat| format!("it answers for seat {}", seat.id));
    let externals = org
        .externals
        .iter()
        .filter(|external| external.relays.contains(member))
        .map(|external| format!("it answers for external party {}", external.id));
    let heads = org
        .departments
        .iter()
        .filter(|department| department.head.as_ref() == Some(member))
        .map(|department| format!("it heads department {}", department.id));
    // A seat the member took is answered by the member (`formerly`), so a team that addresses that id names it too.
    let taken: Vec<&Id> = org
        .people
        .iter()
        .filter(|person| &person.id == member)
        .flat_map(|person| &person.formerly)
        .collect();
    let is_theirs = |id: &Id| id == member || taken.contains(&id);
    let hosted = node.config().teams;
    let teams = hosted
        .iter()
        .filter(|team| team.org == org.org)
        .filter(|team| &team.owner == member || team.people.values().any(is_theirs))
        .map(|team| format!("team {} on this node names it", team.team));
    seats.chain(externals).chain(heads).chain(teams).collect()
}

/// On the root's node: signs the next version of `org` without `member`, with the public keys of its devices added to
/// `revoked`, and then drops what this node kept for it as a hosted member. Refused for the root's own account
/// (403), for a member the org does not have (404), and while something still names it (409, naming each place).
pub fn remove_member(node: &Node, org: &Id, member: &Id) -> Result<SignedOrg> {
    let current = load(node, org)?;
    if &current.body.root.account == member {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "the root's own account cannot be removed from its org",
        ));
    }
    if !current
        .body
        .people
        .iter()
        .any(|person| &person.id == member)
    {
        return Err(Error::not_found(format!(
            "{member} is not a member of {org}"
        )));
    }
    let named = still_named(node, &current.body, member);
    if !named.is_empty() {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!(
                "{member} is still named in {org}: {}; change those first",
                named.join(", ")
            ),
        ));
    }
    let doc = amend(node, org, |body| {
        let keys: Vec<_> = body
            .people
            .iter()
            .filter(|person| &person.id == member)
            .flat_map(|person| {
                person
                    .devices
                    .iter()
                    .map(|device| device.public_key.clone())
            })
            .filter(|key| !body.revoked.contains(key))
            .collect();
        body.people = body
            .people
            .iter()
            .filter(|person| &person.id != member)
            .cloned()
            .collect();
        body.revoked = body.revoked.iter().cloned().chain(keys).collect();
        Ok(())
    })?;
    forget_hosted(node, org, member)?;
    Ok(doc)
}

/// Drops the sign-in records and the push registrations this node kept for `member` of `org` as a guest. The records
/// admit no one once the member is gone (`guests::admit`); dropping them keeps nothing of the person that is of no use.
fn forget_hosted(node: &Node, org: &Id, member: &Id) -> Result<()> {
    for guest in node.store().guests() {
        if &guest.org == org && &guest.member == member {
            node.store().remove_guest(&guest.email)?;
        }
    }
    let whose = GuestRef {
        member: member.clone(),
        org: org.clone(),
    };
    let tokens: Vec<String> = node
        .store()
        .push_tokens()
        .into_iter()
        .filter(|one| one.guest.as_ref() == Some(&whose))
        .map(|one| one.token)
        .collect();
    crate::push::remove_tokens(node, &tokens)?;
    let browsers: Vec<String> = node
        .store()
        .web_subscriptions()
        .into_iter()
        .filter(|one| one.guest.as_ref() == Some(&whose))
        .map(|one| one.id)
        .collect();
    crate::web_push::remove_subscriptions(node, &browsers)
}
