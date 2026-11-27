//! A member hosted by the root's node (`hostedBy` "root") has no account key, its member id as its account,
//! at least one device, and is never the root; every other member has an account key.
use agency_proto::org::{Device, HostedBy, Person, Root};
use agency_proto::{AccountKey, Id, OrgDefinition, Timestamp};

const AT: &str = "2026-11-24T12:00:00Z";

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn device(seed: u8) -> Device {
    let key = AccountKey::from_seed([seed; 32]).public();
    Device {
        id: key.device_id(),
        label: "Laptop".into(),
        public_key: key,
        added_at: Timestamp::parse(AT).unwrap(),
    }
}

fn org(people: Vec<Person>) -> OrgDefinition {
    let alice = Person::new(
        id("alice"),
        "Alice",
        id("alice"),
        AccountKey::from_seed([1; 32]).public(),
    );
    let root = Root::new(id("alice"), AccountKey::from_seed([2; 32]).public());
    let mut all = vec![alice];
    all.extend(people);
    OrgDefinition::new(id("acme"), "Acme".into(), root, AT.into()).with_people(all)
}

#[test]
fn a_hosted_member_with_a_device_and_no_key_is_valid_and_a_member() {
    let carol = Person::hosted(id("carol"), "Carol", vec![device(3)]);
    assert_eq!(carol.hosted_by, Some(HostedBy::Root));
    let definition = org(vec![carol]);
    assert_eq!(definition.validate(), Ok(()));
    assert!(definition
        .member(&id("carol"))
        .is_some_and(Person::is_hosted));
}

#[test]
fn each_broken_rule_of_a_hosted_member_is_refused() {
    let with_key = {
        let mut one = Person::hosted(id("carol"), "Carol", vec![device(3)]);
        one.public_key = Some(AccountKey::from_seed([4; 32]).public());
        one
    };
    let without_device = Person::hosted(id("carol"), "Carol", vec![]);
    let other_account = {
        let mut one = Person::hosted(id("carol"), "Carol", vec![device(3)]);
        one.account = id("caroline");
        one
    };
    let keyless_member = {
        let mut one = Person::new(
            id("carol"),
            "Carol",
            id("carol"),
            AccountKey::from_seed([4; 32]).public(),
        );
        one.public_key = None;
        one
    };
    for (what, person) in [
        ("a hosted member with an account key", with_key),
        ("a hosted member with no device", without_device),
        ("a hosted member whose account is not its id", other_account),
        ("a member with no key that is not hosted", keyless_member),
    ] {
        assert!(org(vec![person]).validate().is_err(), "{what}");
    }
}

#[test]
fn the_root_is_never_hosted() {
    let mut definition = org(vec![]);
    definition.people = vec![Person::hosted(id("alice"), "Alice", vec![device(3)])];
    assert!(definition.validate().is_err());
}
