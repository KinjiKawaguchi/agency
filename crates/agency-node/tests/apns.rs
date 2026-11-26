//! The APNs adapter against a stand-in APNs server: the provider token is an ES256 JWT that verifies with
//! the key's public half, each token goes to its environment's host, and APNs answers map to "gone" or
//! "failed". The key in tests/data/apns-test-key.p8 is made for these tests and used nowhere else.
use agency_adapters::apns::{payload, Apns, ApnsHosts};
use agency_adapters::memory::ManualClock;
use agency_core::config::PushConfig;
use agency_core::ports::Notifier;
use agency_core::push::{Notice, NoticeKind, PushError};
use agency_proto::client::{PushEnvironment, PushEvent, PushToken};
use agency_proto::{Id, Timestamp};
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const KEY_X: &str = "Y9pQi4IitUbyiIVGOtWYWrvw6Q6Dd9_vB58SzG7--TE";
const KEY_Y: &str = "_SHn4FM-2MRI6j1Su4IvVCbS-hp4OGp-GYm7mq0o2ms";
const T0: &str = "2026-11-24T12:00:00Z";

/// What the stand-in server saw: environment, token, headers, and body of each request.
type Seen = Arc<Mutex<Vec<(String, String, HeaderMap, Value)>>>;

/// Answers by the token's first two characters: aa 200, bb 410, cc 400 BadDeviceToken,
/// dd 403 ExpiredProviderToken, anything else 500.
async fn stand_in() -> (ApnsHosts, Seen) {
    let seen: Seen = Arc::default();
    let record = seen.clone();
    let app = axum::Router::new().route(
        "/{environment}/3/device/{token}",
        axum::routing::post(
            move |Path((environment, token)): Path<(String, String)>,
                  headers: HeaderMap,
                  Json(body): Json<Value>| async move {
                record
                    .lock()
                    .unwrap()
                    .push((environment, token.clone(), headers, body));
                let (status, reason) = match &token[..2] {
                    "aa" => (StatusCode::OK, ""),
                    "bb" => (StatusCode::GONE, "Unregistered"),
                    "cc" => (StatusCode::BAD_REQUEST, "BadDeviceToken"),
                    "dd" => (StatusCode::FORBIDDEN, "ExpiredProviderToken"),
                    _ => (StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError"),
                };
                (status, Json(json!({ "reason": reason })))
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let hosts = ApnsHosts {
        production: format!("{url}/production"),
        sandbox: format!("{url}/sandbox"),
    };
    (hosts, seen)
}

fn apns(hosts: ApnsHosts, clock: &ManualClock) -> Apns {
    let config = PushConfig {
        key_path: concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/apns-test-key.p8").into(),
        key_id: "KEY1234567".into(),
        team_id: "TEAM123456".into(),
        bundle_id: "com.example.agency".into(),
        hide_titles: false,
    };
    Apns::new(config, hosts, Arc::new(clock.clone())).unwrap()
}

fn token(prefix: &str, environment: PushEnvironment) -> PushToken {
    PushToken {
        token: prefix.repeat(32),
        environment,
        label: "iPhone".into(),
        registered_at: Timestamp::parse(T0).unwrap(),
        events: vec![PushEvent::Request],
    }
}

fn notice() -> Notice {
    Notice {
        kind: NoticeKind::Request {
            org: Id::parse("acme").unwrap(),
            team: Id::parse("app").unwrap(),
            request: "r-20261124-110000-a1b2".into(),
            team_label: Some("app".into()),
            title: Some("Ship it?".into()),
        },
        badge: 3,
    }
}

async fn send(apns: &Apns, prefix: &str) -> Result<(), PushError> {
    apns.send(&token(prefix, PushEnvironment::Production), &notice())
        .await
}

fn bearer(headers: &HeaderMap) -> String {
    let value = headers["authorization"].to_str().unwrap();
    value.strip_prefix("bearer ").unwrap().to_owned()
}

#[derive(serde::Deserialize)]
struct Claims {
    iss: String,
    iat: i64,
}

#[tokio::test]
async fn sends_a_signed_alert_to_the_tokens_environment() {
    let (hosts, seen) = stand_in().await;
    let clock = ManualClock::at(T0);
    let apns = apns(hosts, &clock);
    apns.send(&token("aa", PushEnvironment::Sandbox), &notice())
        .await
        .unwrap();
    apns.send(&token("aa", PushEnvironment::Production), &notice())
        .await
        .unwrap();

    let seen = seen.lock().unwrap().clone();
    let (environment, device, headers, body) = &seen[0];
    assert_eq!(
        (environment.as_str(), device.as_str()),
        ("sandbox", "aa".repeat(32).as_str())
    );
    assert_eq!(seen[1].0, "production");
    assert_eq!(headers["apns-topic"], "com.example.agency");
    assert_eq!(headers["apns-push-type"], "alert");
    assert_eq!(headers["apns-collapse-id"], "r-20261124-110000-a1b2");
    assert_eq!(
        body,
        &json!({
            "aps": { "alert": { "title": "app", "body": "Ship it?" }, "sound": "default", "thread-id": "acme/app", "badge": 3 },
            "agency": { "org": "acme", "team": "app", "request": "r-20261124-110000-a1b2" },
        })
    );

    let jwt = bearer(headers);
    let header = jsonwebtoken::decode_header(&jwt).unwrap();
    assert_eq!(header.kid.as_deref(), Some("KEY1234567"));
    let key = jsonwebtoken::DecodingKey::from_ec_components(KEY_X, KEY_Y).unwrap();
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::ES256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    let claims = jsonwebtoken::decode::<Claims>(&jwt, &key, &validation)
        .unwrap()
        .claims;
    assert_eq!(claims.iss, "TEAM123456");
    assert_eq!(claims.iat, 1_795_521_600, "issued at the node's clock");
}

#[tokio::test]
async fn reuses_the_provider_token_for_fifty_minutes_and_after_a_refusal_makes_another() {
    let (hosts, seen) = stand_in().await;
    let clock = ManualClock::at(T0);
    let apns = apns(hosts, &clock);
    send(&apns, "aa").await.unwrap();
    clock.advance(agency_proto::clock::Duration::minutes(49));
    send(&apns, "aa").await.unwrap();
    clock.advance(agency_proto::clock::Duration::minutes(2));
    send(&apns, "aa").await.unwrap();
    let tokens: Vec<String> = seen
        .lock()
        .unwrap()
        .iter()
        .map(|one| bearer(&one.2))
        .collect();
    assert_eq!(tokens[0], tokens[1], "reused within 50 minutes");
    assert_ne!(tokens[1], tokens[2], "made again after 50 minutes");

    assert!(matches!(send(&apns, "dd").await, Err(PushError::Failed(_))));
    send(&apns, "aa").await.unwrap();
    let tokens: Vec<String> = seen
        .lock()
        .unwrap()
        .iter()
        .map(|one| bearer(&one.2))
        .collect();
    assert_ne!(
        tokens[3], tokens[4],
        "a refused provider token is not used again"
    );
}

#[tokio::test]
async fn unregistered_and_bad_tokens_are_gone_and_other_errors_fail() {
    let (hosts, _) = stand_in().await;
    let clock = ManualClock::at(T0);
    let apns = apns(hosts, &clock);
    assert_eq!(send(&apns, "bb").await, Err(PushError::Gone));
    assert_eq!(send(&apns, "cc").await, Err(PushError::Gone));
    assert!(matches!(send(&apns, "ee").await, Err(PushError::Failed(_))));
}

fn alert_of(kind: NoticeKind) -> (Value, Value, String, String) {
    let notice = Notice { kind, badge: 2 };
    let body = payload(&notice);
    (
        body["aps"]["alert"].clone(),
        body["agency"].clone(),
        body["aps"]["thread-id"].as_str().unwrap().to_owned(),
        notice.collapse_id().to_owned(),
    )
}

#[test]
fn a_device_is_worded_by_the_app_and_carries_only_its_label() {
    let device = Id::parse("d-a652124afda7").unwrap();
    let with = alert_of(NoticeKind::Device {
        device: device.clone(),
        label: Some("Tablet".into()),
    });
    assert_eq!(
        with,
        (
            json!({ "loc-key": "PUSH_DEVICE_WAITING", "loc-args": ["Tablet"] }),
            json!({ "type": "device" }),
            "devices".into(),
            "d-a652124afda7".into()
        )
    );
    let without = alert_of(NoticeKind::Device {
        device,
        label: None,
    });
    assert_eq!(
        without.0,
        json!({ "loc-key": "PUSH_DEVICE_WAITING_NO_LABEL" })
    );
    assert_eq!(
        without.1,
        json!({ "type": "device" }),
        "never a key or a code"
    );
}

#[test]
fn a_refused_decision_names_where_to_look_and_its_title_only_when_it_has_one() {
    let decision = |title: Option<&str>| NoticeKind::Decision {
        org: Id::parse("acme").unwrap(),
        team: Id::parse("app").unwrap(),
        id: "0123456789abcdef0123456789abcdef".into(),
        title: title.map(str::to_owned),
    };
    let (alert, agency, thread, collapse) = alert_of(decision(Some("Ship it?")));
    assert_eq!(
        alert,
        json!({ "loc-key": "PUSH_DECISION_REFUSED", "loc-args": ["Ship it?"] })
    );
    assert_eq!(
        agency,
        json!({ "type": "decision", "org": "acme", "team": "app", "id": "0123456789abcdef0123456789abcdef" })
    );
    assert_eq!((thread.as_str(), collapse.len()), ("acme/app", 32));
    let (alert, ..) = alert_of(decision(None));
    assert_eq!(
        alert,
        json!({ "loc-key": "PUSH_DECISION_REFUSED_NO_TITLE" })
    );
}

#[test]
fn a_request_with_no_title_to_show_says_only_that_one_waits() {
    let request = NoticeKind::Request {
        org: Id::parse("acme").unwrap(),
        team: Id::parse("app").unwrap(),
        request: "r-1".into(),
        team_label: None,
        title: None,
    };
    let (alert, agency, ..) = alert_of(request);
    assert_eq!(alert, json!({ "loc-key": "PUSH_REQUEST_WAITING_HIDDEN" }));
    assert_eq!(
        agency,
        json!({ "org": "acme", "team": "app", "request": "r-1" }),
        "no `type`: a request, as before"
    );
}
