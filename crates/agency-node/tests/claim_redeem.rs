//! A browser redeems a seat's claim link with the device key it made: the node checks the link, the address and
//! the key before it keeps anything, spends the code once, and answers so that nothing leaks. The routes come
//! later; this is the use case.
mod common;

use agency_core::claims::{self, ClaimState};
use agency_core::{orgs, seats, ErrorKind};
use agency_proto::claim::{ClaimProgress, ClaimStatus};
use agency_proto::clock::Duration;
use agency_proto::seat::Seat;
use agency_proto::AccountKey;
use common::{alice, id, Running};

const CAROL: &str = "carol@example.com";

fn seat(raw: &str, label: &str) -> Seat {
    Seat {
        id: id(raw),
        label: label.into(),
        department: None,
        ask_about: vec![],
        relays: vec![id("alice")],
    }
}

/// Alice's org with the seats `carol` and `dan`, and a link for carol's.
async fn with_a_link() -> (Running, String) {
    let (alice, _) = alice().await;
    seats::add_seat(&alice.node, &id("acme"), seat("carol", "Carol (finance)")).unwrap();
    seats::add_seat(&alice.node, &id("acme"), seat("dan", "Dan (legal)")).unwrap();
    let link = claims::seat_invite(&alice.node, &id("acme"), id("carol"), CAROL).unwrap();
    let code = link.link.split("code=").nth(1).unwrap().to_owned();
    (alice, code)
}

fn key(seed: u8) -> AccountKey {
    AccountKey::from_seed([seed; 32])
}

async fn redeem(
    alice: &Running,
    code: &str,
    email: &str,
    key: &AccountKey,
) -> agency_core::Result<ClaimStatus> {
    claims::redeem(
        &alice.node,
        code,
        email,
        &key.public().encoded(),
        "Firefox on Windows",
    )
    .await
}

fn record_of(alice: &Running, code: &str) -> claims::ClaimRecord {
    let digest = agency_proto::invite::code_digest(code);
    alice
        .node
        .store()
        .load_claim(&digest)
        .expect("the link's record")
}

