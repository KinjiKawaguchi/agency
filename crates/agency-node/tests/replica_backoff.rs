//! A team node that does not answer is read again after a wait that grows as the outbox's does (15 s, 30 s, 1 min,
//! 2 min, 5 min, then every 5 min), not on every pass. One answer ends the wait, `agency sync` reads at once, and
//! `agency status` names the team while it waits.
mod common;

use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::orgs;
use agency_core::outbox::deliver_due;
use agency_core::views::{refresh, refresh_now, waiting_lines};
use agency_proto::clock::Duration;
use agency_proto::org::TeamPlace;
use common::{alice, id, joined_bob, Running};

/// An origin where nothing listens: every read of it fails at once.
fn closed_origin() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    origin
}

fn place_team(alice: &Running, team: &str, origin: &str) {
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.teams
            .push(TeamPlace::new(id(team), team, origin.to_owned()));
        Ok(())
    })
    .unwrap();
}

fn failures(alice: &Running, team: &str) -> u32 {
    alice
        .node
        .store()
        .load_replica(&id("acme"), &id(team))
        .map_or(0, |replica| replica.failures)
}

#[tokio::test]
async fn a_node_that_does_not_answer_is_read_less_and_less_often() {
    let (alice, _) = alice().await;
    place_team(&alice, "gone", &closed_origin());
    let mut tried_at = vec![];
    // A pass every 15 s for an hour.
    for _ in 0..240 {
        refresh(&alice.node).await.unwrap();
        let replica = alice
            .node
            .store()
            .load_replica(&id("acme"), &id("gone"))
            .unwrap();
        if tried_at.last() != Some(&replica.tried_at) {
            tried_at.push(replica.tried_at.clone());
        }
        alice.clock.advance(Duration::seconds(15));
    }
    // Reads at 0 s, 15 s, 45 s, 105 s, 225 s, then every 300 s: 16 in the hour instead of 240.
    assert_eq!(tried_at.len(), 16, "{tried_at:?}");
    assert_eq!(failures(&alice, "gone"), 16);
    let lines = waiting_lines(&alice.node);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("acme/gone") && lines[0].contains("16 tries"),
        "{lines:?}"
    );
}

#[tokio::test]
async fn one_answer_ends_the_wait_and_sync_reads_at_once() {
    let (alice, _) = alice().await;
    let mut bob = joined_bob(&alice).await;
    bob.node
        .update_config(|config| {
            let runtime = RuntimeCommand::of(vec!["/nonexistent".into()]);
            config
                .teams
                .push(HostedTeam::new(id("acme"), id("b0"), id("alice"), runtime));
        })
        .unwrap();
    place_team(&alice, "b0", &bob.url());
    deliver_due(&alice.node).await.unwrap();

    bob.stop().await;
    refresh(&alice.node).await.unwrap();
    refresh(&alice.node).await.unwrap();
    assert_eq!(
        failures(&alice, "b0"),
        1,
        "the second pass waits for the first wait to end"
    );

    bob.start().await;
    refresh(&alice.node).await.unwrap();
    assert_eq!(
        failures(&alice, "b0"),
        1,
        "still waiting, though bob is back"
    );
    refresh_now(&alice.node).await.unwrap();
    assert_eq!(
        failures(&alice, "b0"),
        0,
        "sync reads at once, and the answer ends the wait"
    );
    assert!(waiting_lines(&alice.node).is_empty());
}
