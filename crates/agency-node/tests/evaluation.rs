//! A team's delivery evaluation over the federation route, end to end: worked out on the node the team
//! runs on from the facts its delivery source reads, and read there by a member over a signed path that
//! names the range.
mod common;

use agency_adapters::peers::read_headers;
use agency_core::delivery::{DeliveryFacts, MainRun, MergedPullRequest, Verdict};
use agency_core::read_auth::sign_read;
use agency_proto::clock;
use agency_proto::evaluation::Evaluation;
use agency_proto::Id;
use common::{alice, id, joined_bob, memory_runtime, node_with, Running};
use reqwest::{Client, StatusCode};
use serde_json::Value;

fn at(raw: &str) -> agency_proto::clock::OffsetDateTime {
    clock::parse(raw).unwrap()
}

/// Facts at the node's time (2026-11-24T12:00Z): a feature merged that morning, and CI red since 11:00.
fn facts() -> DeliveryFacts {
    DeliveryFacts {
        pull_requests: vec![MergedPullRequest {
            title: "feat(web): add a screen".into(),
            body: String::new(),
            opened_at: at("2026-11-24T08:00:00Z"),
            merged_at: at("2026-11-24T09:00:00Z"),
            first_commit_at: Some(at("2026-11-24T07:00:00Z")),
            has_issue: true,
            issue_started_at: Some(at("2026-11-24T06:00:00Z")),
        }],
        runs: vec![MainRun {
            repo: "acme/app".into(),
            commit: "abc".into(),
            finished_at: at("2026-11-24T11:00:00Z"),
            verdict: Verdict::Failed,
        }],
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

/// A federation read of alice's team's evaluation by `reader`, signed for `signed_path`, sent to `path`.
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

fn path(days: u32) -> String {
    format!("/fed/v1/teams/acme/app/evaluation?days={days}")
}

#[tokio::test]
async fn a_member_reads_the_evaluation_of_a_team_that_has_repositories() {
    let (alice, _) = alice().await;
    configure(&alice);
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    let bob = joined_bob(&alice).await;

    let (status, answer) = federated(&alice, &bob, &id("bob"), &path(14), &path(14)).await;
    assert_eq!(status, 200, "{answer}");
    let evaluation: Evaluation = serde_json::from_value(answer).unwrap();
    assert_eq!(evaluation.check(&id("acme"), 14), Ok(()));
    assert!(evaluation.unavailable.is_empty());
    let team = &evaluation.teams[0];
    assert_eq!(team.team, id("app"));
    assert_eq!(
        (team.from.as_str(), team.until.as_str()),
        ("2026-11-11", "2026-11-24")
    );
    assert_eq!(team.summary.merged.feat, 1);
    assert_eq!(team.summary.pr_lead_median_seconds, Some(3600));
    assert_eq!(team.summary.issue_lead_median_seconds, Some(10_800));
    assert_eq!(
        team.summary.main_red_seconds, 3600,
        "red since 11:00, and now is 12:00"
    );

    let asked = alice.delivery.0.lock().unwrap().asked.clone();
    assert_eq!(
        asked,
        [(vec!["acme/app".to_string()], at("2026-11-11T00:00:00Z"))],
        "the period's start"
    );
}

#[tokio::test]
async fn a_team_without_repositories_or_whose_facts_cannot_be_read_is_unavailable_with_a_reason() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;

    let (status, answer) = federated(&alice, &bob, &id("bob"), &path(7), &path(7)).await;
    assert_eq!(status, 200, "not an error: the reason is the answer");
    let evaluation: Evaluation = serde_json::from_value(answer).unwrap();
    assert!(evaluation.teams.is_empty());
    assert_eq!(evaluation.unavailable[0].team, id("app"));
    assert!(evaluation.unavailable[0].reason.contains("no repositories"));
    assert_eq!(evaluation.check(&id("acme"), 7), Ok(()));
    assert!(
        alice.delivery.0.lock().unwrap().asked.is_empty(),
        "GitHub is not asked"
    );

    bob.clock.advance(clock::Duration::seconds(1));
    configure(&alice);
    alice.delivery.0.lock().unwrap().facts =
        Some(Err("GitHub's rate limit is used up for now".into()));
    let (status, answer) = federated(&alice, &bob, &id("bob"), &path(7), &path(7)).await;
    assert_eq!(status, 200);
    assert_eq!(
        answer["unavailable"][0]["reason"],
        "GitHub's rate limit is used up for now"
    );
    assert_eq!(answer["teams"], serde_json::json!([]));
}

#[tokio::test]
async fn the_signature_names_the_range_and_only_a_member_reads() {
    let (alice, _) = alice().await;
    configure(&alice);
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
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
            "/fed/v1/teams/acme/app/usage?days=7",
            &path(7)
        )
        .await
        .0,
        404,
        "a usage signature does not read an evaluation"
    );
    assert_eq!(
        federated(&alice, &bob, &bob_id, &path(0), &path(0)).await.0,
        400
    );
    assert_eq!(
        federated(&alice, &bob, &bob_id, &path(91), &path(91))
            .await
            .0,
        400
    );
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

