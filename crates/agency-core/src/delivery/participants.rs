//! Each participant's numbers over the period (docs/protocol.md, "Evaluation" and "Participant metrics"):
//! what their runtime says of their time and tokens, and how many of the merged pull requests GitHub lists
//! it names them the author of. Both come as facts; this only adds them up.
use super::facts::MergedPullRequest;
use super::period::Period;
use agency_proto::evaluation::ParticipantEvaluation;
use agency_proto::participant_metrics::ParticipantMetrics;
use agency_proto::usage::TokenCounts;
use std::collections::{BTreeMap, HashMap};

/// A cost in US dollars, rounded to cents.
pub(super) fn round_to_cents(cost: f64) -> f64 {
    (cost * 100.0).round() / 100.0
}

#[derive(Default)]
struct Totals {
    busy_seconds: u64,
    waiting_seconds: u64,
    idle_seconds: u64,
    cost_usd: f64,
    tokens: TokenCounts,
    merged: u32,
}

impl Totals {
    fn into_evaluation(self, name: String) -> ParticipantEvaluation {
        ParticipantEvaluation {
            name,
            busy_seconds: self.busy_seconds,
            waiting_seconds: self.waiting_seconds,
            idle_seconds: self.idle_seconds,
            cost_usd: round_to_cents(self.cost_usd),
            tokens: self.tokens,
            merged: self.merged,
            cost_per_merged_pr_usd: (self.merged > 0)
                .then(|| round_to_cents(self.cost_usd / f64::from(self.merged))),
        }
    }
}

