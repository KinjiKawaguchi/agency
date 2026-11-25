//! Device keys end to end: a new device asks through the Client API, the terminal adds the first device
//! with the account key, an existing device adds and removes later ones, and only devices sign decisions
//! relayed by the home node. A member's changes travel to the root's node, through the outbox when it
//! sleeps.
mod common;

use agency_core::devices::{add_device, Added, DeviceSigner};
use agency_core::outbox::deliver_due;
use agency_core::signed_decisions::decide_with_device;
use agency_core::views::refresh;
use agency_proto::client::DeviceChangeOutcome;
use agency_proto::{
    AccountKey, Device, DeviceChange, DeviceOp, Envelope, EnvelopeId, EnvelopeKind, Kind, Payload,
    Signed, Timestamp,
};
use common::{alice, body, get, id, joined_bob, post, Running, T0, TOKEN};
use reqwest::StatusCode;
use serde_json::{json, Value};

async fn send(running: &Running, path: &str, payload: &Value) -> (StatusCode, Value) {
    body(
        post(running, path, payload)
            .header("x-agency-token", TOKEN)
            .send()
            .await
            .unwrap(),
    )
    .await
}

async fn devices(running: &Running) -> Value {
    body(get(running, "/api/devices").send().await.unwrap())
        .await
        .1
}

async fn ask_to_add(running: &Running, label: &str, key: &AccountKey) -> Value {
    let (status, request) = send(
        running,
        "/api/devices/requests",
        &json!({ "label": label, "publicKey": key.public().encoded() }),
    )
    .await;
    assert_eq!(status, 200, "{request}");
    request
}

fn envelope(account: &str, target: &str) -> Envelope {
    Envelope {
        id: EnvelopeId::from_bytes(rand_bytes(target)),
        kind: EnvelopeKind::Answer,
        org: id("acme"),
        team: id("app"),
        target: target.into(),
        payload: Payload {
            text: String::new(),
            choice: Some("a".into()),
        },
        account: id(account),
        at: T0.into(),
    }
}

/// Distinct envelope ids per target, without randomness.
fn rand_bytes(seed: &str) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    for (index, byte) in seed.bytes().enumerate() {
        bytes[index % 16] ^= byte;
    }
    bytes
}

fn signed_by(body: Envelope, key: &AccountKey) -> Value {
    let signer = key.public().device_id();
    let signed = Signed::sign(Kind::Envelope, body, signer, key).unwrap();
    json!({ "envelope": signed, "title": "Make a key?" })
}

fn change(account: &str, op: DeviceOp) -> DeviceChange {
    DeviceChange {
        org: id("acme"),
        account: id(account),
        op,
        at: Timestamp::parse(T0).unwrap(),
    }
}

fn add(account: &str, label: &str, key: &AccountKey) -> DeviceChange {
    let device = Device {
        id: key.public().device_id(),
        label: label.into(),
        public_key: key.public(),
        added_at: Timestamp::parse(T0).unwrap(),
    };
    change(
        account,
        DeviceOp::Add {
            device: Box::new(device),
        },
    )
}

fn signed_change(change: DeviceChange, key: &AccountKey) -> Value {
    let signer = key.public().device_id();
    json!(Signed::sign(Kind::Device, change, signer, key).unwrap())
}

