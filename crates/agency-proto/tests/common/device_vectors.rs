//! Builders of the vectors of canonical JSON and of P-256 device keys.
#![allow(dead_code)]
use super::*;

/// Canonical JSON edge cases another implementation must write byte for byte: text people type (non-ASCII,
/// astral emoji, quotes, backslashes, slashes, control characters, U+007F, U+2028, precomposed and
/// decomposed characters, which are never normalized), keys whose UTF-8 order
/// differs from their UTF-16 order, nulls, and integers.
pub fn canonical_cases() -> serde_json::Value {
    let cases = [
        (
            "non-ASCII",
            serde_json::json!({ "text": "リリースは来週に 🚀 Ünïcødé" }),
        ),
        (
            "quotes, backslash, slash",
            serde_json::json!({ "text": "\"quoted\" back\\slash /slash </script>" }),
        ),
        (
            "control characters",
            serde_json::json!({ "text": "line\nnext\ttab\rreturn\u{8}\u{c}\u{0}\u{1f}" }),
        ),
        (
            "precomposed and decomposed, unnormalized",
            serde_json::json!({ "text": "caf\u{e9} cafe\u{301} \u{30ac} \u{30ab}\u{3099}" }),
        ),
        (
            "DEL and line separators",
            serde_json::json!({ "text": "\u{7f} \u{2028} \u{2029}" }),
        ),
        (
            "keys by UTF-8 bytes",
            serde_json::json!({ "😀": 1, "～": 2, "日本": 3, "Z": 4, "a": 5, "": 6 }),
        ),
        (
            "nulls",
            serde_json::json!({ "a": null, "b": [null, 1], "c": { "d": null } }),
        ),
        (
            "integers",
            serde_json::json!({ "zero": 0, "negative": -1, "max": 9_007_199_254_740_991_i64 }),
        ),
    ];
    cases
        .into_iter()
        .map(|(name, value)| {
            let text = agency_proto::canonical::canonical(&value);
            serde_json::json!({ "name": name, "value": value, "canonical": text })
        })
        .collect()
}

/// A P-256 device key from a fixed private scalar, and its signature over the comment envelope's signed
/// text. ECDSA here follows RFC 6979, so the signature repeats; signatures from other signers differ and
/// must still verify.
pub fn p256_device(comment: &str) -> serde_json::Value {
    use base64::Engine;
    use p256::ecdsa::signature::Signer;
    let (key, public) = p256_key(0x0b);
    let message = Kind::Envelope.message(comment);
    let signature: p256::ecdsa::Signature = key.sign(&message);
    let signature = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
    assert!(public.verify(&message, &signature));
    serde_json::json!({
        "privateKey": "0b".repeat(32),
        "publicKey": public.encoded(),
        "fingerprint": public.fingerprint(),
        "pairingCode": public.pairing_code(),
        "deviceId": public.device_id(),
        "commentSignature": signature,
    })
}

/// A P-256 key from a fixed private scalar.
pub fn p256_key(byte: u8) -> (p256::ecdsa::SigningKey, agency_proto::PublicKey) {
    use base64::Engine;
    let key = p256::ecdsa::SigningKey::from_slice(&[byte; 32]).unwrap();
    let point = key.verifying_key().to_sec1_point(false);
    let encoded = base64::engine::general_purpose::STANDARD.encode(point.as_bytes());
    let public = agency_proto::PublicKey::try_from(format!("p256:{encoded}")).unwrap();
    (key, public)
}

/// A device change signed by the P-256 device (scalar `0b`), as the iPhone app signs one: the canonical
/// JSON, the signed text, the signer's device id, and the signature.
pub fn p256_signed_change(change: &DeviceChange) -> serde_json::Value {
    use base64::Engine;
    use p256::ecdsa::signature::Signer;
    let (key, public) = p256_key(0x0b);
    let canonical = canonical_of(change).unwrap();
    let message = Kind::Device.message(&canonical);
    let signature: p256::ecdsa::Signature = key.sign(&message);
    let signature = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
    assert!(public.verify(&message, &signature));
    serde_json::json!({
        "canonical": canonical,
        "signedText": String::from_utf8(message).unwrap(),
        "signer": public.device_id(),
        "signature": signature,
    })
}

/// Device changes signed by a P-256 device: it adds a second P-256 device (a nested `device`) and removes
/// it again (a bare device id).
pub fn device_change_p256() -> serde_json::Value {
    let (_, tablet) = p256_key(0x0c);
    let at = Timestamp::parse(AT).unwrap();
    let add = DeviceChange {
        org: id("acme"),
        account: id("alice"),
        op: DeviceOp::Add {
            device: Box::new(Device {
                id: id("tablet"),
                label: "Tablet".into(),
                public_key: tablet,
                added_at: at.clone(),
            }),
        },
        at: at.clone(),
    };
    let remove = DeviceChange {
        org: id("acme"),
        account: id("alice"),
        op: DeviceOp::Remove {
            device: id("tablet"),
        },
        at,
    };
    serde_json::json!({
        "signerKey": p256_key(0x0b).1.encoded(),
        "add": p256_signed_change(&add),
        "remove": p256_signed_change(&remove),
    })
}
