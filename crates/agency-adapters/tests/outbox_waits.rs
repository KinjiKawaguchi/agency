//! What waits in the outbox, and for how long: an applied id kept while its envelope can still be sent, an
//! envelope that waited past its life, and the versions for a node the org no longer places a team on.
mod node_fixture;

use agency_core::decisions::{accept_envelope, Accepted, Decided};
use agency_core::devices::DeviceSigner;
use agency_core::orgs;
use agency_core::outbox::{deliver_due, pending, Outcome, Parcel, ParcelBody};
use agency_core::signed_decisions::decide_with_device;
use agency_core::ErrorKind;
use agency_proto::clock::Duration;
use agency_proto::org::TeamPlace;
use agency_proto::{EnvelopeId, EnvelopeKind, Payload};
use node_fixture::*;

#[tokio::test]
async fn an_applied_id_is_kept_while_its_envelope_can_still_be_sent_and_dropped_with_the_next_write_after(
) {
    let alice = alice();
    let (org, team) = (id("acme"), id("app"));
    let known = |byte: u8| {
        alice
            .node
            .store()
            .is_applied(&EnvelopeId::from_bytes([byte; 16]))
    };
    let applied = || alice.runtime.0.lock().unwrap().applied.len();
    let at = |plus: Duration| {
        let when = agency_proto::clock::parse(T0).unwrap() + plus;
        when.format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };

    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 1)).await,
        Ok(Accepted::Applied)
    );
    // Six days on: a copy within the envelope's life is still answered as a duplicate, and nothing is dropped.
    alice.clock.advance(Duration::days(6));
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 1)).await,
        Ok(Accepted::Duplicate)
    );
    let now = at(Duration::days(6));
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(&now, 2)).await,
        Ok(Accepted::Applied)
    );
    assert!(known(1) && known(2));

    // Seven days and two minutes: the first envelope has expired, but its id is kept for five minutes more
    // (the skew an envelope's time may be ahead by), and a write now does not drop it.
    alice
        .clock
        .set(&at(Duration::days(7) + Duration::minutes(2)));
    assert_eq!(
        accept_envelope(
            &alice.node,
            &org,
            &team,
            envelope(&alice.node.now_text(), 3)
        )
        .await,
        Ok(Accepted::Applied)
    );
    assert!(known(1), "within the window of 7 days and 5 minutes");

    // Past it, the next write drops the id, and a copy of the envelope is refused as expired: never applied again.
    alice
        .clock
        .set(&at(Duration::days(7) + Duration::minutes(6)));
    assert_eq!(
        accept_envelope(
            &alice.node,
            &org,
            &team,
            envelope(&alice.node.now_text(), 4)
        )
        .await,
        Ok(Accepted::Applied)
    );
    assert!(
        !known(1),
        "older than the window: dropped as a new id was written"
    );
    assert!(
        known(2) && known(3) && known(4),
        "what is within the window stays"
    );
    assert_eq!(
        accept_envelope(&alice.node, &org, &team, envelope(T0, 1))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::BadRequest
    );
    assert_eq!(applied(), 4, "each envelope was applied once");
}

// ── What the outbox stops retrying ──

/// alice has a team `far` at a node nobody listens at, and a decision on it that waits in the outbox.
async fn with_a_decision_waiting_for_far() -> Fixture {
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
    alice
}

fn envelopes(parcels: &[Parcel]) -> usize {
    let is_envelope = |one: &&Parcel| matches!(one.body, ParcelBody::Envelope(_));
    parcels.iter().filter(is_envelope).count()
}

fn versions(parcels: &[Parcel]) -> Vec<u64> {
    let mut found: Vec<u64> = parcels
        .iter()
        .filter_map(|one| match &one.body {
            ParcelBody::Org(doc) => Some(doc.body.version),
            _ => None,
        })
        .collect();
    found.sort();
    found
}

