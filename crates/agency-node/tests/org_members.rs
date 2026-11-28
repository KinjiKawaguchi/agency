//! The owner reads an org's members, their devices, its open seats and external parties, and the changes that
//! wait for the root key, in one answer (`GET /api/orgs/<org>/members`). Carol is a member alice's node
//! hosts, with a sign-in; a guest and any other sign-in are refused.
mod common;

use agency_core::guests::{self, Guest};
use agency_core::invites::{invite, join};
use agency_core::orgs;
use agency_core::pending_root::{PendingRootChange, RootChange};
use agency_proto::device::{DeviceChange, DeviceOp};
use agency_proto::org::{Device, Role};
use agency_proto::org_members::OrgMembers;
use agency_proto::seat::{External, Seat};
use agency_proto::{Kind, PublicKey, Signed, Timestamp};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use common::guests::{call, with_guest, CAROL};
use common::{alice, body, get, id, memory_runtime, node, Running};
use p256::ecdsa::SigningKey;
use reqwest::Method;
use serde_json::{json, Value};

const MEMBERS: &str = "/api/orgs/acme/members";
/// A browser's P-256 device key.
fn browser_key() -> PublicKey {
    let key = SigningKey::from_slice(&[0x0b; 32]).unwrap();
    let point = key.verifying_key().to_sec1_point(false);
    PublicKey::try_from(format!("p256:{}", STANDARD.encode(point.as_bytes()))).unwrap()
}

/// The owner's direct read of the members.
async fn read(running: &Running) -> (u16, Value) {
    let (status, answer) = body(get(running, MEMBERS).send().await.unwrap()).await;
    (status.as_u16(), answer)
}

/// Carol also has a browser (a P-256 key), and the org has a seat and an external party.
fn fill_the_org(alice: &Running) {
    let browser = Device {
        id: id("browser"),
        label: "Firefox".into(),
        public_key: browser_key(),
        added_at: Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    orgs::amend(&alice.node, &id("acme"), |body| {
        let carol = body.people.iter_mut().find(|one| one.id == id("carol"));
        carol.unwrap().devices.push(browser);
        body.seats.push(Seat {
            id: id("dan"),
            label: "Dan (finance)".into(),
            department: None,
            unit: None,
            ask_about: vec!["invoices".into()],
            relays: vec![id("carol")],
        });
        body.externals.push(External {
            id: id("erin"),
            label: "Erin (outside counsel)".into(),
            ask_about: vec!["contracts".into()],
            relays: vec![id("alice")],
        });
        Ok(())
    })
    .unwrap();
}

/// A device removal for carol that waits for the root key, in `org`.
fn hold_a_device_removal(alice: &Running, org: &str, id_of_change: &str) {
    let key = agency_proto::AccountKey::from_seed([0x81; 32]);
    let change = DeviceChange {
        org: id(org),
        account: id("carol"),
        op: DeviceOp::Remove {
            device: key.public().device_id(),
        },
        at: Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    let signed = Signed::sign(Kind::Device, change, key.public().device_id(), &key).unwrap();
    let held = PendingRootChange {
        id: id_of_change.into(),
        org: id(org),
        received_at: alice.node.now_text(),
        order: 0,
        change: RootChange::Device {
            change: Box::new(signed),
        },
        signed_version: None,
    };
    alice.node.store().put_pending_root(&held).unwrap();
}

#[tokio::test]
async fn the_owner_reads_the_whole_list() {
    let (alice, _) = with_guest().await;
    fill_the_org(&alice);
    hold_a_device_removal(&alice, "acme", "held-1");
    hold_a_device_removal(&alice, "other", "held-2");

    let (status, answer) = read(&alice).await;

    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["org"], "acme");
    let members: Vec<(&str, &str, bool)> = answer["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| {
            (
                one["id"].as_str().unwrap(),
                one["role"].as_str().unwrap(),
                one["isHosted"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        members,
        [("alice", "member", false), ("carol", "member", true)]
    );
    let carol = &answer["members"][1];
    assert_eq!(carol["label"], "Carol");
    assert_eq!(carol["hasSignIn"], true);
    let kinds: Vec<&str> = carol["devices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["keyKind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["ed25519", "p256"]);
    assert_eq!(carol["devices"][1]["label"], "Firefox");
    assert!(carol["devices"][1]["addedAt"].is_string());
    assert!(
        answer["members"][0].get("hasSignIn").is_none(),
        "only a hosted member has the field"
    );
    assert_eq!(
        answer["seats"],
        json!([{ "id": "dan", "label": "Dan (finance)", "askAbout": ["invoices"], "answeredBy": ["carol"] }])
    );
    assert_eq!(
        answer["externals"],
        json!([{ "id": "erin", "label": "Erin (outside counsel)", "askAbout": ["contracts"], "answeredBy": ["alice"] }])
    );
    let held = answer["held"].as_array().unwrap();
    assert_eq!(held.len(), 1, "only the change of this org: {answer}");
    assert_eq!(held[0]["kind"], "removeDevice");
    assert_eq!(held[0]["member"], "carol");
    let parsed: OrgMembers = serde_json::from_value(answer).unwrap();
    assert_eq!(parsed.members.len(), 2);
}

#[tokio::test]
async fn nothing_waiting_leaves_held_out_and_a_hosted_member_without_a_sign_in_says_so() {
    let (alice, _) = with_guest().await;
    alice.node.store().remove_guest(CAROL).unwrap();
    // A record of a member of the same id in another org is no sign-in for carol of this one.
    let elsewhere = Guest::new("carol@elsewhere.example", id("other"), id("carol"));
    guests::keep(&alice.node, &elsewhere).unwrap();

    let (status, answer) = read(&alice).await;

    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["members"][1]["hasSignIn"], false);
    assert!(answer.get("held").is_none(), "{answer}");
}

#[tokio::test]
async fn a_guests_address_appears_nowhere() {
    let (alice, _) = with_guest().await;
    fill_the_org(&alice);
    hold_a_device_removal(&alice, "acme", "held-1");

    let (status, answer) = read(&alice).await;

    assert_eq!(status, 200);
    let text = answer.to_string();
    for part in [CAROL, "carol@", "example.com"] {
        assert!(!text.contains(part), "{part} is in {text}");
    }
    assert_eq!(
        guests::admit(&alice.node, CAROL).map(|guest| guest.member),
        Some(id("carol")),
        "the record is there; the answer only says it is"
    );
}

#[tokio::test]
async fn a_guest_and_another_sign_in_are_refused_and_the_owner_through_access_reads() {
    let (alice, _) = with_guest().await;
    for who in [CAROL, "dan@example.com"] {
        let (status, answer) = call(&alice, Method::GET, MEMBERS, who, None).await;
        assert_eq!(status, 403, "{who}: {answer}");
    }
    let (status, answer) = call(&alice, Method::GET, MEMBERS, "alice@example.com", None).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["members"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn an_answerer_is_refused_and_an_org_the_owner_is_not_in_is_not_found() {
    let (alice, _) = alice().await;
    let link = invite(&alice.node, &id("acme"), None, Role::Answerer).unwrap();
    let cara = node("cara", memory_runtime().0).await;
    join(&cara.node, &link, |_| true).await.unwrap();

    let (status, answer) = read(&cara).await;
    assert_eq!(status, 403, "an answerer: {answer}");

    let (status, answer) = body(get(&alice, "/api/orgs/other/members").send().await.unwrap()).await;
    assert_eq!(status, 404, "{answer}");
}
