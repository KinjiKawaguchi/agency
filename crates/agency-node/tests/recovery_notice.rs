//! The owner hears of a started recovery of an org they are the root of at once, on every device whatever
//! kinds it asked for, and sees it in `/api/me` until it is cancelled or finished; one command cancels it.
mod common;

use agency_core::client::me;
use agency_core::notice::NoticeKind;
use agency_core::push::notify_waiting;
use agency_core::{orgs, recovery, ErrorKind};
use agency_proto::org::PendingRoot;
use agency_proto::{AccountKey, Kind, OrgDefinition, Signed};
use common::{alice, body, id, Running, TOKEN};
use reqwest::Client;
use serde_json::json;

const TOKEN_HEX: &str = "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12";

/// An iPhone token that asked for requests only.
async fn register_for_requests_only(running: &Running) {
    let input = json!({ "token": TOKEN_HEX, "environment": "sandbox", "label": "iPhone", "events": ["request"] });
    let (status, answer) = body(
        Client::new()
            .post(format!("{}/api/push/tokens", running.url()))
            .header("x-agency-token", TOKEN)
            .json(&input)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
}

/// Alice's org gets a recovery key, then someone with its words starts a recovery that alice's node takes.
async fn started(alice: &Running) -> AccountKey {
    let words = AccountKey::from_seed([0x31; 32]);
    recovery::add(&alice.node, &id("acme"), words.public()).unwrap();
    let current = alice.node.store().load_org(&id("acme")).unwrap();
    let mut body = OrgDefinition::successor_of(&current, alice.node.now_text());
    body.root.pending = Some(PendingRoot {
        public_key: Some(AccountKey::from_seed([0x32; 32]).public()),
        recovery: None,
    });
    let start = Signed::sign(Kind::Org, body, id("alice"), &words).unwrap();
    orgs::receive(&alice.node, start).await.unwrap();
    words
}

fn recovery_notices(running: &Running) -> Vec<NoticeKind> {
    let state = running.notifier.0.lock().unwrap();
    state
        .sent
        .iter()
        .map(|(_, notice)| notice.kind.clone())
        .filter(|kind| matches!(kind, NoticeKind::Recovery { .. }))
        .collect()
}

#[tokio::test]
async fn the_owner_is_told_once_on_every_device_and_sees_it_until_it_is_cancelled() {
    let (alice, _) = alice().await;
    register_for_requests_only(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    started(&alice).await;

    notify_waiting(&alice.node).await.unwrap();
    let told = recovery_notices(&alice);
    assert_eq!(
        told.len(),
        1,
        "a token that asked for requests only: {told:?}"
    );
    let NoticeKind::Recovery {
        org,
        takes_effect_at,
    } = &told[0]
    else {
        unreachable!()
    };
    assert_eq!(org, &id("acme"));
    let shown = me(&alice.node);
    let recovery = shown.orgs[0].recovery.clone().expect("the desk shows it");
    assert_eq!(&recovery.takes_effect_at, takes_effect_at);
    assert!(recovery.takes_effect_at.is_some());
    assert!(recovery.new_root_key.is_some() && !recovery.replaces_recovery_key);

    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(recovery_notices(&alice).len(), 1, "told once");

    recovery::cancel(&alice.node, &id("acme")).unwrap();
    assert_eq!(me(&alice.node).orgs[0].recovery, None);
    let again = recovery::cancel(&alice.node, &id("acme")).unwrap_err();
    assert_eq!(again.kind, ErrorKind::Conflict, "nothing waits any more");
}

#[tokio::test]
async fn a_recovery_found_on_a_nodes_first_pass_is_told_too() {
    let (alice, _) = alice().await;
    register_for_requests_only(&alice).await;
    started(&alice).await;
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(recovery_notices(&alice).len(), 1);
}

#[tokio::test]
async fn with_the_words_the_owner_moves_the_root_to_a_new_key_after_the_wait() {
    let (alice, _) = alice().await;
    let org = id("acme");
    let words = AccountKey::from_seed([0x33; 32]);
    recovery::add(&alice.node, &org, words.public()).unwrap();
    let again = recovery::add(
        &alice.node,
        &org,
        AccountKey::from_seed([0x34; 32]).public(),
    )
    .unwrap_err();
    assert_eq!(
        again.kind,
        ErrorKind::Conflict,
        "the root never replaces it"
    );

    let wrong = AccountKey::from_seed([0x35; 32]);
    let refused = recovery::start(&alice.node, &org, &wrong, recovery::Recovering::RootKey)
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Forbidden);

    let start = recovery::start(&alice.node, &org, &words, recovery::Recovering::RootKey)
        .await
        .unwrap();
    let new_root = start.body.root.pending.clone().unwrap().public_key.unwrap();
    use agency_core::ports::RootKeys;
    assert!(alice.root_keys.holds(&org, &new_root), "made on this node");
    let twice = recovery::start(&alice.node, &org, &words, recovery::Recovering::RootKey)
        .await
        .unwrap_err();
    assert_eq!(twice.kind, ErrorKind::Conflict);

    let early = recovery::finish(&alice.node, &org, &words)
        .await
        .unwrap_err();
    assert_eq!(early.kind, ErrorKind::Conflict);
    alice.clock.advance(agency_proto::clock::Duration::seconds(
        agency_proto::org::RECOVERY_WAIT_SECONDS,
    ));
    let done = recovery::finish(&alice.node, &org, &words).await.unwrap();
    assert_eq!(done.body.root.public_key, new_root);
    assert!(
        orgs::is_root(&alice.node, &done),
        "this node holds the new root"
    );
    assert_eq!(
        done.body.root.recovery,
        Some(words.public()),
        "the words still work"
    );
}

#[tokio::test]
async fn new_words_replace_the_old_ones_after_the_wait() {
    let (alice, _) = alice().await;
    let org = id("acme");
    let (old, new) = (
        AccountKey::from_seed([0x36; 32]),
        AccountKey::from_seed([0x37; 32]),
    );
    recovery::add(&alice.node, &org, old.public()).unwrap();
    let root = alice
        .node
        .store()
        .load_org(&org)
        .unwrap()
        .body
        .root
        .public_key;
    recovery::start(
        &alice.node,
        &org,
        &old,
        recovery::Recovering::RecoveryKey(new.public()),
    )
    .await
    .unwrap();
    alice.clock.advance(agency_proto::clock::Duration::seconds(
        agency_proto::org::RECOVERY_WAIT_SECONDS,
    ));
    let done = recovery::finish(&alice.node, &org, &old).await.unwrap();
    assert_eq!(done.body.root.recovery, Some(new.public()));
    assert_eq!(done.body.root.public_key, root, "the root key stays");
}

#[tokio::test]
async fn a_second_org_takes_the_same_words_only_when_they_prove_the_first_orgs_key() {
    let (alice, _) = alice().await;
    let words = AccountKey::from_seed([0x39; 32]);
    recovery::add(&alice.node, &id("acme"), words.public()).unwrap();
    orgs::create(&alice.node, id("beta"), "Beta".into()).unwrap();

    let wrong = AccountKey::from_seed([0x3a; 32]);
    let refused = recovery::add_same_as(&alice.node, &id("beta"), &id("acme"), &wrong).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Forbidden);
    let doc = recovery::add_same_as(&alice.node, &id("beta"), &id("acme"), &words).unwrap();
    assert_eq!(doc.body.root.recovery, Some(words.public()));
}

