//! Ending a hosted member's access (`agency guest-remove`): one version takes the member out and revokes their keys,
//! what this node kept for them goes, and with `--to-seat` a seat stands in their place so that the requests
//! addressed to them stay addressed to someone.
mod common;

use agency_core::claims::seat_invite;
use agency_core::client::team_snapshot;
use agency_core::decisions::accept_envelope;
use agency_core::members::{remove_member, remove_member_to_seat};
use agency_core::{guests, seats, ErrorKind};
use agency_proto::client::GuestRef;
use agency_proto::org::SignedOrg;
use agency_proto::seat::Seat;
use agency_proto::{AccountKey, Envelope, EnvelopeId, EnvelopeKind, Id, Kind, Payload, Signed};
use common::guests::{call, with_guest, CAROL};
use common::{id, joined_bob, Running};
use reqwest::Method;
use serde_json::json;
use std::process::Command;

fn acme() -> Id {
    id("acme")
}

fn current(alice: &Running) -> SignedOrg {
    alice.node.store().load_org(&acme()).unwrap()
}

fn carol_ref() -> GuestRef {
    GuestRef {
        member: id("carol"),
        org: acme(),
    }
}

/// Alice's team addresses carol as `finance`; a team that does that is not changed by these tests unless one says so.
fn signed_by(alice: &Running, laptop: &AccountKey, target: &str) -> Signed<Envelope> {
    let body = Envelope {
        id: EnvelopeId::from_bytes([0x82; 16]),
        org: acme(),
        team: id("app"),
        kind: EnvelopeKind::Answer,
        target: target.into(),
        payload: Payload {
            text: "ok".into(),
            choice: None,
        },
        account: id("carol"),
        at: alice.node.now_text(),
    };
    Signed::sign(Kind::Envelope, body, laptop.public().device_id(), laptop).unwrap()
}

/// carol decides r-2 through the Client API, so that the decision log holds an entry of hers.
async fn carol_decides(alice: &Running, laptop: &AccountKey) {
    let envelope = signed_by(alice, laptop, "r-2");
    let (status, answer) = call(
        alice,
        Method::POST,
        "/api/orgs/acme/teams/app/envelopes",
        CAROL,
        Some(json!({ "envelope": envelope })),
    )
    .await;
    assert_eq!(status, 202, "{answer}");
    agency_core::outbox::deliver_due(&alice.node).await.unwrap();
    assert_eq!(alice.node.store().recent_decisions(10).len(), 1);
}

/// Forgets the teams' names for carol, which a removal without a seat is refused over.
fn unname_carol(alice: &Running) {
    alice
        .node
        .update_config(|config| config.teams.iter_mut().for_each(|team| team.people.clear()))
        .unwrap();
}

#[tokio::test]
async fn ending_a_guests_access_drops_what_the_node_kept_and_keeps_the_decision_log() {
    let (alice, laptop) = with_guest().await;
    carol_decides(&alice, &laptop).await;
    let token = json!({ "token": "ab12".repeat(16), "environment": "sandbox", "label": "iPhone" });
    let (status, answer) = call(&alice, Method::POST, "/api/push/tokens", CAROL, Some(token)).await;
    assert_eq!(status, 200, "{answer}");
    alice
        .node
        .store()
        .save_guest_notified(&carol_ref(), &["acme/app/r-2".to_owned()])
        .unwrap();
    unname_carol(&alice);
    let before = current(&alice).body.version;

    let doc = remove_member(&alice.node, &acme(), &id("carol")).unwrap();

    assert_eq!(doc.body.version, before + 1, "one version");
    assert!(doc.body.member(&id("carol")).is_none());
    assert!(doc.body.revoked.contains(&laptop.public()));
    assert!(guests::admit(&alice.node, CAROL).is_none(), "no sign-in");
    assert!(alice.node.store().guest(CAROL).is_none());
    assert!(alice.node.store().push_tokens().is_empty());
    assert!(
        alice.node.store().guest_notified(&carol_ref()).is_none(),
        "the list of requests told of is gone too"
    );
    assert_eq!(
        alice.node.store().recent_decisions(10).len(),
        1,
        "the decision log keeps what was decided"
    );
    let refused = accept_envelope(
        &alice.node,
        &acme(),
        &id("app"),
        signed_by(&alice, &laptop, "r-2"),
    )
    .await
    .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::NotFound, "{refused:?}");
}

#[tokio::test]
async fn a_member_turned_into_a_seat_keeps_the_requests_addressed_to_it_in_one_version() {
    let (alice, laptop) = with_guest().await;
    carol_decides(&alice, &laptop).await;
    let before = current(&alice).body;
    let carol = before.member(&id("carol")).unwrap().clone();

    let refused = remove_member(&alice.node, &acme(), &id("carol")).unwrap_err();
    assert_eq!(
        refused.kind,
        ErrorKind::Conflict,
        "the team still addresses carol: {refused:?}"
    );

    let doc = remove_member_to_seat(&alice.node, &acme(), &id("carol"), &[]).unwrap();

    assert_eq!(doc.body.version, before.version + 1, "a single version");
    assert!(doc.body.member(&id("carol")).is_none());
    assert!(doc.body.revoked.contains(&laptop.public()));
    let seat = doc
        .body
        .seats
        .iter()
        .find(|one| one.id == id("carol"))
        .unwrap();
    assert_eq!(
        (
            seat.label.as_str(),
            &seat.department,
            &seat.ask_about,
            &seat.relays
        ),
        (
            carol.label.as_str(),
            &carol.department,
            &carol.ask_about,
            &vec![]
        )
    );
    assert!(guests::admit(&alice.node, CAROL).is_none());
    assert_eq!(alice.node.store().recent_decisions(10).len(), 1);
    let snapshot = team_snapshot(&alice.node, &acme(), &id("app"))
        .await
        .unwrap();
    assert!(
        snapshot.mine.contains(&"r-2".to_owned()),
        "the request for `finance` is the owner's now, since the seat names no one: {:?}",
        snapshot.mine
    );
    assert_eq!(
        snapshot.relayed_for.get("r-2").map(String::as_str),
        Some("Carol"),
        "and it says whom it is for"
    );
}

