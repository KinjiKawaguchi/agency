//! Tests of the device key rules: who may add or remove a device, which keys may never be device keys,
//! and that only a member's own, unrevoked device signs a decision.
use agency_proto::device::{check_device_change, envelope_signer};
use agency_proto::org::Root;
use agency_proto::{
    clock, AccountKey, Device, DeviceChange, DeviceOp, Envelope, EnvelopeId, EnvelopeKind, Id,
    Kind, OrgDefinition, Payload, Person, Role, Signed, Timestamp,
};

const AT: &str = "2026-11-24T12:00:00Z";

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn now() -> clock::OffsetDateTime {
    clock::parse(AT).unwrap()
}

fn person(account: &str, key: &AccountKey) -> Person {
    Person {
        id: id(account),
        label: account.to_uppercase(),
        account: id(account),
        public_key: key.public(),
        department: None,
        role: Role::Member,
        devices: vec![],
    }
}

/// Keys of one test org: alice is the root, bob a member; each has an account key on their node.
struct Keys {
    alice: AccountKey,
    bob: AccountKey,
}

fn org(keys: &Keys) -> OrgDefinition {
    OrgDefinition::new(
        id("acme"),
        "Acme".into(),
        Root::new(id("alice"), keys.alice.public()),
        AT.into(),
    )
    .with_people(vec![person("alice", &keys.alice), person("bob", &keys.bob)])
}

fn device(name: &str, key: &AccountKey) -> Device {
    Device {
        id: id(name),
        label: name.into(),
        public_key: key.public(),
        added_at: Timestamp::parse(AT).unwrap(),
    }
}

fn change(account: &str, op: DeviceOp) -> DeviceChange {
    DeviceChange {
        org: id("acme"),
        account: id(account),
        op,
        at: Timestamp::parse(AT).unwrap(),
    }
}

fn signed(change: DeviceChange, signer: &str, key: &AccountKey) -> Signed<DeviceChange> {
    Signed::sign(Kind::Device, change, id(signer), key).unwrap()
}

fn add(account: &str, new: Device) -> DeviceChange {
    change(
        account,
        DeviceOp::Add {
            device: Box::new(new),
        },
    )
}

fn keys() -> Keys {
    Keys {
        alice: AccountKey::generate(),
        bob: AccountKey::generate(),
    }
}

/// bob's org with his first device, `laptop`, added with his account key.
fn with_laptop(keys: &Keys, laptop: &AccountKey) -> OrgDefinition {
    let first = signed(add("bob", device("laptop", laptop)), "bob", &keys.bob);
    check_device_change(&org(keys), &first, now()).unwrap()
}

#[test]
fn the_account_key_adds_only_the_first_device() {
    let keys = keys();
    let (laptop, phone) = (AccountKey::generate(), AccountKey::generate());
    let current = with_laptop(&keys, &laptop);
    assert_eq!(
        current
            .device(&id("bob"), &id("laptop"))
            .unwrap()
            .1
            .public_key,
        laptop.public()
    );

    let by_account = signed(add("bob", device("phone", &phone)), "bob", &keys.bob);
    assert!(
        check_device_change(&current, &by_account, now()).is_err(),
        "once bob has a device, his account key cannot add another"
    );
    let by_laptop = signed(add("bob", device("phone", &phone)), "laptop", &laptop);
    let next = check_device_change(&current, &by_laptop, now()).unwrap();
    assert!(next.device(&id("bob"), &id("phone")).is_some());

    let remove_by_account = signed(
        change(
            "bob",
            DeviceOp::Remove {
                device: id("laptop"),
            },
        ),
        "bob",
        &keys.bob,
    );
    assert!(check_device_change(&current, &remove_by_account, now()).is_err());
}

#[test]
fn a_change_is_signed_by_the_device_it_names() {
    let keys = keys();
    let (laptop, phone) = (AccountKey::generate(), AccountKey::generate());
    let current = with_laptop(&keys, &laptop);
    let wrong_key = signed(add("bob", device("phone", &phone)), "laptop", &phone);
    assert!(check_device_change(&current, &wrong_key, now()).is_err());
    let unknown = signed(add("bob", device("phone", &phone)), "tablet", &phone);
    assert!(check_device_change(&current, &unknown, now()).is_err());
    let others = signed(add("alice", device("phone", &phone)), "laptop", &laptop);
    assert!(
        check_device_change(&current, &others, now()).is_err(),
        "bob's device cannot add a device for alice"
    );
    let as_envelope = Signed::sign(
        Kind::Envelope,
        add("bob", device("phone", &phone)),
        id("laptop"),
        &laptop,
    )
    .unwrap();
    assert!(
        check_device_change(&current, &as_envelope, now()).is_err(),
        "a signature of another kind does not count"
    );
}

