//! The owner's side of a claim over the Client API: the claims that wait are listed for the Devices screen to compare
//! their pairing codes, and confirmed or refused, as `agency claims` does at the terminal. Only the owner, with the
//! write token: a guest and a claimant are refused. Alice's node is behind Access, with carol as a guest.
mod common;

use agency_core::{claims, guests, pending_root, seats};
use agency_proto::seat::Seat;
use agency_proto::AccountKey;
use common::access::{access_token, PUBLIC_HOST};
use common::guests::{with_guest, CAROL};
use common::{id, Running, TOKEN};
use reqwest::Method;
use serde_json::{json, Value};

const OWNER: &str = "alice@example.com";
const DAVE: &str = "dave@example.com";

fn phone() -> AccountKey {
    AccountKey::from_seed([0x71; 32])
}

/// A request signed in as `email`, with the write token or without it.
async fn call(
    running: &Running,
    method: Method,
    path: &str,
    email: &str,
    body: Option<Value>,
    with_token: bool,
) -> (u16, Value) {
    let mut request = reqwest::Client::new()
        .request(method, format!("{}{path}", running.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", access_token(email, |_| {}));
    if with_token {
        request = request.header("x-agency-token", TOKEN);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn owner(running: &Running, method: Method, path: &str, body: Option<Value>) -> (u16, Value) {
    call(running, method, path, OWNER, body, true).await
}

/// Alice's node with carol as a guest, and dave's seat, his link, and a redeem by his browser.
async fn with_a_waiting_claim() -> Running {
    let (alice, _) = with_guest().await;
    let seat = Seat {
        id: id("dave"),
        label: "Dave (finance)".into(),
        department: None,
        ask_about: vec![],
        relays: vec![id("alice")],
    };
    seats::add_seat(&alice.node, &id("acme"), seat).unwrap();
    let invite = claims::seat_invite(&alice.node, &id("acme"), id("dave"), DAVE).unwrap();
    let code = invite.link.split("code=").nth(1).unwrap().to_owned();
    claims::redeem(
        &alice.node,
        &code,
        DAVE,
        &phone().public().encoded(),
        "Firefox on Windows",
    )
    .await
    .unwrap();
    alice
}

fn version(alice: &Running) -> u64 {
    alice
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .version
}

#[tokio::test]
async fn the_owner_lists_what_a_claim_compares_and_never_the_address() {
    let alice = with_a_waiting_claim().await;
    let (status, list) = owner(&alice, Method::GET, "/api/claims", None).await;
    assert_eq!(status, 200, "{list}");
    let claims = list["claims"].as_array().unwrap();
    assert_eq!(claims.len(), 1, "{list}");
    let claim = &claims[0];
    assert_eq!(claim["id"], "acme.dave");
    assert_eq!(
        (claim["org"].as_str(), claim["seat"].as_str()),
        (Some("acme"), Some("dave"))
    );
    assert_eq!(claim["seatLabel"], "Dave (finance)");
    assert_eq!(claim["deviceLabel"], "Firefox on Windows");
    assert_eq!(claim["pairingCode"], phone().public().pairing_code());
    assert!(claim["redeemedAt"].is_string());
    assert!(
        !list.to_string().contains("example.com"),
        "no address: {list}"
    );
}

#[tokio::test]
async fn a_confirm_needs_the_pairing_code_the_node_holds_and_makes_the_seat_a_member() {
    let alice = with_a_waiting_claim().await;
    let before = version(&alice);
    let wrong = json!({ "pairingCode": "0000-0000-0000" });
    let (status, answer) = owner(
        &alice,
        Method::POST,
        "/api/claims/acme.dave/confirm",
        Some(wrong),
    )
    .await;
    assert_eq!(status, 409, "{answer}");
    assert_eq!(version(&alice), before, "a wrong code signs nothing");
    let (_, list) = owner(&alice, Method::GET, "/api/claims", None).await;
    assert_eq!(
        list["claims"].as_array().unwrap().len(),
        1,
        "it still waits"
    );

    let code = phone()
        .public()
        .pairing_code()
        .to_uppercase()
        .replace('-', " ");
    let right = json!({ "pairingCode": code });
    let (status, answer) = owner(
        &alice,
        Method::POST,
        "/api/claims/acme.dave/confirm",
        Some(right.clone()),
    )
    .await;
    assert_eq!(
        status, 200,
        "the code as read out, in capitals and with spaces: {answer}"
    );
    assert_eq!(
        (answer["org"].as_str(), answer["seat"].as_str()),
        (Some("acme"), Some("dave"))
    );
    assert_eq!(answer["version"], version(&alice));
    assert!(version(&alice) > before);

    let (_, list) = owner(&alice, Method::GET, "/api/claims", None).await;
    assert!(
        list["claims"].as_array().unwrap().is_empty(),
        "no longer waiting: {list}"
    );
    // dave is a guest now: he reads himself as his member.
    let (status, me) = call(&alice, Method::GET, "/api/me", DAVE, None, false).await;
    assert_eq!(status, 200, "{me}");
    assert_eq!(me["guest"], json!({ "member": "dave", "org": "acme" }));
    let after = version(&alice);
    let (status, _) = owner(
        &alice,
        Method::POST,
        "/api/claims/acme.dave/confirm",
        Some(right),
    )
    .await;
    assert_eq!(status, 200, "a second confirm answers as the first did");
    assert_eq!(version(&alice), after, "and signs nothing");
}

#[tokio::test]
async fn a_reject_drops_the_claim_and_an_unknown_claim_is_not_found() {
    let alice = with_a_waiting_claim().await;
    let (status, _) = owner(
        &alice,
        Method::POST,
        "/api/claims/acme.dave/reject",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, 204);
    let (_, list) = owner(&alice, Method::GET, "/api/claims", None).await;
    assert!(list["claims"].as_array().unwrap().is_empty());
    let (status, _) = owner(
        &alice,
        Method::POST,
        "/api/claims/acme.dave/reject",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, 404, "none waits any more");
    let code = json!({ "pairingCode": phone().public().pairing_code() });
    for path in [
        "/api/claims/acme.dave/confirm",
        "/api/claims/acme.nobody/confirm",
        "/api/claims/other.dave/confirm",
        "/api/claims/nonsense/confirm",
        "/api/claims/acme.dave.x/confirm",
    ] {
        let (status, answer) = owner(&alice, Method::POST, path, Some(code.clone())).await;
        assert_eq!(status, 404, "{path}: {answer}");
    }
}

#[tokio::test]
async fn only_the_owner_with_the_write_token_reaches_the_claims() {
    let alice = with_a_waiting_claim().await;
    let code = json!({ "pairingCode": phone().public().pairing_code() });
    let routes = [
        (Method::GET, "/api/claims", None),
        (
            Method::POST,
            "/api/claims/acme.dave/confirm",
            Some(code.clone()),
        ),
        (
            Method::POST,
            "/api/claims/acme.dave/reject",
            Some(json!({})),
        ),
    ];
    for (method, path, body) in &routes {
        for (who, email) in [("a guest", CAROL), ("a claimant", DAVE)] {
            let (status, answer) =
                call(&alice, method.clone(), path, email, body.clone(), true).await;
            assert_eq!(status, 403, "{who}: {method} {path}: {answer}");
        }
    }
    let (_, list) = owner(&alice, Method::GET, "/api/claims", None).await;
    assert_eq!(
        list["claims"].as_array().unwrap().len(),
        1,
        "nothing was confirmed or dropped"
    );
    for (method, path, body) in routes.iter().skip(1) {
        let (status, answer) = call(&alice, method.clone(), path, OWNER, body.clone(), false).await;
        assert_eq!(
            status, 403,
            "the owner without the write token: {path}: {answer}"
        );
    }
    let (_, list) = owner(&alice, Method::GET, "/api/claims", None).await;
    assert_eq!(list["claims"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_body_that_is_not_a_confirmation_is_refused() {
    let alice = with_a_waiting_claim().await;
    for body in [
        json!({}),
        json!({ "pairingCode": "x", "seat": "dave" }),
        json!("code"),
    ] {
        let (status, answer) = owner(
            &alice,
            Method::POST,
            "/api/claims/acme.dave/confirm",
            Some(body.clone()),
        )
        .await;
        assert_eq!(status, 400, "{body}: {answer}");
    }
    let (_, list) = owner(&alice, Method::GET, "/api/claims", None).await;
    assert_eq!(list["claims"].as_array().unwrap().len(), 1);
}

/// The handles of the changes that wait for the root key.
fn held(alice: &Running) -> Vec<String> {
    pending_root::waiting(&alice.node)
        .unwrap()
        .into_iter()
        .map(|one| one.id)
        .collect()
}

/// Confirms dave's claim from the owner's client with `code`.
async fn confirm(alice: &Running, code: &str) -> (u16, Value) {
    let body = json!({ "pairingCode": code });
    owner(
        alice,
        Method::POST,
        "/api/claims/acme.dave/confirm",
        Some(body),
    )
    .await
}

#[tokio::test]
async fn with_a_root_key_that_asks_for_presence_a_confirmation_is_held_and_named() {
    let alice = with_a_waiting_claim().await;
    let before = version(&alice);
    alice.root_keys.ask_presence(true);
    let (status, answer) = confirm(&alice, &phone().public().pairing_code()).await;
    assert_eq!(status, 202, "{answer}");
    let handle = answer["pending"].as_str().unwrap().to_owned();
    assert_eq!(held(&alice), std::slice::from_ref(&handle));
    assert_eq!(version(&alice), before, "nothing was signed");
    assert!(
        alice.root_keys.prompts().is_empty(),
        "and no prompt was asked"
    );
    let (_, list) = owner(&alice, Method::GET, "/api/claims", None).await;
    assert_eq!(
        list["claims"].as_array().unwrap().len(),
        1,
        "it still waits"
    );

    let (status, again) = confirm(&alice, &phone().public().pairing_code()).await;
    assert_eq!(status, 202, "{again}");
    assert_eq!(again["pending"], handle, "one confirmation for a seat");
    assert_eq!(held(&alice).len(), 1);

    let (_, me) = owner(&alice, Method::GET, "/api/me", None).await;
    let change = &me["rootChanges"][0];
    assert_eq!(change["kind"], "claimSeat", "{me}");
    assert_eq!(change["member"], "dave");
    assert_eq!(change["label"], "Firefox on Windows");
    assert_eq!(change["device"], phone().public().device_id().to_string());
}

#[tokio::test]
async fn a_wrong_pairing_code_holds_nothing() {
    let alice = with_a_waiting_claim().await;
    alice.root_keys.ask_presence(true);
    let (status, answer) = confirm(&alice, "0000-0000").await;
    assert_eq!(status, 409, "{answer}");
    assert!(held(&alice).is_empty());
}

#[tokio::test]
async fn root_sign_names_the_claim_and_makes_the_seat_a_member_with_the_persons_sign_in() {
    let alice = with_a_waiting_claim().await;
    let before = version(&alice);
    alice.root_keys.ask_presence(true);
    confirm(&alice, &phone().public().pairing_code()).await;

    let run = pending_root::sign_held(&alice.node, &id("acme"), &held(&alice))
        .await
        .unwrap();
    assert_eq!(run.signed.len(), 1, "{:?}", run.dropped);
    let prompts = alice.root_keys.prompts();
    assert_eq!(prompts.len(), 1);
    assert!(
        prompts[0].contains(
            "host seat dave (Dave (finance)) as a member, with device Firefox on Windows"
        ),
        "{prompts:?}"
    );
    assert_eq!(version(&alice), before + 1);
    let doc = alice.node.store().load_org(&id("acme")).unwrap();
    let dave = doc.body.member(&id("dave")).unwrap();
    assert!(dave.is_hosted());
    assert_eq!(dave.devices[0].public_key, phone().public());
    assert!(
        guests::admit(&alice.node, DAVE).is_some(),
        "the person signs in as dave"
    );
    let (_, list) = owner(&alice, Method::GET, "/api/claims", None).await;
    assert!(
        list["claims"].as_array().unwrap().is_empty(),
        "the claim is confirmed"
    );
}

#[tokio::test]
async fn a_claim_refused_while_its_confirmation_waited_is_dropped_at_signing() {
    let alice = with_a_waiting_claim().await;
    let before = version(&alice);
    alice.root_keys.ask_presence(true);
    confirm(&alice, &phone().public().pairing_code()).await;
    claims::reject(&alice.node, &id("acme"), &id("dave"))
        .await
        .unwrap();

    let run = pending_root::sign_held(&alice.node, &id("acme"), &held(&alice))
        .await
        .unwrap();
    assert!(run.signed.is_empty());
    assert_eq!(run.dropped.len(), 1);
    assert!(
        run.dropped[0].1.contains("no longer waits"),
        "{:?}",
        run.dropped
    );
    assert_eq!(version(&alice), before);
    assert!(guests::admit(&alice.node, DAVE).is_none());
}

#[tokio::test]
async fn a_held_confirmation_is_dropped_after_seven_days() {
    let alice = with_a_waiting_claim().await;
    alice.root_keys.ask_presence(true);
    confirm(&alice, &phone().public().pairing_code()).await;
    assert_eq!(held(&alice).len(), 1);
    alice.clock.advance(
        agency_proto::clock::Duration::days(7) + agency_proto::clock::Duration::minutes(1),
    );
    assert!(held(&alice).is_empty());
}

#[tokio::test]
async fn a_held_confirmation_waits_for_every_team_node_to_read_hosted_members() {
    use agency_core::peer_about::PeerAbout;
    use agency_proto::about::About;
    use agency_proto::org::TeamPlace;
    use common::{joined, memory_runtime, node};
    // An org with no hosted member yet, and a team on bob's node, which is kept as not reading one.
    let (alice, _) = common::alice().await;
    let bob = joined(&alice, node("bob", memory_runtime().0).await).await;
    agency_core::orgs::add_team(
        &alice.node,
        &id("acme"),
        TeamPlace::new(id("web"), "Web", bob.url()),
    )
    .unwrap();
    let mut older = About::of_release("0.3.0");
    older.capabilities.retain(|one| one != "hostedMembers");
    let read_at = agency_proto::clock::format(alice.node.now());
    let entry = PeerAbout {
        about: Some(older),
        read_at: Some(read_at.clone()),
        tried_at: read_at,
        ..PeerAbout::default()
    };
    alice
        .node
        .store()
        .save_peer_abouts(
            &id("acme"),
            &std::collections::BTreeMap::from([(bob.url(), entry)]),
        )
        .unwrap();
    let seat = Seat {
        id: id("dave"),
        label: "Dave (finance)".into(),
        department: None,
        ask_about: vec![],
        relays: vec![id("alice")],
    };
    seats::add_seat(&alice.node, &id("acme"), seat).unwrap();
    let invite = claims::seat_invite(&alice.node, &id("acme"), id("dave"), DAVE).unwrap();
    let code = invite.link.split("code=").nth(1).unwrap().to_owned();
    claims::redeem(
        &alice.node,
        &code,
        DAVE,
        &phone().public().encoded(),
        "Firefox",
    )
    .await
    .unwrap();
    alice.root_keys.ask_presence(true);
    let pairing = phone().public().pairing_code();
    let confirmed = claims::confirm_from_client(&alice.node, &id("acme"), &id("dave"), &pairing)
        .await
        .unwrap();
    assert!(
        matches!(confirmed, claims::ClientConfirmation::Held(_)),
        "{confirmed:?}"
    );
    let before = version(&alice);

    let run = pending_root::sign_held(&alice.node, &id("acme"), &held(&alice))
        .await
        .unwrap();
    assert!(run.signed.is_empty());
    assert!(
        run.dropped[0].1.contains("hostedMembers"),
        "{:?}",
        run.dropped
    );
    assert_eq!(version(&alice), before);
}
