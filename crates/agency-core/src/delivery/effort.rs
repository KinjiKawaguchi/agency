//! Where a team's tokens went, by work type (docs/protocol.md, "Evaluation", `Effort`). The runtime
//! reports cost by day, so a day's cost is shared out among the work that merged: an estimate, said so
//! in the protocol, and a rule that needs nothing the runtime does not have.
use super::facts::MergedPullRequest;
use super::kinds::{classify, Kind};
use super::period::Period;
use agency_proto::effort::{ByWorkType, DayEffort, TeamEffort, TypeUsage};
use agency_proto::usage::TokenCounts;
use agency_proto::{Date, Id};
use std::collections::BTreeMap;

/// What a team's effort is worked out from.
pub struct EffortInput<'a> {
    pub period: Period,
    pub pull_requests: &'a [MergedPullRequest],
    /// The cost of each day's tokens as the team's token usage reports it.
    pub cost_by_day: &'a [(Date, f64)],
    /// The team's tokens over the whole period.
    pub tokens: TokenCounts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    NewFeature,
    Maintenance,
    Debt,
    Incident,
    Unclassified,
}

const SLOTS: [Slot; 5] = [
    Slot::NewFeature,
    Slot::Maintenance,
    Slot::Debt,
    Slot::Incident,
    Slot::Unclassified,
];

fn get<T>(by: &ByWorkType<T>, slot: Slot) -> &T {
    match slot {
        Slot::NewFeature => &by.new_feature,
        Slot::Maintenance => &by.maintenance,
        Slot::Debt => &by.debt,
        Slot::Incident => &by.incident,
        Slot::Unclassified => &by.unclassified,
    }
}

fn get_mut<T>(by: &mut ByWorkType<T>, slot: Slot) -> &mut T {
    match slot {
        Slot::NewFeature => &mut by.new_feature,
        Slot::Maintenance => &mut by.maintenance,
        Slot::Debt => &mut by.debt,
        Slot::Incident => &mut by.incident,
        Slot::Unclassified => &mut by.unclassified,
    }
}

/// The work type a merged pull request has without the board saying: its title's type, and incident for a
/// fix of an issue labeled `incident`. A revert and a title of no known type are
/// unclassified.
fn slot_of(pull_request: &MergedPullRequest) -> Slot {
    match classify(&pull_request.title, &pull_request.body) {
        Kind::Feat => Slot::NewFeature,
        Kind::Fix if pull_request.closes_incident => Slot::Incident,
        Kind::Refactor => Slot::Debt,
        Kind::Fix | Kind::Docs | Kind::Test | Kind::Chore => Slot::Maintenance,
        Kind::Other | Kind::Revert => Slot::Unclassified,
    }
}

/// Whether the pull request is a new feature by the rule of "Work type": the one the effort split uses, so
/// that the lead time of new features and the effort spent on them speak of the same pull requests.
pub(super) fn is_new_feature(pull_request: &MergedPullRequest) -> bool {
    slot_of(pull_request) == Slot::NewFeature
}

/// How many pull requests of each type merged, in the order of `SLOTS`: the weights a cost is shared out by.
type Weights = [u32; SLOTS.len()];

fn index_of(slot: Slot) -> usize {
    SLOTS.iter().position(|each| *each == slot).unwrap_or(0)
}

/// A cost shared out by the weights, one share for each pull request. No weights put it all in
/// `unclassified`: the cost happened and nothing says what it was for.
fn share(cost: f64, weights: &Weights) -> ByWorkType<f64> {
    let mut shared = ByWorkType::<f64>::default();
    let total: u32 = weights.iter().sum();
    if total == 0 {
        shared.unclassified = cost;
        return shared;
    }
    for (slot, count) in SLOTS.iter().zip(weights) {
        *get_mut(&mut shared, *slot) = cost * f64::from(*count) / f64::from(total);
    }
    shared
}

