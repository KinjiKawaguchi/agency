//! The app releases a node has seen, from the `x-agency-client` header of the requests it admits. Only the
//! platform, the version, the build and when: never the person, the account or the device.
mod common;

use agency_core::clients_seen::{seen, MAX_VERSIONS_PER_PLATFORM};
use agency_proto::clock::Duration;
use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::{alice, get, Running};
use reqwest::StatusCode;
use serde_json::{json, Value};

async fn call(running: &Running, header: Option<&str>) -> StatusCode {
    let mut request = get(running, "/api/me");
    if let Some(value) = header {
        request = request.header("x-agency-client", value);
    }
    request.send().await.unwrap().status()
}

fn file(running: &Running) -> std::path::PathBuf {
    running.data_dir().join("clients-seen.json")
}

#[tokio::test]
async fn an_admitted_request_notes_the_platform_the_version_the_build_and_when() {
    let (alice, _) = alice().await;
    assert_eq!(
        call(&alice, Some("ios/0.3.0 (202611260418)")).await,
        StatusCode::OK
    );
    let all = seen(&alice.node);
    let one = &all["ios"]["0.3.0"];
    assert_eq!(one.last_seen, alice.node.now_text());
    assert_eq!(one.last_build, Some(202_611_260_418));

    // The file holds exactly that: no account, no device, no address.
    let text: Value =
        serde_json::from_str(&std::fs::read_to_string(file(&alice)).unwrap()).unwrap();
    assert_eq!(
        text,
        json!({ "ios": { "0.3.0": { "lastSeen": alice.node.now_text(), "lastBuild": 202_611_260_418u64 } } })
    );
}

#[tokio::test]
async fn a_header_that_is_missing_or_does_not_read_is_served_as_before_and_notes_nothing() {
    let (alice, _) = alice().await;
    for header in [
        None,
        Some("garbage"),
        Some("ios/0.3"),
        Some("ios/0.3.0 (abc)"),
        Some("windows/1.0.0"),
    ] {
        assert_eq!(call(&alice, header).await, StatusCode::OK, "{header:?}");
    }
    assert!(seen(&alice.node).is_empty());
    assert!(!file(&alice).exists());
}

#[tokio::test]
async fn a_request_that_is_not_admitted_notes_nothing() {
    let running = behind_access(None).await;
    let via_access = |token: Option<String>| {
        let mut request = reqwest::Client::new()
            .get(format!("{}/api/me", running.url()))
            .header("host", PUBLIC_HOST)
            .header("x-agency-client", "ios/9.9.9 (1)");
        if let Some(token) = token {
            request = request.header("cf-access-jwt-assertion", token);
        }
        request
    };
    let status = via_access(None).send().await.unwrap().status();
    assert_eq!(status, StatusCode::FORBIDDEN, "not signed in");
    let wrong_host = reqwest::Client::new()
        .get(format!("{}/api/me", running.url()))
        .header("host", "evil.example")
        .header("x-agency-client", "ios/9.9.9 (1)")
        .send()
        .await
        .unwrap();
    assert!(!wrong_host.status().is_success());
    assert!(
        seen(&running.node).is_empty(),
        "nothing was noted for a request that was refused"
    );

    let signed_in = access_token("alice@example.com", |_| {});
    assert_eq!(
        via_access(Some(signed_in)).send().await.unwrap().status(),
        StatusCode::OK
    );
    assert!(
        seen(&running.node)["ios"].contains_key("9.9.9"),
        "and it is noted once admitted"
    );
}

#[tokio::test]
async fn a_version_is_noted_again_only_an_hour_later() {
    let (alice, _) = alice().await;
    call(&alice, Some("ios/0.3.0 (1)")).await;
    let first = seen(&alice.node)["ios"]["0.3.0"].clone();

    alice.clock.advance(Duration::minutes(59));
    call(&alice, Some("ios/0.3.0 (2)")).await;
    assert_eq!(
        seen(&alice.node)["ios"]["0.3.0"],
        first,
        "inside the hour nothing is written"
    );

    alice.clock.advance(Duration::minutes(2));
    call(&alice, Some("ios/0.3.0 (3)")).await;
    let later = seen(&alice.node)["ios"]["0.3.0"].clone();
    assert_eq!(later.last_seen, alice.node.now_text());
    assert_eq!(later.last_build, Some(3));

    call(&alice, Some("macos/0.3.0")).await;
    assert!(
        seen(&alice.node).contains_key("macos"),
        "another platform is noted at once"
    );
    alice.clock.advance(Duration::hours(2));
    call(&alice, Some("ios/0.3.0")).await;
    assert_eq!(
        seen(&alice.node)["ios"]["0.3.0"].last_build,
        Some(3),
        "a call with no build keeps the last one"
    );
}

#[tokio::test]
async fn only_the_versions_seen_most_recently_are_kept_for_a_platform() {
    let (alice, _) = alice().await;
    for patch in 0..(MAX_VERSIONS_PER_PLATFORM + 8) {
        call(&alice, Some(&format!("ios/1.0.{patch}"))).await;
        alice.clock.advance(Duration::hours(2));
    }
    let versions = &seen(&alice.node)["ios"];
    assert_eq!(versions.len(), MAX_VERSIONS_PER_PLATFORM);
    assert!(!versions.contains_key("1.0.0"), "the oldest went first");
    assert!(versions.contains_key(&format!("1.0.{}", MAX_VERSIONS_PER_PLATFORM + 7)));
}