#[tokio::test]
async fn the_terminal_adds_the_first_device_and_only_devices_decide() {
    let (alice, memory) = alice().await;
    let phone = AccountKey::generate();
    let request = ask_to_add(&alice, "Phone", &phone).await;
    assert_eq!(request["pairingCode"], phone.public().pairing_code());
    assert_eq!(request["deviceId"], phone.public().device_id().to_string());
    let listed = devices(&alice).await;
    assert_eq!(listed["requests"][0]["label"], "Phone");
    assert_eq!(listed["orgs"][0]["devices"], json!([]));

    let added = add_device(&alice.node, "Phone", &phone.public(), None).await;
    assert!(matches!(
        &added[..],
        [(org, Added::Done(result))] if org == &id("acme") && result.outcome == DeviceChangeOutcome::Applied
    ));
    let listed = devices(&alice).await;
    assert_eq!(listed["orgs"][0]["devices"][0]["label"], "Phone");
    assert_eq!(
        listed["requests"],
        json!([]),
        "an added device stops waiting"
    );

    let again = add_device(
        &alice.node,
        "Tablet",
        &AccountKey::generate().public(),
        None,
    )
    .await;
    assert!(
        matches!(&again[..], [(_, Added::NeedsDevice)]),
        "the account key adds only the first device"
    );

    let path = "/api/orgs/acme/teams/app/envelopes";
    let (status, result) = send(&alice, path, &signed_by(envelope("alice", "r-1"), &phone)).await;
    assert_eq!(
        (status, result["outcome"].as_str()),
        (StatusCode::OK, Some("applied"))
    );
    assert_eq!(memory.lock().unwrap().applied[0].by, id("alice"));
    let log = body(get(&alice, "/api/decisions").send().await.unwrap())
        .await
        .1;
    assert_eq!(log["entries"][0]["title"], "Make a key?");

    let by_account = Signed::sign(
        Kind::Envelope,
        envelope("alice", "r-1"),
        id("alice"),
        alice.node.key(),
    )
    .unwrap();
    let (status, _) = send(&alice, path, &json!({ "envelope": by_account })).await;
    assert_eq!(
        status, 401,
        "the node's account key does not sign a relayed decision"
    );
    let stranger = AccountKey::generate();
    let (status, _) = send(
        &alice,
        path,
        &signed_by(envelope("alice", "r-1"), &stranger),
    )
    .await;
    assert_eq!(status, 401, "a device that was never added");
    let (status, _) = send(&alice, path, &signed_by(envelope("bob", "r-1"), &phone)).await;
    assert_eq!(
        status, 403,
        "the node relays only its own account's decisions"
    );
    let (status, _) = send(
        &alice,
        "/api/orgs/acme/teams/other/envelopes",
        &signed_by(envelope("alice", "r-1"), &phone),
    )
    .await;
    assert_eq!(status, 400, "an envelope for another team than its path");
    let (status, _) = send(
        &alice,
        "/api/devices/changes",
        &signed_change(add("bob", "Phone", &AccountKey::generate()), &phone),
    )
    .await;
    assert_eq!(
        status, 403,
        "the node relays only its own account's device changes"
    );
    assert_eq!(memory.lock().unwrap().applied.len(), 1);
}

#[tokio::test]
async fn the_cli_device_decides_only_once_added() {
    let (alice, memory) = alice().await;
    let cli = AccountKey::generate();
    let cli_id = cli.public().device_id();
    let (org, team) = (id("acme"), id("app"));
    let decide = || {
        decide_with_device(
            &alice.node,
            DeviceSigner {
                id: &cli_id,
                key: &cli,
            },
            &org,
            &team,
            EnvelopeKind::Answer,
            "r-1".into(),
            Payload {
                text: "Go".into(),
                choice: None,
            },
        )
    };
    assert_eq!(
        decide().await.unwrap_err().kind,
        agency_core::ErrorKind::Unauthorized
    );
    add_device(&alice.node, "CLI", &cli.public(), None).await;
    decide().await.unwrap();
    assert_eq!(memory.lock().unwrap().applied[0].payload.text, "Go");

    let phone = AccountKey::generate();
    let stranger = AccountKey::generate();
    let stranger_id = cli_id.clone();
    let not_added = DeviceSigner {
        id: &stranger_id,
        key: &stranger,
    };
    let refused = add_device(&alice.node, "Phone", &phone.public(), Some(&not_added)).await;
    assert!(
        matches!(&refused[..], [(_, Added::NeedsDevice)]),
        "only a CLI device that was added signs later adds"
    );
    let signer = DeviceSigner {
        id: &cli_id,
        key: &cli,
    };
    let added = add_device(&alice.node, "Phone", &phone.public(), Some(&signer)).await;
    assert!(matches!(&added[..], [(_, Added::Done(_))]), "{added:?}");
    let doc = alice.node.store().load_org(&org).unwrap();
    assert!(doc
        .body
        .device(&id("alice"), &phone.public().device_id())
        .is_some());
}

