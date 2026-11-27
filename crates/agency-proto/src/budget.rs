//! The weekly budget of a team and of its org (docs/protocol.md, "Budget"): what was spent this week
//! against what the org's plan intends, and where that stands. The node works it out; a screen only draws it.
use crate::timestamp::Date;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where a spend stands against its cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum BudgetState {
    /// Below 80% of the cap, or there is no cap.
    Ok,
    /// At or past 80% of the cap and below it.
    Warning,
    /// At or past the cap. A team is also over when its org is.
    Over,
}

/// A team's budget: its own cap and spend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct BudgetPart {
    /// The weekly budget the team's plan states, in whole US dollars; left out when it has no plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_usd: Option<u32>,
    /// What the team spent this week, API-priced as in `usage`. When `partial` is set it is not known and
    /// the 0 here says nothing.
    pub spent_usd: f64,
    pub state: BudgetState,
    /// The team's usage could not be read now. Left out when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, type = "boolean")]
    pub partial: bool,
}

/// The org's budget: the sum over the teams that have a plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct OrgBudget {
    /// The sum of the weekly budgets of the teams that have a plan; left out when none has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_usd: Option<u32>,
    /// What those teams spent this week, the ones whose usage was read.
    pub spent_usd: f64,
    pub state: BudgetState,
    /// The usage of a planned team could not be read, so `spentUsd` is a lower bound and `state` can only
    /// be too low. Left out when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, type = "boolean")]
    pub partial: bool,
}

/// `GET /api/orgs/<org>/teams/<team>/budget`, and what `agency budget status` prints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BudgetStatus {
    /// The Monday of the week, in the time zone of the node that answers.
    pub week_start: Date,
    pub team: BudgetPart,
    pub org: OrgBudget,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const SAMPLE: &str = include_str!("../../../testdata/budget.sample.json");

    #[test]
    fn the_sample_survives_a_round_trip_and_leaves_out_what_has_no_value() {
        let status: BudgetStatus = serde_json::from_str(SAMPLE).unwrap();
        let again = serde_json::to_value(&status).unwrap();
        assert_eq!(again, serde_json::from_str::<Value>(SAMPLE).unwrap());
        assert!(
            !SAMPLE.contains("null"),
            "an optional field is left out, never null"
        );
    }

    #[test]
    fn a_state_is_written_in_lower_case_and_partial_only_when_set() {
        let status: BudgetStatus = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(status.team.state, BudgetState::Warning);
        assert!(!status.team.partial && status.org.partial);
        let written = serde_json::to_value(&status).unwrap();
        assert_eq!(written["team"]["state"], "warning");
        assert!(written["team"].get("partial").is_none());
        assert_eq!(written["org"]["partial"], true);
    }
}
