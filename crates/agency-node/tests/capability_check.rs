//! The root does not sign a version that uses a field older nodes refuse until every node of the org lists the
//! capability for it, judging by what it has kept of them (their `about`). Two real nodes: alice, the root,
//! and bob, who runs the team `web`.
mod common;

use agency_core::peer_about::{read_origin, PeerAbout};
use agency_core::views::refresh;
use agency_core::{departments, kpis, orgs, teams, ErrorKind};
use agency_proto::about::About;
use agency_proto::clock::Duration;
use agency_proto::org::{Allocation, TeamPlace, TeamPlan};
use agency_proto::outcome::{GoodWhen, Kpi, KpiKind, KpiUnit};
use common::{alice, id, joined_bob, Running};
use std::collections::BTreeMap;

fn kpi(raw: &str) -> Kpi {
    Kpi {
        id: id(raw),
        label: raw.into(),
        kind: KpiKind::Product,
        unit: KpiUnit::Count,
        good_when: GoodWhen::Higher,
    }
}

/// Alice's org with a second team on bob's node: bob is the one peer.
async fn alice_and_bob() -> (Running, Running) {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let web = TeamPlace::new(id("web"), "Web", bob.url());
    orgs::add_team(&alice.node, &id("acme"), web).unwrap();
    (alice, bob)
}

fn version(alice: &Running) -> u64 {
    alice
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .version
}

/// What alice has kept of bob: `about` as read `ago` ago.
fn keep(alice: &Running, bob: &Running, about: About, ago: Duration) {
    let read_at = agency_proto::clock::format(alice.node.now() - ago);
    let entry = PeerAbout {
        about: Some(about),
        read_at: Some(read_at.clone()),
        tried_at: read_at,
        ..PeerAbout::default()
    };
    let kept = BTreeMap::from([(bob.url(), entry)]);
    alice
        .node
        .store()
        .save_peer_abouts(&id("acme"), &kept)
        .unwrap();
}

fn without(names: &[&str]) -> About {
    let mut about = About::of_release("0.3.0");
    about
        .capabilities
        .retain(|one| !names.contains(&one.as_str()));
    about
}

#[tokio::test]
async fn a_node_never_read_holds_back_a_version_with_a_new_field_and_the_refusal_says_what_to_do() {
    let (alice, bob) = alice_and_bob().await;
    let before = version(&alice);
    let refused = kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    for part in [
        bob.url().as_str(),
        "orgKpis",
        "has not been read",
        "agency sync",
    ] {
        assert!(
            refused.message.contains(part),
            "{part}: {}",
            refused.message
        );
    }
    assert_eq!(version(&alice), before, "nothing was written");

    refresh(&alice.node).await.unwrap();
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();
    assert_eq!(
        version(&alice),
        before + 1,
        "once bob was read, and lists it"
    );
}

#[tokio::test]
async fn a_node_that_lacks_the_capability_blocks_it_and_is_named_with_its_release() {
    let (alice, bob) = alice_and_bob().await;
    keep(&alice, &bob, without(&["orgKpis"]), Duration::minutes(5));
    let refused = kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert!(
        refused.message.contains("does not list orgKpis"),
        "{}",
        refused.message
    );
    assert!(
        refused.message.contains("release 0.3.0"),
        "{}",
        refused.message
    );

    keep(&alice, &bob, without(&["webPush"]), Duration::minutes(5));
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();
}

#[tokio::test]
async fn an_answer_older_than_three_days_does_not_count() {
    let (alice, bob) = alice_and_bob().await;
    keep(&alice, &bob, About::of_release("0.3.0"), Duration::days(4));
    let refused = kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap_err();
    assert!(
        refused.message.contains("more than 3 days"),
        "{}",
        refused.message
    );
    keep(&alice, &bob, About::of_release("0.3.0"), Duration::days(2));
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();
}

