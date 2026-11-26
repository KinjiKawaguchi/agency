//! A recovery key may move an org's root only after every node held the start for 72 hours by its own
//! clock, and the current root cancels a start by signing any version.
mod common;

use agency_core::orgs;
use agency_core::ErrorKind;
use agency_proto::clock::Duration;
use agency_proto::org::{PendingRoot, Root, SignedOrg, RECOVERY_WAIT_SECONDS};
use agency_proto::{AccountKey, Kind, OrgDefinition, Person, Role, Signed};
use common::{id, memory_runtime, node, Running};

struct Keys {
    root: AccountKey,
    recovery: AccountKey,
    new_root: AccountKey,
}

fn keys() -> Keys {
    Keys {
        root: AccountKey::from_seed([0x21; 32]),
        recovery: AccountKey::from_seed([0x22; 32]),
        new_root: AccountKey::from_seed([0x23; 32]),
    }
}

fn current(running: &Running) -> SignedOrg {
    running.node.store().load_org(&id("acme")).unwrap()
}

fn sign(body: OrgDefinition, key: &AccountKey) -> SignedOrg {
    Signed::sign(Kind::Org, body, id("alice"), key).unwrap()
}

/// Carol's node keeps version 1 of `acme`, whose root has a recovery key.
async fn carol_with_a_recovery_key(keys: &Keys) -> Running {
    let carol = node("carol", memory_runtime().0).await;
    let body = OrgDefinition {
        kpis: vec![],
        org: id("acme"),
        label: "Acme".into(),
        version: 1,
        prev: None,
        root: Root::new(id("alice"), keys.root.public()).with_recovery(keys.recovery.public()),
        departments: vec![],
        people: vec![Person {
            id: id("alice"),
            label: "Alice".into(),
            account: id("alice"),
            public_key: AccountKey::from_seed([0x24; 32]).public(),
            department: None,
            role: Role::Member,
            devices: vec![],
        }],
        teams: vec![],
        revoked: vec![],
        at: carol.node.now_text(),
    };
    let doc = sign(body, &keys.root);
    carol.node.store().save_org(&doc).unwrap();
    carol
        .node
        .update_config(|config| config.trust(id("acme"), keys.root.public().fingerprint()))
        .unwrap();
    carol
}

fn start(previous: &SignedOrg, keys: &Keys) -> SignedOrg {
    let mut body = OrgDefinition::successor_of(previous, previous.body.at.clone());
    body.root.pending = Some(PendingRoot {
        public_key: Some(keys.new_root.public()),
        recovery: None,
    });
    sign(body, &keys.recovery)
}

fn finish(start: &SignedOrg, keys: &Keys) -> SignedOrg {
    let mut body = OrgDefinition::successor_of(start, start.body.at.clone());
    body.root.public_key = keys.new_root.public();
    body.root.pending = None;
    sign(body, &keys.recovery)
}

fn trusted(running: &Running) -> String {
    let config = running.node.config();
    config.trusted_fingerprint(&id("acme")).unwrap().to_owned()
}

#[tokio::test]
async fn a_recovery_takes_effect_only_after_this_node_held_the_start_for_72_hours() {
    let keys = keys();
    let carol = carol_with_a_recovery_key(&keys).await;
    let started = start(&current(&carol), &keys);
    orgs::receive(&carol.node, started.clone()).await.unwrap();
    assert_eq!(
        current(&carol).body.root.public_key,
        keys.root.public(),
        "a start moves nothing"
    );

    let finished = finish(&started, &keys);
    carol
        .clock
        .advance(Duration::seconds(RECOVERY_WAIT_SECONDS - 1));
    let early = orgs::receive(&carol.node, finished.clone())
        .await
        .unwrap_err();
    assert_eq!(early.kind, ErrorKind::Conflict, "{early:?}");
    assert!(early.message.contains("takes effect"), "{early:?}");
    assert_eq!(current(&carol).digest(), started.digest());
    assert_eq!(trusted(&carol), keys.root.public().fingerprint());

    carol.clock.advance(Duration::seconds(1));
    orgs::receive(&carol.node, finished).await.unwrap();
    assert_eq!(current(&carol).body.root.public_key, keys.new_root.public());
    assert_eq!(
        trusted(&carol),
        keys.new_root.public().fingerprint(),
        "the trust follows the recovery"
    );
}

