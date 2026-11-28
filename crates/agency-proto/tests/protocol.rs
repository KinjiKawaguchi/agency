//! Tests of the protocol's deciding rules: the chain of org definition versions (one writer, the root key), envelope shape and expiry.
//! The non-ASCII sample values ("開発", "ア") are on purpose: the test vectors pin UTF-8 in canonical JSON.
use agency_proto::org::{verify_genesis, verify_successor, Succession};
use agency_proto::{clock, AccountKey, Kind, Person, Role, Signed};
mod common;
use common::*;

#[test]
fn org_chain_accepts_only_the_next_version_signed_by_the_root() {
    let root = AccountKey::generate();
    let v1 = genesis(&root);
    assert_eq!(verify_genesis(&v1), Ok(()));
    let bob = AccountKey::generate();
    let v2 = successor(&v1, &root, |body| {
        body.people.push(
            Person::new(id("bob"), "Bob", id("bob"), bob.public())
                .with_department(Some(id("dev")))
                .with_role(Role::Answerer),
        )
    });
    assert_eq!(verify_successor(&v1, &v2), Ok(Succession::Root));
    assert!(v2.body.member(&id("bob")).is_some());
    // Refused: signed by a key that is not the root, a skipped version, a wrong previous fingerprint, and the same version twice.
    assert!(verify_successor(&v1, &successor(&v1, &bob, |_| ())).is_err());
    assert!(verify_successor(&v1, &successor(&v2, &root, |_| ())).is_err());
    assert!(verify_successor(
        &v1,
        &successor(&v1, &root, |body| body.prev = Some("0".repeat(64)))
    )
    .is_err());
    assert!(verify_successor(&v2, &v2).is_err());
}

#[test]
fn revoked_keys_are_not_members() {
    let root = AccountKey::generate();
    let v1 = genesis(&root);
    let v2 = successor(&v1, &root, |body| body.revoked.push(root.public()));
    assert!(v1.body.member(&id("alice")).is_some());
    assert!(v2.body.member(&id("alice")).is_none());
}

#[test]
fn org_shape_is_checked() {
    let root = AccountKey::generate();
    let v1 = genesis(&root);
    let broken = successor(&v1, &root, |body| body.teams[0].node = "file:///etc".into());
    assert!(verify_successor(&v1, &broken).is_err());
    let dangling = successor(&v1, &root, |body| {
        body.people[0].department = Some(id("nowhere"))
    });
    assert!(verify_successor(&v1, &dangling).is_err());
}

#[test]
fn envelope_shape_and_freshness() {
    let now = clock::parse("2026-11-24T12:00:00Z").unwrap();
    assert_eq!(envelope(AT).validate(), Ok(()));
    assert!(envelope("2026-11-18T12:00:00Z").is_fresh(now));
    assert!(
        !envelope("2026-11-16T12:00:00Z").is_fresh(now),
        "8 days old is expired"
    );
    assert!(
        !envelope("2026-11-24T12:10:00Z").is_fresh(now),
        "10 minutes ahead is beyond the allowed clock skew"
    );
    let mut empty = envelope(AT);
    empty.payload.choice = None;
    assert!(empty.validate().is_err());
    let mut bad_target = envelope(AT);
    bad_target.target = "../../etc".into();
    assert!(bad_target.validate().is_err());
}

/// docs/protocol.md, "Item references": a target is 1 to 120 bytes of ASCII letters, digits, and `._#-`.
#[test]
fn an_envelope_target_follows_the_documented_character_set_and_length() {
    let with_target = |target: &str| {
        let mut body = envelope(AT);
        body.target = target.into();
        body.validate()
    };
    for accepted in ["app#12", "ENG-123", "a.b_c-D#9", &"a".repeat(120)] {
        assert_eq!(with_target(accepted), Ok(()), "{accepted}");
    }
    for refused in [
        "",
        "owner/repo#12",
        "PROJ:12",
        "app 12",
        "caf\u{e9}#1",
        &"a".repeat(121),
    ] {
        assert!(with_target(refused).is_err(), "{refused}");
    }
}

#[test]
fn signature_is_bound_to_its_kind() {
    let key = AccountKey::generate();
    let signed = Signed::sign(Kind::Envelope, envelope(AT), id("alice"), &key).unwrap();
    assert!(signed.verify(Kind::Envelope, &key.public()));
    assert!(
        !signed.verify(Kind::Org, &key.public()),
        "an envelope signature cannot be used as an org definition signature"
    );
}
