//! A node behind an OpenID Connect provider, end to end (the tests of oidc_sign_in.rs use an in-process key
//! source; these go through the real fetch): the node's real key fetch (discovery, then the key
//! set, over TLS, with no redirect and a 10 s limit) against a test issuer, and the same admission as Access:
//! only the owner and a current guest, default-deny.
mod common;

use agency_proto::clock::Duration;
use common::access::{PUBLIC_HOST, TEST_KEY_N};
use common::guests::{add_carol, CAROL};
use common::oidc_issuer::{
    behind_oidc, jwk, key_set, token, token_signed_with, Answer, Issuer, AUDIENCE, ISSUER_PATH,
    ROTATED_KEY_DER, ROTATED_KEY_N,
};
use common::{Running, TOKEN};
use reqwest::Method;
use serde_json::{json, Value};

/// The key set's address that discovery names, made from the issuer.
type Address = Box<dyn Fn(&Issuer) -> String>;

const OWNER: &str = "alice@example.com";
const T0: i64 = 1_795_521_600;
const DISCOVERY: &str = "/realms/acme/.well-known/openid-configuration";
const KEYS: &str = "/realms/acme/keys";

fn owner_token(issuer: &Issuer, edit: impl FnOnce(&mut Value)) -> String {
    token(&issuer.issuer(), AUDIENCE, OWNER, "k1", edit)
}

/// `GET /api/me` as a request that came through the gateway: the public host name, and `header` as it is.
async fn status(running: &Running, header: Option<(&str, String)>) -> u16 {
    let mut request = reqwest::Client::new()
        .get(format!("{}/api/me", running.url()))
        .header("host", PUBLIC_HOST);
    if let Some((name, value)) = header {
        request = request.header(name, value);
    }
    request.send().await.unwrap().status().as_u16()
}

async fn bearer(running: &Running, token: String) -> u16 {
    status(running, Some(("authorization", format!("Bearer {token}")))).await
}

#[tokio::test]
async fn the_owner_is_let_in_with_keys_found_by_discovery_and_kept() {
    let issuer = Issuer::start().await;
    let (running, _) = behind_oidc(&issuer, |setting| setting).await;

    assert_eq!(bearer(&running, owner_token(&issuer, |_| {})).await, 200);
    assert_eq!(
        issuer.asked(),
        [DISCOVERY, KEYS],
        "discovery, then the keys"
    );
    assert_eq!(bearer(&running, owner_token(&issuer, |_| {})).await, 200);
    assert_eq!(
        issuer.asked().len(),
        2,
        "a key that is known is not fetched again"
    );
}

#[tokio::test]
async fn a_key_set_address_in_the_setting_is_used_and_discovery_is_not_asked() {
    let issuer = Issuer::start().await;
    issuer.answer(
        "/elsewhere/keys",
        Answer::Json(key_set(vec![jwk("k1", TEST_KEY_N)])),
    );
    issuer.answer(KEYS, Answer::Status(404));
    let keys_url = format!("{}/elsewhere/keys", issuer.origin());
    let (running, _) = behind_oidc(&issuer, |setting| setting.with_keys_url(keys_url)).await;

    assert_eq!(bearer(&running, owner_token(&issuer, |_| {})).await, 200);
    assert_eq!(issuer.asked(), ["/elsewhere/keys"]);
}

