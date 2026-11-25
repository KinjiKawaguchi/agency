//! Token usage of one team, as its runtime reports it (`tokenUsage` in agency-runtime/1) and as nodes and
//! clients pass it on. Costs are estimates at API prices, in US dollars rounded to cents by the runtime.
//! They are the one place where the protocol carries JSON numbers that are not integers.
use crate::ids::Id;
use crate::timestamp::{Date, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// The longest range a report may cover, in days.
pub const MAX_USAGE_DAYS: u32 = 90;
/// The range the Client API uses when the client does not ask for one.
pub const DEFAULT_USAGE_DAYS: u32 = 14;

/// Token use of one team over the last `days` calendar days, today included.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TokenUsage {
    pub org: Id,
    pub team: Id,
    pub days: u32,
    pub generated_at: Timestamp,
    /// Oldest day first; days without use may be left out.
    pub by_day: Vec<DayCost>,
    pub by_person: Vec<PersonUsage>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DayCost {
    pub date: Date,
    /// Estimated cost in USD, keyed by the person's name in the team runtime.
    pub cost_usd: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PersonUsage {
    /// The person's name in the team runtime.
    pub person: String,
    /// Estimated cost in USD over the whole range.
    pub cost_usd: f64,
    pub tokens: TokenCounts,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TokenCounts {
    #[ts(type = "number")]
    pub input: u64,
    #[ts(type = "number")]
    pub output: u64,
    #[ts(type = "number")]
    pub cache_read: u64,
    #[ts(type = "number")]
    pub cache_write: u64,
}

/// Whether `days` is a range a report may cover.
pub fn valid_days(days: u32) -> bool {
    (1..=MAX_USAGE_DAYS).contains(&days)
}

fn valid_cost(cost: f64) -> bool {
    cost.is_finite() && cost >= 0.0
}

impl TokenUsage {
    /// Checks that this is a well-formed report of the team and range that were asked for.
    pub fn check(&self, org: &Id, team: &Id, days: u32) -> Result<(), String> {
        if &self.org != org || &self.team != team || self.days != days {
            return Err(format!(
                "reported usage of {}/{} over {} days instead of {org}/{team} over {days} days",
                self.org, self.team, self.days
            ));
        }
        if self.by_day.len() > days as usize {
            return Err(format!("reported more than {days} days"));
        }
        let ascending = self
            .by_day
            .windows(2)
            .all(|pair| pair[0].date.as_str() < pair[1].date.as_str());
        if !ascending {
            return Err("reported days out of order".into());
        }
        let day_costs = self.by_day.iter().flat_map(|day| day.cost_usd.iter());
        let person_costs = self
            .by_person
            .iter()
            .map(|person| (&person.person, &person.cost_usd));
        for (person, cost) in day_costs.chain(person_costs) {
            if person.is_empty() {
                return Err("reported a person without a name".into());
            }
            if !valid_cost(*cost) {
                return Err(format!("reported a malformed cost for {person}"));
            }
        }
        Ok(())
    }
}
