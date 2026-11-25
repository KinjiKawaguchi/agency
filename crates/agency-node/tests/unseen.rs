//! The federation routes bypass Cloudflare Access, so anyone can call them. Until a request is
//! authenticated, the answer must not tell what this node holds: an unknown org, an unknown team, a
//! missing or wrong signature, and a non-member all get the same status and the same body.
mod common;

use agency_adapters::peers::read_headers;
use agency_core::read_auth::{sign_read, ReadAuth};
use agency_proto::org::{OrgDefinition, SignedOrg};
use agency_proto::{AccountKey, Device, DeviceChange, DeviceOp, Envelope, Kind, Signed, Timestamp};
use common::{alice, device, id, joined_bob, signed_answer, Running, T0};
use reqwest::{RequestBuilder, StatusCode};
use serde_json::{json, Value};

const SAMPLE_USAGE: &str = include_str!("../../../testdata/tokenusage.sample.json");

/// The one answer every unauthenticated refusal gets.
const REFUSED: (StatusCode, &str) = (StatusCode::NOT_FOUND, r#"{"error":"not found"}"#);

async fn answer(request: RequestBuilder) -> (StatusCode, String) {
    let response = request.send().await.unwrap();
    (response.status(), response.text().await.unwrap())
}

fn assert_refused(answer: &(StatusCode, String), case: &str) {
    assert_eq!((answer.0, answer.1.as_str()), REFUSED, "{case}");
}

fn read(running: &Running, path: &str, auth: Option<&ReadAuth>) -> RequestBuilder {
    let mut request = reqwest::Client::new().get(format!("{}{path}", running.url()));
    for (name, value) in auth.map(read_headers).into_iter().flatten() {
        request = request.header(name, value);
    }
    request
}

fn post(running: &Running, path: &str, body: &impl serde::Serialize) -> RequestBuilder {
    reqwest::Client::new()
        .post(format!("{}{path}", running.url()))
        .json(body)
}

/// A read of `path` signed as `account` with `key`, `offset` seconds from the node's time.
fn signed(running: &Running, key: &AccountKey, account: &str, path: &str, offset: i64) -> ReadAuth {
    let at = running.node.now() + agency_proto::clock::Duration::seconds(offset);
    sign_read(key, &id(account), "GET", path, at)
}

/// Every way a signed read of `path` can fail on alice's node, for a path that exists there.
fn failed_reads(
    alice: &Running,
    bob: &Running,
    path: &str,
) -> Vec<(&'static str, Option<ReadAuth>)> {
    let stranger = AccountKey::generate();
    let mut malformed = signed(alice, alice.node.key(), "alice", path, 0);
    malformed.account = "Not An Id".into();
    vec![
        ("no signature", None),
        ("a malformed account", Some(malformed)),
        (
            "a stale time",
            Some(signed(alice, alice.node.key(), "alice", path, -301)),
        ),
        (
            "a time too far ahead",
            Some(signed(alice, alice.node.key(), "alice", path, 301)),
        ),
        (
            "a non-member",
            Some(signed(alice, &stranger, "carol", path, 0)),
        ),
        (
            "a member, signed by another key",
            Some(signed(alice, &stranger, "bob", path, 0)),
        ),
        (
            "a member's signature of another path",
            Some(signed(
                alice,
                bob.node.key(),
                "bob",
                "/fed/v1/orgs/acme/definition/x",
                0,
            )),
        ),
        (
            "a non-member signed with the key that stands in when nothing is here",
            Some(signed(alice, &stand_in(), "carol", path, 0)),
        ),
    ]
}

#[tokio::test]
async fn a_signed_read_that_fails_looks_like_one_of_something_that_is_not_here() {
    let (alice, memory) = alice().await;
    memory.lock().unwrap().usage = Some(serde_json::from_str(SAMPLE_USAGE).unwrap());
    let bob = joined_bob(&alice).await;
    let routes = [
        (
            "/fed/v1/orgs/acme/definition",
            "/fed/v1/orgs/other/definition",
        ),
        (
            "/fed/v1/teams/acme/app/view",
            "/fed/v1/teams/other/app/view",
        ),
        (
            "/fed/v1/teams/acme/app/usage?days=7",
            "/fed/v1/teams/other/app/usage?days=7",
        ),
    ];
    for (path, elsewhere) in routes {
        // The usage route's signature covers its query too.
        let signed_path = path;
        let bob_reads = |path: &str| signed(&bob, bob.node.key(), "bob", path, 0);
        let unknown = answer(read(&alice, elsewhere, Some(&bob_reads(elsewhere)))).await;
        assert_refused(&unknown, &format!("{elsewhere}: an unknown org"));
        for (case, auth) in failed_reads(&alice, &bob, signed_path) {
            let refused = answer(read(&alice, path, auth.as_ref())).await;
            assert_refused(&refused, &format!("{path}: {case}"));
        }
        let member = answer(read(&alice, path, Some(&bob_reads(signed_path)))).await;
        assert_eq!(member.0, StatusCode::OK, "{path}: bob reads it");
    }
    for (path, case) in [
        (
            "/fed/v1/teams/acme/other/view",
            "an unknown team in a known org",
        ),
        (
            "/fed/v1/teams/acme/other/usage?days=7",
            "the usage of an unknown team",
        ),
        ("/fed/v1/teams/acme/Not-An-Id/view", "a malformed team id"),
    ] {
        let auth = signed(&bob, bob.node.key(), "bob", path, 0);
        assert_refused(&answer(read(&alice, path, Some(&auth))).await, case);
    }
}

/// The key whose seed the node uses to spend a signature check when there is no key to check.
fn stand_in() -> AccountKey {
    AccountKey::from_seed([0x5a; 32])
}

/// A well-formed first version of an org, made by whoever holds the stand-in key.
fn planted() -> SignedOrg {
    let key = stand_in();
    let founder = agency_proto::Person {
        id: id("mallory"),
        label: "Mallory".into(),
        account: id("mallory"),
        public_key: key.public(),
        department: None,
        role: agency_proto::Role::Member,
        devices: vec![],
    };
    let body = OrgDefinition {
        org: id("planted"),
        label: "Planted".into(),
        version: 1,
        prev: None,
        root: agency_proto::org::Root {
            account: id("mallory"),
            public_key: key.public(),
        },
        departments: vec![],
        people: vec![founder],
        teams: vec![],
        revoked: vec![],
        at: T0.into(),
    };
    Signed::sign(Kind::Org, body, id("mallory"), &key).unwrap()
}

fn envelope_from(running: &Running, signed: Signed<Envelope>, team: &str) -> RequestBuilder {
    post(
        running,
        &format!("/fed/v1/teams/acme/{team}/envelopes"),
        &signed,
    )
}

#[tokio::test]
async fn an_envelope_that_fails_its_check_looks_like_one_for_a_team_that_is_not_here() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    common::for_bob(&alice, &memory, "r-1");
    let phone = device(&bob).await;
    let stranger = AccountKey::generate();

    let mut tampered = signed_answer(&bob, &phone, "r-1", "a");
    tampered.body.payload.choice = Some("b".into());
    let mut unknown_device = signed_answer(&bob, &stranger, "r-1", "a");
    unknown_device.signer = id("d-000000000000");
    let by_account = Signed::sign(
        Kind::Envelope,
        signed_answer(&bob, &phone, "r-1", "a").body,
        id("bob"),
        bob.node.key(),
    )
    .unwrap();
    let mut non_member = signed_answer(&bob, &stranger, "r-1", "a");
    non_member.body.account = id("carol");
    let non_member = Signed::sign(
        Kind::Envelope,
        non_member.body,
        non_member.signer,
        &stranger,
    )
    .unwrap();
    let mut to_other_team = signed_answer(&bob, &phone, "r-1", "a");
    to_other_team.body.team = id("other");
    let to_other_team = Signed::sign(
        Kind::Envelope,
        to_other_team.body,
        to_other_team.signer,
        &phone,
    )
    .unwrap();
    let mut to_other_org = signed_answer(&bob, &phone, "r-1", "a");
    to_other_org.body.org = id("other");
    let to_other_org = Signed::sign(
        Kind::Envelope,
        to_other_org.body,
        to_other_org.signer,
        &phone,
    )
    .unwrap();

    let cases = [
        (
            "an unknown team",
            envelope_from(&alice, to_other_team, "other"),
        ),
        (
            "an unknown org",
            post(&alice, "/fed/v1/teams/other/app/envelopes", &to_other_org),
        ),
        ("a changed payload", envelope_from(&alice, tampered, "app")),
        (
            "a device the member does not have",
            envelope_from(&alice, unknown_device, "app"),
        ),
        (
            "the member's account key",
            envelope_from(&alice, by_account, "app"),
        ),
        ("a non-member", envelope_from(&alice, non_member, "app")),
    ];
    for (case, request) in cases {
        assert_refused(&answer(request).await, case);
    }
    assert!(memory.lock().unwrap().applied.is_empty(), "nothing applied");
    let genuine = answer(envelope_from(
        &alice,
        signed_answer(&bob, &phone, "r-1", "a"),
        "app",
    ))
    .await;
    assert_eq!(genuine.0, StatusCode::OK, "{}", genuine.1);
}

