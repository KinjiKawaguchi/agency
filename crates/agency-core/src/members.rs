//! Removing a member from an org (docs/protocol.md, "Org definition"): the root signs a version without the person, and
//! with the keys of their devices revoked, so that nothing they signed is taken any more on any node of the org. What
//! this node kept for them as a hosted member (the sign-in, the push registrations, the list of requests they were
//! told of) goes with them. A member can also be turned back into a seat in the same version, so that the requests
//! addressed to them stay addressed to someone.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::{amend, load};
use agency_proto::client::GuestRef;
use agency_proto::org::{Person, SignedOrg};
use agency_proto::seat::Seat;
use agency_proto::{Id, OrgDefinition};

/// The places in the org, and in the teams this node runs, that still name `member`: a seat or an external party it
/// answers for, a department it heads, a team it owns or that addresses it, or a seat it took, by a runtime name.
/// Removing the member while one of them names it would leave a request there addressed to no one, so each must be
/// changed first. A member who becomes a seat leaves the ids a team addresses (their own and the seats they took)
/// addressed to the seats that take them over, so a team that only addresses those ids is not named. Teams on other
/// nodes are named in those nodes' configuration, which this node does not see.
fn still_named(node: &Node, org: &OrgDefinition, member: &Id, becomes_seat: bool) -> Vec<String> {
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
        .groups()
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
        .filter(|team| {
            &team.owner == member || (!becomes_seat && team.people.values().any(is_theirs))
        })
        .map(|team| format!("team {} on this node names it", team.team));
    seats.chain(externals).chain(heads).chain(teams).collect()
}

/// On the root's node: signs the next version of `org` without `member`, with the public keys of its devices added to
/// `revoked`, and then drops what this node kept for it as a hosted member. Refused for the root's own account
/// (403), for a member the org does not have (404), and while something still names it (409, naming each place).
pub fn remove_member(node: &Node, org: &Id, member: &Id) -> Result<SignedOrg> {
    remove(node, org, member, None).map(|(doc, _)| doc)
}

/// What `remove_member_to_seat` did.
#[derive(Debug)]
pub struct ToSeat {
    pub doc: SignedOrg,
    /// The seats the member had taken that the org's kept versions no longer hold, so that they came back with the
    /// member's own label, department and what to ask about.
    pub from_the_member: Vec<Id>,
}

/// `remove_member`, and in the same version a seat of the member's id in its place, and one for each seat the member
/// took (a team may address those ids). The member's seat has their label, department and what to ask about; a seat
/// they took comes back as it was in the last version the node keeps that still had it (`from_the_member` names the
/// ones no kept version has, which take the member's). Each is answered by `answered_by` (members of the org, in order
/// of preference), or by the team's owner when that is empty. A claim link made earlier for one of those seats is
/// dropped, so that it cannot take the seat back. The requests addressed to those ids stay addressed to the seats,
/// which is why a team that addresses them no longer stops it.
pub fn remove_member_to_seat(
    node: &Node,
    org: &Id,
    member: &Id,
    answered_by: &[Id],
) -> Result<ToSeat> {
    let (doc, seats) = remove(node, org, member, Some(answered_by))?;
    Ok(ToSeat {
        doc,
        from_the_member: seats.from_the_member,
    })
}

/// `Some(answered_by)` turns the member into seats, `None` removes them.
fn remove(
    node: &Node,
    org: &Id,
    member: &Id,
    answered_by: Option<&[Id]>,
) -> Result<(SignedOrg, Replacement)> {
    let current = load(node, org)?;
    if &current.body.root.account == member {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "the root's own account cannot be removed from its org",
        ));
    }
    let Some(person) = current
        .body
        .people
        .iter()
        .find(|person| &person.id == member)
    else {
        return Err(Error::not_found(format!(
            "{member} is not a member of {org}"
        )));
    };
    let named = still_named(node, &current.body, member, answered_by.is_some());
    if !named.is_empty() {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!(
                "{member} is still named in {org}: {}; change those first",
                named.join(", ")
            ),
        ));
    }
    let replacement = match answered_by {
        Some(answered_by) => seats_for(node, &current.body, person, answered_by)?,
        None => Replacement::default(),
    };
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
        body.seats.extend(replacement.seats.iter().cloned());
        Ok(())
    })?;
    forget_hosted(node, org, member)?;
    for seat in &replacement.seats {
        node.store().remove_claim_for_seat(org, &seat.id)?;
    }
    Ok((doc, replacement))
}

/// The seats that stand in a removed member's place, and which of them took the member's own data.
#[derive(Default)]
struct Replacement {
    seats: Vec<Seat>,
    from_the_member: Vec<Id>,
}

/// The seat `id` as the last version this node keeps that still had it held it, walking back from the version
/// before the one that took it (and so removed it).
fn seat_as_it_was(node: &Node, org: &OrgDefinition, id: &Id) -> Option<Seat> {
    (1..=org.version)
        .rev()
        .filter_map(|version| node.store().load_org_version(&org.org, version))
        .find_map(|doc| doc.body.seats.into_iter().find(|seat| &seat.id == id))
}

/// The seats that stand in `person`'s place: one of their id, with their label, department and what to ask about,
/// and one of each seat they took, as it was (or, when no kept version has it, with the member's data), answered by
/// `answered_by`, who must be other members of the org.
fn seats_for(
    node: &Node,
    org: &OrgDefinition,
    person: &Person,
    answered_by: &[Id],
) -> Result<Replacement> {
    for relay in answered_by {
        let is_other_member = relay != &person.id && org.people.iter().any(|one| &one.id == relay);
        if !is_other_member {
            return Err(Error::bad_request(format!(
                "{relay} is not another member of {}, so it cannot answer for the seat",
                org.org
            )));
        }
    }
    let of_the_member = |id: &Id| Seat {
        id: id.clone(),
        label: person.label.clone(),
        department: person.department.clone(),
        unit: None,
        ask_about: person.ask_about.clone(),
        relays: answered_by.to_vec(),
    };
    let mut replacement = Replacement {
        seats: vec![of_the_member(&person.id)],
        from_the_member: vec![],
    };
    for id in &person.formerly {
        match seat_as_it_was(node, org, id) {
            Some(was) => replacement.seats.push(Seat {
                // A department the org no longer has cannot be named by a seat.
                department: was
                    .department
                    .filter(|one| org.groups().any(|department| &department.id == one)),
                relays: answered_by.to_vec(),
                ..was
            }),
            None => {
                replacement.seats.push(of_the_member(id));
                replacement.from_the_member.push(id.clone());
            }
        }
    }
    Ok(replacement)
}

/// Drops the sign-in records, the list of requests told of and the push registrations this node kept for `member` of
/// `org` as a guest. The records admit no one once the member is gone (`guests::admit`); dropping them keeps nothing
/// of the person that is of no use.
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
    node.store().remove_guest_notified(&whose)?;
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