#[tokio::test]
async fn a_start_dated_long_ago_still_waits_72_hours_from_when_this_node_took_it() {
    let keys = keys();
    let carol = carol_with_a_recovery_key(&keys).await;
    carol.clock.advance(Duration::days(30));
    let started = start(&current(&carol), &keys);
    assert!(
        started.body.at < carol.node.now_text(),
        "the start carries the old version's time"
    );
    orgs::receive(&carol.node, started.clone()).await.unwrap();
    let refused = orgs::receive(&carol.node, finish(&started, &keys))
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
}

#[tokio::test]
async fn a_finish_sent_with_its_start_waits_too() {
    let keys = keys();
    let carol = carol_with_a_recovery_key(&keys).await;
    let started = start(&current(&carol), &keys);
    let finished = finish(&started, &keys);
    assert!(orgs::receive(&carol.node, finished.clone()).await.is_err());
    orgs::receive(&carol.node, started).await.unwrap();
    assert!(orgs::receive(&carol.node, finished).await.is_err());
}

#[tokio::test]
async fn any_version_the_root_signs_cancels_a_waiting_recovery() {
    let keys = keys();
    let carol = carol_with_a_recovery_key(&keys).await;
    let started = start(&current(&carol), &keys);
    orgs::receive(&carol.node, started.clone()).await.unwrap();
    let mut body = OrgDefinition::successor_of(&started, started.body.at.clone());
    body.root.pending = None;
    body.label = "Acme Inc".into();
    let cancel = sign(body, &keys.root);
    orgs::receive(&carol.node, cancel).await.unwrap();
    assert_eq!(current(&carol).body.root.pending, None);

    carol
        .clock
        .advance(Duration::seconds(RECOVERY_WAIT_SECONDS));
    let late = orgs::receive(&carol.node, finish(&started, &keys))
        .await
        .unwrap_err();
    assert_eq!(late.kind, ErrorKind::Conflict, "the cancel took the number");
    assert_eq!(current(&carol).body.root.public_key, keys.root.public());
}

#[tokio::test]
async fn the_root_node_cancels_by_signing_anything() {
    let (alice, _) = common::alice().await;
    let org = id("acme");
    let recovery = AccountKey::from_seed([0x25; 32]);
    let doc = current(&alice);
    // The root adds a recovery key, then the recovery key starts a recovery that alice's node takes.
    let mut body = OrgDefinition::successor_of(&doc, alice.node.now_text());
    body.root.recovery = Some(recovery.public());
    let root_key = doc.body.root.public_key.clone();
    let with_recovery = Signed::sign_with(Kind::Org, body, id("alice"), |text| {
        use agency_core::ports::RootKeys;
        alice
            .root_keys
            .sign(&org, &root_key, text, "add the recovery key")
    })
    .unwrap()
    .unwrap();
    orgs::receive(&alice.node, with_recovery.clone())
        .await
        .unwrap();
    let mut body = OrgDefinition::successor_of(&with_recovery, alice.node.now_text());
    body.root.pending = Some(PendingRoot {
        public_key: Some(AccountKey::from_seed([0x26; 32]).public()),
        recovery: None,
    });
    let started = sign(body, &recovery);
    orgs::receive(&alice.node, started).await.unwrap();
    assert!(current(&alice).body.root.pending.is_some());

    agency_core::departments::add(
        &alice.node,
        &org,
        agency_proto::org::Department {
            id: id("ops"),
            label: "Ops".into(),
            parent: None,
            head: None,
        },
    )
    .unwrap();
    let after = current(&alice);
    assert_eq!(after.body.root.pending, None, "signing anything cancels");
    assert!(after.verify(Kind::Org, &root_key));
}
