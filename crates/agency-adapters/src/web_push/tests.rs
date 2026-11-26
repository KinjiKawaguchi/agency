use super::*;
use crate::memory::ManualClock;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};
use agency_proto::{Id, Timestamp};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use hkdf::Hkdf;
use p256::ecdh::diffie_hellman;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::{PublicKey, SecretKey};
use std::sync::Mutex;

const T0: &str = "2026-11-24T12:00:00Z";

/// A browser: its subscription keys, and what decrypts a message to it.
struct Browser {
    secret: SecretKey,
    auth: [u8; 16],
}

impl Browser {
    fn new() -> Self {
        Self {
            secret: SecretKey::random(&mut rand_core::OsRng),
            auth: [3u8; 16],
        }
    }

    fn public(&self) -> Vec<u8> {
        self.secret.public_key().to_sec1_bytes().to_vec()
    }

    fn subscription(&self, endpoint: &str) -> WebSubscription {
        WebSubscription {
            id: "abc".into(),
            endpoint: endpoint.into(),
            p256dh: B64.encode(self.public()),
            auth: B64.encode(self.auth),
            label: "Firefox".into(),
            registered_at: Timestamp::parse(T0).unwrap(),
            events: vec![agency_proto::client::PushEvent::Request],
            key_id: "k".into(),
        }
    }

    /// RFC 8291, section 3.4, from the receiving side.
    fn decrypt(&self, body: &[u8]) -> Vec<u8> {
        let salt = &body[..16];
        let key_length = usize::from(body[20]);
        let as_public = &body[21..21 + key_length];
        let record = &body[21 + key_length..];
        let sender = PublicKey::from_sec1_bytes(as_public).unwrap();
        let shared = diffie_hellman(self.secret.to_nonzero_scalar(), sender.as_affine());
        let mut key_info = b"WebPush: info\0".to_vec();
        key_info.extend_from_slice(&self.public());
        key_info.extend_from_slice(as_public);
        let mut ikm = [0u8; 32];
        Hkdf::<Sha256>::new(Some(&self.auth), shared.raw_secret_bytes())
            .expand(&key_info, &mut ikm)
            .unwrap();
        let keys = Hkdf::<Sha256>::new(Some(salt), &ikm);
        let (mut cek, mut nonce) = ([0u8; 16], [0u8; 12]);
        keys.expand(b"Content-Encoding: aes128gcm\0", &mut cek)
            .unwrap();
        keys.expand(b"Content-Encoding: nonce\0", &mut nonce)
            .unwrap();
        let mut plain = Aes128Gcm::new((&cek).into())
            .decrypt(Nonce::from_slice(&nonce), record)
            .unwrap();
        assert_eq!(plain.pop(), Some(0x02), "the delimiter of the last record");
        plain
    }
}

fn request_notice() -> Notice {
    Notice {
        kind: NoticeKind::Request {
            org: Id::parse("acme").unwrap(),
            team: Id::parse("app").unwrap(),
            request: "r-1".into(),
            team_label: Some("App team".into()),
            title: Some("Ship the release?".into()),
        },
        badge: 3,
    }
}

type Seen = Arc<Mutex<Vec<(String, HeaderMap, Vec<u8>)>>>;

/// A stand-in push service: answers with the status in the path (`/status/410`) and records each request.
async fn stand_in() -> (String, Seen) {
    let seen: Seen = Arc::default();
    let handler = |State(seen): State<Seen>,
                   axum::extract::Path(code): axum::extract::Path<u16>,
                   headers: HeaderMap,
                   body: axum::body::Bytes| async move {
        seen.lock()
            .unwrap()
            .push((format!("/status/{code}"), headers, body.to_vec()));
        if code == 307 {
            return (
                StatusCode::TEMPORARY_REDIRECT,
                [("location", "/status/201")],
            )
                .into_response();
        }
        StatusCode::from_u16(code).unwrap().into_response()
    };
    let other = |State(seen): State<Seen>, headers: HeaderMap| async move {
        seen.lock()
            .unwrap()
            .push(("/other".into(), headers, vec![]));
        StatusCode::OK
    };
    let app = axum::Router::new()
        .route("/status/{code}", axum::routing::post(handler))
        .route("/other", axum::routing::post(other))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, seen)
}

