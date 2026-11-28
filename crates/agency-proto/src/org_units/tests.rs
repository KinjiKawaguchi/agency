use super::*;
use crate::keys::AccountKey;
use crate::org::{Department, OrgDefinition, Person, Root, TeamPlace};
use crate::seat::Seat;

/// A case: what it changes, and how.
type Case<'a> = (&'a str, &'a dyn Fn(&mut OrgDefinition));

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn person(name: &str, seed: u8, unit: Option<&str>) -> Person {
    let key = AccountKey::from_seed([seed; 32]).public();
    let mut person = Person::new(id(name), name, id(name), key);
    person.unit = unit.map(id);
    person
}

fn seat(name: &str, unit: Option<&str>) -> Seat {
    Seat {
        id: id(name),
        label: name.into(),
        department: None,
        unit: unit.map(id),
        ask_about: vec![],
        relays: vec![],
    }
}

/// acme in units: `product` (headed by alice) holds `dev` (headed by the agent dev-lead), which holds the team
/// `app` (headed by the agent app-lead), which holds the team `ios`; bob is in dev, carol in app, dave in ios.
fn acme() -> OrgDefinition {
    let root = Root::new(id("alice"), AccountKey::from_seed([1; 32]).public());
    let mut org = OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        root,
        "2026-11-28T00:00:00Z".into(),
    );
    org.shape = Some(UNIT_SHAPE);
    org.units = vec![
        OrgUnit::new(id("product"), "Product").with_head(Some(id("alice"))),
        OrgUnit::new(id("dev"), "Development")
            .with_parent(Some(id("product")))
            .with_head(Some(id("dev-lead"))),
        OrgUnit::new(id("app"), "App")
            .with_parent(Some(id("dev")))
            .with_head(Some(id("app-lead")))
            .with_team("https://one.example.com"),
        OrgUnit::new(id("ios"), "iOS")
            .with_parent(Some(id("app")))
            .with_team("https://two.example.com"),
    ];
    org.agents = vec![
        Agent::new(id("dev-lead"), "Development lead", Some(id("dev"))),
        Agent::new(id("app-lead"), "App lead", Some(id("app"))),
    ];
    org.with_people(vec![
        person("alice", 1, Some("product")),
        person("bob", 2, Some("dev")),
        person("carol", 3, Some("app")),
        person("dave", 4, Some("ios")),
    ])
}

fn refused(org: &OrgDefinition) -> String {
    org.validate_any_shape().unwrap_err()
}

#[test]
fn a_definition_with_departments_writes_none_of_the_new_fields() {
    let root = Root::new(id("alice"), AccountKey::from_seed([1; 32]).public());
    let org = OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        root,
        "2026-11-28T00:00:00Z".into(),
    )
    .with_people(vec![person("alice", 1, None)])
    .with_departments(vec![Department {
        id: id("dev"),
        label: "Dev".into(),
        parent: None,
        head: None,
    }])
    .with_teams(vec![TeamPlace::new(
        id("app"),
        "App",
        "https://one.example.com",
    )]);
    let text = serde_json::to_string(&org).unwrap();
    for field in [
        "\"shape\"",
        "\"units\"",
        "\"agents\"",
        "\"head\":\"",
        "\"unit\"",
        "\"kind\"",
    ] {
        assert!(!text.contains(field), "{field} in {text}");
    }
    assert_eq!(org.validate(), Ok(()));
}

#[test]
fn a_definition_in_units_round_trips_and_is_valid_in_its_shape() {
    let org = acme();
    let text = serde_json::to_string(&org).unwrap();
    assert!(text.contains("\"shape\":2"), "{text}");
    let again: OrgDefinition = serde_json::from_str(&text).unwrap();
    assert_eq!(again, org);
    assert_eq!(org.validate_any_shape(), Ok(()));
}

#[test]
fn this_release_does_not_take_a_definition_in_units_yet() {
    assert!(acme().validate().unwrap_err().contains("does not take yet"));
}

#[test]
fn a_definition_with_no_shape_and_a_field_of_units_is_refused() {
    let root = Root::new(id("alice"), AccountKey::from_seed([1; 32]).public());
    let plain = OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        root,
        "2026-11-28T00:00:00Z".into(),
    )
    .with_people(vec![person("alice", 1, None)]);
    assert_eq!(plain.validate(), Ok(()));
    let with = |edit: &dyn Fn(&mut OrgDefinition)| {
        let mut org = plain.clone();
        edit(&mut org);
        org.validate().unwrap_err()
    };
    let cases: [Case; 6] = [
        ("units", &|org| {
            org.units = vec![OrgUnit::new(id("dev"), "Dev")]
        }),
        ("agents", &|org| {
            org.agents = vec![Agent::new(id("a"), "A", None)]
        }),
        ("head", &|org| org.head = Some(id("alice"))),
        ("a person's unit", &|org| {
            org.people[0].unit = Some(id("dev"))
        }),
        ("a person's kind", &|org| {
            org.people[0].kind = Some(MemberKind::Node)
        }),
        ("a seat's unit", &|org| {
            org.seats = vec![seat("s", Some("dev"))]
        }),
    ];
    for (what, edit) in cases {
        assert!(with(edit).contains("no shape has a field"), "{what}");
    }
}