#[test]
fn refuses_changes_for_other_orgs_strangers_and_out_of_time() {
    let keys = keys();
    let laptop = AccountKey::generate();
    let mut other_org = add("bob", device("laptop", &laptop));
    other_org.org = id("other");
    let mut stale = add("bob", device("laptop", &laptop));
    stale.at = Timestamp::parse("2026-11-16T12:00:00Z").unwrap();
    let mut ahead = add("bob", device("laptop", &laptop));
    ahead.at = Timestamp::parse("2026-11-24T12:10:00Z").unwrap();
    let carol = AccountKey::generate();
    let cases = [
        ("another org", signed(other_org, "bob", &keys.bob)),
        ("8 days old", signed(stale, "bob", &keys.bob)),
        ("10 minutes ahead", signed(ahead, "bob", &keys.bob)),
        (
            "not a member",
            signed(add("carol", device("laptop", &laptop)), "carol", &carol),
        ),
    ];
    for (name, change) in cases {
        assert!(
            check_device_change(&org(&keys), &change, now()).is_err(),
            "{name}"
        );
    }
}

#[test]
fn node_keys_and_repeated_keys_are_never_device_keys() {
    let keys = keys();
    let laptop = AccountKey::generate();
    let current = with_laptop(&keys, &laptop);
    let cases = [
        ("bob's account key", device("phone", &keys.bob)),
        ("the root key", device("phone", &keys.alice)),
        ("a key already on a device", device("phone", &laptop)),
        (
            "a repeated device id",
            device("laptop", &AccountKey::generate()),
        ),
        (
            "an empty label",
            Device {
                label: String::new(),
                ..device("phone", &AccountKey::generate())
            },
        ),
    ];
    for (name, new) in cases {
        let change = signed(add("bob", new), "laptop", &laptop);
        assert!(
            check_device_change(&current, &change, now()).is_err(),
            "{name}"
        );
    }
    let alice_first = signed(add("alice", device("phone", &laptop)), "alice", &keys.alice);
    assert!(
        check_device_change(&current, &alice_first, now()).is_err(),
        "a key on bob's device cannot also be alice's"
    );
}

#[test]
fn a_removed_device_is_revoked_for_good() {
    let keys = keys();
    let (laptop, phone) = (AccountKey::generate(), AccountKey::generate());
    let current = with_laptop(&keys, &laptop);
    let add_phone = signed(add("bob", device("phone", &phone)), "laptop", &laptop);
    let two = check_device_change(&current, &add_phone, now()).unwrap();
    let lost = signed(
        change(
            "bob",
            DeviceOp::Remove {
                device: id("phone"),
            },
        ),
        "laptop",
        &laptop,
    );
    let one = check_device_change(&two, &lost, now()).unwrap();
    assert!(one.device(&id("bob"), &id("phone")).is_none());
    assert!(one.revoked.contains(&phone.public()));

    let back = signed(add("bob", device("phone2", &phone)), "laptop", &laptop);
    assert!(
        check_device_change(&one, &back, now()).is_err(),
        "a revoked key cannot come back under another id"
    );
    let by_lost = signed(
        add("bob", device("tablet", &AccountKey::generate())),
        "phone",
        &phone,
    );
    assert!(check_device_change(&one, &by_lost, now()).is_err());
    assert!(envelope_signer(&one, &decision("phone", &phone)).is_err());
}

fn decision(signer: &str, key: &AccountKey) -> Signed<Envelope> {
    let body = Envelope {
        id: EnvelopeId::from_bytes([1; 16]),
        kind: EnvelopeKind::Answer,
        org: id("acme"),
        team: id("app"),
        target: "r-1".into(),
        payload: Payload {
            text: String::new(),
            choice: Some("a".into()),
        },
        account: id("bob"),
        at: AT.into(),
    };
    Signed::sign(Kind::Envelope, body, id(signer), key).unwrap()
}

#[test]
fn only_a_members_own_device_signs_a_decision() {
    let keys = keys();
    let laptop = AccountKey::generate();
    let current = with_laptop(&keys, &laptop);
    assert_eq!(
        envelope_signer(&current, &decision("laptop", &laptop)).map(|person| &person.id),
        Ok(&id("bob"))
    );
    assert!(
        envelope_signer(&current, &decision("bob", &keys.bob)).is_err(),
        "the account key, which bob's node holds, does not sign decisions"
    );
    assert!(envelope_signer(&current, &decision("laptop", &AccountKey::generate())).is_err());

    let alice_laptop = AccountKey::generate();
    let first = signed(
        add("alice", device("desk", &alice_laptop)),
        "alice",
        &keys.alice,
    );
    let both = check_device_change(&current, &first, now()).unwrap();
    assert!(
        envelope_signer(&both, &decision("desk", &alice_laptop)).is_err(),
        "alice's device cannot decide as bob"
    );
}

#[test]
fn pairing_codes_are_twelve_hex_characters_in_groups() {
    let key = AccountKey::from_seed([9; 32]).public();
    let code = key.pairing_code();
    assert_eq!(code.len(), 14);
    assert_eq!(code.replace('-', ""), key.fingerprint()[..12]);
    assert_eq!(code.matches('-').count(), 2);
}
