//! A signed read can name the node it is made for (`agency/v2/read`, the `x-agency-audience` header): the node
//! it names takes it and no other does; once a node has taken one from an account it takes no read that names
//! no node from it, also after a restart; and the read of `about` stays unbound. Two real nodes over HTTP.
mod common;

use agency_adapters::files::DataDir;
use agency_adapters::peers::read_headers;
use agency_core::federation_config::FederationConfig;
use agency_core::peer_about::PeerAbout;
use agency_core::read_auth::{sign_for_peer, sign_read, sign_read_for, status_lines, ReadAuth};
use agency_core::routes;
use agency_core::views::refresh;
use agency_proto::about::Capability;
use agency_proto::client::node_id;
use agency_proto::clock::Duration;
use common::{alice, id, joined_bob, memory_runtime, Running};
use reqwest::{Client, StatusCode};
use std::collections::BTreeMap;

fn view_path() -> String {
    routes::view(&id("acme"), &id("app"))
}

fn about_path() -> String {
    routes::about(&id("acme"))
}

/// alice's id as another node names her.
fn alices_id(alice: &Running) -> String {
    node_id(&alice.url()).unwrap()
}

async fn send(alice: &Running, auth: &ReadAuth, path: &str) -> StatusCode {
    let mut request = Client::new().get(format!("{}{path}", alice.url()));
    for (name, value) in read_headers(auth) {
        request = request.header(name, value);
    }
    request.send().await.unwrap().status()
}

/// A read by bob of `path` on alice's node that names `audience`, signed one second after the last one so that no
/// two reads share a signature.
async fn bound(alice: &Running, bob: &Running, path: &str, audience: &str) -> StatusCode {
    alice.clock.advance(Duration::seconds(1));
    let now = alice.node.now();
    let auth = sign_read_for(bob.node.key(), &id("bob"), "GET", path, now, audience);
    send(alice, &auth, path).await
}

async fn unbound(alice: &Running, bob: &Running, path: &str) -> StatusCode {
    alice.clock.advance(Duration::seconds(1));
    let auth = sign_read(bob.node.key(), &id("bob"), "GET", path, alice.node.now());
    send(alice, &auth, path).await
}

#[tokio::test]
async fn a_read_that_names_the_node_is_answered_and_one_that_names_another_is_not() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = view_path();
    let other = node_id("https://elsewhere.example.com").unwrap();
    assert_eq!(
        bound(&alice, &bob, &path, &other).await,
        StatusCode::NOT_FOUND,
        "a read made for another node"
    );
    assert_eq!(
        bound(&alice, &bob, &path, &alices_id(&alice)).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn the_name_of_the_node_is_part_of_what_is_signed() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = view_path();
    alice.clock.advance(Duration::seconds(1));
    let now = alice.node.now();
    let mine = alices_id(&alice);
    let other = node_id("https://elsewhere.example.com").unwrap();
    let made_for_other = sign_read_for(bob.node.key(), &id("bob"), "GET", &path, now, &other);

    let stripped = ReadAuth {
        audience: None,
        ..made_for_other.clone()
    };
    assert_eq!(
        send(&alice, &stripped, &path).await,
        StatusCode::NOT_FOUND,
        "the header taken off a read that was made for another node"
    );
    let renamed = ReadAuth {
        audience: Some(mine.clone()),
        ..made_for_other
    };
    assert_eq!(
        send(&alice, &renamed, &path).await,
        StatusCode::NOT_FOUND,
        "the header changed to this node's id"
    );
    let unbound_signature = sign_read(bob.node.key(), &id("bob"), "GET", &path, now);
    let claimed = ReadAuth {
        audience: Some(mine),
        ..unbound_signature
    };
    assert_eq!(
        send(&alice, &claimed, &path).await,
        StatusCode::NOT_FOUND,
        "a read signed without a node, sent with this node's id"
    );
}

#[tokio::test]
async fn a_header_that_is_no_node_id_is_refused() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    for text in ["", "alice", "ZZ"] {
        assert_eq!(
            bound(&alice, &bob, &view_path(), text).await,
            StatusCode::NOT_FOUND,
            "{text:?}"
        );
    }
}

