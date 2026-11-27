//! Tests of the protocol's deciding rules: the chain of org definition versions (one writer, the root key), envelope shape and expiry, and the test vectors.
//! The non-ASCII sample values ("開発", "ア") are on purpose: the test vectors pin UTF-8 in canonical JSON.
use agency_proto::canonical::canonical_of;
use agency_proto::org::{
    verify_genesis, verify_root_successor, verify_successor, Allocation, Department, PendingRoot,
    Root, SignedOrg, Succession, TeamPlace, TeamPlan,
};
use agency_proto::outcome::{
    Aim, Currency, GoodWhen, Kpi, KpiKind, KpiUnit, EPIC, MAX_AIMS, MAX_KPIS,
};
use agency_proto::seat::{External, Seat};
use agency_proto::signed::{is_node_id, read_text, read_text_for};
use agency_proto::{
    clock, AccountKey, Device, DeviceChange, DeviceOp, Envelope, EnvelopeId, EnvelopeKind, Id,
    Kind, OrgDefinition, Payload, Person, Role, Signed, Timestamp,
};

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

const AT: &str = "2026-11-24T12:00:00Z";

fn genesis(root: &AccountKey) -> SignedOrg {
    let body = OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        Root::new(id("alice"), root.public()),
        AT.into(),
    )
    .with_departments(vec![Department {
        id: id("dev"),
        label: "開発".into(),
        parent: None,
        head: Some(id("owner")),
    }])
    .with_people(vec![Person::new(
        id("owner"),
        "Alice",
        id("alice"),
        root.public(),
    )
    .with_department(Some(id("dev")))])
    .with_teams(vec![TeamPlace::new(
        id("app"),
        "app",
        "https://alice.example",
    )
    .with_department(Some(id("dev")))]);
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
        body.people.push(
            Person::new(id("bob"), "Bob", id("bob"), bob.public())
                .with_department(Some(id("dev")))
                .with_role(Role::Answerer),
        )
    });
    assert_eq!(verify_successor(&v1, &v2), Ok(Succession::Root));
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
/// An org moved to a new root key (seed `0d…`): version 2 names it and is signed by the old root, and
/// version 3 is signed by the new one.
fn root_rotation(old: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let new = AccountKey::from_seed([0x0d; 32]);
    let mut naming = OrgDefinition::successor_of(org, AT.into());
    naming.root.public_key = new.public();
    let naming = Signed::sign(Kind::Org, naming, id("alice"), old).unwrap();
    assert!(verify_successor(org, &naming).is_ok());
    let mut after = OrgDefinition::successor_of(&naming, AT.into());
    after.label = "Acme Inc".into();
    let after = Signed::sign(Kind::Org, after, id("alice"), &new).unwrap();
    assert!(verify_successor(&naming, &after).is_ok());
    let by_old = Signed::sign(Kind::Org, after.body.clone(), id("alice"), old).unwrap();
    assert!(
        verify_successor(&naming, &by_old).is_err(),
        "the old root signs no later version"
    );
    let signed = |doc: &SignedOrg| serde_json::json!({ "canonical": canonical_of(&doc.body).unwrap(), "signature": doc.signature, "digest": doc.digest() });
    serde_json::json!({
        "newSeed": "0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d",
        "newPublicKey": new.public().encoded(),
        "namingTheNewRoot": signed(&naming),
        "signedByTheNewRoot": signed(&after),
    })
}

