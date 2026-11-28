//! What the protocol's tests share: the sample org and envelope, and the builders of the values `testdata/vectors.json` pins.
#![allow(dead_code)]
use agency_proto::canonical::canonical_of;
use agency_proto::org::{
    verify_root_successor, verify_successor, Allocation, Department, PendingRoot, Root, SignedOrg,
    Succession, TeamPlace, TeamPlan,
};
use agency_proto::outcome::{Aim, Currency, GoodWhen, Kpi, KpiKind, KpiUnit};
use agency_proto::seat::{External, Seat};
use agency_proto::signed::{read_text, read_text_for};
use agency_proto::view::FederatedView;
use agency_proto::{
    AccountKey, Device, DeviceChange, DeviceOp, Envelope, EnvelopeId, EnvelopeKind, Id, Kind,
    OrgDefinition, Payload, Person, Signed, Timestamp,
};

pub mod device_vectors;
pub mod org_vectors;
pub mod read_vectors;

pub fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

pub const AT: &str = "2026-11-24T12:00:00Z";

pub fn genesis(root: &AccountKey) -> SignedOrg {
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

pub fn successor(
    current: &SignedOrg,
    signer: &AccountKey,
    edit: impl FnOnce(&mut OrgDefinition),
) -> SignedOrg {
    let mut body = OrgDefinition::successor_of(current, AT.into());
    edit(&mut body);
    Signed::sign(Kind::Org, body, id("alice"), signer).unwrap()
}

pub fn envelope(at: &str) -> Envelope {
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

/// A decision that sets a ticket's priority: the ticket's target, the priority as the choice, and no text.
pub fn prioritize_envelope() -> Envelope {
    let mut body = envelope(AT);
    body.id = EnvelopeId::try_from("0f1e2d3c4b5a69788796a5b4c3d2e1f0".to_owned()).unwrap();
    body.kind = EnvelopeKind::Prioritize;
    body.target = "app#12".into();
    body.payload = Payload {
        text: String::new(),
        choice: Some("high".into()),
    };
    body
}

/// A comment: text and no choice.
pub fn comment_envelope() -> Envelope {
    let mut body = envelope(AT);
    body.id = EnvelopeId::try_from("ffeeddccbbaa99887766554433221100".to_owned()).unwrap();
    body.payload = Payload {
        text: "来週にしてください 🙏\n\"理由\"は別途".into(),
        choice: None,
    };
    body
}
