//! A team's label and its place in the org's definition: renamed and removed only on the root's node,
//! and never removed while it still runs on this node.
mod common;

use agency_core::{orgs, teams, ErrorKind};
use agency_proto::org::TeamPlace;
use common::{alice, id, joined_bob, Running};

fn body(running: &Running) -> agency_proto::OrgDefinition {
    running.node.store().load_org(&id("acme")).unwrap().body
}

/// A team in the org's definition that is not in alice's own configuration.
fn add_unhosted_team(alice: &Running, team: &str) {
    let place = TeamPlace {
        id: id(team),
        label: team.into(),
        department: None,
        node: "http://127.0.0.1:9".into(),
    };
    orgs::add_team(&alice.node, &id("acme"), place).unwrap();
}

#[tokio::test]
async fn the_root_renames_a_team_in_one_version_and_changes_nothing_else() {
    let (alice, _) = alice().await;
    let before = body(&alice);
    teams::set_label(&alice.node, &id("acme"), &id("app"), "Application".into()).unwrap();

    let after = body(&alice);
    assert_eq!(after.version, before.version + 1);
    let team = after.team(&id("app")).unwrap();
    assert_eq!(team.label, "Application");
    let was = before.team(&id("app")).unwrap();
    assert_eq!((&team.node, &team.department), (&was.node, &was.department));
}

#[tokio::test]
async fn a_rename_that_changes_nothing_or_names_nothing_valid_is_refused() {
    let (alice, _) = alice().await;
    let version = body(&alice).version;
    let rename = |team: &str, label: &str| {
        teams::set_label(&alice.node, &id("acme"), &id(team), label.into())
    };
    assert_eq!(rename("app", "app").unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(rename("web", "Web").unwrap_err().kind, ErrorKind::NotFound);
    assert_eq!(rename("app", "").unwrap_err().kind, ErrorKind::BadRequest);
    assert_eq!(body(&alice).version, version, "a refusal writes no version");
}

#[tokio::test]
async fn a_team_still_in_the_nodes_config_is_not_removed() {
    let (alice, _) = alice().await;
    let version = body(&alice).version;
    let error = teams::remove(&alice.node, &id("acme"), &id("app")).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert!(
        error
            .to_string()
            .contains("remove it from config.json first"),
        "names the fix: {error}"
    );
    let after = body(&alice);
    assert_eq!(after.version, version);
    assert!(after.team(&id("app")).is_some());
}

#[tokio::test]
async fn a_team_taken_out_of_the_config_is_removed_in_one_version() {
    let (alice, _) = alice().await;
    add_unhosted_team(&alice, "web");
    let before = body(&alice);

    teams::remove(&alice.node, &id("acme"), &id("web")).unwrap();
    let after = body(&alice);
    assert_eq!(after.version, before.version + 1);
    assert!(after.team(&id("web")).is_none());
    assert!(after.team(&id("app")).is_some(), "other teams stay");

    alice
        .node
        .update_config(|config| config.teams.clear())
        .unwrap();
    teams::remove(&alice.node, &id("acme"), &id("app")).unwrap();
    assert!(body(&alice).teams.is_empty());
}

#[tokio::test]
async fn a_team_that_is_not_in_the_org_is_not_found() {
    let (alice, _) = alice().await;
    let error = teams::remove(&alice.node, &id("acme"), &id("web")).unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotFound);
}

#[tokio::test]
async fn only_the_root_changes_a_team() {
    let (alice, _) = alice().await;
    add_unhosted_team(&alice, "web");
    let bob = joined_bob(&alice).await;
    let version = body(&bob).version;
    let renamed = teams::set_label(&bob.node, &id("acme"), &id("web"), "Web".into());
    assert_eq!(renamed.unwrap_err().kind, ErrorKind::Forbidden);
    let removed = teams::remove(&bob.node, &id("acme"), &id("web"));
    assert_eq!(removed.unwrap_err().kind, ErrorKind::Forbidden);
    assert_eq!(body(&bob).version, version);
}