#[tokio::test]
async fn only_a_token_with_every_check_passing_is_let_in() {
    let issuer = Issuer::start().await;
    let (running, _) = behind_oidc(&issuer, |setting| setting).await;
    let ok = |email: &str, kid: &str, edit: &dyn Fn(&mut Value)| {
        token(&issuer.issuer(), AUDIENCE, email, kid, |claims| {
            edit(claims)
        })
    };
    let cases: Vec<(&str, String, u16)> = vec![
        ("the owner", ok(OWNER, "k1", &|_| {}), 200),
        (
            "the owner's email in other letters",
            ok("Alice@Example.COM", "k1", &|_| {}),
            200,
        ),
        (
            "another email",
            ok("mallory@example.com", "k1", &|_| {}),
            403,
        ),
        (
            "no email",
            ok(OWNER, "k1", &|c| {
                c.as_object_mut().unwrap().remove("email");
            }),
            403,
        ),
        (
            "another audience",
            ok(OWNER, "k1", &|c| c["aud"] = json!("someone-else")),
            403,
        ),
        (
            "the audience among others",
            ok(OWNER, "k1", &|c| c["aud"] = json!(["other", AUDIENCE])),
            200,
        ),
        (
            "another issuer",
            ok(OWNER, "k1", &|c| {
                c["iss"] = json!("https://evil.example.test")
            }),
            403,
        ),
        (
            "the issuer with a slash more",
            ok(OWNER, "k1", &|c| {
                c["iss"] = json!(format!("{}/", issuer.issuer()))
            }),
            403,
        ),
        (
            "expired more than a minute ago",
            ok(OWNER, "k1", &|c| c["exp"] = json!(T0 - 61)),
            403,
        ),
        (
            "expired a moment ago, inside the minute allowed",
            ok(OWNER, "k1", &|c| c["exp"] = json!(T0 - 30)),
            200,
        ),
        (
            "not valid for more than a minute",
            ok(OWNER, "k1", &|c| c["nbf"] = json!(T0 + 61)),
            403,
        ),
        (
            "no expiry",
            ok(OWNER, "k1", &|c| {
                c.as_object_mut().unwrap().remove("exp");
            }),
            403,
        ),
        (
            "a key the provider does not publish",
            ok(OWNER, "k9", &|_| {}),
            403,
        ),
        (
            "signed by another key than its kid says",
            token_signed_with(
                ROTATED_KEY_DER,
                &issuer.issuer(),
                AUDIENCE,
                OWNER,
                "k1",
                |_| {},
            ),
            403,
        ),
        ("garbage", "not.a.token".into(), 403),
    ];
    for (what, jwt, wanted) in cases {
        assert_eq!(bearer(&running, jwt).await, wanted, "{what}");
    }
}

#[tokio::test]
async fn a_token_signed_another_way_than_rs256_is_refused() {
    let issuer = Issuer::start().await;
    let (running, _) = behind_oidc(&issuer, |setting| setting).await;
    let claims = json!({
        "iss": issuer.issuer(), "aud": AUDIENCE, "exp": T0 + 600, "nbf": T0 - 60, "email": OWNER,
    });

    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    header.kid = Some("k1".into());
    let hs256 = jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(TEST_KEY_N.as_bytes()),
    )
    .unwrap();
    assert_eq!(
        bearer(&running, hs256).await,
        403,
        "HS256 with the public key as the secret"
    );

    use base64::Engine as _;
    let part =
        |value: Value| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value.to_string());
    let none = format!(
        "{}.{}.",
        part(json!({ "alg": "none", "typ": "JWT", "kid": "k1" })),
        part(claims)
    );
    assert_eq!(bearer(&running, none).await, 403, "alg none");
}

#[tokio::test]
async fn a_request_without_the_token_where_the_setting_says_is_refused() {
    let issuer = Issuer::start().await;
    let (running, _) = behind_oidc(&issuer, |setting| setting).await;
    let jwt = owner_token(&issuer, |_| {});

    assert_eq!(status(&running, None).await, 403, "no token at all");
    assert_eq!(
        status(&running, Some(("authorization", format!("Basic {jwt}")))).await,
        403,
        "another scheme"
    );
    assert_eq!(
        status(&running, Some(("authorization", jwt.clone()))).await,
        403,
        "no scheme"
    );
    assert_eq!(
        status(&running, Some(("authorization", "Bearer ".into()))).await,
        403,
        "an empty token"
    );
    assert_eq!(
        status(&running, Some(("x-forwarded-access-token", jwt.clone()))).await,
        403,
        "another header"
    );
    assert_eq!(
        status(&running, Some(("authorization", format!("bearer {jwt}")))).await,
        200,
        "the scheme in other letters"
    );
    assert!(
        issuer.asked().len() <= 2,
        "a refusal for no token fetches no key"
    );
}

#[tokio::test]
async fn another_header_and_another_claim_are_read_as_the_setting_says() {
    let issuer = Issuer::start().await;
    let (running, _) = behind_oidc(&issuer, |setting| {
        setting
            .with_token_header("X-Forwarded-Access-Token")
            .with_email_claim("upn")
    })
    .await;
    let with_upn = |claims: &mut Value| {
        let email = claims.as_object_mut().unwrap().remove("email").unwrap();
        claims["upn"] = email;
    };
    let jwt = owner_token(&issuer, with_upn);

    assert_eq!(
        status(&running, Some(("x-forwarded-access-token", jwt.clone()))).await,
        200,
        "the token alone in the header, the email in `upn`"
    );
    assert_eq!(
        bearer(&running, jwt).await,
        403,
        "the Authorization header is not read when the setting names another"
    );
    assert_eq!(
        status(
            &running,
            Some(("x-forwarded-access-token", owner_token(&issuer, |_| {})))
        )
        .await,
        403,
        "a token with the email in `email` has no `upn`"
    );
}

