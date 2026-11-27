//! An org's change history in words: every version the node keeps, oldest first, with the key that signed it and
//! every change it made, none left out; a version the node does not keep is listed as such, never skipped.
mod common;

use agency_core::audit::org_history;
use agency_core::{orgs, seats};
use agency_proto::clock::Duration;
use agency_proto::seat::Seat;
use common::{alice, device, id, joined_bob, Running};

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
        "change member bob",
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
