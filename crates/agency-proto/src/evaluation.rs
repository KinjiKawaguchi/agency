//! How fast a team's work ships, worked out by the team's node from its pull requests on GitHub and its
//! token usage (docs/protocol.md, "Evaluation"). Counts and seconds are integers; costs are estimates at
//! API prices in US dollars, as in `usage`. An optional field is left out, never null.
use crate::ids::Id;
use crate::participant_metrics::{MAX_NAME_BYTES, MAX_PARTICIPANTS};
use crate::timestamp::{Date, Timestamp};
use crate::usage::{valid_days, TokenCounts};
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

/// One participant's numbers over the period, added up from the team runtime's `participantMetrics` and
/// the pull requests GitHub says were merged (docs/protocol.md, "Participant metrics").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct ParticipantEvaluation {
    /// The participant's name in the team runtime.
    pub name: String,
    /// Seconds in the period at least one of the participant's sessions was working, was waiting for
    /// a human's decision, or was alive and neither.
    #[ts(type = "number")]
    pub busy_seconds: u64,
    #[ts(type = "number")]
    pub waiting_seconds: u64,
    #[ts(type = "number")]
    pub idle_seconds: u64,
    /// Estimated cost of the participant's tokens, rounded to cents.
    pub cost_usd: f64,
    pub tokens: TokenCounts,
    /// The pull requests merged in the period (`merged` and `reverts` of the summary together) whose
    /// author the runtime named this participant.
    pub merged: u32,
    /// `costUsd` divided by `merged`; left out when the participant merged nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_per_merged_pr_usd: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeamEvaluation {
    pub team: Id,
    /// The period's first and last calendar day in the time zone of the team's node, written like
    /// `byDay[].date`. Every day in `byDay` lies inside it, so a team is drawn against its own period.
    pub from: Date,
    pub until: Date,
    pub summary: EvaluationSummary,
    /// Oldest day first; days with nothing in them may be left out.
    pub by_day: Vec<DayEvaluation>,
    /// The team's participants, by name, when its runtime reports them: none otherwise (also from a node
    /// that does not know of them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<ParticipantEvaluation>>", optional)]
    pub participants: Vec<ParticipantEvaluation>,
    /// The pull requests merged in the period that no participant is named the author of. The merged pull
    /// requests of `participants` and these add up to the summary's `merged` and `reverts`. Left out when
    /// it is 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[ts(as = "Option<u32>", optional)]
    pub unattributed_merged: u32,
    /// The summary of each repository that had a merged pull request or a CI run in the period, by name; left
    /// out unless at least two had, since one would repeat the team's summary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<RepositoryEvaluation>>", optional)]
    pub by_repository: Vec<RepositoryEvaluation>,
}

fn is_zero(count: &u32) -> bool {
    *count == 0
}

/// One repository's numbers over the period: the summary of the pull requests merged into it and of the CI
/// runs on its default branch alone. The cost is the team's tokens by day, which no repository owns, so
/// `costPerMergedPrUsd` is always left out here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepositoryEvaluation {
    /// `owner/name`.
    pub repo: String,
    pub summary: EvaluationSummary,
}

/// The most repositories a team's report breaks its numbers down by, and the longest repository name.
pub const MAX_REPOSITORIES: usize = 50;
pub const MAX_REPOSITORY_BYTES: usize = 140;

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
    /// The period's first and last calendar day in the time zone of the node that answers, written like
    /// `byDay[].date`. A team on a node in another time zone has its own `from` and `until`.
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

/// The median of the samples: the middle one, or for an even count the mean of the two middle ones,
/// rounded down to a whole second. `None` without samples.
pub fn median(samples: &[u64]) -> Option<u64> {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let middle = sorted.len() / 2;
    match sorted.len() {
        0 => None,
        count if count % 2 == 1 => Some(sorted[middle]),
        _ => {
            let (low, high) = (sorted[middle - 1], sorted[middle]);
            Some(low / 2 + high / 2 + (low % 2 + high % 2) / 2)
        }
    }
}

/// The p90 by nearest rank: `sorted[ceil(0.9 n) - 1]`. `None` without samples.
pub fn p90(samples: &[u64]) -> Option<u64> {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = (9 * sorted.len()).div_ceil(10);
    rank.checked_sub(1).map(|index| sorted[index])
}

/// The number of calendar days from `from` to `until`, both included.
fn period_days(from: &Date, until: &Date) -> Option<i64> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    let day = |date: &Date| time::Date::parse(date.as_str(), &format).ok();
    Some(i64::from(day(until)?.to_julian_day() - day(from)?.to_julian_day()) + 1)
}