#[test]
fn a_definition_in_units_with_a_field_of_departments_and_teams_is_refused() {
    let cases: [Case; 4] = [
        ("a department", &|org| {
            org.departments = vec![Department {
                id: id("old"),
                label: "Old".into(),
                parent: None,
                head: None,
            }]
        }),
        ("a team", &|org| {
            org.teams = vec![TeamPlace::new(id("old"), "Old", "https://one.example.com")]
        }),
        ("a person's department", &|org| {
            org.people[1].department = Some(id("dev"))
        }),
        ("a seat's department", &|org| {
            let mut one = seat("s", None);
            one.department = Some(id("dev"));
            org.seats = vec![one];
        }),
    ];
    for (what, edit) in cases {
        let mut org = acme();
        edit(&mut org);
        assert!(
            refused(&org).contains("written in units has a department"),
            "{what}"
        );
    }
}

#[test]
fn the_shape_is_exactly_two_or_left_out() {
    for shape in [0, 1, 3, 255] {
        let mut org = acme();
        org.shape = Some(shape);
        assert_eq!(refused(&org), format!("unknown shape {shape}"));
    }
}

#[test]
fn units_exist_nest_without_a_cycle_and_not_too_deep() {
    let edited = |edit: &dyn Fn(&mut OrgDefinition)| {
        let mut org = acme();
        edit(&mut org);
        refused(&org)
    };
    assert!(edited(&|org| org.units.push(OrgUnit::new(id("dev"), "Again"))).contains("twice"));
    assert!(edited(&|org| org.units[0].parent = Some(id("nowhere"))).contains("does not have"));
    assert!(edited(&|org| org.units[0].parent = Some(id("ios"))).contains("under itself"));
    assert!(edited(&|org| org.units[1].plan =
        org.units[2].plan.clone().or(Some(crate::org::TeamPlan::new(
            crate::org::Allocation {
                new_feature: 100,
                improvement: 0,
                maintenance: 0,
                debt: 0,
                incident: 0,
            },
            1
        ))))
    .contains("plan and no team"));
    assert!(edited(&|org| org.units[2].team = Some(UnitTeam {
        node: "file:///etc".into()
    }))
    .contains("node URL"));
    let deep = |levels: usize| {
        let mut org = acme();
        org.units = (0..levels)
            .map(|n| {
                let parent = (n > 0).then(|| id(&format!("u{}", n - 1)));
                OrgUnit::new(id(&format!("u{n}")), "U").with_parent(parent)
            })
            .collect();
        org.agents.clear();
        org.people.iter_mut().for_each(|p| p.unit = None);
        org
    };
    assert_eq!(deep(MAX_UNIT_DEPTH).validate_any_shape(), Ok(()));
    assert!(refused(&deep(MAX_UNIT_DEPTH + 1)).contains("deep"));
}

#[test]
fn a_head_is_a_person_or_an_agent_and_never_a_seat_or_a_node_account() {
    let edited = |edit: &dyn Fn(&mut OrgDefinition)| {
        let mut org = acme();
        edit(&mut org);
        org.validate_any_shape()
    };
    assert_eq!(
        edited(&|org| org.head = Some(id("dev-lead"))),
        Ok(()),
        "an agent heads the org"
    );
    let seat_head = edited(&|org| {
        org.seats = vec![seat("designer", Some("app"))];
        org.units[0].head = Some(id("designer"));
    });
    assert!(seat_head
        .unwrap_err()
        .contains("is not a person or an agent"));
    let node_head = edited(&|org| {
        org.people[1].kind = Some(MemberKind::Node);
        org.units[1].head = Some(id("bob"));
    });
    assert!(node_head
        .unwrap_err()
        .contains("is not a person or an agent"));
    let unknown = edited(&|org| org.head = Some(id("mallory")));
    assert!(unknown.unwrap_err().contains("is not a person or an agent"));
}

#[test]
fn agents_and_people_are_in_units_the_org_has_and_ids_name_one_party() {
    let edited = |edit: &dyn Fn(&mut OrgDefinition)| {
        let mut org = acme();
        edit(&mut org);
        refused(&org)
    };
    assert!(edited(&|org| org.agents[0].unit = Some(id("nowhere"))).contains("agent dev-lead"));
    assert!(edited(&|org| org.people[1].unit = Some(id("nowhere"))).contains("bob"));
    assert!(
        edited(&|org| org.agents.push(Agent::new(id("bob"), "Bob too", None)))
            .contains("more than one member, seat, external party, or agent")
    );
}

#[test]
fn the_model_reads_a_definition_in_units() {
    let org = acme();
    let teams: Vec<(Id, Option<Id>)> = org.teams().map(|t| (t.id, t.department)).collect();
    assert_eq!(
        teams,
        [(id("app"), Some(id("dev"))), (id("ios"), Some(id("app")))]
    );
    let groups: Vec<Id> = org.groups().map(|g| g.id).collect();
    assert_eq!(groups, [id("product"), id("dev")]);
    assert_eq!(org.units().count(), 4);
    assert_eq!(
        org.head(),
        Some(&id("alice")),
        "no head named: the root's person"
    );
    let reports = |name: &str| org.reports_to(&id(name));
    assert_eq!(
        reports("dave"),
        Some(id("app-lead")),
        "ios has no head: app's, a team above it"
    );
    assert_eq!(reports("carol"), Some(id("app-lead")));
    assert_eq!(
        reports("app-lead"),
        Some(id("dev-lead")),
        "an agent reports up too"
    );
    assert_eq!(reports("dev-lead"), Some(id("alice")));
    assert_eq!(reports("bob"), Some(id("dev-lead")));
    assert_eq!(reports("alice"), None);
    let mut named = org.clone();
    named.head = Some(id("dev-lead"));
    assert_eq!(named.head(), Some(&id("dev-lead")));
    assert_eq!(
        named.reports_to(&id("alice")),
        Some(id("dev-lead")),
        "the root is no longer the head"
    );
}
