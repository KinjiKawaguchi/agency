//! What a guest reads through the Client API, scoped to its hosted member (docs/protocol.md, "Guests"): its
//! own org and the teams on this node that address it, only the requests it answers, its own devices, and
//! none of the owner's: no funding, home nodes, held root changes, recovery, or other orgs.
use crate::client::{membership_of, relayed_for, team_ref};
use crate::devices::my_devices;
use crate::error::{Error, Result};
use crate::guests::{Caller, Guest};
use crate::node::Node;
use crate::push::is_hiding_titles;
use crate::views::{
    answerer_view, answerers, mine, runtime_view, viewer_name, waiting_on_viewer, Reader,
};
use agency_proto::client::{
    AccountRef, DeviceList, GuestRef, Me, OrgDevices, OrgRef, SnapshotSource, TeamSnapshot,
};
use agency_proto::Id;

/// Whether the team `place` runs on this node and addresses `member`: by a runtime name, or through a seat
/// or external party it answers for.
pub(crate) fn addresses(
    node: &Node,
    definition: &agency_proto::OrgDefinition,
    team: &Id,
    member: &Id,
) -> bool {
    let Some(hosted) = node.hosted(&definition.org, team) else {
        return false;
    };
    viewer_name(&hosted, member).is_some()
        || hosted
            .people
            .values()
            .any(|id| id != member && answerers(definition, &hosted, id).contains(&member))
}

/// The guest's `/api/me`: its member, its org, and the teams here that address it.
pub fn me(node: &Node, guest: &Guest) -> Result<Me> {
    let caller = Caller::Guest(guest.clone());
    let (doc, person) = membership_of(node, &caller, &guest.org)?;
    let teams = doc
        .body
        .teams
        .iter()
        .filter(|place| addresses(node, &doc.body, &place.id, &guest.member))
        .map(|place| team_ref(node, &doc.body.org, place, &person))
        .collect();
    Ok(Me {
        account: AccountRef {
            id: person.id.clone(),
            label: person.label.clone(),
            // A hosted member has no account key.
            fingerprint: String::new(),
        },
        orgs: vec![OrgRef {
            id: doc.body.org.clone(),
            label: doc.body.label.clone(),
            is_root: false,
            teams,
            recovery: None,
        }],
        funding: vec![],
        hides_titles: is_hiding_titles(node),
        node_id: agency_proto::client::node_id(&node.config().url),
        home_nodes: None,
        root_changes: None,
        guest: Some(GuestRef {
            member: guest.member.clone(),
            org: guest.org.clone(),
        }),
    })
}

/// A team as the guest reads it: only a team on this node that addresses it, and in it only the requests it
/// answers, with no people, work, or epics (the answerer's view).
pub async fn team_snapshot(
    node: &Node,
    guest: &Guest,
    org: &Id,
    team: &Id,
) -> Result<TeamSnapshot> {
    let caller = Caller::Guest(guest.clone());
    let (doc, person) = membership_of(node, &caller, org)?;
    let not_found = || Error::not_found("not found");
    if !addresses(node, &doc.body, team, &person.id) {
        return Err(not_found());
    }
    let hosted = node.hosted(org, team).ok_or_else(not_found)?;
    let view = answerer_view(
        runtime_view(node, &hosted).await?,
        &hosted,
        &person.id,
        &doc.body,
    );
    let reader = Reader::Hosted {
        hosted: &hosted,
        person: &person.id,
        definition: &doc.body,
    };
    let viewer = viewer_name(&hosted, &person.id);
    let mine = mine(&view, &reader);
    Ok(TeamSnapshot {
        relayed_for: relayed_for(&doc.body, &hosted, &view, &mine),
        mine,
        waiting_on_viewer: waiting_on_viewer(&view, viewer.as_deref()),
        view,
        viewer,
        source: SnapshotSource::Local,
    })
}

/// The guest's own devices in its org and the devices that ask to be added for its member, and no command:
/// that is the owner's.
pub fn device_list(node: &Node, guest: &Guest) -> Result<DeviceList> {
    let caller = Caller::Guest(guest.clone());
    let (doc, _) = membership_of(node, &caller, &guest.org)?;
    Ok(DeviceList {
        orgs: vec![OrgDevices {
            devices: my_devices(&doc.body, &guest.member).to_vec(),
            org: doc.body.org,
        }],
        requests: crate::devices::guest_requests(
            node,
            &GuestRef {
                member: guest.member.clone(),
                org: guest.org.clone(),
            },
        )?,
        node: None,
    })
}