/// The days an answer says it covers: `from` is not after `until`, and the period is `days` long.
pub(crate) fn check_period(from: &Date, until: &Date, days: u32) -> Result<(), String> {
    if from.as_str() > until.as_str() {
        return Err("reported a period that ends before it starts".into());
    }
    if period_days(from, until) != Some(i64::from(days)) {
        return Err(format!("reported a period that is not {days} days long"));
    }
    Ok(())
}

/// What `Evaluation` and `Effort` have in common, for one check.
pub(crate) struct Answer<'a> {
    pub org: &'a Id,
    pub days: u32,
    pub from: &'a Date,
    pub until: &'a Date,
    pub teams: Vec<&'a Id>,
    pub unavailable: &'a [TeamUnavailable],
}

/// Checks that an answer is for the org and range that were asked for, covers a period of that many
/// days, and names no team twice.
pub(crate) fn check_answer(org: &Id, days: u32, answer: &Answer) -> Result<(), String> {
    if answer.org != org || answer.days != days || !valid_days(days) {
        return Err(format!(
            "reported {} over {} days instead of {org} over {days} days",
            answer.org, answer.days
        ));
    }
    check_period(answer.from, answer.until, days)?;
    let mut named = answer.teams.clone();
    named.extend(answer.unavailable.iter().map(|team| &team.team));
    let count = named.len();
    named.sort();
    named.dedup();
    if named.len() != count {
        return Err("reported a team twice".into());
    }
    Ok(())
}

