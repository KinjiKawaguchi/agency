//! Where a team's tokens went, by the kind of work (docs/protocol.md, "Evaluation"): the cost and token
//! counts of each team split by work type. The intended allocation is not here; it is the org
//! definition's team plan. Costs are estimates at API prices in US dollars, as in `usage`.
use crate::evaluation::{check_answer, check_days, Answer, TeamUnavailable};
use crate::ids::Id;
use crate::timestamp::{Date, Timestamp};
use crate::usage::TokenCounts;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A value for each work type, and for the work nothing could classify. The work type is the board's
/// "Work type" field; without it, the pull request's type says (`feat` is new feature, `refactor` is
/// debt, `fix`, `docs`, `test`, and `chore` are maintenance, and `fix` of an issue labeled `incident` is
/// incident); the rest is unclassified.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ByWorkType<T> {
    pub new_feature: T,
    pub improvement: T,
    pub maintenance: T,
    pub debt: T,
    pub incident: T,
    pub unclassified: T,
}

impl<T> ByWorkType<T> {
    pub fn all(&self) -> [&T; 6] {
        [
            &self.new_feature,
            &self.improvement,
            &self.maintenance,
            &self.debt,
            &self.incident,
            &self.unclassified,
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TypeUsage {
    pub cost_usd: f64,
    pub tokens: TokenCounts,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DayEffort {
    pub date: Date,
    pub cost_usd: ByWorkType<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeamEffort {
    pub team: Id,
    /// The period's first and last calendar day in the time zone of the team's node, as on
    /// `TeamEvaluation`.
    pub from: Date,
    pub until: Date,
    pub by_type: ByWorkType<TypeUsage>,
    /// Oldest day first; days with nothing in them may be left out.
    pub by_day: Vec<DayEffort>,
}

/// `GET /api/orgs/<org>/effort?days=<n>`; the federation route answers the same type with one team.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Effort {
    pub org: Id,
    pub days: u32,
    /// The period's first and last calendar day in the time zone of the node that answers, written like
    /// `byDay[].date`. A team on a node in another time zone has its own `from` and `until`.
    pub from: Date,
    pub until: Date,
    pub generated_at: Timestamp,
    pub teams: Vec<TeamEffort>,
    pub unavailable: Vec<TeamUnavailable>,
}

impl Effort {
    /// Checks that this is a well-formed report of the org and range that were asked for.
    pub fn check(&self, org: &Id, days: u32) -> Result<(), String> {
        let answer = Answer {
            org: &self.org,
            days: self.days,
            from: &self.from,
            until: &self.until,
            teams: self.teams.iter().map(|team| &team.team).collect(),
            unavailable: &self.unavailable,
        };
        check_answer(org, days, &answer)?;
        for team in &self.teams {
            let dates: Vec<&Date> = team.by_day.iter().map(|day| &day.date).collect();
            let costs: Vec<f64> = team
                .by_type
                .all()
                .iter()
                .map(|usage| usage.cost_usd)
                .chain(
                    team.by_day
                        .iter()
                        .flat_map(|day| day.cost_usd.all().into_iter().copied()),
                )
                .collect();
            check_days(&dates, &team.from, &team.until, &costs, days)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const SAMPLE: &str = include_str!("../../../testdata/effort.sample.json");

    fn sample() -> Effort {
        serde_json::from_str(SAMPLE).unwrap()
    }

    #[test]
    fn the_sample_survives_a_round_trip() {
        let again = serde_json::to_value(sample()).unwrap();
        assert_eq!(again, serde_json::from_str::<Value>(SAMPLE).unwrap());
    }

    #[test]
    fn accepts_the_sample_and_refuses_a_malformed_report() {
        let acme = Id::parse("acme").unwrap();
        assert_eq!(sample().check(&acme, 14), Ok(()));
        let mut negative = sample();
        negative.teams[0].by_type.debt.cost_usd = -0.5;
        assert!(negative
            .check(&acme, 14)
            .unwrap_err()
            .contains("malformed cost"));
        let mut late = sample();
        late.teams[0].by_day[1].date = Date::parse("2026-12-25").unwrap();
        assert!(late.check(&acme, 14).unwrap_err().contains("outside"));
        let mut days = sample();
        days.teams[0].by_day[0].cost_usd.debt = f64::NAN;
        assert!(days.check(&acme, 14).is_err());
        assert!(sample().check(&acme, 30).is_err());
    }

    #[test]
    fn refuses_what_the_evaluation_check_refuses() {
        let acme = Id::parse("acme").unwrap();
        let mut backwards = sample();
        std::mem::swap(&mut backwards.from, &mut backwards.until);
        assert!(backwards
            .check(&acme, 14)
            .unwrap_err()
            .contains("ends before"));
        let mut twice = sample();
        twice.unavailable.push(TeamUnavailable {
            team: twice.teams[0].team.clone(),
            reason: "x".into(),
        });
        assert!(twice.check(&acme, 14).unwrap_err().contains("twice"));
        let mut long = sample();
        long.until = Date::parse("2030-01-01").unwrap();
        assert!(long
            .check(&acme, 14)
            .unwrap_err()
            .contains("not 14 days long"));
        let mut team = sample();
        let own = &mut team.teams[0];
        std::mem::swap(&mut own.from, &mut own.until);
        assert!(team.check(&acme, 14).unwrap_err().contains("ends before"));
    }
}
