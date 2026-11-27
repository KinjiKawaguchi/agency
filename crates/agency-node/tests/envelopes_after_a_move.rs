//! A team's node takes no envelope that was made before the team came to it: such an envelope was made for the
//! node the team ran on before, which received it and may have applied it, and this node cannot know. A version
//! that does not move the team changes nothing.
mod common;

use agency_core::decisions::{accept_envelope, Accepted};
use agency_core::devices::add_device;
use agency_core::orgs::{amend, move_team};
use agency_core::ErrorKind;
use agency_proto::clock::Duration;
use agency_proto::AccountKey;
use common::{alice, id, signed_answer, Running};

const ELSEWHERE: &str = "http://127.0.0.1:9";

async fn with_phone() -> (Running, AccountKey) {
    let (alice, _) = alice().await;
    let phone = AccountKey::generate();
    add_device(&alice.node, "Phone", &phone.public(), None).await;
    (alice, phone)
}

/// The team moves away and back: its place now names this node again, from a version made a minute ago.
fn move_away_and_back(alice: &Running) {
    alice.clock.advance(Duration::minutes(1));
    move_team(&alice.node, &id("acme"), &id("app"), ELSEWHERE.into()).unwrap();
    alice.clock.advance(Duration::minutes(1));
    move_team(&alice.node, &id("acme"), &id("app"), alice.url()).unwrap();
    alice.clock.advance(Duration::minutes(1));
}

#[tokio::test]
async fn an_envelope_made_before_the_team_came_to_this_node_is_refused() {
    let (alice, phone) = with_phone().await;
    let made_before = signed_answer(&alice, &phone, "r-1", "a");
    move_away_and_back(&alice);

    let refused = accept_envelope(&alice.node, &id("acme"), &id("app"), made_before)
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::BadRequest, "{refused:?}");
    assert!(
        refused.message.contains("before the team came"),
        "{refused:?}"
    );
}

#[tokio::test]
async fn an_envelope_made_after_the_team_came_is_applied() {
    let (alice, phone) = with_phone().await;
    move_away_and_back(&alice);
    let made_after = signed_answer(&alice, &phone, "r-1", "a");

    let accepted = accept_envelope(&alice.node, &id("acme"), &id("app"), made_after)
        .await
        .unwrap();
    assert_eq!(accepted, Accepted::Applied);
}

#[tokio::test]
async fn a_version_that_does_not_move_the_team_refuses_nothing() {
    let (alice, phone) = with_phone().await;
    let made_before = signed_answer(&alice, &phone, "r-1", "a");
    alice.clock.advance(Duration::minutes(1));
    amend(&alice.node, &id("acme"), |body| {
        body.label = "Acme, Inc.".into();
        Ok(())
    })
    .unwrap();

    let accepted = accept_envelope(&alice.node, &id("acme"), &id("app"), made_before)
        .await
        .unwrap();
    assert_eq!(accepted, Accepted::Applied);
}

#[tokio::test]
async fn a_version_that_only_spells_the_place_another_way_is_no_move() {
    let (alice, phone) = with_phone().await;
    alice.clock.advance(Duration::minutes(1));
    move_team(
        &alice.node,
        &id("acme"),
        &id("app"),
        "https://Agency.Example.com".into(),
    )
    .unwrap();
    alice.clock.advance(Duration::minutes(1));
    let made_between = signed_answer(&alice, &phone, "r-1", "a");
    alice.clock.advance(Duration::minutes(1));
    move_team(
        &alice.node,
        &id("acme"),
        &id("app"),
        "https://agency.example.com:443".into(),
    )
    .unwrap();
    alice.clock.advance(Duration::minutes(1));

    let accepted = accept_envelope(&alice.node, &id("acme"), &id("app"), made_between)
        .await
        .unwrap();
    assert_eq!(
        accepted,
        Accepted::Applied,
        "the same origin, written another way"
    );
}