#[tokio::test]
async fn only_a_capability_the_current_version_does_not_use_yet_is_checked() {
    let (alice, bob) = alice_and_bob().await;
    keep(
        &alice,
        &bob,
        About::of_release("0.3.0"),
        Duration::minutes(5),
    );
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();

    // Bob now lists nothing, but the org already carries KPIs and a version without a new field is fine.
    keep(
        &alice,
        &bob,
        without(&["orgKpis", "orgPlan", "orgAims"]),
        Duration::minutes(5),
    );
    kpis::add(&alice.node, &id("acme"), kpi("revenue")).unwrap();
    departments::add(&alice.node, &id("acme"), department("dev")).unwrap();

    // A plan is a field the org does not carry yet: that is checked.
    let allocation = Allocation {
        new_feature: 40,
        improvement: 20,
        maintenance: 20,
        debt: 15,
        incident: 5,
    };
    let plan = TeamPlan::new(allocation, 500);
    let refused =
        departments::plan_team(&alice.node, &id("acme"), &id("web"), Some(plan)).unwrap_err();
    assert!(refused.message.contains("orgPlan"), "{}", refused.message);
}

fn department(raw: &str) -> agency_proto::org::Department {
    agency_proto::org::Department {
        id: id(raw),
        label: raw.into(),
        parent: None,
        head: None,
    }
}

#[tokio::test]
async fn a_version_with_no_new_field_never_waits_for_a_peer() {
    let (alice, _bob) = alice_and_bob().await;
    departments::add(&alice.node, &id("acme"), department("dev")).unwrap();
}

#[tokio::test]
async fn taking_the_lagging_nodes_team_out_of_the_org_lets_the_version_through() {
    let (alice, bob) = alice_and_bob().await;
    keep(&alice, &bob, without(&["orgKpis"]), Duration::minutes(5));
    assert!(kpis::add(&alice.node, &id("acme"), kpi("activation")).is_err());
    teams::remove(&alice.node, &id("acme"), &id("web")).unwrap();
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();
}

#[tokio::test]
async fn the_nodes_checked_are_those_of_the_version_being_signed_not_of_the_one_before() {
    let (alice, bob) = alice_and_bob().await;
    keep(&alice, &bob, without(&["orgKpis"]), Duration::minutes(5));

    // One version that adds a team on a node never read and a KPI: that node is one that reads it.
    let carol = TeamPlace::new(id("mobile"), "Mobile", "http://127.0.0.1:1");
    let refused = orgs::amend(&alice.node, &id("acme"), |body| {
        body.teams.push(carol);
        body.kpis.push(kpi("revenue"));
        Ok(())
    })
    .unwrap_err();
    assert!(
        refused.message.contains("http://127.0.0.1:1"),
        "{}",
        refused.message
    );

    // One version that takes bob's team out and adds a KPI: bob does not read it, so it does not hold it back.
    let both = orgs::amend(&alice.node, &id("acme"), |body| {
        body.teams.retain(|team| team.id != id("web"));
        body.kpis.push(kpi("activation"));
        Ok(())
    });
    assert!(both.is_ok(), "{both:?}");
}

/// What alice has kept of another node (as well as of bob): `about` as read five minutes ago.
fn keep_also(alice: &Running, origin: &str, about: About) {
    let mut kept = alice.node.store().peer_abouts(&id("acme"));
    let read_at = agency_proto::clock::format(alice.node.now() - Duration::minutes(5));
    let entry = PeerAbout {
        about: Some(about),
        read_at: Some(read_at.clone()),
        tried_at: read_at,
        ..PeerAbout::default()
    };
    kept.insert(origin.to_owned(), entry);
    alice
        .node
        .store()
        .save_peer_abouts(&id("acme"), &kept)
        .unwrap();
}

const NEWCOMER: &str = "http://127.0.0.1:2";