use axum::response::IntoResponse;

fn web_push() -> WebPush {
    WebPush::for_tests(Arc::new(ManualClock::at(T0)))
}

#[tokio::test]
async fn a_message_reaches_the_browser_encrypted_and_signed() {
    let (url, seen) = stand_in().await;
    let browser = Browser::new();
    let sender = web_push();
    sender
        .send(
            &browser.subscription(&format!("{url}/status/201")),
            &request_notice(),
        )
        .await
        .unwrap();

    let (_, headers, body) = seen.lock().unwrap()[0].clone();
    let message: Value = serde_json::from_slice(&browser.decrypt(&body)).unwrap();
    assert_eq!(
        message,
        json!({
            "agency": { "org": "acme", "team": "app", "request": "r-1" },
            "title": "App team", "body": "Ship the release?", "badge": 3
        })
    );
    let header = |name: &str| headers.get(name).unwrap().to_str().unwrap().to_owned();
    assert_eq!(header("content-encoding"), "aes128gcm");
    assert_eq!(header("ttl"), "3600");
    assert_eq!(header("urgency"), "high");
    assert!(header("topic").len() <= 32 && !header("topic").contains("r-1"));
    let authorization = header("authorization");
    let (jwt, key) = authorization
        .strip_prefix("vapid t=")
        .and_then(|rest| rest.split_once(", k="))
        .unwrap();
    assert_eq!(key, sender.key().public_key);
    let (signed, signature) = jwt.rsplit_once('.').unwrap();
    let verifying = VerifyingKey::from_sec1_bytes(&B64.decode(key).unwrap()).unwrap();
    let signature = Signature::from_slice(&B64.decode(signature).unwrap()).unwrap();
    assert!(verifying.verify(signed.as_bytes(), &signature).is_ok());
    let claims: Value =
        serde_json::from_slice(&B64.decode(signed.split('.').nth(1).unwrap()).unwrap()).unwrap();
    assert_eq!(claims["aud"], url, "the push service's origin");
    assert_eq!(claims["sub"], "mailto:ops@acme.example");
    assert_eq!(
        claims["exp"],
        ManualClock::at(T0).now().unix_timestamp() + 12 * 3600,
        "twelve hours from now"
    );
}

#[tokio::test]
async fn the_answers_of_a_push_service_map_to_sent_gone_and_failed() {
    let (url, _) = stand_in().await;
    let browser = Browser::new();
    let sender = web_push();
    let answer = |code: u16| {
        let subscription = browser.subscription(&format!("{url}/status/{code}"));
        let sender = &sender;
        async move { sender.send(&subscription, &request_notice()).await }
    };
    assert_eq!(answer(201).await, Ok(()));
    assert_eq!(answer(404).await, Err(PushError::Gone));
    assert_eq!(answer(410).await, Err(PushError::Gone));
    for code in [400, 401, 403, 413, 429, 500, 503] {
        let Err(PushError::Failed(why)) = answer(code).await else {
            panic!("{code} fails and is tried again");
        };
        assert!(
            why.contains(&code.to_string()) && !why.contains("status/"),
            "{why}"
        );
    }
}

#[tokio::test]
async fn a_redirect_is_not_followed() {
    let (url, seen) = stand_in().await;
    let result = web_push()
        .send(
            &Browser::new().subscription(&format!("{url}/status/307")),
            &request_notice(),
        )
        .await;
    assert!(matches!(result, Err(PushError::Failed(_))), "{result:?}");
    assert_eq!(
        seen.lock().unwrap().len(),
        1,
        "only the first address was asked"
    );
}

