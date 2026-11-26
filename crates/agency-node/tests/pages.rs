//! The web client's files and pages: a missing file is 404, and only a client route gets the page,
//! which carries the write token.
mod common;

use common::{alice, get, TOKEN};

#[tokio::test]
async fn a_missing_file_is_not_found_and_only_a_route_gets_the_page() {
    let (alice, _) = alice().await;
    let fetch = |path: &'static str| {
        let request = get(&alice, path);
        async move {
            let response = request.send().await.unwrap();
            let status = response.status().as_u16();
            let content_type = response.headers()["content-type"]
                .to_str()
                .unwrap()
                .to_owned();
            (status, content_type, response.text().await.unwrap())
        }
    };

    let (status, content_type, _) = fetch("/assets/app.js").await;
    assert_eq!(
        (status, content_type.as_str()),
        (200, "text/javascript; charset=utf-8")
    );

    for missing in [
        "/assets/app-0ld0ld.js",
        "/assets/missing.css",
        "/icons/icon-192.png",
        "/icons/none",
        "/sw.js",
        "/manifest.webmanifest",
        "/offline.html",
        "/robots.txt",
    ] {
        let (status, content_type, body) = fetch(missing).await;
        assert_eq!(status, 404, "{missing}");
        assert!(
            content_type.starts_with("text/plain"),
            "{missing}: {content_type}"
        );
        assert!(
            !body.contains(TOKEN),
            "{missing}: the write token stays out of it"
        );
    }

    for route in [
        "/",
        "/index.html",
        "/desk",
        "/org",
        "/requests/r-20261124.1",
        "/people/acme.app",
    ] {
        let (status, content_type, body) = fetch(route).await;
        assert_eq!(status, 200, "{route}");
        assert!(content_type.starts_with("text/html"), "{route}");
        assert!(body.contains(TOKEN), "{route}: a route gets the page");
    }
}