#[tokio::test]
async fn keys_that_rotate_are_followed_no_more_than_once_a_minute() {
    let issuer = Issuer::start().await;
    let (running, _) = behind_oidc(&issuer, |setting| setting).await;
    let k1 = || owner_token(&issuer, |_| {});
    let k2 = || token(&issuer.issuer(), AUDIENCE, OWNER, "k2", |_| {});

    assert_eq!(bearer(&running, k1()).await, 200);
    assert_eq!(issuer.times_asked(KEYS), 1);

    issuer.publish(vec![jwk("k2", ROTATED_KEY_N)]);
    assert_eq!(
        bearer(&running, k2()).await,
        403,
        "the new key is looked for no sooner than a minute after the last look"
    );
    assert_eq!(issuer.times_asked(KEYS), 1);

    running.clock.advance(Duration::seconds(61));
    assert_eq!(
        bearer(&running, k2()).await,
        200,
        "found at the next unknown key id"
    );
    assert_eq!(issuer.times_asked(KEYS), 2);
    assert_eq!(
        bearer(&running, k1()).await,
        403,
        "the old key left the provider's set, and is gone from the node's"
    );
    assert_eq!(
        issuer.times_asked(KEYS),
        2,
        "and no new look for it, within the minute"
    );

    issuer.publish(vec![jwk("k1", TEST_KEY_N), jwk("k2", ROTATED_KEY_N)]);
    running.clock.advance(Duration::seconds(61));
    assert_eq!(bearer(&running, k1()).await, 200, "a key that came back");
    assert_eq!(issuer.times_asked(KEYS), 3);
    assert_eq!(
        issuer.times_asked(DISCOVERY),
        3,
        "discovery is read again with each look, so a moved key set is followed"
    );
}

#[tokio::test]
async fn a_provider_that_does_not_answer_properly_lets_nobody_in_and_is_not_asked_again_at_once() {
    let issuer = Issuer::start().await;
    issuer.answer(DISCOVERY, Answer::Status(503));
    let (running, _) = behind_oidc(&issuer, |setting| setting).await;

    assert_eq!(bearer(&running, owner_token(&issuer, |_| {})).await, 403);
    assert_eq!(bearer(&running, owner_token(&issuer, |_| {})).await, 403);
    assert_eq!(
        issuer.times_asked(DISCOVERY),
        1,
        "not asked twice within a minute"
    );

    issuer.answer(
        DISCOVERY,
        Answer::Json(json!({ "issuer": issuer.issuer(), "jwks_uri": issuer.keys_url() })),
    );
    assert_eq!(
        bearer(&running, owner_token(&issuer, |_| {})).await,
        403,
        "still within the minute"
    );
    running.clock.advance(Duration::seconds(61));
    assert_eq!(bearer(&running, owner_token(&issuer, |_| {})).await, 200);
}

#[tokio::test]
async fn a_redirect_from_the_provider_is_not_followed() {
    for (what, path, target) in [
        ("discovery", DISCOVERY, format!("{ISSUER_PATH}/moved-keys")),
        ("the key set", KEYS, format!("{ISSUER_PATH}/moved-keys")),
    ] {
        let issuer = Issuer::start().await;
        issuer.answer(
            "/realms/acme/moved-keys",
            Answer::Json(key_set(vec![jwk("k1", TEST_KEY_N)])),
        );
        issuer.answer(
            path,
            Answer::Redirect(format!("{}{target}", issuer.origin())),
        );
        let (running, _) = behind_oidc(&issuer, |setting| setting).await;

        assert_eq!(
            bearer(&running, owner_token(&issuer, |_| {})).await,
            403,
            "{what}"
        );
        assert_eq!(
            issuer.times_asked("/realms/acme/moved-keys"),
            0,
            "{what}: the redirect was not followed"
        );
    }
}

