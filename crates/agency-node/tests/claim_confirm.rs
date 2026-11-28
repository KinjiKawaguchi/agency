//! The owner compares the pairing code a person's screen shows with the one the node holds, and confirms: the seat
//! becomes a member the node hosts with that device, and only once the root has signed is the claim confirmed. A
//! claim can also be refused. Alice's node is the root; the person's browser redeemed in core.
mod common;

use agency_core::claims::{self, ClaimState};
use agency_core::{guests, seats, ErrorKind};
use agency_proto::claim::ClaimProgress;
use agency_proto::seat::Seat;
use agency_proto::AccountKey;
use common::{alice, id, Running};

const CAROL: &str = "carol@example.com";

fn seat(raw: &str, label: &str) -> Seat {
    Seat {
        id: id(raw),
        label: label.into(),
        department: None,
        ask_about: vec!["invoices".into()],
        relays: vec![id("alice")],
    }
}

fn phone() -> AccountKey {
    AccountKey::from_seed([0x71; 32])
}

/// Alice's org with carol's seat, her link, and a redeem by her browser with `phone()`.
async fn with_a_waiting_claim() -> (Running, String) {
    let (alice, _) = alice().await;
    seats::add_seat(&alice.node, &id("acme"), seat("carol", "Carol (finance)")).unwrap();
    let link = claims::seat_invite(&alice.node, &id("acme"), id("carol"), CAROL).unwrap();
    let code = link.link.split("code=").nth(1).unwrap().to_owned();
    claims::redeem(
        &alice.node,
        &code,
        CAROL,
        &phone().public().encoded(),
        "Firefox on Windows",
    )
    .await
    .unwrap();
    (alice, code)
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

fn state_of(alice: &Running, code: &str) -> ClaimState {
    let digest = agency_proto::invite::code_digest(code);
    alice.node.store().load_claim(&digest).unwrap().state
}

#[tokio::test]
async fn a_waiting_claim_is_listed_with_what_the_owner_compares_and_no_address() {
    let (alice, _) = with_a_waiting_claim().await;
    let waiting = claims::pending(&alice.node);
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].seat, id("carol"));
    assert_eq!(waiting[0].seat_label, "Carol (finance)");
    assert_eq!(waiting[0].device_label, "Firefox on Windows");
    assert_eq!(waiting[0].pairing_code, phone().public().pairing_code());
    assert!(!format!("{waiting:?}").contains("carol@"), "{waiting:?}");
}

#[tokio::test]
async fn confirming_with_the_right_pairing_code_makes_the_member_the_guest_and_the_claim_in() {
    let (alice, code) = with_a_waiting_claim().await;
    let before = version(&alice);
    // The code as the person reads it out: capitals, dashes or none.
    let typed = phone()
        .public()
        .pairing_code()
        .to_uppercase()
        .replace('-', " ");

    let doc = claims::confirm(&alice.node, &id("acme"), &id("carol"), &typed)
        .await
        .unwrap();

    assert_eq!(doc.body.version, before + 1);
    let carol = doc
        .body
        .member(&id("carol"))
        .expect("the seat's id is the member's");
    assert!(carol.is_hosted() && carol.public_key.is_none());
    assert_eq!(carol.devices.len(), 1);
    assert_eq!(carol.devices[0].public_key, phone().public());
    assert_eq!(carol.devices[0].label, "Firefox on Windows");
    assert!(doc.body.seats.iter().all(|one| one.id != id("carol")));
    assert_eq!(
        guests::admit(&alice.node, CAROL).expect("the guest").member,
        id("carol")
    );
    assert!(matches!(state_of(&alice, &code), ClaimState::Confirmed(_)));
    let status = claims::state(&alice.node, CAROL).unwrap();
    assert_eq!(status.state, ClaimProgress::In);
    assert_eq!(status.seat, "Carol (finance)");
    assert!(claims::pending(&alice.node).is_empty());
    assert!(
        !claims::is_claimant(&alice.node, CAROL),
        "now a guest, no claimant"
    );

    let again = claims::confirm(&alice.node, &id("acme"), &id("carol"), &typed)
        .await
        .unwrap();
    assert_eq!(
        again.body.version, doc.body.version,
        "a second confirm signs nothing"
    );
}

#[tokio::test]
async fn a_wrong_pairing_code_confirms_nothing() {
    let (alice, code) = with_a_waiting_claim().await;
    let before = version(&alice);
    for typed in ["", "0000-0000-0000", "not a code"] {
        let error = claims::confirm(&alice.node, &id("acme"), &id("carol"), typed)
            .await
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Conflict, "{typed:?}: {error:?}");
        assert!(
            !error.message.contains(&phone().public().pairing_code()),
            "{error:?}"
        );
    }
    assert_eq!(version(&alice), before, "nothing was signed");
    assert!(matches!(state_of(&alice, &code), ClaimState::Redeemed(_)));
    assert!(guests::admit(&alice.node, CAROL).is_none());
}