#[tokio::test]
async fn after_a_read_that_names_the_node_the_account_is_taken_no_other_way() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = view_path();
    assert_eq!(
        unbound(&alice, &bob, &path).await,
        StatusCode::OK,
        "before bob names the node, his older reads are taken"
    );
    assert_eq!(
        bound(&alice, &bob, &path, &alices_id(&alice)).await,
        StatusCode::OK
    );
    assert_eq!(
        unbound(&alice, &bob, &path).await,
        StatusCode::NOT_FOUND,
        "the same account without the node's name"
    );
    assert_eq!(
        unbound(&alice, &bob, &about_path()).await,
        StatusCode::OK,
        "the read of about stays open: it is how the capability is learned"
    );
}

#[tokio::test]
async fn a_read_of_about_that_names_the_node_does_not_close_the_account_to_other_reads() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    assert_eq!(
        bound(&alice, &bob, &about_path(), &alices_id(&alice)).await,
        StatusCode::OK
    );
    assert_eq!(unbound(&alice, &bob, &view_path()).await, StatusCode::OK);
}

#[tokio::test]
async fn a_node_that_restarts_still_takes_no_unbound_read_from_that_account() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = view_path();
    assert_eq!(
        bound(&alice, &bob, &path, &alices_id(&alice)).await,
        StatusCode::OK
    );
    assert!(alice.data_dir().join("bound-readers.json").is_file());

    let reopened = agency_node::open(
        DataDir::new(alice.data_dir()),
        agency_node::Overrides {
            runtime: Some(memory_runtime().0),
            clock: Some(std::sync::Arc::new(alice.clock.clone())),
            root_keys: Some(alice.root_keys.clone()),
            ..agency_node::Overrides::default()
        },
    )
    .unwrap();
    assert!(
        reopened
            .node
            .store()
            .bound_readers()
            .unwrap()
            .contains(&id("bob")),
        "the account is kept in the data directory"
    );
    alice.clock.advance(Duration::seconds(1));
    let auth = sign_read(bob.node.key(), &id("bob"), "GET", &path, alice.node.now());
    let doc = reopened.node.store().load_org(&id("acme"));
    let refused = agency_core::read_auth::verify_read_once(
        &reopened.node,
        doc.as_ref().map(|doc| &doc.body),
        true,
        &auth,
        "GET",
        &path,
    );
    assert!(
        refused.is_err(),
        "a read that names no node, after a restart"
    );
}

#[tokio::test]
async fn a_node_set_to_require_the_name_takes_no_other_read_but_the_one_of_about() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    alice
        .node
        .update_config(|config| {
            config.federation = FederationConfig {
                require_read_audience: true,
                ..FederationConfig::default()
            }
        })
        .unwrap();
    assert_eq!(
        unbound(&alice, &bob, &view_path()).await,
        StatusCode::NOT_FOUND,
        "no ratchet yet, but the node requires the name"
    );
    assert_eq!(unbound(&alice, &bob, &about_path()).await, StatusCode::OK);
    assert_eq!(
        bound(&alice, &bob, &view_path(), &alices_id(&alice)).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn an_account_listed_as_unbound_is_taken_either_way() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = view_path();
    assert_eq!(
        bound(&alice, &bob, &path, &alices_id(&alice)).await,
        StatusCode::OK
    );
    assert_eq!(unbound(&alice, &bob, &path).await, StatusCode::NOT_FOUND);
    alice
        .node
        .update_config(|config| {
            config.federation = FederationConfig {
                require_read_audience: true,
                unbound_read_accounts: vec![id("bob")],
            }
        })
        .unwrap();
    assert_eq!(
        unbound(&alice, &bob, &path).await,
        StatusCode::OK,
        "bob's older node is let through although the ratchet and the requirement stand"
    );
}

#[tokio::test]
async fn a_node_names_a_peer_only_once_it_has_read_that_the_peer_lists_readaudience() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let acme = id("acme");
    let origin = alice.url();
    let path = view_path();
    let before = sign_for_peer(&bob.node, &acme, &origin, &path);
    assert_eq!(before.audience, None, "alice's about is not read yet");

    refresh(&bob.node).await.unwrap();
    let after = sign_for_peer(&bob.node, &acme, &origin, &path);
    assert_eq!(
        after.audience,
        Some(alices_id(&alice)),
        "alice lists readAudience"
    );
    let about = sign_for_peer(&bob.node, &acme, &origin, &about_path());
    assert_eq!(about.audience, None, "the read of about names no node");
}