#[tokio::test]
async fn an_envelope_that_waited_past_its_life_is_refused_as_expired_and_not_a_second_before() {
    let alice = with_a_decision_waiting_for_far().await;
    assert_eq!(envelopes(&pending(&alice.node)), 1);

    // Exactly seven days after the envelope was made it can still be accepted: it is tried again.
    alice.clock.advance(Duration::days(7));
    let outcomes = deliver_due(&alice.node).await.unwrap();
    assert!(
        outcomes
            .iter()
            .any(|one| matches!(one, Outcome::Unreachable(_))),
        "{outcomes:?}"
    );
    assert_eq!(
        envelopes(&pending(&alice.node)),
        1,
        "not expired at seven days"
    );

    alice.clock.advance(Duration::seconds(1));
    let outcomes = deliver_due(&alice.node).await.unwrap();
    assert!(
        outcomes.contains(&Outcome::Refused("expired before it was delivered".into())),
        "{outcomes:?}"
    );
    assert_eq!(envelopes(&pending(&alice.node)), 0, "no longer retried");
    let refused = alice.node.store().refused_parcels();
    assert_eq!(envelopes(&refused), 1);
    let reason = refused.iter().find_map(|one| one.last_error.clone());
    assert_eq!(reason.as_deref(), Some("expired before it was delivered"));

    let log = agency_core::client::decision_log(&alice.node, None);
    let state = &log.entries[0].state;
    assert!(
        matches!(state, agency_proto::client::DecisionState::Refused { message } if message == "expired before it was delivered"),
        "{state:?}"
    );
}

#[tokio::test]
async fn of_the_versions_waiting_for_a_node_the_org_no_longer_places_a_team_on_only_the_newest_stays(
) {
    let alice = alice();
    let (org, far) = (id("acme"), id("far"));
    orgs::add_team(
        &alice.node,
        &org,
        TeamPlace::new(far.clone(), "Far", NOWHERE),
    )
    .unwrap();
    let placed = versions(&pending(&alice.node));
    assert_eq!(
        placed.len(),
        1,
        "the version that placed far, for its node: {placed:?}"
    );

    // The org removes far: the version that does is queued for far's node as well, to tell it so.
    orgs::amend(&alice.node, &org, |body| {
        body.teams_mut().retain(|team| team.id != far);
        Ok(())
    })
    .unwrap();
    assert_eq!(versions(&pending(&alice.node)).len(), 2);
    deliver_due(&alice.node).await.unwrap();
    let removal = versions(&pending(&alice.node));
    assert_eq!(
        removal.len(),
        1,
        "the older version is dropped: {removal:?}"
    );
    assert!(
        removal[0] > placed[0],
        "and the one that removes far stays: it tells the node it was removed"
    );
    assert!(
        alice.node.store().refused_parcels().is_empty(),
        "a dropped version is not a refusal"
    );

    // The root signs more versions, none queued for far's node. The removal is still the newest that waits
    // for it, so it stays however many come after it: far's node must still hear that it was removed.
    for label in ["Acme Inc", "Acme Ltd"] {
        orgs::amend(&alice.node, &org, |body| {
            body.label = label.into();
            Ok(())
        })
        .unwrap();
        alice.clock.advance(Duration::minutes(10));
        deliver_due(&alice.node).await.unwrap();
        assert_eq!(versions(&pending(&alice.node)), removal, "after {label}");
    }
}
#[tokio::test]
async fn versions_for_a_node_that_a_newer_version_still_places_a_team_on_are_all_kept() {
    let alice = alice();
    let org = id("acme");
    orgs::add_team(&alice.node, &org, TeamPlace::new(id("far"), "Far", NOWHERE)).unwrap();
    orgs::amend(&alice.node, &org, |body| {
        body.label = "Acme Inc".into();
        Ok(())
    })
    .unwrap();
    deliver_due(&alice.node).await.unwrap();
    assert_eq!(
        versions(&pending(&alice.node)).len(),
        2,
        "both wait for the node, in order"
    );
}

#[tokio::test]
async fn an_origin_written_another_way_is_still_the_node_a_newer_version_places_a_team_on() {
    let alice = alice();
    let org = id("acme");
    // The team is placed at one spelling of its node's origin, and the versions are queued under it.
    let (written, other) = ("http://LOCALHOST:1", "http://localhost:1");
    orgs::add_team(&alice.node, &org, TeamPlace::new(id("far"), "Far", written)).unwrap();
    orgs::amend(&alice.node, &org, |body| {
        body.label = "Acme Inc".into();
        Ok(())
    })
    .unwrap();
    // The latest version spells the same node's origin the other way: it is still the node it places the team on.
    orgs::amend(&alice.node, &org, |body| {
        let team = body
            .teams_mut()
            .iter_mut()
            .find(|team| team.id == id("far"));
        team.unwrap().node = other.into();
        Ok(())
    })
    .unwrap();
    deliver_due(&alice.node).await.unwrap();
    let under_the_first: Vec<Parcel> = pending(&alice.node)
        .into_iter()
        .filter(|parcel| parcel.to == written)
        .collect();
    assert_eq!(
        versions(&under_the_first).len(),
        2,
        "not dropped as if nothing placed a team there"
    );
}
