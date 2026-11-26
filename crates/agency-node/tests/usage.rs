//! Token usage per team, end to end: read from the runtime of a team on this node, read live from the
//! team's node for a team elsewhere, and the signed federation read that only a member can make for the
//! range it signed.
mod common;

use agency_adapters::peers::read_headers;
use agency_core::orgs;
use agency_core::read_auth::sign_read;
use agency_proto::org::TeamPlace;
use agency_proto::{Id, TokenUsage};
use common::{alice, id, joined_bob, memory_runtime, node_with, Running};
use reqwest::{Client, StatusCode};
use serde_json::Value;

const SAMPLE: &str = include_str!("../../../testdata/tokenusage.sample.json");

fn sample() -> TokenUsage {
    serde_json::from_str(SAMPLE).unwrap()
}

async fn usage(running: &Running, query: &str) -> (StatusCode, Value) {
    let response = Client::new()
        .get(format!(
            "{}/api/orgs/acme/teams/app/usage{query}",
            running.url()
        ))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// A federation read of alice's team's usage by `reader`, signed for `signed_path`, sent to `path`.
async fn federated(
    alice: &Running,
    reader: &Running,
    account: &Id,
    signed_path: &str,
    path: &str,
) -> StatusCode {
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
    request.send().await.unwrap().status()
}

#[tokio::test]
async fn reports_usage_of_a_local_team_from_its_runtime() {
    let (alice, memory) = alice().await;
    memory.lock().unwrap().usage = Some(sample());

    let (status, report) = usage(&alice, "").await;
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_value::<TokenUsage>(report).unwrap(),
        sample(),
        "passed through as the runtime reported it"
    );
    let (status, report) = usage(&alice, "?days=30").await;
    assert_eq!(
        (status, report["days"].as_u64()),
        (StatusCode::OK, Some(30))
    );

    for bad in ["?days=0", "?days=91", "?days=two"] {
        assert_eq!(usage(&alice, bad).await.0, 400, "{bad}");
    }
    assert_eq!(
        memory.lock().unwrap().usage_days,
        vec![14, 30],
        "14 days by default; malformed ranges never reach the runtime"
    );

    memory.lock().unwrap().usage = Some(TokenUsage {
        team: id("other"),
        ..sample()
    });
    assert_eq!(
        usage(&alice, "").await.0,
        503,
        "a report of another team is not passed on"
    );

    memory.lock().unwrap().usage = None;
    assert_eq!(
        usage(&alice, "").await.0,
        404,
        "a runtime that does not report usage"
    );
}

#[tokio::test]
async fn reports_usage_of_a_remote_team_live_from_its_node() {
    let (mut alice, memory) = alice().await;
    memory.lock().unwrap().usage = Some(sample());
    let bob = joined_bob(&alice).await;

    let (status, report) = usage(&bob, "?days=7").await;
    assert_eq!(status, 200, "{report}");
    assert_eq!(report["days"], 7);
    assert_eq!(report["byPerson"][0]["tokens"]["cacheRead"], 2_400_000);
    assert_eq!(memory.lock().unwrap().usage_days, vec![7]);

    memory.lock().unwrap().usage = None;
    assert_eq!(usage(&bob, "").await.0, 404);

    alice.stop().await;
    assert_eq!(
        usage(&bob, "").await.0,
        503,
        "the team's node is unreachable"
    );
}

#[tokio::test]
async fn federation_usage_reads_are_signed_by_a_member_for_their_range() {
    let (alice, memory) = alice().await;
    memory.lock().unwrap().usage = Some(sample());
    let bob = joined_bob(&alice).await;
    let path = |days: u32| format!("/fed/v1/teams/acme/app/usage?days={days}");

    assert_eq!(
        federated(&alice, &bob, &id("bob"), &path(7), &path(7)).await,
        200
    );
    assert_eq!(
        federated(&alice, &bob, &id("bob"), &path(1), &path(90)).await,
        404,
        "a signature for one range does not read another"
    );
    assert_eq!(
        federated(
            &alice,
            &bob,
            &id("bob"),
            "/fed/v1/teams/acme/app/view",
            &path(7)
        )
        .await,
        404,
        "a view signature does not read usage"
    );
    assert_eq!(
        federated(&alice, &bob, &id("bob"), &path(0), &path(0)).await,
        400
    );
    let unsigned = Client::new()
        .get(format!("{}{}", alice.url(), path(7)))
        .send()
        .await
        .unwrap();
    assert_eq!(unsigned.status(), 404, "refused like an unknown team");

    let carol = node_with("carol", memory_runtime().0, None).await;
    let copy = alice.node.store().load_org(&id("acme")).unwrap();
    carol.node.store().save_org(&copy).unwrap();
    assert_eq!(
        federated(&alice, &carol, &id("carol"), &path(7), &path(7)).await,
        404,
        "a non-member does not learn the team exists"
    );
    assert_eq!(
        usage(&carol, "").await.0,
        404,
        "nor through its own Client API"
    );
    assert_eq!(memory.lock().unwrap().usage_days, vec![7]);
}

/// A node that answers every usage read with the sample report of acme/app over 14 days.
async fn lying_node() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().route(
        "/fed/v1/teams/{org}/{team}/usage",
        axum::routing::get(|| async { axum::Json(sample()) }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
async fn refuses_a_report_from_a_team_node_of_another_team_or_range() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let place = TeamPlace {
        id: id("ext"),
        label: "ext".into(),
        department: None,
        node: lying_node().await,
        plan: None,
    };
    orgs::add_team(&alice.node, &id("acme"), place).unwrap();
    let latest = alice.node.store().load_org(&id("acme")).unwrap();
    bob.node.store().save_org(&latest).unwrap();

    let response = Client::new()
        .get(format!("{}/api/orgs/acme/teams/ext/usage", bob.url()))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    assert_eq!(status, 503, "{text}");
}
