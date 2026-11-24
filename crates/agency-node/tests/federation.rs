//! 2 つの台を同じプロセスで動かし、招待 → 参加 → 写し → 署名した決裁 → ため置き → 起きた後の届け を通す。
//! 断る側 (所属していない人、書き換えた封筒、使った招待、偽の台、根でない鍵の版) も確かめる。
mod common;

use agency_adapters::peers::read_headers;
use agency_core::decisions::{decide, Decided};
use agency_core::invites::{self, join};
use agency_core::outbox::{deliver, deliver_due, pending, ParcelBody};
use agency_core::read_auth::sign_read;
use agency_core::views::{refresh, Replica};
use agency_proto::invite::Invite;
use agency_proto::org::SignedOrg;
use agency_proto::{EnvelopeKind, Kind, OrgDefinition, Role, Signed};
use common::{alice, answer, id, joined_bob, memory_runtime, node};

#[tokio::test]
async fn invite_join_replica_and_signed_decision() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    // alice の台も、bob を書き足した版を持っている (書き手は alice の根の鍵 1 つ)
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

    let outcome = decide(
        &bob.node,
        &id("acme"),
        &id("app"),
        EnvelopeKind::Answer,
        "r-1".into(),
        answer("ア"),
    )
    .await
    .unwrap();
    assert_eq!(outcome, Decided::Delivered);
    let applied = memory.lock().unwrap().applied.clone();
    assert_eq!(applied.len(), 1);
    assert_eq!(
        (applied[0].by.as_str(), applied[0].payload.choice.as_deref()),
        ("bob", Some("ア")),
        "答えた人は封筒のアカウントの人"
    );

    // A request the team runtime does not know is a refusal: not retried, not queued
    let outcome = decide(
        &bob.node,
        &id("acme"),
        &id("app"),
        EnvelopeKind::Answer,
        "r-404".into(),
        answer("ア"),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Decided::Refused(_)), "{outcome:?}");
    assert!(pending(&bob.node).is_empty());
}

#[tokio::test]
async fn decisions_wait_while_the_team_node_sleeps_and_arrive_after() {
    let (mut alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    alice.stop().await;
    let outcome = decide(
        &bob.node,
        &id("acme"),
        &id("app"),
        EnvelopeKind::Answer,
        "r-1".into(),
        answer("イ"),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Decided::Queued(_)), "{outcome:?}");
    assert_eq!(pending(&bob.node).len(), 1);
    refresh(&bob.node).await.unwrap();
    let replica: Replica = bob
        .node
        .store()
        .load_replica(&id("acme"), &id("app"))
        .unwrap_or_default();
    assert!(replica.error.is_some(), "寝ている台の写しは失敗を書く");

    alice.start().await;
    assert!(
        deliver_due(&bob.node).await.unwrap().is_empty(),
        "送り直しの時刻の前には届けに行かない"
    );
    let parcel = pending(&bob.node).remove(0);
    deliver(&bob.node, &parcel).await.unwrap();
    assert!(pending(&bob.node).is_empty());
    assert_eq!(memory.lock().unwrap().applied.len(), 1);

    // 同じ封筒の 2 度目は当てない (送り直しで 2 回届いた写し)
    let ParcelBody::Envelope(signed) = parcel.body else {
        panic!("封筒のはず")
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
    let http = reqwest::Client::new();

    // 所属していない人の署名つきの読み出しは 404 (組織があることも見せない)
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

    // 書き換えた封筒は 401 で、当てない
    decide(
        &bob.node,
        &id("acme"),
        &id("app"),
        EnvelopeKind::Answer,
        "r-1".into(),
        answer("ア"),
    )
    .await
    .unwrap();
    let mut tampered: Signed<agency_proto::Envelope> = Signed::sign(
        Kind::Envelope,
        agency_proto::Envelope {
            id: agency_proto::EnvelopeId::from_bytes([9; 16]),
            kind: EnvelopeKind::Answer,
            org: id("acme"),
            team: id("app"),
            target: "r-1".into(),
            payload: answer("ア"),
            account: id("bob"),
            at: alice.node.now_text(),
        },
        id("bob"),
        bob.node.key(),
    )
    .unwrap();
    tampered.body.payload = answer("イ");
    let response = http
        .post(format!("{}/fed/v1/teams/acme/app/envelopes", alice.url()))
        .json(&tampered)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(memory.lock().unwrap().applied.len(), 1);

    // 使った招待では、もう入れない
    let link = invites::invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    join(&carol.node, &link, |_| true).await.unwrap();
    let dave = node("dave", memory_runtime().0).await;
    assert!(join(&dave.node, &link, |_| true).await.is_err());

    // 指紋の合わない招待のリンク (偽の台) では申し込まない
    let mut fake =
        Invite::parse(&invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap())
            .unwrap();
    fake.fp = "0".repeat(32);
    let error = join(&dave.node, &fake.to_link(), |_| true)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("fingerprint"), "{error}");

    // 根でない鍵で署名した次の版は、受け取らない
    let current = alice.node.store().load_org(&id("acme")).unwrap();
    let mut body = OrgDefinition::successor_of(&current, alice.node.now_text());
    body.label = "乗っ取り".into();
    let forged: SignedOrg = Signed::sign(Kind::Org, body, id("alice"), bob.node.key()).unwrap();
    let response = http
        .post(format!("{}/fed/v1/orgs/acme/definition", alice.url()))
        .json(&forged)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(
        alice.node.store().load_org(&id("acme")).unwrap().body.label,
        "Acme"
    );
}
