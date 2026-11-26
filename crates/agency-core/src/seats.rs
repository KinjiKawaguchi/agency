//! An org's seats and external parties (docs/glossary.md, "Seat", "External party"): the root states them,
//! changes what is given, and removes them, each as the next version of the org definition, like everything in
//! it. The rules (an id used once, a department of the org, members to answer for them) are the definition's
//! own, checked when the version is signed, so a refusal names the id that broke one.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::amend;
use agency_proto::org::SignedOrg;
use agency_proto::seat::{External, Seat};
use agency_proto::{Id, OrgDefinition};

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
    let is_taken = body.people.iter().any(|one| &one.id == id)
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
