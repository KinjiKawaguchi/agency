//! An org's seats and external parties (docs/glossary.md, "Seat", "External party"): the root states them,
//! changes what is given, and removes them, each as the next version of the org definition, like everything in
//! it. The rules (an id used once, a department of the org, members to answer for them) are the definition's
//! own, checked when the version is signed, so a refusal names the id that broke one.
use crate::error::{Error, ErrorKind, Result};
use crate::guests::{self, Guest};
use crate::invites::take_seat;
use crate::node::Node;
use crate::orgs::{amend, amend_with};
use agency_proto::org::{Device, SignedOrg};
use agency_proto::seat::{External, Seat};
use agency_proto::{Id, OrgDefinition, Person};

/// What to change in a seat. `None` leaves a field as it is; `Some(None)` clears the department.
#[derive(Clone, Debug, Default)]
pub struct SeatChange {
    pub label: Option<String>,
    pub department: Option<Option<Id>>,
    pub ask_about: Option<Vec<String>>,
    pub relays: Option<Vec<Id>>,
}

/// What to change in an external party. `None` leaves a field as it is.
#[derive(Clone, Debug, Default)]
pub struct ExternalChange {
    pub label: Option<String>,
    pub ask_about: Option<Vec<String>>,
    pub relays: Option<Vec<Id>>,
}

fn not_found(what: &str, id: &Id, org: &Id) -> Error {
    Error::not_found(format!("no {what} {id} in org {org}"))
}

/// The id is free among the org's members, seats and external parties.
fn check_free(body: &OrgDefinition, id: &Id) -> Result<()> {
    let is_taken = body
        .people
        .iter()
        .any(|one| &one.id == id || one.formerly.contains(id))
        || body.seats.iter().any(|one| &one.id == id)
        || body.externals.iter().any(|one| &one.id == id);
    if is_taken {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!("{id} is already the id of a member, a seat, or an external party"),
        ));
    }
    Ok(())
}

pub fn add_seat(node: &Node, org: &Id, seat: Seat) -> Result<SignedOrg> {
    amend(node, org, |body| {
        check_free(body, &seat.id)?;
        body.seats.push(seat);
        Ok(())
    })
}

/// Changes only what `change` gives.
pub fn change_seat(node: &Node, org: &Id, id: &Id, change: SeatChange) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let seat = body
            .seats
            .iter_mut()
            .find(|one| &one.id == id)
            .ok_or_else(|| not_found("seat", id, org))?;
        if let Some(label) = change.label {
            seat.label = label;
        }
        if let Some(department) = change.department {
            seat.department = department;
        }
        if let Some(ask_about) = change.ask_about {
            seat.ask_about = ask_about;
        }
        if let Some(relays) = change.relays {
            seat.relays = relays;
        }
        Ok(())
    })
}

/// A member takes a seat that is in the org already, for example someone who joined before the seat's claim
/// link was made: the seat is gone, its id is the member's `formerly`, and what to ask about it is added to
/// what to ask the member about (a text once; refused when that is more than the limit allows). Their
/// department stays theirs.
pub fn merge_seat(node: &Node, org: &Id, seat: &Id, into: &Id) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let taken = take_seat(body, seat).map_err(|_| not_found("seat", seat, org))?;
        let member = body
            .people
            .iter_mut()
            .find(|one| &one.id == into)
            .ok_or_else(|| not_found("member", into, org))?;
        member.formerly.push(taken.id);
        for text in taken.ask_about {
            if !member.ask_about.contains(&text) {
                member.ask_about.push(text);
            }
        }
        Ok(())
    })
}

/// The longest a sign-in email may be (RFC 5321).
const MOST_EMAIL_CHARACTERS: usize = 254;

/// The email, lowercased, when it can be a sign-in address: text with one `@` that has text on both sides, no control
/// character, at most `MOST_EMAIL_CHARACTERS` characters.
pub(crate) fn check_email(email: &str) -> Result<String> {
    let email = email.trim().to_lowercase();
    let is_shaped = email.chars().count() <= MOST_EMAIL_CHARACTERS
        && !email.chars().any(char::is_control)
        && email.split_once('@').is_some_and(|(name, host)| {
            !name.is_empty() && !host.is_empty() && !host.contains('@')
        });
    if is_shaped {
        return Ok(email);
    }
    Err(Error::bad_request(
        "the sign-in address must be text with one @ and something on both sides",
    ))
}

