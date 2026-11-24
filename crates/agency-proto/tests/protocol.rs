//! 約束の決める行の試験: 組織の定義の版のつながり (書き手は根の鍵 1 つ)、封筒の形と期限、試験の値。
use agency_proto::canonical::canonical_of;
use agency_proto::org::{verify_genesis, verify_successor, Department, Root, SignedOrg, TeamPlace};
use agency_proto::{
    clock, AccountKey, Envelope, EnvelopeId, EnvelopeKind, Id, Kind, OrgDefinition, Payload,
    Person, Role, Signed,
};

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

const AT: &str = "2026-11-24T12:00:00Z";

fn genesis(root: &AccountKey) -> SignedOrg {
    let body = OrgDefinition {
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
        }],
        teams: vec![TeamPlace {
            id: id("app"),
            label: "app".into(),
            department: Some(id("dev")),
            node: "https://alice.example".into(),
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
        })
    });
    assert_eq!(verify_successor(&v1, &v2), Ok(()));
    assert!(v2.body.member(&id("bob")).is_some());
    // 根でない鍵の署名、版を飛ばしたもの、前の版の指紋が違うもの、2 度目の同じ版は受け取らない
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
        "8 日前は期限切れ"
    );
    assert!(
        !envelope("2026-11-24T12:10:00Z").is_fresh(now),
        "10 分先は時計のずれの幅の外"
    );
    let mut empty = envelope(AT);
    empty.payload.choice = None;
    assert!(empty.validate().is_err());
    let mut bad_target = envelope(AT);
    bad_target.target = "../../etc".into();
    assert!(bad_target.validate().is_err());
}

#[test]
fn signature_is_bound_to_its_kind() {
    let key = AccountKey::generate();
    let signed = Signed::sign(Kind::Envelope, envelope(AT), id("alice"), &key).unwrap();
    assert!(signed.verify(Kind::Envelope, &key.public()));
    assert!(
        !signed.verify(Kind::Org, &key.public()),
        "封筒の署名を組織の定義の署名として使えない"
    );
}

/// 試験の値: 決まった種の鍵で、決まった本体の決まった順の JSON と署名。別の言語で作り直しても同じ値になる
#[test]
fn vectors_match_testdata() {
    let key = AccountKey::from_seed([7u8; 32]);
    let mut body = envelope(AT);
    body.id = EnvelopeId::try_from("00112233445566778899aabbccddeeff".to_owned()).unwrap();
    let signed = Signed::sign(Kind::Envelope, body.clone(), id("alice"), &key).unwrap();
    let org = genesis(&key);
    let actual = serde_json::json!({
        "seed": "0707070707070707070707070707070707070707070707070707070707070707",
        "publicKey": key.public().encoded(),
        "fingerprint": key.public().fingerprint(),
        "envelope": { "canonical": canonical_of(&body).unwrap(), "signature": signed.signature },
        "org": { "canonical": canonical_of(&org.body).unwrap(), "signature": org.signature, "digest": org.digest() },
    });
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/vectors.json");
    if std::env::var("AGENCY_WRITE_VECTORS").is_ok() {
        std::fs::write(path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
    }
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(path)
            .expect("testdata/vectors.json が無い (AGENCY_WRITE_VECTORS=1 で作る)"),
    )
    .unwrap();
    assert_eq!(actual, expected);
}
