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

/// alice's org with the versions of `with_guest` and three more, each signed two minutes after the one before.
async fn alice_with_many_versions() -> Running {
    let (alice, _) = with_guest().await;
    for team in ["web", "api", "ops"] {
        alice.clock.advance(Duration::minutes(2));
        let place = TeamPlace::new(id(team), team, alice.url());
        orgs::add_team(&alice.node, &id("acme"), place).unwrap();
    }
    alice
}

async fn entries_of(alice: &Running, query: &str) -> (Vec<Value>, bool) {
    let (status, answer) = read(alice, OWNER, query).await;
    assert_eq!(status, 200, "{query}: {answer}");
    let entries = answer["entries"].as_array().unwrap().clone();
    (entries, answer.get("more").is_some_and(|more| more == true))
}

#[tokio::test]
async fn a_limit_cuts_the_answer_says_there_is_more_and_after_gives_the_rest() {
    let alice = alice_with_many_versions().await;
    let (all, more) = entries_of(&alice, "").await;
    assert!(all.len() >= 5 && !more, "the whole history, with no flag");
    assert!(
        read(&alice, OWNER, "").await.1.get("more").is_none(),
        "more is left out when there is none"
    );

    let mut joined: Vec<Value> = vec![];
    let mut after = 0;
    loop {
        let (page, more) = entries_of(&alice, &format!("?limit=2&after={after}")).await;
        assert!(page.len() <= 2, "{page:?}");
        assert!(page
            .iter()
            .all(|entry| entry["version"].as_u64().unwrap() > after));
        joined.extend(page.clone());
        if !more {
            break;
        }
        assert_eq!(page.len(), 2, "a cut page is a full one");
        after = page.last().unwrap()["version"].as_u64().unwrap();
    }

    assert_eq!(joined, all, "the pages, joined, are the whole history");
    let (exact, more) = entries_of(&alice, &format!("?limit={}", all.len())).await;
    assert_eq!(
        (exact.len(), more),
        (all.len(), false),
        "nothing left out, no flag"
    );
}

#[tokio::test]
async fn since_and_after_together_equal_the_whole_history_cut_afterwards() {
    let alice = alice_with_many_versions().await;
    let (all, _) = entries_of(&alice, "").await;
    let at = all[all.len() - 3]["at"].as_str().unwrap().to_owned();
    let after = all[all.len() - 2]["version"].as_u64().unwrap();

    let (cut, more) = entries_of(&alice, &format!("?since={at}&after={after}")).await;

    let wanted: Vec<Value> = all
        .iter()
        .filter(|entry| {
            entry["version"].as_u64().unwrap() > after
                && entry["at"].as_str().unwrap() >= at.as_str()
        })
        .cloned()
        .collect();
    assert!(!wanted.is_empty() && wanted.len() < all.len());
    assert_eq!((cut, more), (wanted, false));
}

#[tokio::test]
async fn a_limit_or_an_after_that_is_not_allowed_is_400() {
    let alice = alice_with_many_versions().await;
    for query in [
        "?limit=0",
        "?limit=1001",
        "?limit=many",
        "?limit=-1",
        "?after=first",
        "?after=-1",
    ] {
        let (status, answer) = read(&alice, OWNER, query).await;
        assert_eq!(status, 400, "{query}: {answer}");
    }
    let (status, _) = read(&alice, OWNER, "?limit=1000").await;
    assert_eq!(status, 200, "the most is allowed");
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