/// Refuses an email that cannot be this seat's guest: the owner's own sign-in (the owner is never a guest), or
/// one that already is the guest of another member.
pub(crate) fn check_email_is_free(node: &Node, email: &str, org: &Id, seat: &Id) -> Result<()> {
    let owner = node
        .config()
        .access
        .map(|access| access.owner_email.trim().to_lowercase());
    if owner.as_deref() == Some(email) {
        return Err(Error::new(
            ErrorKind::Conflict,
            "that address is the node owner's sign-in, and the owner is never a guest",
        ));
    }
    match guests::admit(node, email) {
        Some(guest) if guest.org != *org || guest.member != *seat => Err(Error::new(
            ErrorKind::Conflict,
            "that address is already the sign-in of another member this node hosts",
        )),
        _ => Ok(()),
    }
}

/// Makes `seat` of `body` a member this node hosts, with the seat's id, label, department and what to ask it about,
/// and `device` as its only device. Not found when the seat is not in the org.
pub(crate) fn host_in(body: &mut OrgDefinition, org: &Id, seat: &Id, device: Device) -> Result<()> {
    let taken = take_seat(body, seat).map_err(|_| not_found("seat", seat, org))?;
    let member = Person::hosted(taken.id, taken.label, vec![device])
        .with_department(taken.department)
        .with_ask_about(taken.ask_about);
    body.people.push(member);
    Ok(())
}

/// A seat becomes a member this node hosts, for a person with no node of their own (docs/glossary.md, "Hosted
/// member"): one version removes `seats[seat]` and adds the member with **the same id**, so requests, `people`
/// maps of a team's node and history stay attached; it has the seat's label, department and what to ask about, its
/// member id as its account, no account key, and `device` as its only device. Those who answered for the seat
/// (`relays`) no longer do: the request is the member's own. The record that makes `email` the member's sign-in
/// is kept once the root has signed and before the version is published, so a refusal leaves nothing behind.
///
/// Refused: not the root's node (403); no such seat (404); a device whose id is not the one its key has, a key the
/// definition already has, or one that is not a device key (400, from the definition's own checks); an address that
/// is no sign-in address (400), is the owner's, or is another member's (409); an org one of whose nodes does not
/// list `hostedMembers` (409, as for any version that needs a capability).
pub fn host_seat(
    node: &Node,
    org: &Id,
    seat: &Id,
    device: Device,
    email: &str,
) -> Result<SignedOrg> {
    let email = check_email(email)?;
    check_email_is_free(node, &email, org, seat)?;
    if device.id != device.public_key.device_id() {
        return Err(Error::bad_request(
            "a device's id is d- and the pairing code of its key",
        ));
    }
    amend_with(
        node,
        org,
        |body| host_in(body, org, seat, device),
        |_| guests::keep(node, &Guest::new(&email, org.clone(), seat.clone())),
    )
}

pub fn remove_seat(node: &Node, org: &Id, id: &Id) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let before = body.seats.len();
        body.seats.retain(|one| &one.id != id);
        if body.seats.len() == before {
            return Err(not_found("seat", id, org));
        }
        Ok(())
    })
}

pub fn add_external(node: &Node, org: &Id, external: External) -> Result<SignedOrg> {
    amend(node, org, |body| {
        check_free(body, &external.id)?;
        body.externals.push(external);
        Ok(())
    })
}

/// Changes only what `change` gives.
pub fn change_external(
    node: &Node,
    org: &Id,
    id: &Id,
    change: ExternalChange,
) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let external = body
            .externals
            .iter_mut()
            .find(|one| &one.id == id)
            .ok_or_else(|| not_found("external party", id, org))?;
        if let Some(label) = change.label {
            external.label = label;
        }
        if let Some(ask_about) = change.ask_about {
            external.ask_about = ask_about;
        }
        if let Some(relays) = change.relays {
            external.relays = relays;
        }
        Ok(())
    })
}

pub fn remove_external(node: &Node, org: &Id, id: &Id) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let before = body.externals.len();
        body.externals.retain(|one| &one.id != id);
        if body.externals.len() == before {
            return Err(not_found("external party", id, org));
        }
        Ok(())
    })
}
