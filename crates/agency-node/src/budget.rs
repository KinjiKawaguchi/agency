//! The week-sized part of a team's runtime usage report.
use agency_core::budget::BudgetStatus;
use agency_proto::{Date, TokenUsage};
use anyhow::Result;

/// Adds the daily costs reported from the start of this week through today, inclusive.
///
/// Runtime reports can include an earlier Sunday when the command runs on Monday, so the caller gives
/// both bounds rather than relying on the number of days requested.
pub fn week_spend(usage: &TokenUsage, start: &Date, today: &Date) -> f64 {
    usage
        .by_day
        .iter()
        .filter(|day| day.date.as_str() >= start.as_str() && day.date.as_str() <= today.as_str())
        .flat_map(|day| day.cost_usd.values())
        .sum()
}

/// Serializes the complete command answer as its only stdout line.
pub fn json_line(status: &BudgetStatus) -> Result<String> {
    Ok(serde_json::to_string(status)?)
}