#[tokio::test]
async fn a_node_the_version_brings_into_the_org_must_list_everything_the_org_already_uses() {
    let (alice, bob) = alice_and_bob().await;
    keep(
        &alice,
        &bob,
        About::of_release("0.3.0"),
        Duration::minutes(5),
    );
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();

    // The org carries KPIs already, so a team on a node that cannot read them must not be added: that node
    // would refuse every version of the org from then on.
    keep_also(&alice, NEWCOMER, without(&["orgKpis"]));
    let team = || TeamPlace::new(id("mobile"), "Mobile", NEWCOMER);
    let refused = orgs::add_team(&alice.node, &id("acme"), team()).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    for part in [NEWCOMER, "new to the org", "does not list orgKpis"] {
        assert!(
            refused.message.contains(part),
            "{part}: {}",
            refused.message
        );
    }
    assert!(
        !refused.message.contains(&bob.url()),
        "bob is fine: {}",
        refused.message
    );

    keep_also(&alice, NEWCOMER, About::of_release("0.3.0"));
    orgs::add_team(&alice.node, &id("acme"), team()).unwrap();
}

#[tokio::test]
async fn a_node_not_read_at_all_is_not_brought_in_either() {
    let (alice, bob) = alice_and_bob().await;
    keep(
        &alice,
        &bob,
        About::of_release("0.3.0"),
        Duration::minutes(5),
    );
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();
    let team = TeamPlace::new(id("mobile"), "Mobile", NEWCOMER);
    let refused = orgs::add_team(&alice.node, &id("acme"), team).unwrap_err();
    assert!(
        refused.message.contains("has not been read yet"),
        "{}",
        refused.message
    );
}

#[tokio::test]
async fn a_second_team_on_a_node_already_in_the_org_is_not_a_new_node() {
    let (alice, bob) = alice_and_bob().await;
    keep(
        &alice,
        &bob,
        About::of_release("0.3.0"),
        Duration::minutes(5),
    );
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();
    keep(&alice, &bob, without(&["orgKpis"]), Duration::minutes(5));
    let second = TeamPlace::new(id("web2"), "Web 2", bob.url());
    orgs::add_team(&alice.node, &id("acme"), second).unwrap();
}

#[tokio::test]
async fn moving_a_team_to_a_node_brings_that_node_in() {
    let (alice, bob) = alice_and_bob().await;
    keep(
        &alice,
        &bob,
        About::of_release("0.3.0"),
        Duration::minutes(5),
    );
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();
    keep_also(&alice, NEWCOMER, without(&["orgKpis"]));
    let refused =
        orgs::move_team(&alice.node, &id("acme"), &id("web"), NEWCOMER.into()).unwrap_err();
    assert!(
        refused.message.contains("does not list orgKpis"),
        "{}",
        refused.message
    );
    keep_also(&alice, NEWCOMER, About::of_release("0.3.0"));
    orgs::move_team(&alice.node, &id("acme"), &id("web"), NEWCOMER.into()).unwrap();
}

#[tokio::test]
async fn a_node_that_joined_the_org_can_be_read_before_it_is_given_a_team() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    kpis::add(&alice.node, &id("acme"), kpi("activation")).unwrap();

    // Alice has never read bob: no team names his node.
    let team = || TeamPlace::new(id("mobile"), "Mobile", bob.url());
    let refused = orgs::add_team(&alice.node, &id("acme"), team()).unwrap_err();
    assert!(
        refused.message.contains("has not been read yet"),
        "{}",
        refused.message
    );

    // Reading the node first, as `agency team-add --node` does, is enough: bob joined, so he holds the
    // definition (with its KPIs) and answers.
    read_origin(&alice.node, &id("acme"), &bob.url())
        .await
        .unwrap();
    let kept = alice.node.store().peer_abouts(&id("acme"));
    assert_eq!(
        kept[&bob.url()].about,
        Some(agency_core::about::this_node())
    );
    orgs::add_team(&alice.node, &id("acme"), team()).unwrap();
}

#[tokio::test]
async fn a_node_that_cannot_be_reached_is_kept_as_not_answering() {
    let (alice, _) = alice().await;
    read_origin(&alice.node, &id("acme"), "http://127.0.0.1:1")
        .await
        .unwrap();
    let entry = alice.node.store().peer_abouts(&id("acme"))["http://127.0.0.1:1"].clone();
    assert_eq!(entry.about, None);
    assert!(entry.error.is_some());
}
