//! One number for each calendar day of a team's delivery, for what reads a day at a time (a KPI that takes
//! its numbers from the node's delivery numbers). They are the numbers of `evaluate`, cut by day with the
//! same rules, so a day's number and the evaluation's `byDay` agree.
use super::evaluate::{tally, Day, Input};
use super::participants::round_to_cents;
use agency_proto::clock::Duration;
use agency_proto::evaluation::median;
use agency_proto::Date;

/// What one calendar day of delivery came to. A day with nothing in it has a count of 0 and no median: a
/// median of no samples is no number, never 0.
#[derive(Clone, Debug, PartialEq)]
pub struct DayNumbers {
    pub date: Date,
    /// The new features merged that day (`NewFeatureSummary.merged` for the day).
    pub new_features_merged: u32,
    pub new_feature_pr_lead_median_seconds: Option<u64>,
    pub pr_lead_median_seconds: Option<u64>,
    pub main_red_seconds: u64,
    /// The day's cost over the pull requests merged that day (reverts too), as the summary has it; none when
    /// nothing merged.
    pub cost_per_merged_pr_usd: Option<f64>,
}

fn numbers_of(date: Date, day: &Day) -> DayNumbers {
    let merged = day.merged;
    let pull_requests = merged.feat
        + merged.fix
        + merged.refactor
        + merged.docs
        + merged.test
        + merged.chore
        + merged.other
        + day.reverts;
    DayNumbers {
        date,
        new_features_merged: day.features_merged,
        new_feature_pr_lead_median_seconds: median(&day.feature_pr_leads),
        pr_lead_median_seconds: median(&day.pr_leads),
        main_red_seconds: day.red_seconds,
        cost_per_merged_pr_usd: (pull_requests > 0)
            .then(|| round_to_cents(day.cost_usd / f64::from(pull_requests))),
    }
}

/// The numbers of each calendar day of the last `input.days` days, oldest first, the days with nothing in
/// them included.
pub fn day_numbers(input: &Input) -> Vec<DayNumbers> {
    let (period, days, _) = tally(input);
    let first = super::period::calendar_date(&period.first());
    let count = i64::from(input.days.max(1));
    (0..count)
        .map(|offset| {
            let date = super::period::proto_date(first + Duration::days(offset));
            let day = days.get(date.as_str()).map(|day| numbers_of(date.clone(), day));
            day.unwrap_or_else(|| numbers_of(date, &Day::default()))
        })
        .collect()
}
