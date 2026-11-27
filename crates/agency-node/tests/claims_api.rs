//! The owner's side of a claim over the Client API: the claims that wait are listed for the Devices screen to compare
//! their pairing codes, and confirmed or refused, as `agency claims` does at the terminal. Only the owner, with the
//! write token: a guest and a claimant are refused. Alice's node is behind Access, with carol as a guest.
mod common;

use agency_core::{claims, seats};
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

#[tokio::test]
async fn a_root_key_that_asks_for_presence_is_confirmed_at_the_terminal_and_never_from_the_client()
{
    let alice = with_a_waiting_claim().await;
    let before = version(&alice);
    alice.root_keys.ask_presence(true);
    let code = json!({ "pairingCode": phone().public().pairing_code() });
    let (status, answer) = owner(
        &alice,
        Method::POST,
        "/api/claims/acme.dave/confirm",
        Some(code),
    )
    .await;
    assert_eq!(status, 409, "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .unwrap()
            .contains("agency claim-confirm"),
        "{answer}"
    );
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
}
