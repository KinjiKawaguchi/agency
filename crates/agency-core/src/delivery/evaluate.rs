//! A team's delivery numbers over a period, worked out from facts (docs/protocol.md, "Evaluation").
use super::facts::{MainRun, MergedPullRequest};
use super::kinds::{classify, count, Kind};
use super::period::Period;
use super::red::red_spans;
use agency_proto::clock::{OffsetDateTime, UtcOffset};
use agency_proto::evaluation::{
    median, p90, DayEvaluation, EvaluationSummary, MergedCounts, TeamEvaluation,
};
use agency_proto::{Date, Id};
use std::collections::BTreeMap;

/// Everything one team's numbers are worked out from.
pub struct Input<'a> {
    pub now: OffsetDateTime,
    /// The node's time zone: the days are its calendar days.
    pub offset: UtcOffset,
    pub days: u32,
    pub pull_requests: &'a [MergedPullRequest],
    pub runs: &'a [MainRun],
    /// The cost of each day's tokens as the team's token usage reports it; days without use may be left out.
    pub cost_by_day: &'a [(Date, f64)],
}

/// One calendar day, as it is added up.
#[derive(Default)]
struct Day {
    merged: MergedCounts,
    reverts: u32,
    red_seconds: u64,
    cost_usd: f64,
    pr_leads: Vec<u64>,
    issue_leads: Vec<u64>,
}

impl Day {
    fn is_empty(&self) -> bool {
        let merged = self.merged;
        let any_merged = [
            merged.feat,
            merged.fix,
            merged.refactor,
            merged.docs,
            merged.test,
            merged.chore,
            merged.other,
        ]
        .iter()
        .any(|count| *count > 0);
        !any_merged && self.reverts == 0 && self.red_seconds == 0 && self.cost_usd == 0.0
    }
}

fn seconds_between(from: OffsetDateTime, until: OffsetDateTime) -> u64 {
    (until - from).whole_seconds().max(0).unsigned_abs()
}

/// When the work on a pull request's issue started: its first move to In Progress, or else the pull
/// request's first commit. A start after the merge is no start.
fn issue_started(pull_request: &MergedPullRequest) -> Option<OffsetDateTime> {
    let before_merge = |at: &OffsetDateTime| *at <= pull_request.merged_at;
    pull_request
        .issue_started_at
        .filter(before_merge)
        .or(pull_request.first_commit_at.filter(before_merge))
}

fn add_pull_requests(days: &mut BTreeMap<String, Day>, period: &Period, input: &Input) {
    let mut merged: Vec<&MergedPullRequest> = input
        .pull_requests
        .iter()
        .filter(|pull_request| period.contains(pull_request.merged_at))
        .collect();
    merged.sort_by_key(|pull_request| pull_request.merged_at);
    for pull_request in merged {
        let day = days
            .entry(period.day_of(pull_request.merged_at).to_string())
            .or_default();
        match classify(&pull_request.title, &pull_request.body) {
            Kind::Revert => day.reverts += 1,
            kind => count(&mut day.merged, kind),
        }
        day.pr_leads.push(seconds_between(
            pull_request.opened_at,
            pull_request.merged_at,
        ));
        if pull_request.has_issue {
            if let Some(started) = issue_started(pull_request) {
                day.issue_leads
                    .push(seconds_between(started, pull_request.merged_at));
            }
        }
    }
}

/// Adds the red time of each day, and returns the longest single span inside the period.
fn add_red(days: &mut BTreeMap<String, Day>, period: &Period, input: &Input) -> u64 {
    let mut longest = 0;
    for (from, until) in red_spans(input.runs, input.now) {
        let by_day = period.seconds_by_day(from, until);
        longest = longest.max(by_day.iter().map(|(_, seconds)| seconds).sum());
        for (day, seconds) in by_day {
            days.entry(day.to_string()).or_default().red_seconds += seconds;
        }
    }
    longest
}

fn add_costs(days: &mut BTreeMap<String, Day>, period: &Period, input: &Input) {
    let (first, last) = (period.first(), period.last());
    for (date, cost) in input.cost_by_day {
        if date.as_str() >= first.as_str() && date.as_str() <= last.as_str() {
            days.entry(date.as_str().to_owned()).or_default().cost_usd += cost;
        }
    }
}

fn round_to_cents(cost: f64) -> f64 {
    (cost * 100.0).round() / 100.0
}

fn summarize(days: &BTreeMap<String, Day>, longest_red_seconds: u64) -> EvaluationSummary {
    let mut merged = MergedCounts::default();
    let (mut reverts, mut red, mut cost) = (0, 0, 0.0);
    let (mut pr_leads, mut issue_leads) = (vec![], vec![]);
    for day in days.values() {
        merged.feat += day.merged.feat;
        merged.fix += day.merged.fix;
        merged.refactor += day.merged.refactor;
        merged.docs += day.merged.docs;
        merged.test += day.merged.test;
        merged.chore += day.merged.chore;
        merged.other += day.merged.other;
        reverts += day.reverts;
        red += day.red_seconds;
        cost += day.cost_usd;
        pr_leads.extend(&day.pr_leads);
        issue_leads.extend(&day.issue_leads);
    }
    let pull_requests = merged.feat
        + merged.fix
        + merged.refactor
        + merged.docs
        + merged.test
        + merged.chore
        + merged.other
        + reverts;
    EvaluationSummary {
        merged,
        pr_lead_median_seconds: median(&pr_leads),
        pr_lead_p90_seconds: p90(&pr_leads),
        issue_lead_median_seconds: median(&issue_leads),
        issue_lead_p90_seconds: p90(&issue_leads),
        main_red_seconds: red,
        longest_red_seconds,
        reverts,
        cost_per_merged_pr_usd: (pull_requests > 0)
            .then(|| round_to_cents(cost / f64::from(pull_requests))),
    }
}

/// The team's delivery numbers over the last `input.days` calendar days.
pub fn evaluate(team: Id, input: &Input) -> TeamEvaluation {
    let period = Period::new(input.now, input.offset, input.days);
    let mut days = BTreeMap::new();
    add_pull_requests(&mut days, &period, input);
    let longest = add_red(&mut days, &period, input);
    add_costs(&mut days, &period, input);
    let summary = summarize(&days, longest);
    let by_day = days
        .into_iter()
        .filter(|(_, day)| !day.is_empty())
        .map(|(date, day)| DayEvaluation {
            date: Date::parse(&date).expect("a calendar date is written as YYYY-MM-DD"),
            merged: day.merged,
            reverts: day.reverts,
            main_red_seconds: day.red_seconds,
            cost_usd: round_to_cents(day.cost_usd),
            pr_lead_seconds: day.pr_leads,
            issue_lead_seconds: day.issue_leads,
        })
        .collect();
    TeamEvaluation {
        team,
        from: period.first(),
        until: period.last(),
        summary,
        by_day,
    }
}