/// The period's tokens split in the proportion of the cost of each type, rounded down, and what rounding
/// left over goes to `unclassified`, so that the types add up to the team's tokens as the costs do. The
/// runtime reports tokens for the whole period only, so this is an estimate like the cost.
fn split_tokens(tokens: TokenCounts, costs: &ByWorkType<f64>) -> ByWorkType<TokenCounts> {
    let total: f64 = costs.all().into_iter().sum();
    let mut split = ByWorkType::<TokenCounts>::default();
    if total <= 0.0 {
        split.unclassified = tokens;
        return split;
    }
    let part = |count: u64, cost: f64| (count as f64 * cost / total).floor() as u64;
    for slot in SLOTS {
        let cost = *get(costs, slot);
        *get_mut(&mut split, slot) = TokenCounts {
            input: part(tokens.input, cost),
            output: part(tokens.output, cost),
            cache_read: part(tokens.cache_read, cost),
            cache_write: part(tokens.cache_write, cost),
        };
    }
    let kept = |count: fn(&TokenCounts) -> u64| {
        SLOTS
            .iter()
            .map(|slot| count(get(&split, *slot)))
            .sum::<u64>()
    };
    let left = TokenCounts {
        input: tokens.input.saturating_sub(kept(|t| t.input)),
        output: tokens.output.saturating_sub(kept(|t| t.output)),
        cache_read: tokens.cache_read.saturating_sub(kept(|t| t.cache_read)),
        cache_write: tokens.cache_write.saturating_sub(kept(|t| t.cache_write)),
    };
    let unclassified = &mut split.unclassified;
    unclassified.input += left.input;
    unclassified.output += left.output;
    unclassified.cache_read += left.cache_read;
    unclassified.cache_write += left.cache_write;
    split
}

fn sum(by_day: &[DayEffort]) -> ByWorkType<f64> {
    let mut total = ByWorkType::<f64>::default();
    for day in by_day {
        for slot in SLOTS {
            *get_mut(&mut total, slot) += *get(&day.cost_usd, slot);
        }
    }
    total
}

/// The weights of each day on which something merged, by date.
fn weights_by_day(period: &Period, merged: &[&MergedPullRequest]) -> BTreeMap<String, Weights> {
    let mut by_day: BTreeMap<String, Weights> = BTreeMap::new();
    for pull_request in merged {
        let day = period.day_of(pull_request.merged_at).to_string();
        by_day.entry(day).or_default()[index_of(slot_of(pull_request))] += 1;
    }
    by_day
}

/// A team's effort over a period: each day's cost shared out among the types of the pull requests merged
/// that day. A day nothing merged on is the work of the next merges, so it is shared like the next day of
/// the period that has some, and the days after the last merge are `unclassified`: work not merged yet.
/// A day without cost is left out.
pub fn effort(team: Id, input: &EffortInput) -> TeamEffort {
    let merged: Vec<&MergedPullRequest> = input
        .pull_requests
        .iter()
        .filter(|pull_request| input.period.contains(pull_request.merged_at))
        .collect();
    let merges = weights_by_day(&input.period, &merged);
    let mut costs: BTreeMap<String, f64> = BTreeMap::new();
    for (date, cost) in input.cost_by_day {
        let inside = date.as_str() >= input.period.first().as_str()
            && date.as_str() <= input.period.last().as_str();
        if inside && *cost > 0.0 {
            *costs.entry(date.as_str().to_owned()).or_default() += cost;
        }
    }
    let by_day: Vec<DayEffort> = costs
        .into_iter()
        .map(|(date, cost)| {
            let next = merges.range(date.clone()..).next();
            DayEffort {
                date: Date::parse(&date).expect("a day of the period is a date"),
                cost_usd: share(
                    cost,
                    next.map(|(_, weights)| weights)
                        .unwrap_or(&Weights::default()),
                ),
            }
        })
        .collect();
    let totals = sum(&by_day);
    let tokens = split_tokens(input.tokens, &totals);
    let mut by_type = ByWorkType::<TypeUsage>::default();
    for slot in SLOTS {
        *get_mut(&mut by_type, slot) = TypeUsage {
            cost_usd: *get(&totals, slot),
            tokens: *get(&tokens, slot),
        };
    }
    TeamEffort {
        team,
        from: input.period.first(),
        until: input.period.last(),
        by_type,
        by_day,
    }
}
