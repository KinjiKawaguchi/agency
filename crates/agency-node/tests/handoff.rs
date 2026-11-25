//! Signing in to Access outside the app's web view: the handoff redirect, the callback page, and the
//! app association file.
mod common;

use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::Running;
use reqwest::header::HeaderMap;
use reqwest::{redirect, Client, RequestBuilder, StatusCode};
use serde_json::{json, Value};

const APP_ID: &str = "ABCDE12345.com.example.agency";
const STATE: &str = "Zm9vYmFyLWJhei1xdXV4LTAxMjM0NTY3ODlhYmNkZWZ";
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

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

async fn status(builder: RequestBuilder) -> StatusCode {
    builder.send().await.unwrap().status()
}

#[tokio::test]
async fn the_owner_is_sent_to_the_fixed_callback_with_the_access_token() {
    let running = behind_access(Some(APP_ID)).await;
    assert_eq!(STATE.len(), 43);
    let token = access_token("alice@example.com", |_| {});
    let expected = format!("https://{PUBLIC_HOST}/app/callback#token={token}&state={STATE}");

    let response = handoff(&running, &format!("state={STATE}"), "alice@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let headers = response.headers();
    assert_eq!(header(headers, "location"), Some(expected.as_str()));
    assert_eq!(header(headers, "cache-control"), Some("no-store"));
    assert_eq!(header(headers, "referrer-policy"), Some("no-referrer"));

    let elsewhere = request(&running, &format!("/app/handoff?state={STATE}"))
        .header("host", "evil.example.com")
        .header("cf-ray", "1")
        .header("cf-access-jwt-assertion", &token)
        .header("x-forwarded-host", "evil.example.com")
        .header("referer", "https://evil.example.com/")
        .send()
        .await
        .unwrap();
    assert_eq!(
        header(elsewhere.headers(), "location"),
        Some(expected.as_str()),
        "nothing from the request but state goes into the target"
    );
}

#[tokio::test]
async fn only_the_owners_access_sign_in_is_handed_over() {
    let running = behind_access(Some(APP_ID)).await;
    let path = format!("/app/handoff?state={STATE}");
    assert_eq!(
        status(handoff(
            &running,
            &format!("state={STATE}"),
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
async fn the_state_is_exactly_43_base64url_characters_and_nothing_else() {
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
        let handed = handoff(&running, &format!("state={STATE}"), "alice@example.com");
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
