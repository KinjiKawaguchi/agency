//! What the owner reads of an org's people in one place (docs/protocol.md, "Members, devices, and open
//! seats"): from the org's definition, with whether a hosted member has a sign-in on this node and the changes
//! that wait for the root key. Nothing is changed here; people change by the root's signed versions.
use crate::client::membership;
use crate::error::{Error, ErrorKind, Result};
use crate::guests::Guest;
use crate::node::Node;
use crate::pending_root;
use agency_proto::org::{Device, Role};
use agency_proto::org_members::{KeyKind, MemberDevice, OrgMember, OrgMembers, OrgParty};
use agency_proto::{Id, OrgDefinition, Person};

fn device(device: &Device) -> MemberDevice {
    MemberDevice {
        id: device.id.clone(),
        label: device.label.clone(),
        key_kind: if device.public_key.is_ed25519() {
            KeyKind::Ed25519
        } else {
            KeyKind::P256
        },
        added_at: device.added_at.clone(),
    }
}

/// A member as the owner reads it. A guest's address is in the guest records and goes no further than
/// whether one is set.
fn member(person: &Person, guests: &[Guest], org: &Id) -> OrgMember {
    let has_sign_in = person.is_hosted().then(|| {
        guests
            .iter()
            .any(|guest| &guest.org == org && guest.member == person.id)
    });
    OrgMember {
        id: person.id.clone(),
        label: person.label.clone(),
        role: person.role,
        department: person.department.clone(),
        is_hosted: person.is_hosted(),
        has_sign_in,
        devices: person.devices.iter().map(device).collect(),
    }
}

fn seats(body: &OrgDefinition) -> Vec<OrgParty> {
    body.seats
        .iter()
        .map(|seat| OrgParty {
            id: seat.id.clone(),
            label: seat.label.clone(),
            department: seat.department.clone(),
            ask_about: seat.ask_about.clone(),
            answered_by: seat.relays.clone(),
        })
        .collect()
}

fn externals(body: &OrgDefinition) -> Vec<OrgParty> {
    body.externals
        .iter()
        .map(|external| OrgParty {
            id: external.id.clone(),
            label: external.label.clone(),
            department: None,
            ask_about: external.ask_about.clone(),
            answered_by: external.relays.clone(),
        })
        .collect()
}

/// The org's members with their devices, its open seats and external parties, and the changes that wait for
/// this node's root key, for the node's owner. Not found when the owner is no member of the org; forbidden
/// when they are an answerer there, who reads nothing of the org's people.
pub fn org_members(node: &Node, org: &Id) -> Result<OrgMembers> {
    let (doc, owner) = membership(node, org)?;
    if owner.role == Role::Answerer {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "an answerer does not read the org's members",
        ));
    }
    let guests = node.store().guests();
    let held = pending_root::waiting(node)?
        .iter()
        .filter(|change| &change.org == org)
        .map(|change| change.as_ref_for_desk())
        .collect();
    Ok(OrgMembers {
        org: org.clone(),
        members: doc
            .body
            .people
            .iter()
            .map(|person| member(person, &guests, org))
            .collect(),
        seats: seats(&doc.body),
        externals: externals(&doc.body),
        held,
    })
}
