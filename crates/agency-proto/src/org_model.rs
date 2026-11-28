//! The org as units, teams and heads, read the same way whatever shape its definition is written in. A unit is a
//! department or a team: units nest by `parent`, a team is a unit a harness runs, and a unit may have a head. Code
//! reads the org's structure through these, never through the definition's lists, so that a definition written
//! another way is read by the same code (agency#674). The lists are private to this crate, so a reader that went
//! around these would not compile.
use crate::org::{Department, OrgDefinition, TeamPlace};
use crate::Id;

/// A unit of the org: a department, or a team (`team` is then the team's place).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub id: Id,
    pub label: String,
    pub parent: Option<Id>,
    /// Who heads the unit: a person (an agent too, in a definition that has agents).
    pub head: Option<Id>,
    pub team: Option<TeamPlace>,
}

impl OrgDefinition {
    /// The departments as written, for the commands that change them in a definition written with departments.
    /// Every reader goes through `groups()` or `units()`.
    pub fn departments_mut(&mut self) -> &mut Vec<Department> {
        &mut self.departments
    }

    /// The team places as written, for the commands that change them in a definition written with teams. Every
    /// reader goes through `teams()` or `team()`.
    pub fn teams_mut(&mut self) -> &mut Vec<TeamPlace> {
        &mut self.teams
    }

    /// The org's teams, in the definition's order: each one's id, label, node, the unit it is in
    /// (`department`), and its plan.
    pub fn teams(&self) -> impl Iterator<Item = TeamPlace> + '_ {
        self.teams.iter().cloned()
    }

    pub fn team(&self, id: &Id) -> Option<TeamPlace> {
        self.teams.iter().find(|one| &one.id == id).cloned()
    }

    /// The units that hold others and are not teams (the departments, in a definition written with them), in the
    /// definition's order.
    pub fn groups(&self) -> impl Iterator<Item = Department> + '_ {
        self.departments.iter().cloned()
    }

    /// The unit a parent names: one that holds others, since only those do.
    pub fn group(&self, id: &Id) -> Option<Department> {
        self.departments.iter().find(|one| &one.id == id).cloned()
    }

    /// Every unit of the org: the groups, then the teams. A group and a team may share an id in a definition with
    /// departments; a unit's parent is then always the group.
    pub fn units(&self) -> impl Iterator<Item = Unit> + '_ {
        let groups = self.groups().map(|group| Unit {
            id: group.id,
            label: group.label,
            parent: group.parent,
            head: group.head,
            team: None,
        });
        let teams = self.teams().map(|team| Unit {
            id: team.id.clone(),
            label: team.label.clone(),
            parent: team.department.clone(),
            head: None,
            team: Some(team),
        });
        groups.chain(teams)
    }

    /// Who heads the org: the person whose account is the root's, or no one when the root's account is no person
    /// of the org. The root is an authority; the head is a place in the structure, and holds no authority.
    pub fn head(&self) -> Option<&Id> {
        self.people
            .iter()
            .find(|person| person.account == self.root.account)
            .map(|person| &person.id)
    }

    /// Whom `party` (a person) reports to: the head of the unit it is in, or for that unit's head the nearest head
    /// above it, and in the end the org's head. The org's head reports to no one, and a party the org does not
    /// have reports to no one.
    pub fn reports_to(&self, party: &Id) -> Option<Id> {
        let person = self.people.iter().find(|person| &person.id == party)?;
        let head = self.head().cloned();
        if head.as_ref() == Some(party) {
            return None;
        }
        let mut at = person.department.clone();
        for _ in 0..=self.departments.len() {
            let Some(group) = at.as_ref().and_then(|id| self.group(id)) else {
                break;
            };
            if let Some(group_head) = group.head.filter(|one| one != party) {
                return Some(group_head);
            }
            at = group.parent;
        }
        head
    }
}

#[cfg(test)]
mod tests;
