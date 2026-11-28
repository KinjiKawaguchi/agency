//! KPIs that take their daily number from the node's own delivery numbers: one number for each finished day,
//! never over a number that is already there, and never twice. All figures here are made up.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_core::config::NodeConfig;
use agency_core::connectors::{check_units, write_finished_days};
use agency_core::delivery::{DeliveryFacts, MainRun, MergedPullRequest, Verdict};
use agency_core::kpis;
use agency_core::outcomes::{record, record_new};
use agency_core::outcomes_config::{Connector, Metric};
use agency_core::ports::ConfigStore as _;
use agency_proto::clock;
use agency_proto::observation::{Observation, ObservationInput, Source};
use agency_proto::outcome::{Currency, GoodWhen, Kpi, KpiKind, KpiUnit};
use common::{alice, id, Running};
use std::sync::{Arc, Mutex};

const USAGE: &str = include_str!("../../../testdata/tokenusage.sample.json");

fn at(raw: &str) -> agency_proto::clock::OffsetDateTime {
    clock::parse(raw).unwrap()
}

fn kpi(name: &str, unit: KpiUnit) -> Kpi {
    Kpi {
        id: id(name),
        label: name.into(),
        kind: KpiKind::Product,
        unit,
        good_when: GoodWhen::Higher,
    }
}

fn pull_request(number: u64, title: &str, opened: &str, merged: &str) -> MergedPullRequest {
    MergedPullRequest {
        repo: "acme/app".into(),
        number,
        title: title.into(),
        body: String::new(),
        opened_at: at(opened),
        merged_at: at(merged),
        first_commit_at: None,
        has_issue: false,
        issue_started_at: None,
        closes_incident: false,
        work_type: None,
        board: vec![],
    }
}

/// At the node's time (2026-11-24T12:00Z): on the 23rd a feature (an hour from opened to merged) and a fix
/// merged, and main was red from 10:00 to 12:00. The usage of the sample has 9.55 dollars on the 23rd.
fn facts() -> DeliveryFacts {
    let run = |commit: &str, finished: &str, verdict| MainRun {
        repo: "acme/app".into(),
        commit: commit.into(),
        finished_at: at(finished),
        verdict,
    };
    DeliveryFacts {
        pull_requests: vec![
            pull_request(
                1,
                "feat: a screen",
                "2026-11-23T08:00:00Z",
                "2026-11-23T09:00:00Z",
            ),
            pull_request(
                2,
                "fix: a crash",
                "2026-11-23T08:00:00Z",
                "2026-11-23T11:00:00Z",
            ),
        ],
        runs: vec![
            run("abc", "2026-11-23T10:00:00Z", Verdict::Failed),
            run("def", "2026-11-23T12:00:00Z", Verdict::Passed),
        ],
    }
}

fn connector(kpi: &str, metric: Metric) -> Connector {
    Connector {
        org: id("acme"),
        team: id("app"),
        kpi: id(kpi),
        metric,
    }
}

/// alice with the four KPIs of the agency org's delivery, a team with repositories, its facts and usage,
/// and a connector for each KPI.
async fn alice_with_connectors() -> (Running, Arc<Mutex<MemoryRuntimeState>>) {
    let (alice, memory) = alice().await;
    let usd = KpiUnit::Money {
        currency: Currency::parse("USD").unwrap(),
    };
    for (name, unit) in [
        ("features", KpiUnit::Count),
        ("lead", KpiUnit::Seconds),
        ("red", KpiUnit::Seconds),
        ("cost", usd),
    ] {
        kpis::add(&alice.node, &id("acme"), kpi(name, unit)).unwrap();
    }
    alice
        .node
        .update_config(|config| {
            let team = config
                .teams
                .remove(0)
                .with_delivery(vec!["acme/app".into()]);
            config.teams.insert(0, team);
            config.outcomes.connectors = vec![
                connector("features", Metric::NewFeaturesMerged),
                connector("lead", Metric::NewFeaturePrLeadMedianSeconds),
                connector("red", Metric::MainRedSeconds),
                connector("cost", Metric::CostPerMergedPrUsd),
            ];
        })
        .unwrap();
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    memory.lock().unwrap().usage = Some(serde_json::from_str(USAGE).unwrap());
    (alice, memory)
}

fn of(alice: &Running, kpi: &str) -> Vec<Observation> {
    alice
        .node
        .store()
        .observations(&id("acme"))
        .into_iter()
        .filter(|one| one.kpi == id(kpi))
        .collect()
}

