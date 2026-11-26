//! Tests of the protocol's deciding rules: the chain of org definition versions (one writer, the root key), envelope shape and expiry, and the test vectors.
//! The non-ASCII sample values ("開発", "ア") are on purpose: the test vectors pin UTF-8 in canonical JSON.
use agency_proto::canonical::canonical_of;
use agency_proto::org::{
    verify_genesis, verify_successor, Allocation, Department, Root, SignedOrg, TeamPlace, TeamPlan,
};
use agency_proto::outcome::{
    Aim, Currency, GoodWhen, Kpi, KpiKind, KpiUnit, EPIC, MAX_AIMS, MAX_KPIS,
};
use agency_proto::{
    clock, AccountKey, Device, DeviceChange, DeviceOp, Envelope, EnvelopeId, EnvelopeKind, Id,
    Kind, OrgDefinition, Payload, Person, Role, Signed, Timestamp,
};

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

const AT: &str = "2026-11-24T12:00:00Z";

fn genesis(root: &AccountKey) -> SignedOrg {
    let body = OrgDefinition {
        kpis: vec![],
        org: id("acme"),
        label: "Acme".into(),
        version: 1,
        prev: None,
        root: Root {
            account: id("alice"),
            public_key: root.public(),
        },
        departments: vec![Department {
            id: id("dev"),
            label: "開発".into(),
            parent: None,
            head: Some(id("owner")),
        }],
        people: vec![Person {
            id: id("owner"),
            label: "Alice".into(),
            account: id("alice"),
            public_key: root.public(),
            department: Some(id("dev")),
            role: Role::Member,
            devices: vec![],
        }],
        teams: vec![TeamPlace {
            id: id("app"),
            label: "app".into(),
            department: Some(id("dev")),
            node: "https://alice.example".into(),
            plan: None,
        }],
        revoked: vec![],
        at: AT.into(),
    };
    Signed::sign(Kind::Org, body, id("alice"), root).unwrap()
}

fn successor(
    current: &SignedOrg,
    signer: &AccountKey,
    edit: impl FnOnce(&mut OrgDefinition),
) -> SignedOrg {
    let mut body = OrgDefinition::successor_of(current, AT.into());
    edit(&mut body);
    Signed::sign(Kind::Org, body, id("alice"), signer).unwrap()
}

#[test]
fn org_chain_accepts_only_the_next_version_signed_by_the_root() {
    let root = AccountKey::generate();
    let v1 = genesis(&root);
    assert_eq!(verify_genesis(&v1), Ok(()));
    let bob = AccountKey::generate();
    let v2 = successor(&v1, &root, |body| {
        body.people.push(Person {
            id: id("bob"),
            label: "Bob".into(),
            account: id("bob"),
            public_key: bob.public(),
            department: Some(id("dev")),
            role: Role::Answerer,
            devices: vec![],
        })
    });
    assert_eq!(verify_successor(&v1, &v2), Ok(()));
    assert!(v2.body.member(&id("bob")).is_some());
    // Refused: signed by a key that is not the root, a skipped version, a wrong previous fingerprint, and the same version twice.
    assert!(verify_successor(&v1, &successor(&v1, &bob, |_| ())).is_err());
    assert!(verify_successor(&v1, &successor(&v2, &root, |_| ())).is_err());
    assert!(verify_successor(
        &v1,
        &successor(&v1, &root, |body| body.prev = Some("0".repeat(64)))
    )
    .is_err());
    assert!(verify_successor(&v2, &v2).is_err());
}

#[test]
fn revoked_keys_are_not_members() {
    let root = AccountKey::generate();
    let v1 = genesis(&root);
    let v2 = successor(&v1, &root, |body| body.revoked.push(root.public()));
    assert!(v1.body.member(&id("alice")).is_some());
    assert!(v2.body.member(&id("alice")).is_none());
}

