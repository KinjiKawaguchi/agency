//! An org id names one org on a node. Joining through a link to another org with an id this node
//! already has must change nothing here.
mod common;

use agency_core::invites::{invite, join};
use agency_core::{orgs, ErrorKind};
use agency_proto::Role;
use common::{alice, id, memory_runtime, node, Running};

fn snapshot(running: &Running) -> (String, Option<String>, Vec<String>) {
    let doc = running.node.store().load_org(&id("acme")).unwrap();
    let config = running.node.config();
    let trusted = config
        .trusted
        .iter()
        .map(|one| format!("{} {} {:?}", one.org, one.root_fingerprint, one.root_node))
        .collect();
    (
        doc.digest(),
        config.trusted_fingerprint(&id("acme")).map(str::to_owned),
        trusted,
    )
}

#[tokio::test]
async fn a_link_to_another_org_with_the_same_id_changes_nothing() {
    let (alice, _) = alice().await;
    let mallory = node("mallory", memory_runtime().0).await;
    orgs::create(&mallory.node, id("acme"), "Acme".into()).unwrap();
    let link = invite(&mallory.node, &id("acme"), None, Role::Member).unwrap();

    let before = snapshot(&alice);
    let refused = join(&alice.node, &link, |_| true).await.unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict, "{refused:?}");
    assert!(refused.message.contains("another root key"), "{refused:?}");
    assert_eq!(
        snapshot(&alice),
        before,
        "the definition and the trust stay"
    );
    let doc = alice.node.store().load_org(&id("acme")).unwrap();
    assert!(
        orgs::is_root(&alice.node, &doc),
        "alice's node is still the root"
    );
    assert!(doc.body.member(&id("mallory")).is_none());
}

#[tokio::test]
async fn a_node_that_only_trusts_an_id_joins_only_that_org() {
    let (alice, _) = alice().await;
    let fingerprint = alice.node.key().public().fingerprint();

    let host = node("carol", memory_runtime().0).await;
    host.node
        .update_config(|config| config.trust(id("acme"), fingerprint.clone()))
        .unwrap();
    let mallory = node("mallory", memory_runtime().0).await;
    orgs::create(&mallory.node, id("acme"), "Acme".into()).unwrap();
    let other = invite(&mallory.node, &id("acme"), None, Role::Member).unwrap();
    let refused = join(&host.node, &other, |_| true).await.unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert!(host.node.store().load_org(&id("acme")).is_none());
    assert_eq!(
        host.node.config().trusted_fingerprint(&id("acme")),
        Some(fingerprint.as_str())
    );

    let genuine = invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    let joined = join(&host.node, &genuine, |_| true).await.unwrap();
    assert!(
        joined.body.member(&id("carol")).is_some(),
        "the same org, by its root key"
    );
}

#[tokio::test]
async fn an_org_is_not_created_with_an_id_this_node_already_trusts() {
    let host = node("carol", memory_runtime().0).await;
    host.node
        .update_config(|config| config.trust(id("acme"), "0".repeat(32)))
        .unwrap();
    let refused = orgs::create(&host.node, id("acme"), "Acme".into()).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert_eq!(
        host.node.config().trusted_fingerprint(&id("acme")),
        Some("0".repeat(32).as_str())
    );
}