#[tokio::test]
async fn a_members_devices_are_added_and_removed_through_the_roots_node() {
    let (alice, memory) = alice().await;
    let bob = joined_bob(&alice).await;
    let (laptop, phone) = (AccountKey::generate(), AccountKey::generate());

    let added = add_device(&bob.node, "Laptop", &laptop.public(), None).await;
    assert!(matches!(
        &added[..],
        [(_, Added::Done(result))] if result.outcome == DeviceChangeOutcome::Delivered
    ));
    let root_copy = alice.node.store().load_org(&id("acme")).unwrap();
    assert!(root_copy
        .body
        .device(&id("bob"), &laptop.public().device_id())
        .is_some());

    ask_to_add(&bob, "Phone", &phone).await;
    let by_account = json!(Signed::sign(
        Kind::Device,
        add("bob", "Phone", &phone),
        id("bob"),
        bob.node.key()
    )
    .unwrap());
    let (status, _) = send(&bob, "/api/devices/changes", &by_account).await;
    assert_eq!(status, 400, "the account key cannot add a second device");
    let (status, result) = send(
        &bob,
        "/api/devices/changes",
        &signed_change(add("bob", "Phone", &phone), &laptop),
    )
    .await;
    assert_eq!(
        (status, result["outcome"].as_str()),
        (StatusCode::OK, Some("delivered"))
    );
    assert_eq!(
        devices(&bob).await["orgs"][0]["devices"][1]["label"],
        "Phone"
    );

    let path = "/api/orgs/acme/teams/app/envelopes";
    let (status, result) = send(&bob, path, &signed_by(envelope("bob", "r-1"), &phone)).await;
    assert_eq!(
        (status, result["outcome"].as_str()),
        (StatusCode::OK, Some("delivered"))
    );
    assert_eq!(
        memory.lock().unwrap().applied[0].by,
        id("bob"),
        "applied as bob by alice's node"
    );

    let lost = change(
        "bob",
        DeviceOp::Remove {
            device: phone.public().device_id(),
        },
    );
    let (status, _) = send(&bob, "/api/devices/changes", &signed_change(lost, &laptop)).await;
    assert_eq!(status, 200);
    let (status, _) = send(&bob, path, &signed_by(envelope("bob", "r-2"), &phone)).await;
    assert_eq!(status, 401, "a removed device no longer decides");
    let delivered = Signed::sign(
        Kind::Envelope,
        envelope("bob", "r-3"),
        phone.public().device_id(),
        &phone,
    )
    .unwrap();
    let direct = reqwest::Client::new()
        .post(format!("{}/fed/v1/teams/acme/app/envelopes", alice.url()))
        .json(&delivered)
        .send()
        .await
        .unwrap();
    assert_eq!(direct.status(), 401, "the team's node refuses it too");
    assert_eq!(memory.lock().unwrap().applied.len(), 1);
}

