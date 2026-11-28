//! The parts of an org definition written in units (shape 2, agency#674): units that nest, a harness as what makes
//! a unit a team, agents with no key, heads, and accounts that only host teams. Each is optional in the definition
//! and left out when absent, so a definition written before units keeps its canonical JSON and its signature.
use crate::org::TeamPlan;
use crate::Id;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The shape of a definition written in units. A definition without `shape` is written with departments and teams.
pub const UNIT_SHAPE: u8 = 2;
/// The most units a definition holds.
pub const MAX_UNITS: usize = 200;
/// The most agents a definition holds.
pub const MAX_AGENTS: usize = 500;
/// How deep units nest: a unit at the top is at depth 1.
pub const MAX_UNIT_DEPTH: usize = 16;

/// A unit of the org: an area that holds people, agents and other units, and that a harness may run (`team`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct OrgUnit {
    pub id: Id,
    pub label: String,
    /// The unit it is in. Left out for a unit at the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Id>,
    /// Who heads it: a person or an agent of the org. A head holds no authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<Id>,
    /// Set when a harness runs the unit, which makes it a team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<UnitTeam>,
    /// What the org intends for the team. Only a unit with `team` has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<TeamPlan>,
}

impl OrgUnit {
    /// A unit at the top, with no head, no team and no plan.
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            parent: None,
            head: None,
            team: None,
            plan: None,
        }
    }

    pub fn with_parent(self, parent: Option<Id>) -> Self {
        Self { parent, ..self }
    }

    pub fn with_head(self, head: Option<Id>) -> Self {
        Self { head, ..self }
    }

    /// The same unit, run by a harness on the node at `node`.
    pub fn with_team(self, node: impl Into<String>) -> Self {
        Self {
            team: Some(UnitTeam { node: node.into() }),
            ..self
        }
    }

    pub fn with_plan(self, plan: Option<TeamPlan>) -> Self {
        Self { plan, ..self }
    }
}

/// Where the harness that runs a unit runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UnitTeam {
    /// The node's origin (http or https, no path).
    pub node: String,
}

/// An agent of the org: it has an id and a place, and no key, no account and no devices. It is never a member, so
/// it signs nothing and answers nothing; which harness runs it is the node's configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct Agent {
    pub id: Id,
    pub label: String,
    /// The unit it is in. Left out for an agent in no unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<Id>,
}

impl Agent {
    pub fn new(id: Id, label: impl Into<String>, unit: Option<Id>) -> Self {
        Self {
            id,
            label: label.into(),
            unit,
        }
    }
}

/// What a member's account is, when it is not a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MemberKind {
    /// An account that only hosts teams: it signs reads and runs harnesses, and is no one on the org's chart.
    Node,
}

/// The structure of a definition written in units: none of the fields of one written with departments and teams
/// (the two shapes never mix), units that exist and nest without a cycle and not too deep, agents in units that
/// exist, and heads that are people or agents.
pub(crate) fn validate(definition: &crate::org::OrgDefinition) -> Result<(), String> {
    use crate::org::{is_node_origin, label_ok};
    use std::collections::{HashMap, HashSet};
    let has_old_fields = !definition.departments.is_empty()
        || !definition.teams.is_empty()
        || definition.people.iter().any(|p| p.department.is_some())
        || definition
            .seats
            .iter()
            .any(|seat| seat.department.is_some());
    if has_old_fields {
        return Err(
            "a definition written in units has a department, a team, or a department of a person or a seat"
                .into(),
        );
    }
    if definition.units.len() > MAX_UNITS {
        return Err(format!("an org has at most {MAX_UNITS} units"));
    }
    if definition.agents.len() > MAX_AGENTS {
        return Err(format!("an org has at most {MAX_AGENTS} agents"));
    }
    let mut parent_of = HashMap::new();
    for unit in &definition.units {
        if parent_of.insert(&unit.id, unit.parent.as_ref()).is_some() {
            return Err(format!("unit {} appears twice", unit.id));
        }
    }
    for unit in &definition.units {
        if !label_ok(&unit.label) {
            return Err(format!("malformed label of unit {}", unit.id));
        }
        if let Some(parent) = &unit.parent {
            if !parent_of.contains_key(parent) {
                return Err(format!(
                    "unit {} is in unit {parent}, which the org does not have",
                    unit.id
                ));
            }
        }
        if unit
            .team
            .as_ref()
            .is_some_and(|team| !is_node_origin(&team.node))
        {
            return Err(format!("unit {}'s team has a malformed node URL", unit.id));
        }
        if unit.plan.is_some() && unit.team.is_none() {
            return Err(format!("unit {} has a plan and no team", unit.id));
        }
        let mut depth = 1;
        let mut at = unit.parent.as_ref();
        while let Some(id) = at {
            depth += 1;
            if id == &unit.id || depth > MAX_UNIT_DEPTH {
                return Err(format!(
                    "unit {} is under itself, or more than {MAX_UNIT_DEPTH} units deep",
                    unit.id
                ));
            }
            at = parent_of.get(id).copied().flatten();
        }
    }
    let is_unit = |id: &Option<Id>| id.as_ref().is_none_or(|one| parent_of.contains_key(one));
    for agent in &definition.agents {
        if !label_ok(&agent.label) || !is_unit(&agent.unit) {
            return Err(format!(
                "agent {} has a malformed label or a unit the org does not have",
                agent.id
            ));
        }
    }
    if let Some(person) = definition.people.iter().find(|p| !is_unit(&p.unit)) {
        return Err(format!("{} is in a unit the org does not have", person.id));
    }
    let may_head: HashSet<&Id> = definition
        .people
        .iter()
        .filter(|p| p.kind.is_none())
        .map(|p| &p.id)
        .chain(definition.agents.iter().map(|a| &a.id))
        .collect();
    let heads = definition
        .units
        .iter()
        .filter_map(|unit| unit.head.as_ref())
        .chain(definition.head.as_ref());
    for head in heads {
        if !may_head.contains(head) {
            return Err(format!(
                "{head} heads a unit or the org, and is not a person or an agent of it"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
