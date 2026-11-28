use crate::keys::AccountKey;
use crate::org::{Department, OrgDefinition, Person, Root, TeamPlace};
use crate::Id;

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn person(name: &str, seed: u8, department: Option<&str>) -> Person {
    let key = AccountKey::from_seed([seed; 32]).public();
    Person::new(id(name), name, id(name), key).with_department(department.map(id))
}

fn department(name: &str, parent: Option<&str>, head: Option<&str>) -> Department {
    Department {
        id: id(name),
        label: name.into(),
        parent: parent.map(id),
        head: head.map(id),
    }
}

/// acme: alice is the root; `product` (headed by alice) holds `dev` (headed by bob) and `ops` (no head); carol is in
/// `dev`, dave in `ops`, bob in `dev`; the team `app` is in `dev`, `web` in no department.
fn acme() -> OrgDefinition {
    let root = Root::new(id("alice"), AccountKey::from_seed([1; 32]).public());
    OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        root,
        "2026-11-28T00:00:00Z".into(),
    )
    .with_departments(vec![
        department("product", None, Some("alice")),
        department("dev", Some("product"), Some("bob")),
        department("ops", Some("product"), None),
    ])
    .with_people(vec![
        person("alice", 1, Some("product")),
        person("bob", 2, Some("dev")),
        person("carol", 3, Some("dev")),
        person("dave", 4, Some("ops")),
    ])
    .with_teams(vec![
        TeamPlace::new(id("app"), "App", "https://one.example.com")
            .with_department(Some(id("dev"))),
        TeamPlace::new(id("web"), "Web", "https://two.example.com"),
    ])
}

#[test]
fn the_teams_are_the_definitions_places_in_its_order() {
    let org = acme();
    let teams: Vec<(Id, String, Option<Id>)> = org
        .teams()
        .map(|team| (team.id, team.node, team.department))
        .collect();
    assert_eq!(
        teams,
        [
            (
                id("app"),
                "https://one.example.com".to_owned(),
                Some(id("dev"))
            ),
            (id("web"), "https://two.example.com".to_owned(), None),
        ]
    );
    assert_eq!(org.team(&id("web")).unwrap().label, "Web");
    assert!(org.team(&id("dev")).is_none(), "a department is not a team");
}

#[test]
fn the_units_are_the_departments_then_the_teams_and_only_the_departments_hold_others() {
    let org = acme();
    let units: Vec<(Id, Option<Id>, bool)> = org
        .units()
        .map(|unit| (unit.id, unit.parent, unit.team.is_some()))
        .collect();
    assert_eq!(
        units,
        [
            (id("product"), None, false),
            (id("dev"), Some(id("product")), false),
            (id("ops"), Some(id("product")), false),
            (id("app"), Some(id("dev")), true),
            (id("web"), None, true),
        ]
    );
    assert_eq!(org.group(&id("dev")).unwrap().head, Some(id("bob")));
    assert!(org.group(&id("app")).is_none(), "a team holds no unit");
}

#[test]
fn a_department_and_a_team_may_share_an_id_and_a_parent_is_the_department() {
    let org = acme().with_teams(vec![TeamPlace::new(
        id("dev"),
        "Dev team",
        "https://one.example.com",
    )]);
    assert_eq!(org.units().filter(|unit| unit.id == id("dev")).count(), 2);
    assert_eq!(org.group(&id("dev")).unwrap().label, "dev");
    assert_eq!(org.team(&id("dev")).unwrap().label, "Dev team");
}

#[test]
fn the_org_is_headed_by_the_root_s_person_or_by_no_one() {
    assert_eq!(acme().head(), Some(&id("alice")));
    let without = acme().with_people(vec![person("bob", 2, None)]);
    assert_eq!(
        without.head(),
        None,
        "the root's account is no person of the org"
    );
}

#[test]
fn a_person_reports_to_the_head_of_the_unit_it_is_in_and_a_head_to_the_head_above() {
    let org = acme();
    let reports = |name: &str| org.reports_to(&id(name));
    assert_eq!(reports("carol"), Some(id("bob")), "dev's head");
    assert_eq!(
        reports("bob"),
        Some(id("alice")),
        "dev's head reports to product's"
    );
    assert_eq!(
        reports("dave"),
        Some(id("alice")),
        "ops has no head: the head above it"
    );
    assert_eq!(reports("alice"), None, "the org's head reports to no one");
    assert_eq!(reports("mallory"), None, "no such person");

    let loose = org
        .clone()
        .with_people(vec![person("alice", 1, None), person("erin", 5, None)]);
    assert_eq!(
        loose.reports_to(&id("erin")),
        Some(id("alice")),
        "a person in no unit reports to the org's head"
    );
}
