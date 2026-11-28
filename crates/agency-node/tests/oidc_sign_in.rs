//! A node behind an OpenID Connect provider (`oidc`): the same checks as Access, with the provider's issuer,
//! audience, and header. The owner is the issuer's subject the setting names; a guest is matched by an email the
//! provider says it verified. The keys come from an in-process source, by the address the node asks for.
mod common;

use agency_adapters::access::CertSource;
use agency_core::guests::{self, Guest};
use agency_core::oidc_config::OidcConfig;
use agency_core::orgs;
use agency_proto::org::{Device, Person};
use agency_proto::{AccountKey, Timestamp};
use async_trait::async_trait;
use common::access::{TEST_KEY_DER, TEST_KEY_N};
use common::{id, memory_runtime, node_with, Running, TOKEN};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const HOST: &str = "agency.example.com";
const ISSUER: &str = "https://login.example.com/tenant/v2.0";
const AUDIENCE: &str = "agency-client";
const KEYS: &str = "https://login.example.com/tenant/keys";
const OWNER: &str = "alice@example.com";
const OWNER_SUBJECT: &str = "sub-alice";
const GUEST: &str = "carol@example.com";
/// 2026-11-24T12:00:00Z, the tests' clock.
const T0: i64 = 1_795_521_600;

/// The provider's documents by address, and the addresses the node asked for.
struct Provider {
    documents: Vec<(String, Value)>,
    asked: Mutex<Vec<String>>,
}

#[async_trait]
impl CertSource for Provider {
    async fn certs(&self, url: &str) -> Result<Value, String> {
        self.asked.lock().unwrap().push(url.to_owned());
        self.documents
            .iter()
            .find(|(at, _)| at == url)
            .map(|(_, document)| document.clone())
            .ok_or_else(|| format!("nothing at {url}"))
    }
}

fn key_set() -> Value {
    json!({ "keys": [{ "kid": "k1", "kty": "RSA", "alg": "RS256", "n": TEST_KEY_N, "e": "AQAB" }] })
}

/// A provider that publishes discovery (naming `jwks_uri`) and the key set at `KEYS`.
fn provider(jwks_uri: &str) -> Arc<Provider> {
    provider_saying(ISSUER, jwks_uri)
}

/// `provider`, whose discovery document names `issuer`.
fn provider_saying(issuer: &str, jwks_uri: &str) -> Arc<Provider> {
    Arc::new(Provider {
        documents: vec![
            (
                format!("{ISSUER}/.well-known/openid-configuration"),
                json!({ "issuer": issuer, "jwks_uri": jwks_uri }),
            ),
            (KEYS.to_owned(), key_set()),
        ],
        asked: Mutex::new(vec![]),
    })
}

fn setting() -> OidcConfig {
    OidcConfig::new(HOST, ISSUER, AUDIENCE, OWNER_SUBJECT)
}

/// alice's node behind `oidc`, with carol a guest of org acme.
async fn behind_oidc(oidc: OidcConfig, source: Arc<Provider>) -> Running {
    let (runtime, _) = memory_runtime();
    let mut running = node_with("alice", runtime, Some(source)).await;
    let node = &running.node;
    orgs::create(node, id("acme"), "Acme".into()).unwrap();
    let laptop = AccountKey::from_seed([0x81; 32]);
    let device = Device {
        id: laptop.public().device_id(),
        label: "Laptop".into(),
        public_key: laptop.public(),
        added_at: Timestamp::parse(&node.now_text()).unwrap(),
    };
    orgs::amend(node, &id("acme"), |body| {
        body.people
            .push(Person::hosted(id("carol"), "Carol", vec![device]));
        Ok(())
    })
    .unwrap();
    guests::keep(node, &Guest::new(GUEST, id("acme"), id("carol"))).unwrap();
    node.update_config(|config| *config = config.clone().with_oidc(oidc))
        .unwrap();
    running.stop().await;
    running.start().await;
    running
}

/// A token of subject `sub` naming `email`, signed by the provider's key, valid around T0 unless `edit` changes
/// its claims.
fn token(sub: &str, email: &str, edit: impl FnOnce(&mut Value)) -> String {
    let mut claims = json!({ "aud": AUDIENCE, "iss": ISSUER, "exp": T0 + 600, "nbf": T0 - 60, "sub": sub, "email": email });
    edit(&mut claims);
    sign(&claims, "k1")
}

