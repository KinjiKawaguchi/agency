//! The checks that decide whether a signed document is accepted: an envelope signed by a device of its account
//! for this team, a join request signed by the key it adds, and a first copy of an org signed by its trusted root.
mod node_fixture;

use agency_core::decisions::accept_envelope;
use agency_core::invites::{accept_join, invite, open_invite};
use agency_core::orgs::receive;
use agency_core::ErrorKind;
use agency_proto::invite::Invite;
use agency_proto::org::{Root, SignedOrg};
use agency_proto::{AccountKey, JoinRequest, Kind, OrgDefinition, Person, Role, Signed};
use node_fixture::*;

#[tokio::test]
async fn envelopes_must_be_signed_by_a_device_of_their_account_for_this_team() {
    let alice = alice();
    let (org, team) = (id("acme"), id("app"));
    let mut wrong_signer = envelope(T0, 1);
    wrong_signer.signer = id("bob");
    let error = accept_envelope(&alice.node, &org, &team, wrong_signer)
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "the signer must be one of the account's devices; refused like an unknown team"
    );
    let by_account = Signed::sign(
        Kind::Envelope,
        envelope(T0, 1).body,
        id("alice"),
        alice.node.key(),
    )
    .unwrap();
    let error = accept_envelope(&alice.node, &org, &team, by_account)
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "the account key, which the node holds, never signs a decision"
    );

    let error = accept_envelope(&alice.node, &org, &id("other"), envelope(T0, 2))
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::BadRequest,
        "an envelope for acme/app sent to acme/other"
    );
    assert!(alice.runtime.0.lock().unwrap().applied.is_empty());
}

#[tokio::test]
async fn join_requests_must_be_signed_by_the_key_they_add() {
    let alice = alice();
    let code = Invite::parse(&invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
        .unwrap()
        .code;
    let (bob, mallory) = (AccountKey::generate(), AccountKey::generate());
    let body = JoinRequest {
        org: id("acme"),
        code: code.clone(),
        account: id("bob"),
        label: "Bob".into(),
        public_key: bob.public(),
        at: T0.into(),
    };
    let forged = Signed::sign(Kind::Join, body, id("bob"), &mallory).unwrap();
    let error = accept_join(&alice.node, &id("acme"), forged)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Unauthorized);
    assert!(
        open_invite(&alice.node, &id("acme"), &code).is_some(),
        "the invite stays unused"
    );
}

/// A first copy of acme, signed by `signer` but naming `root` as its root key.
fn first_copy(root: &AccountKey, signer: &AccountKey) -> SignedOrg {
    let alice = Person::new(id("alice"), "Alice", id("alice"), root.public());
    let body = OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        Root::new(id("alice"), root.public()),
        T0.into(),
    )
    .with_people(vec![alice]);
    Signed::sign(Kind::Org, body, id("alice"), signer).unwrap()
}

#[tokio::test]
async fn a_first_copy_must_be_signed_by_the_trusted_root_key() {
    let carol = bare("carol");
    let (root, mallory) = (AccountKey::generate(), AccountKey::generate());
    carol
        .node
        .update_config(|config| config.trust(id("acme"), root.public().fingerprint()))
        .unwrap();

    let error = receive(&carol.node, first_copy(&mallory, &mallory))
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "a different root key than the trusted fingerprint; refused like an untrusted org"
    );
    let error = receive(&carol.node, first_copy(&root, &mallory))
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::NotFound,
        "the trusted root key, but signed by another key"
    );
    assert!(carol.node.store().load_org(&id("acme")).is_none());

    receive(&carol.node, first_copy(&root, &root))
        .await
        .unwrap();
    assert!(carol.node.store().load_org(&id("acme")).is_some());
}
