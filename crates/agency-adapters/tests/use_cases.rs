//! Use cases against real files and a manual clock: expiry and timing (the envelope's 7 days, the invite's
//! 7 days, the signed read's 5 minutes, the join request's 10 minutes, the outbox backoff).
mod node_fixture;

use agency_core::decisions::{accept_envelope, Accepted, Decided};
use agency_core::devices::DeviceSigner;
use agency_core::invites::{accept_join, invite, open_invite};
use agency_core::orgs;
use agency_core::outbox::{deliver_due, pending, Parcel, ParcelBody};
use agency_core::read_auth::{sign_read, verify_read, ReadPolicy};
use agency_core::signed_decisions::decide_with_device;
use agency_core::ErrorKind;
use agency_proto::clock::Duration;
use agency_proto::invite::Invite;
use agency_proto::org::TeamPlace;
use agency_proto::{AccountKey, EnvelopeKind, JoinRequest, Kind, Payload, Role, Signed};
use node_fixture::*;

#[tokio::test]
async fn envelopes_are_accepted_for_seven_days_and_up_to_five_minutes_ahead() {
    let alice = alice();
    let (org, team) = (id("acme"), id("app"));
    alice.clock.set("2026-12-01T12:00:00Z");
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 1)).await,
        Ok(Accepted::Applied),
        "exactly 7 days old"
    );
    alice.clock.advance(Duration::seconds(1));
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 2))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::BadRequest,
        "7 days and 1 second old"
    );

    alice.clock.set(T0);
    assert!(
        accept_envelope(
            &alice.node,
            &org,
            &team,
            envelope("2026-11-24T12:05:00Z", 3)
        )
        .await
        .is_ok(),
        "5 minutes ahead"
    );
    assert_eq!(
        accept_envelope(
            &alice.node,
            &org,
            &team,
            envelope("2026-11-24T12:05:01Z", 4)
        )
        .await
        .unwrap_err()
        .kind,
        ErrorKind::BadRequest,
        "more than 5 minutes ahead"
    );
    assert_eq!(alice.runtime.0.lock().unwrap().applied.len(), 2);
}

#[tokio::test]
async fn invites_open_for_seven_days() {
    let alice = alice();
    let code = Invite::parse(&invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
        .unwrap()
        .code;
    alice
        .clock
        .advance(Duration::days(7) - Duration::seconds(1));
    assert!(open_invite(&alice.node, &id("acme"), &code).is_some());
    alice.clock.advance(Duration::seconds(1));
    assert!(open_invite(&alice.node, &id("acme"), &code).is_none());
}

#[tokio::test]
async fn join_requests_must_be_signed_within_ten_minutes() {
    let alice = alice();
    let code = Invite::parse(&invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
        .unwrap()
        .code;
    let bob = AccountKey::generate();
    let request = |at: &str| {
        let body = JoinRequest {
            org: id("acme"),
            code: code.clone(),
            account: id("bob"),
            label: "Bob".into(),
            public_key: bob.public(),
            at: at.into(),
        };
        Signed::sign(Kind::Join, body, id("bob"), &bob).unwrap()
    };
    alice.clock.set("2026-11-24T12:10:01Z");
    assert_eq!(
        accept_join(&alice.node, &id("acme"), request(T0))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::BadRequest
    );
    alice.clock.set("2026-11-24T12:10:00Z");
    let answer = accept_join(&alice.node, &id("acme"), request(T0))
        .await
        .unwrap();
    let agency_proto::root_change::RootAnswer::Signed(doc) = answer else {
        panic!("held: {answer:?}")
    };
    assert!(doc.body.member(&id("bob")).is_some());
}

#[test]
fn signed_reads_are_accepted_within_five_minutes_either_way() {
    let alice = alice();
    let definition = alice.node.store().load_org(&id("acme")).unwrap().body;
    let path = "/fed/v1/teams/acme/app/view";
    let signed_at = |offset: i64| {
        sign_read(
            alice.node.key(),
            &id("alice"),
            "GET",
            path,
            alice.node.now() + Duration::seconds(offset),
        )
    };
    let now = alice.node.now();
    for offset in [-300, 300] {
        assert!(
            verify_read(
                Some(&definition),
                true,
                &signed_at(offset),
                "GET",
                path,
                now,
                &ReadPolicy::open(None)
            )
            .is_ok(),
            "{offset}"
        );
    }
    for offset in [-301, 301] {
        assert_eq!(
            verify_read(
                Some(&definition),
                true,
                &signed_at(offset),
                "GET",
                path,
                now,
                &ReadPolicy::open(None)
            )
            .unwrap_err()
            .kind,
            ErrorKind::NotFound,
            "{offset}: refused like an unknown org"
        );
    }
    assert!(
        verify_read(
            Some(&definition),
            true,
            &signed_at(0),
            "GET",
            "/fed/v1/teams/acme/other/view",
            now,
            &ReadPolicy::open(None)
        )
        .is_err(),
        "the signature covers the path"
    );
}

#[tokio::test]
async fn the_outbox_retries_only_when_the_backoff_is_due() {
    let alice = alice();
    let place = TeamPlace::new(id("far"), "Far", NOWHERE);
    orgs::add_team(&alice.node, &id("acme"), place).unwrap();
    let (key, key_id) = (laptop(), laptop().public().device_id());
    let device = DeviceSigner {
        id: &key_id,
        key: &key,
    };
    let outcome = decide_with_device(
        &alice.node,
        device,
        &id("acme"),
        &id("far"),
        EnvelopeKind::Answer,
        "r-9".into(),
        Payload {
            text: "yes".into(),
            choice: None,
        },
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Decided::Queued(_)), "{outcome:?}");
    let envelope = |parcels: &[Parcel]| {
        parcels
            .iter()
            .find(|one| matches!(one.body, ParcelBody::Envelope(_)))
            .cloned()
    };
    assert_eq!(envelope(&pending(&alice.node)).unwrap().attempts, 1);

    alice.clock.advance(Duration::seconds(14));
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(
        envelope(&pending(&alice.node)).unwrap().attempts,
        1,
        "not due 14 seconds after the first attempt"
    );
    alice.clock.advance(Duration::seconds(1));
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(
        envelope(&pending(&alice.node)).unwrap().attempts,
        2,
        "due after 15 seconds"
    );
    alice.clock.advance(Duration::seconds(29));
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(
        envelope(&pending(&alice.node)).unwrap().attempts,
        2,
        "the second wait is 30 seconds"
    );
}
