//! A team's effort by work type, end to end: worked out on the node the team runs on from the facts its
//! delivery source reads and the usage its runtime reports, read there by a member over a signed path that
//! names the range, and read for every team of an org through the Client API.
mod common;

use agency_adapters::peers::read_headers;
use agency_core::delivery::{DeliveryFacts, MergedPullRequest};
use agency_core::read_auth::sign_read;
use agency_proto::clock;
use agency_proto::effort::Effort;
use agency_proto::Id;
use common::{alice, id, joined_bob, memory_runtime, node_with, read_delivery_on, Running};
use reqwest::{Client, StatusCode};
use serde_json::Value;

const USAGE: &str = include_str!("../../../testdata/tokenusage.sample.json");

fn at(raw: &str) -> agency_proto::clock::OffsetDateTime {
    clock::parse(raw).unwrap()
}

fn merged(number: u64, title: &str, merged_at: &str) -> MergedPullRequest {
    MergedPullRequest {
        repo: "acme/app".into(),
        number,
        title: title.into(),
        body: String::new(),
        opened_at: at("2026-11-20T00:00:00Z"),
        merged_at: at(merged_at),
        first_commit_at: None,
        has_issue: false,
        issue_started_at: None,
        closes_incident: false,
        work_type: None,
        board: vec![],
    }
}

/// At the node's time (2026-11-24T12:00Z): a fix merged on the 23rd and a feature on the 24th. The usage
/// of the sample has 9.55 dollars on the 23rd and 3.75 on the 24th.
fn facts() -> DeliveryFacts {
    DeliveryFacts {
        pull_requests: vec![
            merged(100, "fix(web): a crash", "2026-11-23T10:00:00Z"),
            merged(101, "feat(web): add a screen", "2026-11-24T09:00:00Z"),
        ],
        runs: vec![],
    }
}

fn configure(alice: &Running) {
    alice
        .node
        .update_config(|config| {
            let at = config
                .teams
                .iter()
                .position(|team| team.team == id("app"))
                .expect("alice hosts acme/app");
            let team = config.teams.remove(at);
            config
                .teams
                .insert(at, team.with_delivery(vec!["acme/app".into()]));
        })
        .unwrap();
}

fn path(days: u32) -> String {
    format!("/fed/v1/teams/acme/app/effort?days={days}")
}

/// A federation read of alice's team's effort by `reader`, signed for `signed_path`, sent to `path`.
async fn federated(
    alice: &Running,
    reader: &Running,
    account: &Id,
    signed_path: &str,
    path: &str,
) -> (StatusCode, Value) {
    let auth = sign_read(
        reader.node.key(),
        account,
        "GET",
        signed_path,
        reader.node.now(),
    );
    let mut request = Client::new().get(format!("{}{path}", alice.url()));
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn client_effort(running: &Running, query: &str) -> (StatusCode, Value) {
    let response = Client::new()
        .get(format!("{}/api/orgs/acme/effort{query}", running.url()))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// A running node whose team has repositories, facts, and usage.
async fn alice_with_everything() -> Running {
    let (alice, memory) = alice().await;
    configure(&alice);
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    memory.lock().unwrap().usage = Some(serde_json::from_str(USAGE).unwrap());
    alice
}

#[tokio::test]
async fn a_team_on_a_github_host_of_its_own_has_its_effort_read_from_that_host() {
    let (alice, memory) = alice().await;
    read_delivery_on(&alice, "git.example.test");
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    memory.lock().unwrap().usage = Some(serde_json::from_str(USAGE).unwrap());
    let bob = joined_bob(&alice).await;

    let (status, answer) = federated(&alice, &bob, &id("bob"), &path(14), &path(14)).await;
    assert_eq!(status, 200, "{answer}");
    let asked = alice.delivery.0.lock().unwrap().asked.clone();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].0.host.as_deref(), Some("git.example.test"));
}

#[tokio::test]
async fn a_member_reads_where_a_teams_tokens_went_by_the_work_that_merged() {
    let alice = alice_with_everything().await;
    let bob = joined_bob(&alice).await;

    let (status, answer) = federated(&alice, &bob, &id("bob"), &path(14), &path(14)).await;
    assert_eq!(status, 200, "{answer}");
    let effort: Effort = serde_json::from_value(answer).unwrap();
    assert_eq!(effort.check(&id("acme"), 14), Ok(()));
    assert!(effort.unavailable.is_empty());
    let team = &effort.teams[0];
    assert_eq!(team.team, id("app"));
    assert_eq!(
        (team.from.as_str(), team.until.as_str()),
        ("2026-11-11", "2026-11-24")
    );
    let days: Vec<(&str, f64, f64)> = team
        .by_day
        .iter()
        .map(|day| {
            (
                day.date.as_str(),
                day.cost_usd.maintenance,
                day.cost_usd.new_feature,
            )
        })
        .collect();
    assert_eq!(days, [("2026-11-23", 9.55, 0.0), ("2026-11-24", 0.0, 3.75)]);
    assert_eq!(team.by_type.maintenance.cost_usd, 9.55);
    assert_eq!(team.by_type.new_feature.cost_usd, 3.75);
    assert_eq!(team.by_type.unclassified.cost_usd, 0.0);
    let input = |usage: &agency_proto::effort::TypeUsage| usage.tokens.input;
    assert_eq!(
        (
            input(&team.by_type.maintenance),
            input(&team.by_type.new_feature),
            input(&team.by_type.unclassified)
        ),
        (134_992, 53_007, 1),
        "the period's 188 000 input tokens split by cost, and what rounding down left over unclassified"
    );
}

