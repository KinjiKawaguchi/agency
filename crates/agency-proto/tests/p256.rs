//! P-256 device keys: the one text a key may have, ECDSA signatures over the same signed text as Ed25519,
//! fingerprints over the key's 65 bytes, and that only a device key may be P-256.
use agency_proto::canonical::canonical_of;
use agency_proto::device::{check_device_change, envelope_signer};
use agency_proto::org::Root;
use agency_proto::{
    AccountKey, Device, DeviceChange, DeviceOp, Envelope, EnvelopeId, EnvelopeKind, Id, Kind,
    OrgDefinition, Payload, Person, PublicKey, Signed, Timestamp,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use sha2::{Digest, Sha256};

const AT: &str = "2026-11-24T12:00:00Z";

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn signing_key(byte: u8) -> SigningKey {
    SigningKey::from_slice(&[byte; 32]).unwrap()
}

/// The 65-byte uncompressed point of `key`.
fn point(key: &SigningKey) -> Vec<u8> {
    key.verifying_key().to_sec1_point(false).as_bytes().to_vec()
}

fn public(key: &SigningKey) -> PublicKey {
    PublicKey::try_from(format!("p256:{}", STANDARD.encode(point(key)))).unwrap()
}

/// A raw `r‖s` signature over `message` in base64.
fn sign(key: &SigningKey, message: &[u8]) -> String {
    let signature: Signature = key.sign(message);
    STANDARD.encode(signature.to_bytes())
}

#[test]
fn a_p256_key_has_exactly_one_text() {
    let key = signing_key(3);
    let text = format!("p256:{}", STANDARD.encode(point(&key)));
    let read = PublicKey::try_from(text.clone()).unwrap();
    assert_eq!(read.encoded(), text);
    assert!(!read.is_ed25519());

    let compressed = key.verifying_key().to_sec1_point(true);
    let mut off_curve = point(&key);
    off_curve[64] ^= 1;
    let mut hybrid = point(&key);
    hybrid[0] = 0x06;
    let refused = [
        ("compressed", STANDARD.encode(compressed.as_bytes())),
        ("off the curve", STANDARD.encode(&off_curve)),
        ("not the uncompressed tag", STANDARD.encode(&hybrid)),
        ("too short", STANDARD.encode(&point(&key)[..64])),
        ("not base64", "not base64!".into()),
    ];
    for (name, encoded) in refused {
        assert!(
            PublicKey::try_from(format!("p256:{encoded}")).is_err(),
            "{name}"
        );
    }
    assert!(
        PublicKey::try_from(STANDARD.encode(point(&key))).is_err(),
        "a P-256 point without the prefix is not an Ed25519 key"
    );
}

#[test]
fn fingerprint_pairing_code_and_device_id_hash_the_65_key_bytes() {
    let key = signing_key(3);
    let digest = Sha256::digest(point(&key));
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    let read = public(&key);
    assert_eq!(read.fingerprint(), hex);
    assert_eq!(
        read.pairing_code(),
        format!("{}-{}-{}", &hex[..4], &hex[4..8], &hex[8..12])
    );
    assert_eq!(read.device_id(), id(&format!("d-{}", &hex[..12])));
}

#[test]
fn signatures_are_ecdsa_sha256_as_raw_r_and_s() {
    let key = signing_key(3);
    let read = public(&key);
    let signature = sign(&key, b"hello");
    assert!(read.verify(b"hello", &signature));
    assert!(!read.verify(b"hellO", &signature));
    assert!(!public(&signing_key(4)).verify(b"hello", &signature));

    let parsed = Signature::from_slice(&STANDARD.decode(&signature).unwrap()).unwrap();
    let der = STANDARD.encode(parsed.to_der().as_bytes());
    assert!(!read.verify(b"hello", &der), "DER is not the encoding");
    let (r, s) = parsed.split_scalars();
    let high_s = Signature::from_scalars(r, -*s).unwrap();
    assert!(
        read.verify(b"hello", &STANDARD.encode(high_s.to_bytes())),
        "either S is accepted; replay protection does not depend on the signature bytes"
    );
    let ed25519 = AccountKey::from_seed([9; 32]).sign(b"hello");
    assert!(!read.verify(b"hello", &ed25519));
}

fn person(account: &str, key: PublicKey) -> Person {
    Person::new(id(account), account.to_uppercase(), id(account), key)
}

fn org(root: PublicKey, bob: PublicKey) -> OrgDefinition {
    OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        Root::new(id("alice"), root.clone()),
        AT.into(),
    )
    .with_people(vec![person("alice", root), person("bob", bob)])
}

#[test]
fn only_a_device_key_may_be_p256() {
    let alice = AccountKey::from_seed([1; 32]).public();
    let bob = AccountKey::from_seed([2; 32]);
    let p256 = public(&signing_key(3));
    assert_eq!(org(alice.clone(), bob.public()).validate(), Ok(()));
    assert!(org(p256.clone(), bob.public()).validate().is_err(), "root");
    assert!(
        org(alice.clone(), p256.clone()).validate().is_err(),
        "account"
    );

    let phone = Device {
        id: p256.device_id(),
        label: "iPhone".into(),
        public_key: p256,
        added_at: Timestamp::parse(AT).unwrap(),
    };
    let add = DeviceChange {
        org: id("acme"),
        account: id("bob"),
        op: DeviceOp::Add {
            device: Box::new(phone),
        },
        at: Timestamp::parse(AT).unwrap(),
    };
    let signed = Signed::sign(Kind::Device, add, id("bob"), &bob).unwrap();
    let now = agency_proto::clock::parse(AT).unwrap();
    let with_phone = check_device_change(&org(alice, bob.public()), &signed, now).unwrap();
    assert_eq!(with_phone.validate(), Ok(()));
}

#[test]
fn a_p256_device_signs_a_decision() {
    let bob = AccountKey::from_seed([2; 32]);
    let key = signing_key(3);
    let phone = public(&key);
    let mut current = org(AccountKey::from_seed([1; 32]).public(), bob.public());
    current.people[1].devices.push(Device {
        id: phone.device_id(),
        label: "iPhone".into(),
        public_key: phone.clone(),
        added_at: Timestamp::parse(AT).unwrap(),
    });
    let body = Envelope {
        id: EnvelopeId::from_bytes([1; 16]),
        kind: EnvelopeKind::Answer,
        org: id("acme"),
        team: id("app"),
        target: "r-1".into(),
        payload: Payload {
            text: "了解".into(),
            choice: None,
        },
        account: id("bob"),
        at: AT.into(),
    };
    let text = canonical_of(&body).unwrap();
    let decision = |signature: String| Signed {
        body: body.clone(),
        signer: phone.device_id(),
        signature,
    };
    let signed = decision(sign(&key, &Kind::Envelope.message(&text)));
    assert_eq!(
        envelope_signer(&current, &signed).map(|p| &p.id),
        Ok(&id("bob"))
    );
    let as_device_change = decision(sign(&key, &Kind::Device.message(&text)));
    assert!(
        envelope_signer(&current, &as_device_change).is_err(),
        "the kind line is signed"
    );
}
