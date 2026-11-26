//! An org's root key is its own, apart from the node's account key, and can be moved to a new key.
mod common;

use agency_core::departments;
use agency_core::orgs;
use agency_core::views::refresh;
use agency_core::ErrorKind;
use agency_proto::org::{Department, Root, SignedOrg};
use agency_proto::{Kind, OrgDefinition, Person, Role, Signed};
use common::{alice, id, joined_bob, memory_runtime, node, Running};

fn current(running: &Running) -> SignedOrg {
    running.node.store().load_org(&id("acme")).unwrap()
}

fn department(raw: &str) -> Department {
    Department {
        id: id(raw),
        label: raw.into(),
        parent: None,
        head: None,
    }
}

#[tokio::test]
async fn a_new_org_gets_a_root_key_of_its_own() {
    let (alice, _) = alice().await;
    let doc = current(&alice);
    let root = &doc.body.root.public_key;
    assert_ne!(root, &alice.node.key().public(), "not the account key");
    assert!(orgs::is_root(&alice.node, &doc));
    assert!(doc.verify(Kind::Org, root));
    let member = doc.body.member(&id("alice")).unwrap();
    assert_eq!(
        member.public_key,
        alice.node.key().public(),
        "the member keeps the account key"
    );
    assert_eq!(
        alice.node.config().trusted_fingerprint(&id("acme")),
        Some(root.fingerprint().as_str())
    );
}

/// An org made the old way, with the account key as its root.
fn org_with_the_account_key_as_root(running: &Running) {
    let key = running.node.key();
    let me = Person {
        id: id("alice"),
        label: "Alice".into(),
        account: id("alice"),
        public_key: key.public(),
        department: None,
        role: Role::Member,
        devices: vec![],
    };
    let body = OrgDefinition {
        kpis: vec![],
        org: id("legacy"),
        label: "Legacy".into(),
        version: 1,
        prev: None,
        root: Root {
            account: id("alice"),
            public_key: key.public(),
        },
        departments: vec![],
        people: vec![me],
        teams: vec![],
        revoked: vec![],
        at: running.node.now_text(),
    };
    let doc = Signed::sign(Kind::Org, body, id("alice"), key).unwrap();
    running.node.store().save_org(&doc).unwrap();
    running
        .node
        .update_config(|config| config.trust(id("legacy"), key.public().fingerprint()))
        .unwrap();
}

#[tokio::test]
async fn an_org_rooted_in_the_account_key_moves_to_a_key_of_its_own() {
    let running = node("alice", memory_runtime().0).await;
    org_with_the_account_key_as_root(&running);
    let legacy = id("legacy");
    let before = running.node.store().load_org(&legacy).unwrap();
    assert!(
        orgs::is_root(&running.node, &before),
        "the account key is its root"
    );

    let rotated = orgs::rotate_root(&running.node, &legacy).unwrap();
    let new = rotated.body.root.public_key.clone();
    assert_ne!(new, running.node.key().public());
    assert!(
        rotated.verify(Kind::Org, &running.node.key().public()),
        "the version naming the new root is signed by the old one"
    );
    assert!(agency_proto::org::verify_successor(&before, &rotated).is_ok());
    assert_eq!(
        running.node.config().trusted_fingerprint(&legacy),
        Some(new.fingerprint().as_str())
    );

    let next = departments::add(&running.node, &legacy, department("ops")).unwrap();
    assert!(
        next.verify(Kind::Org, &new),
        "later versions are signed by the new root"
    );
    assert!(agency_proto::org::verify_successor(&rotated, &next).is_ok());

    let mut taken = OrgDefinition::successor_of(&next, running.node.now_text());
    taken.label = "Taken".into();
    let by_old: SignedOrg =
        Signed::sign(Kind::Org, taken, id("alice"), running.node.key()).unwrap();
    assert!(
        agency_proto::org::verify_successor(&next, &by_old).is_err(),
        "the account key, the old root, signs no later version"
    );
}

#[tokio::test]
async fn members_follow_a_rotation_and_the_old_root_signs_no_more() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let old = current(&alice).body.root.public_key;

    let rotated = orgs::rotate_root(&alice.node, &id("acme")).unwrap();
    let new = rotated.body.root.public_key.clone();
    use agency_core::ports::RootKeys;
    assert!(
        !alice.root_keys.holds(&id("acme"), &old),
        "the old key is forgotten"
    );
    assert!(alice.root_keys.holds(&id("acme"), &new));

    departments::add(&alice.node, &id("acme"), department("ops")).unwrap();
    refresh(&bob.node).await.unwrap();
    let seen = current(&bob);
    assert_eq!(
        seen.body.version,
        current(&alice).body.version,
        "bob caught up through the rotation"
    );
    assert_eq!(seen.body.root.public_key, new);
    assert_eq!(
        bob.node.config().trusted_fingerprint(&id("acme")),
        Some(new.fingerprint().as_str()),
        "bob's trust moved to the new root"
    );
}

#[tokio::test]
async fn a_refused_signature_writes_nothing() {
    let (alice, _) = alice().await;
    let before = current(&alice);
    alice.root_keys.refuse(true);
    let refused = departments::add(&alice.node, &id("acme"), department("ops")).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Forbidden);
    assert!(refused.message.contains("did not sign"), "{refused:?}");
    let rotation = orgs::rotate_root(&alice.node, &id("acme")).unwrap_err();
    assert_eq!(rotation.kind, ErrorKind::Forbidden);
    assert_eq!(
        current(&alice).digest(),
        before.digest(),
        "no version was written"
    );
    assert!(
        orgs::is_root(&alice.node, &current(&alice)),
        "a refused rotation keeps the current root"
    );
    alice.root_keys.refuse(false);
    departments::add(&alice.node, &id("acme"), department("ops")).unwrap();
}
