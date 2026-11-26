//! A member hosted by the root's node has no account key, so it never signs a read: a read signed as it is
//! refused like a stranger's, whatever key signed it.
mod common;

use agency_core::orgs;
use agency_core::read_auth::sign_read;
use agency_core::{routes, ErrorKind};
use agency_proto::org::{Device, Person};
use agency_proto::{AccountKey, Timestamp};
use common::{alice, id};

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