#[tokio::test]
async fn a_team_without_repositories_usage_or_facts_is_unavailable_and_never_zero() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    let read = |days: u32| {
        let (alice, bob) = (&alice, &bob);
        async move { federated(alice, bob, &id("bob"), &path(days), &path(days)).await }
    };

    let (status, answer) = read(7).await;
    assert_eq!(status, 200, "the reason is the answer, not an error");
    assert_eq!(answer["teams"], serde_json::json!([]));
    assert!(answer["unavailable"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("no repositories"));

    configure(&alice);
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    let (_, answer) = read(8).await;
    assert_eq!(
        answer["unavailable"][0]["reason"], "the team's runtime does not report token usage",
        "a runtime without usage is not a team that spent nothing"
    );
    assert_eq!(answer["teams"], serde_json::json!([]));

    memory.lock().unwrap().usage = Some(serde_json::from_str(USAGE).unwrap());
    alice.delivery.0.lock().unwrap().facts = Some(Err("GitHub's rate limit is used up".into()));
    let (_, answer) = read(9).await;
    assert_eq!(
        answer["unavailable"][0]["reason"],
        "GitHub's rate limit is used up"
    );
}

#[tokio::test]
async fn the_signature_names_the_range_and_only_a_member_reads() {
    let alice = alice_with_everything().await;
    let bob = joined_bob(&alice).await;
    let bob_id = id("bob");

    assert_eq!(
        federated(&alice, &bob, &bob_id, &path(1), &path(90))
            .await
            .0,
        404,
        "another range"
    );
    assert_eq!(
        federated(
            &alice,
            &bob,
            &bob_id,
            "/fed/v1/teams/acme/app/evaluation?days=7",
            &path(7)
        )
        .await
        .0,
        404,
        "an evaluation signature does not read effort"
    );
    for days in [0, 91] {
        assert_eq!(
            federated(&alice, &bob, &bob_id, &path(days), &path(days))
                .await
                .0,
            400
        );
    }
    let unsigned = Client::new()
        .get(format!("{}{}", alice.url(), path(7)))
        .send()
        .await
        .unwrap();
    assert_eq!(unsigned.status(), 404);

    let carol = node_with("carol", memory_runtime().0, None).await;
    let copy = alice.node.store().load_org(&id("acme")).unwrap();
    carol.node.store().save_org(&copy).unwrap();
    assert_eq!(
        federated(&alice, &carol, &id("carol"), &path(7), &path(7))
            .await
            .0,
        404,
        "a non-member"
    );
    assert!(
        alice.delivery.0.lock().unwrap().asked.is_empty(),
        "none of them reached GitHub"
    );
}

#[tokio::test]
async fn the_org_route_works_out_this_nodes_teams_and_reads_the_others_from_theirs() {
    let mut alice = alice_with_everything().await;
    let bob = joined_bob(&alice).await;

    let (status, answer) = client_effort(&alice, "").await;
    assert_eq!(status, 200, "{answer}");
    let own: Effort = serde_json::from_value(answer).unwrap();
    assert_eq!(own.check(&id("acme"), 14), Ok(()));
    assert_eq!(own.teams[0].by_type.new_feature.cost_usd, 3.75);

    let (status, answer) = client_effort(&bob, "?days=7").await;
    assert_eq!(status, 200, "alice's team is read from her node: {answer}");
    let remote: Effort = serde_json::from_value(answer).unwrap();
    assert_eq!((remote.days, remote.teams.len()), (7, 1));
    assert_eq!(remote.teams[0].by_type.maintenance.cost_usd, 9.55);

    alice.stop().await;
    let (status, answer) = client_effort(&bob, "").await;
    assert_eq!(status, 200, "a team's node that is down does not blank it");
    assert_eq!(answer["teams"], serde_json::json!([]));
    assert_eq!(
        answer["unavailable"][0]["reason"],
        "the team's node did not answer"
    );
}

/// A node that answers every effort read with the report of the team `other`, whatever was asked.
async fn lying_node() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut report: Effort =
        serde_json::from_str(include_str!("../../../testdata/effort.sample.json")).unwrap();
    report.teams[0].team = id("other");
    let app = axum::Router::new().route(
        "/fed/v1/teams/{org}/{team}/effort",
        axum::routing::get(move || {
            let report = report.clone();
            async move { axum::Json(report) }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
async fn a_team_node_that_answers_for_another_team_is_unavailable_and_the_others_are_answered() {
    let alice = alice_with_everything().await;
    let place = agency_proto::org::TeamPlace::new(id("ext"), "ext", lying_node().await);
    agency_core::orgs::add_team(&alice.node, &id("acme"), place).unwrap();

    let (status, answer) = client_effort(&alice, "?days=14").await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["teams"].as_array().unwrap().len(), 1);
    assert_eq!(answer["teams"][0]["team"], "app");
    assert_eq!(answer["unavailable"][0]["team"], "ext");
    assert_eq!(
        answer["unavailable"][0]["reason"],
        "the team's node sent an answer that is not what was asked for"
    );
    assert!(
        !answer.to_string().contains("127.0.0.1"),
        "no address in an answer"
    );
}

#[tokio::test]
async fn a_range_that_is_not_allowed_and_an_org_that_is_not_the_accounts_are_refused() {
    let alice = alice_with_everything().await;
    for bad in ["?days=0", "?days=91", "?days=two"] {
        assert_eq!(client_effort(&alice, bad).await.0, 400, "{bad}");
    }
    assert!(alice.delivery.0.lock().unwrap().asked.is_empty());
    let carol = node_with("carol", memory_runtime().0, None).await;
    let copy = alice.node.store().load_org(&id("acme")).unwrap();
    carol.node.store().save_org(&copy).unwrap();
    assert_eq!(client_effort(&carol, "").await.0, 404);
}
