//! Weekly budget calculation, independent of how team usage is read.
use agency_proto::clock::{Duration, OffsetDateTime, UtcOffset};
use agency_proto::org::OrgDefinition;
use agency_proto::{Date, Id};
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
    use agency_proto::org::{Allocation, Root, TeamPlace, TeamPlan};
    use agency_proto::AccountKey;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    fn plan(cap: u32) -> TeamPlan {
        TeamPlan::new(
            Allocation {
                new_feature: 100,
                improvement: 0,
                maintenance: 0,
                debt: 0,
                incident: 0,
            },
            cap,
        )
    }

    fn org(teams: &[(&str, Option<u32>)]) -> OrgDefinition {
        let root = Root::new(id("alice"), AccountKey::from_seed([1; 32]).public());
        OrgDefinition::new(
            id("acme"),
            "Acme".into(),
            root,
            "2026-11-24T12:00:00Z".into(),
        )
        .with_teams(
            teams
                .iter()
                .map(|(name, cap)| {
                    TeamPlace::new(id(name), *name, "https://node.example").with_plan(cap.map(plan))
                })
                .collect(),
        )
    }

    fn start() -> Date {
        Date::parse("2026-11-23").unwrap()
    }

    fn spends(entries: &[(&str, Option<f64>)]) -> BTreeMap<Id, Option<f64>> {
        entries
            .iter()
            .map(|(team, spent)| (id(team), *spent))
            .collect()
    }
    #[test]
    fn monday_in_nodes_offset() {
        let n = clock::parse("2026-11-22T15:00:00Z").unwrap();
        assert_eq!(
            week_start(n, UtcOffset::from_hms(9, 0, 0).unwrap()).as_str(),
            "2026-11-23"
        );
    }

    #[test]
    fn unavailable_selected_team_is_partial_even_without_a_plan() {
        let planned = org(&[("app", Some(200)), ("ops", Some(300))]);
        let report = status(
            &planned,
            start(),
            &id("app"),
            &spends(&[("app", None), ("ops", Some(100.0))]),
        )
        .unwrap();
        assert!(report.team.partial);
        assert!(report.org.partial);
        assert_eq!(report.team.spent_usd, 0.0);
        assert_eq!(report.org.spent_usd, 100.0);

        let unplanned = org(&[("app", Some(200)), ("ops", None)]);
        let report = status(
            &unplanned,
            start(),
            &id("ops"),
            &spends(&[("app", Some(20.0)), ("ops", None)]),
        )
        .unwrap();
        assert!(report.team.partial);
        assert!(
            report.org.partial,
            "the selected team counts toward partial"
        );
        assert_eq!(report.team.cap_usd, None);
    }

    #[test]
    fn team_is_over_when_its_org_is_over() {
        let report = status(
            &org(&[("app", Some(200)), ("ops", Some(300))]),
            start(),
            &id("app"),
            &spends(&[("app", Some(50.0)), ("ops", Some(470.0))]),
        )
        .unwrap();
        assert_eq!(report.org.state, BudgetState::Over);
        assert_eq!(report.team.state, BudgetState::Over);
    }

    #[test]
    fn budget_threshold_edges_are_inclusive() {
        for (spent, expected) in [
            (79.99, BudgetState::Ok),
            (80.0, BudgetState::Warning),
            (80.01, BudgetState::Warning),
            (99.99, BudgetState::Warning),
            (100.0, BudgetState::Over),
            (100.01, BudgetState::Over),
        ] {
            let report = status(
                &org(&[("app", Some(100))]),
                start(),
                &id("app"),
                &spends(&[("app", Some(spent))]),
            )
            .unwrap();
            assert_eq!(report.team.state, expected, "spent {spent}");
            assert_eq!(report.org.state, expected, "spent {spent}");
        }
    }

    #[test]
    fn a_team_without_a_plan_has_no_cap_and_is_ok() {
        let report = status(
            &org(&[("app", None)]),
            start(),
            &id("app"),
            &spends(&[("app", Some(1_000.0))]),
        )
        .unwrap();
        assert_eq!(report.team.cap_usd, None);
        assert_eq!(report.team.state, BudgetState::Ok);
        assert_eq!(report.org.cap_usd, None);
        assert_eq!(report.org.state, BudgetState::Ok);
    }
}