#[tokio::test]
async fn a_change_waits_in_the_outbox_while_the_roots_node_sleeps() {
    let (mut alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let laptop = AccountKey::generate();
    alice.stop().await;
    let added = add_device(&bob.node, "Laptop", &laptop.public(), None).await;
    assert!(matches!(
        &added[..],
        [(_, Added::Done(result))] if result.outcome == DeviceChangeOutcome::Queued
    ));
    alice.start().await;
    bob.clock
        .advance(agency_proto::clock::Duration::seconds(15));
    deliver_due(&bob.node).await.unwrap();
    refresh(&bob.node).await.unwrap();
    let copy = bob.node.store().load_org(&id("acme")).unwrap();
    assert!(copy
        .body
        .device(&id("bob"), &laptop.public().device_id())
        .is_some());
}

#[tokio::test]
async fn device_requests_are_checked_and_expire() {
    let (alice, _) = alice().await;
    let request = |label: &str, key: String| json!({ "label": label, "publicKey": key });
    let phone = AccountKey::generate().public().encoded();
    let account = alice.node.key().public().encoded();
    for (name, input) in [
        ("an empty label", request(" ", phone.clone())),
        ("the account key", request("Phone", account)),
        ("not a key", request("Phone", "AAAA".into())),
    ] {
        assert_eq!(
            send(&alice, "/api/devices/requests", &input).await.0,
            400,
            "{name}"
        );
    }
    let unsigned = post(
        &alice,
        "/api/devices/requests",
        &request("Phone", phone.clone()),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(unsigned.status(), 403, "a write needs the page's token");

    ask_to_add(&alice, "Phone", &AccountKey::generate()).await;
    alice
        .clock
        .advance(agency_proto::clock::Duration::minutes(61));
    assert_eq!(
        devices(&alice).await["requests"],
        json!([]),
        "a request waits an hour"
    );
}

async fn post_fed(running: &Running, path: &str, payload: &Value) -> StatusCode {
    reqwest::Client::new()
        .post(format!("{}{path}", running.url()))
        .json(payload)
        .send()
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn only_the_roots_node_writes_a_device_change_and_only_a_valid_one() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let laptop = AccountKey::generate();
    let first = json!(Signed::sign(
        Kind::Device,
        add("bob", "Laptop", &laptop),
        id("bob"),
        bob.node.key()
    )
    .unwrap());
    assert_eq!(
        post_fed(&bob, "/fed/v1/orgs/acme/devices", &first).await,
        403,
        "a member's node does not hold the root key"
    );
    let stranger = AccountKey::generate();
    let forged = json!(Signed::sign(
        Kind::Device,
        add("bob", "Laptop", &laptop),
        id("bob"),
        &stranger
    )
    .unwrap());
    assert_eq!(
        post_fed(&alice, "/fed/v1/orgs/acme/devices", &forged).await,
        400,
        "not signed by bob's account key"
    );
    assert_eq!(
        post_fed(&alice, "/fed/v1/orgs/acme/devices", &first).await,
        200
    );
}

/// A node that answers every device change with a next version signed by some other key.
async fn fake_root(doc: agency_proto::org::SignedOrg) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let forger = AccountKey::generate();
    let mut body = agency_proto::OrgDefinition::successor_of(&doc, T0.into());
    body.label = "Taken over".into();
    let forged = Signed::sign(Kind::Org, body, doc.body.root.account.clone(), &forger).unwrap();
    let app = axum::Router::new().route(
        "/fed/v1/orgs/{org}/devices",
        axum::routing::post(move || async move { axum::Json(forged.clone()) }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
async fn a_version_from_the_roots_node_is_kept_only_if_the_root_signed_it() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let before = bob.node.store().load_org(&id("acme")).unwrap();
    let fake = fake_root(before.clone()).await;
    bob.node
        .update_config(|config| {
            config.trust_with_root_node(
                id("acme"),
                before.body.root.public_key.fingerprint(),
                fake.clone(),
            )
        })
        .unwrap();
    let added = add_device(&bob.node, "Laptop", &AccountKey::generate().public(), None).await;
    assert!(matches!(&added[..], [(_, Added::Failed(_))]), "{added:?}");
    let after = bob.node.store().load_org(&id("acme")).unwrap();
    assert_eq!(after.body.version, before.body.version);
    assert_eq!(after.body.label, "Acme");
}