// ── The Client API: every team of an org ──

async fn client_evaluation(running: &Running, query: &str) -> (StatusCode, Value) {
    let response = Client::new()
        .get(format!("{}/api/orgs/acme/evaluation{query}", running.url()))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// A node that answers every evaluation read with the report of the team `other`, whatever was asked.
async fn lying_node() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut report: Evaluation =
        serde_json::from_str(include_str!("../../../testdata/evaluation.sample.json")).unwrap();
    report.teams.truncate(1);
    report.teams[0].team = id("other");
    report.unavailable.clear();
    let app = axum::Router::new().route(
        "/fed/v1/teams/{org}/{team}/evaluation",
        axum::routing::get(move || {
            let report = report.clone();
            async move { axum::Json(report) }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
async fn the_org_route_works_out_the_teams_of_this_node_and_reads_the_others_from_theirs() {
    let (mut alice, _) = alice().await;
    configure(&alice);
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    let bob = joined_bob(&alice).await;

    let (status, answer) = client_evaluation(&alice, "").await;
    assert_eq!(status, 200, "{answer}");
    let evaluation: Evaluation = serde_json::from_value(answer).unwrap();
    assert_eq!(evaluation.check(&id("acme"), 14), Ok(()));
    assert_eq!(evaluation.teams.len(), 1);
    assert_eq!(evaluation.teams[0].summary.merged.feat, 1);
    assert!(evaluation.unavailable.is_empty());

    let (status, answer) = client_evaluation(&bob, "?days=7").await;
    assert_eq!(
        status, 200,
        "bob has no team of his own; alice's is read from her node: {answer}"
    );
    let remote: Evaluation = serde_json::from_value(answer).unwrap();
    assert_eq!((remote.days, remote.teams.len()), (7, 1));
    assert_eq!(remote.teams[0].team, id("app"));
    assert_eq!(
        remote.teams[0].summary.merged.feat, 1,
        "the numbers are the team's own node's"
    );
    assert_eq!(
        alice.delivery.0.lock().unwrap().asked.len(),
        2,
        "each read worked it out on alice's node"
    );

    alice.stop().await;
    let (status, answer) = client_evaluation(&bob, "").await;
    assert_eq!(
        status, 200,
        "a team's node that is down does not blank the answer"
    );
    assert_eq!(answer["teams"], serde_json::json!([]));
    assert_eq!(answer["unavailable"][0]["team"], "app");
    assert_eq!(
        answer["unavailable"][0]["reason"],
        "the team's node did not answer"
    );
}

#[tokio::test]
async fn a_range_that_is_not_allowed_and_an_org_that_is_not_the_accounts_are_refused() {
    let (alice, _) = alice().await;
    for bad in ["?days=0", "?days=91", "?days=two"] {
        assert_eq!(client_evaluation(&alice, bad).await.0, 400, "{bad}");
    }
    assert!(
        alice.delivery.0.lock().unwrap().asked.is_empty(),
        "a bad range never reaches GitHub"
    );
    let carol = node_with("carol", memory_runtime().0, None).await;
    let copy = alice.node.store().load_org(&id("acme")).unwrap();
    carol.node.store().save_org(&copy).unwrap();
    assert_eq!(
        client_evaluation(&carol, "").await.0,
        404,
        "an account that is not a member does not learn the org"
    );
}

#[tokio::test]
async fn a_team_node_that_answers_for_another_team_is_unavailable_and_the_others_are_still_answered(
) {
    let (alice, _) = alice().await;
    configure(&alice);
    alice.delivery.0.lock().unwrap().facts = Some(Ok(facts()));
    let place = agency_proto::org::TeamPlace::new(id("ext"), "ext", lying_node().await);
    agency_core::orgs::add_team(&alice.node, &id("acme"), place).unwrap();

    let (status, answer) = client_evaluation(&alice, "?days=14").await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer["teams"].as_array().unwrap().len(),
        1,
        "alice's own team is answered"
    );
    assert_eq!(answer["teams"][0]["team"], "app");
    assert_eq!(answer["unavailable"][0]["team"], "ext");
    assert_eq!(
        answer["unavailable"][0]["reason"],
        "the team's node sent an answer that is not what was asked for",
        "another team's numbers are not passed on as ext's"
    );
    let text = answer.to_string();
    assert!(
        !text.contains("127.0.0.1"),
        "no address in an answer: {text}"
    );
}

#[tokio::test]
async fn a_team_of_this_node_without_repositories_is_listed_as_unavailable_with_its_reason() {
    let (alice, _) = alice().await;
    let (status, answer) = client_evaluation(&alice, "").await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["teams"], serde_json::json!([]));
    assert_eq!(answer["unavailable"][0]["team"], "app");
    assert!(answer["unavailable"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("no repositories"));
}