#[tokio::test]
async fn the_members_named_to_answer_for_the_seat_get_the_requests_and_the_owner_does_not() {
    let (alice, _) = with_guest().await;
    let bob = joined_bob(&alice).await;
    drop(bob);

    let doc = remove_member_to_seat(&alice.node, &acme(), &id("carol"), &[id("bob")]).unwrap();

    let seat = doc
        .body
        .seats
        .iter()
        .find(|one| one.id == id("carol"))
        .unwrap();
    assert_eq!(seat.relays, [id("bob")]);
    let snapshot = team_snapshot(&alice.node, &acme(), &id("app"))
        .await
        .unwrap();
    assert!(
        !snapshot.mine.contains(&"r-2".to_owned()),
        "bob answers for the seat, not the owner: {:?}",
        snapshot.mine
    );
}

#[tokio::test]
async fn the_seats_a_member_took_come_back_and_a_pending_claim_of_one_is_dropped() {
    let (alice, _) = with_guest().await;
    let dan = Seat {
        id: id("dan"),
        label: "Dan (tax)".into(),
        department: None,
        ask_about: vec!["tax".into()],
        relays: vec![],
    };
    seats::add_seat(&alice.node, &acme(), dan).unwrap();
    seat_invite(&alice.node, &acme(), id("dan"), "dan@example.com").unwrap();
    let digest = alice
        .node
        .store()
        .claim_for_seat(&acme(), &id("dan"))
        .unwrap();
    let record = alice
        .data_dir()
        .join("claims")
        .join(format!("{digest}.json"));
    assert!(record.is_file());
    seats::merge_seat(&alice.node, &acme(), &id("dan"), &id("carol")).unwrap();
    alice
        .node
        .update_config(|config| {
            config.teams[0].people.insert("tax".into(), id("dan"));
        })
        .unwrap();

    let doc = remove_member_to_seat(&alice.node, &acme(), &id("carol"), &[]).unwrap();

    let ids: Vec<&str> = doc.body.seats.iter().map(|seat| seat.id.as_str()).collect();
    assert_eq!(
        ids,
        ["carol", "dan"],
        "the member's id and the seat it took"
    );
    let dan = doc
        .body
        .seats
        .iter()
        .find(|seat| seat.id == id("dan"))
        .unwrap();
    assert_eq!(
        dan.ask_about,
        ["tax"],
        "what to ask about came with the member"
    );
    assert!(
        alice
            .node
            .store()
            .claim_for_seat(&acme(), &id("dan"))
            .is_none()
            && !record.exists(),
        "a link made before cannot take the seat back"
    );
}

#[tokio::test]
async fn a_seat_cannot_be_answered_for_by_a_stranger_the_member_itself_or_leave_a_team_without_its_owner(
) {
    let (alice, _) = with_guest().await;
    joined_bob(&alice).await;
    let before = current(&alice).body.version;

    for relay in ["nobody", "carol"] {
        let error =
            remove_member_to_seat(&alice.node, &acme(), &id("carol"), &[id(relay)]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::BadRequest, "{relay}: {error:?}");
        assert!(
            error.message.contains("cannot answer for the seat"),
            "{error:?}"
        );
    }
    alice
        .node
        .update_config(|config| config.teams[0].owner = id("carol"))
        .unwrap();
    let error = remove_member_to_seat(&alice.node, &acme(), &id("carol"), &[]).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict, "a team's owner: {error:?}");
    assert!(error.message.contains("team app"), "{error:?}");

    assert_eq!(current(&alice).body.version, before, "nothing was signed");
}

fn agency(alice: &Running, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_agency"))
        .arg("--dir")
        .arg(alice.data_dir())
        .args(args)
        .output()
        .unwrap()
}

#[tokio::test]
async fn the_commands_list_the_hosted_members_and_decide_who_is_hosted_from_the_definition() {
    let (alice, _) = with_guest().await;

    let listed = agency(&alice, &["guests", "--org", "acme"]);
    let text = String::from_utf8_lossy(&listed.stdout);
    assert_eq!(text.trim(), "carol (1 devices)", "{listed:?}");
    assert!(!text.contains("example.com"), "no email");

    let not_hosted = agency(
        &alice,
        &["guest-remove", "--org", "acme", "--member", "alice"],
    );
    assert!(!not_hosted.status.success());
    assert!(String::from_utf8_lossy(&not_hosted.stderr).contains("not a hosted member"));
    let stray = agency(
        &alice,
        &[
            "guest-remove",
            "--org",
            "acme",
            "--member",
            "carol",
            "--answered-by",
            "bob",
        ],
    );
    assert!(!stray.status.success(), "--answered-by needs --to-seat");
    assert!(current(&alice).body.member(&id("carol")).is_some());

    // The sign-in record is already dropped, and the definition still says carol is hosted: the command goes on past
    // that check. It stops at the signing, since the root key of these tests is held in memory, not by the binary.
    alice.node.store().remove_guest(CAROL).unwrap();
    let ended = agency(
        &alice,
        &[
            "guest-remove",
            "--org",
            "acme",
            "--member",
            "carol",
            "--to-seat",
        ],
    );
    let said = String::from_utf8_lossy(&ended.stderr);
    assert!(!said.contains("not a hosted member"), "{said}");
    assert!(said.contains("does not hold the root key"), "{said}");
}