/// Checks the days of one team's report: in order, inside the team's own period, at most `days` of them,
/// and every cost a number that is not negative.
pub(crate) fn check_days(
    dates: &[&Date],
    from: &Date,
    until: &Date,
    costs: &[f64],
    days: u32,
) -> Result<(), String> {
    check_period(from, until, days)?;
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

/// The pull requests merged in the period, as the summary counts them.
fn merged_total(summary: &EvaluationSummary) -> u64 {
    let counts = summary.merged;
    [
        counts.feat,
        counts.fix,
        counts.refactor,
        counts.docs,
        counts.test,
        counts.chore,
        counts.other,
        summary.reverts,
    ]
    .iter()
    .map(|count| u64::from(*count))
    .sum()
}

/// Checks a team's participants: each named and listed once, no more time than the period holds (the three
/// times of a participant add up to at most `days` times 86 400 seconds), costs that are numbers not
/// negative, and their merged pull requests, with the unattributed ones, adding up to the summary's.
fn check_participants(team: &TeamEvaluation, team_days: u32) -> Result<(), String> {
    if team.participants.is_empty() && team.unattributed_merged == 0 {
        return Ok(());
    }
    if team.participants.len() > MAX_PARTICIPANTS {
        return Err("reported more participants than a report may hold".into());
    }
    let mut names = std::collections::BTreeSet::new();
    let mut merged = u64::from(team.unattributed_merged);
    for participant in &team.participants {
        let name = &participant.name;
        if name.is_empty() || name.len() > MAX_NAME_BYTES || !names.insert(name.as_str()) {
            return Err("reported a participant without a usable name, or twice".into());
        }
        let seconds = [
            participant.busy_seconds,
            participant.waiting_seconds,
            participant.idle_seconds,
        ]
        .iter()
        .fold(0u64, |sum, part| sum.saturating_add(*part));
        if seconds > u64::from(team_days) * 86_400 {
            return Err(format!(
                "reported more time than the period holds for {name}"
            ));
        }
        let costs = [
            Some(participant.cost_usd),
            participant.cost_per_merged_pr_usd,
        ];
        if costs.iter().flatten().any(|cost| !valid_cost(*cost)) {
            return Err(format!("reported a malformed cost for {name}"));
        }
        merged += u64::from(participant.merged);
    }
    if merged != merged_total(&team.summary) {
        return Err("reported merged pull requests of participants that do not add up".into());
    }
    Ok(())
}

fn is_repository(name: &str) -> bool {
    let part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    name.len() <= MAX_REPOSITORY_BYTES
        && name
            .split_once('/')
            .is_some_and(|(owner, repo)| part(owner) && part(repo))
}

/// Checks a team's repositories: named `owner/name`, in order and once, with no cost, and adding up to the
/// team's summary: the same pull requests by type and the same reverts, and a red time of the team that is
/// at least the longest of theirs and at most their sum (the branches can be red at the same time).
fn check_by_repository(team: &TeamEvaluation) -> Result<(), String> {
    let found = &team.by_repository;
    if found.is_empty() {
        return Ok(());
    }
    if found.len() > MAX_REPOSITORIES {
        return Err("reported more repositories than a report may hold".into());
    }
    if found.iter().any(|one| !is_repository(&one.repo)) {
        return Err("reported a repository that is not written owner/name".into());
    }
    if found.windows(2).any(|pair| pair[0].repo >= pair[1].repo) {
        return Err("reported repositories out of order or twice".into());
    }
    if found
        .iter()
        .any(|one| one.summary.cost_per_merged_pr_usd.is_some())
    {
        return Err("reported a cost for a repository, which has none".into());
    }
    let sum = |count: fn(&EvaluationSummary) -> u32| -> u64 {
        found.iter().map(|one| u64::from(count(&one.summary))).sum()
    };
    let counts: [fn(&EvaluationSummary) -> u32; 8] = [
        |s| s.merged.feat,
        |s| s.merged.fix,
        |s| s.merged.refactor,
        |s| s.merged.docs,
        |s| s.merged.test,
        |s| s.merged.chore,
        |s| s.merged.other,
        |s| s.reverts,
    ];
    if counts
        .iter()
        .any(|count| sum(*count) != u64::from(count(&team.summary)))
    {
        return Err("reported repositories whose pull requests do not add up to the team's".into());
    }
    let red: Vec<u64> = found
        .iter()
        .map(|one| one.summary.main_red_seconds)
        .collect();
    let total = red.iter().fold(0u64, |sum, part| sum.saturating_add(*part));
    let longest = red.iter().copied().max().unwrap_or(0);
    if team.summary.main_red_seconds < longest || team.summary.main_red_seconds > total {
        return Err("reported repositories whose red time does not fit the team's".into());
    }
    Ok(())
}

impl Evaluation {
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
            let mut costs: Vec<f64> = team.by_day.iter().map(|day| day.cost_usd).collect();
            costs.extend(team.summary.cost_per_merged_pr_usd);
            check_days(&dates, &team.from, &team.until, &costs, days)?;
            check_participants(team, days)?;
            check_by_repository(team)?;
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
    fn a_team_in_another_time_zone_is_drawn_against_its_own_period() {
        let mut report = sample();
        report.teams[0].from = Date::parse("2026-11-12").unwrap();
        report.teams[0].until = Date::parse("2026-11-25").unwrap();
        report.teams[0].by_day[1].date = Date::parse("2026-11-25").unwrap();
        assert_eq!(
            report.check(&acme(), 14),
            Ok(()),
            "a day after the answering node's `until`"
        );
        report.teams[0].from = Date::parse("2026-11-11").unwrap();
        report.teams[0].until = Date::parse("2026-11-24").unwrap();
        let outside = report.check(&acme(), 14).unwrap_err();
        assert!(
            outside.contains("outside"),
            "but not outside its own period: {outside}"
        );
        let team = &mut report.teams[0];
        std::mem::swap(&mut team.from, &mut team.until);
        assert!(report
            .check(&acme(), 14)
            .unwrap_err()
            .contains("ends before"));
    }

    #[test]
    fn a_team_without_participants_leaves_them_out_and_reads_them_back_when_there_are_some() {
        let value = serde_json::to_value(sample()).unwrap();
        let docs = &value["teams"][1];
        assert!(docs.get("participants").is_none() && docs.get("unattributedMerged").is_none());
        let app = &value["teams"][0];
        assert_eq!(app["participants"].as_array().unwrap().len(), 3);
        assert_eq!(app["unattributedMerged"], 1);
        assert!(app["participants"][2].get("costPerMergedPrUsd").is_none());
        // An answer of a node that does not know of participants still reads.
        let mut old = value.clone();
        old["teams"][0]
            .as_object_mut()
            .unwrap()
            .remove("participants");
        old["teams"][0]
            .as_object_mut()
            .unwrap()
            .remove("unattributedMerged");
        let read: Evaluation = serde_json::from_value(old).unwrap();
        assert_eq!(read.check(&acme(), 14), Ok(()));
        assert!(read.teams[0].participants.is_empty());
    }

    #[test]
    fn a_team_with_two_repositories_breaks_its_numbers_down_and_a_team_with_one_does_not() {
        let value = serde_json::to_value(sample()).unwrap();
        let repos = value["teams"][0]["byRepository"].as_array().unwrap();
        assert_eq!(repos.len(), 2);
        assert!(repos[0]["summary"].get("costPerMergedPrUsd").is_none());
        assert!(value["teams"][1].get("byRepository").is_none());
        // An answer of a node that does not know of it still reads.
        let mut old = value.clone();
        old["teams"][0]
            .as_object_mut()
            .unwrap()
            .remove("byRepository");
        let read: Evaluation = serde_json::from_value(old).unwrap();
        assert_eq!(read.check(&acme(), 14), Ok(()));
        assert!(read.teams[0].by_repository.is_empty());
    }

    #[test]
    fn refuses_repositories_that_are_not_well_formed_or_do_not_add_up_to_the_team() {
        let refused = |change: &dyn Fn(&mut TeamEvaluation)| {
            let mut report = sample();
            change(&mut report.teams[0]);
            report.check(&acme(), 14).unwrap_err()
        };
        assert!(refused(&|t| t.by_repository[0].summary.merged.feat += 1).contains("do not add up"));
        assert!(refused(&|t| t.by_repository[1].summary.reverts = 0).contains("do not add up"));
        assert!(refused(&|t| t.by_repository.reverse()).contains("out of order"));
        assert!(refused(&|t| t.by_repository[1].repo = "acme/app".into())
            .contains("out of order or twice"));
        for bad in ["acme", "acme/", "/web", "acme/we b", "acme/web/x", ""] {
            let bad = bad.to_owned();
            assert!(
                refused(&|t| t.by_repository[1].repo = bad.clone()).contains("owner/name"),
                "{bad}"
            );
        }
        assert!(
            refused(&|t| t.by_repository[0].summary.cost_per_merged_pr_usd = Some(1.0))
                .contains("has none")
        );
        assert!(
            refused(&|t| t.by_repository[0].summary.main_red_seconds = 1799).contains("red time")
        );
        assert!(
            refused(&|t| t.by_repository[0].summary.main_red_seconds = 1801).contains("red time")
        );
        assert!(refused(&|t| t.summary.main_red_seconds = 3601).contains("red time"));
        assert!(refused(&|t| {
            let one = t.by_repository[0].clone();
            t.by_repository = (0..=MAX_REPOSITORIES)
                .map(|n| RepositoryEvaluation {
                    repo: format!("acme/r{n:03}"),
                    ..one.clone()
                })
                .collect();
        })
        .contains("more repositories"));
    }

    #[test]
    fn refuses_participants_that_are_not_well_formed_or_do_not_add_up_to_the_summary() {
        let refused = |change: &dyn Fn(&mut TeamEvaluation)| {
            let mut report = sample();
            change(&mut report.teams[0]);
            report.check(&acme(), 14).unwrap_err()
        };
        assert!(refused(&|t| t.participants[0].merged += 1).contains("do not add up"));
        assert!(refused(&|t| t.unattributed_merged = 0).contains("do not add up"));
        assert!(refused(&|t| t.participants[1].name = "backend".into()).contains("twice"));
        assert!(refused(&|t| t.participants[0].name = String::new()).contains("usable name"));
        assert!(refused(&|t| t.participants[0].name = "x".repeat(201)).contains("usable name"));
        assert!(refused(&|t| t.participants[0].cost_usd = -0.01).contains("malformed cost"));
        let whole = 14 * 86_400;
        assert!(refused(&|t| {
            let p = &mut t.participants[0];
            (p.busy_seconds, p.waiting_seconds, p.idle_seconds) = (whole, 1, 0);
        })
        .contains("more time than the period"));
        assert!(
            refused(&|t| t.participants[0].idle_seconds = 1_000_000_000_000)
                .contains("more time than the period")
        );
        assert!(refused(&|t| t.participants[0].busy_seconds = u64::MAX).contains("more time"));
        let mut full = sample();
        let p = &mut full.teams[0].participants[0];
        (p.busy_seconds, p.waiting_seconds, p.idle_seconds) = (whole, 0, 0);
        assert_eq!(
            full.check(&acme(), 14),
            Ok(()),
            "alive the whole period is the most"
        );
        assert!(refused(&|t| t.participants[0].cost_usd = f64::NAN).contains("malformed cost"));
        assert!(
            refused(&|t| t.participants[0].cost_per_merged_pr_usd = Some(f64::INFINITY))
                .contains("malformed cost")
        );
        assert!(refused(&|t| {
            let one = t.participants[0].clone();
            t.participants = (0..1001)
                .map(|n| ParticipantEvaluation {
                    name: n.to_string(),
                    ..one.clone()
                })
                .collect();
        })
        .contains("more participants"));
        // Only unattributed pull requests, with no participant, still have to add up.
        assert!(refused(&|t| {
            t.participants.clear();
            t.unattributed_merged = 6;
        })
        .contains("do not add up"));
        let mut all = sample();
        all.teams[0].participants.clear();
        all.teams[0].unattributed_merged = 7;
        assert_eq!(all.check(&acme(), 14), Ok(()));
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

    #[test]
    fn the_period_is_as_long_as_the_days_asked_for() {
        let mut report = sample();
        report.until = Date::parse("2030-01-01").unwrap();
        assert!(report
            .check(&acme(), 14)
            .unwrap_err()
            .contains("not 14 days long"));
        let mut report = sample();
        report.teams[0].from = Date::parse("2020-01-01").unwrap();
        assert!(report
            .check(&acme(), 14)
            .unwrap_err()
            .contains("not 14 days long"));
        assert_eq!(
            period_days(
                &Date::parse("2026-02-27").unwrap(),
                &Date::parse("2026-03-02").unwrap()
            ),
            Some(4)
        );
    }

    #[test]
    fn the_samples_numbers_follow_the_rules() {
        let team = &sample().teams[0];
        let days = &team.by_day;
        let all = |pick: fn(&DayEvaluation) -> &Vec<u64>| -> Vec<u64> {
            days.iter().flat_map(|day| pick(day).clone()).collect()
        };
        let pr_leads = all(|day| &day.pr_lead_seconds);
        let issue_leads = all(|day| &day.issue_lead_seconds);
        let summary = &team.summary;
        assert_eq!(summary.pr_lead_median_seconds, median(&pr_leads));
        assert_eq!(summary.pr_lead_p90_seconds, p90(&pr_leads));
        assert_eq!(summary.issue_lead_median_seconds, median(&issue_leads));
        assert_eq!(summary.issue_lead_p90_seconds, p90(&issue_leads));
        let red: u64 = days.iter().map(|day| day.main_red_seconds).sum();
        assert_eq!(summary.main_red_seconds, red);
        let longest = summary.longest_red_seconds;
        assert!(
            longest <= red && (red == 0) == (longest == 0),
            "a span is part of the red time"
        );
        assert_eq!(
            summary.reverts,
            days.iter().map(|day| day.reverts).sum::<u32>()
        );
        let sum =
            |pick: fn(&MergedCounts) -> u32| days.iter().map(|day| pick(&day.merged)).sum::<u32>();
        let merged = &summary.merged;
        assert_eq!(
            [
                merged.feat,
                merged.fix,
                merged.refactor,
                merged.docs,
                merged.test,
                merged.chore,
                merged.other
            ],
            [
                sum(|m| m.feat),
                sum(|m| m.fix),
                sum(|m| m.refactor),
                sum(|m| m.docs),
                sum(|m| m.test),
                sum(|m| m.chore),
                sum(|m| m.other)
            ]
        );
        let total = merged.feat
            + merged.fix
            + merged.refactor
            + merged.docs
            + merged.test
            + merged.chore
            + merged.other
            + summary.reverts;
        assert_eq!(
            pr_leads.len() as u32,
            total,
            "one lead time for each pull request merged, reverts included"
        );
        let cost: f64 = days.iter().map(|day| day.cost_usd).sum();
        let per_pr = summary.cost_per_merged_pr_usd.unwrap();
        assert!(
            (per_pr - cost / f64::from(total)).abs() < 0.005,
            "{per_pr} against {cost} over {total}"
        );
    }

    #[test]
    fn the_median_and_the_p90_are_worked_out_as_the_spec_says() {
        assert_eq!((median(&[]), p90(&[])), (None, None));
        assert_eq!((median(&[7]), p90(&[7])), (Some(7), Some(7)));
        assert_eq!(
            median(&[9000, 21600]),
            Some(15300),
            "the mean of the two middle samples"
        );
        assert_eq!(median(&[3, 4]), Some(3), "rounded down");
        assert_eq!(
            median(&[u64::MAX, u64::MAX]),
            Some(u64::MAX),
            "without overflow"
        );
        let ten: Vec<u64> = (1..=10).collect();
        assert_eq!(p90(&ten), Some(9), "9 in 10 samples do not exceed it");
        let eleven: Vec<u64> = (1..=11).collect();
        assert_eq!(p90(&eleven), Some(10), "ceil(0.9 * 11) = 10");
        assert_eq!(p90(&[5, 1, 3]), Some(5));
    }
}
