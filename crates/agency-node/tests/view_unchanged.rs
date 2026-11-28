//! A team's node answers a read of its view with a tag, and `304` with no body to a reader that holds that tag; a reader
//! sends the tag only to a node that lists `viewUnchanged`. Alice runs team app; bob is a member who reads it.
mod common;

use agency_core::peer_about::PeerAbout;
use agency_core::read_auth::sign_read;
use agency_core::routes;
use agency_core::views::{refresh, refresh_now};
use agency_proto::about::About;
use agency_proto::clock::Duration;
use common::{alice, get, id, joined_bob, Running};
use reqwest::{Client, StatusCode};
use std::collections::BTreeMap;

fn replica(bob: &Running) -> agency_core::views::Replica {
    bob.node
        .store()
        .load_replica(&id("acme"), &id("app"))
        .expect("bob read the team")
}

fn pass(alice: &Running, bob: &Running) {
    alice.clock.advance(Duration::seconds(10));
    bob.clock.advance(Duration::seconds(10));
}

/// How many reads of the view alice answered, by the class of the status.
async fn answered(alice: &Running, class: &str) -> u64 {
    let response = get(
        alice,
        "/api/metrics?kind=serve&name=GET%20/fed/v1/teams/%7Borg%7D/%7Bteam%7D/view",
    )
    .send()
    .await
    .unwrap();
    let metrics: serde_json::Value = response.json().await.unwrap();
    metrics["series"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|series| series["status"] == class)
        .map(|series| series["count"].as_u64().unwrap_or(0))
        .sum()
}

/// A signed read of alice's team view by bob, with `if_none_match` when given.
async fn read_as_bob(
    alice: &Running,
    bob: &Running,
    if_none_match: Option<&str>,
) -> reqwest::Response {
    alice.clock.advance(Duration::seconds(1));
    let path = routes::view(&id("acme"), &id("app"));
    let auth = sign_read(bob.node.key(), &id("bob"), "GET", &path, alice.node.now());
    let mut request = Client::new().get(format!("{}{path}", alice.url()));
    for (name, value) in agency_adapters::peers::read_headers(&auth) {
        request = request.header(name, value);
    }
    if let Some(tags) = if_none_match {
        request = request.header("if-none-match", tags);
    }
    request.send().await.unwrap()
}

#[tokio::test]
async fn an_unchanged_view_is_answered_304_and_the_replica_keeps_it_and_moves_its_time() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    let first = replica(&bob);
    let tag = first.tag.clone().expect("alice gave the view a tag");
    assert_eq!(tag.len(), 64, "the SHA-256 of the answer, in hex");
    assert_eq!(answered(&alice, "3xx").await, 0);

    pass(&alice, &bob);
    refresh(&bob.node).await.unwrap();

    assert_eq!(
        answered(&alice, "3xx").await,
        1,
        "the second read held a tag and was answered 304"
    );
    let second = replica(&bob);
    assert_eq!(second.view, first.view, "the view is kept");
    assert_eq!(second.tag, first.tag);
    assert_ne!(second.fetched_at, first.fetched_at, "read again just now");
    assert_eq!(
        second.fetched_at.as_deref(),
        Some(bob.node.now_text().as_str())
    );
    assert_eq!((second.error.clone(), second.failures), (None, 0));
}

#[tokio::test]
async fn a_view_that_changed_is_answered_in_full_with_a_new_tag() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    let first = replica(&bob);

    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-9".into(), "A new request".into()));
    pass(&alice, &bob);
    refresh(&bob.node).await.unwrap();

    let second = replica(&bob);
    assert_ne!(second.tag, first.tag, "another view, another tag");
    assert_ne!(second.view, first.view);
    assert_eq!(answered(&alice, "3xx").await, 0, "nothing was unchanged");
}

