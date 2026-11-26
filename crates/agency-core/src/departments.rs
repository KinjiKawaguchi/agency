//! An org's departments, the department each team is in, and what the org intends for each team. Like everything in an org's definition,
//! only the root key's node changes them, by signing the next version (`orgs::amend`).
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::amend;
use agency_proto::org::{Department, SignedOrg, TeamPlan};
use agency_proto::{Id, OrgDefinition};

/// What to change in a department. `None` leaves a field as it is; `Some(None)` clears it.
#[derive(Clone, Debug, Default)]
pub struct Change {
    pub label: Option<String>,
    pub parent: Option<Option<Id>>,
    pub head: Option<Option<Id>>,
}

fn not_found(what: &str, id: &Id, org: &Id) -> Error {
    Error::not_found(format!("no {what} {id} in org {org}"))
}

/// The parent is a department of the org, and the head one of its members (by member id).
fn check_links(body: &OrgDefinition, department: &Department) -> Result<()> {
    if let Some(parent) = &department.parent {
        if !body.departments.iter().any(|one| &one.id == parent) {
            return Err(not_found("department", parent, &body.org));
        }
    }
    if let Some(head) = &department.head {
        if !body.people.iter().any(|one| &one.id == head) {
            return Err(not_found("member", head, &body.org));
        }
    }
    Ok(())
}

pub fn add(node: &Node, org: &Id, department: Department) -> Result<SignedOrg> {
    amend(node, org, |body| {
        if body.departments.iter().any(|one| one.id == department.id) {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("department {} already exists", department.id),
            ));
        }
        check_links(body, &department)?;
        body.departments.push(department);
        Ok(())
    })
}

/// Changes a department's label, parent, or head. A parent that would put the department under
/// itself is refused by the definition's own check.
pub fn change(node: &Node, org: &Id, id: &Id, change: Change) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let index = body
            .departments
            .iter()
            .position(|one| &one.id == id)
            .ok_or_else(|| not_found("department", id, org))?;
        let mut department = body.departments[index].clone();
        if let Some(label) = change.label {
            department.label = label;
        }
        if let Some(parent) = change.parent {
            department.parent = parent;
        }
        if let Some(head) = change.head {
            department.head = head;
        }
        check_links(body, &department)?;
        body.departments[index] = department;
        Ok(())
    })
}

/// Removes a department that nothing is in: no department under it, and no member or team in it.
pub fn remove(node: &Node, org: &Id, id: &Id) -> Result<SignedOrg> {
    amend(node, org, |body| {
        if !body.departments.iter().any(|one| &one.id == id) {
            return Err(not_found("department", id, org));
        }
        let is_here = |department: &Option<Id>| department.as_ref() == Some(id);
        let children = body.departments.iter().filter(|one| is_here(&one.parent));
        let people = body.people.iter().filter(|one| is_here(&one.department));
        let teams = body.teams.iter().filter(|one| is_here(&one.department));
        let in_it: Vec<String> = children
            .map(|one| format!("department {}", one.id))
            .chain(people.map(|one| format!("member {}", one.id)))
            .chain(teams.map(|one| format!("team {}", one.id)))
            .collect();
        if !in_it.is_empty() {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("department {id} still holds {}", in_it.join(", ")),
            ));
        }
        body.departments.retain(|one| &one.id != id);
        Ok(())
    })
}

/// Puts a team in a department, or with `None` in none.
pub fn place_team(node: &Node, org: &Id, team: &Id, department: Option<Id>) -> Result<SignedOrg> {
    amend(node, org, |body| {
        if let Some(department) = &department {
            if !body.departments.iter().any(|one| &one.id == department) {
                return Err(not_found("department", department, org));
            }
        }
        let place = body
            .teams
            .iter_mut()
            .find(|one| &one.id == team)
            .ok_or_else(|| not_found("team", team, org))?;
        if place.department == department {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("team {team} is already there"),
            ));
        }
        place.department = department;
        Ok(())
    })
}

/// Sets what the org intends for a team (its plan), or with `None` clears it (the aims with it). The
/// allocation must add up to 100. Setting a plan keeps the aims the team's plan already has: they are
/// changed with `kpis::aim` and `kpis::unaim`, not here.
pub fn plan_team(node: &Node, org: &Id, team: &Id, plan: Option<TeamPlan>) -> Result<SignedOrg> {
    if let Some(plan) = &plan {
        let total = plan.allocation.total();
        if total != 100 {
            return Err(Error::bad_request(format!(
                "the allocation adds up to {total}, not 100"
            )));
        }
    }
    amend(node, org, |body| {
        let place = body
            .teams
            .iter_mut()
            .find(|one| &one.id == team)
            .ok_or_else(|| not_found("team", team, org))?;
        let aims = place.plan.as_ref().map(|one| one.aims.clone());
        let plan = match (plan, aims) {
            (Some(plan), Some(aims)) => Some(plan.with_aims(aims)),
            (plan, _) => plan,
        };
        if place.plan == plan {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("team {team} already has that plan"),
            ));
        }
        place.plan = plan;
        Ok(())
    })
}
