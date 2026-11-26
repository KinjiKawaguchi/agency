//! Who an org's agents may ask for what (docs/protocol.md, "The directory for agents"): the org's members,
//! seats, and external parties from its definition, with the names each has in the teams this node runs. A
//! team's runtime knows the people of its team only by its own names, and the definition reaches it no other
//! way, so a harness reads this from its own node. It holds labels and what to ask about, nothing of a key,
//! a device, or an account.
use crate::client::membership;
use crate::config::HostedTeam;
use crate::error::Result;
use crate::node::Node;
use agency_proto::directory::{
    Directory, DirectoryDepartment, DirectoryMember, DirectoryParty, RuntimeNames,
};
use agency_proto::org::Role;
use agency_proto::{Id, OrgDefinition};

/// The names the teams of this node (in `teams`) know `id` by: `owner` for a team's owner, and each key of
/// the team's `people` that maps to it. A team that addresses no such name is left out.
fn names_in(teams: &[&HostedTeam], id: &Id) -> Vec<RuntimeNames> {
    teams
        .iter()
        .filter_map(|hosted| {
            let mapped = hosted
                .people
                .iter()
                .filter(|(_, mapped)| *mapped == id)
                .map(|(name, _)| name.clone());
            let owner = (&hosted.owner == id).then(|| "owner".to_owned());
            let names: Vec<String> = owner.into_iter().chain(mapped).collect();
            (!names.is_empty()).then(|| RuntimeNames {
                team: hosted.team.clone(),
                names,
            })
        })
        .collect()
}

fn departments(body: &OrgDefinition) -> Vec<DirectoryDepartment> {
    let departments = body.departments.iter();
    departments
        .map(|one| DirectoryDepartment {
            id: one.id.clone(),
            label: one.label.clone(),
            parent: one.parent.clone(),
            head: one.head.clone(),
        })
        .collect()
}

/// The directory of `org`, for a node whose account is a member of it: the members, the seats, and the
/// external parties, in the order of the definition, with their names in this node's teams of the org.
pub fn directory(node: &Node, org: &Id) -> Result<Directory> {
    let (doc, _) = membership(node, org)?;
    let body = &doc.body;
    let config = node.config();
    let teams: Vec<&HostedTeam> = config
        .teams
        .iter()
        .filter(|hosted| &hosted.org == org && body.team(&hosted.team).is_some())
        .collect();
    let members = body.people.iter().map(|person| DirectoryMember {
        id: person.id.clone(),
        label: person.label.clone(),
        department: person.department.clone(),
        role: match person.role {
            Role::Member => "member",
            Role::Answerer => "answerer",
        }
        .to_owned(),
        runtime_names: names_in(&teams, &person.id),
    });
    let seats = body.seats.iter().map(|seat| DirectoryParty {
        id: seat.id.clone(),
        label: seat.label.clone(),
        department: seat.department.clone(),
        ask_about: seat.ask_about.clone(),
        answered_by: seat.relays.clone(),
        runtime_names: names_in(&teams, &seat.id),
    });
    let externals = body.externals.iter().map(|external| DirectoryParty {
        id: external.id.clone(),
        label: external.label.clone(),
        department: None,
        ask_about: external.ask_about.clone(),
        answered_by: external.relays.clone(),
        runtime_names: names_in(&teams, &external.id),
    });
    Ok(Directory {
        org: org.clone(),
        departments: departments(body),
        members: members.collect(),
        seats: seats.collect(),
        externals: externals.collect(),
    })
}