#[tokio::test]
async fn a_subscription_whose_keys_are_not_a_browsers_is_gone_without_a_request() {
    let (url, seen) = stand_in().await;
    let endpoint = format!("{url}/status/201");
    let browser = Browser::new();
    let mut not_base64 = browser.subscription(&endpoint);
    not_base64.auth = "***".into();
    let mut short_auth = browser.subscription(&endpoint);
    short_auth.auth = B64.encode([1u8; 8]);
    let mut not_a_point = browser.subscription(&endpoint);
    not_a_point.p256dh = B64.encode([4u8; 65]);
    for subscription in [not_base64, short_auth, not_a_point] {
        assert_eq!(
            web_push().send(&subscription, &request_notice()).await,
            Err(PushError::Gone)
        );
    }
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_node_sends_only_to_an_allowed_https_push_service() {
    let dir = tempfile::tempdir().unwrap();
    let sender = WebPush::new(
        &WebPushConfig {
            subject: None,
            extra_hosts: vec!["push.acme.example".into()],
        },
        "https://node.acme.example".into(),
        &dir.path().join("vapid.key"),
        Arc::new(ManualClock::at(T0)),
    )
    .unwrap();
    assert_eq!(sender.key().key_id.len(), 12);
    let browser = Browser::new();
    for endpoint in [
        "http://fcm.googleapis.com/fcm/send/x",
        "https://evil.example/fcm/send/x",
        "https://127.0.0.1/x",
        "https://localhost/x",
        "https://fcm.googleapis.com@evil.example/x",
        "https://[::1]/x",
        "https://fcm.googleapis.com:8443/x",
    ] {
        assert_eq!(
            sender
                .send(&browser.subscription(endpoint), &request_notice())
                .await,
            Err(PushError::Gone),
            "{endpoint}"
        );
    }
    // An allowed name that resolves inside the machine is refused when connecting, not sent to.
    let result = sender
        .send(
            &browser.subscription("https://push.acme.example/x"),
            &request_notice(),
        )
        .await;
    assert!(
        matches!(result, Err(PushError::Failed(_)) | Err(PushError::Gone)),
        "{result:?}"
    );
}

#[test]
fn what_the_worker_is_told_leaves_out_the_text_the_node_does_not_have() {
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
        payload(&hidden),
        json!({ "agency": { "org": "acme", "team": "app", "request": "r-1" }, "badge": 1 })
    );
    let device = |label: Option<&str>| Notice {
        kind: NoticeKind::Device {
            device: Id::parse("d-abcdef123456").unwrap(),
            label: label.map(str::to_owned),
        },
        badge: 0,
    };
    assert_eq!(
        payload(&device(Some("Pixel"))),
        json!({ "agency": { "type": "device" }, "badge": 0, "detail": "Pixel" })
    );
    assert_eq!(
        payload(&device(None)),
        json!({ "agency": { "type": "device" }, "badge": 0 })
    );
    let decision = Notice {
        kind: NoticeKind::Decision {
            org: Id::parse("acme").unwrap(),
            team: Id::parse("app").unwrap(),
            id: "dec-1".into(),
            title: Some("x".repeat(500)),
        },
        badge: 0,
    };
    let message = payload(&decision);
    assert_eq!(
        message["agency"],
        json!({ "type": "decision", "org": "acme", "team": "app", "id": "dec-1" })
    );
    assert_eq!(
        message["detail"].as_str().unwrap().chars().count(),
        TEXT_CHARS,
        "a long title is cut"
    );
    assert_eq!(urgency(&device(None)), "normal");
}

#[tokio::test]
async fn the_client_of_a_real_node_does_not_connect_to_a_name_that_resolves_inside_the_machine() {
    let (url, seen) = stand_in().await;
    let port = url.rsplit(':').next().unwrap();
    let address = format!("http://localhost:{port}/other");
    // Without the filter, the stand-in is reached through the name: the check below is not vacuous.
    assert!(web_push().http.post(&address).send().await.is_ok());
    assert_eq!(seen.lock().unwrap().len(), 1);

    let dir = tempfile::tempdir().unwrap();
    let node_sender = WebPush::new(
        &WebPushConfig::default(),
        "https://node.acme.example".into(),
        &dir.path().join("vapid.key"),
        Arc::new(ManualClock::at(T0)),
    )
    .unwrap();
    assert!(node_sender.http.post(&address).send().await.is_err());
    assert_eq!(
        seen.lock().unwrap().len(),
        1,
        "the stand-in saw nothing more"
    );
}