#[test]
fn org_shape_is_checked() {
    let root = AccountKey::generate();
    let v1 = genesis(&root);
    let broken = successor(&v1, &root, |body| body.teams[0].node = "file:///etc".into());
    assert!(verify_successor(&v1, &broken).is_err());
    let dangling = successor(&v1, &root, |body| {
        body.people[0].department = Some(id("nowhere"))
    });
    assert!(verify_successor(&v1, &dangling).is_err());
}

fn envelope(at: &str) -> Envelope {
    Envelope {
        id: EnvelopeId::from_bytes([1; 16]),
        kind: EnvelopeKind::Answer,
        org: id("acme"),
        team: id("app"),
        target: "r-20260101-000000-abcd".into(),
        payload: Payload {
            text: String::new(),
            choice: Some("ア".into()),
        },
        account: id("alice"),
        at: at.into(),
    }
}

#[test]
fn envelope_shape_and_freshness() {
    let now = clock::parse("2026-11-24T12:00:00Z").unwrap();
    assert_eq!(envelope(AT).validate(), Ok(()));
    assert!(envelope("2026-11-18T12:00:00Z").is_fresh(now));
    assert!(
        !envelope("2026-11-16T12:00:00Z").is_fresh(now),
        "8 days old is expired"
    );
    assert!(
        !envelope("2026-11-24T12:10:00Z").is_fresh(now),
        "10 minutes ahead is beyond the allowed clock skew"
    );
    let mut empty = envelope(AT);
    empty.payload.choice = None;
    assert!(empty.validate().is_err());
    let mut bad_target = envelope(AT);
    bad_target.target = "../../etc".into();
    assert!(bad_target.validate().is_err());
}

/// docs/protocol.md, "Item references": a target is 1 to 120 bytes of ASCII letters, digits, and `._#-`.
#[test]
fn an_envelope_target_follows_the_documented_character_set_and_length() {
    let with_target = |target: &str| {
        let mut body = envelope(AT);
        body.target = target.into();
        body.validate()
    };
    for accepted in ["app#12", "ENG-123", "a.b_c-D#9", &"a".repeat(120)] {
        assert_eq!(with_target(accepted), Ok(()), "{accepted}");
    }
    for refused in [
        "",
        "owner/repo#12",
        "PROJ:12",
        "app 12",
        "caf\u{e9}#1",
        &"a".repeat(121),
    ] {
        assert!(with_target(refused).is_err(), "{refused}");
    }
}

#[test]
fn signature_is_bound_to_its_kind() {
    let key = AccountKey::generate();
    let signed = Signed::sign(Kind::Envelope, envelope(AT), id("alice"), &key).unwrap();
    assert!(signed.verify(Kind::Envelope, &key.public()));
    assert!(
        !signed.verify(Kind::Org, &key.public()),
        "an envelope signature cannot be used as an org definition signature"
    );
}

