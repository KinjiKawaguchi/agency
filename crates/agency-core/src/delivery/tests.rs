use super::*;
use agency_proto::clock::{self, OffsetDateTime, UtcOffset};
use agency_proto::evaluation::{DayEvaluation, Evaluation, MergedCounts, TeamEvaluation};
use agency_proto::{Date, Id, Timestamp};

fn at(raw: &str) -> OffsetDateTime {
    clock::parse(raw).unwrap()
}

fn tokyo() -> UtcOffset {
    UtcOffset::from_hms(9, 0, 0).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn pull_request(
    title: &str,
    body: &str,
    opened: &str,
    merged: &str,
    first_commit: Option<&str>,
    has_issue: bool,
    in_progress: Option<&str>,
) -> MergedPullRequest {
    MergedPullRequest {
        repo: "acme/app".into(),
        number: 1,
        title: title.into(),
        body: body.into(),
        opened_at: at(opened),
        merged_at: at(merged),
        first_commit_at: first_commit.map(at),
        has_issue,
        issue_started_at: in_progress.map(at),
        closes_incident: false,
        work_type: None,
        board: vec![],
    }
}

fn run(commit: &str, finished: &str, verdict: Verdict) -> MainRun {
    MainRun {
        repo: "acme/app".into(),
        commit: commit.into(),
        finished_at: at(finished),
        verdict,
    }
}

fn date(raw: &str) -> Date {
    Date::parse(raw).unwrap()
}

fn team() -> Id {
    Id::parse("app").unwrap()
}

fn counts(feat: u32, fix: u32, docs: u32, other: u32) -> MergedCounts {
    MergedCounts {
        feat,
        fix,
        docs,
        other,
        ..MergedCounts::default()
    }
}

/// The scenario every test builds on, in Tokyo time (UTC+9), asked at 21:00 on 2026-11-24 for 3 days:
/// the period is 2026-11-22 to 2026-11-24, from 2026-11-21T15:00Z to 2026-11-24T15:00Z.
fn scenario() -> (Vec<MergedPullRequest>, Vec<MainRun>, Vec<(Date, f64)>) {
    let pull_requests = vec![
        // 22nd: a feature, 3 h open; its issue moved to In Progress 6 h before the merge.
        pull_request(
            "feat(web): add a screen",
            "",
            "2026-11-22T01:00:00Z",
            "2026-11-22T04:00:00Z",
            Some("2026-11-22T00:00:00Z"),
            true,
            Some("2026-11-21T22:00:00Z"),
        ),
        // Merged at 01:00 on the 23rd in Tokyo: a fix, 2 h open; its issue has no board move, so the first commit counts (3 h).
        pull_request(
            "fix(core): refuse it",
            "",
            "2026-11-22T14:00:00Z",
            "2026-11-22T16:00:00Z",
            Some("2026-11-22T13:00:00Z"),
            true,
            None,
        ),
        // A revert and a docs change on the 23rd; neither has an issue.
        pull_request(
            "Revert \"feat(web): add a screen\"",
            "",
            "2026-11-23T01:00:00Z",
            "2026-11-23T01:30:00Z",
            None,
            false,
            None,
        ),
        pull_request(
            "docs: explain it",
            "",
            "2026-11-23T05:00:00Z",
            "2026-11-23T05:10:00Z",
            None,
            false,
            None,
        ),
        // On the 24th, a title that is not conventional.
        pull_request(
            "release 0.1.0",
            "",
            "2026-11-24T09:00:00Z",
            "2026-11-24T09:30:00Z",
            None,
            false,
            None,
        ),
        // Before the period, and after now: not counted.
        pull_request(
            "feat: old",
            "",
            "2026-11-19T01:00:00Z",
            "2026-11-20T01:00:00Z",
            None,
            true,
            None,
        ),
        pull_request(
            "feat: future",
            "",
            "2026-11-24T14:00:00Z",
            "2026-11-24T16:00:00Z",
            None,
            true,
            None,
        ),
    ];
    let runs = vec![
        // Red from before the period until 01:00 on the 22nd in Tokyo: only the hour inside counts.
        run("c0", "2026-11-21T10:00:00Z", Verdict::Failed),
        run("c0b", "2026-11-21T16:00:00Z", Verdict::Passed),
        run("c1", "2026-11-22T03:00:00Z", Verdict::Passed),
        // Red across midnight on the 23rd, from 23:30 to 01:00.
        run("c2", "2026-11-23T14:30:00Z", Verdict::Failed),
        run("c3", "2026-11-23T16:00:00Z", Verdict::Passed),
        // Red from 19:00 and still red at 21:00 (now).
        run("c4", "2026-11-24T10:00:00Z", Verdict::Failed),
    ];
    let costs = vec![
        (date("2026-11-20"), 99.0),
        (date("2026-11-22"), 6.0),
        (date("2026-11-23"), 3.0),
        (date("2026-11-24"), 1.0),
    ];
    (pull_requests, runs, costs)
}

fn evaluate_scenario() -> TeamEvaluation {
    let (pull_requests, runs, cost_by_day) = scenario();
    let input = Input {
        now: at("2026-11-24T12:00:00Z"),
        offset: tokyo(),
        days: 3,
        pull_requests: &pull_requests,
        runs: &runs,
        cost_by_day: &cost_by_day,
    };
    evaluate(team(), &input)
}

#[test]
fn counts_each_pull_request_on_the_day_it_merged_in_the_nodes_time_zone() {
    let evaluation = evaluate_scenario();
    assert_eq!(
        (evaluation.from.as_str(), evaluation.until.as_str()),
        ("2026-11-22", "2026-11-24")
    );
    let days = |pick: fn(&DayEvaluation) -> String| -> Vec<String> {
        evaluation.by_day.iter().map(pick).collect()
    };
    assert_eq!(
        days(|day| day.date.as_str().to_owned()),
        ["2026-11-22", "2026-11-23", "2026-11-24"]
    );
    let merged: Vec<MergedCounts> = evaluation.by_day.iter().map(|day| day.merged).collect();
    assert_eq!(
        merged,
        [counts(1, 0, 0, 0), counts(0, 1, 1, 0), counts(0, 0, 0, 1)],
        "the fix merged at 01:00 on the 23rd in Tokyo"
    );
    let reverts: Vec<u32> = evaluation.by_day.iter().map(|day| day.reverts).collect();
    assert_eq!(reverts, [0, 1, 0]);
}

#[test]
fn works_out_the_lead_times_of_each_day() {
    let evaluation = evaluate_scenario();
    let pr: Vec<&Vec<u64>> = evaluation
        .by_day
        .iter()
        .map(|day| &day.pr_lead_seconds)
        .collect();
    assert_eq!(
        pr,
        [&vec![10_800], &vec![7_200, 1_800, 600], &vec![1_800]],
        "in the order of the merges"
    );
    let issue: Vec<&Vec<u64>> = evaluation
        .by_day
        .iter()
        .map(|day| &day.issue_lead_seconds)
        .collect();
    assert_eq!(
        issue,
        [&vec![21_600], &vec![10_800], &Vec::<u64>::new()],
        "In Progress, else the first commit; none for a pull request without an issue"
    );
}

#[test]
fn red_time_is_split_between_days_and_clipped_to_the_period() {
    let evaluation = evaluate_scenario();
    let red: Vec<u64> = evaluation
        .by_day
        .iter()
        .map(|day| day.main_red_seconds)
        .collect();
    assert_eq!(red, [3_600, 1_800, 3_600 + 7_200]);
    assert_eq!(evaluation.summary.main_red_seconds, 16_200);
    assert_eq!(
        evaluation.summary.longest_red_seconds, 7_200,
        "the span that is still red, until now"
    );
}

#[test]
fn the_summary_is_worked_out_from_the_whole_period() {
    let summary = evaluate_scenario().summary;
    assert_eq!(summary.merged, counts(1, 1, 1, 1));
    assert_eq!(summary.reverts, 1);
    // PR leads 10800, 7200, 1800, 600, 1800 sorted are 600, 1800, 1800, 7200, 10800; issue leads 21600, 10800.
    assert_eq!(summary.pr_lead_median_seconds, Some(1_800));
    assert_eq!(summary.pr_lead_p90_seconds, Some(10_800));
    assert_eq!(
        summary.issue_lead_median_seconds,
        Some(16_200),
        "the mean of the two middle ones"
    );
    assert_eq!(summary.issue_lead_p90_seconds, Some(21_600));
    // Cost 6 + 3 + 1 = 10 over 4 merged and 1 revert; the day outside the period is not counted.
    assert_eq!(summary.cost_per_merged_pr_usd, Some(2.0));
}

#[test]
fn the_answer_passes_the_check_of_the_protocol() {
    let team = evaluate_scenario();
    let report = Evaluation {
        org: Id::parse("acme").unwrap(),
        days: 3,
        from: team.from.clone(),
        until: team.until.clone(),
        generated_at: Timestamp::parse("2026-11-24T12:00:00Z").unwrap(),
        teams: vec![team],
        unavailable: vec![],
    };
    assert_eq!(report.check(&Id::parse("acme").unwrap(), 3), Ok(()));
}

#[test]
fn a_team_with_nothing_has_a_quiet_report() {
    let input = Input {
        now: at("2026-11-24T12:00:00Z"),
        offset: UtcOffset::UTC,
        days: 14,
        pull_requests: &[],
        runs: &[],
        cost_by_day: &[],
    };
    let evaluation = evaluate(team(), &input);
    assert_eq!(
        (evaluation.from.as_str(), evaluation.until.as_str()),
        ("2026-11-11", "2026-11-24")
    );
    assert!(evaluation.by_day.is_empty());
    let summary = evaluation.summary;
    assert_eq!(summary.merged, MergedCounts::default());
    assert_eq!(
        (summary.pr_lead_median_seconds, summary.pr_lead_p90_seconds),
        (None, None)
    );
    assert_eq!(
        (
            summary.issue_lead_median_seconds,
            summary.issue_lead_p90_seconds
        ),
        (None, None)
    );
    assert_eq!(
        (
            summary.main_red_seconds,
            summary.longest_red_seconds,
            summary.reverts
        ),
        (0, 0, 0)
    );
    assert_eq!(summary.cost_per_merged_pr_usd, None);
}

#[test]
fn a_day_with_only_cost_is_listed_and_cost_without_merges_has_no_cost_per_pull_request() {
    let costs = [(date("2026-11-24"), 4.5)];
    let input = Input {
        now: at("2026-11-24T12:00:00Z"),
        offset: UtcOffset::UTC,
        days: 1,
        pull_requests: &[],
        runs: &[],
        cost_by_day: &costs,
    };
    let evaluation = evaluate(team(), &input);
    assert_eq!(evaluation.by_day.len(), 1);
    assert_eq!(evaluation.by_day[0].cost_usd, 4.5);
    assert_eq!(evaluation.summary.cost_per_merged_pr_usd, None);
}

#[test]
fn an_issue_start_that_makes_no_sense_is_not_a_sample() {
    let facts = [
        // In Progress after the merge: the first commit counts instead.
        pull_request(
            "feat: a",
            "",
            "2026-11-24T08:00:00Z",
            "2026-11-24T10:00:00Z",
            Some("2026-11-24T07:00:00Z"),
            true,
            Some("2026-11-24T11:00:00Z"),
        ),
        // In Progress after the merge and no first commit: no sample.
        pull_request(
            "feat: b",
            "",
            "2026-11-24T08:00:00Z",
            "2026-11-24T10:00:00Z",
            None,
            true,
            Some("2026-11-24T11:00:00Z"),
        ),
        // Opened after it merged (a clock that is off): a lead time of zero, not a huge one. It closes no
        // issue, so its first commit and the board move give it no issue lead time.
        pull_request(
            "feat: c",
            "",
            "2026-11-24T10:05:00Z",
            "2026-11-24T10:00:00Z",
            Some("2026-11-24T09:00:00Z"),
            false,
            Some("2026-11-24T09:00:00Z"),
        ),
    ];
    let input = Input {
        now: at("2026-11-24T12:00:00Z"),
        offset: UtcOffset::UTC,
        days: 1,
        pull_requests: &facts,
        runs: &[],
        cost_by_day: &[],
    };
    let day = &evaluate(team(), &input).by_day[0];
    assert_eq!(day.issue_lead_seconds, [10_800]);
    assert_eq!(day.pr_lead_seconds, [7_200, 7_200, 0]);
}

#[test]
fn the_same_facts_fall_on_other_days_in_another_time_zone() {
    let (pull_requests, runs, cost_by_day) = scenario();
    let input = Input {
        now: at("2026-11-24T12:00:00Z"),
        offset: UtcOffset::UTC,
        days: 3,
        pull_requests: &pull_requests,
        runs: &runs,
        cost_by_day: &cost_by_day,
    };
    let evaluation = evaluate(team(), &input);
    assert_eq!(evaluation.from.as_str(), "2026-11-22");
    let merged: Vec<MergedCounts> = evaluation.by_day.iter().map(|day| day.merged).collect();
    assert_eq!(
        merged[0],
        counts(1, 1, 0, 0),
        "in UTC the fix merged at 16:00 on the 22nd"
    );
}

// ── New-feature lead times ──

fn pull_requests_for_lead_times() -> Vec<MergedPullRequest> {
    let mut found = vec![];
    // Eight quick docs and chore merges: an hour each (opened to merged), the issue started two hours before.
    for n in 0..8 {
        let title = if n % 2 == 0 {
            "docs: a note"
        } else {
            "chore: a bump"
        };
        found.push(pull_request(
            title,
            "",
            "2026-11-23T01:00:00Z",
            "2026-11-23T02:00:00Z",
            None,
            true,
            Some("2026-11-22T23:00:00Z"),
        ));
    }
    // Three new features: 1 day, 2 days, and 3 days from opened to merged; their issues started a day earlier.
    for (opened, merged, started) in [
        (
            "2026-11-20T00:00:00Z",
            "2026-11-21T00:00:00Z",
            "2026-11-19T00:00:00Z",
        ),
        (
            "2026-11-19T00:00:00Z",
            "2026-11-21T00:00:00Z",
            "2026-11-18T00:00:00Z",
        ),
        (
            "2026-11-18T00:00:00Z",
            "2026-11-21T00:00:00Z",
            "2026-11-17T00:00:00Z",
        ),
    ] {
        found.push(pull_request(
            "feat(web): a screen",
            "",
            opened,
            merged,
            None,
            true,
            Some(started),
        ));
    }
    found
}

fn lead_time_evaluation(pull_requests: &[MergedPullRequest]) -> TeamEvaluation {
    let input = Input {
        now: at("2026-11-24T12:00:00Z"),
        offset: UtcOffset::UTC,
        days: 7,
        pull_requests,
        runs: &[],
        cost_by_day: &[],
    };
    evaluate(team(), &input)
}

#[test]
fn the_new_features_have_their_own_lead_times_and_the_summarys_are_over_every_pull_request() {
    let summary = lead_time_evaluation(&pull_requests_for_lead_times()).summary;
    let features = summary.new_feature.expect("three features merged");
    assert_eq!(features.merged, 3);
    assert_eq!(features.pr_lead_median_seconds, Some(2 * 86_400));
    assert_eq!(features.pr_lead_p90_seconds, Some(3 * 86_400));
    assert_eq!(features.issue_lead_median_seconds, Some(3 * 86_400));
    assert_eq!(features.issue_lead_p90_seconds, Some(4 * 86_400));
    assert_eq!(
        summary.pr_lead_median_seconds,
        Some(3600),
        "eleven pull requests, eight of them quick: the summary's median is the quick one"
    );
}

#[test]
fn only_a_new_feature_by_the_rule_of_work_type_is_one_and_a_revert_is_never() {
    let day = ("2026-11-20T00:00:00Z", "2026-11-21T00:00:00Z");
    let make = |title: &str, closes_incident: bool| {
        let mut one = pull_request(title, "", day.0, day.1, None, false, None);
        one.closes_incident = closes_incident;
        one
    };
    let found = [
        make("feat: counts", false),
        make("feat(web)!: counts too", false),
        make("fix: not one", false),
        make("fix: an incident is not one", true),
        make("refactor: not one", false),
        make("docs: not one", false),
        make("perf: not one", false),
        make("Revert \"feat: a screen\"", false),
    ];
    let features = lead_time_evaluation(&found).summary.new_feature.unwrap();
    assert_eq!(features.merged, 2);
    assert_eq!(
        features.issue_lead_median_seconds, None,
        "no issue, no sample"
    );
    assert_eq!(features.pr_lead_median_seconds, Some(86_400));
}

#[test]
fn a_period_without_a_new_feature_leaves_the_part_out() {
    let found: Vec<_> = pull_requests_for_lead_times()
        .into_iter()
        .filter(|one| !one.title.starts_with("feat"))
        .collect();
    assert_eq!(lead_time_evaluation(&found).summary.new_feature, None);
}

#[test]
fn a_board_option_names_a_work_type_by_its_name_alone() {
    use agency_proto::view::WorkType::*;
    for (option, expected) in [
        ("New feature", Some(NewFeature)),
        ("new_feature", Some(NewFeature)),
        ("NEW-FEATURE", Some(NewFeature)),
        ("Improvement", Some(Improvement)),
        ("Maintenance", Some(Maintenance)),
        ("Debt", Some(Debt)),
        ("Tech debt", Some(Debt)),
        ("tech-debt", Some(Debt)),
        ("Incident", Some(Incident)),
        ("Technical debt", None),
        ("Bug", None),
        ("", None),
        ("newFeatures", None),
    ] {
        assert_eq!(board_work_type(option), expected, "{option:?}");
    }
}

fn in_repo(mut one: MergedPullRequest, repo: &str) -> MergedPullRequest {
    one.repo = repo.into();
    one
}

fn run_in(repo: &str, commit: &str, finished: &str, verdict: Verdict) -> MainRun {
    MainRun {
        repo: repo.into(),
        ..run(commit, finished, verdict)
    }
}

/// Two repositories over three days (UTC): `acme/app` had a feature (2 h open) and a fix (1 h) on the 22nd and
/// main red from 10:00 to 12:00 on the 23rd; `acme/web` had a doc change (4 h), a feature (30 min), and a
/// revert (10 min), and main always green.
fn two_repositories() -> (Vec<MergedPullRequest>, Vec<MainRun>) {
    let make = |title: &str, repo: &str, opened: &str, merged: &str| {
        in_repo(
            pull_request(title, "", opened, merged, None, false, None),
            repo,
        )
    };
    let pull_requests = vec![
        make(
            "feat: a",
            "acme/app",
            "2026-11-22T08:00:00Z",
            "2026-11-22T10:00:00Z",
        ),
        make(
            "fix: b",
            "acme/app",
            "2026-11-22T09:00:00Z",
            "2026-11-22T10:00:00Z",
        ),
        make(
            "docs: c",
            "acme/web",
            "2026-11-23T08:00:00Z",
            "2026-11-23T12:00:00Z",
        ),
        make(
            "feat: d",
            "acme/web",
            "2026-11-23T08:00:00Z",
            "2026-11-23T08:30:00Z",
        ),
        make(
            "Revert \"feat: d\"",
            "acme/web",
            "2026-11-24T08:00:00Z",
            "2026-11-24T08:10:00Z",
        ),
    ];
    let runs = vec![
        run_in("acme/app", "a1", "2026-11-23T10:00:00Z", Verdict::Failed),
        run_in("acme/app", "a2", "2026-11-23T12:00:00Z", Verdict::Passed),
        run_in("acme/web", "w1", "2026-11-23T09:00:00Z", Verdict::Passed),
        run_in("acme/web", "w2", "2026-11-24T09:00:00Z", Verdict::Passed),
    ];
    (pull_requests, runs)
}

fn evaluate_three_days(pull_requests: &[MergedPullRequest], runs: &[MainRun]) -> TeamEvaluation {
    let costs = [(date("2026-11-23"), 9.0)];
    let input = Input {
        now: at("2026-11-24T12:00:00Z"),
        offset: UtcOffset::UTC,
        days: 3,
        pull_requests,
        runs,
        cost_by_day: &costs,
    };
    evaluate(team(), &input)
}

#[test]
fn each_repository_has_its_own_counts_lead_times_and_red_time_and_the_teams_are_as_before() {
    let (pull_requests, runs) = two_repositories();
    let team = evaluate_three_days(&pull_requests, &runs);
    let [app, web] = &team.by_repository[..] else {
        panic!("two repositories: {:?}", team.by_repository);
    };
    assert_eq!(
        (app.repo.as_str(), web.repo.as_str()),
        ("acme/app", "acme/web")
    );

    assert_eq!(app.summary.merged, counts(1, 1, 0, 0));
    assert_eq!(app.summary.reverts, 0);
    assert_eq!(
        app.summary.pr_lead_median_seconds,
        Some(5_400),
        "2 h and 1 h"
    );
    assert_eq!(app.summary.main_red_seconds, 7_200);
    assert_eq!(app.summary.longest_red_seconds, 7_200);

    assert_eq!(web.summary.merged, counts(1, 0, 1, 0));
    assert_eq!(web.summary.reverts, 1);
    assert_eq!(
        web.summary.pr_lead_median_seconds,
        Some(1_800),
        "10 min, 30 min, 4 h"
    );
    assert_eq!(web.summary.main_red_seconds, 0, "never red");
    assert_eq!(web.summary.longest_red_seconds, 0);

    assert_eq!(
        app.summary.cost_per_merged_pr_usd, None,
        "tokens are the team's, by day"
    );
    assert_eq!(web.summary.cost_per_merged_pr_usd, None);

    let whole = &team.summary;
    assert_eq!(whole.merged, counts(2, 1, 1, 0));
    assert_eq!(whole.reverts, 1);
    assert_eq!(whole.main_red_seconds, 7_200);
    assert_eq!(
        whole.pr_lead_median_seconds,
        Some(3_600),
        "five pull requests"
    );
    assert_eq!(
        whole.cost_per_merged_pr_usd,
        Some(1.8),
        "9 dollars over five"
    );
}

#[test]
fn a_team_with_one_active_repository_or_none_has_no_breakdown_and_a_quiet_one_is_not_listed() {
    let (pull_requests, runs) = two_repositories();
    let only_app: Vec<_> = pull_requests
        .iter()
        .filter(|one| one.repo == "acme/app")
        .cloned()
        .collect();
    let app_runs: Vec<_> = runs
        .iter()
        .filter(|one| one.repo == "acme/app")
        .cloned()
        .collect();
    assert!(evaluate_three_days(&only_app, &app_runs)
        .by_repository
        .is_empty());
    assert!(evaluate_three_days(&[], &[]).by_repository.is_empty());

    // A repository whose only pull request merged before the period has nothing in it.
    let mut old = pull_requests.clone();
    for one in old.iter_mut().filter(|one| one.repo == "acme/web") {
        one.merged_at = at("2026-10-01T00:00:00Z");
    }
    let found = evaluate_three_days(&old, &app_runs).by_repository;
    assert!(found.is_empty(), "{found:?}");

    // A repository with only a CI run is listed.
    let ci_only = [run_in(
        "acme/web",
        "w1",
        "2026-11-23T09:00:00Z",
        Verdict::Passed,
    )];
    let all_runs: Vec<_> = app_runs.iter().chain(&ci_only).cloned().collect();
    let found = evaluate_three_days(&only_app, &all_runs).by_repository;
    assert_eq!(found.len(), 2);
    assert_eq!(found[1].summary.merged, counts(0, 0, 0, 0));
}

#[test]
fn a_repository_that_was_red_through_the_whole_period_is_listed_though_nothing_happened_in_it() {
    let (pull_requests, mut runs) = two_repositories();
    // acme/web's main went red before the period began, and no run has finished since: its red time is still
    // counted in the team's, so the repository has to be listed for the numbers to add up.
    runs.retain(|one| one.repo != "acme/web");
    runs.push(run_in(
        "acme/web",
        "w0",
        "2026-11-10T00:00:00Z",
        Verdict::Failed,
    ));
    let only_app: Vec<_> = pull_requests
        .iter()
        .filter(|one| one.repo == "acme/app")
        .cloned()
        .collect();
    let team = evaluate_three_days(&only_app, &runs);
    let [app, web] = &team.by_repository[..] else {
        panic!("the red repository is listed: {:?}", team.by_repository);
    };
    assert_eq!(
        (app.repo.as_str(), web.repo.as_str()),
        ("acme/app", "acme/web")
    );
    assert_eq!(web.summary.merged, counts(0, 0, 0, 0));
    let whole = 2 * 86_400 + 12 * 3_600;
    assert_eq!(
        web.summary.main_red_seconds, whole,
        "red from the period's start (the 22nd) to now (the 24th at 12:00): {}",
        web.summary.main_red_seconds
    );
    assert!(
        team.summary.main_red_seconds >= web.summary.main_red_seconds
            && team.summary.main_red_seconds
                <= app.summary.main_red_seconds + web.summary.main_red_seconds,
        "the team's red time is between the longest and the sum"
    );
}
