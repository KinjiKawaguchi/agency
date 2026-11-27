//! A node reached at a public host name tells browsers to use HTTPS only.
mod common;

use common::access::{behind_access, PUBLIC_HOST};
use common::{alice, Running};

async fn hsts(running: &Running, path: &str, through_cloudflare: bool) -> Option<String> {
    let mut request = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .get(format!("{}{path}", running.url()));
    if through_cloudflare {
        request = request.header("host", PUBLIC_HOST).header("cf-ray", "1");
    }
    let response = request.send().await.unwrap();
    let value = response.headers().get("strict-transport-security");
    value.map(|value| value.to_str().unwrap().to_owned())
}

#[tokio::test]
async fn a_node_with_a_public_host_sends_hsts_on_every_answer() {
    let public = behind_access(Some("ABCDE12345.com.example.agency")).await;
    for (path, through_cloudflare) in [
        ("/.well-known/apple-app-site-association", true),
        ("/api/me", true),
        ("/fed/v1/orgs/acme/definition", true),
        ("/", false),
    ] {
        assert_eq!(
            hsts(&public, path, through_cloudflare).await.as_deref(),
            Some("max-age=31536000"),
            "{path}: a year, and nothing about other names in the domain"
        );
    }

    let (local, _) = alice().await;
    assert_eq!(
        hsts(&local, "/api/me", false).await,
        None,
        "a node without a public host"
    );
}

/// alice's node without an access setting, with `url` as its origin, answering `GET /api/me` (through
/// Cloudflare or directly) from a built app, as the node serves after a start with that url.
async fn hsts_of_app(url: &str, through_cloudflare: bool) -> Option<String> {
    use axum::extract::ConnectInfo;
    use tower::ServiceExt;
    let (alice, _) = alice().await;
    alice
        .node
        .update_config(|config| config.url = url.into())
        .unwrap();
    let host = if through_cloudflare {
        PUBLIC_HOST.to_owned()
    } else {
        format!("127.0.0.1:{}", alice.port)
    };
    let mut request = axum::http::Request::get("/api/me").header("host", host);
    if through_cloudflare {
        request = request.header("cf-ray", "1");
    }
    let mut request = request.body(axum::body::Body::empty()).unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<std::net::SocketAddr>().unwrap(),
    ));
    let response = alice.app().oneshot(request).await.unwrap();
    let value = response.headers().get("strict-transport-security");
    value.map(|value| value.to_str().unwrap().to_owned())
}

#[tokio::test]
async fn a_node_on_https_without_access_sends_hsts_on_what_comes_through_cloudflare() {
    let https = "https://agency.example.com";
    assert_eq!(
        hsts_of_app(https, true).await.as_deref(),
        Some("max-age=31536000"),
        "reached at its public host name through the tunnel, refused or not"
    );
    assert_eq!(
        hsts_of_app(https, false).await,
        None,
        "a direct local request"
    );
    assert_eq!(
        hsts_of_app("http://127.0.0.1:4400", true).await,
        None,
        "a node whose url is not HTTPS says nothing of HTTPS"
    );
}
