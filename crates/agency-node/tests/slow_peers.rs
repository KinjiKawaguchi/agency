//! A team node that never answers costs one read's timeout, not one per team on it, and holds nothing else up:
//! the replicas of other nodes, the owner's notifications, and the outbox keep their pace.
mod common;

use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::orgs;
use agency_core::outbox::deliver_due;
use agency_core::views::refresh;
use agency_node::background::{self, Schedule};
use agency_proto::org::TeamPlace;
use common::{alice, body, id, joined_bob, post, Running, TOKEN};
use serde_json::json;
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// The timeout a node gives another node's answer (`HttpPeers`).
const TIMEOUT: Duration = Duration::from_secs(8);

/// A listener that takes connections and never answers.
fn silent_node() -> (String, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let accepting = listener.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut held = vec![];
        for stream in accepting.incoming().flatten() {
            held.push(stream);
        }
    });
    (origin, listener)
}

/// Places `count` teams of acme on `origin`.
fn teams_on(alice: &Running, origin: &str, prefix: &str, count: usize) {
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.teams_mut().extend((0..count).map(|team| {
            let team = id(&format!("{prefix}{team}"));
            TeamPlace::new(team.clone(), team.to_string(), origin.to_owned())
        }));
        Ok(())
    })
    .unwrap();
}

/// bob hosts team b0 of acme, and has the version that places it.
async fn bob_hosts_a_team(alice: &Running, bob: &Running) {
    bob.node
        .update_config(|config| {
            let runtime = RuntimeCommand::of(vec!["/nonexistent".into()]);
            config
                .teams
                .push(HostedTeam::new(id("acme"), id("b0"), id("bob"), runtime));
        })
        .unwrap();
    teams_on(alice, &bob.url(), "b", 1);
    deliver_due(&alice.node).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn teams_on_a_node_that_never_answers_cost_one_timeout_together() {
    let (alice, _) = alice().await;
    let (origin, _silent) = silent_node();
    teams_on(&alice, &origin, "s", 4);

    let started = Instant::now();
    refresh(&alice.node).await.unwrap();
    let took = started.elapsed();
    assert!(
        took < TIMEOUT * 3 / 2,
        "4 teams read at once wait about one timeout, not four: {took:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_node_that_never_answers_does_not_hold_the_outbox_for_another_node() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    bob_hosts_a_team(&alice, &bob).await;
    // Two silent nodes, so that delivering one node after another would wait twice whatever their order.
    let (first, _first) = silent_node();
    let (second, _second) = silent_node();
    teams_on(&alice, &first, "s", 1);
    teams_on(&alice, &second, "z", 1);
    // Several versions in the same second: bob takes each only after the one before it.
    for round in 0..5 {
        orgs::amend(&alice.node, &id("acme"), |body| {
            body.label = format!("Acme {round}");
            Ok(())
        })
        .unwrap();
    }

    let started = Instant::now();
    deliver_due(&alice.node).await.unwrap();
    let took = started.elapsed();
    assert!(
        took < TIMEOUT * 3 / 2,
        "the nodes at once, each silent one tried once: {took:?}"
    );
    let current = alice.node.store().load_org(&id("acme")).unwrap();
    let at_bob = bob.node.store().load_org(&id("acme")).unwrap();
    assert_eq!(
        at_bob.body.version, current.body.version,
        "bob has every version, in order"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn notifications_and_the_outbox_keep_their_pace_while_replicas_wait() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    bob_hosts_a_team(&alice, &bob).await;
    let (origin, _silent) = silent_node();
    teams_on(&alice, &origin, "s", 8);
    // The silent node's first delivery fails and waits for its backoff, as it does once a node has gone quiet.
    deliver_due(&alice.node).await.unwrap();
    let token = json!({ "token": "ab12".repeat(16), "environment": "sandbox", "label": "iPhone" });
    let request = post(&alice, "/api/push/tokens", &token).header("x-agency-token", TOKEN);
    let (status, answer) = body(request.send().await.unwrap()).await;
    assert_eq!(status, 200, "{answer}");

    let fast = Duration::from_millis(100);
    background::start(
        alice.node.clone(),
        alice.logger(),
        Schedule {
            replicas: fast,
            push: fast,
            outbox: fast,
        },
    );
    // The first push pass only records what waits; a request after it is news.
    tokio::time::sleep(Duration::from_millis(500)).await;
    memory
        .lock()
        .unwrap()
        .requests
        .push(("r-2".into(), "Ship it?".into()));
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.label = "Acme, Inc.".into();
        Ok(())
    })
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let is_told = alice
            .notifier
            .0
            .lock()
            .unwrap()
            .sent
            .iter()
            .any(|(_, notice)| format!("{:?}", notice.kind).contains("r-2"));
        let at_bob = bob.node.store().load_org(&id("acme")).unwrap();
        if is_told && at_bob.body.label == "Acme, Inc." {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "within 3 s, while every replica read waits {TIMEOUT:?} for the silent node: told {is_told}, bob's label {:?}",
            at_bob.body.label
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_version_that_waits_for_its_next_attempt_holds_back_the_later_ones() {
    let (alice, _) = alice().await;
    let mut bob = joined_bob(&alice).await;
    bob_hosts_a_team(&alice, &bob).await;
    let before = bob.node.store().load_org(&id("acme")).unwrap().body.version;

    bob.stop().await;
    for round in 0..2 {
        orgs::amend(&alice.node, &id("acme"), |body| {
            body.label = format!("Acme {round}");
            Ok(())
        })
        .unwrap();
    }
    // bob does not answer: the first version waits for its next attempt, and the second is not tried.
    deliver_due(&alice.node).await.unwrap();
    bob.start().await;
    // bob is back, but the first version is not due yet: the second must not go ahead of it.
    deliver_due(&alice.node).await.unwrap();
    let at_bob = bob.node.store().load_org(&id("acme")).unwrap();
    assert_eq!(at_bob.body.version, before, "nothing out of order");

    alice
        .clock
        .advance(agency_proto::clock::Duration::seconds(16));
    deliver_due(&alice.node).await.unwrap();
    let current = alice.node.store().load_org(&id("acme")).unwrap();
    let at_bob = bob.node.store().load_org(&id("acme")).unwrap();
    assert_eq!(at_bob.body.version, current.body.version, "both, in order");
    assert_eq!(at_bob.body.label, "Acme 1");
}
