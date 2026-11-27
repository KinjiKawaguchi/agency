//! A feature of a team on another node (its activity, its effort) is read as "not supported, and the node needs
//! an update" when what this node kept of that node's `about` says the node lacks the capability, and the route
//! is not called then. A 404 from a node that lists the capability, or from one that has not been read, stays
//! "not found" (docs/compatibility.md, "Node and node").
mod common;

use agency_core::orgs;
use agency_core::peer_about::PeerAbout;
use agency_proto::about::{About, Capability};
use agency_proto::clock::{self, Duration};
use agency_proto::org::TeamPlace;
use common::{alice, id, joined_bob, Running};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A node that answers every request with 404, and counts the requests.
async fn node_without_the_routes() -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = hits.clone();
    let app = axum::Router::new().fallback(move || {
        let counted = counted.clone();
        async move {
            counted.fetch_add(1, Ordering::SeqCst);
            StatusCode::NOT_FOUND
        }
    });
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, hits)
}

/// What a node that runs `release` and lacks `lacking` answers to a read of its `about`.
fn about_of(release: &str, lacking: Option<Capability>) -> About {
    let mut about = About::of_release(release);
    if let Some(lacking) = lacking {
        about.capabilities.retain(|name| name != lacking.name());
    }
    about
}

/// alice's node with a team `ext` on a node that answers 404 to everything, and `kept` as what alice has kept
/// of that node (`None`: nothing).
async fn alice_with_team_on(
    kept: impl FnOnce(&Running) -> Option<PeerAbout>,
) -> (Running, Arc<AtomicUsize>) {
    let (alice, _) = alice().await;
    let (url, hits) = node_without_the_routes().await;
    orgs::add_team(
        &alice.node,
        &id("acme"),
        TeamPlace::new(id("ext"), "ext", url.clone()),
    )
    .unwrap();
    if let Some(entry) = kept(&alice) {
        let abouts = BTreeMap::from([(url, entry)]);
        alice
            .node
            .store()
            .save_peer_abouts(&id("acme"), &abouts)
            .unwrap();
    }
    (alice, hits)
}

/// An answer of `about`, read `age` ago.
fn read_ago(alice: &Running, about: About, age: Duration) -> PeerAbout {
    let at = clock::format(alice.node.now() - age);
    PeerAbout {
        about: Some(about),
        read_at: Some(at.clone()),
        tried_at: at,
        ..PeerAbout::default()
    }
}