#[tokio::test]
async fn a_node_that_does_not_list_the_capability_is_not_sent_a_tag() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    // bob has read what alice says of herself, and it is an older release's.
    let mut older = About::of_release("0.3.0");
    older.capabilities.retain(|name| name != "viewUnchanged");
    let read_at = agency_proto::clock::format(bob.node.now());
    let kept = PeerAbout {
        about: Some(older),
        read_at: Some(read_at.clone()),
        tried_at: read_at,
        ..PeerAbout::default()
    };
    bob.node
        .store()
        .save_peer_abouts(&id("acme"), &BTreeMap::from([(alice.url(), kept)]))
        .unwrap();

    pass(&alice, &bob);
    refresh(&bob.node).await.unwrap();

    assert_eq!(answered(&alice, "3xx").await, 0, "read as before, in full");
    assert_eq!(
        replica(&bob).fetched_at.as_deref(),
        Some(bob.node.now_text().as_str())
    );
}

#[tokio::test]
async fn the_tag_is_compared_only_after_the_read_is_authenticated_and_only_when_it_is_the_views() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = routes::view(&id("acme"), &id("app"));

    let full = read_as_bob(&alice, &bob, None).await;
    assert_eq!(full.status(), StatusCode::OK);
    let etag = full.headers()["etag"].to_str().unwrap().to_owned();
    assert!(
        etag.starts_with('"') && etag.ends_with('"') && etag.len() == 66,
        "{etag}"
    );
    assert!(!full.bytes().await.unwrap().is_empty());

    let same = read_as_bob(&alice, &bob, Some(&etag)).await;
    assert_eq!(same.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(same.headers()["etag"], etag.as_str());
    assert!(same.bytes().await.unwrap().is_empty(), "no body");

    let weak = read_as_bob(&alice, &bob, Some(&format!("\"other\", W/{etag}"))).await;
    assert_eq!(
        weak.status(),
        StatusCode::NOT_MODIFIED,
        "one of several tags, weak mark"
    );

    for not_it in [
        "\"0000\"".to_owned(),
        "*".to_owned(),
        String::new(),
        etag.replace('a', "b").replace("\"b", "\"c"),
    ] {
        let other = read_as_bob(&alice, &bob, Some(&not_it)).await;
        let expected = if not_it == etag {
            StatusCode::NOT_MODIFIED
        } else {
            StatusCode::OK
        };
        assert_eq!(other.status(), expected, "{not_it:?}");
    }

    // A read nobody signed, and one signed for another path, learn nothing from the tag: 404 as without it.
    let bare = Client::new()
        .get(format!("{}{path}", alice.url()))
        .header("if-none-match", &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(bare.status(), StatusCode::NOT_FOUND);
    alice.clock.advance(Duration::seconds(1));
    let other_path = routes::about(&id("acme"));
    let auth = sign_read(
        bob.node.key(),
        &id("bob"),
        "GET",
        &other_path,
        alice.node.now(),
    );
    let mut forged = Client::new()
        .get(format!("{}{path}", alice.url()))
        .header("if-none-match", &etag);
    for (name, value) in agency_adapters::peers::read_headers(&auth) {
        forged = forged.header(name, value);
    }
    assert_eq!(forged.send().await.unwrap().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_answer_of_304_ends_a_wait() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    // The team's node did not answer for a while.
    let mut failing = replica(&bob);
    failing.failures = 3;
    failing.failing_since = Some(bob.node.now_text());
    failing.error = Some("timed out".into());
    bob.node
        .store()
        .save_replica(&id("acme"), &id("app"), &failing)
        .unwrap();
    pass(&alice, &bob);

    refresh_now(&bob.node).await.unwrap();

    let after = replica(&bob);
    assert_eq!(
        (after.failures, after.failing_since, after.error),
        (0, None, None)
    );
    assert_eq!(
        answered(&alice, "3xx").await,
        1,
        "and it was a 304 that did it"
    );
}

#[tokio::test]
async fn a_tag_is_sent_only_with_the_view_it_belongs_to() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    // A replica that has a tag and no view (a file that was edited, or half written) cannot be answered 304.
    let mut lost = replica(&bob);
    lost.view = None;
    bob.node
        .store()
        .save_replica(&id("acme"), &id("app"), &lost)
        .unwrap();
    pass(&alice, &bob);

    refresh(&bob.node).await.unwrap();

    assert!(replica(&bob).view.is_some(), "read in full again");
    assert_eq!(answered(&alice, "3xx").await, 0);
}
