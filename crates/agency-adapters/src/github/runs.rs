//! The runs of CI on a repository's default branch (every workflow), from what a push started.
use super::gh::{Gh, GhError};
use agency_core::delivery::{MainRun, Verdict};
use agency_proto::clock::{self, Duration, OffsetDateTime};

/// How long before the period to read runs, to know what state the branch was in when it began.
const MARGIN: Duration = Duration::days(7);

/// How a run ended, from its `conclusion`. A run with none, or one that says nothing about the commit
/// (cancelled, superseded, waiting for someone), is cancelled.
fn verdict(conclusion: &str) -> Verdict {
    match conclusion {
        "success" | "skipped" | "neutral" => Verdict::Passed,
        "failure" | "timed_out" | "startup_failure" => Verdict::Failed,
        _ => Verdict::Cancelled,
    }
}

/// The runs in the lines of `sha<TAB>conclusion<TAB>updated_at`, which `gh api --jq` made. A line that
/// is not one is left out.
fn read_runs(repo: &str, text: &str) -> Vec<MainRun> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let (commit, conclusion, finished) = (fields.next()?, fields.next()?, fields.next()?);
            (!commit.is_empty()).then_some(MainRun {
                repo: repo.to_owned(),
                commit: commit.to_owned(),
                finished_at: clock::parse(finished)?,
                verdict: verdict(conclusion),
            })
        })
        .collect()
}

/// The default branch of `owner/name`.
async fn default_branch(gh: &dyn Gh, owner: &str, name: &str) -> Result<String, GhError> {
    let args = [
        "api",
        &format!("repos/{owner}/{name}"),
        "--jq",
        ".default_branch",
    ]
    .map(String::from);
    let branch = gh.run(&args).await?.trim().to_owned();
    if branch.is_empty() {
        Err(GhError::Failed)
    } else {
        Ok(branch)
    }
}

/// The completed runs on the default branch of `owner/name` created from a week before `since`.
pub async fn main_runs(
    gh: &dyn Gh,
    owner: &str,
    name: &str,
    since: OffsetDateTime,
) -> Result<Vec<MainRun>, GhError> {
    let branch = default_branch(gh, owner, name).await?;
    let from = (since - MARGIN).date();
    let args: Vec<String> = [
        "api",
        &format!("repos/{owner}/{name}/actions/runs"),
        "--method",
        "GET",
        "-f",
        &format!("branch={branch}"),
        "-f",
        "event=push",
        "-f",
        "status=completed",
        "-f",
        "per_page=100",
        "-f",
        &format!("created=>={from}"),
        "--paginate",
        "--jq",
        ".workflow_runs[] | [.head_sha, (.conclusion // \"\"), .updated_at] | @tsv",
    ]
    .map(String::from)
    .to_vec();
    Ok(read_runs(&format!("{owner}/{name}"), &gh.run(&args).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_a_conclusion_to_what_it_says_about_the_commit() {
        for passed in ["success", "skipped", "neutral"] {
            assert_eq!(verdict(passed), Verdict::Passed);
        }
        for failed in ["failure", "timed_out", "startup_failure"] {
            assert_eq!(verdict(failed), Verdict::Failed);
        }
        for says_nothing in ["cancelled", "stale", "action_required", ""] {
            assert_eq!(
                verdict(says_nothing),
                Verdict::Cancelled,
                "{says_nothing:?}"
            );
        }
    }

    #[test]
    fn reads_the_lines_gh_makes_and_skips_the_ones_that_are_not() {
        let text = "abc123\tsuccess\t2026-11-24T10:00:00Z\nabc123\tfailure\t2026-11-24T10:05:00Z\n\nbroken line\nfff\t\t2026-11-24T11:00:00Z\n\tsuccess\t2026-11-24T12:00:00Z\nggg\tsuccess\tnot a time\n";
        let runs = read_runs("acme/app", text);
        let summary: Vec<(&str, Verdict)> = runs
            .iter()
            .map(|run| (run.commit.as_str(), run.verdict))
            .collect();
        assert_eq!(
            summary,
            [
                ("abc123", Verdict::Passed),
                ("abc123", Verdict::Failed),
                ("fff", Verdict::Cancelled)
            ]
        );
        assert!(runs.iter().all(|run| run.repo == "acme/app"));
    }
}