fn first_device(account: &str, key: &AccountKey) -> DeviceChange {
    DeviceChange {
        org: id("acme"),
        account: id(account),
        op: DeviceOp::Add {
            device: Box::new(Device {
                id: key.public().device_id(),
                label: "Laptop".into(),
                public_key: key.public(),
                added_at: Timestamp::parse(T0).unwrap(),
            }),
        },
        at: Timestamp::parse(T0).unwrap(),
    }
}

#[tokio::test]
async fn a_device_change_that_fails_its_check_looks_like_one_for_an_org_that_is_not_here() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let laptop = AccountKey::generate();
    let stranger = AccountKey::generate();
    let sign = |change: DeviceChange, signer: &str, key: &AccountKey| {
        json!(Signed::sign(Kind::Device, change, id(signer), key).unwrap())
    };
    let mut elsewhere = first_device("bob", &laptop);
    elsewhere.org = id("other");
    let path = "/fed/v1/orgs/acme/devices";
    let cases = [
        (
            "an unknown org",
            post(
                &alice,
                "/fed/v1/orgs/other/devices",
                &sign(elsewhere, "bob", bob.node.key()),
            ),
        ),
        (
            "signed by another key",
            post(
                &alice,
                path,
                &sign(first_device("bob", &laptop), "bob", &stranger),
            ),
        ),
        (
            "a non-member",
            post(
                &alice,
                path,
                &sign(first_device("carol", &laptop), "carol", &stranger),
            ),
        ),
        (
            "a non-member, signed with the key that stands in when nothing is here",
            post(
                &alice,
                path,
                &sign(first_device("carol", &laptop), "carol", &stand_in()),
            ),
        ),
        (
            "a node without the root key, sent a forged change",
            post(
                &bob,
                path,
                &sign(first_device("bob", &laptop), "bob", &stranger),
            ),
        ),
    ];
    for (case, request) in cases {
        assert_refused(&answer(request).await, case);
    }
    let genuine = sign(first_device("bob", &laptop), "bob", bob.node.key());
    let (status, _) = answer(post(&bob, path, &genuine)).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "authenticated, so a member's node may say it is not the root's"
    );
    let (status, _) = answer(post(&alice, path, &genuine)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_definition_not_signed_by_the_root_looks_like_one_for_an_org_that_is_not_here() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let current = alice.node.store().load_org(&id("acme")).unwrap();
    let forge = |version_offset: u64| -> SignedOrg {
        let mut body = OrgDefinition::successor_of(&current, alice.node.now_text());
        body.version += version_offset;
        Signed::sign(Kind::Org, body, id("alice"), bob.node.key()).unwrap()
    };
    let stale: SignedOrg =
        Signed::sign(Kind::Org, current.body.clone(), id("alice"), bob.node.key()).unwrap();
    let mut other = forge(0);
    other.body.org = id("other");
    let cases: [(&str, &Running, Value); 5] = [
        ("an org the node does not know", &alice, json!(other)),
        (
            "a first version of an org the node does not trust, signed with the key that stands in when nothing is here",
            &alice,
            json!(planted()),
        ),
        ("the next version, signed by another key", &alice, json!(forge(0))),
        ("a version far ahead, signed by another key", &alice, json!(forge(5))),
        ("the current version, signed by another key", &alice, json!(stale)),
    ];
    for (case, running, body) in cases {
        let org = body["body"]["org"].as_str().unwrap().to_owned();
        let path = format!("/fed/v1/orgs/{org}/definition");
        assert_refused(&answer(post(running, &path, &body)).await, case);
    }
    assert!(alice.node.store().load_org(&id("planted")).is_none());
    let resent = answer(post(&bob, "/fed/v1/orgs/acme/definition", &current)).await;
    assert_ne!(
        resent.0,
        StatusCode::NOT_FOUND,
        "the root's own version is authenticated: {}",
        resent.1
    );
}
