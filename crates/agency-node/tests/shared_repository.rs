//! Teams that share one repository: each counts only the pull requests whose issue the board gives its
//! `Team` option, and a team that names no option counts them all.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_core::config::{HostedTeam, NodeConfig, RuntimeCommand};
use agency_core::delivery::{BoardValue, DeliveryFacts, MergedPullRequest};
use agency_core::orgs;
use agency_core::ports::ConfigStore as _;
use agency_proto::clock;
use agency_proto::effort::Effort;
use agency_proto::evaluation::Evaluation;
use agency_proto::org::TeamPlace;
use common::{alice, id, Running};
use reqwest::Client;
use std::sync::{Arc, Mutex};

const USAGE: &str = include_str!("../../../testdata/tokenusage.sample.json");

fn at(raw: &str) -> agency_proto::clock::OffsetDateTime {
    clock::parse(raw).unwrap()
}

/// A pull request that closes an issue whose board has these (field, option) pairs, or none when the
/// pull request closes no issue (`None`).
fn merged(number: u64, title: &str, board: Option<&[(&str, &str)]>) -> MergedPullRequest {
    MergedPullRequest {
        repo: "acme/app".into(),
        number,
        title: title.into(),
        body: String::new(),
        opened_at: at("2026-11-24T08:00:00Z"),
        merged_at: at("2026-11-24T09:00:00Z"),
        first_commit_at: None,
        has_issue: board.is_some(),
        issue_started_at: None,
        closes_incident: false,
        work_type: None,
        board: board
            .unwrap_or_default()
            .iter()
            .map(|(field, value)| BoardValue {
                field: (*field).into(),
                value: (*value).into(),
            })
            .collect(),
    }
}

/// Three pull requests of the team Keel, one of the team Web, one that closes an issue with no team, and
/// one that closes no issue.
fn facts() -> DeliveryFacts {
    let keel = [("Status", "Done"), ("Team", "Keel")];
    DeliveryFacts {
        pull_requests: vec![
            merged(1, "feat: a", Some(&keel)),
            merged(2, "fix: b", Some(&keel)),
            merged(3, "docs: c", Some(&[("team", "keel")])),
            merged(4, "feat: d", Some(&[("Team", "Web")])),
            merged(5, "chore: e", Some(&[("Status", "Done")])),
            merged(6, "fix: f", None),
        ],
        runs: vec![],
    }
}

/// alice hosts the teams app and web, which share acme/app. `keys` is the (field, option) each names.
async fn alice_with(
    app: Option<(&str, &str)>,
    web: Option<(&str, &str)>,
) -> (Running, Arc<Mutex<MemoryRuntimeState>>) {
    let (alice, memory) = alice().await;
    orgs::add_team(
        &alice.node,
        &id("acme"),
        TeamPlace::new(id("web"), "web", alice.url()),
    )
    .unwrap();
    alice
        .node
        .update_config(|config| {
            let owner = id("alice");
            let runtime = || RuntimeCommand::of(vec!["/nonexistent".into()]);
            let mut web_team = HostedTeam::new(id("acme"), id("web"), owner, runtime());
            let mut app_team = config.teams.remove(0);
            for (team, keys) in [(&mut app_team, app), (&mut web_team, web)] {
                *team = team.clone().with_delivery(vec!["acme/app".into()]);
                if let Some((field, value)) = keys {
                    *team = team.clone().with_team_value(field, value);
                }
            }
            config.teams = vec![app_team, web_team];
        })
        .unwrap();
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    (alice, memory)
}

