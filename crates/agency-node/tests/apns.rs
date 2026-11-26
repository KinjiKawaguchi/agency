//! The APNs adapter against a stand-in APNs server: the provider token is an ES256 JWT that verifies with
//! the key's public half, each token goes to its environment's host, and APNs answers map to "gone" or
//! "failed". The key in tests/data/apns-test-key.p8 is made for these tests and used nowhere else.
use agency_adapters::apns::{desk_payload, payload, Apns, ApnsHosts};
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
                    "ff" => (
                        StatusCode::SERVICE_UNAVAILABLE,
                        "the body says: a secret detail about the phone",
                    ),
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
        last_succeeded_at: None,
        last_failure: None,
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
    assert_eq!(headers["apns-priority"], "10");
    assert!(
        headers.get("apns-expiration").is_none(),
        "an alert is kept until it can be delivered"
    );
    assert_eq!(
        body,
        &json!({
            "aps": { "alert": { "title": "app", "body": "Ship it?" }, "sound": "default", "thread-id": "acme/app", "badge": 3, "mutable-content": 1, "content-available": 1 },
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

#[tokio::test]
async fn a_failure_is_the_status_and_the_reason_code_and_nothing_else_of_what_apns_said() {
    let (hosts, _) = stand_in().await;
    let clock = ManualClock::at(T0);
    let apns = apns(hosts, &clock);
    assert_eq!(
        send(&apns, "ee").await,
        Err(PushError::Failed(
            "APNs answered 500 InternalServerError".into()
        ))
    );
    assert_eq!(
        send(&apns, "ff").await,
        Err(PushError::Failed("APNs answered 503".into())),
        "words that are not a reason code are not kept, and no space is left after the status"
    );
    assert_eq!(
        send(&apns, "dd").await,
        Err(PushError::Failed(
            "APNs refused the provider token (ExpiredProviderToken)".into()
        ))
    );
}

#[tokio::test]
async fn an_unreachable_apns_is_said_so_without_the_address_that_has_the_token_in_it() {
    let hosts = ApnsHosts {
        production: "http://127.0.0.1:1/production".into(),
        sandbox: "http://127.0.0.1:1/sandbox".into(),
    };
    let clock = ManualClock::at(T0);
    let Err(PushError::Failed(why)) = send(&apns(hosts, &clock), "ee").await else {
        panic!("a closed port answers nothing");
    };
    assert_eq!(why, "APNs could not be reached");
    assert!(!why.contains(&"ee".repeat(32)) && !why.contains("127.0.0.1"));
}

fn alert_of(kind: NoticeKind) -> (Value, Value, String, String) {
    let notice = Notice { kind, badge: 2 };
    let body = payload(&notice, None);
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

#[tokio::test]
async fn the_desk_push_is_a_background_push_with_nothing_of_the_desk_in_it() {
    let (hosts, seen) = stand_in().await;
    let clock = ManualClock::at(T0);
    let apns = apns(hosts, &clock);
    apns.send_desk_changed(&token("aa", PushEnvironment::Production))
        .await
        .unwrap();
    let seen = seen.lock().unwrap().clone();
    let (environment, device, headers, body) = &seen[0];
    assert_eq!(
        (environment.as_str(), device.as_str()),
        ("production", "aa".repeat(32).as_str())
    );
    assert_eq!(headers["apns-topic"], "com.example.agency");
    assert_eq!(headers["apns-push-type"], "background");
    assert_eq!(
        headers["apns-priority"], "5",
        "Apple refuses priority 10 for a push with only content-available"
    );
    assert_eq!(headers["apns-collapse-id"], "desk");
    let expires = agency_proto::clock::parse(T0).unwrap().unix_timestamp() + 60 * 60;
    assert_eq!(headers["apns-expiration"], expires.to_string().as_str());
    assert_eq!(
        body,
        &json!({ "aps": { "content-available": 1 }, "agency": { "type": "desk" } }),
        "no alert, sound, badge, title, label or count"
    );
    assert_eq!(body, &desk_payload(None));
    assert!(headers["authorization"]
        .to_str()
        .unwrap()
        .starts_with("bearer "));
}

#[tokio::test]
async fn the_desk_push_has_the_same_answers_as_an_alert() {
    let (hosts, _) = stand_in().await;
    let clock = ManualClock::at(T0);
    let apns = apns(hosts, &clock);
    let wake = |prefix: &'static str| {
        let apns = &apns;
        async move {
            apns.send_desk_changed(&token(prefix, PushEnvironment::Production))
                .await
        }
    };
    assert_eq!(wake("bb").await, Err(PushError::Gone));
    assert_eq!(wake("cc").await, Err(PushError::Gone));
    assert!(matches!(wake("dd").await, Err(PushError::Failed(_))));
    assert!(matches!(wake("ee").await, Err(PushError::Failed(_))));
}

#[test]
fn only_a_requests_alert_asks_to_wake_the_app() {
    let aps_of = |notice: &Notice| payload(notice, None)["aps"].clone();
    assert_eq!(aps_of(&notice())["content-available"], 1);
    let hidden = Notice {
        kind: NoticeKind::Request {
            org: Id::parse("acme").unwrap(),
            team: Id::parse("app").unwrap(),
            request: "r-1".into(),
            team_label: None,
            title: None,
        },
        badge: 1,
    };
    assert_eq!(
        aps_of(&hidden)["content-available"],
        1,
        "also with the titles left out"
    );
    let device = Notice {
        kind: NoticeKind::Device {
            device: Id::parse("d-1").unwrap(),
            label: Some("Tablet".into()),
        },
        badge: 1,
    };
    let decision = Notice {
        kind: NoticeKind::Decision {
            org: Id::parse("acme").unwrap(),
            team: Id::parse("app").unwrap(),
            id: "dc-1".into(),
            title: None,
        },
        badge: 1,
    };
    for other in [device, decision] {
        assert!(
            aps_of(&other).get("content-available").is_none(),
            "{other:?}"
        );
    }
}

const NODE: &str = "02ae509c6b20999646d3c600687ee1bf";

fn every_kind() -> Vec<Notice> {
    let org = || Id::parse("acme").unwrap();
    let team = || Id::parse("app").unwrap();
    vec![
        notice(),
        Notice {
            kind: NoticeKind::Request {
                org: org(),
                team: team(),
                request: "r-1".into(),
                team_label: None,
                title: None,
            },
            badge: 1,
        },
        Notice {
            kind: NoticeKind::Device {
                device: Id::parse("d-a652124afda7").unwrap(),
                label: Some("Pixel".into()),
            },
            badge: 0,
        },
        Notice {
            kind: NoticeKind::Recovery {
                org: org(),
                takes_effect_at: Some("2026-11-29T09:30:00Z".into()),
            },
            badge: 0,
        },
        Notice {
            kind: NoticeKind::Decision {
                org: org(),
                team: team(),
                id: "r-1".into(),
                title: None,
            },
            badge: 0,
        },
    ]
}

#[test]
fn every_kind_of_alert_names_the_node_that_sent_it_and_leaves_the_field_out_without_one() {
    for notice in every_kind() {
        let named = payload(&notice, Some(NODE));
        assert_eq!(named["agency"]["node"], NODE, "{:?}", notice.kind);
        let anonymous = payload(&notice, None);
        assert!(
            anonymous["agency"].get("node").is_none(),
            "{:?}",
            notice.kind
        );
        let mut without = named.clone();
        without["agency"].as_object_mut().unwrap().remove("node");
        assert_eq!(without, anonymous, "nothing else changes");
    }
}

#[test]
fn an_alert_can_be_changed_by_the_extension_and_the_desk_wake_cannot() {
    for notice in every_kind() {
        assert_eq!(payload(&notice, None)["aps"]["mutable-content"], 1);
    }
    let wake = desk_payload(Some(NODE));
    assert!(wake["aps"].get("mutable-content").is_none());
    assert_eq!(
        wake,
        json!({ "aps": { "content-available": 1 }, "agency": { "type": "desk", "node": NODE } }),
        "a wake names its node and carries nothing of the desk"
    );
}

#[tokio::test]
async fn a_node_with_an_origin_names_itself_in_what_it_sends() {
    let (hosts, seen) = stand_in().await;
    let clock = ManualClock::at(T0);
    let apns = apns(hosts, &clock).with_node_id(agency_proto::client::node_id(
        "https://Work.Example.com:443/",
    ));
    apns.send(&token("aa", PushEnvironment::Production), &notice())
        .await
        .unwrap();
    apns.send_desk_changed(&token("aa", PushEnvironment::Production))
        .await
        .unwrap();
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen[0].3["agency"]["node"], NODE);
    assert_eq!(seen[1].3["agency"]["node"], NODE);
}

#[tokio::test]
async fn a_node_that_does_not_know_its_origin_sends_no_node_field() {
    let (hosts, seen) = stand_in().await;
    let clock = ManualClock::at(T0);
    let apns = apns(hosts, &clock).with_node_id(agency_proto::client::node_id("not an origin"));
    apns.send(&token("aa", PushEnvironment::Production), &notice())
        .await
        .unwrap();
    assert!(seen.lock().unwrap()[0].3["agency"].get("node").is_none());
}
