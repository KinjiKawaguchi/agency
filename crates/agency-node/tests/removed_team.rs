//! A team the org removed: its node is told, nodes drop their copy of its view, and a node that still
//! has it in its config stops serving it.
mod common;

use agency_core::config::{HostedTeam, RuntimeCommand};
use agency_core::orgs::{self, misplaced_teams};
use agency_core::outbox::{deliver_due, pending, ParcelBody};
use agency_core::teams;
use agency_core::views::refresh;
use agency_proto::org::TeamPlace;
use common::{alice, body, get, id, joined_bob, Running};
use reqwest::StatusCode;

fn place(team: &str, node: &str) -> TeamPlace {
    TeamPlace::new(id(team), team, node)
}

fn host(running: &Running, team: &str) {
    running
        .node
        .update_config(|config| {
            let runtime = RuntimeCommand::of(vec!["/nonexistent".into()]);
            config
                .teams
                .push(HostedTeam::new(id("acme"), id(team), id("alice"), runtime))
        })
        .unwrap();
}

fn unhost(running: &Running, team: &str) {
    running
        .node
        .update_config(|config| config.teams.retain(|one| one.team != id(team)))
        .unwrap();
}

#[tokio::test]
async fn a_node_does_not_serve_a_team_its_org_no_longer_lists() {
    let (alice, _) = alice().await;
    // The root removes the team while this node's config still lists it: what a node that does not hold
    // the root key sees after it takes the version that removes the team.
    orgs::amend(&alice.node, &id("acme"), |definition| {
        definition.teams_mut().clear();
        Ok(())
    })
    .unwrap();
    assert!(alice.node.hosted(&id("acme"), &id("app")).is_none());
    assert_eq!(misplaced_teams(&alice.node).len(), 1, "it says so at start");
    let (status, _) = body(
        get(&alice, "/api/orgs/acme/teams/app/view")
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_version_that_removes_a_team_is_sent_to_the_teams_own_node_too() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    orgs::add_team(&alice.node, &id("acme"), place("web", &bob.url())).unwrap();
    deliver_due(&alice.node).await.unwrap();
    assert!(body_of(&bob).team(&id("web")).is_some());

    teams::remove(&alice.node, &id("acme"), &id("web")).unwrap();
    let to_bob = pending(&alice.node)
        .into_iter()
        .filter(|parcel| parcel.to == bob.url())
        .collect::<Vec<_>>();
    assert_eq!(to_bob.len(), 1, "the removed team's node is not left out");
    assert!(matches!(&to_bob[0].body, ParcelBody::Org(doc) if doc.body.team(&id("web")).is_none()));
    deliver_due(&alice.node).await.unwrap();
    assert!(body_of(&bob).team(&id("web")).is_none());
}

#[tokio::test]
async fn the_version_that_removes_a_team_waits_for_its_node_whatever_the_root_signs_after_it() {
    let (alice, _) = alice().await;
    let mut bob = joined_bob(&alice).await;
    orgs::add_team(&alice.node, &id("acme"), place("web", &bob.url())).unwrap();
    deliver_due(&alice.node).await.unwrap();
    assert!(body_of(&bob).team(&id("web")).is_some());

    // bob is down when the team is removed, and the root signs another version after it: nothing is queued for
    // bob after the removal, which is the newest that waits for him and must not be dropped.
    bob.stop().await;
    teams::remove(&alice.node, &id("acme"), &id("web")).unwrap();
    orgs::amend(&alice.node, &id("acme"), |definition| {
        definition.label = "Acme Inc".into();
        Ok(())
    })
    .unwrap();
    deliver_due(&alice.node).await.unwrap();
    let bob_url = bob.url();
    let to_bob = |running: &Running| {
        pending(&running.node)
            .into_iter()
            .filter(|parcel| parcel.to == bob_url)
            .count()
    };
    assert_eq!(to_bob(&alice), 1, "the removal still waits for bob");

    bob.start().await;
    alice
        .clock
        .advance(agency_proto::clock::Duration::minutes(10));
    deliver_due(&alice.node).await.unwrap();
    assert!(
        body_of(&bob).team(&id("web")).is_none(),
        "bob heard that the team was removed"
    );
    assert_eq!(to_bob(&alice), 0);
}

fn body_of(running: &Running) -> agency_proto::OrgDefinition {
    running.node.store().load_org(&id("acme")).unwrap().body
}

#[tokio::test]
async fn a_member_drops_the_replica_of_a_removed_team() {
    let (alice, _) = alice().await;
    host(&alice, "web");
    orgs::add_team(&alice.node, &id("acme"), place("web", &alice.url())).unwrap();
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    let replica = |team: &str| bob.node.store().load_replica(&id("acme"), &id(team));
    assert!(replica("app").is_some() && replica("web").is_some());

    unhost(&alice, "web");
    teams::remove(&alice.node, &id("acme"), &id("web")).unwrap();
    bob.clock.advance(agency_proto::clock::Duration::seconds(1));
    refresh(&bob.node).await.unwrap();
    assert!(body_of(&bob).team(&id("web")).is_none());
    assert!(
        replica("web").is_none(),
        "the copy of a removed team's view goes"
    );
    assert!(replica("app").is_some(), "the others stay");
}
