//! The test vectors: `testdata/vectors.json` is what the protocol's code makes of the fixed keys and values here
//! (`AGENCY_WRITE_VECTORS=1` writes it again; CI fails when an existing entry changes).
use agency_proto::canonical::canonical_of;
use agency_proto::{
    AccountKey, Device, DeviceChange, DeviceOp, EnvelopeId, Kind, Signed, Timestamp,
};
mod common;
use common::device_vectors::*;
use common::org_vectors::*;
use common::read_vectors::*;
use common::*;

#[test]
fn vectors_match_testdata() {
    let key = AccountKey::from_seed([7u8; 32]);
    let device_key = AccountKey::from_seed([9u8; 32]);
    let mut body = envelope(AT);
    body.id = EnvelopeId::try_from("00112233445566778899aabbccddeeff".to_owned()).unwrap();
    let signed = Signed::sign(Kind::Envelope, body.clone(), id("laptop"), &device_key).unwrap();
    let org = genesis(&key);
    let first_device = DeviceChange {
        org: id("acme"),
        account: id("alice"),
        op: DeviceOp::Add {
            device: Box::new(Device {
                id: id("laptop"),
                label: "Laptop".into(),
                public_key: device_key.public(),
                added_at: Timestamp::parse(AT).unwrap(),
            }),
        },
        at: Timestamp::parse(AT).unwrap(),
    };
    let change = Signed::sign(Kind::Device, first_device.clone(), id("alice"), &key).unwrap();
    let comment = comment_envelope();
    let signed_comment =
        Signed::sign(Kind::Envelope, comment.clone(), id("laptop"), &device_key).unwrap();
    let actual = serde_json::json!({
        "seed": "0707070707070707070707070707070707070707070707070707070707070707",
        "publicKey": key.public().encoded(),
        "fingerprint": key.public().fingerprint(),
        "device": {
            "seed": "0909090909090909090909090909090909090909090909090909090909090909",
            "publicKey": device_key.public().encoded(),
            "pairingCode": device_key.public().pairing_code(),
        },
        "envelope": { "canonical": canonical_of(&body).unwrap(), "signer": "laptop", "signature": signed.signature },
        "deviceChange": { "canonical": canonical_of(&first_device).unwrap(), "signer": "alice", "signature": change.signature },
        "org": { "canonical": canonical_of(&org.body).unwrap(), "signature": org.signature, "digest": org.digest() },
        "commentEnvelope": { "canonical": canonical_of(&comment).unwrap(), "signer": "laptop", "signature": signed_comment.signature },
        "canonical": canonical_cases(),
        "p256Device": p256_device(&canonical_of(&comment).unwrap()),
        "orgWithPlan": org_with_plan(&key, &org),
        "orgWithSeats": org_with_seats(&key, &org),
        "orgWithClaimedSeat": org_with_a_claimed_seat(&key, &org),
        "orgWithHostedMember": org_with_hosted_member(&key, &org),
        "rootRotation": root_rotation(&key, &org),
        "rootRecovery": root_recovery(&key, &org),
        "orgWithOutcomes": org_with_outcomes(&key, &org),
        "deviceChangeP256": device_change_p256(),
        "signedRead": signed_read(&key),
        "viewTag": view_tag_vector(),
    });
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/vectors.json");
    if std::env::var("AGENCY_WRITE_VECTORS").is_ok() {
        std::fs::write(path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
    }
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(path)
            .expect("testdata/vectors.json is missing (write it with AGENCY_WRITE_VECTORS=1)"),
    )
    .unwrap();
    assert_eq!(actual, expected);
}
