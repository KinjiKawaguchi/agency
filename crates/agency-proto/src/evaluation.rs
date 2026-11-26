//! How fast a team's work ships, worked out by the team's node from its pull requests on GitHub and its
//! token usage (docs/protocol.md, "Evaluation"). Counts and seconds are integers; costs are estimates at
//! API prices in US dollars, as in `usage`. An optional field is left out, never null.
use crate::ids::Id;
use crate::timestamp::{Date, Timestamp};
use crate::usage::valid_days;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Merged pull requests by the Conventional Commits type of their titles. Anything that is not one of
/// the six types is `other`. A pull request that reverts another is counted in `reverts`, not here, so
/// the pull requests merged are these counts and `reverts` together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MergedCounts {
    pub feat: u32,
    pub fix: u32,
    pub refactor: u32,
    pub docs: u32,
    pub test: u32,
    pub chore: u32,
    pub other: u32,
}

/// One calendar day in the node's time zone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DayEvaluation {
    pub date: Date,
    pub merged: MergedCounts,
    /// Pull requests merged that day that revert another.
    pub reverts: u32,
    /// Seconds main's CI was red that day; a span across midnight is split between the days.
    #[ts(type = "number")]
    pub main_red_seconds: u64,
    /// Estimated cost of that day's tokens.
    pub cost_usd: f64,
    /// Pull request opened to merged, one for each pull request merged that day.
    #[ts(type = "Array<number>")]
    pub pr_lead_seconds: Vec<u64>,
    /// Issue started (its first move to In Progress on the board, else the pull request's first commit)
    /// to merged, one for each merged pull request that has an issue.
    #[ts(type = "Array<number>")]
    pub issue_lead_seconds: Vec<u64>,
}

/// The numbers for the whole period, worked out by the node so that a screen and an agent that cites
/// them agree. A median or p90 is left out when there are no samples. The p90 is the nearest rank
/// (`sorted[ceil(0.9 n) - 1]`); the median of an even count is the mean of the two middle samples,
/// rounded down to a whole second.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct EvaluationSummary {
    pub merged: MergedCounts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number")]
    pub pr_lead_median_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number")]
    pub pr_lead_p90_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number")]
    pub issue_lead_median_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number")]
    pub issue_lead_p90_seconds: Option<u64>,
    #[ts(type = "number")]
    pub main_red_seconds: u64,
    /// The longest single span main was red in the period.
    #[ts(type = "number")]
    pub longest_red_seconds: u64,
    pub reverts: u32,
    /// The period's cost divided by the pull requests merged in it (`merged` and `reverts`); the token
    /// usage is per day, so there is no per-pull-request attribution. Left out when nothing merged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_per_merged_pr_usd: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeamEvaluation {
    pub team: Id,
    pub summary: EvaluationSummary,
    /// Oldest day first; days with nothing in them may be left out.
    pub by_day: Vec<DayEvaluation>,
}

/// A team the node could not get numbers for. The reason is plain text with no secret and no address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeamUnavailable {
    pub team: Id,
    pub reason: String,
}

/// `GET /api/orgs/<org>/evaluation?days=<n>`: every team of the org. The federation route answers the
/// same type with one team in `teams` (or the team in `unavailable`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Evaluation {
    pub org: Id,
    pub days: u32,
    /// The period's first and last calendar day in the node's time zone, written like `byDay[].date`.
    pub from: Date,
    pub until: Date,
    pub generated_at: Timestamp,
    pub teams: Vec<TeamEvaluation>,
    /// Teams that could not answer, so that one of them does not blank the others.
    pub unavailable: Vec<TeamUnavailable>,
}

fn valid_cost(cost: f64) -> bool {
    cost.is_finite() && cost >= 0.0
}

