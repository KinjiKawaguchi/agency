//! An org's change history in words: every version the node keeps, oldest first, with the key that signed it and
//! every change it made, none left out; a version the node does not keep is listed as such, never skipped.
mod common;

use agency_core::audit::{org_history, SignatureCheck};
use agency_core::{orgs, seats};
use agency_proto::clock::Duration;
use agency_proto::org::{Root, SignedOrg};
use agency_proto::seat::Seat;
use agency_proto::{AccountKey, Kind, OrgDefinition, Person, Signed};
use common::{alice, device, id, joined_bob, memory_runtime, node, Running};

fn changes_of(alice: &Running) -> Vec<(u64, String)> {
    org_history(&alice.node, &id("acme"), None)
        .unwrap()
        .into_iter()
        .map(|entry| (entry.version, entry.changes.join("; ")))
        .collect()
}

#[tokio::test]
async fn every_version_is_listed_with_its_signer_and_every_change() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    device(&bob).await;
    let seat = Seat {
        id: id("dan"),
        label: "Dan (tax)".into(),
        department: None,
        ask_about: vec![],
        relays: vec![],
    };
    seats::add_seat(&alice.node, &id("acme"), seat).unwrap();
    seats::merge_seat(&alice.node, &id("acme"), &id("dan"), &id("bob")).unwrap();
    orgs::move_team(
        &alice.node,
        &id("acme"),
        &id("app"),
        "http://127.0.0.1:9".into(),
    )
    .unwrap();

    let history = org_history(&alice.node, &id("acme"), None).unwrap();
    let current = alice.node.store().load_org(&id("acme")).unwrap();
    assert_eq!(
        history.len() as u64,
        current.body.version,
        "one entry for each version"
    );
    let root = format!("root key {}", current.body.root.public_key.fingerprint());
    for entry in &history {
        assert!(entry.is_kept);
        assert_eq!(entry.signed_by.as_deref(), Some(root.as_str()), "{entry:?}");
    }
    let all: Vec<String> = changes_of(&alice)
        .into_iter()
        .map(|(_, words)| words)
        .collect();
    let text = all.join("\n");
    for words in [
        "create the org Acme",
        "add team app",
        "add member bob",
        "add device",
        "add seat dan",
        "bob takes seat dan",
        "remove seat dan",
        "change team app",
    ] {
        assert!(text.contains(words), "{words}:\n{text}");
    }
}

#[tokio::test]
async fn since_keeps_the_later_versions_and_a_gap_is_listed() {
    let (alice, _) = alice().await;
    // Less than the 10 minutes a join may be off the root node's clock.
    alice.clock.advance(Duration::minutes(2));
    let later = alice.node.now();
    joined_bob(&alice).await;

    let since = org_history(&alice.node, &id("acme"), Some(later)).unwrap();
    assert_eq!(since.len(), 1, "{since:?}");
    assert!(
        since[0].changes.join(" ").contains("add member bob"),
        "{since:?}"
    );

    let gone = alice
        .data_dir()
        .join("orgs")
        .join("acme.history")
        .join(format!("{:06}.json", 2));
    std::fs::remove_file(gone).unwrap();
    let history = org_history(&alice.node, &id("acme"), None).unwrap();
    assert!(!history[1].is_kept, "{history:?}");
    assert!(history[2].is_kept);
    assert_eq!(
        history[2].signature,
        Some(SignatureCheck::NotChecked),
        "the root may have moved in the missing version: {history:?}"
    );
    assert_eq!(history[2].signed_by, None);
    assert_eq!(history[2].compared_with, Some(1));
    assert_eq!(history[2].follows, None);
    assert!(
        history[2].changes.join(" ").contains("add member bob"),
        "compared with the version before it that is kept: {history:?}"
    );
}

#[tokio::test]
async fn the_json_of_an_entry_has_its_fields() {
    let (alice, _) = alice().await;
    let history = org_history(&alice.node, &id("acme"), None).unwrap();
    let value = serde_json::to_value(&history[0]).unwrap();
    assert_eq!(value["version"], 1);
    assert!(value["at"].is_string());
    assert!(value["signedBy"].as_str().unwrap().starts_with("root key "));
    assert_eq!(value["isKept"], true);
    assert!(value["changes"][0]
        .as_str()
        .unwrap()
        .contains("create the org"));
}

