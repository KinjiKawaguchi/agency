use super::*;
use agency_proto::clock::{self, OffsetDateTime, UtcOffset};
use agency_proto::usage::TokenCounts;
use agency_proto::{Date, Id};

fn at(raw: &str) -> OffsetDateTime {
    clock::parse(raw).unwrap()
}

fn date(raw: &str) -> Date {
    Date::parse(raw).unwrap()
}

fn merged(title: &str, merged_at: &str) -> MergedPullRequest {
    MergedPullRequest {
        repo: "acme/app".into(),
        number: 1,
        title: title.into(),
        body: String::new(),
        opened_at: at("2026-11-01T00:00:00Z"),
        merged_at: at(merged_at),
        first_commit_at: None,
        has_issue: false,
        issue_started_at: None,
        closes_incident: false,
        work_type: None,
        board: vec![],
    }
}

/// The three days 2026-11-22 to 11-24 in UTC.
fn period() -> Period {
    Period::new(at("2026-11-24T12:00:00Z"), UtcOffset::UTC, 3)
}

fn run(
    pull_requests: &[MergedPullRequest],
    costs: &[(&str, f64)],
) -> agency_proto::effort::TeamEffort {
    let cost_by_day: Vec<(Date, f64)> = costs.iter().map(|(d, c)| (date(d), *c)).collect();
    effort(
        Id::parse("app").unwrap(),
        &EffortInput {
            period: period(),
            pull_requests,
            cost_by_day: &cost_by_day,
            tokens: TokenCounts {
                input: 1000,
                output: 100,
                cache_read: 0,
                cache_write: 7,
            },
        },
    )
}

#[test]
fn a_day_s_cost_is_shared_out_by_the_pull_requests_merged_that_day() {
    let prs = [
        merged("feat(web): a", "2026-11-23T01:00:00Z"),
        merged("feat(web): b", "2026-11-23T02:00:00Z"),
        merged("fix(web): c", "2026-11-23T03:00:00Z"),
        merged("refactor: d", "2026-11-23T04:00:00Z"),
    ];
    let team = run(&prs, &[("2026-11-23", 8.0)]);
    let day = &team.by_day[0];
    assert_eq!(day.date, date("2026-11-23"));
    assert_eq!(day.cost_usd.new_feature, 4.0);
    assert_eq!(day.cost_usd.maintenance, 2.0);
    assert_eq!(day.cost_usd.debt, 2.0);
    assert_eq!(day.cost_usd.unclassified, 0.0);
    assert_eq!(day.cost_usd.improvement, 0.0);
    assert_eq!(day.cost_usd.incident, 0.0);
}

#[test]
fn a_day_with_no_merge_is_the_work_of_the_next_merges() {
    let prs = [
        merged("feat(web): a", "2026-11-23T01:00:00Z"),
        merged("docs: b", "2026-11-24T02:00:00Z"),
        merged("docs: c", "2026-11-24T03:00:00Z"),
    ];
    let team = run(&prs, &[("2026-11-22", 4.0), ("2026-11-23", 1.0)]);
    let on = |date: &str| {
        team.by_day
            .iter()
            .find(|day| day.date == self::date(date))
            .map(|day| day.cost_usd)
            .unwrap()
    };
    assert_eq!(
        on("2026-11-22").new_feature,
        4.0,
        "split like the 23rd, the next day with a merge"
    );
    assert_eq!(on("2026-11-22").maintenance, 0.0);
    assert_eq!(on("2026-11-23").new_feature, 1.0);
}

#[test]
fn a_day_after_the_last_merge_is_unclassified() {
    let prs = [merged("feat(web): a", "2026-11-22T01:00:00Z")];
    let team = run(
        &prs,
        &[
            ("2026-11-22", 1.0),
            ("2026-11-23", 2.0),
            ("2026-11-24", 4.0),
        ],
    );
    assert_eq!(team.by_type.new_feature.cost_usd, 1.0);
    assert_eq!(
        team.by_type.unclassified.cost_usd, 6.0,
        "work that has not merged yet"
    );
}

#[test]
fn cost_with_nothing_merged_in_the_period_is_unclassified() {
    let team = run(&[], &[("2026-11-24", 5.0)]);
    assert_eq!(team.by_type.unclassified.cost_usd, 5.0);
    assert_eq!(team.by_type.new_feature.cost_usd, 0.0);
    assert_eq!(team.by_type.unclassified.tokens.input, 1000);
}

#[test]
fn a_revert_and_a_type_nobody_knows_are_unclassified() {
    let prs = [
        merged("Revert \"feat: a\"", "2026-11-24T01:00:00Z"),
        merged("perf: faster", "2026-11-24T02:00:00Z"),
    ];
    let team = run(&prs, &[("2026-11-24", 2.0)]);
    assert_eq!(team.by_day[0].cost_usd.unclassified, 2.0);
    assert_eq!(team.by_day[0].cost_usd.new_feature, 0.0);
}

#[test]
fn a_pull_request_outside_the_period_gives_no_weight() {
    let prs = [
        merged("feat: old", "2026-11-01T01:00:00Z"),
        merged("docs: now", "2026-11-24T01:00:00Z"),
    ];
    let team = run(&prs, &[("2026-11-23", 3.0)]);
    assert_eq!(team.by_day[0].cost_usd.maintenance, 3.0);
    assert_eq!(team.by_day[0].cost_usd.new_feature, 0.0);
}

