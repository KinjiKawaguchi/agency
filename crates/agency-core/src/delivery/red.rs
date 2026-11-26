//! How long a default branch's CI was red.
use super::facts::{MainRun, Verdict};
use agency_proto::clock::OffsetDateTime;
use std::collections::BTreeMap;

/// What the runs of one commit say about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Standing {
    Red,
    Green,
}

/// The standing of each commit, in the order the commits finished, from its runs. A commit is red when
/// any run failed and green when its runs passed. Runs that were cancelled say nothing, so a commit with
/// only those has no standing. A commit finishes when its last run does.
fn standings(runs: &[MainRun]) -> Vec<(OffsetDateTime, Standing)> {
    let mut by_commit: BTreeMap<&str, Vec<&MainRun>> = BTreeMap::new();
    for run in runs {
        by_commit.entry(run.commit.as_str()).or_default().push(run);
    }
    let mut standings: Vec<(OffsetDateTime, Standing)> = by_commit
        .values()
        .filter_map(|commit_runs| {
            let counted: Vec<&&MainRun> = commit_runs
                .iter()
                .filter(|run| run.verdict != Verdict::Cancelled)
                .collect();
            let finished = commit_runs.iter().map(|run| run.finished_at).max()?;
            let red = counted.iter().any(|run| run.verdict == Verdict::Failed);
            let green = !counted.is_empty() && !red;
            (red || green).then_some((finished, if red { Standing::Red } else { Standing::Green }))
        })
        .collect();
    standings.sort_by_key(|(finished, _)| *finished);
    standings
}

/// The spans during which the branch was red: from the finish of the first red commit to the finish of
/// the next green one, or to `now` while it is still red.
pub fn red_spans(runs: &[MainRun], now: OffsetDateTime) -> Vec<(OffsetDateTime, OffsetDateTime)> {
    let mut spans = vec![];
    let mut red_since: Option<OffsetDateTime> = None;
    for (finished, standing) in standings(runs) {
        match (standing, red_since) {
            (Standing::Red, None) => red_since = Some(finished),
            (Standing::Green, Some(since)) => {
                spans.push((since, finished));
                red_since = None;
            }
            _ => {}
        }
    }
    if let Some(since) = red_since {
        spans.push((since, now.max(since)));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::clock;

    fn at(raw: &str) -> OffsetDateTime {
        clock::parse(raw).unwrap()
    }

    fn run(commit: &str, finished: &str, verdict: Verdict) -> MainRun {
        MainRun {
            commit: commit.into(),
            finished_at: at(finished),
            verdict,
        }
    }

    fn spans(runs: &[MainRun], now: &str) -> Vec<(String, String)> {
        red_spans(runs, at(now))
            .into_iter()
            .map(|(from, until)| (clock::format(from), clock::format(until)))
            .collect()
    }

    #[test]
    fn red_lasts_from_the_first_red_commit_to_the_next_green_one() {
        let runs = [
            run("a", "2026-11-24T10:00:00Z", Verdict::Passed),
            run("b", "2026-11-24T11:00:00Z", Verdict::Failed),
            run("c", "2026-11-24T11:20:00Z", Verdict::Failed),
            run("d", "2026-11-24T11:30:00Z", Verdict::Passed),
            run("e", "2026-11-24T12:00:00Z", Verdict::Passed),
        ];
        assert_eq!(
            spans(&runs, "2026-11-24T13:00:00Z"),
            [(
                "2026-11-24T11:00:00Z".to_string(),
                "2026-11-24T11:30:00Z".to_string()
            )],
            "two red commits in a row are one span"
        );
    }

    #[test]
    fn a_branch_that_is_still_red_is_red_until_now() {
        let runs = [
            run("a", "2026-11-24T10:00:00Z", Verdict::Passed),
            run("b", "2026-11-24T11:00:00Z", Verdict::Failed),
        ];
        assert_eq!(
            spans(&runs, "2026-11-24T11:45:00Z"),
            [(
                "2026-11-24T11:00:00Z".to_string(),
                "2026-11-24T11:45:00Z".to_string()
            )]
        );
        assert_eq!(
            spans(&runs, "2026-11-24T10:00:00Z").len(),
            1,
            "a clock behind the run does not make a negative span"
        );
    }

    #[test]
    fn the_runs_of_one_commit_decide_it_together() {
        let runs = [
            run("a", "2026-11-24T10:00:00Z", Verdict::Passed),
            run("a", "2026-11-24T10:05:00Z", Verdict::Failed),
            run("b", "2026-11-24T10:30:00Z", Verdict::Passed),
            run("b", "2026-11-24T10:40:00Z", Verdict::Passed),
        ];
        assert_eq!(
            spans(&runs, "2026-11-24T12:00:00Z"),
            [(
                "2026-11-24T10:05:00Z".to_string(),
                "2026-11-24T10:40:00Z".to_string()
            )],
            "a commit is red when one of its runs failed, and finishes with its last run"
        );
    }

    #[test]
    fn a_cancelled_run_counts_for_nothing() {
        let runs = [
            run("a", "2026-11-24T10:00:00Z", Verdict::Failed),
            run("b", "2026-11-24T10:10:00Z", Verdict::Cancelled),
            run("c", "2026-11-24T10:20:00Z", Verdict::Passed),
            run("c", "2026-11-24T10:25:00Z", Verdict::Cancelled),
        ];
        assert_eq!(
            spans(&runs, "2026-11-24T12:00:00Z"),
            [("2026-11-24T10:00:00Z".to_string(), "2026-11-24T10:25:00Z".to_string())],
            "the cancelled commit does not end the span, and a commit with a pass and a cancel is green"
        );
        assert!(spans(
            &[run("x", "2026-11-24T10:00:00Z", Verdict::Cancelled)],
            "2026-11-24T12:00:00Z"
        )
        .is_empty());
    }

    #[test]
    fn runs_in_any_order_and_no_runs() {
        let runs = [
            run("b", "2026-11-24T11:00:00Z", Verdict::Passed),
            run("a", "2026-11-24T10:00:00Z", Verdict::Failed),
        ];
        assert_eq!(spans(&runs, "2026-11-24T12:00:00Z").len(), 1);
        assert!(spans(&[], "2026-11-24T12:00:00Z").is_empty());
    }
}