#[tokio::test]
async fn a_member_node_that_has_read_the_peers_capabilities_reads_it_by_name() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    refresh(&bob.node).await.unwrap();
    refresh(&bob.node).await.unwrap();
    assert!(
        alice
            .node
            .store()
            .bound_readers()
            .unwrap()
            .contains(&id("bob")),
        "bob's node reads alice's team by naming it once it knows she lists readAudience"
    );
}

#[tokio::test]
async fn an_account_that_is_no_member_of_an_org_the_node_has_is_forgotten_when_a_definition_is_kept(
) {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    assert_eq!(
        bound(&alice, &bob, &view_path(), &alices_id(&alice)).await,
        StatusCode::OK
    );
    let mut kept = alice.node.store().bound_readers().unwrap();
    kept.insert(id("carol"));
    alice.node.store().save_bound_readers(&kept).unwrap();

    let department = agency_proto::org::Department {
        id: id("ops"),
        label: "Operations".into(),
        parent: None,
        head: None,
    };
    agency_core::departments::add(&alice.node, &id("acme"), department).unwrap();
    let after = alice.node.store().bound_readers().unwrap();
    assert!(after.contains(&id("bob")), "a member stays: {after:?}");
    assert!(!after.contains(&id("carol")), "no member: {after:?}");
}

#[tokio::test]
async fn a_peer_whose_kept_about_lacks_readaudience_is_read_the_older_way() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let acme = id("acme");
    let mut older = agency_core::about::this_node();
    older
        .capabilities
        .retain(|name| name != Capability::ReadAudience.name());
    let mut kept = BTreeMap::new();
    kept.insert(
        alice.url(),
        PeerAbout {
            about: Some(older),
            ..PeerAbout::default()
        },
    );
    bob.node.store().save_peer_abouts(&acme, &kept).unwrap();
    let auth = sign_for_peer(&bob.node, &acme, &alice.url(), &view_path());
    assert_eq!(auth.audience, None, "an older node is read the older way");
}

#[tokio::test]
async fn the_status_names_the_accounts_the_ratchet_holds_and_the_ones_let_through() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    assert!(
        status_lines(&alice.node).is_empty(),
        "nothing to say before anything changes"
    );
    assert_eq!(
        bound(&alice, &bob, &view_path(), &alices_id(&alice)).await,
        StatusCode::OK
    );
    let lines = status_lines(&alice.node);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("bob") && lines[0].contains("names no node"),
        "{lines:?}"
    );
    alice
        .node
        .update_config(|config| {
            config.federation = FederationConfig {
                unbound_read_accounts: vec![id("bob")],
                ..FederationConfig::default()
            }
        })
        .unwrap();
    let lines = status_lines(&alice.node);
    assert_eq!(lines.len(), 1, "bob is let through now: {lines:?}");
    assert!(lines[0].contains("federation.unboundReadAccounts") && lines[0].contains("bob"));
}

#[tokio::test]
async fn a_file_of_accounts_that_cannot_be_read_is_left_as_it_is_and_reads_are_still_answered() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    let path = alice.data_dir().join("bound-readers.json");
    std::fs::write(&path, "not json\n").unwrap();

    assert_eq!(
        bound(&alice, &bob, &view_path(), &alices_id(&alice)).await,
        StatusCode::OK,
        "a read that names the node is answered"
    );
    assert_eq!(
        unbound(&alice, &bob, &view_path()).await,
        StatusCode::OK,
        "an older read is taken as before, since nothing says the account named the node"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "not json\n",
        "the file that cannot be read is not written over"
    );
    let lines = status_lines(&alice.node);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("bound-readers.json cannot be read"),
        "{lines:?}"
    );

    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        bound(&alice, &bob, &view_path(), &alices_id(&alice)).await,
        StatusCode::OK
    );
    assert!(alice
        .node
        .store()
        .bound_readers()
        .unwrap()
        .contains(&id("bob")));
}
