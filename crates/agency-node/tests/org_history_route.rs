//! `GET /api/orgs/<org>/history`: the owner reads the entries `agency audit --json` prints, worked out by the
//! same function, and no one else reads them.
mod common;

use agency_core::orgs;
use agency_proto::clock::Duration;
use agency_proto::history::OrgHistory;
use agency_proto::org::TeamPlace;
use common::guests::{call, with_guest, CAROL};
use common::{id, Running};
use reqwest::Method;
use serde_json::Value;
use std::process::Command;

const OWNER: &str = "alice@example.com";
const ROUTE: &str = "/api/orgs/acme/history";

/// What `agency audit --org acme --json` prints at the node's data directory, one entry for each line.
fn printed_by_the_command(alice: &Running, extra: &[&str]) -> Vec<Value> {
    let output = Command::new(env!("CARGO_BIN_EXE_agency"))
        .arg("--dir")
        .arg(alice.data_dir())
        .args(["audit", "--org", "acme", "--json"])
        .args(extra)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

async fn read(alice: &Running, email: &str, query: &str) -> (u16, Value) {
    call(alice, Method::GET, &format!("{ROUTE}{query}"), email, None).await
}

#[tokio::test]
async fn the_owner_reads_what_the_command_prints_for_the_same_org() {
    let (alice, _) = with_guest().await;

    let (status, answer) = read(&alice, OWNER, "").await;

    assert_eq!(status, 200, "{answer}");
    let history: OrgHistory = serde_json::from_value(answer.clone()).unwrap();
    assert!(history.entries.len() >= 2, "{answer}");
    assert_eq!(history.entries[0].version, 1);
    assert!(history.entries.iter().all(|entry| entry.is_kept));
    let printed = printed_by_the_command(&alice, &[]);
    assert_eq!(
        answer["entries"].as_array().unwrap(),
        &printed,
        "the route and the command answer the same entries"
    );
}

#[tokio::test]
async fn since_keeps_the_later_versions_as_the_command_does() {
    let (alice, _) = with_guest().await;
    alice.clock.advance(Duration::minutes(2));
    let place = TeamPlace::new(id("web"), "web", alice.url());
    orgs::add_team(&alice.node, &id("acme"), place).unwrap();
    let (_, all) = read(&alice, OWNER, "").await;
    let all = all["entries"].as_array().unwrap().clone();
    let last = all.last().unwrap();
    let at = last["at"].as_str().unwrap();
    assert_ne!(
        all[0]["at"], last["at"],
        "the versions were signed at different times"
    );

    let (status, answer) = read(&alice, OWNER, &format!("?since={at}")).await;

    assert_eq!(status, 200, "{answer}");
    let entries = answer["entries"].as_array().unwrap();
    assert_eq!(
        entries,
        std::slice::from_ref(last),
        "only the version signed at that time"
    );
    assert_eq!(
        entries,
        &printed_by_the_command(&alice, &["--since", at]),
        "the same cut as the command's"
    );

    let (status, answer) = read(&alice, OWNER, "?since=yesterday").await;
    assert_eq!(status, 400, "{answer}");
}

#[tokio::test]
async fn an_org_this_node_does_not_have_is_not_found() {
    let (alice, _) = with_guest().await;

    let (status, _) = call(
        &alice,
        Method::GET,
        "/api/orgs/nowhere/history",
        OWNER,
        None,
    )
    .await;

    assert_eq!(status, 404);
}

#[tokio::test]
async fn a_guest_and_another_sign_in_are_refused_and_told_nothing_of_the_history() {
    let (alice, _) = with_guest().await;
    for email in [CAROL, "dave@example.com"] {
        let (status, answer) = read(&alice, email, "").await;
        assert_eq!(status, 403, "{email}: {answer}");
        assert!(answer.get("entries").is_none(), "{email}: {answer}");
        assert!(
            !answer.to_string().contains("root key"),
            "no key in a refusal: {answer}"
        );
    }
}

#[tokio::test]
async fn the_node_lists_the_capability_so_a_client_can_tell() {
    let (alice, _) = with_guest().await;

    let (status, about) = call(&alice, Method::GET, "/api/about", OWNER, None).await;

    assert_eq!(status, 200);
    let listed: Vec<&str> = about["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(listed.contains(&"history"), "{listed:?}");
}
