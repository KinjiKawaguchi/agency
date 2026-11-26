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
