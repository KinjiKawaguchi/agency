//! Signing in to Access outside the app's web view: the handoff redirect with a single-use code, the redeem
//! that gives the token to the app that holds the verifier, the callback page, and the app association file.
mod common;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use common::access::{access_token, behind_access, behind_access_allowing_scheme, PUBLIC_HOST};
use common::Running;
use reqwest::header::HeaderMap;
use reqwest::{redirect, Client, RequestBuilder, StatusCode};
use serde_json::{json, Value};

const APP_ID: &str = "ABCDE12345.com.example.agency";
const STATE: &str = "Zm9vYmFyLWJhei1xdXV4LTAxMjM0NTY3ODlhYmNkZWZ";
/// What the app keeps: 256 random bits; the node is given only their SHA-256.
const VERIFIER: &str = "dmVyaWZpZXItb25seS10aGUtYXBwLWtub3dzLTAxMjM";
const OTHER_VERIFIER: &str = "b3RoZXItdmVyaWZpZXItbm90LXRoZS1hcHBzLTAxMjM";
const ASSOCIATION: &str = "/.well-known/apple-app-site-association";

/// A request that does not follow redirects.
fn request(running: &Running, path: &str) -> RequestBuilder {
    let client = Client::builder()
        .redirect(redirect::Policy::none())
        .build()
        .unwrap();
    client.get(format!("{}{path}", running.url()))
}

/// The handoff through Cloudflare, signed in as `email`.
fn handoff(running: &Running, query: &str, email: &str) -> RequestBuilder {
    request(running, &format!("/app/handoff?{query}"))
        .header("host", PUBLIC_HOST)
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", access_token(email, |_| {}))
}