/// The reading of a team's activity: its status and the answer.
async fn activity(alice: &Running) -> (StatusCode, Value) {
    let path = "/api/orgs/acme/teams/ext/activity";
    let response = Client::new()
        .get(format!("{}{path}", alice.url()))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// The reason the org's effort gives for team `ext`, which it cannot read.
async fn effort_reason(alice: &Running) -> String {
    let response = Client::new()
        .get(format!("{}/api/orgs/acme/effort", alice.url()))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let answer: Value = response.json().await.unwrap();
    let ext = answer["unavailable"]
        .as_array()
        .unwrap()
        .iter()
        .find(|team| team["team"] == "ext")
        .unwrap_or_else(|| panic!("ext is unavailable: {answer}"));
    assert!(
        !answer.to_string().contains("127.0.0.1"),
        "no address in an answer"
    );
    ext["reason"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn activity_of_a_node_that_lacks_it_is_not_read_and_the_reason_names_its_release() {
    let (alice, hits) = alice_with_team_on(|alice| {
        let about = about_of("0.2.0", Some(Capability::Activity));
        Some(read_ago(alice, about, Duration::hours(1)))
    })
    .await;

    let (status, answer) = activity(&alice).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{answer}");
    let reason = answer["error"].as_str().unwrap();
    assert!(reason.contains("release 0.2.0"), "{reason}");
    assert!(reason.contains("activity"), "{reason}");
    assert!(reason.contains("needs an update"), "{reason}");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "no request reached the node"
    );
}

#[tokio::test]
async fn effort_of_a_node_that_lacks_it_is_not_read_and_the_reason_names_its_release() {
    let (alice, hits) = alice_with_team_on(|alice| {
        let about = about_of("0.2.0", Some(Capability::Effort));
        Some(read_ago(alice, about, Duration::hours(1)))
    })
    .await;

    let reason = effort_reason(&alice).await;

    assert!(reason.contains("release 0.2.0"), "{reason}");
    assert!(reason.contains("effort"), "{reason}");
    assert!(reason.contains("needs an update"), "{reason}");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "no request reached the node"
    );
}

#[tokio::test]
async fn a_node_that_lists_the_capability_and_answers_404_is_still_not_found() {
    let (alice, hits) = alice_with_team_on(|alice| {
        Some(read_ago(alice, about_of("0.3.0", None), Duration::hours(1)))
    })
    .await;

    assert_eq!(activity(&alice).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        effort_reason(&alice).await,
        "the team's node does not give effort numbers"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2, "both routes were called");
}

#[tokio::test]
async fn a_404_from_either_read_marks_the_node_to_be_read_again_soon() {
    let again = |alice: &Running| {
        alice
            .node
            .store()
            .peer_abouts(&id("acme"))
            .values()
            .all(|kept| kept.should_read_again)
    };
    for read_effort in [false, true] {
        let (alice, _) = alice_with_team_on(|alice| {
            Some(read_ago(alice, about_of("0.3.0", None), Duration::hours(1)))
        })
        .await;
        assert!(!again(&alice));

        if read_effort {
            effort_reason(&alice).await;
        } else {
            activity(&alice).await;
        }

        assert!(
            again(&alice),
            "the node may not be the release it was read as (effort: {read_effort})"
        );
    }
}

#[tokio::test]
async fn a_node_that_was_never_read_is_asked() {
    let (alice, hits) = alice_with_team_on(|_| None).await;

    assert_eq!(activity(&alice).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        effort_reason(&alice).await,
        "the team's node does not give effort numbers"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2, "both routes were called");
}

#[tokio::test]
async fn an_answer_older_than_three_days_or_no_answer_is_not_taken_for_lacking() {
    for kept in [
        |alice: &Running| {
            let about = about_of("0.2.0", Some(Capability::Activity));
            let about = About {
                capabilities: vec![],
                ..about
            };
            read_ago(alice, about, Duration::days(4))
        },
        |_: &Running| PeerAbout {
            error: Some("no answer".into()),
            tried_at: "2026-11-24T12:00:00Z".into(),
            ..PeerAbout::default()
        },
    ] {
        let (alice, hits) = alice_with_team_on(|alice| Some(kept(alice))).await;

        assert_eq!(activity(&alice).await.0, StatusCode::NOT_FOUND);
        assert_eq!(
            effort_reason(&alice).await,
            "the team's node does not give effort numbers"
        );
        assert_eq!(hits.load(Ordering::SeqCst), 2, "both routes were called");
    }
}

#[tokio::test]
async fn a_node_lacking_one_feature_is_still_asked_for_the_other() {
    let (alice, hits) = alice_with_team_on(|alice| {
        let about = about_of("0.2.0", Some(Capability::Activity));
        Some(read_ago(alice, about, Duration::hours(1)))
    })
    .await;

    assert_eq!(activity(&alice).await.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert_eq!(
        effort_reason(&alice).await,
        "the team's node does not give effort numbers"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1, "effort was asked for");
}

/// bob's kept answer of alice's node: her release, which lacks `activity`, read just now.
fn keep_alice_as_behind(bob: &Running, alice: &Running) {
    let about = about_of("0.2.0", Some(Capability::Activity));
    let abouts = BTreeMap::from([(alice.url(), read_ago(bob, about, Duration::ZERO))]);
    bob.node
        .store()
        .save_peer_abouts(&id("acme"), &abouts)
        .unwrap();
}

#[tokio::test]
async fn a_node_that_was_updated_since_is_read_again_and_then_asked() {
    let (alice, memory) = alice().await;
    memory.lock().unwrap().activity = Some(
        serde_json::from_str::<agency_proto::activity::Activity>(include_str!(
            "../../../testdata/activity.sample.json"
        ))
        .unwrap()
        .events,
    );
    let bob = joined_bob(&alice).await;
    let read_activity = |bob: &Running| {
        let url = format!("{}/api/orgs/acme/teams/app/activity", bob.url());
        async move { Client::new().get(url).send().await.unwrap().status() }
    };
    let pass = |by: Duration| {
        alice.clock.advance(by);
        bob.clock.advance(by);
    };
    keep_alice_as_behind(&bob, &alice);

    assert_eq!(read_activity(&bob).await, StatusCode::SERVICE_UNAVAILABLE);
    let kept = bob.node.store().peer_abouts(&id("acme"));
    assert!(
        kept[&alice.url()].should_read_again,
        "asking marks it for a new read"
    );

    pass(Duration::minutes(5));
    agency_core::views::refresh(&bob.node).await.unwrap();
    assert_eq!(
        read_activity(&bob).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "not read again before ten minutes"
    );

    pass(Duration::minutes(6));
    agency_core::views::refresh(&bob.node).await.unwrap();
    assert_eq!(
        read_activity(&bob).await,
        StatusCode::OK,
        "alice's node lists activity now"
    );
}

#[tokio::test]
async fn a_release_that_hides_text_is_printed_with_a_question_mark() {
    let (alice, hits) = alice_with_team_on(|alice| {
        let about = about_of("0.2.0\u{202e}gnp.exe", Some(Capability::Effort));
        Some(read_ago(alice, about, Duration::hours(1)))
    })
    .await;

    let reason = effort_reason(&alice).await;

    assert!(reason.contains("release 0.2.0?gnp.exe,"), "{reason}");
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}