/// The owner's token: its subject, and its email, which the provider does not say it verified.
fn owner(edit: impl FnOnce(&mut Value)) -> String {
    token(OWNER_SUBJECT, OWNER, edit)
}

/// carol's token, with an email the provider says it verified.
fn guest(edit: impl FnOnce(&mut Value)) -> String {
    token("sub-carol", GUEST, |c| {
        c["email_verified"] = json!(true);
        edit(c);
    })
}

fn sign(claims: &Value, kid: &str) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(kid.into());
    jsonwebtoken::encode(
        &header,
        claims,
        &jsonwebtoken::EncodingKey::from_rsa_der(TEST_KEY_DER),
    )
    .unwrap()
}

/// The status of `GET /api/me` at the public host with `header: value`.
async fn me_with(running: &Running, header: Option<(&str, String)>) -> u16 {
    let mut request = reqwest::Client::new()
        .get(format!("{}/api/me", running.url()))
        .header("host", HOST);
    if let Some((name, value)) = header {
        request = request.header(name, value);
    }
    request.send().await.unwrap().status().as_u16()
}

async fn me(running: &Running, token: String) -> u16 {
    me_with(running, Some(("authorization", format!("Bearer {token}")))).await
}

#[tokio::test]
async fn the_owner_is_its_subject_and_a_guest_its_verified_email() {
    let running = behind_oidc(setting().with_keys_url(KEYS), provider(KEYS)).await;
    assert_eq!(me(&running, owner(|_| {})).await, 200, "the owner");
    assert_eq!(
        me(
            &running,
            token(OWNER_SUBJECT, "mallory@example.com", |_| {})
        )
        .await,
        200,
        "the owner's subject, whatever email it names"
    );
    assert_eq!(
        me(
            &running,
            token(OWNER_SUBJECT, OWNER, |c| {
                c.as_object_mut().unwrap().remove("email");
            })
        )
        .await,
        200,
        "the owner's subject with no email"
    );
    assert_eq!(
        me(
            &running,
            token("sub-mallory", OWNER, |c| c["email_verified"] = json!(true))
        )
        .await,
        403,
        "the owner's verified email under another subject"
    );
    assert_eq!(me(&running, guest(|_| {})).await, 200, "a guest");
    assert_eq!(
        me(&running, guest(|c| c["email_verified"] = json!("true"))).await,
        200,
        "a guest whose provider writes true as text"
    );
    for (case, unverified) in [
        ("absent", None),
        ("false", Some(json!(false))),
        ("\"false\"", Some(json!("false"))),
        ("null", Some(json!(null))),
        ("0", Some(json!(0))),
    ] {
        let edit = |c: &mut Value| match unverified {
            Some(value) => c["email_verified"] = value,
            None => {
                c.as_object_mut().unwrap().remove("email_verified");
            }
        };
        assert_eq!(
            me(&running, guest(edit)).await,
            403,
            "a guest whose email_verified is {case}"
        );
    }
    assert_eq!(
        me(&running, owner(|c| c["aud"] = json!(["other", AUDIENCE]))).await,
        200,
        "one of several audiences"
    );
}