/// Canonical JSON edge cases another implementation must write byte for byte: text people type (non-ASCII,
/// astral emoji, quotes, backslashes, slashes, control characters, U+007F, U+2028, precomposed and
/// decomposed characters, which are never normalized), keys whose UTF-8 order
/// differs from their UTF-16 order, nulls, and integers.
fn canonical_cases() -> serde_json::Value {
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
fn p256_device(comment: &str) -> serde_json::Value {
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
fn p256_key(byte: u8) -> (p256::ecdsa::SigningKey, agency_proto::PublicKey) {
    use base64::Engine;
    let key = p256::ecdsa::SigningKey::from_slice(&[byte; 32]).unwrap();
    let point = key.verifying_key().to_encoded_point(false);
    let encoded = base64::engine::general_purpose::STANDARD.encode(point.as_bytes());
    let public = agency_proto::PublicKey::try_from(format!("p256:{encoded}")).unwrap();
    (key, public)
}

/// A device change signed by the P-256 device (scalar `0b`), as the iPhone app signs one: the canonical
/// JSON, the signed text, the signer's device id, and the signature.
fn p256_signed_change(change: &DeviceChange) -> serde_json::Value {
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
fn device_change_p256() -> serde_json::Value {
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

/// A comment: text and no choice.
fn comment_envelope() -> Envelope {
    let mut body = envelope(AT);
    body.id = EnvelopeId::try_from("ffeeddccbbaa99887766554433221100".to_owned()).unwrap();
    body.payload = Payload {
        text: "来週にしてください 🙏\n\"理由\"は別途".into(),
        choice: None,
    };
    body
}

/// Test vectors: canonical JSON and signatures of fixed bodies with keys from fixed seeds: the account key (seed 7) signs
/// the org definition and the first device change, the device key (seed 9) signs the envelope and a comment, and a
/// P-256 device key signs the same comment and two device changes. Another implementation
/// must produce the same values.
/// The next version of `org`, with a plan for its team: how a plan is signed.
fn org_with_plan(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    body.teams[0].plan = Some(TeamPlan {
        aims: vec![],
        allocation: Allocation {
            new_feature: 40,
            improvement: 20,
            maintenance: 20,
            debt: 15,
            incident: 5,
        },
        weekly_budget_usd: 500,
    });
    let signed = Signed::sign(Kind::Org, body, id("alice"), root).unwrap();
    assert!(verify_successor(org, &signed).is_ok());
    serde_json::json!({ "canonical": canonical_of(&signed.body).unwrap(), "signature": signed.signature, "digest": signed.digest() })
}

fn allocation() -> Allocation {
    Allocation {
        new_feature: 40,
        improvement: 20,
        maintenance: 20,
        debt: 15,
        incident: 5,
    }
}

fn kpi(raw: &str, label: &str, unit: KpiUnit) -> Kpi {
    Kpi {
        id: id(raw),
        label: label.into(),
        kind: KpiKind::Business,
        unit,
        good_when: GoodWhen::Higher,
    }
}

fn aim(epic: &str, kpi: &str) -> Aim {
    Aim {
        epic: epic.into(),
        kpi: id(kpi),
    }
}

/// The next version of `org`, with KPIs and a team plan that aims at one: how what an org measures is signed.
fn org_with_outcomes(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    let yen = KpiUnit::Money {
        currency: Currency::parse("JPY").unwrap(),
    };
    let mut activation = kpi("activation", "Activation", KpiUnit::Percent);
    activation.kind = KpiKind::Product;
    let mut lead_time = kpi("lead-time", "Lead time of a feature", KpiUnit::Seconds);
    lead_time.kind = KpiKind::Product;
    lead_time.good_when = GoodWhen::Lower;
    body.kpis = vec![kpi("revenue", "Revenue", yen), activation, lead_time];
    let plan = TeamPlan::new(allocation(), 500).with_aims(vec![aim("app#10", "activation")]);
    body.teams[0].plan = Some(plan);
    let signed = Signed::sign(Kind::Org, body, id("alice"), root).unwrap();
    assert!(verify_successor(org, &signed).is_ok());
    serde_json::json!({ "canonical": canonical_of(&signed.body).unwrap(), "signature": signed.signature, "digest": signed.digest() })
}

#[test]
fn what_an_org_measures_is_left_out_of_a_version_that_states_nothing() {
    let key = AccountKey::from_seed([7u8; 32]);
    let canonical = canonical_of(&genesis(&key).body).unwrap();
    assert!(
        !canonical.contains("kpis") && !canonical.contains("aims"),
        "{canonical}"
    );
}

#[test]
fn a_kpi_is_written_with_its_unit_and_currency_beside_each_other() {
    let yen = kpi(
        "revenue",
        "Revenue",
        KpiUnit::Money {
            currency: Currency::parse("JPY").unwrap(),
        },
    );
    let written = serde_json::to_value(&yen).unwrap();
    assert_eq!(
        written,
        serde_json::json!({ "id": "revenue", "label": "Revenue", "kind": "business", "unit": "money", "currency": "JPY", "goodWhen": "higher" })
    );
    assert_eq!(serde_json::from_value::<Kpi>(written).unwrap(), yen);
    let seconds = serde_json::to_value(kpi("lead-time", "Lead time", KpiUnit::Seconds)).unwrap();
    assert_eq!(seconds["unit"], "seconds");
    assert!(seconds.get("currency").is_none());
    for bad in [
        serde_json::json!({ "id": "r", "label": "R", "kind": "business", "unit": "usd", "goodWhen": "higher" }),
        serde_json::json!({ "id": "r", "label": "R", "kind": "business", "unit": "money", "goodWhen": "higher" }),
        serde_json::json!({ "id": "r", "label": "R", "kind": "business", "unit": "money", "currency": "yen", "goodWhen": "higher" }),
    ] {
        assert!(serde_json::from_value::<Kpi>(bad.clone()).is_err(), "{bad}");
    }
    for bad in ["jpy", "JP", "JPYY", "J1Y", "", "円円円"] {
        assert!(Currency::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn what_an_org_measures_and_aims_at_is_checked_by_every_node() {
    let key = AccountKey::from_seed([7u8; 32]);
    let with = |kpis: Vec<Kpi>, aims: Vec<Aim>| {
        let mut body = genesis(&key).body;
        body.kpis = kpis;
        body.teams[0].plan = Some(TeamPlan::new(allocation(), 0).with_aims(aims));
        body.validate()
    };
    let count = || kpi("count", "Count", KpiUnit::Count);
    assert_eq!(with(vec![count()], vec![aim("app#1", "count")]), Ok(()));
    assert_eq!(with(vec![], vec![]), Ok(()));
    assert!(with(vec![count(), count()], vec![]).is_err(), "a KPI twice");
    assert!(
        with(vec![kpi("count", "", KpiUnit::Count)], vec![]).is_err(),
        "a label"
    );
    let many: Vec<Kpi> = (0..=MAX_KPIS)
        .map(|n| kpi(&format!("k{n}"), "K", KpiUnit::Count))
        .collect();
    assert!(with(many, vec![]).is_err(), "more than {MAX_KPIS} KPIs");
    assert!(
        with(vec![count()], vec![aim("app#1", "other")]).is_err(),
        "a KPI that is not stated"
    );
    assert!(
        with(
            vec![count()],
            vec![aim("app#1", "count"), aim("app#1", "count")]
        )
        .is_err(),
        "repeated"
    );
    assert_eq!(
        with(
            vec![count()],
            vec![aim("app#1", "count"), aim("app#2", "count")]
        ),
        Ok(()),
        "two epics may aim at one KPI"
    );
    for epic in ["", &"e".repeat(EPIC + 1), "app\n#1"] {
        assert!(
            with(vec![count()], vec![aim(epic, "count")]).is_err(),
            "{epic:?}"
        );
    }
    let aims: Vec<Aim> = (0..=MAX_AIMS)
        .map(|n| aim(&format!("app#{n}"), "count"))
        .collect();
    assert!(
        with(vec![count()], aims).is_err(),
        "more than {MAX_AIMS} aims"
    );
}

#[test]
fn a_plan_allocation_adds_up_to_100() {
    let key = AccountKey::from_seed([7u8; 32]);
    let mut body = genesis(&key).body;
    let mut allocation = Allocation {
        new_feature: 40,
        improvement: 20,
        maintenance: 20,
        debt: 15,
        incident: 5,
    };
    body.teams[0].plan = Some(TeamPlan {
        aims: vec![],
        allocation: allocation.clone(),
        weekly_budget_usd: 0,
    });
    assert_eq!(body.validate(), Ok(()));
    allocation.incident = 4;
    body.teams[0].plan = Some(TeamPlan {
        aims: vec![],
        allocation: allocation.clone(),
        weekly_budget_usd: 0,
    });
    assert!(body.validate().is_err(), "99");
    allocation.incident = u32::MAX;
    body.teams[0].plan = Some(TeamPlan {
        aims: vec![],
        allocation,
        weekly_budget_usd: 0,
    });
    assert!(body.validate().is_err(), "no overflow to 100");
}

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
        "orgWithOutcomes": org_with_outcomes(&key, &org),
        "deviceChangeP256": device_change_p256(),
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
