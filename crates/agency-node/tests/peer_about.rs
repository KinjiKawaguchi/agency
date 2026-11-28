//! What a node has read of the other nodes of an org (their `about`): kept for each peer, read again daily,
//! early after a 404, and never more than the node can hold. Two real nodes over HTTP.
mod common;

use agency_core::peer_about::{note_not_found, PeerAbout};
use agency_core::views::refresh;
use agency_proto::about::About;
use agency_proto::clock::Duration;
use common::{alice, id, joined_bob, Running};
use std::collections::BTreeMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn kept(bob: &Running, alice: &Running) -> PeerAbout {
    let all = bob.node.store().peer_abouts(&id("acme"));
    all.get(&alice.url())
        .cloned()
        .expect("an entry for alice's node")
}

/// Both nodes' clocks move on together: a node refuses a signed read whose time is far from its own.
fn pass(alice: &Running, bob: &Running, by: Duration) {
    alice.clock.advance(by);
    bob.clock.advance(by);
}

#[tokio::test]
async fn a_node_reads_what_each_peer_speaks_and_keeps_it() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    let entry = kept(&bob, &alice);
    assert_eq!(entry.about, Some(agency_core::about::this_node()));
    assert_eq!(entry.read_at, Some(bob.node.now_text()));
    assert_eq!((entry.error, entry.should_read_again), (None, false));
    let all = bob.node.store().peer_abouts(&id("acme"));
    assert_eq!(all.len(), 1, "only the org's other nodes, not this one");
    refresh(&alice.node).await.unwrap();
    let own = alice.node.store().peer_abouts(&id("acme"));
    assert!(
        own.is_empty(),
        "alice runs the only team: no other node to read"
    );
    assert!(bob.data_dir().join("about").join("acme.json").is_file());
}

#[tokio::test]
async fn a_good_read_is_left_for_a_day() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    let first = kept(&bob, &alice);

    pass(&alice, &bob, Duration::hours(23));
    refresh(&bob.node).await.unwrap();
    assert_eq!(kept(&bob, &alice), first, "not read again before a day");

    pass(&alice, &bob, Duration::hours(2));
    refresh(&bob.node).await.unwrap();
    let again = kept(&bob, &alice);
    assert_eq!(again.read_at, Some(bob.node.now_text()));
    assert_ne!(again.read_at, first.read_at);
}

#[tokio::test]
async fn a_failed_read_keeps_the_last_answer_and_is_tried_again_after_ten_minutes() {
    let (mut alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    let good = kept(&bob, &alice);

    alice.stop().await;
    pass(&alice, &bob, Duration::hours(25));
    refresh(&bob.node).await.unwrap();
    let failed = kept(&bob, &alice);
    assert_eq!(failed.about, good.about, "the last good answer is kept");
    assert_eq!(failed.read_at, good.read_at, "and so is when it was read");
    assert!(failed.error.is_some());
    assert_eq!(failed.tried_at, bob.node.now_text());

    pass(&alice, &bob, Duration::minutes(5));
    refresh(&bob.node).await.unwrap();
    assert_eq!(
        kept(&bob, &alice),
        failed,
        "not tried again inside ten minutes"
    );

    alice.start().await;
    pass(&alice, &bob, Duration::minutes(6));
    refresh(&bob.node).await.unwrap();
    let back = kept(&bob, &alice);
    assert_eq!(back.error, None, "a good read clears the failure");
    assert_eq!(back.read_at, Some(bob.node.now_text()));
}

#[tokio::test]
async fn a_404_from_a_peer_makes_the_next_read_early() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    let first = kept(&bob, &alice);

    pass(&alice, &bob, Duration::minutes(11));
    refresh(&bob.node).await.unwrap();
    assert_eq!(kept(&bob, &alice), first, "no reason to read it again yet");

    // Alice stops running the team: it answers 404 to bob's read of the team's view.
    alice
        .node
        .update_config(|config| config.teams.retain(|team| team.team != id("app")))
        .unwrap();
    pass(&alice, &bob, Duration::minutes(1));
    // The pass that gets the 404 reads the peer's `about` at the same time as the views, so the early read is the
    // next pass's.
    refresh(&bob.node).await.unwrap();
    refresh(&bob.node).await.unwrap();
    let after = kept(&bob, &alice);
    assert_eq!(
        after.read_at,
        Some(bob.node.now_text()),
        "read on the next pass, a day early"
    );
    assert!(!after.should_read_again, "and the request is answered");
}

#[tokio::test]
async fn a_404_is_noted_only_for_a_peer_that_was_read() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    note_not_found(&bob.node, &id("acme"), &alice.url()).unwrap();
    assert!(
        bob.node.store().peer_abouts(&id("acme")).is_empty(),
        "nothing to mark yet"
    );
    refresh(&bob.node).await.unwrap();
    note_not_found(&bob.node, &id("acme"), &alice.url()).unwrap();
    assert!(kept(&bob, &alice).should_read_again);
}

#[tokio::test]
async fn a_team_that_runs_here_is_not_another_node_even_if_the_definition_names_one() {
    let (alice, _) = alice().await;
    // The definition places the team at another origin, but alice's own configuration still runs it.
    agency_core::orgs::move_team(
        &alice.node,
        &id("acme"),
        &id("app"),
        "http://127.0.0.1:1".into(),
    )
    .unwrap();
    refresh(&alice.node).await.unwrap();
    assert!(
        alice.node.store().peer_abouts(&id("acme")).is_empty(),
        "a team that runs here is read from the runtime, not from a peer"
    );
}

#[tokio::test]
async fn a_node_that_is_no_longer_one_of_the_orgs_is_forgotten() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let gone = "http://127.0.0.1:1".to_owned();
    let stale = BTreeMap::from([(gone.clone(), PeerAbout::default())]);
    bob.node
        .store()
        .save_peer_abouts(&id("acme"), &stale)
        .unwrap();
    refresh(&bob.node).await.unwrap();
    let all = bob.node.store().peer_abouts(&id("acme"));
    assert!(!all.contains_key(&gone));
    assert!(all.contains_key(&alice.url()));
}

/// A peer that answers every request with `body` as JSON.
async fn serve_always(port: u16, body: String) {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let body = body.clone();
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                let _ = socket.read(&mut buffer).await;
                let head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(body.as_bytes()).await;
            });
        }
    });
}

#[tokio::test]
async fn an_answer_beyond_the_limits_is_not_kept() {
    let (mut alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    let good = kept(&bob, &alice);

    let mut lie = About::of_release("0.3.0");
    lie.capabilities = (0..1000).map(|n| format!("name{n}")).collect();
    alice.stop().await;
    serve_always(alice.port, serde_json::to_string(&lie).unwrap()).await;
    pass(&alice, &bob, Duration::hours(25));
    refresh(&bob.node).await.unwrap();
    let after = kept(&bob, &alice);
    assert_eq!(
        after.about, good.about,
        "the answer that was too long is not kept"
    );
    assert!(after.error.unwrap().contains("beyond the limits"));
}