fn on(alice: &Running, kpi: &str, date: &str) -> Option<Observation> {
    of(alice, kpi)
        .into_iter()
        .find(|one| one.date.as_str() == date)
}

/// A team on a GitHub Enterprise Server: the connectors read its delivery numbers from its own host, as the
/// evaluation does, and never from github.com.
#[tokio::test]
async fn the_connectors_read_a_team_on_its_own_github_host() {
    let (alice, _) = alice_with_connectors().await;
    alice
        .node
        .update_config(|config| {
            let team = config
                .teams
                .remove(0)
                .with_delivery_on("git.example.test", vec!["acme/app".into()]);
            config.teams.insert(0, team);
        })
        .unwrap();
    let pass = write_finished_days(&alice.node).await;
    assert!(
        pass.failures.is_empty() && pass.refused.is_empty(),
        "{pass:?}"
    );
    let asked = alice.delivery.0.lock().unwrap().asked.clone();
    assert!(!asked.is_empty(), "the connectors read the team's numbers");
    for (site, _) in &asked {
        assert_eq!(site.host.as_deref(), Some("git.example.test"), "{site:?}");
        assert_eq!(site.repos, ["acme/app"]);
    }
    assert_eq!(of(&alice, "features").len(), 30, "and wrote what they read");
}

#[tokio::test]
async fn a_connected_kpi_gets_one_number_for_each_finished_day_from_the_connector() {
    let (alice, _) = alice_with_connectors().await;
    let pass = write_finished_days(&alice.node).await;
    assert!(
        pass.failures.is_empty() && pass.refused.is_empty(),
        "{pass:?}"
    );

    let features = of(&alice, "features");
    assert_eq!(
        features.len(),
        30,
        "the 30 finished days before today, each with its count, which is 0 on most"
    );
    assert_eq!(features[0].date.as_str(), "2026-10-25");
    assert_eq!(features[29].date.as_str(), "2026-11-23");
    assert!(
        features.iter().all(|one| one.source
            == Source::Connector {
                name: "delivery".into()
            }
            && one.entered_by == id("alice")),
        "named as a connector, entered by the node's account"
    );
    let value = |kpi: &str, date: &str| on(&alice, kpi, date).map(|one| one.value);
    assert_eq!(value("features", "2026-11-23"), Some(1.0));
    assert_eq!(value("features", "2026-11-22"), Some(0.0));
    assert_eq!(value("features", "2026-11-24"), None, "today is not over");
    assert_eq!(value("red", "2026-11-23"), Some(7200.0));
    assert_eq!(value("red", "2026-11-22"), Some(0.0));
    assert_eq!(of(&alice, "red").len(), 30);
    assert_eq!(
        of(&alice, "lead").len(),
        1,
        "a median of no pull request is no number, never 0"
    );
    assert_eq!(value("lead", "2026-11-23"), Some(3600.0));
    assert_eq!(of(&alice, "cost").len(), 1);
    assert_eq!(
        value("cost", "2026-11-23"),
        Some(4.78),
        "9.55 over the two merged"
    );
}

#[tokio::test]
async fn a_day_with_a_number_entered_by_hand_is_left_and_a_day_is_not_written_twice() {
    let (alice, _) = alice_with_connectors().await;
    let by_hand = ObservationInput {
        kpi: id("features"),
        date: agency_proto::Date::parse("2026-11-23").unwrap(),
        value: 99.0,
        note: Some("counted by hand".into()),
        connector: None,
    };
    record(&alice.node, &id("acme"), by_hand).await.unwrap();

    let first = write_finished_days(&alice.node).await;
    assert!(first.written > 0);
    let kept = of(&alice, "features");
    assert_eq!(kept.len(), 30, "29 written and the one entered by hand");
    let hand = on(&alice, "features", "2026-11-23").unwrap();
    assert_eq!((hand.value, hand.source), (99.0, Source::Manual));

    // What GitHub says changes afterwards: the days already written stay as they were.
    let mut changed = facts();
    changed.pull_requests[0].merged_at = at("2026-11-23T10:00:00Z");
    alice.delivery.0.lock().unwrap().facts = Some(Ok(changed));
    let second = write_finished_days(&alice.node).await;
    assert_eq!(second.written, 0, "{second:?}");
    assert_eq!(of(&alice, "features").len(), 30);
    assert_eq!(
        on(&alice, "lead", "2026-11-23").unwrap().value,
        3600.0,
        "not written again with the correction (two hours now)"
    );
}

