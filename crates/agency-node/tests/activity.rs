//! A team's activity feed through the Client API: read from the runtime of a team on this node, cut at
//! the limit, and refused when the question or the runtime's answer is not valid.
mod common;

use agency_adapters::memory::MemoryRuntimeState;
use agency_core::invites::{invite, join};
use agency_proto::activity::Activity;
use agency_proto::{Role, Timestamp};
use common::{alice, body, get, id, joined_bob, memory_runtime, node, Running};
use reqwest::StatusCode;
use serde_json::Value;
use std::sync::{Arc, Mutex};

const SAMPLE: &str = include_str!("../../../testdata/activity.sample.json");

fn events() -> Vec<agency_proto::activity::ActivityEvent> {
    serde_json::from_str::<Activity>(SAMPLE).unwrap().events
}

async fn activity(running: &Running, query: &str) -> (StatusCode, Value) {
    let path = format!("/api/orgs/acme/teams/app/activity{query}");
    body(get(running, &path).send().await.unwrap()).await
}

fn reports(memory: &Arc<Mutex<MemoryRuntimeState>>) {
    memory.lock().unwrap().activity = Some(events());
}

#[tokio::test]
async fn reports_the_events_after_since_oldest_first() {
    let (alice, memory) = alice().await;
    reports(&memory);
    let (status, answer) = activity(&alice, "?since=2026-11-24T11:00:00Z").await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let ids: Vec<&str> = answer["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 6);
    assert_eq!(ids[0], "b-0000000000a1");
    assert_eq!(
        (answer["org"].as_str(), answer["truncated"].as_bool()),
        (Some("acme"), Some(false))
    );

    let (_, later) = activity(&alice, "?since=2026-11-24T11:30:00Z").await;
    assert_eq!(
        later["events"].as_array().unwrap().len(),
        2,
        "strictly after since"
    );
}

#[tokio::test]
async fn cuts_the_list_at_the_limit_and_says_so() {
    let (alice, memory) = alice().await;
    reports(&memory);
    let (_, answer) = activity(&alice, "?since=2026-11-24T11:00:00Z&limit=2").await;
    assert_eq!(answer["events"].as_array().unwrap().len(), 2);
    assert_eq!(answer["truncated"], true);
    let asked = memory.lock().unwrap().activity_asked.clone();
    assert_eq!(asked[0].1, 2, "the limit goes to the runtime");
}

#[tokio::test]
async fn asks_for_the_last_hour_and_200_events_without_a_question() {
    let (alice, memory) = alice().await;
    reports(&memory);
    let (status, _) = activity(&alice, "").await;
    assert_eq!(status, StatusCode::OK);
    let asked = memory.lock().unwrap().activity_asked.clone();
    let since = Timestamp::parse("2026-11-24T11:00:00Z").unwrap();
    assert_eq!(asked, vec![(since, 200)], "the node's now is 12:00");
}

#[tokio::test]
async fn refuses_a_question_that_is_not_valid() {
    let (alice, memory) = alice().await;
    reports(&memory);
    for query in [
        "?limit=0",
        "?limit=501",
        "?limit=many",
        "?since=yesterday",
        "?since=2026-11-24T11:00:00%2B09:00",
    ] {
        let (status, _) = activity(&alice, query).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
    }
    assert!(
        memory.lock().unwrap().activity_asked.is_empty(),
        "the runtime is not asked"
    );
}

#[tokio::test]
async fn a_runtime_without_activity_or_an_unknown_team_is_not_found() {
    let (alice, _memory) = alice().await;
    let (status, _) = activity(&alice, "").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the runtime does not report activity"
    );
    let other = body(
        get(&alice, "/api/orgs/acme/teams/none/activity")
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(other.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_answer_that_is_not_valid_is_unavailable() {
    let (alice, memory) = alice().await;
    let mut out_of_order = events();
    out_of_order.reverse();
    memory.lock().unwrap().activity = Some(out_of_order);
    let (status, _) = activity(&alice, "?since=2026-11-24T11:00:00Z").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn a_team_on_another_node_is_not_answered_yet_and_an_answerer_reads_none() {
    let (alice, memory) = alice().await;
    reports(&memory);
    let bob = joined_bob(&alice).await;
    let (status, _) = activity(&bob, "").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the team runs on alice's node"
    );

    let carol = node("carol", memory_runtime().0).await;
    let link = invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    join(&carol.node, &link, |_| true).await.unwrap();
    let (status, _) = activity(&carol, "").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an answerer does not read the feed"
    );
}
