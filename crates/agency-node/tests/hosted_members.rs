//! A member hosted by the root's node has no account key, so it never signs a read: a read signed as it is
//! refused like a stranger's, whatever key signed it.
mod common;

use agency_core::orgs;
use agency_core::peer_about::PeerAbout;
use agency_core::read_auth::sign_read;
use agency_core::{routes, ErrorKind};
use agency_proto::about::About;
use agency_proto::clock::Duration;
use agency_proto::org::TeamPlace;
use agency_proto::org::{Device, Person};
use agency_proto::{AccountKey, Timestamp};
use common::{alice, id, joined_bob};
use std::collections::BTreeMap;

#[tokio::test]
async fn a_read_signed_as_a_hosted_member_is_refused_like_a_strangers() {
    let (alice, _) = alice().await;
    let browser = AccountKey::from_seed([0x71; 32]);
    let device = Device {
        id: browser.public().device_id(),
        label: "Laptop".into(),
        public_key: browser.public(),
        added_at: Timestamp::parse(&alice.node.now_text()).unwrap(),
    };
    orgs::amend(&alice.node, &id("acme"), |body| {
        body.people
            .push(Person::hosted(id("carol"), "Carol", vec![device]));
        Ok(())
    })
    .unwrap();
    let doc = alice.node.store().load_org(&id("acme")).unwrap();
    assert!(doc.body.member(&id("carol")).is_some_and(Person::is_hosted));

    let path = routes::definition(&id("acme"));
    for key in [browser, AccountKey::from_seed([0x72; 32])] {
        let auth = sign_read(&key, &id("carol"), "GET", &path, alice.node.now());
        let refused =
            orgs::read_definition(&alice.node, &id("acme"), None, &auth, &path).unwrap_err();
        assert_eq!(refused.kind, ErrorKind::NotFound, "{refused:?}");
    }
}

fn laptop(seed: u8, at: &str) -> Device {
    let key = AccountKey::from_seed([seed; 32]).public();
    Device {
        id: key.device_id(),
        label: "Laptop".into(),
        public_key: key,
        added_at: Timestamp::parse(at).unwrap(),
    }
}

#[tokio::test]
async fn a_node_that_does_not_list_hosted_members_holds_the_version_back() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    orgs::add_team(
        &alice.node,
        &id("acme"),
        TeamPlace::new(id("web"), "Web", bob.url()),
    )
    .unwrap();
    let read_at = agency_proto::clock::format(alice.node.now() - Duration::minutes(5));
    let mut about = About::of_release("0.3.0");
    about.capabilities.retain(|name| name != "hostedMembers");
    let entry = PeerAbout {
        about: Some(about),
        read_at: Some(read_at.clone()),
        tried_at: read_at,
        ..PeerAbout::default()
    };
    alice
        .node
        .store()
        .save_peer_abouts(&id("acme"), &BTreeMap::from([(bob.url(), entry)]))
        .unwrap();
    let device = laptop(0x73, &alice.node.now_text());
    let refused = orgs::amend(&alice.node, &id("acme"), |body| {
        body.people
            .push(Person::hosted(id("carol"), "Carol", vec![device]));
        Ok(())
    })
    .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert!(
        refused.message.contains("hostedMembers"),
        "{}",
        refused.message
    );
    assert!(refused.message.contains(&bob.url()), "{}", refused.message);
}