#[tokio::test]
async fn a_claim_is_confirmed_only_once_the_root_has_signed() {
    let (alice, code) = with_a_waiting_claim().await;
    let before = version(&alice);
    alice.root_keys.refuse(true);

    let error = claims::confirm(
        &alice.node,
        &id("acme"),
        &id("carol"),
        &phone().public().pairing_code(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Forbidden, "{error:?}");
    assert_eq!(version(&alice), before);
    assert!(
        matches!(state_of(&alice, &code), ClaimState::Redeemed(_)),
        "still waiting"
    );
    assert!(
        alice.node.store().guest(CAROL).is_none(),
        "no guest without a signed version"
    );

    alice.root_keys.refuse(false);
    claims::confirm(
        &alice.node,
        &id("acme"),
        &id("carol"),
        &phone().public().pairing_code(),
    )
    .await
    .unwrap();
    assert!(matches!(state_of(&alice, &code), ClaimState::Confirmed(_)));
}

#[tokio::test]
async fn a_seat_that_went_or_a_key_the_org_got_meanwhile_is_refused_and_the_claim_waits() {
    let (alice, code) = with_a_waiting_claim().await;
    // The org gets a device with the very key that waits.
    let device = agency_proto::org::Device {
        id: phone().public().device_id(),
        label: "Laptop".into(),
        public_key: phone().public(),
        added_at: agency_proto::Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    agency_core::orgs::amend(&alice.node, &id("acme"), |body| {
        body.people[0].devices.push(device);
        Ok(())
    })
    .unwrap();
    let before = version(&alice);
    let taken = claims::confirm(
        &alice.node,
        &id("acme"),
        &id("carol"),
        &phone().public().pairing_code(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        taken.kind,
        ErrorKind::BadRequest,
        "a key the org has now: {taken:?}"
    );
    assert_eq!(version(&alice), before);
    assert!(matches!(state_of(&alice, &code), ClaimState::Redeemed(_)));

    seats::remove_seat(&alice.node, &id("acme"), &id("carol")).unwrap();
    let gone = claims::confirm(
        &alice.node,
        &id("acme"),
        &id("carol"),
        &phone().public().pairing_code(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        gone.kind,
        ErrorKind::NotFound,
        "a seat that is gone: {gone:?}"
    );
}

#[tokio::test]
async fn a_member_hosted_by_an_earlier_run_that_did_not_record_the_claim_is_confirmed_without_signing_again(
) {
    let (alice, code) = with_a_waiting_claim().await;
    // The node signed the member and then stopped before it marked the claim.
    let device = agency_proto::org::Device {
        id: phone().public().device_id(),
        label: "Firefox on Windows".into(),
        public_key: phone().public(),
        added_at: agency_proto::Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    seats::host_seat(&alice.node, &id("acme"), &id("carol"), device, CAROL).unwrap();
    let before = version(&alice);

    claims::confirm(
        &alice.node,
        &id("acme"),
        &id("carol"),
        &phone().public().pairing_code(),
    )
    .await
    .unwrap();

    assert_eq!(version(&alice), before, "no second version");
    assert!(matches!(state_of(&alice, &code), ClaimState::Confirmed(_)));
}

#[tokio::test]
async fn a_refused_claim_is_dropped_and_the_link_stays_spent() {
    let (alice, code) = with_a_waiting_claim().await;

    claims::reject(&alice.node, &id("acme"), &id("carol"))
        .await
        .unwrap();

    assert!(claims::pending(&alice.node).is_empty());
    assert_eq!(
        claims::state(&alice.node, CAROL).unwrap_err().kind,
        ErrorKind::NotFound
    );
    assert!(!claims::is_claimant(&alice.node, CAROL));
    let again = claims::redeem(
        &alice.node,
        &code,
        CAROL,
        &phone().public().encoded(),
        "Firefox",
    )
    .await
    .unwrap_err();
    assert_eq!(
        again.kind,
        ErrorKind::NotFound,
        "the link is spent: {again:?}"
    );
    let confirm = claims::confirm(
        &alice.node,
        &id("acme"),
        &id("carol"),
        &phone().public().pairing_code(),
    )
    .await
    .unwrap_err();
    assert_eq!(confirm.kind, ErrorKind::NotFound, "nothing waits any more");
    assert_eq!(
        claims::reject(&alice.node, &id("acme"), &id("carol"))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    );
    assert!(
        alice
            .node
            .store()
            .load_org(&id("acme"))
            .unwrap()
            .body
            .seats
            .iter()
            .any(|one| one.id == id("carol")),
        "the seat is still a seat"
    );
}

#[tokio::test]
async fn each_seat_is_confirmed_with_its_own_pairing_code_and_no_other() {
    let (alice, _) = with_a_waiting_claim().await;
    seats::add_seat(&alice.node, &id("acme"), seat("dan", "Dan (legal)")).unwrap();
    let link = claims::seat_invite(&alice.node, &id("acme"), id("dan"), "dan@example.com").unwrap();
    let dan_code = link.link.split("code=").nth(1).unwrap().to_owned();
    let tablet = AccountKey::from_seed([0x72; 32]);
    claims::redeem(
        &alice.node,
        &dan_code,
        "dan@example.com",
        &tablet.public().encoded(),
        "Safari",
    )
    .await
    .unwrap();
    let before = version(&alice);

    let mixed = claims::confirm(
        &alice.node,
        &id("acme"),
        &id("dan"),
        &phone().public().pairing_code(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        mixed.kind,
        ErrorKind::Conflict,
        "carol's code for dan's seat: {mixed:?}"
    );
    assert_eq!(version(&alice), before);

    claims::confirm(
        &alice.node,
        &id("acme"),
        &id("dan"),
        &tablet.public().pairing_code(),
    )
    .await
    .unwrap();
    let doc = alice.node.store().load_org(&id("acme")).unwrap();
    assert!(doc.body.member(&id("dan")).is_some());
    assert!(
        doc.body.member(&id("carol")).is_none(),
        "carol's claim still waits"
    );
    assert_eq!(claims::pending(&alice.node).len(), 1);
}
