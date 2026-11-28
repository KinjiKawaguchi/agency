//! What an org measures (its KPIs) and what a team's plan aims at, changed on the root key's node like the
//! rest of the definition: the next version, signed (`orgs::amend`). Only labels and ids are here; the
//! numbers are business data kept on the owner's node and never in the definition.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::amend;
use agency_proto::org::{SignedOrg, TeamPlan};
use agency_proto::outcome::{Aim, GoodWhen, Kpi, KpiUnit};
use agency_proto::{Id, OrgDefinition};

/// What to change in a KPI. `None` leaves a field as it is.
#[derive(Clone, Debug, Default)]
pub struct Change {
    pub label: Option<String>,
    pub unit: Option<KpiUnit>,
    pub good_when: Option<GoodWhen>,
}

fn not_found(what: &str, id: &Id, org: &Id) -> Error {
    Error::not_found(format!("no {what} {id} in org {org}"))
}

fn find_kpi<'a>(body: &'a mut OrgDefinition, id: &Id) -> Result<&'a mut Kpi> {
    let org = body.org.clone();
    body.kpis
        .iter_mut()
        .find(|kpi| &kpi.id == id)
        .ok_or_else(|| not_found("KPI", id, &org))
}

/// States a KPI of the org. Its id must be new.
pub fn add(node: &Node, org: &Id, kpi: Kpi) -> Result<SignedOrg> {
    amend(node, org, |body| {
        if body.kpis.iter().any(|one| one.id == kpi.id) {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("KPI {} already exists", kpi.id),
            ));
        }
        body.kpis.push(kpi);
        Ok(())
    })
}

/// Changes a KPI's label, unit, or direction. A KPI's kind and id stay: the numbers kept for it on the
/// owner's node are filed under them.
pub fn change(node: &Node, org: &Id, id: &Id, change: Change) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let kpi = find_kpi(body, id)?;
        let before = kpi.clone();
        if let Some(label) = change.label {
            kpi.label = label;
        }
        if let Some(unit) = change.unit {
            kpi.unit = unit;
        }
        if let Some(good_when) = change.good_when {
            kpi.good_when = good_when;
        }
        if *kpi == before {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("KPI {id} is already like that"),
            ));
        }
        Ok(())
    })
}

/// Takes a KPI out of the definition. Refused while a team's plan aims at it, which names the teams.
pub fn remove(node: &Node, org: &Id, id: &Id) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let aimed_by: Vec<String> = body
            .teams()
            .filter(|team| {
                let aims = team.plan.iter().flat_map(|plan| &plan.aims);
                aims.into_iter().any(|aim| &aim.kpi == id)
            })
            .map(|team| team.id.to_string())
            .collect();
        if !aimed_by.is_empty() {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!(
                    "KPI {id} is still aimed at by team {}; take the aim out first (plan-unaim)",
                    aimed_by.join(", ")
                ),
            ));
        }
        let before = body.kpis.len();
        body.kpis.retain(|kpi| &kpi.id != id);
        if body.kpis.len() == before {
            return Err(not_found("KPI", id, &body.org));
        }
        Ok(())
    })
}

fn plan_of<'a>(body: &'a mut OrgDefinition, team: &Id) -> Result<&'a mut TeamPlan> {
    let org = body.org.clone();
    let place = body
        .teams_mut()
        .iter_mut()
        .find(|one| &one.id == team)
        .ok_or_else(|| not_found("team", team, &org))?;
    place.plan.as_mut().ok_or_else(|| {
        Error::new(
            ErrorKind::Conflict,
            format!("team {team} has no plan; set one with team-plan first"),
        )
    })
}

/// States that a team's plan aims `epic` (an item reference) at a KPI of the org.
pub fn aim(node: &Node, org: &Id, team: &Id, epic: String, kpi: &Id) -> Result<SignedOrg> {
    amend(node, org, |body| {
        if !body.kpis.iter().any(|one| &one.id == kpi) {
            return Err(not_found("KPI", kpi, &body.org));
        }
        let plan = plan_of(body, team)?;
        let aim = Aim {
            epic,
            kpi: kpi.clone(),
        };
        if plan.aims.contains(&aim) {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("team {team} already aims {} at {kpi}", aim.epic),
            ));
        }
        plan.aims.push(aim);
        Ok(())
    })
}

/// Takes an aim out of a team's plan.
pub fn unaim(node: &Node, org: &Id, team: &Id, epic: &str, kpi: &Id) -> Result<SignedOrg> {
    amend(node, org, |body| {
        let plan = plan_of(body, team)?;
        let before = plan.aims.len();
        plan.aims
            .retain(|aim| !(aim.epic == epic && &aim.kpi == kpi));
        if plan.aims.len() == before {
            return Err(Error::not_found(format!(
                "team {team} does not aim {epic} at {kpi}"
            )));
        }
        Ok(())
    })
}