/// An org given a recovery key (seed `0e…`), then recovered to a new root key (seed `0f…`): the root adds
/// the recovery key, the recovery key starts the recovery and, after the wait, finishes it; the root may
/// instead cancel the start by signing any version. Then the recovery key replaces itself (seed `10…`)
/// the same way. The refused versions show what neither key may sign.
fn root_recovery(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let (recovery, new_root, new_recovery) = (
        AccountKey::from_seed([0x0e; 32]),
        AccountKey::from_seed([0x0f; 32]),
        AccountKey::from_seed([0x10; 32]),
    );
    let sign = |body: OrgDefinition, key: &AccountKey| {
        Signed::sign(Kind::Org, body, id("alice"), key).unwrap()
    };
    let next = |doc: &SignedOrg| OrgDefinition::successor_of(doc, AT.into());
    let is = |current: &SignedOrg, doc: &SignedOrg, succession: Succession| {
        assert_eq!(verify_successor(current, doc), Ok(succession));
    };

    let mut body = next(org);
    body.root.recovery = Some(recovery.public());
    let with_recovery = sign(body, root);
    is(org, &with_recovery, Succession::Root);

    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: Some(new_root.public()),
        recovery: None,
    });
    let start = sign(body, &recovery);
    is(&with_recovery, &start, Succession::RecoveryStart);

    let mut body = next(&start);
    body.root.pending = None;
    let cancel = sign(body, root);
    is(&start, &cancel, Succession::Root);

    let mut body = next(&start);
    body.root.public_key = new_root.public();
    body.root.pending = None;
    let finish = sign(body, &recovery);
    is(&start, &finish, Succession::RecoveryFinish);
    assert!(
        verify_root_successor(&start, &finish).is_err(),
        "an answer from the root's node is never a recovery"
    );
    assert!(verify_root_successor(&start, &cancel).is_ok());
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: None,
        recovery: None,
    });
    let empty_start = sign(body, &recovery);
    for kept_on_a_node in [
        root.public(),
        org.body.people[0].public_key.clone().unwrap(),
    ] {
        let mut body = next(org);
        body.root.recovery = Some(kept_on_a_node);
        assert!(
            body.validate().unwrap_err().contains("recovery key"),
            "a recovery key a node keeps"
        );
    }
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: None,
        recovery: Some(root.public()),
    });
    assert!(
        body.validate().is_err(),
        "a new recovery key that is the root key"
    );
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: org.body.people[0].public_key.clone(),
        recovery: None,
    });
    assert!(
        body.validate().is_err(),
        "a new root key that is an account key"
    );
    assert!(verify_successor(&with_recovery, &empty_start)
        .unwrap_err()
        .contains("names no change"));

    let mut body = next(&finish);
    body.root.pending = Some(PendingRoot {
        public_key: None,
        recovery: Some(new_recovery.public()),
    });
    let replace_start = sign(body, &recovery);
    is(&finish, &replace_start, Succession::RecoveryStart);
    let mut body = next(&replace_start);
    body.root.recovery = Some(new_recovery.public());
    body.root.pending = None;
    let replace_finish = sign(body, &recovery);
    is(&replace_start, &replace_finish, Succession::RecoveryFinish);

    let refused = |current: &SignedOrg, doc: SignedOrg, why: &str| {
        let error = verify_successor(current, &doc).unwrap_err();
        assert!(error.contains(why), "{error}");
        serde_json::json!({ "after": current.body.version, "canonical": canonical_of(&doc.body).unwrap(), "signature": doc.signature, "why": error })
    };
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: Some(new_root.public()),
        recovery: None,
    });
    body.label = "Taken".into();
    let start_changing_more = refused(
        &with_recovery,
        sign(body, &recovery),
        "changes nothing but the root",
    );
    let mut body = next(&with_recovery);
    body.root.public_key = new_root.public();
    let rotation_without_a_start =
        refused(&with_recovery, sign(body, &recovery), "names no change");
    let mut body = next(&with_recovery);
    body.root.pending = Some(PendingRoot {
        public_key: Some(new_root.public()),
        recovery: None,
    });
    let start_by_root = refused(
        &with_recovery,
        sign(body, root),
        "does not start a recovery",
    );
    let mut body = next(&with_recovery);
    body.root.recovery = Some(new_recovery.public());
    let recovery_by_root = refused(&with_recovery, sign(body, root), "only the recovery key");

    let signed = |doc: &SignedOrg| serde_json::json!({ "canonical": canonical_of(&doc.body).unwrap(), "signature": doc.signature, "digest": doc.digest() });
    serde_json::json!({
        "recoverySeed": "0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e",
        "newRootSeed": "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f",
        "newRecoverySeed": "1010101010101010101010101010101010101010101010101010101010101010",
        "addingTheRecoveryKey": signed(&with_recovery),
        "start": signed(&start),
        "cancel": signed(&cancel),
        "finish": signed(&finish),
        "replacingTheWordsStart": signed(&replace_start),
        "replacingTheWordsFinish": signed(&replace_finish),
        "refused": {
            "startChangingMore": start_changing_more,
            "rotationWithoutAStart": rotation_without_a_start,
            "startSignedByTheRoot": start_by_root,
            "recoveryChangedByTheRoot": recovery_by_root,
        },
    })
}

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