#[tokio::test]
async fn a_key_set_address_in_discovery_that_is_not_https_is_refused_unread() {
    let origin = |issuer: &Issuer| issuer.origin().to_owned();
    let cases: Vec<(&str, Address)> = vec![
        ("http", Box::new(|_| "http://localhost:1/keys".to_owned())),
        ("a file", Box::new(|_| "file:///etc/passwd".to_owned())),
        (
            "user information, at the issuer's own port",
            Box::new(move |i| {
                format!(
                    "https://bob@{}{KEYS}",
                    origin(i).trim_start_matches("https://")
                )
            }),
        ),
        (
            "a query, at the issuer's own address",
            Box::new(move |i| format!("{}{KEYS}?x=1", i.origin())),
        ),
        (
            "a fragment",
            Box::new(move |i| format!("{}{KEYS}#k", i.origin())),
        ),
        ("none", Box::new(|_| String::new())),
    ];
    for (what, uri) in cases {
        let issuer = Issuer::start().await;
        let mut document = json!({ "issuer": issuer.issuer() });
        let uri = uri(&issuer);
        if !uri.is_empty() {
            document["jwks_uri"] = json!(uri);
        }
        issuer.answer(DISCOVERY, Answer::Json(document));
        let (running, _) = behind_oidc(&issuer, |setting| setting).await;

        assert_eq!(
            bearer(&running, owner_token(&issuer, |_| {})).await,
            403,
            "{what}"
        );
        assert_eq!(
            issuer.asked(),
            [DISCOVERY],
            "{what}: nothing but discovery was read"
        );
    }
}

#[tokio::test]
async fn a_guest_is_let_in_to_its_own_routes_and_the_owner_to_all() {
    let issuer = Issuer::start().await;
    let (running, memory) = behind_oidc(&issuer, |setting| setting).await;
    add_carol(&running, &memory);
    let as_carol = || token(&issuer.issuer(), AUDIENCE, CAROL, "k1", |_| {});

    let (status, me) = call(&running, Method::GET, "/api/me", as_carol(), None).await;
    assert_eq!(status, 200, "{me}");
    assert_eq!(me["guest"], json!({ "member": "carol", "org": "acme" }));
    let (status, _) = call(&running, Method::GET, "/api/metrics", as_carol(), None).await;
    assert_eq!(status, 403, "the owner's routes are not a guest's");
    let (status, _) = call(
        &running,
        Method::GET,
        "/api/me",
        token(&issuer.issuer(), AUDIENCE, "dave@example.com", "k1", |_| {}),
        None,
    )
    .await;
    assert_eq!(
        status, 403,
        "an email that is neither the owner's nor a guest's"
    );

    let (status, me) = call(
        &running,
        Method::GET,
        "/api/me",
        owner_token(&issuer, |_| {}),
        None,
    )
    .await;
    assert_eq!(status, 200, "{me}");
    assert_eq!(me["account"]["id"], "alice");
}

/// A request through the gateway, as `jwt`.
async fn call(
    running: &Running,
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
async fn behind_a_provider_a_local_request_reads_but_only_the_signed_in_owner_writes() {
    let issuer = Issuer::start().await;
    let (running, _) = behind_oidc(&issuer, |setting| setting).await;
    let write = json!({ "token": "ab12".repeat(16), "environment": "sandbox", "label": "iPhone" });

    let local_read = common::get(&running, "/api/me")
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(local_read, 200, "a local read is still let in");
    let local = reqwest::Client::new()
        .post(format!("{}/api/push/tokens", running.url()))
        .header("x-agency-token", TOKEN)
        .json(&write)
        .send()
        .await
        .unwrap();
    assert_eq!(local.status(), 403, "a local write is refused");
    let refusal: Value = local.json().await.unwrap();
    assert!(
        refusal["error"]
            .as_str()
            .unwrap()
            .contains("OpenID Connect provider"),
        "{refusal}"
    );

    let (status, answer) = call(
        &running,
        Method::POST,
        "/api/push/tokens",
        owner_token(&issuer, |_| {}),
        Some(write.clone()),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    let (status, _) = call(
        &running,
        Method::POST,
        "/api/push/tokens",
        token(&issuer.issuer(), AUDIENCE, "dave@example.com", "k1", |_| {}),
        Some(write),
    )
    .await;
    assert_eq!(status, 403, "another email's write");
}

#[tokio::test]
async fn a_node_behind_a_provider_tells_browsers_to_use_https_only() {
    let issuer = Issuer::start().await;
    let (running, _) = behind_oidc(&issuer, |setting| setting).await;
    let response = reqwest::Client::new()
        .get(format!("{}/api/me", running.url()))
        .header("host", PUBLIC_HOST)
        .header(
            "authorization",
            format!("Bearer {}", owner_token(&issuer, |_| {})),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["strict-transport-security"],
        "max-age=31536000"
    );
    let refused = reqwest::Client::new()
        .get(format!("{}/api/me", running.url()))
        .header("host", PUBLIC_HOST)
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 403);
    assert!(
        refused.headers().contains_key("strict-transport-security"),
        "a refusal carries it too"
    );
}