#[tokio::test]
async fn a_token_that_fails_any_check_is_refused() {
    let running = behind_oidc(setting().with_keys_url(KEYS), provider(KEYS)).await;
    let cases: [(&str, String); 10] = [
        (
            "another subject and email",
            token("sub-mallory", "mallory@example.com", |c| {
                c["email_verified"] = json!(true)
            }),
        ),
        (
            "no subject",
            owner(|c| {
                c.as_object_mut().unwrap().remove("sub");
            }),
        ),
        ("a subject that is not text", owner(|c| c["sub"] = json!(1))),
        ("a wrong audience", owner(|c| c["aud"] = json!("other"))),
        (
            "a wrong issuer",
            owner(|c| c["iss"] = json!("https://login.example.com/other")),
        ),
        ("an expired token", owner(|c| c["exp"] = json!(T0 - 61))),
        (
            "a token not valid yet",
            owner(|c| c["nbf"] = json!(T0 + 61)),
        ),
        (
            "no expiry",
            owner(|c| {
                c.as_object_mut().unwrap().remove("exp");
            }),
        ),
        (
            "an expiry that is a string",
            owner(|c| c["exp"] = json!((T0 + 600).to_string())),
        ),
        (
            "an unknown key",
            sign(
                &json!({ "aud": AUDIENCE, "iss": ISSUER, "exp": T0 + 600, "sub": OWNER_SUBJECT }),
                "k2",
            ),
        ),
    ];
    for (case, token) in cases {
        assert_eq!(me(&running, token).await, 403, "{case}");
    }
    let hs256 = jsonwebtoken::encode(
        &jsonwebtoken::Header {
            kid: Some("k1".into()),
            ..jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256)
        },
        &json!({ "aud": AUDIENCE, "iss": ISSUER, "exp": T0 + 600, "sub": OWNER_SUBJECT }),
        &jsonwebtoken::EncodingKey::from_secret(TEST_KEY_N.as_bytes()),
    )
    .unwrap();
    assert_eq!(me(&running, hs256).await, 403, "a token that is not RS256");
    assert_eq!(me_with(&running, None).await, 403, "no token");
    assert_eq!(
        me_with(
            &running,
            Some(("authorization", format!("Basic {}", owner(|_| {}))))
        )
        .await,
        403,
        "another scheme"
    );
    assert_eq!(
        me_with(&running, Some(("cf-access-jwt-assertion", owner(|_| {})))).await,
        403,
        "the token in Access's header"
    );
}

#[tokio::test]
async fn the_guest_email_and_the_token_come_from_where_the_setting_says() {
    let oidc = setting()
        .with_keys_url(KEYS)
        .with_email_claim("upn")
        .with_token_header("X-Auth-Token");
    let running = behind_oidc(oidc, provider(KEYS)).await;
    let as_upn = guest(|c| {
        c["email"] = json!("mallory@example.com");
        c["upn"] = json!(GUEST);
    });
    assert_eq!(
        me_with(&running, Some(("x-auth-token", as_upn.clone()))).await,
        200,
        "the guest in upn, the token alone in its header"
    );
    assert_eq!(
        me_with(&running, Some(("x-auth-token", guest(|_| {})))).await,
        403,
        "the guest in email only"
    );
    assert_eq!(
        me_with(
            &running,
            Some(("authorization", format!("Bearer {as_upn}")))
        )
        .await,
        403,
        "the token in another header"
    );
}

#[tokio::test]
async fn the_key_set_is_found_through_discovery_and_must_be_https() {
    let source = provider(KEYS);
    let running = behind_oidc(setting(), source.clone()).await;
    assert_eq!(me(&running, owner(|_| {})).await, 200);
    assert_eq!(
        *source.asked.lock().unwrap(),
        [
            format!("{ISSUER}/.well-known/openid-configuration"),
            KEYS.to_owned()
        ]
    );

    let source = provider("http://login.example.com/tenant/keys");
    let running = behind_oidc(setting(), source.clone()).await;
    assert_eq!(me(&running, owner(|_| {})).await, 403, "an http jwks_uri");
    assert!(
        !source
            .asked
            .lock()
            .unwrap()
            .iter()
            .any(|url| url.starts_with("http:")),
        "the http address is never read"
    );
    let source = provider_saying("https://login.example.com/other", KEYS);
    let running = behind_oidc(setting(), source.clone()).await;
    assert_eq!(
        me(&running, owner(|_| {})).await,
        403,
        "a discovery document of another issuer"
    );
    assert!(
        !source.asked.lock().unwrap().iter().any(|url| url == KEYS),
        "its key set is never read"
    );
}

#[tokio::test]
async fn a_local_write_is_refused_while_oidc_is_set_and_a_malformed_setting_admits_no_one() {
    let running = behind_oidc(setting().with_keys_url(KEYS), provider(KEYS)).await;
    let local = reqwest::Client::new()
        .post(format!("{}/api/push/subscriptions", running.url()))
        .header("x-agency-token", TOKEN)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(local.status().as_u16(), 403);
    assert!(local.text().await.unwrap().contains("OpenID Connect"));

    let mut malformed = setting().with_keys_url(KEYS);
    malformed.issuer = "http://login.example.com/tenant/v2.0".into();
    let running = behind_oidc(malformed, provider(KEYS)).await;
    // A malformed setting is no provider: the public host is then no host of this node (421), as with Access.
    assert_eq!(
        me(&running, owner(|_| {})).await,
        421,
        "the owner, through a malformed setting"
    );
}
