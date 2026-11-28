//! The org as units, teams and heads, read the same way whatever shape its definition is written in. A unit is a
//! department or a team: units nest by `parent`, a team is a unit a harness runs, and a unit may have a head. Code
//! reads the org's structure through these, never through the definition's lists, so that a definition written
//! another way is read by the same code (agency#674). The lists are private to this crate, so a reader that went
//! around these would not compile.
use crate::org::{Department, OrgDefinition, Person, TeamPlace};
use crate::org_units::{Agent, UNIT_SHAPE};
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

    /// Whether the definition is written in units (shape 2) rather than with departments and teams.
    pub fn is_in_units(&self) -> bool {
        self.shape == Some(UNIT_SHAPE)
    }

    /// The org's teams, in the definition's order: each one's id, label, node, the unit it is in
    /// (`department`), and its plan.
    pub fn teams(&self) -> impl Iterator<Item = TeamPlace> + '_ {
        let written = self.teams.iter().cloned();
        let in_units = self.units.iter().filter_map(|unit| {
            let team = unit.team.as_ref()?;
            Some(
                TeamPlace::new(unit.id.clone(), unit.label.clone(), team.node.clone())
                    .with_department(unit.parent.clone())
                    .with_plan(unit.plan.clone()),
            )
        });
        written.chain(in_units)
    }

    pub fn team(&self, id: &Id) -> Option<TeamPlace> {
        self.teams().find(|one| &one.id == id)
    }

    /// The units that are not teams (the departments, in a definition written with them), in the definition's
    /// order.
    pub fn groups(&self) -> impl Iterator<Item = Department> + '_ {
        let written = self.departments.iter().cloned();
        let in_units = self
            .units
            .iter()
            .filter(|unit| unit.team.is_none())
            .map(|unit| Department {
                id: unit.id.clone(),
                label: unit.label.clone(),
                parent: unit.parent.clone(),
                head: unit.head.clone(),
            });
        written.chain(in_units)
    }

    /// A unit that is not a team, by id.
    pub fn group(&self, id: &Id) -> Option<Department> {
        self.groups().find(|one| &one.id == id)
    }

    /// Every unit of the org. A definition with departments lists its departments, then its teams; there a
    /// department and a team may share an id, and a unit's parent is always the department. A definition in units
    /// lists its units as written.
    pub fn units(&self) -> impl Iterator<Item = Unit> + '_ {
        let groups = self.departments.iter().map(|group| Unit {
            id: group.id.clone(),
            label: group.label.clone(),
            parent: group.parent.clone(),
            head: group.head.clone(),
            team: None,
        });
        let teams = self.teams.iter().map(|team| Unit {
            id: team.id.clone(),
            label: team.label.clone(),
            parent: team.department.clone(),
            head: None,
            team: Some(team.clone()),
        });
        let in_units = self.units.iter().map(|unit| Unit {
            id: unit.id.clone(),
            label: unit.label.clone(),
            parent: unit.parent.clone(),
            head: unit.head.clone(),
            team: unit.team.as_ref().map(|team| {
                TeamPlace::new(unit.id.clone(), unit.label.clone(), team.node.clone())
                    .with_department(unit.parent.clone())
                    .with_plan(unit.plan.clone())
            }),
        });
        groups.chain(teams).chain(in_units)
    }

    /// The same definition with these agents. They are read only in a definition written in units, which refuses
    /// them otherwise.
    pub fn with_agents(self, agents: Vec<Agent>) -> Self {
        Self { agents, ..self }
    }

    /// The agents of the org, in a definition written in units (none otherwise).
    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    /// The unit a member is in: `unit` in a definition written in units, `department` otherwise.
    pub fn unit_of(&self, person: &Person) -> Option<Id> {
        if self.is_in_units() {
            person.unit.clone()
        } else {
            person.department.clone()
        }
    }

    /// The unit a parent or a member's unit names, as a unit that may hold others: in a definition written with
    /// departments only a department does; in one written in units any unit does, a team too.
    fn holder(&self, id: &Id) -> Option<Unit> {
        if self.is_in_units() {
            self.units().find(|unit| &unit.id == id)
        } else {
            self.groups().find(|one| &one.id == id).map(|group| Unit {
                id: group.id,
                label: group.label,
                parent: group.parent,
                head: group.head,
                team: None,
            })
        }
    }

    /// Who heads the org: the head it names, in a definition written in units, else the person whose account is the
    /// root's, or no one when the root's account is no person of the org. The root is an authority; the head is a
    /// place in the structure, and holds no authority.
    pub fn head(&self) -> Option<&Id> {
        if let Some(head) = self.head.as_ref().filter(|_| self.is_in_units()) {
            return Some(head);
        }
        self.people
            .iter()
            .find(|person| person.account == self.root.account)
            .map(|person| &person.id)
    }

    /// Whom `party` (a person, or an agent) reports to: the head of the unit it is in, or for that unit's head the
    /// nearest head above it, and in the end the org's head. The org's head reports to no one, and a party the org
    /// does not have reports to no one.
    pub fn reports_to(&self, party: &Id) -> Option<Id> {
        let person = self.people.iter().find(|person| &person.id == party);
        let agent = self.agents.iter().find(|agent| &agent.id == party);
        let mut at = match (person, agent) {
            (Some(person), _) => self.unit_of(person),
            (None, Some(agent)) => agent.unit.clone(),
            (None, None) => return None,
        };
        let head = self.head().cloned();
        if head.as_ref() == Some(party) {
            return None;
        }
        for _ in 0..=(self.departments.len() + self.units.len()) {
            let Some(unit) = at.as_ref().and_then(|id| self.holder(id)) else {
                break;
            };
            if let Some(unit_head) = unit.head.filter(|one| one != party) {
                return Some(unit_head);
            }
            at = unit.parent;
        }
        head
    }
}

#[cfg(test)]
mod tests;
