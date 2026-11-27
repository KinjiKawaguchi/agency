//! Weekly budget calculation, independent of how team usage is read.
use crate::client::membership;
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use agency_proto::clock::{Duration, OffsetDateTime, UtcOffset};
use agency_proto::org::OrgDefinition;
use agency_proto::{Date, Id, Role, TokenUsage};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BudgetState {
    Ok,
    Warning,
    Over,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_usd: Option<u32>,
    pub spent_usd: f64,
    pub state: BudgetState,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetStatus {
    pub week_start: Date,
    pub team: BudgetPart,
    pub org: OrgBudget,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrgBudget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_usd: Option<u32>,
    pub spent_usd: f64,
    pub state: BudgetState,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

pub fn week_start(now: OffsetDateTime, offset: UtcOffset) -> Date {
    let day = now.to_offset(offset).date();
    let start = day - Duration::days(day.weekday().number_days_from_monday().into());
    Date::parse(&start.to_string()).unwrap()
}
/// The calendar day of `now` in the node's time zone.
pub fn today(now: OffsetDateTime, offset: UtcOffset) -> Date {
    Date::parse(&now.to_offset(offset).date().to_string())
        .expect("a calendar date is a valid protocol date")
}

/// Adds the daily costs reported from the start of this week through today, inclusive.
///
/// A runtime's report can reach back to an earlier Sunday when it is asked for seven days on a Monday,
/// so the caller gives both bounds instead of relying on the number of days it asked for.
pub fn week_spend(usage: &TokenUsage, start: &Date, today: &Date) -> f64 {
    usage
        .by_day
        .iter()
        .filter(|day| day.date.as_str() >= start.as_str() && day.date.as_str() <= today.as_str())
        .flat_map(|day| day.cost_usd.values())
        .sum()
}

/// The org's definition, for a read of one team's budget by the node's account: not found when the
/// account is not a member or the org has no such team, and forbidden for an answerer, who reads no
/// usage.
pub fn definition_for_budget(node: &Node, org: &Id, team: &Id) -> Result<OrgDefinition> {
    let (doc, person) = membership(node, org)?;
    if person.role == Role::Answerer {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "an answerer does not read the org's budget",
        ));
    }
    match doc.body.team(team) {
        Some(_) => Ok(doc.body),
        None => Err(Error::not_found("not found")),
    }
}

fn state(cap: Option<u32>, spent: f64) -> BudgetState {
    match cap {
        None => BudgetState::Ok,
        Some(c) if spent >= c as f64 => BudgetState::Over,
        Some(c) if spent >= c as f64 * 0.8 => BudgetState::Warning,
        _ => BudgetState::Ok,
    }
}
pub fn status(
    doc: &OrgDefinition,
    start: Date,
    team: &Id,
    spends: &BTreeMap<Id, Option<f64>>,
) -> Option<BudgetStatus> {
    let place = doc.team(team)?;
    let cap = place.plan.as_ref().map(|p| p.weekly_budget_usd);
    let team_spent = spends.get(team).and_then(|v| *v).unwrap_or(0.0);
    let planned: Vec<_> = doc.teams.iter().filter(|t| t.plan.is_some()).collect();
    let org_cap = (!planned.is_empty()).then(|| {
        planned
            .iter()
            .map(|t| t.plan.as_ref().unwrap().weekly_budget_usd)
            .sum()
    });
    let team_partial = !matches!(spends.get(team), Some(Some(_)));
    let partial = team_partial
        || planned
            .iter()
            .any(|t| matches!(spends.get(&t.id), Some(None) | None));
    let org_spent = planned
        .iter()
        .filter_map(|t| spends.get(&t.id).and_then(|v| *v))
        .sum();
    let org_state = state(org_cap, org_spent);
    let mut team_state = state(cap, team_spent);
    if org_state == BudgetState::Over {
        team_state = BudgetState::Over;
    }
    Some(BudgetStatus {
        week_start: start,
        team: BudgetPart {
            cap_usd: cap,
            spent_usd: team_spent,
            state: team_state,
            partial: team_partial,
        },
        org: OrgBudget {
            cap_usd: org_cap,
            spent_usd: org_spent,
            state: org_state,
            partial,
        },
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::clock;
    #[test]
    fn monday_in_nodes_offset() {
        let n = clock::parse("2026-11-22T15:00:00Z").unwrap();
        assert_eq!(
            week_start(n, UtcOffset::from_hms(9, 0, 0).unwrap()).as_str(),
            "2026-11-23"
        );
    }
}