/// The participants of `report`, with the pull requests merged in `period` counted for the participant the
/// report names their author, by name; and how many merged pull requests it names no author of. A
/// participant the report names only as an author has no time. `report` has been checked by the caller.
pub fn attribute(
    report: &ParticipantMetrics,
    pull_requests: &[MergedPullRequest],
    period: &Period,
) -> (Vec<ParticipantEvaluation>, u32) {
    let (first, last) = (period.first(), period.last());
    let mut totals: BTreeMap<&str, Totals> = BTreeMap::new();
    for participant in &report.participants {
        let sum = totals.entry(participant.name.as_str()).or_default();
        for day in &participant.by_day {
            if day.date.as_str() < first.as_str() || day.date.as_str() > last.as_str() {
                continue;
            }
            sum.busy_seconds += u64::from(day.busy_seconds);
            sum.waiting_seconds += u64::from(day.waiting_seconds);
            sum.idle_seconds += u64::from(day.idle_seconds);
            sum.cost_usd += day.cost_usd;
            let (tokens, day) = (&mut sum.tokens, day.tokens);
            tokens.input = tokens.input.saturating_add(day.input);
            tokens.output = tokens.output.saturating_add(day.output);
            tokens.cache_read = tokens.cache_read.saturating_add(day.cache_read);
            tokens.cache_write = tokens.cache_write.saturating_add(day.cache_write);
        }
    }
    // A pull request the report names two authors for (a report the node's check refuses, so only a caller
    // that skipped it gets here) has no author: it is counted once, as unattributed, never twice.
    let mut authors: HashMap<(&str, u64), Option<&str>> = HashMap::new();
    for author in &report.pull_requests {
        authors
            .entry((author.repo.as_str(), author.number))
            .and_modify(|known| *known = None)
            .or_insert(Some(author.by.as_str()));
    }
    let mut unattributed = 0;
    for pull_request in pull_requests
        .iter()
        .filter(|pull_request| period.contains(pull_request.merged_at))
    {
        match authors.get(&(pull_request.repo.as_str(), pull_request.number)) {
            Some(Some(name)) => totals.entry(name).or_default().merged += 1,
            _ => unattributed += 1,
        }
    }
    let participants = totals
        .into_iter()
        .map(|(name, sum)| sum.into_evaluation(name.to_owned()))
        .collect();
    (participants, unattributed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::clock::{self, OffsetDateTime, UtcOffset};

    const REPORT: &str = include_str!("../../../../testdata/participantmetrics.sample.json");

    fn at(raw: &str) -> OffsetDateTime {
        clock::parse(raw).unwrap()
    }

    /// The sample's two days, 2026-11-23 and 2026-11-24, in its time zone (UTC+9).
    fn period() -> Period {
        Period::new(
            at("2026-11-24T12:00:00Z"),
            UtcOffset::from_hms(9, 0, 0).unwrap(),
            2,
        )
    }

    fn merged(repo: &str, number: u64, at_text: &str) -> MergedPullRequest {
        MergedPullRequest {
            repo: repo.into(),
            number,
            title: "feat: x".into(),
            body: String::new(),
            opened_at: at("2026-11-20T00:00:00Z"),
            merged_at: at(at_text),
            first_commit_at: None,
            has_issue: false,
            issue_started_at: None,
            closes_incident: false,
            work_type: None,
        }
    }

    fn report() -> ParticipantMetrics {
        serde_json::from_str(REPORT).unwrap()
    }

    #[test]
    fn adds_up_each_participants_days_and_counts_the_pull_requests_the_runtime_names_them_for() {
        let mut report = report();
        for (number, by) in [
            (107, "backend"),
            (108, "backend"),
            (106, "newcomer"),
            (105, "backend"),
        ] {
            report
                .pull_requests
                .push(agency_proto::participant_metrics::PullRequestAuthor {
                    repo: "acme/app".into(),
                    number,
                    by: by.into(),
                });
        }
        // A day outside the period is not added.
        let mut extra = report.participants[1].by_day[0].clone();
        extra.date = agency_proto::Date::parse("2026-11-22").unwrap();
        extra.busy_seconds = 99_999;
        report.participants[1].by_day.insert(0, extra);
        let pull_requests = [
            merged("acme/app", 101, "2026-11-23T10:00:00Z"),
            merged("acme/app", 107, "2026-11-23T20:00:00Z"),
            merged("acme/app", 108, "2026-11-24T05:00:00Z"),
            merged("acme/app", 102, "2026-11-24T06:00:00Z"),
            merged("acme/app", 106, "2026-11-24T07:00:00Z"),
            merged("acme/app", 103, "2026-11-24T08:00:00Z"),
            merged("acme/web", 101, "2026-11-24T09:00:00Z"),
            // Merged before the period: not counted for anyone, and not unattributed either.
            merged("acme/app", 105, "2026-11-22T14:59:59Z"),
        ];
        let (participants, unattributed) = attribute(&report, &pull_requests, &period());
        let names: Vec<&str> = participants.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            ["backend", "lead", "newcomer", "reviewer"],
            "by name"
        );
        let by_name = |name: &str| participants.iter().find(|p| p.name == name).unwrap();
        let backend = by_name("backend");
        assert_eq!(
            (
                backend.busy_seconds,
                backend.waiting_seconds,
                backend.idle_seconds
            ),
            (30_600, 2_400, 4_800)
        );
        assert_eq!(backend.cost_usd, 8.45);
        assert_eq!(backend.tokens.cache_read, 2_400_000);
        assert_eq!(
            backend.merged, 3,
            "101, 107, and 108 (105 was merged before the period)"
        );
        assert_eq!(
            backend.cost_per_merged_pr_usd,
            Some(2.82),
            "8.45 / 3, to cents"
        );
        let lead = by_name("lead");
        assert_eq!(
            (lead.busy_seconds, lead.merged),
            (10_800, 1),
            "the day outside is not added"
        );
        assert_eq!(lead.cost_per_merged_pr_usd, Some(4.35));
        let newcomer = by_name("newcomer");
        assert_eq!(
            (newcomer.busy_seconds, newcomer.cost_usd, newcomer.merged),
            (0, 0.0, 1)
        );
        assert_eq!(newcomer.cost_per_merged_pr_usd, Some(0.0));
        let reviewer = by_name("reviewer");
        assert_eq!(
            (reviewer.merged, reviewer.cost_per_merged_pr_usd),
            (0, None)
        );
        assert_eq!(unattributed, 2, "103, and 101 of another repository");
    }

    #[test]
    fn a_pull_request_with_two_authors_is_counted_once_as_unattributed_never_twice() {
        let mut report = report();
        report.pull_requests = vec![
            agency_proto::participant_metrics::PullRequestAuthor {
                repo: "acme/app".into(),
                number: 101,
                by: "backend".into(),
            },
            agency_proto::participant_metrics::PullRequestAuthor {
                repo: "acme/app".into(),
                number: 101,
                by: "lead".into(),
            },
            agency_proto::participant_metrics::PullRequestAuthor {
                repo: "acme/app".into(),
                number: 102,
                by: "lead".into(),
            },
        ];
        let pull_requests = [
            merged("acme/app", 101, "2026-11-23T10:00:00Z"),
            merged("acme/app", 102, "2026-11-23T11:00:00Z"),
        ];
        let (participants, unattributed) = attribute(&report, &pull_requests, &period());
        let counted: u32 = participants.iter().map(|p| p.merged).sum();
        assert_eq!(
            (counted, unattributed),
            (1, 1),
            "the sum is the two pull requests, once each"
        );
        assert_eq!(
            participants
                .iter()
                .find(|p| p.name == "lead")
                .unwrap()
                .merged,
            1
        );
        assert_eq!(
            participants
                .iter()
                .find(|p| p.name == "backend")
                .unwrap()
                .merged,
            0
        );
    }

    #[test]
    fn a_report_with_no_pull_requests_leaves_all_of_them_unattributed() {
        let mut report = report();
        report.pull_requests.clear();
        let pull_requests = [merged("acme/app", 101, "2026-11-23T10:00:00Z")];
        let (participants, unattributed) = attribute(&report, &pull_requests, &period());
        assert_eq!(participants.len(), 3);
        assert!(participants.iter().all(|p| p.merged == 0));
        assert_eq!(unattributed, 1);
    }

    #[test]
    fn a_sum_of_costs_is_rounded_to_cents_and_not_left_as_float_noise() {
        let mut report = report();
        report.participants[0].by_day[0].cost_usd = 0.1;
        report.participants[0].by_day[1].cost_usd = 0.2;
        let (participants, _) = attribute(&report, &[], &period());
        assert_eq!(
            participants[0].cost_usd, 0.3,
            "0.1 + 0.2 is 0.30000000000000004 in floats"
        );
    }

    #[test]
    fn tokens_that_would_overflow_stop_at_the_largest_number() {
        let mut report = report();
        report.participants[0].by_day[0].tokens.input = u64::MAX;
        let (participants, _) = attribute(&report, &[], &period());
        assert_eq!(participants[0].tokens.input, u64::MAX);
    }
}