async fn evaluation(running: &Running) -> Evaluation {
    Client::new()
        .get(format!(
            "{}/api/orgs/acme/evaluation?days=14",
            running.url()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

fn merged_of(evaluation: &Evaluation, team: &str) -> u32 {
    let team = evaluation
        .teams
        .iter()
        .find(|one| one.team == id(team))
        .expect("the team has numbers");
    let merged = team.summary.merged;
    merged.feat
        + merged.fix
        + merged.refactor
        + merged.docs
        + merged.test
        + merged.chore
        + merged.other
}

#[tokio::test]
async fn teams_sharing_a_repository_each_count_only_the_pull_requests_of_their_own_team() {
    let (alice, _) = alice_with(Some(("Team", "Keel")), Some(("Team", "Web"))).await;
    let answer = evaluation(&alice).await;
    assert!(answer.unavailable.is_empty(), "{:?}", answer.unavailable);
    assert_eq!(
        merged_of(&answer, "app"),
        3,
        "three for Keel, with the field and option matched without regard to case"
    );
    assert_eq!(merged_of(&answer, "web"), 1);
    let feats = |team: &str| {
        let one = answer
            .teams
            .iter()
            .find(|one| one.team == id(team))
            .unwrap();
        one.summary.merged.feat
    };
    assert_eq!((feats("app"), feats("web")), (1, 1));
}

#[tokio::test]
async fn a_team_that_names_no_option_counts_every_pull_request_of_its_repositories() {
    let (alice, _) = alice_with(None, Some(("Team", "Web"))).await;
    let answer = evaluation(&alice).await;
    assert_eq!(
        merged_of(&answer, "app"),
        6,
        "as before the setting existed"
    );
    assert_eq!(merged_of(&answer, "web"), 1);
}

#[tokio::test]
async fn a_team_whose_option_no_issue_has_counts_nothing_not_everything() {
    let (alice, _) = alice_with(Some(("Team", "Apple")), None).await;
    let answer = evaluation(&alice).await;
    assert_eq!(merged_of(&answer, "app"), 0);
}

#[tokio::test]
async fn the_effort_of_a_team_is_split_over_its_own_pull_requests_only() {
    let (alice, memory) = alice_with(None, Some(("Team", "Web"))).await;
    memory.lock().unwrap().usage = Some(serde_json::from_str(USAGE).unwrap());
    let effort: Effort = Client::new()
        .get(format!("{}/api/orgs/acme/effort?days=14", alice.url()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cost = |team: &str| {
        let one = effort
            .teams
            .iter()
            .find(|one| one.team == id(team))
            .unwrap();
        (
            one.by_type.new_feature.cost_usd,
            one.by_type.maintenance.cost_usd,
        )
    };
    // The usage has 9.55 dollars on the 23rd and 3.75 on the 24th. The only pull request of the team
    // Web is a feature, so all of it is new feature; the team that names no option shares it over all six.
    assert_eq!(cost("web"), (13.3, 0.0));
    assert!(
        cost("app").0 < 13.3,
        "app counts every pull request: {:?}",
        cost("app")
    );
}

#[tokio::test]
async fn a_team_field_without_an_option_or_the_other_way_round_stops_the_node_and_names_the_setting(
) {
    let dir = tempfile::tempdir().unwrap();
    let data = agency_adapters::files::DataDir::new(dir.path().join("node"));
    data.create_key().unwrap();
    let account = agency_core::config::Account {
        id: id("alice"),
        label: "ALICE".into(),
    };
    for (edit, wanted) in [
        (
            r#"{"teamField":"Team"}"#,
            "teams[0].delivery sets only one of teamField and teamValue",
        ),
        (
            r#"{"teamValue":"Keel"}"#,
            "teams[0].delivery sets only one of teamField and teamValue",
        ),
        (
            r#"{"teamField":" ","teamValue":"Keel"}"#,
            "teams[0].delivery.teamField is empty",
        ),
    ] {
        let mut config = NodeConfig::new(
            "http://127.0.0.1:1",
            "127.0.0.1:1",
            account.clone(),
            "scrum",
        );
        let mut delivery: serde_json::Value = serde_json::from_str(edit).unwrap();
        delivery["repos"] = serde_json::json!(["acme/app"]);
        let team = HostedTeam::new(
            id("acme"),
            id("app"),
            id("alice"),
            RuntimeCommand::of(vec!["/nonexistent".into()]),
        );
        let mut team_json = serde_json::to_value(team).unwrap();
        team_json["delivery"] = delivery;
        config.teams = vec![serde_json::from_value(team_json).unwrap()];
        data.save_config(&config).unwrap();
        let error = match agency_node::open(data.clone(), agency_node::Overrides::default()) {
            Ok(_) => panic!("the node started"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains(wanted), "{error}");
    }
}