#[tokio::test]
async fn a_redeem_keeps_the_key_spends_the_code_and_says_where_it_stands() {
    let (alice, code) = with_a_link().await;
    let phone = key(0x71);

    let status = redeem(&alice, &code, "Carol@Example.com", &phone)
        .await
        .unwrap();
    assert_eq!(
        status,
        ClaimStatus {
            state: ClaimProgress::Waiting,
            seat: "Carol (finance)".into(),
            org: "Acme".into(),
            pairing_code: phone.public().pairing_code(),
        }
    );
    match record_of(&alice, &code).state {
        ClaimState::Redeemed(kept) => {
            assert_eq!(kept.public_key, phone.public());
            assert_eq!(kept.label, "Firefox on Windows");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        claims::state(&alice.node, CAROL).unwrap(),
        status,
        "asked again by the address"
    );
    let seats_left = alice
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .seats
        .len();
    assert_eq!(
        seats_left, 2,
        "the seat is still a seat until the owner confirms"
    );

    let again = redeem(&alice, &code, CAROL, &key(0x72)).await.unwrap_err();
    assert_eq!(
        again.kind,
        ErrorKind::NotFound,
        "a second redeem: the link is spent"
    );
}

#[tokio::test]
async fn a_code_that_is_none_unknown_or_spent_gets_the_same_answer() {
    let (alice, code) = with_a_link().await;
    redeem(&alice, &code, CAROL, &key(0x71)).await.unwrap();
    let mut answers = vec![];
    for tried in [
        "",
        "short",
        "not a code at all!!!!!!",
        &"a".repeat(22),
        &code,
    ] {
        let error = redeem(&alice, tried, CAROL, &key(0x73)).await.unwrap_err();
        assert_eq!(error.kind, ErrorKind::NotFound, "{tried:?}");
        answers.push(error.message);
    }
    assert!(
        answers.windows(2).all(|pair| pair[0] == pair[1]),
        "{answers:?}"
    );
}

#[tokio::test]
async fn an_expired_link_is_gone_and_another_address_is_forbidden_without_spending_it() {
    let (alice, code) = with_a_link().await;
    let wrong = redeem(&alice, &code, "mallory@example.com", &key(0x71))
        .await
        .unwrap_err();
    assert_eq!(wrong.kind, ErrorKind::Forbidden);
    assert!(
        !wrong.message.contains("mallory") && !wrong.message.contains("carol"),
        "{wrong:?}"
    );
    assert_eq!(
        record_of(&alice, &code).state,
        ClaimState::Open,
        "another address spends nothing"
    );

    alice.clock.advance(Duration::days(8));
    let late = redeem(&alice, &code, CAROL, &key(0x71)).await.unwrap_err();
    assert_eq!(late.kind, ErrorKind::Gone, "{late:?}");
    assert_eq!(record_of(&alice, &code).state, ClaimState::Open);
}

#[tokio::test]
async fn a_seat_that_has_gone_and_keys_or_labels_that_cannot_be_used_refuse_and_spend_nothing() {
    let (alice, code) = with_a_link().await;
    let own_key = alice.node.key().public().encoded();
    let bad_keys = [
        ("not a key", ErrorKind::BadRequest),
        ("", ErrorKind::BadRequest),
        (own_key.as_str(), ErrorKind::Conflict),
    ];
    for (text, kind) in bad_keys {
        let error = claims::redeem(&alice.node, &code, CAROL, text, "Phone")
            .await
            .unwrap_err();
        assert_eq!(error.kind, kind, "{text:?}");
        assert!(!error.message.contains(&own_key), "{error:?}");
    }
    let long = "x".repeat(81);
    for label in ["", long.as_str(), "ph\u{202e}one", "line\nbreak"] {
        let phone = key(0x71).public().encoded();
        let error = claims::redeem(&alice.node, &code, CAROL, &phone, label)
            .await
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::BadRequest, "{label:?}");
    }
    assert_eq!(
        record_of(&alice, &code).state,
        ClaimState::Open,
        "no refusal spent the link"
    );

    seats::remove_seat(&alice.node, &id("acme"), &id("carol")).unwrap();
    let gone = redeem(&alice, &code, CAROL, &key(0x71)).await.unwrap_err();
    assert_eq!(gone.kind, ErrorKind::Conflict, "{gone:?}");
    assert_eq!(record_of(&alice, &code).state, ClaimState::Open);
}

#[tokio::test]
async fn a_key_a_device_of_the_org_or_another_claim_holds_is_refused() {
    let (alice, code) = with_a_link().await;
    // A key of a device the org already has.
    let laptop = key(0x74);
    let device = agency_proto::org::Device {
        id: laptop.public().device_id(),
        label: "Laptop".into(),
        public_key: laptop.public(),
        added_at: agency_proto::Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.people[0].devices.push(device);
        Ok(())
    })
    .unwrap();
    let error = redeem(&alice, &code, CAROL, &laptop).await.unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::Conflict,
        "a device of the org: {error:?}"
    );

    // A key another claim already holds.
    let dan_link =
        claims::seat_invite(&alice.node, &id("acme"), id("dan"), "dan@example.com").unwrap();
    let dan_code = dan_link.link.split("code=").nth(1).unwrap().to_owned();
    let shared = key(0x75);
    redeem(&alice, &dan_code, "dan@example.com", &shared)
        .await
        .unwrap();
    let error = redeem(&alice, &code, CAROL, &shared).await.unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::Conflict,
        "a key another claim holds: {error:?}"
    );
    assert_eq!(record_of(&alice, &code).state, ClaimState::Open);
}

#[tokio::test]
async fn a_claim_asked_about_by_an_address_that_redeemed_nothing_is_not_found() {
    let (alice, code) = with_a_link().await;
    assert_eq!(
        claims::state(&alice.node, CAROL).unwrap_err().kind,
        ErrorKind::NotFound,
        "open, not redeemed"
    );
    redeem(&alice, &code, CAROL, &key(0x71)).await.unwrap();
    assert_eq!(
        claims::state(&alice.node, "dan@example.com")
            .unwrap_err()
            .kind,
        ErrorKind::NotFound,
        "another address"
    );
}
