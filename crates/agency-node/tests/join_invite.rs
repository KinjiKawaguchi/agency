//! An invite is used only by a join that the root's node takes: one it signs, or one it holds for its owner. A
//! join refused on the way, at signing or because too many joins wait already, leaves the invite as it was, and
//! two joins with one invite at once still make one member.
mod common;

use agency_core::invites::{invite, join, Joined};
use agency_proto::{Id, Role};
use common::{alice, id, memory_runtime, node};

fn acme() -> Id {
    id("acme")
}

#[tokio::test]
async fn a_join_the_root_key_does_not_sign_leaves_the_invite_for_the_next_try() {
    let (alice, _) = alice().await;
    let bob = node("bob", memory_runtime().0).await;
    let link = invite(&alice.node, &acme(), None, Role::Member).unwrap();

    alice.root_keys.refuse(true);
    let refused = join(&bob.node, &link, |_| true).await;
    assert!(refused.is_err(), "{refused:?}");
    alice.root_keys.refuse(false);

    let joined = join(&bob.node, &link, |_| true).await.unwrap();
    assert!(matches!(joined, Joined::Member(_)), "{joined:?}");
    let carol = node("carol", memory_runtime().0).await;
    assert!(
        join(&carol.node, &link, |_| true).await.is_err(),
        "the invite is used once bob is in"
    );
}

#[tokio::test]
async fn a_join_that_is_not_held_because_too_many_wait_gives_its_invite_back() {
    let (alice, _) = alice().await;
    alice.root_keys.ask_presence(true);
    let bob = node("bob", memory_runtime().0).await;
    let most = agency_core::pending_root::MOST_HELD_PER_ACCOUNT;
    for _ in 0..most {
        let link = invite(&alice.node, &acme(), None, Role::Member).unwrap();
        let joined = join(&bob.node, &link, |_| true).await.unwrap();
        assert!(matches!(joined, Joined::Waiting(_)), "{joined:?}");
    }
    let link = invite(&alice.node, &acme(), None, Role::Member).unwrap();
    let refused = join(&bob.node, &link, |_| true).await;
    assert!(refused.is_err(), "one more than may wait: {refused:?}");

    let carol = node("carol", memory_runtime().0).await;
    let joined = join(&carol.node, &link, |_| true).await.unwrap();
    assert!(
        matches!(joined, Joined::Waiting(_)),
        "the invite is still there for carol: {joined:?}"
    );
}

#[tokio::test]
async fn two_joins_with_one_invite_at_once_make_one_member() {
    let (alice, _) = alice().await;
    let bob = node("bob", memory_runtime().0).await;
    let carol = node("carol", memory_runtime().0).await;
    let link = invite(&alice.node, &acme(), None, Role::Member).unwrap();

    let (by_bob, by_carol) = tokio::join!(
        join(&bob.node, &link, |_| true),
        join(&carol.node, &link, |_| true)
    );
    assert_eq!(
        [by_bob.is_ok(), by_carol.is_ok()]
            .iter()
            .filter(|is_in| **is_in)
            .count(),
        1,
        "bob: {by_bob:?}, carol: {by_carol:?}"
    );
    let doc = alice.node.store().load_org(&acme()).unwrap();
    let joined = ["bob", "carol"]
        .iter()
        .filter(|one| doc.body.member(&id(one)).is_some())
        .count();
    assert_eq!(joined, 1);
}