fn challenge_of(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// The handoff query the app sends for `verifier`.
fn query(verifier: &str, via: &str) -> String {
    format!(
        "state={STATE}&challenge={}&via={via}",
        challenge_of(verifier)
    )
}

/// The code in the fragment of a handoff's `Location`.
fn code_of(location: &str) -> String {
    let fragment = location.split_once('#').unwrap().1;
    let code = fragment.strip_prefix("code=").unwrap();
    code.split_once('&').unwrap().0.to_owned()
}

/// Signs `alice` in through the handoff and returns the code.
async fn code(running: &Running, verifier: &str, via: &str) -> String {
    let response = handoff(running, &query(verifier, via), "alice@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    code_of(header(response.headers(), "location").unwrap())
}

/// A redeem as the app makes it: no Access sign-in, no write token; `client` is who Cloudflare says it is.
fn redeem(running: &Running, code: &str, verifier: &str, client: &str) -> RequestBuilder {
    Client::new()
        .post(format!("{}/app/redeem", running.url()))
        .header("cf-connecting-ip", client)
        .json(&json!({ "code": code, "verifier": verifier }))
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

async fn status(builder: RequestBuilder) -> StatusCode {
    builder.send().await.unwrap().status()
}

#[tokio::test]
async fn the_owner_is_sent_to_the_fixed_callback_with_a_code_and_never_the_token() {
    let running = behind_access(Some(APP_ID)).await;
    assert_eq!(STATE.len(), 43);
    let token = access_token("alice@example.com", |_| {});
    let response = handoff(&running, &query(VERIFIER, "link"), "alice@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let headers = response.headers();
    let location = header(headers, "location").unwrap();
    let code = code_of(location);
    assert_eq!(code.len(), 43, "256 bits as base64url");
    assert_eq!(
        location,
        format!("https://{PUBLIC_HOST}/app/callback#code={code}&state={STATE}")
    );
    assert!(!location.contains(&token), "the token is in no URL");
    assert!(!location.contains("token"), "nor is there a field for it");
    assert_eq!(header(headers, "cache-control"), Some("no-store"));
    assert_eq!(header(headers, "referrer-policy"), Some("no-referrer"));

    let elsewhere = request(
        &running,
        &format!("/app/handoff?{}", query(VERIFIER, "link")),
    )
    .header("host", "evil.example.com")
    .header("cf-ray", "1")
    .header("cf-access-jwt-assertion", &token)
    .header("x-forwarded-host", "evil.example.com")
    .header("referer", "https://evil.example.com/")
    .send()
    .await
    .unwrap();
    let again = header(elsewhere.headers(), "location").unwrap();
    assert!(
        again.starts_with(&format!("https://{PUBLIC_HOST}/app/callback#code=")),
        "nothing from the request but state, challenge and via goes into the target: {again}"
    );
    assert_ne!(code_of(again), code, "a new code each time");
}

#[tokio::test]
async fn for_a_host_without_a_universal_link_the_callback_is_the_apps_scheme() {
    let running = behind_access_allowing_scheme(Some(APP_ID)).await;
    let response = handoff(&running, &query(VERIFIER, "scheme"), "alice@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = header(response.headers(), "location").unwrap();
    let code = code_of(location);
    assert_eq!(
        location,
        format!("agency-signin://callback#code={code}&state={STATE}")
    );
}

/// RED-15: the scheme's callback goes to the app that started the session, whatever app that is, and a starter picks
/// its own challenge and holds its own verifier. So the node does not make a code for it unless its owner said so.
#[tokio::test]
async fn the_apps_scheme_is_refused_unless_the_owner_turned_it_on_and_no_code_is_made() {
    let running = behind_access(Some(APP_ID)).await;
    let response = handoff(&running, &query(VERIFIER, "scheme"), "alice@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        header(response.headers(), "location").is_none(),
        "no redirect, so no code"
    );
    let body = response.text().await.unwrap();
    assert!(
        body.contains("allowSchemeSignIn") && !body.contains("agency-signin"),
        "{body}"
    );
    // Nothing was made, so nothing can be redeemed for the owner's token, and the universal link still works.
    let code = code(&running, VERIFIER, "link").await;
    let redeemed = redeem(&running, &code, VERIFIER, "198.51.100.1")
        .send()
        .await
        .unwrap();
    assert_eq!(redeemed.status(), StatusCode::OK);
}

#[tokio::test]
async fn refusing_the_scheme_takes_no_place_among_the_codes_that_wait() {
    let running = behind_access(Some(APP_ID)).await;
    for _ in 0..80 {
        let refused = handoff(&running, &query(VERIFIER, "scheme"), "alice@example.com");
        assert_eq!(status(refused).await, StatusCode::BAD_REQUEST);
    }
    assert_eq!(
        code_of(&running_location(&running).await).len(),
        43,
        "a sign-in still gets its code"
    );
}

async fn running_location(running: &Running) -> String {
    let response = handoff(running, &query(VERIFIER, "link"), "alice@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    header(response.headers(), "location").unwrap().to_owned()
}

#[tokio::test]
async fn only_the_owners_access_sign_in_is_handed_over() {
    let running = behind_access(Some(APP_ID)).await;
    let path = format!("/app/handoff?{}", query(VERIFIER, "link"));
    assert_eq!(
        status(handoff(
            &running,
            &query(VERIFIER, "link"),
            "carol@example.com"
        ))
        .await,
        403,
        "someone Access lets read, but not the owner"
    );
    assert_eq!(
        status(request(&running, &path).header("cf-ray", "1")).await,
        403,
        "no Access token"
    );
    let expired = access_token("alice@example.com", |claims| {
        claims["exp"] = json!(1_795_521_600 - 61)
    });
    assert_eq!(
        status(
            request(&running, &path)
                .header("cf-ray", "1")
                .header("cf-access-jwt-assertion", expired)
        )
        .await,
        403,
        "an expired token"
    );
    assert_eq!(
        status(request(&running, &path)).await,
        403,
        "a local request has no Access sign-in to hand over"
    );
    let owner = access_token("alice@example.com", |_| {});
    assert_eq!(
        status(request(&running, &path).header("cf-access-jwt-assertion", owner)).await,
        403,
        "a local request with a token header is still local"
    );
}

#[tokio::test]
async fn the_query_is_exactly_state_challenge_and_via_and_nothing_else() {
    let running = behind_access(Some(APP_ID)).await;
    let challenge = challenge_of(VERIFIER);
    let short_state = &STATE[..42];
    let short_challenge = &challenge[..42];
    let queries = [
        format!("state={STATE}&via=link"),
        format!("state={STATE}&challenge={challenge}"),
        format!("state={short_state}&challenge={challenge}&via=link"),
        format!("state={STATE}A&challenge={challenge}&via=link"),
        format!("state={STATE}&challenge={short_challenge}&via=link"),
        format!("state={STATE}&challenge={challenge}A&via=link"),
        format!("state={short_state}+&challenge={challenge}&via=link"),
        format!("state={STATE}&challenge={short_challenge}/&via=link"),
        format!("state={STATE}&challenge={short_challenge}=&via=link"),
        format!("state={STATE}&challenge={short_challenge}%41&via=link"),
        format!("state={STATE}&challenge={challenge}&via=https://evil.example.com"),
        format!("state={STATE}&challenge={challenge}&via=evil"),
        format!("state={STATE}&challenge={challenge}&via=Link"),
        format!("state={STATE}&challenge={challenge}&via="),
        format!("state={STATE}&challenge={challenge}&via=link&next=https://evil.example.com"),
        format!("state={STATE}&challenge={challenge}&via=link&via=scheme"),
        format!("state={STATE}&state={STATE}&challenge={challenge}&via=link"),
        format!("challenge={challenge}&state={STATE}&via=link"),
        format!("next=x&state={STATE}&challenge={challenge}&via=link"),
        format!("State={STATE}&challenge={challenge}&via=link"),
        "state=".into(),
        String::new(),
    ];
    for query in queries {
        let response = handoff(&running, &query, "alice@example.com")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400, "{query}");
        assert!(response.headers().get("location").is_none(), "{query}");
    }
}

/// The apps that exist before the code exchange send `state` alone and get the token in the fragment. That form is
/// deprecated and stays until they are gone (docs/protocol.md); nothing new may be built on it.
#[tokio::test]
async fn the_deprecated_state_only_form_still_answers_as_before() {
    let running = behind_access(Some(APP_ID)).await;
    let token = access_token("alice@example.com", |_| {});
    let response = handoff(&running, &format!("state={STATE}"), "alice@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        header(response.headers(), "location"),
        Some(format!("https://{PUBLIC_HOST}/app/callback#token={token}&state={STATE}").as_str())
    );
    assert_eq!(
        header(response.headers(), "cache-control"),
        Some("no-store")
    );
    assert_eq!(
        header(response.headers(), "referrer-policy"),
        Some("no-referrer")
    );
    let other = request(&running, &format!("/app/handoff?state={STATE}"))
        .header("host", "evil.example.com")
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", &token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        header(other.headers(), "location"),
        Some(format!("https://{PUBLIC_HOST}/app/callback#token={token}&state={STATE}").as_str()),
        "nothing from the request but state goes into the target"
    );
    assert_eq!(
        status(handoff(
            &running,
            &format!("state={STATE}"),
            "carol@example.com"
        ))
        .await,
        403,
        "still only the owner"
    );
}

#[tokio::test]
async fn the_deprecated_form_keeps_its_old_exactness_and_makes_no_code() {
    let running = behind_access(Some(APP_ID)).await;
    let short = &STATE[..42];
    let queries = [
        format!("state={short}"),
        format!("state={STATE}A"),
        format!("state={short}+"),
        format!("state={short}/"),
        format!("state={short}="),
        format!("state={short}%41"),
        format!("state={STATE}&state={STATE}"),
        format!("state={STATE}&next=https://evil.example.com"),
        format!("next=x&state={STATE}"),
        format!("State={STATE}"),
        "state=".into(),
        String::new(),
    ];
    for query in queries {
        let response = handoff(&running, &query, "alice@example.com")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400, "{query}");
        assert!(response.headers().get("location").is_none(), "{query}");
    }
    // 64 sign-ins of the old form wait for nothing: they make no code, so the pool is not used up.
    for _ in 0..70 {
        assert_eq!(
            status(handoff(
                &running,
                &format!("state={STATE}"),
                "alice@example.com"
            ))
            .await,
            303
        );
    }
    code(&running, VERIFIER, "link").await;
}

#[tokio::test]
async fn the_app_redeems_the_code_with_its_verifier_for_the_token_and_nothing_else() {
    let running = behind_access(Some(APP_ID)).await;
    let token = access_token("alice@example.com", |_| {});
    let code = code(&running, VERIFIER, "link").await;
    let response = redeem(&running, &code, VERIFIER, "203.0.113.7")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        header(response.headers(), "cache-control"),
        Some("no-store")
    );
    let answer: Value = response.json().await.unwrap();
    assert_eq!(
        answer,
        json!({ "token": token }),
        "the token, and nothing else"
    );
}

#[tokio::test]
async fn a_code_is_redeemed_once() {
    let running = behind_access(Some(APP_ID)).await;
    let code = code(&running, VERIFIER, "link").await;
    assert_eq!(
        redeem(&running, &code, VERIFIER, "203.0.113.7")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        redeem(&running, &code, VERIFIER, "203.0.113.7")
            .send()
            .await
            .unwrap()
            .status(),
        400,
        "a second redeem of the same code, with the right verifier"
    );
}

#[tokio::test]
async fn a_wrong_verifier_gets_nothing_and_burns_the_code() {
    let running = behind_access_allowing_scheme(Some(APP_ID)).await;
    let code = code(&running, VERIFIER, "scheme").await;
    let stolen = redeem(&running, &code, OTHER_VERIFIER, "198.51.100.9")
        .send()
        .await
        .unwrap();
    assert_eq!(stolen.status(), 400);
    assert!(
        !stolen.text().await.unwrap().contains("eyJ"),
        "no token in the refusal"
    );
    assert_eq!(
        redeem(&running, &code, VERIFIER, "203.0.113.7")
            .send()
            .await
            .unwrap()
            .status(),
        400,
        "someone who read the callback tried once and spent the code: the app starts again"
    );
}

#[tokio::test]
async fn a_code_lives_a_minute() {
    let running = behind_access(Some(APP_ID)).await;
    let early = code(&running, VERIFIER, "link").await;
    running
        .clock
        .advance(agency_proto::clock::Duration::seconds(59));
    assert_eq!(
        redeem(&running, &early, VERIFIER, "203.0.113.7")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let late = code(&running, VERIFIER, "link").await;
    running
        .clock
        .advance(agency_proto::clock::Duration::seconds(60));
    assert_eq!(
        redeem(&running, &late, VERIFIER, "203.0.113.7")
            .send()
            .await
            .unwrap()
            .status(),
        400,
        "sixty seconds after it was made"
    );
}

#[tokio::test]
async fn every_way_to_fail_gets_the_same_answer() {
    let running = behind_access(Some(APP_ID)).await;
    let good = code(&running, VERIFIER, "link").await;
    let post = |body: Value| {
        Client::new()
            .post(format!("{}/app/redeem", running.url()))
            .header("cf-connecting-ip", "203.0.113.7")
            .json(&body)
    };
    let mut answers = vec![];
    for body in [
        json!({ "code": "A".repeat(43), "verifier": VERIFIER }),
        json!({ "code": "short", "verifier": VERIFIER }),
        json!({ "code": good, "verifier": "short" }),
        json!({ "code": good, "verifier": VERIFIER, "extra": 1 }),
        json!({ "code": good }),
        json!({ "verifier": VERIFIER }),
        json!({}),
        json!("code"),
        json!([1]),
    ] {
        let response = post(body.clone()).send().await.unwrap();
        assert_eq!(response.status(), 400, "{body}");
        answers.push(response.text().await.unwrap());
    }
    let not_json = Client::new()
        .post(format!("{}/app/redeem", running.url()))
        .header("cf-connecting-ip", "203.0.113.7")
        .body("not json")
        .send()
        .await
        .unwrap();
    assert_eq!(not_json.status(), 400);
    answers.push(not_json.text().await.unwrap());
    assert!(
        answers.windows(2).all(|pair| pair[0] == pair[1]),
        "{answers:?}"
    );
    assert_eq!(
        redeem(&running, &good, VERIFIER, "203.0.113.8")
            .send()
            .await
            .unwrap()
            .status(),
        200,
        "none of those tries spent the good code"
    );
}

#[tokio::test]
async fn a_client_address_gets_twenty_tries_a_minute() {
    let running = behind_access(Some(APP_ID)).await;
    for attempt in 0..20 {
        let wrong = redeem(&running, &"A".repeat(43), VERIFIER, "203.0.113.7")
            .send()
            .await
            .unwrap();
        assert_eq!(wrong.status(), 400, "try {attempt}");
    }
    let limited = redeem(&running, &"A".repeat(43), VERIFIER, "203.0.113.7")
        .send()
        .await
        .unwrap();
    assert_eq!(limited.status(), 429);
    assert_eq!(header(limited.headers(), "retry-after"), Some("60"));
    let code = code(&running, VERIFIER, "link").await;
    assert_eq!(
        redeem(&running, &code, VERIFIER, "203.0.113.7")
            .send()
            .await
            .unwrap()
            .status(),
        429,
        "a limited address is refused even with a good code"
    );
    assert_eq!(
        redeem(&running, &code, VERIFIER, "198.51.100.1")
            .send()
            .await
            .unwrap()
            .status(),
        200,
        "another address is not limited by it"
    );
    running
        .clock
        .advance(agency_proto::clock::Duration::seconds(61));
    assert_eq!(
        redeem(&running, &"A".repeat(43), VERIFIER, "203.0.113.7")
            .send()
            .await
            .unwrap()
            .status(),
        400,
        "a minute later the address is heard again"
    );
}

async fn try_redeem(running: &Running, address: &str) -> u16 {
    redeem(running, &"A".repeat(43), VERIFIER, address)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

fn address_of(number: u32) -> String {
    format!(
        "10.{}.{}.{}",
        number >> 16,
        (number >> 8) & 255,
        number & 255
    )
}

fn pass_a_second(running: &Running) {
    running
        .clock
        .advance(agency_proto::clock::Duration::seconds(1));
}

#[tokio::test]
async fn a_full_address_table_forgets_the_quietest_address_and_never_refuses_a_new_one() {
    let running = behind_access(Some(APP_ID)).await;
    // The quietest address uses up its allowance first.
    for _ in 0..20 {
        assert_eq!(try_redeem(&running, "192.0.2.9").await, 400);
    }
    assert_eq!(try_redeem(&running, "192.0.2.9").await, 429);
    pass_a_second(&running);
    for number in 0..1022u32 {
        assert_eq!(try_redeem(&running, &address_of(number)).await, 400);
    }
    pass_a_second(&running);
    // The most recent address uses up its allowance too: the table now holds 1024 addresses.
    for _ in 0..20 {
        assert_eq!(try_redeem(&running, "192.0.2.10").await, 400);
    }
    assert_eq!(try_redeem(&running, "192.0.2.10").await, 429);

    assert_eq!(
        try_redeem(&running, "192.0.2.9").await,
        429,
        "a known address is not forgotten by its own try, even the quietest one"
    );
    assert_eq!(
        try_redeem(&running, "198.51.100.1").await,
        400,
        "a new address is heard when the table is full"
    );
    assert_eq!(
        try_redeem(&running, "192.0.2.9").await,
        400,
        "the address that had been quiet the longest was forgotten, so it has its allowance again"
    );
    assert_eq!(
        try_redeem(&running, "192.0.2.10").await,
        429,
        "a recent address is remembered, with its tries"
    );
}

#[tokio::test]
async fn the_address_table_empties_a_minute_after_the_last_try() {
    let running = behind_access(Some(APP_ID)).await;
    for number in 0..1024u32 {
        assert_eq!(try_redeem(&running, &address_of(number)).await, 400);
    }
    running
        .clock
        .advance(agency_proto::clock::Duration::seconds(61));
    for _ in 0..20 {
        assert_eq!(try_redeem(&running, &address_of(0)).await, 400);
    }
    assert_eq!(try_redeem(&running, &address_of(0)).await, 429);
    running
        .clock
        .advance(agency_proto::clock::Duration::seconds(61));
    assert_eq!(try_redeem(&running, &address_of(0)).await, 400);
}

#[tokio::test]
async fn at_most_sixty_four_sign_ins_wait_at_once() {
    let running = behind_access(Some(APP_ID)).await;
    for _ in 0..64 {
        code(&running, VERIFIER, "link").await;
    }
    let refused = handoff(&running, &query(VERIFIER, "link"), "alice@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 503);
    running
        .clock
        .advance(agency_proto::clock::Duration::seconds(61));
    code(&running, VERIFIER, "link").await;
}

#[tokio::test]
async fn without_a_well_formed_app_id_there_is_no_redeem_either() {
    let running = behind_access(None).await;
    let response = redeem(&running, &"A".repeat(43), VERIFIER, "203.0.113.7")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn the_association_file_is_fixed_json_served_without_signing_in() {
    let running = behind_access(Some(APP_ID)).await;
    for builder in [
        request(&running, ASSOCIATION)
            .header("host", PUBLIC_HOST)
            .header("cf-ray", "1"),
        request(
            &running,
            &format!("{ASSOCIATION}?x=https://evil.example.com"),
        ),
    ] {
        let response = builder.send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            header(response.headers(), "content-type"),
            Some("application/json")
        );
        let body: Value = response.json().await.unwrap();
        assert_eq!(
            body,
            json!({
                "applinks": { "details": [{ "appIDs": [APP_ID], "components": [{ "/": "/app/callback" }] }] },
                "webcredentials": { "apps": [APP_ID] },
            })
        );
    }
}

#[tokio::test]
async fn without_a_well_formed_app_id_there_is_no_handoff_and_no_association_file() {
    for app_id in [
        None,
        Some("abcde12345.com.example.agency"),
        Some("ABCDE1234.com.example.agency"),
        Some("ABCDE12345"),
        Some("ABCDE12345."),
        Some("ABCDE12345..com.example"),
        Some("ABCDE12345.com.example."),
        Some("ABCDE12345.com/example"),
        Some("ABCDE12345.com.example.agency\"}"),
    ] {
        let running = behind_access(app_id).await;
        let through_cloudflare = request(&running, ASSOCIATION).header("cf-ray", "1");
        assert_eq!(status(through_cloudflare).await, 404, "{app_id:?}");
        assert_eq!(
            status(request(&running, ASSOCIATION)).await,
            404,
            "{app_id:?}: not the web client's page"
        );
        let handed = handoff(&running, &query(VERIFIER, "link"), "alice@example.com");
        assert_eq!(status(handed).await, 404, "{app_id:?}");
    }
}

#[tokio::test]
async fn the_callback_page_is_behind_access_and_reads_nothing() {
    let running = behind_access(Some(APP_ID)).await;
    let page = request(&running, &format!("/app/callback?token=x&state={STATE}"))
        .send()
        .await
        .unwrap();
    assert_eq!(page.status(), 200);
    assert!(header(page.headers(), "content-security-policy")
        .is_some_and(|policy| policy.contains("script-src 'self'")));
    let text = page.text().await.unwrap();
    assert!(text.contains("Return to the Agency app"));
    assert!(!text.contains("<script"), "no script on the page");
    assert!(
        !text.contains("token") && !text.contains(STATE),
        "nothing echoed"
    );

    let unsigned = request(&running, "/app/callback").header("cf-ray", "1");
    assert_eq!(
        status(unsigned).await,
        403,
        "the same Access check as the rest"
    );
}
