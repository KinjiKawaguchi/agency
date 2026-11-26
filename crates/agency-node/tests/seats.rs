//! An org's seats and external parties through the use cases the CLI calls: each is the next version of the
//! definition, written by the root and read by the other nodes of the org. Names here are made up.
mod common;

use agency_core::peer_about::PeerAbout;
use agency_core::seats::{self, ExternalChange, SeatChange};
use agency_core::views::refresh;
use agency_core::{departments, orgs, ErrorKind};
use agency_proto::about::About;
use agency_proto::clock::Duration;
use agency_proto::org::{Department, SignedOrg, TeamPlace};
use agency_proto::seat::{External, Seat};
use agency_proto::{Id, Kind};
use common::{alice, id, joined_bob, Running};
use std::collections::BTreeMap;

fn acme() -> Id {
    id("acme")
}

fn current(running: &Running) -> SignedOrg {
    running.node.store().load_org(&acme()).unwrap()
}

fn department(raw: &str) -> Department {
    Department {
        id: id(raw),
        label: raw.to_uppercase(),
        parent: None,
        head: None,
    }
}

fn carol() -> Seat {
    Seat {
        id: id("carol"),
        label: "Carol (finance)".into(),
        department: Some(id("ops")),
        ask_about: vec!["invoices".into(), "budget approval".into()],
        relays: vec![id("alice")],
    }
}

fn erin() -> External {
    External {
        id: id("erin"),
        label: "Erin (outside counsel)".into(),
        ask_about: vec!["legal checks".into()],
        relays: vec![id("alice")],
    }
}

/// Alice's org with the department `ops`, and bob as a second member.
async fn org() -> (Running, Running) {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    departments::add(&alice.node, &acme(), department("ops")).unwrap();
    (alice, bob)
}

#[tokio::test]
async fn each_seat_and_external_party_is_one_signed_version_that_a_member_node_takes() {
    let (alice, bob) = org().await;
    let before = current(&alice).body.version;
    let doc = seats::add_seat(&alice.node, &acme(), carol()).unwrap();
    assert_eq!(doc.body.version, before + 1);
    assert!(
        doc.verify(Kind::Org, &doc.body.root.public_key),
        "signed by the root"
    );
    let doc = seats::add_external(&alice.node, &acme(), erin()).unwrap();
    assert_eq!(
        (doc.body.seats, doc.body.externals),
        (vec![carol()], vec![erin()])
    );

    refresh(&bob.node).await.unwrap();
    let seen = current(&bob);
    assert_eq!(
        seen.digest(),
        current(&alice).digest(),
        "bob took both versions, and reads them"
    );
    assert_eq!(seen.body.seats, vec![carol()]);
}

#[tokio::test]
async fn a_change_alters_only_the_fields_it_gives() {
    let (alice, _) = org().await;
    seats::add_seat(&alice.node, &acme(), carol()).unwrap();
    let seat = |running: &Running| current(running).body.seats[0].clone();

    seats::change_seat(
        &alice.node,
        &acme(),
        &id("carol"),
        SeatChange {
            label: Some("Carol F.".into()),
            ..SeatChange::default()
        },
    )
    .unwrap();
    assert_eq!(
        seat(&alice),
        Seat {
            label: "Carol F.".into(),
            ..carol()
        }
    );
    let ask = SeatChange {
        ask_about: Some(vec!["payroll".into()]),
        ..SeatChange::default()
    };
    seats::change_seat(&alice.node, &acme(), &id("carol"), ask).unwrap();
    assert_eq!(seat(&alice).ask_about, ["payroll"]);
    assert_eq!(
        seat(&alice).relays,
        [id("alice")],
        "the members who answer for it stayed"
    );
    let no_department = SeatChange {
        department: Some(None),
        relays: Some(vec![]),
        ..SeatChange::default()
    };
    seats::change_seat(&alice.node, &acme(), &id("carol"), no_department).unwrap();
    let changed = seat(&alice);
    assert_eq!((changed.department, changed.relays), (None, vec![]));
    assert_eq!(changed.label, "Carol F.");

    seats::add_external(&alice.node, &acme(), erin()).unwrap();
    let change = ExternalChange {
        relays: Some(vec![id("alice"), id("bob")]),
        ..ExternalChange::default()
    };
    seats::change_external(&alice.node, &acme(), &id("erin"), change).unwrap();
    let external = current(&alice).body.externals[0].clone();
    assert_eq!(
        (external.relays, external.label, external.ask_about),
        (vec![id("alice"), id("bob")], erin().label, erin().ask_about)
    );
}

#[tokio::test]
async fn an_id_is_used_once_among_members_seats_and_external_parties() {
    let (alice, _) = org().await;
    seats::add_seat(&alice.node, &acme(), carol()).unwrap();
    let before = current(&alice).digest();
    let twice = seats::add_seat(&alice.node, &acme(), carol()).unwrap_err();
    assert_eq!(twice.kind, ErrorKind::Conflict);
    let as_member = seats::add_seat(
        &alice.node,
        &acme(),
        Seat {
            id: id("bob"),
            ..carol()
        },
    )
    .unwrap_err();
    assert_eq!(as_member.kind, ErrorKind::Conflict);
    assert!(as_member.message.contains("bob"), "{}", as_member.message);
    let as_seat = seats::add_external(
        &alice.node,
        &acme(),
        External {
            id: id("carol"),
            ..erin()
        },
    )
    .unwrap_err();
    assert_eq!(as_seat.kind, ErrorKind::Conflict);
    assert_eq!(current(&alice).digest(), before, "nothing was written");
}

