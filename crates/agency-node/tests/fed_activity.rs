//! A team's activity on another node: the signed federation read that only a member of the org can make
//! for the question it signed, and the Client API reading a team elsewhere live from its node.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_adapters::peers::read_headers;
use agency_core::invites::{invite, join};
use agency_core::orgs;
use agency_core::read_auth::sign_read;
use agency_proto::activity::Activity;
use agency_proto::org::TeamPlace;
use agency_proto::{Id, Role, Timestamp};
use common::{alice, id, joined_bob, memory_runtime, node_with, Running};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::sync::{Arc, Mutex};

const SAMPLE: &str = include_str!("../../../testdata/activity.sample.json");
const SINCE: &str = "2026-11-24T11:00:00Z";

fn sample() -> Activity {
    serde_json::from_str(SAMPLE).unwrap()
}

fn reports(memory: &Arc<Mutex<MemoryRuntimeState>>) {
    memory.lock().unwrap().activity = Some(sample().events);
}

fn path(since: &str, limit: u32) -> String {
    format!("/fed/v1/teams/acme/app/activity?since={since}&limit={limit}")
}

async fn activity(running: &Running, query: &str) -> (StatusCode, Value) {
    let response = Client::new()
        .get(format!(
            "{}/api/orgs/acme/teams/app/activity{query}",
            running.url()
        ))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// A federation read of alice's team's activity by `reader`, signed for `signed_path`, sent to `sent`.
async fn federated(
    alice: &Running,
    reader: &Running,
    account: &Id,
    signed_path: &str,
    sent: &str,
) -> (StatusCode, String) {
    let auth = sign_read(
        reader.node.key(),
        account,
        "GET",
        signed_path,
        reader.node.now(),
    );
    let mut request = Client::new().get(format!("{}{sent}", alice.url()));
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    let response = request.send().await.unwrap();
    (response.status(), response.text().await.unwrap())
}

#[tokio::test]
async fn a_team_on_another_node_is_read_live_from_its_node() {
    let (mut alice, memory) = alice().await;
    reports(&memory);
    let bob = joined_bob(&alice).await;

    let (status, answer) = activity(&bob, &format!("?since={SINCE}&limit=3")).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let ids: Vec<&str> = answer["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(answer["truncated"], true);
    let since = Timestamp::parse(SINCE).unwrap();
    assert_eq!(
        memory.lock().unwrap().activity_asked,
        vec![(since, 3)],
        "alice's runtime was asked what bob asked"
    );

    let (status, _) = activity(&bob, "").await;
    assert_eq!(status, StatusCode::OK, "the defaults are the reader's");
    assert_eq!(
        memory.lock().unwrap().activity_asked[1].1,
        200,
        "the default limit reaches the team's node"
    );

    memory.lock().unwrap().activity = None;
    assert_eq!(
        activity(&bob, "").await.0,
        StatusCode::NOT_FOUND,
        "a runtime that does not report activity"
    );

    alice.stop().await;
    assert_eq!(
        activity(&bob, "").await.0,
        StatusCode::SERVICE_UNAVAILABLE,
        "the team's node is unreachable"
    );
}

#[tokio::test]
async fn a_federation_read_is_a_members_for_the_question_it_signed() {
    let (alice, memory) = alice().await;
    reports(&memory);
    let bob = joined_bob(&alice).await;
    let bob_id = id("bob");
    let asked = path(SINCE, 200);

    let (status, text) = federated(&alice, &bob, &bob_id, &asked, &asked).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(
        serde_json::from_str::<Activity>(&text)
            .unwrap()
            .events
            .len(),
        6
    );

    let refused = (StatusCode::NOT_FOUND, r#"{"error":"not found"}"#.to_owned());
    for (case, signed, sent) in [
        (
            "another since",
            path("2026-11-24T10:00:00Z", 200),
            asked.clone(),
        ),
        ("another limit", path(SINCE, 1), asked.clone()),
        (
            "a view signature",
            "/fed/v1/teams/acme/app/view".to_owned(),
            asked.clone(),
        ),
        (
            "a usage signature",
            "/fed/v1/teams/acme/app/usage?days=7".to_owned(),
            asked.clone(),
        ),
        (
            "the path without its query",
            "/fed/v1/teams/acme/app/activity".to_owned(),
            asked.clone(),
        ),
        (
            "a limit that is not a number, signed for the other",
            path(SINCE, 200),
            format!("/fed/v1/teams/acme/app/activity?since={SINCE}&limit=many"),
        ),
        (
            "no limit, signed for the other",
            path(SINCE, 200),
            format!("/fed/v1/teams/acme/app/activity?since={SINCE}"),
        ),
    ] {
        assert_eq!(
            federated(&alice, &bob, &bob_id, &signed, &sent).await,
            refused,
            "{case}"
        );
    }
    let unsigned = Client::new()
        .get(format!("{}{asked}", alice.url()))
        .send()
        .await
        .unwrap();
    assert_eq!(unsigned.status(), StatusCode::NOT_FOUND, "no signature");
    assert_eq!(
        memory.lock().unwrap().activity_asked.len(),
        1,
        "no refused read reached the runtime"
    );

    let carol = node_with("carol", memory_runtime().0, None).await;
    let copy = alice.node.store().load_org(&id("acme")).unwrap();
    carol.node.store().save_org(&copy).unwrap();
    assert_eq!(
        federated(&alice, &carol, &id("carol"), &asked, &asked).await,
        refused,
        "a non-member does not learn the team exists"
    );
    assert_eq!(
        activity(&carol, "").await.0,
        StatusCode::NOT_FOUND,
        "nor through its own Client API"
    );
}

#[tokio::test]
async fn a_question_that_is_not_valid_is_refused_only_after_the_member_is_known() {
    let (alice, memory) = alice().await;
    reports(&memory);
    let bob = joined_bob(&alice).await;
    let bob_id = id("bob");

    for sent in [
        path(SINCE, 0),
        path(SINCE, 501),
        "/fed/v1/teams/acme/app/activity?since=yesterday&limit=5".to_string(),
        format!("/fed/v1/teams/acme/app/activity?since={SINCE}&limit=many"),
        "/fed/v1/teams/acme/app/activity?limit=5".to_string(),
    ] {
        let (status, _) = federated(&alice, &bob, &bob_id, &sent, &sent).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{sent}");
        let unsigned = Client::new()
            .get(format!("{}{sent}", alice.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(
            unsigned.status(),
            StatusCode::NOT_FOUND,
            "{sent}: without a member, the same answer as for a team that is not here"
        );
    }
    assert!(
        memory.lock().unwrap().activity_asked.is_empty(),
        "the runtime is not asked"
    );
}

#[tokio::test]
async fn an_answerer_reads_none_of_it_from_either_node() {
    let (alice, memory) = alice().await;
    reports(&memory);
    let carol = node_with("carol", memory_runtime().0, None).await;
    let link = invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    join(&carol.node, &link, |_| true).await.unwrap();

    let asked = path(SINCE, 200);
    let (status, _) = federated(&alice, &carol, &id("carol"), &asked, &asked).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "alice's node");
    assert_eq!(
        activity(&carol, "").await.0,
        StatusCode::FORBIDDEN,
        "her own node"
    );
    assert!(memory.lock().unwrap().activity_asked.is_empty());
}

/// A node that answers every activity read with `answer`, whatever was asked.
async fn lying_node(answer: Activity) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().route(
        "/fed/v1/teams/{org}/{team}/activity",
        axum::routing::get(move || {
            let answer = answer.clone();
            async move { axum::Json(answer) }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
async fn an_answer_that_is_not_valid_is_unavailable() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let mut out_of_order = sample();
    out_of_order
        .events
        .retain(|event| event.at.as_str() > "2026-11-24T11:25:00Z");
    out_of_order.events.reverse();
    let valid_after = "since=2026-11-24T11:25:00Z";
    let cases = [
        (
            "another team",
            Activity {
                team: id("other"),
                ..sample()
            },
            format!("{valid_after}&limit=200"),
        ),
        (
            "events out of order",
            out_of_order,
            format!("{valid_after}&limit=200"),
        ),
        (
            "an event that is not after since",
            sample(),
            format!("{valid_after}&limit=200"),
        ),
        (
            "more events than the limit",
            sample(),
            "since=2026-11-24T11:00:00Z&limit=2".to_owned(),
        ),
    ];
    for (number, (case, answer, query)) in cases.into_iter().enumerate() {
        let team = format!("ext{number}");
        let place = TeamPlace::new(id(&team), "ext", lying_node(answer).await);
        orgs::add_team(&alice.node, &id("acme"), place).unwrap();
        let latest = alice.node.store().load_org(&id("acme")).unwrap();
        bob.node.store().save_org(&latest).unwrap();
        let response = Client::new()
            .get(format!(
                "{}/api/orgs/acme/teams/{team}/activity?{query}",
                bob.url()
            ))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{case}: {text}");
    }
}