/// The next version of `org`, with a seat and an external party: how they are signed. The seat has no account,
/// key or device; both are answered for by the member `owner`.
fn org_with_seats(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    body.seats.push(Seat {
        id: id("carol"),
        label: "Carol (finance)".into(),
        department: Some(id("dev")),
        ask_about: vec!["invoices".into(), "budget approval".into()],
        relays: vec![id("owner")],
    });
    body.externals.push(External {
        id: id("erin"),
        label: "Erin (outside counsel)".into(),
        ask_about: vec!["contract review".into(), "legal checks".into()],
        relays: vec![id("owner")],
    });
    let signed = Signed::sign(Kind::Org, body, id("alice"), root).unwrap();
    assert!(verify_successor(org, &signed).is_ok());
    serde_json::json!({ "canonical": canonical_of(&signed.body).unwrap(), "signature": signed.signature, "digest": signed.digest() })
}

/// The next version of `org`, with a member hosted by the root's node: no account key (`publicKey` left out),
/// `hostedBy` "root", its member id as its account, and one device, a P-256 key from a browser.
fn org_with_hosted_member(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    let browser = AccountKey::from_seed([0x61; 32]).public();
    let device = Device {
        id: browser.device_id(),
        label: "Carol's laptop".into(),
        public_key: browser,
        added_at: Timestamp::parse(AT).unwrap(),
    };
    body.people
        .push(Person::hosted(id("carol"), "Carol", vec![device]));
    let signed = Signed::sign(Kind::Org, body, id("alice"), root).unwrap();
    assert!(verify_successor(org, &signed).is_ok());
    assert!(!canonical_of(&signed.body)
        .unwrap()
        .contains("\"publicKey\":null"));
    serde_json::json!({ "canonical": canonical_of(&signed.body).unwrap(), "signature": signed.signature, "digest": signed.digest() })
}