#[tokio::test]
async fn a_version_with_many_changes_lists_every_one() {
    let (alice, _) = alice().await;
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.seats.extend((0..6).map(|n| Seat {
            id: id(&format!("s{n}")),
            label: format!("Seat {n}"),
            department: None,
            ask_about: vec![],
            relays: vec![],
        }));
        Ok(())
    })
    .unwrap();
    let history = org_history(&alice.node, &id("acme"), None).unwrap();
    let last = history.last().unwrap();
    assert_eq!(last.changes.len(), 6, "{last:?}");
    for n in 0..6 {
        assert!(
            last.changes
                .iter()
                .any(|one| one.contains(&format!("add seat s{n}"))),
            "s{n}: {last:?}"
        );
    }
}

#[tokio::test]
async fn a_rotation_is_signed_by_the_old_root_and_the_next_version_by_the_new_one() {
    let (alice, _) = alice().await;
    let old = alice
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .root
        .public_key;
    let rotated = orgs::rotate_root(&alice.node, &id("acme")).unwrap();
    let new = rotated.body.root.public_key.clone();
    assert_ne!(old, new);
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.label = "Acme, Inc.".into();
        Ok(())
    })
    .unwrap();

    let history = org_history(&alice.node, &id("acme"), None).unwrap();
    let n = history.len();
    let by = |entry: &agency_core::audit::HistoryEntry| entry.signed_by.clone().unwrap();
    assert_eq!(
        by(&history[n - 2]),
        format!("root key {}", old.fingerprint()),
        "{history:?}"
    );
    assert_eq!(
        by(&history[n - 1]),
        format!("root key {}", new.fingerprint()),
        "{history:?}"
    );
    assert!(history.iter().all(|entry| entry.follows == Some(true)));
}

/// A node that keeps version 1 of `acme` with a root key and a recovery key, and a version 2 that `sign` makes from
/// it, with `edit` applied to its body first.
fn org_with_recovery(
    running: &Running,
    edit: impl FnOnce(&mut OrgDefinition),
    sign: impl FnOnce(OrgDefinition, &Keys) -> SignedOrg,
) -> Keys {
    let keys = Keys {
        root: AccountKey::from_seed([0x31; 32]),
        recovery: AccountKey::from_seed([0x32; 32]),
    };
    let body = OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        Root::new(id("alice"), keys.root.public()).with_recovery(keys.recovery.public()),
        running.node.now_text(),
    )
    .with_people(vec![Person::new(
        id("alice"),
        "Alice",
        id("alice"),
        AccountKey::from_seed([0x33; 32]).public(),
    )]);
    let first = Signed::sign(Kind::Org, body, id("alice"), &keys.root).unwrap();
    running.node.store().save_org(&first).unwrap();
    let mut next = OrgDefinition::successor_of(&first, first.body.at.clone());
    next.label = "Acme again".into();
    edit(&mut next);
    let second = sign(next, &keys);
    running.node.store().save_org(&second).unwrap();
    keys
}

struct Keys {
    root: AccountKey,
    recovery: AccountKey,
}

#[tokio::test]
async fn a_version_the_recovery_key_signed_says_so() {
    let carol = node("carol", memory_runtime().0).await;
    let keys = org_with_recovery(
        &carol,
        |_| {},
        |body, keys| Signed::sign(Kind::Org, body, id("alice"), &keys.recovery).unwrap(),
    );
    let history = org_history(&carol.node, &id("acme"), None).unwrap();
    assert_eq!(history[1].signature, Some(SignatureCheck::Valid));
    assert_eq!(
        history[1].signed_by,
        Some(format!(
            "recovery key {}",
            keys.recovery.public().fingerprint()
        ))
    );
}

#[tokio::test]
async fn a_version_changed_after_it_was_signed_has_no_signer() {
    let carol = node("carol", memory_runtime().0).await;
    org_with_recovery(
        &carol,
        |_| {},
        |body, keys| {
            let mut signed = Signed::sign(Kind::Org, body, id("alice"), &keys.root).unwrap();
            signed.body.label = "Changed afterwards".into();
            signed
        },
    );
    let history = org_history(&carol.node, &id("acme"), None).unwrap();
    assert_eq!(
        history[1].signature,
        Some(SignatureCheck::Invalid),
        "{history:?}"
    );
    assert_eq!(history[1].signed_by, None);
}

#[tokio::test]
async fn a_version_whose_prev_is_not_the_version_before_is_a_fork() {
    let carol = node("carol", memory_runtime().0).await;
    org_with_recovery(
        &carol,
        |body| body.prev = Some("0".repeat(64)),
        |body, keys| Signed::sign(Kind::Org, body, id("alice"), &keys.root).unwrap(),
    );
    let history = org_history(&carol.node, &id("acme"), None).unwrap();
    assert_eq!(history[1].signature, Some(SignatureCheck::Valid));
    assert_eq!(history[1].follows, Some(false), "{history:?}");
    assert_eq!(history[0].follows, Some(true));
}
