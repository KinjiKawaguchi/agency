//! Departments and where teams are, kept in the org's signed definition and changed only by its root.
mod common;

use agency_core::departments::{self, Change};
use agency_core::{orgs, ErrorKind};
use agency_proto::org::{Department, TeamPlace};
use common::{alice, id, joined_bob, Running};

fn department(raw: &str, label: &str, parent: Option<&str>, head: Option<&str>) -> Department {
    Department {
        id: id(raw),
        label: label.into(),
        parent: parent.map(id),
        head: head.map(id),
    }
}

fn body(running: &Running) -> agency_proto::OrgDefinition {
    running.node.store().load_org(&id("acme")).unwrap().body
}

#[tokio::test]
async fn the_root_builds_departments_and_places_teams_in_them() {
    let (alice, _) = alice().await;
    let org = id("acme");
    let before = body(&alice).version;
    departments::add(
        &alice.node,
        &org,
        department("dev", "Development", None, Some("alice")),
    )
    .unwrap();
    departments::add(
        &alice.node,
        &org,
        department("clients", "Clients", Some("dev"), None),
    )
    .unwrap();
    departments::place_team(&alice.node, &org, &id("app"), Some(id("dev"))).unwrap();
    let web = TeamPlace {
        id: id("web"),
        label: "Web".into(),
        department: Some(id("clients")),
        node: alice.url(),
    };
    orgs::add_team(&alice.node, &org, web).unwrap();

    let now = body(&alice);
    assert_eq!(now.version, before + 4, "one signed version per change");
    assert_eq!(now.departments[0].head, Some(id("alice")));
    assert_eq!(now.departments[1].parent, Some(id("dev")));
    let team_of = |team: &str| now.team(&id(team)).unwrap().department.clone();
    assert_eq!(
        (team_of("app"), team_of("web")),
        (Some(id("dev")), Some(id("clients")))
    );

    let renamed = Change {
        label: Some("Agency development".into()),
        head: Some(None),
        ..Change::default()
    };
    departments::change(&alice.node, &org, &id("dev"), renamed).unwrap();
    let dev = body(&alice).departments[0].clone();
    assert_eq!((dev.label.as_str(), dev.head), ("Agency development", None));

    departments::place_team(&alice.node, &org, &id("app"), None).unwrap();
    assert_eq!(body(&alice).team(&id("app")).unwrap().department, None);
}

#[tokio::test]
async fn departments_stay_a_tree_with_real_links() {
    let (alice, _) = alice().await;
    let org = id("acme");
    departments::add(
        &alice.node,
        &org,
        department("dev", "Development", None, None),
    )
    .unwrap();
    departments::add(
        &alice.node,
        &org,
        department("clients", "Clients", Some("dev"), None),
    )
    .unwrap();
    let version = body(&alice).version;

    let refusals = [
        (
            "a duplicate id",
            departments::add(&alice.node, &org, department("dev", "Again", None, None)),
            ErrorKind::Conflict,
        ),
        (
            "an unknown parent",
            departments::add(
                &alice.node,
                &org,
                department("ops", "Ops", Some("nope"), None),
            ),
            ErrorKind::NotFound,
        ),
        (
            "a head who is not a member",
            departments::add(
                &alice.node,
                &org,
                department("ops", "Ops", None, Some("carol")),
            ),
            ErrorKind::NotFound,
        ),
        (
            "a department under its own child",
            departments::change(
                &alice.node,
                &org,
                &id("dev"),
                Change {
                    parent: Some(Some(id("clients"))),
                    ..Change::default()
                },
            ),
            ErrorKind::BadRequest,
        ),
        (
            "a department under itself",
            departments::change(
                &alice.node,
                &org,
                &id("dev"),
                Change {
                    parent: Some(Some(id("dev"))),
                    ..Change::default()
                },
            ),
            ErrorKind::BadRequest,
        ),
        (
            "removing a department that holds one",
            departments::remove(&alice.node, &org, &id("dev")),
            ErrorKind::Conflict,
        ),
        (
            "a team in an unknown department",
            departments::place_team(&alice.node, &org, &id("app"), Some(id("nope"))),
            ErrorKind::NotFound,
        ),
        (
            "a new team in an unknown department",
            orgs::add_team(
                &alice.node,
                &org,
                TeamPlace {
                    id: id("web"),
                    label: "Web".into(),
                    department: Some(id("nope")),
                    node: alice.url(),
                },
            ),
            ErrorKind::NotFound,
        ),
    ];
    for (case, result, kind) in refusals {
        assert_eq!(result.map(|_| ()).unwrap_err().kind, kind, "{case}");
    }
    assert_eq!(body(&alice).version, version, "nothing was signed");

    departments::remove(&alice.node, &org, &id("clients")).unwrap();
    departments::remove(&alice.node, &org, &id("dev")).unwrap();
    assert!(body(&alice).departments.is_empty());
}

#[tokio::test]
async fn only_the_roots_node_changes_departments() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let refused = departments::add(
        &bob.node,
        &id("acme"),
        department("dev", "Development", None, None),
    )
    .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Forbidden);
}
