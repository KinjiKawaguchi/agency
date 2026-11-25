//! Runs two nodes in one process through invite -> join -> replica -> signed decision -> outbox -> delivery after waking up.
//! Also checks the refusals: non-members, tampered envelopes, used invites, fake nodes, and versions not signed by the root key.
mod common;

use agency_adapters::peers::read_headers;
use agency_core::decisions::Decided;
use agency_core::invites::{self, join};
use agency_core::outbox::{deliver, deliver_due, pending, ParcelBody};
use agency_core::read_auth::sign_read;
use agency_core::views::{refresh, Replica};
use agency_proto::invite::Invite;
use agency_proto::org::SignedOrg;
use agency_proto::{AccountKey, Kind, OrgDefinition, Role, Signed};
use common::{
    alice, answer, decide_as, device, id, joined_bob, memory_runtime, node, signed_answer,
};

#[tokio::test]
async fn invite_join_replica_and_signed_decision() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    common::for_bob(&alice, &memory, "r-1");
    let bob_key = device(&bob).await;
    // alice's node also has the version with bob added (its one writer is alice's root key)
    assert!(alice
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .member(&id("bob"))
        .is_some());

    refresh(&bob.node).await.unwrap();
    let replica: Replica = bob
        .node
        .store()
        .load_replica(&id("acme"), &id("app"))
        .unwrap();
    assert_eq!(replica.error, None);
    assert_eq!(replica.view.unwrap().requests[0].id, "r-1");

    let outcome = decide_as(&bob, &bob_key, "r-1", answer("a")).await.unwrap();
    assert_eq!(outcome, Decided::Delivered);
    let applied = memory.lock().unwrap().applied.clone();
    assert_eq!(applied.len(), 1);
    assert_eq!(
        (applied[0].by.as_str(), applied[0].payload.choice.as_deref()),
        ("bob", Some("a")),
        "the decider is the envelope's account"
    );

    // A request the team runtime does not know is a refusal: not retried, not queued
    let outcome = decide_as(&bob, &bob_key, "r-404", answer("a"))
        .await
        .unwrap();
    assert!(matches!(outcome, Decided::Refused(_)), "{outcome:?}");
    assert!(pending(&bob.node).is_empty());
}

#[tokio::test]
async fn decisions_wait_while_the_team_node_sleeps_and_arrive_after() {
    let (mut alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    common::for_bob(&alice, &memory, "r-1");
    let bob_key = device(&bob).await;
    alice.stop().await;
    let outcome = decide_as(&bob, &bob_key, "r-1", answer("b")).await.unwrap();
    assert!(matches!(outcome, Decided::Queued(_)), "{outcome:?}");
    assert_eq!(pending(&bob.node).len(), 1);
    refresh(&bob.node).await.unwrap();
    let replica: Replica = bob
        .node
        .store()
        .load_replica(&id("acme"), &id("app"))
        .unwrap_or_default();
    assert!(
        replica.error.is_some(),
        "a replica of a sleeping node records the failure"
    );

    alice.start().await;
    assert!(
        deliver_due(&bob.node).await.unwrap().is_empty(),
        "no delivery before the retry is due"
    );
    let parcel = pending(&bob.node).remove(0);
    deliver(&bob.node, &parcel).await.unwrap();
    assert!(pending(&bob.node).is_empty());
    assert_eq!(memory.lock().unwrap().applied.len(), 1);

    // The same envelope is not applied twice (a copy delivered again by a retry)
    let ParcelBody::Envelope(signed) = parcel.body else {
        panic!("expected an envelope")
    };
    let signed = *signed;
    let again = reqwest::Client::new()
        .post(format!("{}/fed/v1/teams/acme/app/envelopes", alice.url()))
        .json(&signed)
        .send()
        .await
        .unwrap();
    assert_eq!(
        again.json::<serde_json::Value>().await.unwrap()["duplicate"],
        true
    );
    assert_eq!(memory.lock().unwrap().applied.len(), 1);
}

#[tokio::test]
async fn refuses_strangers_tampering_reused_invites_and_fake_nodes() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    common::for_bob(&alice, &memory, "r-1");
    let bob_key = device(&bob).await;
    let http = reqwest::Client::new();

    // A non-member's signed read gets 404 (the org's existence is not revealed)
    let carol = node("carol", memory_runtime().0).await;
    carol
        .node
        .store()
        .save_org(&alice.node.store().load_org(&id("acme")).unwrap())
        .unwrap();
    refresh(&carol.node).await.unwrap();
    assert!(carol
        .node
        .store()
        .load_replica(&id("acme"), &id("app"))
        .is_none());
    let mut request = http.get(format!("{}/fed/v1/teams/acme/app/view", alice.url()));
    let auth = sign_read(
        carol.node.key(),
        &id("carol"),
        "GET",
        "/fed/v1/teams/acme/app/view",
        carol.node.now(),
    );
    for (name, value) in read_headers(&auth) {
        request = request.header(name, value);
    }
    assert_eq!(request.send().await.unwrap().status(), 404);

    // A tampered envelope is refused like an unknown team, and is not applied
    decide_as(&bob, &bob_key, "r-1", answer("a")).await.unwrap();
    let mut tampered = signed_answer(&bob, &bob_key, "r-1", "a");
    tampered.body.payload = answer("b");
    let post = |signed: Signed<agency_proto::Envelope>| {
        http.post(format!("{}/fed/v1/teams/acme/app/envelopes", alice.url()))
            .json(&signed)
            .send()
    };
    assert_eq!(post(tampered).await.unwrap().status(), 404);

    // An envelope signed by bob's account key, which bob's node holds, is not a decision
    let by_account = Signed::sign(
        Kind::Envelope,
        signed_answer(&bob, &bob_key, "r-1", "a").body,
        id("bob"),
        bob.node.key(),
    )
    .unwrap();
    assert_eq!(post(by_account).await.unwrap().status(), 404);

    // A non-member's envelope gets 404 like a non-member's read, whatever key signed it
    let from_carol = signed_answer(&carol, &AccountKey::generate(), "r-1", "a");
    assert_eq!(post(from_carol).await.unwrap().status(), 404);
    assert_eq!(memory.lock().unwrap().applied.len(), 1);

    // A used invite cannot be used again
    let link = invites::invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    join(&carol.node, &link, |_| true).await.unwrap();
    let dave = node("dave", memory_runtime().0).await;
    assert!(join(&dave.node, &link, |_| true).await.is_err());

    // No join request through a link whose fingerprint does not match (a fake node)
    let mut fake =
        Invite::parse(&invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
            .unwrap();
    fake.fp = "0".repeat(32);
    let error = join(&dave.node, &fake.to_link(), |_| true)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("fingerprint"), "{error}");

    // A next version signed by a key that is not the root is refused
    let current = alice.node.store().load_org(&id("acme")).unwrap();
    let mut body = OrgDefinition::successor_of(&current, alice.node.now_text());
    body.label = "Taken over".into();
    let forged: SignedOrg = Signed::sign(Kind::Org, body, id("alice"), bob.node.key()).unwrap();
    let response = http
        .post(format!("{}/fed/v1/orgs/acme/definition", alice.url()))
        .json(&forged)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    assert_eq!(
        alice.node.store().load_org(&id("acme")).unwrap().body.label,
        "Acme"
    );
}