#[tokio::test]
async fn a_recovery_finishes_only_where_its_new_root_key_is_and_a_cancel_forgets_that_key() {
    use agency_core::ports::RootKeys;
    let (alice, _) = alice().await;
    let org = id("acme");
    let words = AccountKey::from_seed([0x3b; 32]);
    recovery::add(&alice.node, &org, words.public()).unwrap();
    let start = recovery::start(&alice.node, &org, &words, recovery::Recovering::RootKey)
        .await
        .unwrap();
    let made = start.body.root.pending.clone().unwrap().public_key.unwrap();
    alice.clock.advance(agency_proto::clock::Duration::seconds(
        agency_proto::org::RECOVERY_WAIT_SECONDS,
    ));
    alice.root_keys.forget(&org, &made).unwrap();
    let elsewhere = recovery::finish(&alice.node, &org, &words)
        .await
        .unwrap_err();
    assert_eq!(elsewhere.kind, ErrorKind::Conflict);
    assert!(
        elsewhere.message.contains("where it was started"),
        "{elsewhere:?}"
    );

    let (bob_like, _) = common::alice().await;
    recovery::add(&bob_like.node, &org, words.public()).unwrap();
    let start = recovery::start(&bob_like.node, &org, &words, recovery::Recovering::RootKey)
        .await
        .unwrap();
    let made = start.body.root.pending.clone().unwrap().public_key.unwrap();
    assert!(bob_like.root_keys.holds(&org, &made));
    recovery::cancel(&bob_like.node, &org).unwrap();
    assert!(
        !bob_like.root_keys.holds(&org, &made),
        "the cancelled start's key is forgotten"
    );
}

#[tokio::test]
async fn the_node_logs_a_started_recovery_even_with_no_device() {
    let (alice, _) = alice().await;
    notify_waiting(&alice.node).await.unwrap();
    started(&alice).await;
    let pass = notify_waiting(&alice.node).await.unwrap();
    assert_eq!(pass.recoveries.len(), 1, "{:?}", pass.recoveries);
    assert!(
        pass.recoveries[0].contains("acme") && pass.recoveries[0].contains("root-cancel-recovery")
    );
}

#[tokio::test]
async fn a_root_moves_into_a_keychain_only_once_the_org_has_a_recovery_key() {
    let (alice, _) = alice().await;
    let org = id("acme");
    alice.root_keys.need_recovery(true);
    let before = alice.node.store().load_org(&org).unwrap();
    let refused = orgs::rotate_root(&alice.node, &org).unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert!(refused.message.contains("root-recovery"), "{refused:?}");
    assert_eq!(
        alice.node.store().load_org(&org).unwrap().digest(),
        before.digest()
    );

    recovery::add(
        &alice.node,
        &org,
        AccountKey::from_seed([0x38; 32]).public(),
    )
    .unwrap();
    orgs::rotate_root(&alice.node, &org).unwrap();
}