#[test]
fn a_day_outside_the_period_and_a_day_without_cost_are_left_out() {
    let prs = [merged("feat: a", "2026-11-24T01:00:00Z")];
    let team = run(
        &prs,
        &[
            ("2026-11-10", 9.0),
            ("2026-11-23", 0.0),
            ("2026-11-24", 1.0),
        ],
    );
    let dates: Vec<&str> = team.by_day.iter().map(|day| day.date.as_str()).collect();
    assert_eq!(dates, ["2026-11-24"]);
    assert_eq!(team.by_type.new_feature.cost_usd, 1.0);
}

#[test]
fn the_day_s_own_time_zone_decides_which_day_a_merge_is_on() {
    let tokyo = UtcOffset::from_hms(9, 0, 0).unwrap();
    let period = Period::new(at("2026-11-24T12:00:00Z"), tokyo, 2);
    // 2026-11-23T16:00Z is 11-24 01:00 in Tokyo: it is a merge of the 24th, not of the 23rd.
    let prs = [merged("feat: a", "2026-11-23T16:00:00Z")];
    let costs = [(date("2026-11-24"), 2.0)];
    let team = effort(
        Id::parse("app").unwrap(),
        &EffortInput {
            period,
            pull_requests: &prs,
            cost_by_day: &costs,
            tokens: TokenCounts::default(),
        },
    );
    assert_eq!(team.from, date("2026-11-23"));
    assert_eq!(team.by_day[0].cost_usd.new_feature, 2.0);
}

#[test]
fn each_type_adds_up_to_its_days_and_the_tokens_follow_the_cost() {
    let prs = [
        merged("feat: a", "2026-11-23T01:00:00Z"),
        merged("fix: b", "2026-11-24T01:00:00Z"),
    ];
    let team = run(&prs, &[("2026-11-23", 3.0), ("2026-11-24", 1.0)]);
    assert_eq!(team.by_type.new_feature.cost_usd, 3.0);
    assert_eq!(team.by_type.maintenance.cost_usd, 1.0);
    assert_eq!(team.by_type.new_feature.tokens.input, 750);
    assert_eq!(team.by_type.maintenance.tokens.input, 250);
    assert_eq!(team.by_type.new_feature.tokens.cache_write, 5);
    assert_eq!(team.by_type.maintenance.tokens.cache_write, 1);
    assert_eq!(
        team.by_type.unclassified.tokens.cache_write, 1,
        "what rounding down left over, so that the types add up to the 7 the team used"
    );
    assert_eq!(team.by_type.unclassified.tokens.input, 0);
}

#[test]
fn the_answer_passes_the_check_every_node_makes() {
    let prs = [merged("feat: a", "2026-11-24T01:00:00Z")];
    let one = run(&prs, &[("2026-11-23", 3.0), ("2026-11-24", 1.0)]);
    let answer = agency_proto::effort::Effort {
        org: Id::parse("acme").unwrap(),
        days: 3,
        from: period().first(),
        until: period().last(),
        generated_at: agency_proto::Timestamp::parse("2026-11-24T12:00:00Z").unwrap(),
        teams: vec![one],
        unavailable: vec![],
    };
    assert_eq!(answer.check(&Id::parse("acme").unwrap(), 3), Ok(()));
}

#[test]
fn a_fix_of_an_issue_labeled_incident_is_incident_and_only_a_fix_is() {
    let mut incident = merged("fix(web): the crash", "2026-11-24T01:00:00Z");
    incident.closes_incident = true;
    let mut feature = merged("feat(web): a screen", "2026-11-24T02:00:00Z");
    feature.closes_incident = true;
    let team = run(&[incident, feature], &[("2026-11-24", 4.0)]);
    assert_eq!(team.by_day[0].cost_usd.incident, 2.0);
    assert_eq!(team.by_day[0].cost_usd.new_feature, 2.0);
    assert_eq!(team.by_day[0].cost_usd.maintenance, 0.0);
}

#[test]
fn the_boards_work_type_decides_before_the_title() {
    let mut improvement = merged("feat(web): a screen", "2026-11-24T01:00:00Z");
    improvement.work_type = Some(agency_proto::view::WorkType::Improvement);
    let mut debt = merged("fix(web): a crash", "2026-11-24T02:00:00Z");
    debt.work_type = Some(agency_proto::view::WorkType::Debt);
    debt.closes_incident = true;
    let team = run(&[improvement, debt], &[("2026-11-24", 4.0)]);
    let cost = team.by_day[0].cost_usd;
    assert_eq!((cost.improvement, cost.debt), (2.0, 2.0));
    assert_eq!(
        (cost.new_feature, cost.maintenance, cost.incident),
        (0.0, 0.0, 0.0)
    );
}

#[test]
fn a_revert_is_unclassified_even_when_the_issue_it_closes_has_a_work_type() {
    let mut revert = merged("Revert \"feat(web): a screen\"", "2026-11-24T01:00:00Z");
    revert.work_type = Some(agency_proto::view::WorkType::Debt);
    revert.closes_incident = true;
    let mut body_revert = merged("fix: undo it", "2026-11-24T02:00:00Z");
    body_revert.body = "This reverts commit 0123456789abcdef.".into();
    body_revert.work_type = Some(agency_proto::view::WorkType::Improvement);
    let team = run(&[revert, body_revert], &[("2026-11-24", 4.0)]);
    let cost = team.by_day[0].cost_usd;
    assert_eq!(cost.unclassified, 4.0, "a revert is no type of work");
    assert_eq!(
        (cost.debt, cost.improvement, cost.incident),
        (0.0, 0.0, 0.0)
    );
}