#[tokio::test]
async fn the_definitions_own_rules_refuse_a_version_and_name_the_id() {
    let (alice, _) = org().await;
    let before = current(&alice).digest();
    let in_sales = seats::add_seat(
        &alice.node,
        &acme(),
        Seat {
            department: Some(id("sales")),
            ..carol()
        },
    )
    .unwrap_err();
    assert_eq!(in_sales.kind, ErrorKind::BadRequest);
    assert!(
        in_sales.message.contains("carol") && in_sales.message.contains("sales"),
        "{}",
        in_sales.message
    );
    let stranger = seats::add_seat(
        &alice.node,
        &acme(),
        Seat {
            relays: vec![id("zed")],
            ..carol()
        },
    )
    .unwrap_err();
    assert!(
        stranger.message.contains("carol") && stranger.message.contains("zed"),
        "{}",
        stranger.message
    );
    let alone = seats::add_external(
        &alice.node,
        &acme(),
        External {
            relays: vec![],
            ..erin()
        },
    )
    .unwrap_err();
    assert_eq!(alone.kind, ErrorKind::BadRequest);
    assert!(alone.message.contains("erin"), "{}", alone.message);
    assert_eq!(current(&alice).digest(), before, "nothing was written");
}

#[tokio::test]
async fn a_seat_or_an_external_party_can_be_removed_and_one_that_is_not_there_is_not_found() {
    let (alice, _) = org().await;
    seats::add_seat(&alice.node, &acme(), carol()).unwrap();
    seats::add_external(&alice.node, &acme(), erin()).unwrap();
    let doc = seats::remove_seat(&alice.node, &acme(), &id("carol")).unwrap();
    assert!(doc.body.seats.is_empty() && !doc.body.externals.is_empty());
    let doc = seats::remove_external(&alice.node, &acme(), &id("erin")).unwrap();
    assert!(doc.body.externals.is_empty());
    for result in [
        seats::remove_seat(&alice.node, &acme(), &id("carol")),
        seats::remove_external(&alice.node, &acme(), &id("erin")),
        seats::change_seat(&alice.node, &acme(), &id("carol"), SeatChange::default()),
        seats::change_external(&alice.node, &acme(), &id("erin"), ExternalChange::default()),
    ] {
        assert_eq!(result.unwrap_err().kind, ErrorKind::NotFound);
    }
}

#[tokio::test]
async fn a_department_that_still_holds_a_seat_is_not_removed() {
    let (alice, _) = org().await;
    seats::add_seat(&alice.node, &acme(), carol()).unwrap();
    let refused = departments::remove(&alice.node, &acme(), &id("ops")).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert!(
        refused.message.contains("seat carol"),
        "{}",
        refused.message
    );
    seats::remove_seat(&alice.node, &acme(), &id("carol")).unwrap();
    departments::remove(&alice.node, &acme(), &id("ops")).unwrap();
}

#[tokio::test]
async fn a_node_that_does_not_list_the_capability_holds_the_version_back() {
    let (alice, bob) = org().await;
    orgs::add_team(
        &alice.node,
        &acme(),
        TeamPlace::new(id("web"), "Web", bob.url()),
    )
    .unwrap();
    let read_at = agency_proto::clock::format(alice.node.now() - Duration::minutes(5));
    let mut about = About::of_release("0.3.0");
    about.capabilities.retain(|name| name != "orgSeats");
    let entry = PeerAbout {
        about: Some(about),
        read_at: Some(read_at.clone()),
        tried_at: read_at,
        ..PeerAbout::default()
    };
    alice
        .node
        .store()
        .save_peer_abouts(&acme(), &BTreeMap::from([(bob.url(), entry)]))
        .unwrap();

    let refused = seats::add_seat(&alice.node, &acme(), carol()).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert!(refused.message.contains("orgSeats"), "{}", refused.message);
    assert!(refused.message.contains(&bob.url()), "{}", refused.message);
    // An external party is another capability, which bob lists.
    seats::add_external(&alice.node, &acme(), erin()).unwrap();
}

#[tokio::test]
async fn a_new_label_or_ask_about_text_that_could_reorder_a_line_is_refused_for_seats_and_external_parties(
) {
    let (alice, _) = org().await;
    let version = current(&alice).body.version;
    let hidden = "invoices\u{202E}x";
    let refuse = |error: agency_core::Error| {
        assert_eq!(error.kind, ErrorKind::BadRequest, "{error:?}");
    };

    refuse(
        seats::add_seat(
            &alice.node,
            &acme(),
            Seat {
                label: "Carol\u{202E}x".into(),
                ..carol()
            },
        )
        .unwrap_err(),
    );
    refuse(
        seats::add_seat(
            &alice.node,
            &acme(),
            Seat {
                ask_about: vec!["budget approval".into(), hidden.into()],
                ..carol()
            },
        )
        .unwrap_err(),
    );
    refuse(
        seats::add_external(
            &alice.node,
            &acme(),
            External {
                label: "Erin\u{2066}x".into(),
                ..erin()
            },
        )
        .unwrap_err(),
    );
    refuse(
        seats::add_external(
            &alice.node,
            &acme(),
            External {
                ask_about: vec![hidden.into()],
                ..erin()
            },
        )
        .unwrap_err(),
    );
    assert_eq!(current(&alice).body.version, version, "nothing was signed");

    seats::add_seat(&alice.node, &acme(), carol()).unwrap();
    let change = SeatChange {
        ask_about: Some(vec![hidden.into()]),
        ..SeatChange::default()
    };
    refuse(seats::change_seat(&alice.node, &acme(), &id("carol"), change).unwrap_err());
}
