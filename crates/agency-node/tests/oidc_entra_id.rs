//! A node behind Microsoft Entra ID, end to end against the test issuer shaped as Entra ID is: the issuer is
//! `<origin>/<tenant id>/v2.0`, the key set is at `<origin>/<tenant id>/discovery/v2.0/keys`, the audience is
//! the application's client id, the subject is the one Entra ID gives this application, and the tokens carry
//! an email with no `email_verified`. So the owner is let in by its subject, and a guest is refused. Every id
//! here is made up.
mod common;

use agency_core::oidc_config::OidcConfig;
use common::access::PUBLIC_HOST;
use common::guests::{add_carol, CAROL};
use common::oidc_issuer::{behind_oidc, jwk, key_set, token, Answer, Issuer};
use common::TOKEN;
use reqwest::Method;
use serde_json::{json, Value};

/// The directory (tenant) id of the made-up tenant.
const TENANT: &str = "0000aaaa-1111-2222-3333-444455556666";
/// The application (client) id of the node's made-up registration: the audience of its ID tokens.
const CLIENT_ID: &str = "11111111-2222-3333-4444-555555555555";
/// The owner's subject as Entra ID gives it to this one application (it differs from the object id `oid`).
const OWNER_SUB: &str = "pA6w3x1kHs9QYqJ0vT2mLbE4rUcN8dF5gZ7iK0oWnXs";
const OWNER_OID: &str = "22222222-3333-4444-5555-666666666666";

fn entra_issuer(issuer: &Issuer) -> String {
    format!("{}/{TENANT}/v2.0", issuer.origin())
}

/// The test issuer answering as Entra ID's tenant-specific endpoints do: discovery under the issuer, and the
/// key set at the tenant's own path, which is not under the issuer.
fn shaped_as_entra_id(issuer: &Issuer) {
    let keys = format!("{}/{TENANT}/discovery/v2.0/keys", issuer.origin());
    issuer.answer(
        &format!("/{TENANT}/v2.0/.well-known/openid-configuration"),
        Answer::Json(json!({ "issuer": entra_issuer(issuer), "jwks_uri": keys })),
    );
    issuer.answer(
        &format!("/{TENANT}/discovery/v2.0/keys"),
        Answer::Json(key_set(vec![jwk("k1", common::access::TEST_KEY_N)])),
    );
}

/// The node's setting for the tenant: its issuer, the client id, and the owner's subject.
fn setting(issuer: &Issuer) -> impl FnOnce(OidcConfig) -> OidcConfig {
    let issuer = entra_issuer(issuer);
    move |_| OidcConfig::new(PUBLIC_HOST, issuer, CLIENT_ID, OWNER_SUB)
}

/// An ID token as Entra ID signs one: `ver` 2.0, the tenant, the object id, a sign-in name and an email, and no
/// `email_verified`.
fn id_token(issuer: &Issuer, sub: &str, oid: &str, email: &str) -> String {
    let (iss, tenant) = (entra_issuer(issuer), TENANT.to_owned());
    let (oid, email) = (oid.to_owned(), email.to_owned());
    token(&iss, CLIENT_ID, sub, None, "k1", move |claims| {
        claims["ver"] = json!("2.0");
        claims["tid"] = json!(tenant);
        claims["oid"] = json!(oid);
        claims["preferred_username"] = json!(email);
        claims["email"] = json!(email);
        claims["name"] = json!("A Person");
    })
}