/// The next version of `org`, in which dan has taken the seat carol: the seat is gone from `seats`, and dan has
/// its id as `formerly` and what to ask about it as `askAbout`.
fn org_with_a_claimed_seat(root: &AccountKey, org: &SignedOrg) -> serde_json::Value {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    body.people.push(
        Person::new(
            id("dan"),
            "Dan",
            id("dan"),
            AccountKey::from_seed([4; 32]).public(),
        )
        .with_department(Some(id("dev")))
        .with_formerly(vec![id("carol")])
        .with_ask_about(vec!["invoices".into(), "budget approval".into()]),
    );
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

/// A change to a version's JSON, to make it as a sender with another idea of the shape would.
type Change = fn(&mut serde_json::Value);

/// What a receiving node does with a version the root signed: it reads the JSON, and checks the signature
/// over the canonical JSON of what it read (`Signed::verify`), not over the bytes it was sent.
fn received(
    org: &SignedOrg,
    root: &AccountKey,
    change: impl FnOnce(&mut serde_json::Value),
) -> Result<(), String> {
    let mut body = OrgDefinition::successor_of(org, AT.into());
    body.kpis = vec![kpi("revenue", "Revenue", KpiUnit::Percent)];
    let mut value = serde_json::to_value(&body).unwrap();
    change(&mut value);
    // The root signs the bytes it holds, the unknown field included.
    let text = agency_proto::canonical::canonical(&value);
    let signature = root.sign(&Kind::Org.message(&text));
    let sent = serde_json::json!({ "body": value, "signer": "alice", "signature": signature });
    let next: SignedOrg = serde_json::from_value(sent).map_err(|error| error.to_string())?;
    verify_successor(org, &next).map(|_| ())
}

#[test]
fn a_kpi_with_a_field_this_node_does_not_know_is_refused_by_the_signature() {
    let key = AccountKey::from_seed([7u8; 32]);
    let org = genesis(&key);
    assert_eq!(
        received(&org, &key, |_| {}),
        Ok(()),
        "the same version without the extra field"
    );

    // `Kpi` cannot refuse unknown fields while parsing (its unit is flattened), so the parse drops them. The
    // signature is checked over what was parsed, which no longer holds the field the root signed.
    let changes: [(&str, Change); 2] = [
        ("an extra field on a KPI", |v| {
            v["kpis"][0]["target"] = serde_json::json!(5)
        }),
        ("a currency on a unit that has none", |v| {
            v["kpis"][0]["currency"] = serde_json::json!("JPY")
        }),
    ];
    for (what, change) in changes {
        let refused = received(&org, &key, change);
        assert_eq!(
            refused,
            Err("not signed by the current root key".to_owned()),
            "{what} is refused by the signature, not accepted"
        );
    }

    // The other definition structs refuse unknown fields while parsing.
    let aimed = |v: &mut serde_json::Value| {
        v["teams"][0]["plan"] = serde_json::json!({
            "allocation": { "newFeature": 100, "improvement": 0, "maintenance": 0, "debt": 0, "incident": 0 },
            "weeklyBudgetUsd": 0,
            "aims": [{ "epic": "app#1", "kpi": "revenue", "target": 5 }]
        });
    };
    let refused = received(&org, &key, aimed);
    assert!(
        refused.is_err(),
        "an extra field on an aim is refused: {refused:?}"
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

const READ_PATH: &str = "/fed/v1/teams/acme/app/view";
const READ_DATE: &str = "2026-11-24T12:00:00.123Z";
const READ_AUDIENCE: &str = "02ae509c6b20999646d3c600687ee1bf";
const OTHER_AUDIENCE: &str = "0881dc11d90faa5bd241e84dd481c251";

/// A signed read as the account key (seed 7) makes it for a node, in the two texts a node reads: the one that
/// names no node (`agency/v1/read`) and the one bound to the node it is made for (`agency/v2/read`).
fn signed_read(key: &AccountKey) -> serde_json::Value {
    let alice = id("alice");
    let unbound = read_text("GET", READ_PATH, READ_DATE, &alice);
    let bound = read_text_for("GET", READ_PATH, READ_DATE, &alice, READ_AUDIENCE);
    serde_json::json!({
        "audience": READ_AUDIENCE,
        "unbound": { "text": unbound, "signature": key.sign(&Kind::Read.message(&unbound)) },
        "bound": { "text": bound, "signature": key.sign(&Kind::ReadFor.message(&bound)) },
    })
}

#[test]
fn a_read_bound_to_a_node_is_no_read_for_another_node_and_no_unbound_read() {
    let key = AccountKey::from_seed([7u8; 32]);
    let alice = id("alice");
    let public = key.public();
    let bound = |audience: &str| {
        Kind::ReadFor.message(&read_text_for(
            "GET", READ_PATH, READ_DATE, &alice, audience,
        ))
    };
    let unbound = Kind::Read.message(&read_text("GET", READ_PATH, READ_DATE, &alice));
    let signature = key.sign(&bound(READ_AUDIENCE));
    assert!(public.verify(&bound(READ_AUDIENCE), &signature));
    assert!(
        !public.verify(&bound(OTHER_AUDIENCE), &signature),
        "another node's id"
    );
    assert!(
        !public.verify(&unbound, &signature),
        "the signature is not one over the unbound text"
    );
    let unbound_signature = key.sign(&unbound);
    assert!(public.verify(&unbound, &unbound_signature));
    assert!(
        !public.verify(&bound(READ_AUDIENCE), &unbound_signature),
        "an unbound signature is no bound one"
    );
    let v1_kind_over_bound_lines = Kind::Read.message(&read_text_for(
        "GET",
        READ_PATH,
        READ_DATE,
        &alice,
        READ_AUDIENCE,
    ));
    assert!(
        !public.verify(&v1_kind_over_bound_lines, &signature),
        "the kind line differs, so the bound lines under the v1 kind are not the bound text"
    );
}

#[test]
fn a_node_id_is_thirty_two_lowercase_hex_characters() {
    assert!(is_node_id(READ_AUDIENCE));
    for refused in [
        "",
        "02AE509C6B20999646D3C600687EE1BF",
        "02ae509c6b20999646d3c600687ee1b",
        "02ae509c6b20999646d3c600687ee1bff",
        "02ae509c6b20999646d3c600687ee1bg",
        "02ae509c6b20999646d3c600687ee1b\n",
    ] {
        assert!(!is_node_id(refused), "{refused:?}");
    }
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
        "orgWithSeats": org_with_seats(&key, &org),
        "orgWithClaimedSeat": org_with_a_claimed_seat(&key, &org),
        "orgWithHostedMember": org_with_hosted_member(&key, &org),
        "rootRotation": root_rotation(&key, &org),
        "rootRecovery": root_recovery(&key, &org),
        "orgWithOutcomes": org_with_outcomes(&key, &org),
        "deviceChangeP256": device_change_p256(),
        "signedRead": signed_read(&key),
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