/// Checks the days of a team's report: in order, inside the period, at most `days` of them, and every
/// cost a number that is not negative.
pub(crate) fn check_period(
    dates: &[&Date],
    from: &Date,
    until: &Date,
    costs: &[f64],
    days: u32,
) -> Result<(), String> {
    if dates
        .windows(2)
        .any(|pair| pair[0].as_str() >= pair[1].as_str())
    {
        return Err("reported days out of order".into());
    }
    if dates
        .iter()
        .any(|date| date.as_str() < from.as_str() || date.as_str() > until.as_str())
    {
        return Err("reported a day outside the period".into());
    }
    if dates.len() > days as usize {
        return Err(format!("reported more than {days} days"));
    }
    if costs.iter().any(|cost| !valid_cost(*cost)) {
        return Err("reported a malformed cost".into());
    }
    Ok(())
}

impl Evaluation {
    /// Checks that this is a well-formed report of the org and range that were asked for.
    pub fn check(&self, org: &Id, days: u32) -> Result<(), String> {
        if &self.org != org || self.days != days || !valid_days(days) {
            return Err(format!(
                "reported {} over {} days instead of {org} over {days} days",
                self.org, self.days
            ));
        }
        if self.from.as_str() > self.until.as_str() {
            return Err("reported a period that ends before it starts".into());
        }
        let teams: Vec<&Id> = self
            .teams
            .iter()
            .map(|team| &team.team)
            .chain(self.unavailable.iter().map(|team| &team.team))
            .collect();
        let mut unique = teams.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != teams.len() {
            return Err("reported a team twice".into());
        }
        for team in &self.teams {
            let dates: Vec<&Date> = team.by_day.iter().map(|day| &day.date).collect();
            let mut costs: Vec<f64> = team.by_day.iter().map(|day| day.cost_usd).collect();
            costs.extend(team.summary.cost_per_merged_pr_usd);
            check_period(&dates, &self.from, &self.until, &costs, days)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const SAMPLE: &str = include_str!("../../../testdata/evaluation.sample.json");

    fn sample() -> Evaluation {
        serde_json::from_str(SAMPLE).unwrap()
    }

    fn acme() -> Id {
        Id::parse("acme").unwrap()
    }

    #[test]
    fn the_sample_survives_a_round_trip_and_leaves_out_what_has_no_value() {
        let again = serde_json::to_value(sample()).unwrap();
        assert_eq!(again, serde_json::from_str::<Value>(SAMPLE).unwrap());
        let quiet = &again["teams"][1]["summary"];
        assert!(
            quiet.get("prLeadMedianSeconds").is_none() && quiet.get("costPerMergedPrUsd").is_none()
        );
        assert!(
            !SAMPLE.contains("null"),
            "an optional field is left out, never null"
        );
    }

    #[test]
    fn accepts_the_sample_and_refuses_reports_that_are_not_what_was_asked_for() {
        assert_eq!(sample().check(&acme(), 14), Ok(()));
        assert!(sample().check(&Id::parse("other").unwrap(), 14).is_err());
        assert!(sample().check(&acme(), 7).is_err());
        assert!(sample().check(&acme(), 91).is_err());
    }

    #[test]
    fn refuses_a_malformed_report() {
        let refused = |change: &dyn Fn(&mut Evaluation)| {
            let mut report = sample();
            change(&mut report);
            report.check(&acme(), 14).unwrap_err()
        };
        assert!(refused(&|r| r.teams[0].by_day.reverse()).contains("out of order"));
        assert!(
            refused(&|r| r.teams[0].by_day[1].date = Date::parse("2026-12-01").unwrap())
                .contains("outside")
        );
        assert!(refused(&|r| r.teams[0].by_day[0].cost_usd = -1.0).contains("malformed cost"));
        assert!(refused(&|r| r.teams[0].by_day[0].cost_usd = f64::NAN).contains("malformed cost"));
        assert!(
            refused(&|r| r.teams[0].summary.cost_per_merged_pr_usd = Some(f64::INFINITY))
                .contains("malformed cost")
        );
        assert!(refused(&|r| r.unavailable[0].team = Id::parse("app").unwrap()).contains("twice"));
        assert!(refused(&|r| std::mem::swap(&mut r.from, &mut r.until)).contains("ends before"));
    }
}