/// A request through the gateway, as `jwt`, with the write token.
async fn call(
    running: &common::Running,
    method: Method,
    path: &str,
    jwt: String,
    body: Option<Value>,
) -> (u16, Value) {
    let mut request = reqwest::Client::new()
        .request(method, format!("{}{path}", running.url()))
        .header("host", PUBLIC_HOST)
        .header("authorization", format!("Bearer {jwt}"))
        .header("x-agency-token", TOKEN);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn the_owner_signs_in_by_the_subject_of_this_application_and_writes() {
    let issuer = Issuer::start().await;
    shaped_as_entra_id(&issuer);
    let (running, _) = behind_oidc(&issuer, setting(&issuer)).await;
    let owner = || id_token(&issuer, OWNER_SUB, OWNER_OID, "alice@example.com");

    let (status, answer) = call(&running, Method::GET, "/api/me", owner(), None).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["account"]["id"], "alice");
    assert_eq!(
        issuer.asked(),
        [
            format!("/{TENANT}/v2.0/.well-known/openid-configuration"),
            format!("/{TENANT}/discovery/v2.0/keys"),
        ],
        "discovery under the issuer, then the tenant's key set"
    );
    let write = json!({ "token": "ab12".repeat(16), "environment": "sandbox", "label": "iPhone" });
    let (status, answer) = call(
        &running,
        Method::POST,
        "/api/push/tokens",
        owner(),
        Some(write),
    )
    .await;
    assert_eq!(status, 200, "the owner writes: {answer}");

    let (status, _) = call(
        &running,
        Method::GET,
        "/api/me",
        id_token(&issuer, OWNER_OID, OWNER_OID, "alice@example.com"),
        None,
    )
    .await;
    assert_eq!(
        status, 403,
        "the object id is not the subject: a token whose sub is the owner's oid is not the owner's"
    );
}

#[tokio::test]
async fn a_guest_is_refused_since_the_tokens_carry_no_email_verified() {
    let issuer = Issuer::start().await;
    shaped_as_entra_id(&issuer);
    let (running, memory) = behind_oidc(&issuer, setting(&issuer)).await;
    add_carol(&running, &memory);
    let carol = id_token(
        &issuer,
        "cR0lsUbJ3cT9xYzQ2wErTyUiOpAsDfGhJkLzXcVbNm1",
        "33333333-4444-5555-6666-777777777777",
        CAROL,
    );

    let (status, answer) = call(&running, Method::GET, "/api/me", carol.clone(), None).await;
    assert_eq!(
        status, 403,
        "a registered guest's address, unverified, names no guest: {answer}"
    );
    let claims_of = |jwt: &str| -> Value {
        use base64::Engine;
        let part = jwt.split('.').nth(1).unwrap();
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(part)
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    };
    let claims = claims_of(&carol);
    assert_eq!(claims["email"], CAROL);
    assert!(claims.get("email_verified").is_none(), "{claims}");

    let edov = token(
        &entra_issuer(&issuer),
        CLIENT_ID,
        "cR0lsUbJ3cT9xYzQ2wErTyUiOpAsDfGhJkLzXcVbNm1",
        None,
        "k1",
        |claims| {
            claims["email"] = json!(CAROL);
            claims["xms_edov"] = json!(true);
        },
    );
    let (status, _) = call(&running, Method::GET, "/api/me", edov, None).await;
    assert_eq!(
        status, 403,
        "the email domain owner claim is not email_verified, and is not read"
    );
}

#[tokio::test]
async fn the_common_endpoint_names_no_issuer_a_token_has_so_it_lets_no_one_in() {
    let issuer = Issuer::start().await;
    shaped_as_entra_id(&issuer);
    // The tenant-independent endpoint's discovery names its issuer with a placeholder, `{tenantid}`.
    let common = format!("{}/common/v2.0", issuer.origin());
    issuer.answer(
        "/common/v2.0/.well-known/openid-configuration",
        Answer::Json(json!({
            "issuer": format!("{}/{{tenantid}}/v2.0", issuer.origin()),
            "jwks_uri": format!("{}/common/discovery/v2.0/keys", issuer.origin()),
        })),
    );
    let (running, _) = behind_oidc(&issuer, move |_| {
        OidcConfig::new(PUBLIC_HOST, common, CLIENT_ID, OWNER_SUB)
    })
    .await;

    let owner = id_token(&issuer, OWNER_SUB, OWNER_OID, "alice@example.com");
    let (status, _) = call(&running, Method::GET, "/api/me", owner, None).await;
    assert_eq!(status, 403, "the tenant's own issuer is the one to set");
    assert_eq!(
        issuer.times_asked("/common/discovery/v2.0/keys"),
        0,
        "the key set of a discovery document for another issuer is never read"
    );
}
