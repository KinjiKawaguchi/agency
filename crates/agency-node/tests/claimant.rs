//! A person who signed in to Access with the address of a claim link reaches the claim routes and the pages, and
//! nothing else; the owner and a guest are what they were; the redeem needs the Access sign-in and the code, not the
//! write token, and each client address gets a few tries a minute. Alice's node is behind Access.
mod common;

use agency_core::claims;
use agency_core::seats;
use agency_proto::seat::Seat;
use agency_proto::AccountKey;
use common::access::{access_token, behind_access_with_runtime, PUBLIC_HOST};
use common::{id, Running, TOKEN};
use reqwest::Method;
use serde_json::{json, Value};

const CAROL: &str = "carol@example.com";
const OWNER: &str = "alice@example.com";

fn seat(raw: &str, label: &str) -> Seat {
    Seat {
        id: id(raw),
        label: label.into(),
        department: None,
        ask_about: vec![],
        relays: vec![id("alice")],
    }
}

/// Alice's org with carol's seat and a link for carol's address; the code of the link.
async fn with_a_link() -> (Running, String) {
    let (alice, _) = behind_access_with_runtime(Some("ABCDE12345.dev.example.agency")).await;
    seats::add_seat(&alice.node, &id("acme"), seat("carol", "Carol (finance)")).unwrap();
    let invite = claims::seat_invite(&alice.node, &id("acme"), id("carol"), CAROL).unwrap();
    let code = invite.link.split("code=").nth(1).unwrap().to_owned();
    (alice, code)
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

fn redeem_body(code: &str, key: &AccountKey) -> Value {
    json!({ "code": code, "publicKey": key.public().encoded(), "label": "Firefox on Windows" })
}

/// A path of the route table with its parameters filled in.
fn filled(pattern: &str) -> String {
    pattern
        .replace("{org}", "acme")
        .replace("{team}", "app")
        .replace("{token}", &"ab12".repeat(16))
        .replace("{id}", "x")
}

#[tokio::test]
async fn a_claimant_redeems_with_the_write_token_and_asks_where_it_stands() {
    let (alice, code) = with_a_link().await;
    let key = AccountKey::from_seed([0x71; 32]);

    let (status, answer) = call(&alice, Method::GET, "/api/claim", CAROL, None, false).await;
    assert_eq!(status, 404, "nothing redeemed yet: {answer}");

    let without = call(
        &alice,
        Method::POST,
        "/api/claim",
        CAROL,
        Some(redeem_body(&code, &key)),
        false,
    )
    .await;
    assert_eq!(without.0, 403, "a write needs the token: {}", without.1);

    let (status, answer) = call(
        &alice,
        Method::POST,
        "/api/claim",
        CAROL,
        Some(redeem_body(&code, &key)),
        true,
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer,
        json!({
            "state": "waiting",
            "seat": "Carol (finance)",
            "org": "Acme",
            "pairingCode": key.public().pairing_code(),
        })
    );
    let (status, again) = call(&alice, Method::GET, "/api/claim", CAROL, None, false).await;
    assert_eq!(
        (status, again),
        (200, answer),
        "still a claimant while it waits"
    );

    let (status, spent) = call(
        &alice,
        Method::POST,
        "/api/claim",
        CAROL,
        Some(redeem_body(&code, &AccountKey::from_seed([0x72; 32]))),
        true,
    )
    .await;
    assert_eq!(status, 404, "the link is spent: {spent}");
}

#[tokio::test]
async fn the_statuses_of_a_redeem_come_through_the_route() {
    let (alice, code) = with_a_link().await;
    seats::add_seat(&alice.node, &id("acme"), seat("dan", "Dan (legal)")).unwrap();
    claims::seat_invite(&alice.node, &id("acme"), id("dan"), "dan@example.com").unwrap();
    let key = AccountKey::from_seed([0x71; 32]);
    let post = |email: &'static str, body: Value| {
        let alice = &alice;
        async move { call(alice, Method::POST, "/api/claim", email, Some(body), true).await }
    };

    let (status, answer) = post("dan@example.com", redeem_body(&code, &key)).await;
    assert_eq!(status, 403, "another address's link: {answer}");
    assert!(!answer.to_string().contains("carol"), "{answer}");

    let (status, answer) = post(CAROL, redeem_body("nope", &key)).await;
    assert_eq!(status, 404, "an unknown code: {answer}");

    let mut bad_key = redeem_body(&code, &key);
    bad_key["publicKey"] = json!("not a key");
    assert_eq!(post(CAROL, bad_key).await.0, 400, "a key that is none");

    let mut bad_label = redeem_body(&code, &key);
    bad_label["label"] = json!("");
    assert_eq!(post(CAROL, bad_label).await.0, 400, "an empty label");

    let mut extra = redeem_body(&code, &key);
    extra["email"] = json!("someone@example.com");
    assert_eq!(
        post(CAROL, extra).await.0,
        400,
        "a field the body has no place for"
    );

    let (status, answer) = post(CAROL, json!({ "code": code })).await;
    assert_eq!(status, 400, "a body that lacks fields: {answer}");

    let response = reqwest::Client::new()
        .post(format!("{}/api/claim", alice.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", access_token(CAROL, |_| {}))
        .header("x-agency-token", TOKEN)
        .header("content-type", "text/plain")
        .body(redeem_body(&code, &key).to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 415, "the body must be JSON");

    assert_eq!(
        post(CAROL, redeem_body(&code, &key)).await.0,
        200,
        "none of the refusals spent the link"
    );
}

#[tokio::test]
async fn the_owner_and_a_guest_are_never_claimants_and_a_stranger_is_nobody() {
    let (alice, _) = with_a_link().await;
    // A link made for the owner's own address, and for a hosted member's.
    seats::add_seat(&alice.node, &id("acme"), seat("erin", "Erin")).unwrap();
    seats::add_seat(&alice.node, &id("acme"), seat("gus", "Gus")).unwrap();
    let for_owner = claims::seat_invite(&alice.node, &id("acme"), id("erin"), OWNER).unwrap();
    let owners_code = for_owner.link.split("code=").nth(1).unwrap().to_owned();
    // Redeemed in core (the owner could not do it through the route): the owner's own address then has a claim.
    let owners_key = AccountKey::from_seed([0x73; 32]).public().encoded();
    claims::redeem(&alice.node, &owners_code, OWNER, &owners_key, "Phone")
        .await
        .unwrap();
    claims::seat_invite(&alice.node, &id("acme"), id("gus"), "dan@example.com").unwrap();
    let laptop = AccountKey::from_seed([0x81; 32]);
    let device = agency_proto::org::Device {
        id: laptop.public().device_id(),
        label: "Laptop".into(),
        public_key: laptop.public(),
        added_at: agency_proto::Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    agency_core::orgs::amend(&alice.node, &id("acme"), |body| {
        body.people.push(agency_proto::org::Person::hosted(
            id("dan"),
            "Dan",
            vec![device],
        ));
        Ok(())
    })
    .unwrap();
    agency_core::guests::keep(
        &alice.node,
        &agency_core::guests::Guest::new("dan@example.com", id("acme"), id("dan")),
    )
    .unwrap();
    let (status, me) = call(&alice, Method::GET, "/api/me", OWNER, None, false).await;
    assert_eq!(status, 200, "the owner is the owner: {me}");
    assert!(me.get("guest").is_none(), "{me}");
    let (status, answer) = call(&alice, Method::GET, "/api/claim", OWNER, None, false).await;
    assert_eq!(
        status, 404,
        "the owner is no claimant, whatever claims its address has: {answer}"
    );

    let (status, me) = call(
        &alice,
        Method::GET,
        "/api/me",
        "dan@example.com",
        None,
        false,
    )
    .await;
    assert_eq!(status, 200, "a guest is a guest: {me}");
    assert!(me.get("guest").is_some(), "{me}");
    let (status, answer) = call(
        &alice,
        Method::GET,
        "/api/claim",
        "dan@example.com",
        None,
        false,
    )
    .await;
    assert_eq!(status, 403, "a guest reaches no claim route: {answer}");

    let (status, answer) = call(
        &alice,
        Method::GET,
        "/api/claim",
        "mallory@example.com",
        None,
        false,
    )
    .await;
    assert_eq!(status, 403, "an address with no link is refused: {answer}");
    assert!(!answer.to_string().contains("mallory"), "{answer}");
}

#[tokio::test]
async fn every_route_outside_the_claimant_table_answers_a_claimant_403() {
    let (alice, _) = with_a_link().await;
    let table = agency_adapters::client_api::claimant_routes();
    let others: Vec<(&str, &str)> = agency_adapters::client_api::routes_behind_the_guard()
        .into_iter()
        .filter(|route| !table.contains(route))
        .collect();
    assert!(others.len() >= 12, "{others:?}");
    for (method, pattern) in others {
        let method = Method::from_bytes(method.as_bytes()).unwrap();
        let path = filled(pattern);
        for with_token in [true, false] {
            let body = (method != Method::GET).then(|| json!({}));
            let (status, answer) =
                call(&alice, method.clone(), &path, CAROL, body, with_token).await;
            assert_eq!(
                status, 403,
                "a claimant: {method} {path} (token {with_token}): {answer}"
            );
        }
    }
    for (method, path) in [
        (Method::PUT, "/api/claim"),
        (Method::DELETE, "/api/claim"),
        (Method::GET, "/api/claim/extra"),
        (Method::GET, "/metrics"),
    ] {
        let body = (method == Method::PUT).then(|| json!({}));
        let (status, answer) = call(&alice, method.clone(), path, CAROL, body, true).await;
        assert_eq!(status, 403, "a claimant: {method} {path}: {answer}");
    }
    let page = reqwest::Client::new()
        .get(format!("{}/", alice.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", access_token(CAROL, |_| {}))
        .send()
        .await
        .unwrap();
    assert_eq!(page.status(), 200, "the claim page loads");
}

#[tokio::test]
async fn a_client_address_gets_twenty_tries_a_minute_even_with_the_right_code() {
    let (alice, code) = with_a_link().await;
    let key = AccountKey::from_seed([0x71; 32]);
    for tried in 0..20 {
        let (status, answer) = call(
            &alice,
            Method::POST,
            "/api/claim",
            CAROL,
            Some(redeem_body("wrongwrongwrongwrongwr", &key)),
            true,
        )
        .await;
        assert_eq!(status, 404, "try {tried}: {answer}");
    }
    let response = reqwest::Client::new()
        .post(format!("{}/api/claim", alice.url()))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", access_token(CAROL, |_| {}))
        .header("x-agency-token", TOKEN)
        .json(&redeem_body(&code, &key))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429, "the 21st try, with the right code");
    assert!(response.headers().get("retry-after").is_some());
}