#[tokio::test]
async fn a_number_worked_out_for_a_day_that_has_one_is_never_entered_over_it() {
    let (alice, _) = alice_with_connectors().await;
    let input = |value: f64, connector: Option<&str>| ObservationInput {
        kpi: id("features"),
        date: agency_proto::Date::parse("2026-11-20").unwrap(),
        value,
        note: None,
        connector: connector.map(str::to_owned),
    };
    record(&alice.node, &id("acme"), input(5.0, None))
        .await
        .unwrap();
    let entered = record_new(&alice.node, &id("acme"), input(1.0, Some("delivery")))
        .await
        .unwrap();
    assert!(entered.is_none(), "the day has a number");
    assert_eq!(of(&alice, "features").len(), 1);
    let on_another_day = ObservationInput {
        date: agency_proto::Date::parse("2026-11-21").unwrap(),
        ..input(1.0, Some("delivery"))
    };
    assert!(record_new(&alice.node, &id("acme"), on_another_day)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn nothing_is_written_while_the_numbers_cannot_be_read_and_the_days_come_later() {
    let (alice, memory) = alice_with_connectors().await;
    alice.delivery.0.lock().unwrap().facts = Some(Err("GitHub did not answer".into()));
    let pass = write_finished_days(&alice.node).await;
    assert_eq!(pass.written, 0);
    assert!(
        pass.failures.len() == 4 && pass.failures[0].contains("outcomes.connectors[0]"),
        "{pass:?}"
    );
    assert!(
        of(&alice, "features").is_empty(),
        "no zero stands for unknown"
    );

    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    memory.lock().unwrap().usage = None;
    let pass = write_finished_days(&alice.node).await;
    assert_eq!(
        pass.failures.len(),
        1,
        "only the cost needs the runtime's usage: {pass:?}"
    );
    assert!(pass.failures[0].contains("outcomes.connectors[3]"));
    assert_eq!(of(&alice, "features").len(), 30);
    assert!(
        of(&alice, "cost").is_empty(),
        "a cost of 0 is not an unknown cost"
    );

    memory.lock().unwrap().usage = Some(serde_json::from_str(USAGE).unwrap());
    let pass = write_finished_days(&alice.node).await;
    assert!(pass.failures.is_empty(), "{pass:?}");
    assert_eq!(of(&alice, "cost").len(), 1);
}

#[tokio::test]
async fn a_kpi_the_definition_does_not_list_yet_waits_and_a_unit_that_does_not_fit_is_refused() {
    let (alice, _) = alice().await;
    alice
        .node
        .update_config(|config| {
            let team = config
                .teams
                .remove(0)
                .with_delivery(vec!["acme/app".into()]);
            config.teams.insert(0, team);
            config.outcomes.connectors = vec![connector("later", Metric::NewFeaturesMerged)];
        })
        .unwrap();
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    let pass = write_finished_days(&alice.node).await;
    assert_eq!(pass, Default::default(), "not listed yet: nothing to say");
    assert_eq!(check_units(&alice.node), Ok(()));

    kpis::add(&alice.node, &id("acme"), kpi("later", KpiUnit::Percent)).unwrap();
    let error = check_units(&alice.node).unwrap_err();
    assert!(
        error.contains("outcomes.connectors[0].metric: newFeaturesMerged is a number of count"),
        "{error}"
    );
    let pass = write_finished_days(&alice.node).await;
    assert_eq!((pass.written, pass.refused.len()), (0, 1), "{pass:?}");
    assert!(of(&alice, "later").is_empty());
}

#[tokio::test]
async fn the_unit_each_metric_fits_is_its_own() {
    let usd = KpiUnit::Money {
        currency: Currency::parse("USD").unwrap(),
    };
    let jpy = KpiUnit::Money {
        currency: Currency::parse("JPY").unwrap(),
    };
    let units = [
        KpiUnit::Count,
        KpiUnit::Seconds,
        usd,
        jpy,
        KpiUnit::Percent,
        KpiUnit::Ratio,
    ];
    let fits =
        |metric: Metric| -> Vec<bool> { units.iter().map(|unit| metric.fits(unit)).collect() };
    assert_eq!(
        fits(Metric::NewFeaturesMerged),
        [true, false, false, false, false, false]
    );
    for metric in [
        Metric::NewFeaturePrLeadMedianSeconds,
        Metric::MainRedSeconds,
        Metric::PrLeadMedianSeconds,
    ] {
        assert_eq!(fits(metric), [false, true, false, false, false, false]);
    }
    assert_eq!(
        fits(Metric::CostPerMergedPrUsd),
        [false, false, true, false, false, false],
        "money in USD alone"
    );
}

#[tokio::test]
async fn a_connector_that_names_what_the_node_cannot_serve_stops_the_node_and_names_the_setting() {
    let dir = tempfile::tempdir().unwrap();
    let data = agency_adapters::files::DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let account = agency_core::config::Account {
        id: id("alice"),
        label: "ALICE".into(),
    };
    let team = agency_core::config::HostedTeam::new(
        id("acme"),
        id("app"),
        id("alice"),
        agency_core::config::RuntimeCommand::of(vec!["/nonexistent".into()]),
    );
    let with_repos = team.clone().with_delivery(vec!["acme/app".into()]);
    let cases = [
        (
            vec![],
            vec![connector("x", Metric::MainRedSeconds)],
            "outcomes.connectors[0].team: acme/app does not run on this node",
        ),
        (
            vec![team],
            vec![connector("x", Metric::MainRedSeconds)],
            "has no delivery repositories",
        ),
        (
            vec![with_repos],
            vec![
                connector("x", Metric::MainRedSeconds),
                connector("x", Metric::PrLeadMedianSeconds),
            ],
            "outcomes.connectors[1].kpi: x is already set by outcomes.connectors[0]",
        ),
    ];
    for (teams, connectors, wanted) in cases {
        let mut config = NodeConfig::new(
            "http://127.0.0.1:1",
            "127.0.0.1:1",
            account.clone(),
            "scrum",
        );
        config.teams = teams;
        config.outcomes.connectors = connectors;
        data.save_config(&config).unwrap();
        let error = match agency_node::open(data.clone(), agency_node::Overrides::default()) {
            Ok(_) => panic!("the node started"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains(wanted), "{error}");
    }
}

#[tokio::test]
async fn a_team_that_shares_its_repository_is_read_for_its_own_pull_requests_only() {
    let (alice, _) = alice_with_connectors().await;
    alice
        .node
        .update_config(|config| {
            let team = config.teams.remove(0).with_team_value("Team", "Keel");
            config.teams.insert(0, team);
        })
        .unwrap();
    write_finished_days(&alice.node).await;
    assert_eq!(
        on(&alice, "features", "2026-11-23").map(|one| one.value),
        Some(0.0),
        "neither of the day's pull requests closes an issue of the team Keel"
    );
}

#[tokio::test]
async fn the_oldest_day_read_is_never_entered_since_its_red_time_can_be_cut() {
    let (alice, _) = alice_with_connectors().await;
    let mut older = facts();
    // 2026-10-24 is 31 days before today: the first day of the 32 read, one more than the 30 that are entered.
    older.pull_requests.push(pull_request(
        9,
        "feat: long ago",
        "2026-10-24T08:00:00Z",
        "2026-10-24T09:00:00Z",
    ));
    alice.delivery.0.lock().unwrap().facts = Some(Ok(older));
    write_finished_days(&alice.node).await;
    for kpi in ["features", "lead", "red", "cost"] {
        assert!(on(&alice, kpi, "2026-10-24").is_none(), "{kpi}");
    }
    assert_eq!(
        on(&alice, "features", "2026-10-25").map(|one| one.value),
        Some(0.0)
    );
    assert_eq!(
        of(&alice, "features")
            .iter()
            .map(|one| one.date.as_str())
            .min(),
        Some("2026-10-25")
    );
}

#[tokio::test]
async fn a_node_whose_connector_does_not_fit_its_kpis_unit_does_not_start() {
    let (alice, _) = alice().await;
    kpis::add(&alice.node, &id("acme"), kpi("shipped", KpiUnit::Percent)).unwrap();
    alice
        .node
        .update_config(|config| {
            let team = config
                .teams
                .remove(0)
                .with_delivery(vec!["acme/app".into()]);
            config.teams.insert(0, team);
            config.outcomes.connectors = vec![connector("shipped", Metric::NewFeaturesMerged)];
        })
        .unwrap();
    let data = agency_adapters::files::DataDir::new(alice.data_dir());
    let error = match agency_node::open(data, agency_node::Overrides::default()) {
        Ok(_) => panic!("the node started"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("outcomes.connectors[0].metric: newFeaturesMerged is a number of count"),
        "{error}"
    );
}
