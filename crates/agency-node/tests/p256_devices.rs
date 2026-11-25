//! A P-256 device key end to end: a phone asks to be added with a key it holds in hardware, the terminal
//! adds it, and its decisions are applied through the Client API like an Ed25519 device's.
mod common;

use agency_core::devices::{add_device, Added};
use agency_proto::canonical::canonical_of;
use agency_proto::{Envelope, EnvelopeId, EnvelopeKind, Kind, Payload, PublicKey, Signed};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use common::{alice, body, id, post, Running, ENVELOPES, T0, TOKEN};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use reqwest::StatusCode;
use serde_json::{json, Value};

fn phone_key() -> (SigningKey, PublicKey) {
    let key = SigningKey::from_slice(&[0x0b; 32]).unwrap();
    let point = key.verifying_key().to_encoded_point(false);
    let public =
        PublicKey::try_from(format!("p256:{}", STANDARD.encode(point.as_bytes()))).unwrap();
    (key, public)
}

async fn send(running: &Running, path: &str, payload: &Value) -> (StatusCode, Value) {
    let request = post(running, path, payload).header("x-agency-token", TOKEN);
    body(request.send().await.unwrap()).await
}

/// A comment on `target` signed by the phone, as the iOS app signs it.
fn signed_comment(key: &SigningKey, device: &PublicKey, target: &str) -> Signed<Envelope> {
    let body = Envelope {
        id: EnvelopeId::from_bytes([0x0b; 16]),
        kind: EnvelopeKind::Answer,
        org: id("acme"),
        team: id("app"),
        target: target.into(),
        payload: Payload {
            text: "来週にしてください".into(),
            choice: None,
        },
        account: id("alice"),
        at: T0.into(),
    };
    let message = Kind::Envelope.message(&canonical_of(&body).unwrap());
    let signature: Signature = key.sign(&message);
    Signed {
        body,
        signer: device.device_id(),
        signature: STANDARD.encode(signature.to_bytes()),
    }
}

#[tokio::test]
async fn a_phone_with_a_p256_key_is_added_and_decides() {
    let (alice, memory) = alice().await;
    let (key, phone) = phone_key();
    let request = json!({ "label": "iPhone", "publicKey": phone.encoded() });
    let (status, asked) = send(&alice, "/api/devices/requests", &request).await;
    assert_eq!(status, 200, "{asked}");
    assert_eq!(asked["pairingCode"], phone.pairing_code());

    let added = add_device(&alice.node, "iPhone", &phone, None).await;
    assert!(
        added
            .iter()
            .all(|(_, outcome)| matches!(outcome, Added::Done(_))),
        "{added:?}"
    );

    let mut forged = signed_comment(&key, &phone, "r-1");
    forged.body.payload.text = "Ship it".into();
    let (status, _) = send(&alice, ENVELOPES, &json!({ "envelope": forged })).await;
    assert_eq!(status, 401, "a changed body does not match the signature");

    let envelope = signed_comment(&key, &phone, "r-1");
    let (status, result) = send(&alice, ENVELOPES, &json!({ "envelope": envelope })).await;
    assert_eq!(
        (status, &result["outcome"]),
        (StatusCode::OK, &json!("applied")),
        "{result}"
    );
    let applied = memory.lock().unwrap().applied.clone();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].by, id("alice"));
}
